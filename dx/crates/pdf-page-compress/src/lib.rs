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
#[async_trait::async_trait]
impl MultiModalTokenSaver for PdfPageCompressSaver {
    fn name(&self) -> &str { "pdf-page-compress" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 64 }
    fn modality(&self) -> Modality { Modality::Document }

    async fn process_multimodal(
        &self,
        mut input: MultiModalSaverInput,
        _ctx: &SaverContext,
    ) -> Result<MultiModalSaverOutput, SaverError> {
        let mut total_before = 0usize;
        let mut total_after = 0usize;
        let mut new_images = Vec::new();
        let mut page_count = 0usize;

        for img in &input.base.images {
            if page_count >= self.config.max_pages {
                total_before += img.original_tokens;
                // dropped
                continue;
            }

            let decoded = match image::load_from_memory(&img.data) {
                Ok(i) => i,
                Err(_) => { new_images.push(img.clone()); total_before += img.original_tokens; total_after += img.original_tokens; continue; }
            };

            total_before += img.original_tokens;
            let compressed_bytes = self.compress_page(&decoded);
            let compressed_tokens = self.config.target_tokens_per_page;
            total_after += compressed_tokens;

            new_images.push(ImageInput {
                data: compressed_bytes,
                mime: "image/jpeg".into(),
                detail: ImageDetail::Low,
                original_tokens: img.original_tokens,
                processed_tokens: compressed_tokens,
            });
            page_count += 1;
        }

        let total_saved = total_before.saturating_sub(total_after);
        if page_count > 0 {
            let annotation = Self::annotate_document(
                page_count,
                page_count * self.config.target_tokens_per_page,
            );
            let tokens = annotation.len() / 4;
            input.base.messages.push(Message {
                role: "system".into(),
                content: annotation,
                images: Vec::new(),
                tool_call_id: None,
                token_count: tokens,
            });
        }

        input.base.images = new_images;

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "pdf-page-compress".into(),
                tokens_before: total_before,
                tokens_after: total_after,
                tokens_saved: total_saved,
                description: format!("compressed {} PDF pages: {} → {} tokens", page_count, total_before, total_after),
            };
        }

        Ok(MultiModalSaverOutput {
            base: SaverOutput {
                messages: input.base.messages,
                tools: input.base.tools,
                images: input.base.images,
                skipped: false,
                cached_response: None,
            },
            audio: input.audio,
            live_frames: input.live_frames,
            documents: input.documents,
            videos: input.videos,
            assets_3d: input.assets_3d,
        })
    }

    fn last_savings(&self) -> TokenSavingsReport {
        self.report.lock().unwrap().clone()
    }
}
