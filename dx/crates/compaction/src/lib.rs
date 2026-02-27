//! # compaction
//!
//! Automatically compacts conversation history when token count exceeds
//! thresholds. Supports local rule-based compaction.
//!
//! Anthropic reports 84% token reduction in 100-turn web search eval.
//!
//! SAVINGS: 50-84% on conversation history tokens
//! STAGE: InterTurn (priority 30)

use dx_core::*;
use std::sync::Mutex;

pub struct CompactionSaver {
    config: CompactionConfig,
    state: Mutex<CompactionState>,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct CompactionConfig {
    /// Token count that triggers compaction consideration
    pub soft_limit: usize,
    /// Token count that forces compaction
    pub hard_limit: usize,
    /// Minimum turns between compactions
    pub min_turns_between: usize,
    /// Number of recent turn pairs to always preserve
    pub keep_last_turns: usize,
    /// Preserve messages containing error indicators
    pub keep_errors: bool,
    /// Preserve messages about file mutations (write/patch results)
    pub keep_mutations: bool,
}

struct CompactionState {
    turns_since_compaction: usize,
    total_compactions: usize,
    total_tokens_saved: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            soft_limit: 40_000,
            hard_limit: 80_000,
            min_turns_between: 5,
            keep_last_turns: 3,
            keep_errors: true,
            keep_mutations: true,
        }
    }
}

impl CompactionSaver {
    pub fn new(config: CompactionConfig) -> Self {
        Self {
            config,
            state: Mutex::new(CompactionState {
                turns_since_compaction: 0,
                total_compactions: 0,
                total_tokens_saved: 0,
            }),
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(CompactionConfig::default())
    }

    fn should_compact(&self, total_tokens: usize) -> CompactionDecision {
        let state = self.state.lock().unwrap();
        if state.turns_since_compaction < self.config.min_turns_between {
            return CompactionDecision::Skip;
        }
        if total_tokens >= self.config.hard_limit {
            CompactionDecision::Force
        } else if total_tokens >= self.config.soft_limit {
            CompactionDecision::Suggest
        } else {
            CompactionDecision::Skip
        }
    }

    fn is_preservable(&self, msg: &Message, is_recent: bool) -> bool {
        if msg.role == "system" { return true; }
        if is_recent { return true; }

        if self.config.keep_errors {
            let lower = msg.content.to_lowercase();
            if lower.contains("error") || lower.contains("failed")
                || lower.contains("panic") || lower.contains("exception")
                || lower.contains("traceback")
            {
                return true;
            }
        }

        if self.config.keep_mutations {
            let lower = msg.content.to_lowercase();
            if lower.contains("created") || lower.contains("updated")
                || lower.contains("patched") || lower.contains("deleted")
                || lower.contains("wrote")
            {
                return true;
            }
        }

        false
    }

    fn compact_messages(&self, messages: Vec<Message>) -> Vec<Message> {
        let msg_count = messages.len();
        let keep_from = msg_count.saturating_sub(self.config.keep_last_turns * 2);

        let mut compacted = Vec::new();
        let mut removed_count = 0;
        let mut removed_tokens = 0;

        for (i, msg) in messages.into_iter().enumerate() {
            let is_recent = i >= keep_from;
            if self.is_preservable(&msg, is_recent) {
                compacted.push(msg);
            } else {
                removed_count += 1;
                removed_tokens += msg.token_count;
            }
        }

        if removed_count > 0 {
            let insert_pos = compacted.iter()
                .position(|m| m.role != "system")
                .unwrap_or(compacted.len());

            compacted.insert(insert_pos, Message {
                role: "system".into(),
                content: format!(
                    "[Context compacted: {} messages ({} tokens) summarized. Recent context and errors preserved.]",
                    removed_count, removed_tokens
                ),
                images: vec![],
                tool_call_id: None,
                token_count: 20,
            });

            let mut state = self.state.lock().unwrap();
            state.turns_since_compaction = 0;
            state.total_compactions += 1;
            state.total_tokens_saved += removed_tokens;
        }

        compacted
    }
}

enum CompactionDecision { Skip, Suggest, Force }

#[async_trait::async_trait]
impl TokenSaver for CompactionSaver {
    fn name(&self) -> &str { "compaction" }
    fn stage(&self) -> SaverStage { SaverStage::InterTurn }
    fn priority(&self) -> u32 { 30 }

    async fn process(&self, input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        {
            let mut state = self.state.lock().unwrap();
            state.turns_since_compaction += 1;
        }

        let total_tokens: usize = input.messages.iter().map(|m| m.token_count).sum();
        let decision = self.should_compact(total_tokens);

        let messages = match decision {
            CompactionDecision::Skip => input.messages,
            CompactionDecision::Suggest | CompactionDecision::Force => {
                let before_tokens = total_tokens;
                let compacted = self.compact_messages(input.messages);
                let after_tokens: usize = compacted.iter().map(|m| m.token_count).sum();

                let mut report = self.report.lock().unwrap();
                *report = TokenSavingsReport {
                    technique: "compaction".into(),
                    tokens_before: before_tokens,
                    tokens_after: after_tokens,
                    tokens_saved: before_tokens.saturating_sub(after_tokens),
                    description: format!(
                        "compacted {} → {} tokens ({:.1}% reduction)",
                        before_tokens, after_tokens,
                        (1.0 - after_tokens as f64 / before_tokens.max(1) as f64) * 100.0
                    ),
                };
                compacted
            }
        };

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

    fn make_msg(role: &str, content: &str, tokens: usize) -> Message {
        Message { role: role.into(), content: content.into(), images: vec![], tool_call_id: None, token_count: tokens }
    }

    #[test]
    fn test_preserves_system_messages() {
        let saver = CompactionSaver::with_defaults();
        let msgs = vec![
            make_msg("system", "You are DX", 5),
            make_msg("user", "old question 1", 100),
            make_msg("assistant", "old answer 1", 200),
            make_msg("user", "recent question", 100),
            make_msg("assistant", "recent answer", 200),
        ];
        let compacted = saver.compact_messages(msgs);
        assert!(compacted[0].role == "system");
        assert!(compacted[0].content.contains("DX"));
    }

    #[test]
    fn test_preserves_error_messages() {
        let saver = CompactionSaver::with_defaults();
        let msgs = vec![
            make_msg("system", "sys", 5),
            make_msg("tool", "error: compilation failed", 500),
            make_msg("user", "old irrelevant message here", 100),
            make_msg("user", "recent", 100),
            make_msg("assistant", "recent answer here", 200),
        ];
        let compacted = saver.compact_messages(msgs);
        assert!(compacted.iter().any(|m| m.content.contains("compilation failed")));
    }
}
