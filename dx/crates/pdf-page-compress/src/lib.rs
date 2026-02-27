//! Compresses PDF pages to targeted token budget using DocOwl2 patch strategy.
//! Target: ~324 tokens per page (18×18 patches)
//! STAGE: PrePrompt (priority 64)

use dx_core::*;
use image::DynamicImage;
use std::sync::Mutex;

pub struct PdfPageCompressSaver {
    config: PageCompressConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct PageCompressConfig {
    /// Target tokens per page (DocOwl2 strategy: 18×18 patches)
    pub target_tokens_per_page: usize,
    pub max_pages: usize,
    pub jpeg_quality: u8,
}

impl Default for PageCompressConfig {
    fn default() -> Self {
        Self {
            target_tokens_per_page: 324, // 18 × 18 patches
            max_pages: 20,
            jpeg_quality: 75,
        }
    }
}

impl PdfPageCompressSaver {
    pub fn new(config: PageCompressConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(PageCompressConfig::default())
    }

    /// Compress a page image to target patch resolution.
    pub fn compress_page(&self, img: &DynamicImage) -> Vec<u8> {
        // DocOwl2 uses 18×18 patches per page → 504×504 effective resolution
        let target_size = 504u32;
        let resized = img.resize_exact(
            target_size, target_size,
            image::imageops::FilterType::Lanczos3,
        );
        let mut buf = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
            &mut buf, self.config.jpeg_quality,
        );
        encoder.encode_image(&resized).ok();
        buf
    }

    pub fn annotate_document(pages: usize, total_tokens: usize) -> String {
        format!(
            "[PDF DOCUMENT: {} pages, ~{} tokens total ({} per page)]",
            pages, total_tokens, total_tokens / pages.max(1)
        )
    }
}

#[async_trait::async_trait]
impl MultiModalTokenSaver for PdfPageCompressSaver {
    fn modality(&self) -> Modality { Modality::Document }
}

#[async_trait::async_trait]
impl TokenSaver for PdfPageCompressSaver {
    fn name(&self) -> &str { "pdf-page-compress" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 64 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut total_saved = 0usize;
        let mut new_images = Vec::new();
        let mut page_count = 0usize;

        for img_bytes in &input.images {
            if page_count >= self.config.max_pages {
                // Drop excess pages
                total_saved += img_bytes.len() / 4 / 100;
                continue;
            }

            let img = match image::load_from_memory(img_bytes.as_slice()) {
                Ok(i) => i,
                Err(_) => { new_images.push(img_bytes.clone()); continue; }
            };

            let original_est = img_bytes.len() * 8 / 4; // rough token estimate
            let compressed = self.compress_page(&img);
            let compressed_tokens = self.config.target_tokens_per_page;

            total_saved += original_est.saturating_sub(compressed_tokens * 4);
            new_images.push(compressed);
            page_count += 1;
        }

        input.images = new_images;

        if page_count > 0 {
            let annotation = Self::annotate_document(
                page_count,
                page_count * self.config.target_tokens_per_page,
            );
            let mut doc_msg = Message::default();
            doc_msg.role = "system".into();
            doc_msg.content = annotation;
            doc_msg.token_count = doc_msg.content.len() / 4;
            input.messages.push(doc_msg);
        }

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "pdf-page-compress".into(),
                tokens_before: total_saved,
                tokens_after: page_count * self.config.target_tokens_per_page,
                tokens_saved: total_saved,
                description: format!("compressed {} PDF pages to DocOwl2 resolution", page_count),
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
