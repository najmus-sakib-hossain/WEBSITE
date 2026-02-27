//! Routes tasks to appropriate reasoning effort levels.
//! Simple reads → low effort. Complex debugging → high effort.
//! SAVINGS: 30-80% on reasoning tokens
//! STAGE: PreCall (priority 10)

use dx_core::*;
use std::sync::Mutex;

pub struct ReasoningRouterSaver {
    rules: Vec<ReasoningRule>,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Effort {
    None,
    Low,
    Medium,
    High,
}

#[derive(Clone)]
pub struct ReasoningRule {
    pub pattern: Pattern,
    pub effort: Effort,
}

#[derive(Clone)]
pub enum Pattern {
    /// Match if task/message contains any of these strings
    ContainsAny(Vec<String>),
    /// Match if turn number <= given value
    EarlyTurn(usize),
}

impl ReasoningRouterSaver {
    pub fn new() -> Self {
        Self {
            rules: Self::default_rules(),
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_rules(rules: Vec<ReasoningRule>) -> Self {
        Self {
            rules,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    fn default_rules() -> Vec<ReasoningRule> {
        vec![
            // Simple reads/lists → low reasoning
            ReasoningRule {
                pattern: Pattern::ContainsAny(vec![
                    "read".into(), "list".into(), "show".into(), "display".into(),
                    "print".into(), "cat ".into(), "ls ".into(),
                ]),
                effort: Effort::Low,
            },
            // Debug/fix/error → high reasoning
            ReasoningRule {
                pattern: Pattern::ContainsAny(vec![
                    "debug".into(), "fix".into(), "error".into(), "panic".into(),
                    "bug".into(), "broken".into(), "crash".into(), "fail".into(),
                ]),
                effort: Effort::High,
            },
            // Retry → high reasoning
            ReasoningRule {
                pattern: Pattern::ContainsAny(vec![
                    "retry".into(), "try again".into(), "again".into(),
                    "doesn't work".into(), "not working".into(),
                ]),
                effort: Effort::High,
            },
            // Early turns → medium by default via EarlyTurn
            ReasoningRule {
                pattern: Pattern::EarlyTurn(3),
                effort: Effort::Medium,
            },
        ]
    }

    /// Classify a task context and return recommended effort level.
    pub fn classify(&self, ctx: &SaverContext) -> Effort {
        let combined = format!(
            "{} {}",
            ctx.task_description.to_lowercase(),
            ctx.model.to_lowercase()
        );

        // Short messages → low effort
        if ctx.task_description.len() < 20 {
            return Effort::Low;
        }

        for rule in &self.rules {
            let matches = match &rule.pattern {
                Pattern::ContainsAny(kws) => kws.iter().any(|kw| combined.contains(kw.as_str())),
                Pattern::EarlyTurn(max) => ctx.turn_number <= *max,
            };
            if matches {
                return rule.effort;
            }
        }

        Effort::Medium
    }
}

impl Effort {
    pub fn to_openai(&self) -> &'static str {
        match self {
            Effort::None => "none",
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
        }
    }

    pub fn estimated_tokens(&self) -> usize {
        match self {
            Effort::None => 0,
            Effort::Low => 200,
            Effort::Medium => 1500,
            Effort::High => 8000,
        }
    }
}

impl Default for ReasoningRouterSaver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TokenSaver for ReasoningRouterSaver {
    fn name(&self) -> &str { "reasoning-router" }
    fn stage(&self) -> SaverStage { SaverStage::PreCall }
    fn priority(&self) -> u32 { 10 }

    async fn process(&self, input: SaverInput, ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let effort = self.classify(ctx);
        let default_tokens = Effort::High.estimated_tokens();
        let actual_tokens = effort.estimated_tokens();
        let saved = default_tokens.saturating_sub(actual_tokens);

        if saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "reasoning-router".into(),
                tokens_before: default_tokens,
                tokens_after: actual_tokens,
                tokens_saved: saved,
                description: format!(
                    "routed to {:?} effort (est. {} → {} reasoning tokens)",
                    effort, default_tokens, actual_tokens
                ),
            };
        }

        let mut messages = input.messages;

        // Annotate last user message with reasoning level
        if let Some(last_user) = messages.iter_mut().rfind(|m| m.role == "user") {
            let annotation = format!(" [reasoning: {}]", effort.to_openai());
            last_user.content.push_str(&annotation);
            last_user.token_count = last_user.content.len() / 4;
        }

        Ok(SaverOutput {
            messages,
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

    fn ctx(desc: &str, turn: usize) -> SaverContext {
        SaverContext {
            task_description: desc.into(),
            turn_number: turn,
            model: "gpt-4o".into(),
            token_budget: None,
        }
    }

    #[test]
    fn test_file_read_gets_low() {
        let router = ReasoningRouterSaver::new();
        assert_eq!(router.classify(&ctx("read the main.rs file and show me its contents", 5)), Effort::Low);
    }

    #[test]
    fn test_retry_gets_high() {
        let router = ReasoningRouterSaver::new();
        assert_eq!(router.classify(&ctx("retry the last operation that failed with an error", 5)), Effort::High);
    }

    #[test]
    fn test_short_message_gets_low() {
        let router = ReasoningRouterSaver::new();
        assert_eq!(router.classify(&ctx("ok", 5)), Effort::Low);
    }
}
