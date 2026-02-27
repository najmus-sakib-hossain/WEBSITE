//! Extracts text from PDF documents using system pdftotext.
//! SAVINGS: 95-99% vs sending raw PDF image tokens
//! STAGE: PrePrompt (priority 60)

use dx_core::*;
use std::sync::Mutex;

pub struct PdfTextExtractSaver {
    report: Mutex<TokenSavingsReport>,
}

impl PdfTextExtractSaver {
    pub fn new() -> Self {
        Self {
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    /// Try to extract text from PDF bytes using system pdftotext via stdin/stdout.
    pub fn try_pdftotext(pdf_bytes: &[u8]) -> Option<String> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut child = Command::new("pdftotext")
            .arg("-")  // read from stdin
            .arg("-")  // write to stdout
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(pdf_bytes).ok()?;
        }

        let output = child.wait_with_output().ok()?;
        if !output.status.success() { return None; }
        let text = String::from_utf8(output.stdout).ok()?;
        if text.trim().len() < 100 { return None; }
        Some(text)
    }
}

impl Default for PdfTextExtractSaver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TokenSaver for PdfTextExtractSaver {
    fn name(&self) -> &str { "pdf-text-extract" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 60 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut total_saved = 0usize;

        // Process images that are PDF mime type — extract text and add as message
        let mut extracted_texts: Vec<String> = Vec::new();
        let mut pdf_images: Vec<usize> = Vec::new();

        for (i, img) in input.images.iter().enumerate() {
            if img.mime.contains("pdf") || img.mime.contains("PDF") {
                let original_tokens = img.original_tokens;
                if let Some(text) = Self::try_pdftotext(&img.data) {
                    let text_tokens = text.len() / 4;
                    if text_tokens < original_tokens {
                        total_saved += original_tokens.saturating_sub(text_tokens);
                        extracted_texts.push(text);
                        pdf_images.push(i);
                    }
                }
            }
        }

        // Remove processed PDF images (in reverse order to preserve indices)
        for &i in pdf_images.iter().rev() {
            input.images.remove(i);
        }

        // Add extracted text as system messages
        for (idx, text) in extracted_texts.into_iter().enumerate() {
            let content = format!("[PDF EXTRACT {}]\n{}", idx + 1, text);
            let token_count = content.len() / 4;
            input.messages.push(Message {
                role: "system".into(),
                content,
                images: vec![],
                tool_call_id: None,
                token_count,
            });
        }

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "pdf-text-extract".into(),
                tokens_before: total_saved,
                tokens_after: 0,
                tokens_saved: total_saved,
                description: format!("PDF text extraction saved {} tokens", total_saved),
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
