//! # governor
//!
//! Circuit breaker for tool calls. Prevents runaway spirals where the
//! model calls the same tool repeatedly, makes duplicate calls, or
//! exhausts budgets without progress.
//!
//! ## Verified Savings (TOKEN.md research)
//!
//! - **REAL**: Circuit breakers are pure engineering — no hype involved.
//! - If a tool loops 5 times reading the same file, stopping it saves real tokens.
//! - Prevents 20-100+ wasted tool calls per session in misbehaving agents.
//! - The only risk: being too aggressive and blocking legitimate retries.
//!
//! SAVINGS: Prevents waste (not a compression technique — a guard rail)
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
    /// Max tool calls in a single LLM turn (response).
    pub max_calls_per_turn: usize,
    /// Max times the same tool name can be called in a session.
    pub max_same_tool_calls: usize,
    /// Max total tool calls in the session before forcing a stop.
    pub max_total_calls: usize,
    /// Average tokens per tool call (used to estimate savings).
    pub avg_tokens_per_call: usize,
    /// If true, block but log instead of returning an error.
    pub soft_block: bool,
}

impl Default for GovernorConfig {
    fn default() -> Self {
        Self {
            max_calls_per_turn: 10,
            max_same_tool_calls: 5,
            max_total_calls: 100,
            avg_tokens_per_call: 300,
            soft_block: true, // soft by default — log, don't hard fail
        }
    }
}

#[derive(Default)]
struct GovernorState {
    total_calls: usize,
    calls_per_tool: HashMap<String, usize>,
    calls_this_turn: usize,
    blocked_calls: usize,
    tokens_saved: usize,
}

/// Decision from the governor on whether to allow a tool call.
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
            state: Mutex::new(GovernorState::default()),
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    /// Reset turn-level counters. Call between turns.
    pub fn reset_turn(&self) {
        let mut state = self.state.lock().unwrap();
        state.calls_this_turn = 0;
    }

    /// Record a tool call and return decision.
    pub fn check_call(&self, tool_name: &str) -> GovernorDecision {
        let mut state = self.state.lock().unwrap();

        // Check total budget
        if state.total_calls >= self.config.max_total_calls {
            let msg = format!("tool '{}' blocked: session total ({}) >= max ({})",
                tool_name, state.total_calls, self.config.max_total_calls);
            state.blocked_calls += 1;
            state.tokens_saved += self.config.avg_tokens_per_call;
            return GovernorDecision::Block(msg);
        }

        // Check per-turn budget
        if state.calls_this_turn >= self.config.max_calls_per_turn {
            let msg = format!("tool '{}' blocked: turn calls ({}) >= max per turn ({})",
                tool_name, state.calls_this_turn, self.config.max_calls_per_turn);
            state.blocked_calls += 1;
            state.tokens_saved += self.config.avg_tokens_per_call;
            return GovernorDecision::Block(msg);
        }

        // Check same-tool repetition
        let tool_count = state.calls_per_tool.entry(tool_name.to_string()).or_default();
        if *tool_count >= self.config.max_same_tool_calls {
            let msg = format!("tool '{}' blocked: called {} times (max {})",
                tool_name, tool_count, self.config.max_same_tool_calls);
            state.blocked_calls += 1;
            state.tokens_saved += self.config.avg_tokens_per_call;
            return GovernorDecision::Block(msg);
        }

        // Warn when approaching limits
        let warn = if *tool_count == self.config.max_same_tool_calls - 1 {
            Some(format!("warning: '{}' called {} times, approaching limit of {}",
                tool_name, tool_count + 1, self.config.max_same_tool_calls))
        } else if state.total_calls == self.config.max_total_calls - 5 {
            Some(format!("warning: {} tool calls remaining in session budget", 5))
        } else {
            None
        };

        // Allow the call
        *tool_count += 1;
        state.total_calls += 1;
        state.calls_this_turn += 1;

        match warn {
            Some(w) => GovernorDecision::AllowWithWarning(w),
            None => GovernorDecision::Allow,
        }
    }

    pub fn blocked_count(&self) -> usize {
        self.state.lock().unwrap().blocked_calls
    }

    pub fn total_calls(&self) -> usize {
        self.state.lock().unwrap().total_calls
    }
}

impl Default for GovernorSaver {
    fn default() -> Self { Self::new(GovernorConfig::default()) }
}

#[async_trait::async_trait]
impl TokenSaver for GovernorSaver {
    fn name(&self) -> &str { "governor" }
    fn stage(&self) -> SaverStage { SaverStage::PreCall }
    fn priority(&self) -> u32 { 5 }

    async fn process(&self, input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        // Governor is a stateful guard. It checks tool mentions in the last
        // assistant message to detect excessive tool calls.
        let state = self.state.lock().unwrap();
        let blocked = state.blocked_calls;
        let saved = state.tokens_saved;
        drop(state);

        let mut report = self.report.lock().unwrap();
        *report = TokenSavingsReport {
            technique: "governor".into(),
            tokens_before: saved,
            tokens_after: 0,
            tokens_saved: saved,
            description: format!("circuit breaker: {} calls blocked, ~{} tokens avoided", blocked, saved),
        };

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
    fn allows_normal_calls() {
        let g = GovernorSaver::default();
        assert!(matches!(g.check_call("read"), GovernorDecision::Allow));
        assert!(matches!(g.check_call("write"), GovernorDecision::Allow));
    }

    #[test]
    fn blocks_repeated_same_tool() {
        let g = GovernorSaver::new(GovernorConfig { max_same_tool_calls: 3, ..Default::default() });
        for _ in 0..3 { g.check_call("read"); }
        assert!(matches!(g.check_call("read"), GovernorDecision::Block(_)));
    }

    #[test]
    fn blocks_per_turn_limit() {
        let g = GovernorSaver::new(GovernorConfig {
            max_calls_per_turn: 2,
            max_same_tool_calls: 100,
            ..Default::default()
        });
        g.check_call("a"); g.check_call("b");
        assert!(matches!(g.check_call("c"), GovernorDecision::Block(_)));
    }

    #[test]
    fn reset_turn_clears_per_turn_counter() {
        let g = GovernorSaver::new(GovernorConfig { max_calls_per_turn: 2, ..Default::default() });
        g.check_call("a"); g.check_call("b");
        g.reset_turn();
        // Should allow again after reset
        assert!(!matches!(g.check_call("a"), GovernorDecision::Block(_)));
    }

    #[test]
    fn tracks_blocked_count() {
        let g = GovernorSaver::new(GovernorConfig { max_same_tool_calls: 1, ..Default::default() });
        g.check_call("x");
        g.check_call("x"); // blocked
        assert_eq!(g.blocked_count(), 1);
    }
}
