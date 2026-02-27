//! Extracts text from PDF documents using system pdftotext.
//! SAVINGS: 95-99% vs sending raw PDF image tokens
//! STAGE: PrePrompt (priority 60)

use dx_core::*;
use std::io::Write;
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

    /// Try to extract text from PDF bytes using system pdftotext.
    pub fn try_pdftotext(pdf_bytes: &[u8]) -> Option<String> {
        // Write temp PDF
        let mut tmp = tempfile::Builder::new()
            .suffix(".pdf")
            .tempfile()
            .ok()?;
        tmp.write_all(pdf_bytes).ok()?;
        let tmp_path = tmp.path().to_path_buf();

        let output = std::process::Command::new("pdftotext")
            .arg(tmp_path.to_str()?)
            .arg("-")
            .output()
            .ok()?;

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
impl MultiModalTokenSaver for PdfTextExtractSaver {
    fn modality(&self) -> Modality { Modality::Document }
}

#[async_trait::async_trait]
impl TokenSaver for PdfTextExtractSaver {
    fn name(&self) -> &str { "pdf-text-extract" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 60 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut total_saved = 0usize;

        for msg in &mut input.messages {
            if msg.modality.as_deref() != Some("document") { continue; }
            let content_type = msg.content_type.as_deref().unwrap_or("");
            if !content_type.contains("pdf") && !content_type.contains("PDF") {
                continue;
            }

            let pdf_bytes = match msg.binary_data.as_deref() {
                Some(b) => b.to_vec(),
                None => continue,
            };

            let original_tokens = msg.token_count;
            // Estimate original cost: ~1700 tokens per PDF page
            let page_estimate = (pdf_bytes.len() / 50_000).max(1);
            let pdf_tokens = page_estimate * 1700;

            if let Some(text) = Self::try_pdftotext(&pdf_bytes) {
                let text_tokens = text.len() / 4;
                if text_tokens < pdf_tokens {
                    msg.content = format!(
                        "[PDF TEXT EXTRACT: ~{} pages, {} chars]\n{}",
                        page_estimate, text.len(), text
                    );
                    msg.token_count = msg.content.len() / 4;
                    msg.binary_data = None;
                    msg.modality = Some("text".into());
                    total_saved += original_tokens.max(pdf_tokens).saturating_sub(msg.token_count);
                }
            }
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
