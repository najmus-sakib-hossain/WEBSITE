//! Normalizes whitespace in all messages to reduce token count.
//! SAVINGS: 1-5% on whitespace-heavy content
//! STAGE: PrePrompt (priority 20)

use dx_core::*;
use std::sync::Mutex;

pub struct WhitespaceNormalizeSaver {
    report: Mutex<TokenSavingsReport>,
}

impl WhitespaceNormalizeSaver {
    pub fn new() -> Self {
        Self {
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    /// Normalize whitespace in text.
    pub fn normalize(text: &str) -> String {
        // Remove UTF-8 BOM if present
        let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);

        // Normalize CRLF to LF
        let text = text.replace("\r\n", "\n").replace('\r', "\n");

        // Remove zero-width space and similar invisible chars
        let text: String = text.chars().filter(|&c| {
            c != '\u{200B}' && c != '\u{200C}' && c != '\u{200D}'
                && c != '\u{FEFF}' && c != '\u{00AD}'
        }).collect();

        // Normalize tabs to 2 spaces
        let text = text.replace('\t', "  ");

        // Remove trailing whitespace per line
        let text: String = text.lines()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n");

        // Collapse 3+ consecutive blank lines to 2
        let mut result = String::new();
        let mut blank_count = 0usize;
        for line in text.lines() {
            if line.is_empty() {
                blank_count += 1;
                if blank_count <= 2 {
                    result.push('\n');
                }
            } else {
                blank_count = 0;
                result.push_str(line);
                result.push('\n');
            }
        }

        // Trim trailing newlines
        result.trim_end_matches('\n').to_string()
    }
}

impl Default for WhitespaceNormalizeSaver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TokenSaver for WhitespaceNormalizeSaver {
    fn name(&self) -> &str { "whitespace-normalize" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 20 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut total_saved = 0usize;

        for msg in &mut input.messages {
            let before = msg.content.len();
            let normalized = Self::normalize(&msg.content);
            let after = normalized.len();
            if after < before {
                let saved = (before - after) / 4;
                msg.content = normalized;
                msg.token_count = msg.content.len() / 4;
                total_saved += saved;
            }
        }

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "whitespace-normalize".into(),
                tokens_before: total_saved,
                tokens_after: 0,
                tokens_saved: total_saved,
                description: format!("normalized whitespace, saved {} tokens", total_saved),
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
    fn test_crlf_normalized() {
        let normalized = WhitespaceNormalizeSaver::normalize("line1\r\nline2\r\n");
        assert!(!normalized.contains('\r'));
        assert!(normalized.contains("line1\nline2"));
    }

    #[test]
    fn test_trailing_whitespace_removed() {
        let normalized = WhitespaceNormalizeSaver::normalize("hello   \nworld  ");
        for line in normalized.lines() {
            assert_eq!(line, line.trim_end());
        }
    }

    #[test]
    fn test_blank_lines_capped() {
        let text = "a\n\n\n\n\nb";
        let normalized = WhitespaceNormalizeSaver::normalize(text);
        assert!(!normalized.contains("\n\n\n"));
    }
}
