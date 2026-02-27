//! Prunes stale context from conversation history.
//! SAVINGS: 20-40% on multi-turn conversations
//! STAGE: InterTurn (priority 20)

use dx_core::*;
use std::sync::Mutex;

pub struct ContextPrunerSaver {
    preserve_recent: usize,
    report: Mutex<TokenSavingsReport>,
}

impl ContextPrunerSaver {
    pub fn new() -> Self {
        Self {
            preserve_recent: 4,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_preserve_recent(n: usize) -> Self {
        Self {
            preserve_recent: n,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    fn is_stale_read(msg: &Message, later_messages: &[Message]) -> bool {
        if msg.role != "tool" && msg.tool_call_id.is_none() {
            return false;
        }
        let path = Self::extract_path(&msg.content);
        let path = match path {
            Some(p) => p,
            None => return false,
        };
        later_messages.iter().any(|m| {
            let content_lower = m.content.to_lowercase();
            content_lower.contains(&path) && (
                content_lower.contains("updated")
                    || content_lower.contains("patched")
                    || content_lower.contains("created")
                    || content_lower.contains("wrote")
                    || content_lower.contains("modified")
            )
        })
    }

    fn is_trivial_success(msg: &Message) -> bool {
        if msg.role != "tool" && msg.tool_call_id.is_none() {
            return false;
        }
        let c = &msg.content;
        msg.token_count < 15 && (
            c.contains("created") || c.contains("updated")
                || c.contains("patched") || c.starts_with("ok")
        )
    }

    pub fn extract_path(content: &str) -> Option<String> {
        for word in content.split_whitespace() {
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '.' && c != '_' && c != '-');
            if clean.contains('/') && clean.contains('.') && clean.len() > 3 {
                return Some(clean.to_lowercase());
            }
        }
        None
    }
}

impl Default for ContextPrunerSaver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TokenSaver for ContextPrunerSaver {
    fn name(&self) -> &str { "context-pruner" }
    fn stage(&self) -> SaverStage { SaverStage::InterTurn }
    fn priority(&self) -> u32 { 20 }

    async fn process(&self, input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let msg_count = input.messages.len();
        let preserve_from = msg_count.saturating_sub(self.preserve_recent * 2);
        let before_tokens: usize = input.messages.iter().map(|m| m.token_count).sum();

        let mut pruned = Vec::new();
        let mut total_saved = 0usize;

        for (i, msg) in input.messages.iter().enumerate() {
            let is_recent = i >= preserve_from;

            if msg.role == "system" || is_recent {
                pruned.push(msg.clone());
                continue;
            }

            let later = &input.messages[i + 1..];
            if Self::is_stale_read(msg, later) {
                total_saved += msg.token_count;
                continue;
            }

            if !Self::is_trivial_success(msg) {
                pruned.push(msg.clone());
            } else {
                total_saved += msg.token_count;
            }
        }

        if total_saved > 0 {
            let after_tokens: usize = pruned.iter().map(|m| m.token_count).sum();
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "context-pruner".into(),
                tokens_before: before_tokens,
                tokens_after: after_tokens,
                tokens_saved: total_saved,
                description: format!("pruned {} stale/trivial tokens from history", total_saved),
            };
        }

        Ok(SaverOutput {
            messages: pruned,
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
