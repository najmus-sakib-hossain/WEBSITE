//! Smart truncation of tool outputs preserving head and tail.
//! SAVINGS: 50-95% on large tool outputs
//! STAGE: PostResponse (priority 10)

use dx_core::*;
use std::sync::Mutex;

pub struct OutputTruncatorSaver {
    config: TruncatorConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct TruncatorConfig {
    pub max_tokens: usize,
    pub head_ratio: f64,
    pub marker: String,
}

impl Default for TruncatorConfig {
    fn default() -> Self {
        Self {
            max_tokens: 4000,
            head_ratio: 0.6,
            marker: "[... {n} tokens truncated ...]".into(),
        }
    }
}

impl OutputTruncatorSaver {
    pub fn new(config: TruncatorConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(TruncatorConfig::default())
    }

    pub fn with_max_tokens(max: usize) -> Self {
        Self::new(TruncatorConfig {
            max_tokens: max,
            ..Default::default()
        })
    }

    pub fn truncate(&self, content: &str) -> (String, usize) {
        let estimated_tokens = content.len() / 4;
        if estimated_tokens <= self.config.max_tokens {
            return (content.to_string(), 0);
        }

        let max_chars = self.config.max_tokens * 4;
        let head_chars = (max_chars as f64 * self.config.head_ratio) as usize;
        let tail_chars = max_chars - head_chars;

        let head_end = if head_chars < content.len() {
            content[..head_chars].rfind('\n').unwrap_or(head_chars)
        } else {
            content.len()
        };

        let tail_start_raw = content.len().saturating_sub(tail_chars);
        let tail_start = if tail_start_raw < content.len() {
            content[tail_start_raw..].find('\n')
                .map(|p| tail_start_raw + p + 1)
                .unwrap_or(tail_start_raw)
        } else {
            content.len()
        };

        if head_end >= tail_start {
            return (content.to_string(), 0);
        }

        let truncated_chars = tail_start - head_end;
        let truncated_tokens = truncated_chars / 4;
        let marker = self.config.marker.replace("{n}", &truncated_tokens.to_string());

        let result = format!(
            "{}\n\n{}\n\n{}",
            &content[..head_end],
            marker,
            &content[tail_start..]
        );

        (result, truncated_tokens)
    }
}

#[async_trait::async_trait]
impl TokenSaver for OutputTruncatorSaver {
    fn name(&self) -> &str { "output-truncator" }
    fn stage(&self) -> SaverStage { SaverStage::PostResponse }
    fn priority(&self) -> u32 { 10 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut total_saved = 0usize;

        for msg in &mut input.messages {
            if msg.tool_call_id.is_some() || msg.role == "tool" {
                let (truncated, saved) = self.truncate(&msg.content);
                if saved > 0 {
                    msg.content = truncated;
                    msg.token_count = msg.content.len() / 4;
                    total_saved += saved;
                }
            }
        }

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "output-truncator".into(),
                tokens_before: total_saved + input.messages.iter()
                    .filter(|m| m.tool_call_id.is_some() || m.role == "tool")
                    .map(|m| m.token_count)
                    .sum::<usize>(),
                tokens_after: input.messages.iter()
                    .filter(|m| m.tool_call_id.is_some() || m.role == "tool")
                    .map(|m| m.token_count)
                    .sum::<usize>(),
                tokens_saved: total_saved,
                description: format!("truncated tool outputs, saved {} tokens", total_saved),
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
    fn test_short_content_not_truncated() {
        let saver = OutputTruncatorSaver::with_max_tokens(1000);
        let (result, saved) = saver.truncate("short output");
        assert_eq!(result, "short output");
        assert_eq!(saved, 0);
    }

    #[test]
    fn test_long_content_truncated() {
        let saver = OutputTruncatorSaver::with_max_tokens(100);
        let content: String = (0..1000).map(|i| format!("line {}: some build output here\n", i)).collect();
        let (result, saved) = saver.truncate(&content);
        assert!(saved > 0);
        assert!(result.contains("truncated"));
        assert!(result.len() < content.len());
    }

    #[test]
    fn test_preserves_head_and_tail() {
        let saver = OutputTruncatorSaver::with_max_tokens(50);
        let mut content = String::from("HEADER: build started\n");
        for i in 0..500 { content.push_str(&format!("compiling module {}\n", i)); }
        content.push_str("ERROR: build failed at line 42\n");
        let (result, _) = saver.truncate(&content);
        assert!(result.contains("HEADER"));
        assert!(result.contains("ERROR"));
    }
}
