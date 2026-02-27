//! Prevents context window overflow by trimming conversation history.
//! SAVINGS: Preventative — prevents model errors from long context
//! STAGE: PreCall (priority 20)

use dx_core::*;
use std::sync::Mutex;

pub struct TokenBudgetSaver {
    config: BudgetConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct BudgetConfig {
    pub max_context_tokens: usize,
    pub output_reserve: usize,
    pub warning_threshold: f64,
    pub hard_limit_threshold: f64,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            max_context_tokens: 128_000,
            output_reserve: 8_000,
            warning_threshold: 0.75,
            hard_limit_threshold: 0.90,
        }
    }
}

impl TokenBudgetSaver {
    pub fn new(config: BudgetConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(BudgetConfig::default())
    }

    pub fn count_tokens(text: &str) -> usize {
        text.len() / 4
    }
}

#[async_trait::async_trait]
impl TokenSaver for TokenBudgetSaver {
    fn name(&self) -> &str { "token-budget" }
    fn stage(&self) -> SaverStage { SaverStage::PreCall }
    fn priority(&self) -> u32 { 20 }

    async fn process(&self, mut input: SaverInput, ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let effective_max = ctx.token_budget
            .unwrap_or(self.config.max_context_tokens)
            .saturating_sub(self.config.output_reserve);

        let total: usize = input.messages.iter().map(|m| Self::count_tokens(&m.content)).sum::<usize>()
            + input.tools.iter().map(|t| t.definition_tokens).sum::<usize>();

        if (total as f64 / effective_max as f64) < self.config.warning_threshold {
            return Ok(SaverOutput {
                messages: input.messages,
                tools: input.tools,
                images: input.images,
                skipped: false,
                cached_response: None,
            });
        }

        let hard_limit = (effective_max as f64 * self.config.hard_limit_threshold) as usize;
        let before_total = total;

        // Remove oldest non-system messages until under hard limit
        let mut running = total;
        loop {
            if running <= hard_limit { break; }
            let oldest_non_sys = input.messages.iter().position(|m| m.role != "system");
            match oldest_non_sys {
                Some(idx) => {
                    let t = Self::count_tokens(&input.messages[idx].content);
                    running = running.saturating_sub(t);
                    input.messages.remove(idx);
                }
                None => break,
            }
        }

        let saved = before_total.saturating_sub(running);
        if saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "token-budget".into(),
                tokens_before: before_total,
                tokens_after: running,
                tokens_saved: saved,
                description: format!(
                    "trimmed history: {} → {} tokens (limit {})",
                    before_total, running, effective_max
                ),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_tokens() {
        assert_eq!(TokenBudgetSaver::count_tokens("hello"), 1);
        assert_eq!(TokenBudgetSaver::count_tokens("a".repeat(400).as_str()), 100);
    }
}
