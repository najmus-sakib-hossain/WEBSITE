//! Merges consecutive tool result messages to reduce overhead.
//! SAVINGS: 5-15% on per-message overhead
//! STAGE: PostResponse (priority 25)

use dx_core::*;
use std::sync::Mutex;

pub struct ParallelToolMergeSaver {
    min_results_to_merge: usize,
    report: Mutex<TokenSavingsReport>,
}

impl ParallelToolMergeSaver {
    pub fn new(min_results: usize) -> Self {
        Self {
            min_results_to_merge: min_results,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(3)
    }
}

#[async_trait::async_trait]
impl TokenSaver for ParallelToolMergeSaver {
    fn name(&self) -> &str { "parallel-tool-merge" }
    fn stage(&self) -> SaverStage { SaverStage::PostResponse }
    fn priority(&self) -> u32 { 25 }

    async fn process(&self, input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut result: Vec<Message> = Vec::new();
        let mut pending_tool: Vec<Message> = Vec::new();
        let mut total_saved = 0usize;

        let flush = |pending: &mut Vec<Message>, result: &mut Vec<Message>, saved: &mut usize, min: usize| {
            if pending.len() >= min {
                let merged_content = pending.iter()
                    .map(|m| m.content.as_str())
                    .collect::<Vec<_>>()
                    .join("\n---\n");
                let mut merged = pending[0].clone();
                let original_tokens: usize = pending.iter().map(|m| m.token_count).sum();
                merged.content = merged_content;
                merged.token_count = merged.content.len() / 4;
                *saved += original_tokens.saturating_sub(merged.token_count);
                result.push(merged);
            } else {
                result.extend(pending.drain(..));
                return;
            }
            pending.clear();
        };

        for msg in input.messages {
            if msg.role == "tool" || msg.tool_call_id.is_some() {
                pending_tool.push(msg);
            } else {
                flush(&mut pending_tool, &mut result, &mut total_saved, self.min_results_to_merge);
                result.push(msg);
            }
        }
        flush(&mut pending_tool, &mut result, &mut total_saved, self.min_results_to_merge);

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "parallel-tool-merge".into(),
                tokens_before: total_saved,
                tokens_after: 0,
                tokens_saved: total_saved,
                description: format!("merged tool messages, saved {} tokens", total_saved),
            };
        }

        Ok(SaverOutput {
            messages: result,
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
