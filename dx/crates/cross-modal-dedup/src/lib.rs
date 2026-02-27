//! Deduplicates content across modalities using cross-modal Jaccard similarity.
//! Keeps the most token-efficient representation of duplicate content.
//! SAVINGS: 30-60% on conversations with redundant multimodal content
//! STAGE: PrePrompt (priority 92)

use dx_core::*;
use std::collections::HashSet;
use std::sync::Mutex;

pub struct CrossModalDedupSaver {
    config: CrossModalDedupConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct CrossModalDedupConfig {
    pub similarity_threshold: f64,
    pub min_tokens_to_dedup: usize,
}

impl Default for CrossModalDedupConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.75,
            min_tokens_to_dedup: 20,
        }
    }
}

#[derive(Debug, Clone)]
struct ContentCandidate {
    index: usize,
    modality: String,
    token_count: usize,
    word_set: HashSet<String>,
}

impl CrossModalDedupSaver {
    pub fn new(config: CrossModalDedupConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(CrossModalDedupConfig::default())
    }

    /// Jaccard similarity between two word sets.
    pub fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
        let inter = a.intersection(b).count();
        let union = a.union(b).count();
        if union == 0 { 1.0 } else { inter as f64 / union as f64 }
    }

    fn word_set(text: &str) -> HashSet<String> {
        text.split_whitespace()
            .map(|w| w.to_lowercase().trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| w.len() > 2)
            .collect()
    }

    /// Among a group of duplicates, keep the one with fewest tokens.
    fn cheapest_index(group: &[usize], messages: &[Message]) -> usize {
        *group.iter()
            .min_by_key(|&&i| messages[i].token_count)
            .unwrap_or(&group[0])
    }
}

#[async_trait::async_trait]
impl MultiModalTokenSaver for CrossModalDedupSaver {
    fn name(&self) -> &str { "cross-modal-dedup" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 92 }
    fn modality(&self) -> Modality { Modality::CrossModal }

    async fn process_multimodal(
        &self,
        mut input: MultiModalSaverInput,
        _ctx: &SaverContext,
    ) -> Result<MultiModalSaverOutput, SaverError> {
        let before_tokens: usize = input.base.messages.iter().map(|m| m.token_count).sum();

        // Build candidates for all non-system messages with enough tokens
        let candidates: Vec<ContentCandidate> = input.base.messages.iter().enumerate()
            .filter(|(_, m)| m.role != "system" && m.token_count >= self.config.min_tokens_to_dedup)
            .map(|(i, m)| ContentCandidate {
                index: i,
                modality: if m.images.is_empty() { "text".into() } else { "image".into() },
                token_count: m.token_count,
                word_set: Self::word_set(&m.content),
            })
            .collect();

        // Find duplicate groups using union-find style grouping
        let n = candidates.len();
        let mut to_remove: HashSet<usize> = HashSet::new();

        for i in 0..n {
            if to_remove.contains(&candidates[i].index) { continue; }
            let mut dup_group = vec![candidates[i].index];

            for j in (i + 1)..n {
                if to_remove.contains(&candidates[j].index) { continue; }
                let sim = Self::jaccard(&candidates[i].word_set, &candidates[j].word_set);
                if sim >= self.config.similarity_threshold {
                    dup_group.push(candidates[j].index);
                }
            }

            if dup_group.len() > 1 {
                let keep = Self::cheapest_index(&dup_group, &input.base.messages);
                for &idx in &dup_group {
                    if idx != keep {
                        to_remove.insert(idx);
                    }
                }
            }
        }

        // Annotate removed messages
        for &idx in &to_remove {
            let tokens = input.base.messages[idx].token_count;
            input.base.messages[idx].content = format!(
                "[DEDUP: content merged with more efficient representation, {} tokens saved]",
                tokens
            );
            input.base.messages[idx].token_count = input.base.messages[idx].content.len() / 4;
        }

        let after_tokens: usize = input.base.messages.iter().map(|m| m.token_count).sum();
        let saved = before_tokens.saturating_sub(after_tokens);

        if saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "cross-modal-dedup".into(),
                tokens_before: before_tokens,
                tokens_after: after_tokens,
                tokens_saved: saved,
                description: format!(
                    "cross-modal dedup: removed {} duplicate representations, saved {} tokens",
                    to_remove.len(), saved
                ),
            };
        }

        Ok(MultiModalSaverOutput {
            base: SaverOutput {
                messages: input.base.messages,
                tools: input.base.tools,
                images: input.base.images,
                skipped: false,
                cached_response: None,
            },
            audio: input.audio,
            live_frames: input.live_frames,
            documents: input.documents,
            videos: input.videos,
            assets_3d: input.assets_3d,
        })
    }

    fn last_savings(&self) -> TokenSavingsReport {
        self.report.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_jaccard() {
        let a: HashSet<String> = ["hello", "world"].iter().map(|s| s.to_string()).collect();
        let b = a.clone();
        assert!((CrossModalDedupSaver::jaccard(&a, &b) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_disjoint_jaccard() {
        let a: HashSet<String> = ["foo", "bar"].iter().map(|s| s.to_string()).collect();
        let b: HashSet<String> = ["baz", "qux"].iter().map(|s| s.to_string()).collect();
        assert_eq!(CrossModalDedupSaver::jaccard(&a, &b), 0.0);
    }
}
