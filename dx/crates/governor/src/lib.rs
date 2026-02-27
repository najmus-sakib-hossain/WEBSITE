//! # governor
//!
//! Circuit breaker for tool calls. Prevents runaway spirals where the
//! model calls the same tool repeatedly, makes duplicate calls, or
//! exhausts budgets that burn tokens without progress.
//!
//! SAVINGS: prevents 20-100+ wasted tool calls per session
//! STAGE: PreCall (priority 5)

use dx_core::*;
use std::collections::HashMap;
use std::sync::Mutex;

pub struct GovernorSaver {
    config: GovernorConfig,
    state: Mutex<GovernorState>,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct GovernorConfig {
    /// Max tool calls per single LLM response
    pub max_per_response: usize,
    /// Max total tool calls per task/session
    pub max_per_task: usize,
    /// Max consecutive calls to the same tool
    pub max_consecutive_same: usize,
    /// Deduplicate identical tool calls (same name + same args)
    pub dedupe_identical: bool,
    /// Max tokens allowed in a single tool output before truncation flag
    pub max_output_tokens: usize,
}

struct GovernorState {
    total_calls: usize,
    response_calls: usize,
    call_signatures: Vec<(String, blake3::Hash)>,
    last_tool: Option<String>,
    consecutive_count: usize,
    blocked_calls: usize,
    blocked_tokens_saved: usize,
}

impl Default for GovernorConfig {
    fn default() -> Self {
        Self {
            max_per_response: 10,
            max_per_task: 50,
            max_consecutive_same: 3,
            dedupe_identical: true,
            max_output_tokens: 4000,
        }
    }
}

#[derive(Debug, Clone)]
pub enum GovernorDecision {
    Allow,
    Block(String),
    AllowWithWarning(String),
}

impl GovernorSaver {
    pub fn new(config: GovernorConfig) -> Self {
        Self {
            config,
            state: Mutex::new(GovernorState {
                total_calls: 0,
                response_calls: 0,
                call_signatures: Vec::new(),
                last_tool: None,
                consecutive_count: 0,
                blocked_calls: 0,
                blocked_tokens_saved: 0,
            }),
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(GovernorConfig::default())
    }

    /// Reset per-response counters (call at start of each new LLM response)
    pub fn reset_response(&self) {
        let mut state = self.state.lock().unwrap();
        state.response_calls = 0;
    }

    /// Check if a specific tool call should be allowed.
    pub fn check_call(&self, tool_name: &str, args: &serde_json::Value) -> GovernorDecision {
        let state = self.state.lock().unwrap();

        // 1. Task budget
        if state.total_calls >= self.config.max_per_task {
            return GovernorDecision::Block(format!(
                "task tool budget exhausted ({}/{})",
                state.total_calls, self.config.max_per_task
            ));
        }

        // 2. Response budget
        if state.response_calls >= self.config.max_per_response {
            return GovernorDecision::Block(format!(
                "response tool budget exhausted ({}/{})",
                state.response_calls, self.config.max_per_response
            ));
        }

        // 3. Consecutive same tool
        if state.last_tool.as_deref() == Some(tool_name)
            && state.consecutive_count >= self.config.max_consecutive_same
        {
            return GovernorDecision::Block(format!(
                "too many consecutive '{}' calls ({}/{})",
                tool_name, state.consecutive_count, self.config.max_consecutive_same
            ));
        }

        // 4. Duplicate detection
        if self.config.dedupe_identical {
            let args_str = serde_json::to_string(args).unwrap_or_default();
            let hash = blake3::hash(format!("{}:{}", tool_name, args_str).as_bytes());
            if state.call_signatures.iter().any(|(n, h)| n == tool_name && h == &hash) {
                return GovernorDecision::Block("duplicate call (same tool + same args)".into());
            }
        }

        // 5. Warning zone (approaching limits)
        let total_pct = state.total_calls as f64 / self.config.max_per_task as f64;
        if total_pct > 0.8 {
            return GovernorDecision::AllowWithWarning(format!(
                "tool budget at {:.0}%", total_pct * 100.0
            ));
        }

        GovernorDecision::Allow
    }

    /// Record that a tool call was made
    pub fn record_call(&self, tool_name: &str, args: &serde_json::Value) {
        let mut state = self.state.lock().unwrap();
        state.total_calls += 1;
        state.response_calls += 1;

        // Update consecutive counter
        if state.last_tool.as_deref() == Some(tool_name) {
            state.consecutive_count += 1;
        } else {
            state.consecutive_count = 1;
            state.last_tool = Some(tool_name.to_string());
        }

        // Record signature for dedup
        if self.config.dedupe_identical {
            let args_str = serde_json::to_string(args).unwrap_or_default();
            let hash = blake3::hash(format!("{}:{}", tool_name, args_str).as_bytes());
            state.call_signatures.push((tool_name.to_string(), hash));
        }
    }

    /// Record that a tool call was blocked
    pub fn record_block(&self, estimated_tokens: usize) {
        let mut state = self.state.lock().unwrap();
        state.blocked_calls += 1;
        state.blocked_tokens_saved += estimated_tokens;

        let mut report = self.report.lock().unwrap();
        *report = TokenSavingsReport {
            technique: "governor".into(),
            tokens_before: state.blocked_tokens_saved + estimated_tokens,
            tokens_after: 0,
            tokens_saved: state.blocked_tokens_saved,
            description: format!(
                "blocked {} redundant tool calls, saved ~{} tokens",
                state.blocked_calls, state.blocked_tokens_saved
            ),
        };
    }
}

#[async_trait::async_trait]
impl TokenSaver for GovernorSaver {
    fn name(&self) -> &str { "governor" }
    fn stage(&self) -> SaverStage { SaverStage::PreCall }
    fn priority(&self) -> u32 { 5 }

    async fn process(&self, input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        // Governor primarily works via check_call() / record_call() called from orchestrator.
        // In pipeline mode, we do a pass-through but could inject circuit-breaker instructions.
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
    fn test_allow_first_call() {
        let gov = GovernorSaver::with_defaults();
        let decision = gov.check_call("read", &serde_json::json!({"path": "foo.rs"}));
        assert!(matches!(decision, GovernorDecision::Allow));
    }

    #[test]
    fn test_blocks_duplicate() {
        let gov = GovernorSaver::with_defaults();
        let args = serde_json::json!({"path": "foo.rs"});
        gov.record_call("read", &args);
        let decision = gov.check_call("read", &args);
        assert!(matches!(decision, GovernorDecision::Block(_)));
    }

    #[test]
    fn test_blocks_excessive_consecutive() {
        let gov = GovernorSaver::new(GovernorConfig {
            max_consecutive_same: 2,
            dedupe_identical: false,
            ..Default::default()
        });
        let args = serde_json::json!({"path": "a.rs"});
        let args2 = serde_json::json!({"path": "b.rs"});
        let args3 = serde_json::json!({"path": "c.rs"});
        gov.record_call("search", &args);
        gov.record_call("search", &args2);
        let decision = gov.check_call("search", &args3);
        assert!(matches!(decision, GovernorDecision::Block(_)));
    }
}
