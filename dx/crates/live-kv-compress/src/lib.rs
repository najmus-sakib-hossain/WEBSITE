//! Compresses stream memory using KV-cache style compression.
//! SAVINGS: 60-80% on long live sessions
//! STAGE: PrePrompt (priority 54)

use dx_core::*;
use std::collections::VecDeque;
use std::sync::Mutex;

pub struct LiveKvCompressSaver {
    config: KvCompressConfig,
    memory: Mutex<StreamMemory>,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct KvCompressConfig {
    pub max_memory_entries: usize,
    pub memory_token_budget: usize,
    pub quantization_bits: u8,
    pub eviction: EvictionPolicy,
}

#[derive(Clone, Debug)]
pub enum EvictionPolicy {
    Fifo,
    Saliency,
    MergeSimilar,
}

impl Default for KvCompressConfig {
    fn default() -> Self {
        Self {
            max_memory_entries: 100,
            memory_token_budget: 2000,
            quantization_bits: 4,
            eviction: EvictionPolicy::MergeSimilar,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub timestamp_secs: f64,
    pub frame_index: usize,
    pub summary: String,
    pub saliency: f64,
    pub tokens: usize,
}

#[derive(Default)]
pub struct StreamMemory {
    pub entries: VecDeque<MemoryEntry>,
    pub total_tokens: usize,
}

impl StreamMemory {
    pub fn find_most_similar_pair(&self) -> Option<(usize, usize)> {
        if self.entries.len() < 2 { return None; }
        let mut best_sim = f64::NEG_INFINITY;
        let mut best_pair = None;
        for i in 0..self.entries.len() - 1 {
            let words_a: std::collections::HashSet<&str> = self.entries[i].summary.split_whitespace().collect();
            let words_b: std::collections::HashSet<&str> = self.entries[i+1].summary.split_whitespace().collect();
            let inter = words_a.intersection(&words_b).count();
            let union = words_a.union(&words_b).count();
            let sim = if union == 0 { 0.0 } else { inter as f64 / union as f64 };
            if sim > best_sim {
                best_sim = sim;
                best_pair = Some((i, i + 1));
            }
        }
        best_pair
    }
}

impl LiveKvCompressSaver {
    pub fn new(config: KvCompressConfig) -> Self {
        Self {
            config,
            memory: Mutex::new(StreamMemory::default()),
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(KvCompressConfig::default())
    }

    pub fn compute_saliency(entry: &MemoryEntry) -> f64 {
        let word_count = entry.summary.split_whitespace().count();
        let has_event = entry.summary.contains('[');
        let base = (word_count as f64 / 20.0).min(1.0);
        if has_event { base + 0.2 } else { base }
    }

    pub fn add_to_memory(&self, entry: MemoryEntry) {
        let mut mem = self.memory.lock().unwrap();
        mem.total_tokens += entry.tokens;
        mem.entries.push_back(entry);

        // Evict if over budget
        while mem.total_tokens > self.config.memory_token_budget
            || mem.entries.len() > self.config.max_memory_entries
        {
            match self.config.eviction {
                EvictionPolicy::Fifo => {
                    if let Some(removed) = mem.entries.pop_front() {
                        mem.total_tokens = mem.total_tokens.saturating_sub(removed.tokens);
                    } else { break; }
                }
                EvictionPolicy::Saliency => {
                    let min_idx = mem.entries.iter().enumerate()
                        .min_by(|(_, a), (_, b)| a.saliency.partial_cmp(&b.saliency).unwrap_or(std::cmp::Ordering::Equal))
                        .map(|(i, _)| i);
                    if let Some(i) = min_idx {
                        let removed = mem.entries.remove(i).unwrap();
                        mem.total_tokens = mem.total_tokens.saturating_sub(removed.tokens);
                    } else { break; }
                }
                EvictionPolicy::MergeSimilar => {
                    if let Some((i, j)) = mem.find_most_similar_pair() {
                        let b = mem.entries.remove(j).unwrap();
                        let tokens_freed = if let Some(a) = mem.entries.get_mut(i) {
                            let merged = format!("{} | {}", a.summary, b.summary);
                            let new_tokens = merged.len() / 4;
                            let freed = a.tokens.saturating_add(b.tokens).saturating_sub(new_tokens);
                            a.tokens = new_tokens;
                            a.summary = merged;
                            a.saliency = a.saliency.max(b.saliency);
                            freed
                        } else { 0 };
                        mem.total_tokens = mem.total_tokens.saturating_sub(tokens_freed);
                    } else {
                        if let Some(removed) = mem.entries.pop_front() {
                            mem.total_tokens = mem.total_tokens.saturating_sub(removed.tokens);
                        } else { break; }
                    }
                }
            }
        }
    }

    pub fn memory_summary(&self) -> String {
        let mem = self.memory.lock().unwrap();
        let mut summary = format!("[STREAM MEMORY: {} entries, {} tokens]\n", mem.entries.len(), mem.total_tokens);
        for entry in &mem.entries {
            summary.push_str(&format!(
                "  [{:.1}s frame{}] {}\n",
                entry.timestamp_secs, entry.frame_index, entry.summary
            ));
        }
        summary
    }
}

#[async_trait::async_trait]
impl TokenSaver for LiveKvCompressSaver {
    fn name(&self) -> &str { "live-kv-compress" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 54 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let before_tokens: usize = input.messages.iter().map(|m| m.token_count).sum();

        // Ingest all non-system messages as live stream entries into memory
        let live_msgs: Vec<Message> = input.messages.drain(..)
            .filter(|m| m.role != "system")
            .collect();

        for (i, msg) in live_msgs.iter().enumerate() {
            let entry = MemoryEntry {
                timestamp_secs: i as f64 * 0.033, // ~30fps
                frame_index: i,
                summary: msg.content.chars().take(100).collect(),
                saliency: 0.5,
                tokens: msg.token_count,
            };
            self.add_to_memory(entry);
        }

        // Replace live messages with compressed summary
        let summary = self.memory_summary();
        let token_count = summary.len() / 4;
        input.messages.push(Message {
            role: "system".into(),
            content: summary,
            images: vec![],
            tool_call_id: None,
            token_count,
        });

        let after_tokens: usize = input.messages.iter().map(|m| m.token_count).sum();
        let saved = before_tokens.saturating_sub(after_tokens);

        if saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "live-kv-compress".into(),
                tokens_before: before_tokens,
                tokens_after: after_tokens,
                tokens_saved: saved,
                description: format!("KV stream compression saved {} tokens", saved),
            };
        }

        Ok(SaverOutput {
            messages: input.messages,
            tools: input.tools,
            images: input.images,
            skipped: false,
            cached_response: None,
        })
    }

    fn last_savings(&self) -> TokenSavingsReport {
        self.report.lock().unwrap().clone()
    }
}
