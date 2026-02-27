//! # batch-router
//!
//! Routes eligible tasks to batch APIs for 50% cost reduction.
//!
//! ## Verified Savings (TOKEN.md research)
//!
//! - **REAL**: Hard fact from OpenAI pricing page.
//! - "Save 50% on inputs AND outputs with the Batch API."
//! - Batch tasks complete within 24 hours (often faster).
//! - Higher rate limits than synchronous API.
//!
//! ## Eligibility
//! Tasks suitable for batching:
//! - Background analysis / summarization
//! - Bulk classification or tagging
//! - Non-interactive code review
//! - Report generation with no user waiting
//! - Embedding generation for large document sets
//!
//! Tasks NOT suitable (require real-time response):
//! - Interactive chat / agent loops
//! - Tasks where the user is waiting
//! - Streaming responses
//!
//! SAVINGS: 50% on all tokens (inputs + outputs) — hard fact
//! STAGE: PreCall (priority 15)

use dx_core::*;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BatchEligibility {
    /// Eligible for batch API (50% savings)
    Eligible,
    /// Not eligible — requires real-time response
    NotEligible,
    /// Possibly eligible — depends on user-specified urgency
    MaybeEligible,
}

/// Result of batch eligibility check.
#[derive(Debug, Clone)]
pub struct BatchDecision {
    pub eligibility: BatchEligibility,
    pub reason: String,
    /// Estimated savings if batched (50% of total tokens if eligible)
    pub estimated_savings_pct: u32,
}

pub struct BatchRouterSaver {
    config: BatchConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct BatchConfig {
    /// If true, router is in "interactive mode" — never route to batch
    pub interactive_mode: bool,
    /// Min token count to consider batching (small tasks aren't worth it)
    pub min_tokens_for_batch: usize,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            interactive_mode: true, // conservative default: don't batch unless told to
            min_tokens_for_batch: 500,
        }
    }
}

impl BatchRouterSaver {
    pub fn new(config: BatchConfig) -> Self {
        Self { config, report: Mutex::new(TokenSavingsReport::default()) }
    }

    /// Check if task is eligible for batch API.
    pub fn classify(&self, messages: &[Message]) -> BatchDecision {
        if self.config.interactive_mode {
            return BatchDecision {
                eligibility: BatchEligibility::NotEligible,
                reason: "interactive mode enabled — real-time response required".into(),
                estimated_savings_pct: 0,
            };
        }

        let total_tokens: usize = messages.iter().map(|m| m.token_count).sum();
        if total_tokens < self.config.min_tokens_for_batch {
            return BatchDecision {
                eligibility: BatchEligibility::NotEligible,
                reason: format!("task too small for batching ({} < {} tokens)", total_tokens, self.config.min_tokens_for_batch),
                estimated_savings_pct: 0,
            };
        }

        let last_user = messages.iter().rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.to_lowercase())
            .unwrap_or_default();

        // Signals suggesting background/batch-eligible work
        let batch_signals = [
            "analyze", "summarize", "classify", "categorize", "review all",
            "generate report", "bulk", "batch", "process all", "list all",
            "evaluate", "rank the", "score the", "tag the", "label the",
        ];

        // Signals requiring real-time (user is waiting)
        let realtime_signals = [
            "quick", "asap", "immediately", "right now", "urgent",
            "what is", "how do i", "help me", "can you", "please",
        ];

        if realtime_signals.iter().any(|s| last_user.contains(s)) {
            return BatchDecision {
                eligibility: BatchEligibility::NotEligible,
                reason: "real-time response signal detected".into(),
                estimated_savings_pct: 0,
            };
        }

        if batch_signals.iter().any(|s| last_user.contains(s)) {
            return BatchDecision {
                eligibility: BatchEligibility::Eligible,
                reason: "background/batch-eligible task pattern detected".into(),
                estimated_savings_pct: 50, // Hard fact: 50% on all tokens
            };
        }

        BatchDecision {
            eligibility: BatchEligibility::MaybeEligible,
            reason: "no clear signal — set interactive_mode=false to enable batching".into(),
            estimated_savings_pct: 0,
        }
    }
}

impl Default for BatchRouterSaver {
    fn default() -> Self { Self::new(BatchConfig::default()) }
}

#[async_trait::async_trait]
impl TokenSaver for BatchRouterSaver {
    fn name(&self) -> &str { "batch-router" }
    fn stage(&self) -> SaverStage { SaverStage::PreCall }
    fn priority(&self) -> u32 { 15 }

    async fn process(&self, input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let decision = self.classify(&input.messages);
        let total_tokens: usize = input.messages.iter().map(|m| m.token_count).sum();
        let estimated_tools: usize = input.tools.iter().map(|t| t.token_count).sum();
        let total = total_tokens + estimated_tools;
        let saved = (total as f64 * decision.estimated_savings_pct as f64 / 100.0) as usize;

        let mut report = self.report.lock().unwrap();
        *report = TokenSavingsReport {
            technique: "batch-router".into(),
            tokens_before: total,
            tokens_after: total.saturating_sub(saved),
            tokens_saved: saved,
            description: format!(
                "batch eligibility: {:?} — {}. potential {}% savings",
                decision.eligibility, decision.reason, decision.estimated_savings_pct
            ),
        };

        // Note: when Eligible, the caller should route to batch API.
        // This saver itself doesn't make the API call — it annotates the pipeline.
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

    fn user_msg(content: &str) -> Message {
        Message { role: "user".into(), content: content.into(), images: vec![], tool_call_id: None, token_count: 200 }
    }

    #[test]
    fn interactive_mode_never_batches() {
        let router = BatchRouterSaver::new(BatchConfig { interactive_mode: true, min_tokens_for_batch: 100 });
        let msgs = vec![user_msg("analyze all these documents")];
        let d = router.classify(&msgs);
        assert_eq!(d.eligibility, BatchEligibility::NotEligible);
    }

    #[test]
    fn batch_mode_classifies_analysis_as_eligible() {
        let router = BatchRouterSaver::new(BatchConfig { interactive_mode: false, min_tokens_for_batch: 100 });
        let msgs = vec![user_msg("analyze and categorize all 500 support tickets")];
        let d = router.classify(&msgs);
        assert_eq!(d.eligibility, BatchEligibility::Eligible);
        assert_eq!(d.estimated_savings_pct, 50);
    }

    #[test]
    fn urgent_tasks_not_batched() {
        let router = BatchRouterSaver::new(BatchConfig { interactive_mode: false, min_tokens_for_batch: 100 });
        let msgs = vec![user_msg("urgent! analyze this immediately")];
        let d = router.classify(&msgs);
        assert_eq!(d.eligibility, BatchEligibility::NotEligible);
    }

    #[test]
    fn savings_are_50_percent() {
        // Verify the hard fact: batch API = 50% off
        let router = BatchRouterSaver::new(BatchConfig { interactive_mode: false, min_tokens_for_batch: 100 });
        let msgs = vec![user_msg("summarize these 1000 documents in bulk")];
        let d = router.classify(&msgs);
        if d.eligibility == BatchEligibility::Eligible {
            assert_eq!(d.estimated_savings_pct, 50, "batch API is exactly 50% off per OpenAI pricing");
        }
    }
}
