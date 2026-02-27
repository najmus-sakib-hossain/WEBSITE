//! Caps retrieval payloads to prevent context bloat.
//! SAVINGS: 60-90% on retrieval context tokens
//! STAGE: PrePrompt (priority 15)

use dx_core::*;
use std::sync::Mutex;

pub struct RetrievalBudgetSaver {
    config: RetrievalConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct RetrievalConfig {
    pub max_tokens: usize,
    pub max_results: usize,
    pub min_result_tokens: usize,
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            max_tokens: 8000,
            max_results: 10,
            min_result_tokens: 20,
        }
    }
}

impl RetrievalBudgetSaver {
    pub fn new(config: RetrievalConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(RetrievalConfig::default())
    }

    fn is_retrieval_message(msg: &Message) -> bool {
        msg.content.contains("[retrieved]")
            || msg.content.contains("[context]")
            || msg.content.contains("[search result]")
            || msg.content.contains("[file chunk]")
    }
}

#[async_trait::async_trait]
impl TokenSaver for RetrievalBudgetSaver {
    fn name(&self) -> &str { "retrieval-budget" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 15 }

    async fn process(&self, input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut budget = self.config.max_tokens;
        let mut count = 0usize;
        let mut total_before = 0usize;
        let mut total_after = 0usize;

        let mut kept = Vec::new();

        for msg in input.messages {
            if Self::is_retrieval_message(&msg) {
                total_before += msg.token_count;

                if count >= self.config.max_results { continue; }
                if msg.token_count < self.config.min_result_tokens { continue; }
                if msg.token_count > budget { continue; }

                budget = budget.saturating_sub(msg.token_count);
                count += 1;
                total_after += msg.token_count;
                kept.push(msg);
            } else {
                kept.push(msg);
            }
        }

        let saved = total_before.saturating_sub(total_after);
        if saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "retrieval-budget".into(),
                tokens_before: total_before,
                tokens_after: total_after,
                tokens_saved: saved,
                description: format!(
                    "capped retrieval: {} results, {} → {} tokens",
                    count, total_before, total_after
                ),
            };
        }

        Ok(SaverOutput {
            messages: kept,
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
