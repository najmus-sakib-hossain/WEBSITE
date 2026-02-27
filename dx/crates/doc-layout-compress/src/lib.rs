//! Compresses document layout by stripping repetitive headers/footers.
//! SAVINGS: 10-30% on paginated documents
//! STAGE: PrePrompt (priority 66)

use dx_core::*;
use std::collections::HashMap;
use std::sync::Mutex;

pub struct DocLayoutCompressSaver {
    report: Mutex<TokenSavingsReport>,
}

impl DocLayoutCompressSaver {
    pub fn new() -> Self {
        Self {
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    /// Detect repeated lines across pages (appearing 3+ times).
    pub fn detect_repeated_lines(pages: &[&str]) -> Vec<String> {
        let mut freq: HashMap<String, usize> = HashMap::new();
        for page in pages {
            let lines: std::collections::HashSet<&str> = page.lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty() && l.len() < 100)
                .collect();
            for line in lines {
                *freq.entry(line.to_string()).or_default() += 1;
            }
        }
        freq.into_iter()
            .filter(|(_, count)| *count >= 3)
            .map(|(line, _)| line)
            .collect()
    }

    /// Strip repeated header/footer lines from page text.
    pub fn strip_repeated(text: &str, repeated: &[String]) -> String {
        text.lines()
            .filter(|line| {
                let trimmed = line.trim();
                !repeated.iter().any(|r| r == trimmed)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Compress page number annotations like "Page 1 of 10" → "[p.1/10]".
    pub fn compress_page_numbers(text: &str) -> String {
        let re = regex_lite::Regex::new(r"(?i)\bpage\s+(\d+)\s+of\s+(\d+)\b").unwrap();
        re.replace_all(text, |caps: &regex_lite::Captures| {
            format!("[p.{}/{}]", &caps[1], &caps[2])
        }).into_owned()
    }
}

impl Default for DocLayoutCompressSaver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl MultiModalTokenSaver for DocLayoutCompressSaver {
    fn modality(&self) -> Modality { Modality::Document }
}

#[async_trait::async_trait]
impl TokenSaver for DocLayoutCompressSaver {
    fn name(&self) -> &str { "doc-layout-compress" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 66 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let doc_msgs: Vec<usize> = input.messages.iter().enumerate()
            .filter(|(_, m)| m.modality.as_deref() == Some("document"))
            .map(|(i, _)| i)
            .collect();

        if doc_msgs.is_empty() {
            return Ok(SaverOutput {
                messages: input.messages,
                tools: input.tools,
                images: input.images,
                skipped: false,
                cached_response: None,
            });
        }

        let pages: Vec<&str> = doc_msgs.iter()
            .map(|&i| input.messages[i].content.as_str())
            .collect();

        let repeated = Self::detect_repeated_lines(&pages);
        let mut total_saved = 0usize;

        for &i in &doc_msgs {
            let original = input.messages[i].content.clone();
            let stripped = Self::strip_repeated(&original, &repeated);
            let compressed = Self::compress_page_numbers(&stripped);
            let saved = original.len().saturating_sub(compressed.len()) / 4;
            if saved > 0 {
                input.messages[i].content = compressed;
                input.messages[i].token_count = input.messages[i].content.len() / 4;
                total_saved += saved;
            }
        }

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "doc-layout-compress".into(),
                tokens_before: total_saved,
                tokens_after: 0,
                tokens_saved: total_saved,
                description: format!("layout compression saved {} tokens", total_saved),
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
