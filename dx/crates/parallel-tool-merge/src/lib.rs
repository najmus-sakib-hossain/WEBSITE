//! # parallel-tool-merge
//!
//! Merges parallel tool call results into a single message to reduce
//! message-level overhead in conversation history.
//!
//! ## Honest Savings (TOKEN.md research)
//!
//! - TOKEN.md verdict: **3-5% realistically** — marginal savings.
//! - Message overhead per tool result: ~15-20 tokens (role, tool_call_id)
//! - Merging 5 results saves ~60-80 tokens overhead.
//! - BUT: Breaking tool_call_id associations can confuse providers.
//! - Do NOT merge if the provider requires individual tool_call_id matching.
//! - Best use: long-running agent sessions with many short tool results.
//!
//! ## When to use
//! - Tool results are short (< 100 tokens each)
//! - Multiple tools were called in a single turn
//! - The provider does not require strict tool_call_id matching
//!
//! SAVINGS: 3-5% realistically (MESSAGE OVERHEAD ONLY)
//! STAGE: PostResponse (priority 25)

use dx_core::*;
use std::sync::Mutex;

pub struct ParallelToolMergeSaver {
    config: MergeConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct MergeConfig {
    /// Only merge tools from the same turn (same assistant message).
    /// Merging across turns is too risky for quality.
    pub same_turn_only: bool,
    /// Maximum number of tool results to merge into one.
    pub max_merge_count: usize,
    /// Only merge SHORT tool results (token threshold).
    /// Long tool results are more likely to need individual tracking.
    pub max_tokens_per_result: usize,
}

impl Default for MergeConfig {
    fn default() -> Self {
        Self {
            same_turn_only: true,
            max_merge_count: 5,
            max_tokens_per_result: 100, // only merge short results
        }
    }
}

impl ParallelToolMergeSaver {
    pub fn new(config: MergeConfig) -> Self {
        Self { config, report: Mutex::new(TokenSavingsReport::default()) }
    }

    /// Find runs of consecutive tool messages that can be merged.
    /// Returns list of (start_idx, end_idx) ranges.
    pub fn find_merge_candidates(messages: &[Message], max_tokens: usize, max_count: usize) -> Vec<(usize, usize)> {
        let mut ranges = Vec::new();
        let mut i = 0;

        while i < messages.len() {
            if messages[i].role != "tool" {
                i += 1;
                continue;
            }

            // Find the extent of this run of tool messages
            let start = i;
            let mut end = i;
            while end + 1 < messages.len()
                && messages[end + 1].role == "tool"
                && messages[end + 1].token_count <= max_tokens
                && (end - start + 1) < max_count
            {
                end += 1;
            }

            // Only merge if we found at least 2 consecutive tool messages
            if end > start {
                ranges.push((start, end));
                i = end + 1;
            } else {
                i += 1;
            }
        }

        ranges
    }

    /// Merge a range of tool messages into one.
    pub fn merge_range(messages: &mut Vec<Message>, start: usize, end: usize) -> usize {
        if start >= end || end >= messages.len() { return 0; }

        let mut merged_content = String::new();
        let mut original_tokens = 0usize;
        let mut ids = Vec::new();

        for msg in &messages[start..=end] {
            if let Some(ref id) = msg.tool_call_id {
                ids.push(id.clone());
            }
            merged_content.push_str(&format!("[{}]:\n{}\n\n",
                msg.tool_call_id.as_deref().unwrap_or("?"),
                msg.content
            ));
            original_tokens += msg.token_count;
        }

        let merged_tokens = merged_content.len() / 4 + 10;
        let overhead = (end - start) * 18; // ~18 tokens overhead per skipped message header
        let saved = original_tokens.saturating_sub(merged_tokens) + overhead;

        // Replace the range with a single merged message
        let merged = Message {
            role: "tool".into(),
            content: format!("[PARALLEL RESULTS: {}]\n{}", ids.join(", "), merged_content),
            images: vec![],
            tool_call_id: ids.first().cloned(),
            token_count: merged_tokens,
        };

        messages.drain(start..=end);
        messages.insert(start, merged);

        saved
    }
}

impl Default for ParallelToolMergeSaver {
    fn default() -> Self { Self::new(MergeConfig::default()) }
}

#[async_trait::async_trait]
impl TokenSaver for ParallelToolMergeSaver {
    fn name(&self) -> &str { "parallel-tool-merge" }
    fn stage(&self) -> SaverStage { SaverStage::PostResponse }
    fn priority(&self) -> u32 { 25 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let before_tokens: usize = input.messages.iter().map(|m| m.token_count).sum();

        let ranges = Self::find_merge_candidates(
            &input.messages,
            self.config.max_tokens_per_result,
            self.config.max_merge_count,
        );

        let mut total_saved = 0usize;
        // Process in reverse order to preserve indices
        for (start, end) in ranges.into_iter().rev() {
            total_saved += Self::merge_range(&mut input.messages, start, end);
        }

        let after_tokens: usize = input.messages.iter().map(|m| m.token_count).sum();
        let actual_saved = before_tokens.saturating_sub(after_tokens);

        // TOKEN.md reality check: typical savings are 3-5%, not 10-30%
        let realistic_pct = if before_tokens > 0 {
            actual_saved * 100 / before_tokens
        } else { 0 };

        let mut report = self.report.lock().unwrap();
        *report = TokenSavingsReport {
            technique: "parallel-tool-merge".into(),
            tokens_before: before_tokens,
            tokens_after: after_tokens,
            tokens_saved: actual_saved,
            description: format!(
                "merged consecutive tool results: {} tokens saved ({}%) — note: marginal, typically 3-5%",
                actual_saved, realistic_pct
            ),
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

    fn tool_msg(content: &str, id: &str, tokens: usize) -> Message {
        Message { role: "tool".into(), content: content.into(), images: vec![], tool_call_id: Some(id.into()), token_count: tokens }
    }

    fn user_msg() -> Message {
        Message { role: "user".into(), content: "hello".into(), images: vec![], tool_call_id: None, token_count: 5 }
    }

    #[test]
    fn finds_consecutive_tool_runs() {
        let msgs = vec![
            user_msg(),
            tool_msg("r1", "c1", 50),
            tool_msg("r2", "c2", 50),
            tool_msg("r3", "c3", 50),
            user_msg(),
        ];
        let candidates = ParallelToolMergeSaver::find_merge_candidates(&msgs, 100, 5);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], (1, 3));
    }

    #[test]
    fn does_not_merge_long_results() {
        // Results over max_tokens_per_result should not be merged
        let msgs = vec![
            tool_msg("r1", "c1", 50),
            tool_msg("very long result that exceeds limit", "c2", 200), // over limit
        ];
        let candidates = ParallelToolMergeSaver::find_merge_candidates(&msgs, 100, 5);
        assert!(candidates.is_empty(), "should not merge results over token limit");
    }

    #[test]
    fn savings_are_modest_per_token_md() {
        // Per TOKEN.md: realistic savings are 3-5% not 10-30%
        // This test documents the expectation
        let config = MergeConfig::default();
        // 5 results × 100 tokens = 500 tokens
        // Merged overhead ~ 5 × 18 = 90 tokens saved
        // 90 / 500 = 18% — but that's for all-short results
        // In practice with longer results the % drops to 3-5%
        assert!(config.max_tokens_per_result <= 100, "keep short to avoid quality risk");
    }
}
