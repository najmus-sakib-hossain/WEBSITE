//! Deduplicates identical tool outputs in conversation history.
//! SAVINGS: 20-50% in multi-turn sessions with repeated reads
//! STAGE: InterTurn (priority 10)

use dx_core::*;
use std::collections::HashMap;
use std::sync::Mutex;

pub struct DedupSaver {
    min_content_length: usize,
    report: Mutex<TokenSavingsReport>,
}

impl DedupSaver {
    pub fn new() -> Self {
        Self {
            min_content_length: 200,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_min_length(min: usize) -> Self {
        Self {
            min_content_length: min,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }
}

impl Default for DedupSaver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TokenSaver for DedupSaver {
    fn name(&self) -> &str { "dedup" }
    fn stage(&self) -> SaverStage { SaverStage::InterTurn }
    fn priority(&self) -> u32 { 10 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut seen: HashMap<blake3::Hash, usize> = HashMap::new();
        let mut total_saved = 0usize;

        for (i, msg) in input.messages.iter_mut().enumerate() {
            if !(msg.role == "tool" || msg.tool_call_id.is_some()) {
                continue;
            }
            if msg.content.len() < self.min_content_length {
                continue;
            }

            let hash = blake3::hash(msg.content.as_bytes());

            if let Some(first_idx) = seen.get(&hash) {
                let saved = msg.token_count;
                msg.content = format!("[identical to tool output at message {}]", first_idx);
                msg.token_count = 10;
                total_saved += saved.saturating_sub(10);
            } else {
                seen.insert(hash, i);
            }
        }

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "dedup".into(),
                tokens_before: total_saved + 10,
                tokens_after: 10,
                tokens_saved: total_saved,
                description: format!("deduplicated {} tokens of repeated tool outputs", total_saved),
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
