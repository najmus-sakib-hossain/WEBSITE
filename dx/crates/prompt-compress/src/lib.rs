//! Compresses prompt text by removing redundancy.
//! SAVINGS: 5-25% on prompt tokens
//! STAGE: PrePrompt (priority 25)

use dx_core::*;
use regex_lite::Regex;
use std::sync::Mutex;

pub struct PromptCompressSaver {
    report: Mutex<TokenSavingsReport>,
}

impl PromptCompressSaver {
    pub fn new() -> Self {
        Self {
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    /// Compress text by removing redundancy.
    pub fn compress(text: &str) -> String {
        let mut result = text.to_string();

        // Collapse 3+ consecutive blank lines to 2
        let blank_re = Regex::new(r"\n{3,}").unwrap();
        result = blank_re.replace_all(&result, "\n\n").into_owned();

        // Remove trailing spaces on each line
        let trail_re = Regex::new(r" +\n").unwrap();
        result = trail_re.replace_all(&result, "\n").into_owned();

        // Collapse multiple spaces (not at line start)
        let multispace_re = Regex::new(r"(?m)(?<=\S) {2,}").unwrap();
        result = multispace_re.replace_all(&result, " ").into_owned();

        // Remove filler phrases
        let fillers = [
            "Please note that ",
            "It is important to note that ",
            "I want to make sure that ",
            "As an AI language model, ",
            "Certainly! ",
            "Of course! ",
            "Sure! ",
            "Absolutely! ",
            "Great question! ",
        ];
        for filler in &fillers {
            result = result.replace(filler, "");
        }

        // Remove markdown separators (---/===) if they appear on their own line
        let sep_re = Regex::new(r"(?m)^[-=]{3,}$\n?").unwrap();
        result = sep_re.replace_all(&result, "").into_owned();

        result
    }
}

impl Default for PromptCompressSaver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TokenSaver for PromptCompressSaver {
    fn name(&self) -> &str { "prompt-compress" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 25 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut total_saved = 0usize;

        for msg in &mut input.messages {
            let original_len = msg.content.len();
            let compressed = Self::compress(&msg.content);
            let new_len = compressed.len();
            if new_len < original_len {
                let saved = (original_len - new_len) / 4;
                if saved > 0 {
                    msg.content = compressed;
                    msg.token_count = msg.content.len() / 4;
                    total_saved += saved;
                }
            }
        }

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "prompt-compress".into(),
                tokens_before: total_saved,
                tokens_after: 0,
                tokens_saved: total_saved,
                description: format!("compressed prompts, saved ~{} tokens", total_saved),
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
    fn test_removes_filler() {
        let text = "Certainly! Let me help you. Of course! Here is the answer.";
        let compressed = PromptCompressSaver::compress(text);
        assert!(!compressed.contains("Certainly!"));
        assert!(!compressed.contains("Of course!"));
    }

    #[test]
    fn test_collapses_blank_lines() {
        let text = "line1\n\n\n\nline2";
        let compressed = PromptCompressSaver::compress(text);
        assert!(!compressed.contains("\n\n\n"));
    }
}
