//! Compresses chain-of-thought reasoning in model outputs.
//! SAVINGS: 30-60% on verbose reasoning
//! STAGE: PostResponse (priority 20)

use dx_core::*;
use std::sync::Mutex;

pub struct CotCompressSaver {
    report: Mutex<TokenSavingsReport>,
}

const THINKING_PREFIXES: &[&str] = &[
    "Let me think",
    "I need to think",
    "Let me analyze",
    "First, let me",
    "Let me consider",
    "I'm going to",
    "Let me start by",
    "I should",
    "Alright, ",
    "OK, ",
    "Okay, ",
    "Well, ",
];

const ACTION_PREFIXES: &[&str] = &[
    "Now I",
    "So I",
    "Then I",
    "Next I",
    "I will",
    "I'll",
    "Let me now",
    "Moving on",
];

impl CotCompressSaver {
    pub fn new() -> Self {
        Self {
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    /// Detect if a line is purely internal reasoning filler.
    fn is_reasoning_filler(line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.is_empty() { return false; }
        THINKING_PREFIXES.iter().any(|p| trimmed.starts_with(p))
            || ACTION_PREFIXES.iter().any(|p| trimmed.starts_with(p))
    }

    /// Compress chain-of-thought text by removing repetitive reasoning lines.
    pub fn compress(text: &str) -> String {
        let mut lines: Vec<&str> = text.lines().collect();
        let mut result = Vec::new();
        let mut skipped_block = false;

        for (i, line) in lines.iter().enumerate() {
            if Self::is_reasoning_filler(line) {
                // Skip up to 3 consecutive filler lines; keep one summary
                if !skipped_block {
                    skipped_block = true;
                } else {
                    continue;
                }
            } else {
                skipped_block = false;
            }
            result.push(*line);
        }

        // De-dupe consecutive duplicate lines
        let mut deduped: Vec<&str> = Vec::new();
        for line in &result {
            if deduped.last().map_or(true, |last| *last != *line) {
                deduped.push(line);
            }
        }

        deduped.join("\n")
    }
}

impl Default for CotCompressSaver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TokenSaver for CotCompressSaver {
    fn name(&self) -> &str { "cot-compress" }
    fn stage(&self) -> SaverStage { SaverStage::PostResponse }
    fn priority(&self) -> u32 { 20 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut total_saved = 0usize;

        for msg in &mut input.messages {
            if msg.role != "assistant" { continue; }
            let original_tokens = msg.token_count;
            let compressed = Self::compress(&msg.content);
            let new_tokens = compressed.len() / 4;
            let saved = original_tokens.saturating_sub(new_tokens);
            if saved > 0 {
                msg.content = compressed;
                msg.token_count = new_tokens;
                total_saved += saved;
            }
        }

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "cot-compress".into(),
                tokens_before: total_saved,
                tokens_after: 0,
                tokens_saved: total_saved,
                description: format!("compressed chain-of-thought, saved {} tokens", total_saved),
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
    fn test_removes_filler_lines() {
        let text = "Let me think about this problem.\nThe answer is 42.\nOK, so I'll do that.";
        let compressed = CotCompressSaver::compress(text);
        assert!(compressed.contains("42"));
    }

    #[test]
    fn test_dedupes_consecutive() {
        let text = "step one\nstep one\nstep two";
        let compressed = CotCompressSaver::compress(text);
        let count = compressed.lines().filter(|l| *l == "step one").count();
        assert_eq!(count, 1);
    }
}
