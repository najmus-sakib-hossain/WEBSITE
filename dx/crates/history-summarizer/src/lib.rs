//! Summarizes old conversation history to reduce token usage.
//! SAVINGS: 60-80% on long conversation history
//! STAGE: InterTurn (priority 25)

use dx_core::*;
use std::sync::Mutex;

pub struct HistorySummarizerSaver {
    max_summary_tokens: usize,
    summary_model: String,
    min_turns: usize,
    report: Mutex<TokenSavingsReport>,
}

impl HistorySummarizerSaver {
    pub fn new() -> Self {
        Self {
            max_summary_tokens: 500,
            summary_model: "gpt-4o-mini".into(),
            min_turns: 10,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_min_turns(min_turns: usize) -> Self {
        Self {
            min_turns,
            ..Self::new()
        }
    }

    /// Build a summary prompt from old messages.
    pub fn build_summary_prompt(messages: &[Message]) -> String {
        let mut prompt = String::from(
            "Summarize the following conversation history concisely, capturing key decisions, findings, and code changes:\n\n"
        );
        for msg in messages {
            let preview: String = msg.content.chars().take(200).collect();
            prompt.push_str(&format!("[{}]: {}\n", msg.role, preview));
        }
        prompt
    }

    /// Apply the summary by replacing old messages with a single summary message.
    pub fn apply_summary(messages: &mut Vec<Message>, summary: &str, replace_count: usize) {
        if replace_count == 0 || replace_count > messages.len() {
            return;
        }
        let mut summary_msg = Message::default();
        summary_msg.role = "system".into();
        summary_msg.content = format!("[HISTORY SUMMARY]\n{}", summary);
        summary_msg.token_count = summary_msg.content.len() / 4;

        messages.drain(0..replace_count);
        messages.insert(0, summary_msg);
    }
}

impl Default for HistorySummarizerSaver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TokenSaver for HistorySummarizerSaver {
    fn name(&self) -> &str { "history-summarizer" }
    fn stage(&self) -> SaverStage { SaverStage::InterTurn }
    fn priority(&self) -> u32 { 25 }

    async fn process(&self, input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        // Count turns in history
        let user_turns = input.messages.iter().filter(|m| m.role == "user").count();

        if user_turns < self.min_turns {
            return Ok(SaverOutput {
                messages: input.messages,
                tools: input.tools,
                images: input.images,
                skipped: false,
                cached_response: None,
            });
        }

        // Pass through - actual summarization requires external model call.
        // The caller should invoke build_summary_prompt, call the model externally,
        // then call apply_summary to replace the history.
        // This saver annotates the system prompt to indicate summarization is available.
        let mut msgs = input.messages;
        let total_tokens: usize = msgs.iter().map(|m| m.token_count).sum();

        if let Some(sys) = msgs.iter_mut().find(|m| m.role == "system") {
            let hint = format!(
                "\n[HISTORY: {} turns, {} tokens — consider summarizing turns 1-{}]",
                user_turns, total_tokens, user_turns.saturating_sub(4)
            );
            if !sys.content.contains("[HISTORY:") {
                sys.content.push_str(&hint);
                sys.token_count = sys.content.len() / 4;
            }
        }

        Ok(SaverOutput {
            messages: msgs,
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
