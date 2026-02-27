//! Reduces image token cost by 70-90% through downscaling and detail level control.
//! SAVINGS: 70-96% per image
//! STAGE: PrePrompt (priority 10)

use dx_core::*;
use image::{GenericImageView, imageops::FilterType};
use std::sync::Mutex;

pub struct VisionCompressSaver {
    config: VisionConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct VisionConfig {
    pub max_dimension: u32,
    pub default_detail: ImageDetail,
    pub max_image_tokens_per_turn: usize,
    pub jpeg_quality: u8,
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            max_dimension: 1024,
            default_detail: ImageDetail::Low,
            max_image_tokens_per_turn: 500,
            jpeg_quality: 80,
        }
    }
}

impl VisionCompressSaver {
    pub fn new(config: VisionConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(VisionConfig::default())
    }

    /// Estimate token cost using OpenAI's tile-based formula.
    pub fn estimate_tokens(w: u32, h: u32, detail: ImageDetail) -> usize {
        match detail {
            ImageDetail::Low => 85,
            ImageDetail::High | ImageDetail::Auto => {
                // Scale so shortest side = 768, cap at 2048
                let scale = 768.0 / w.min(h) as f64;
                let sw = (w as f64 * scale).min(2048.0) as u32;
                let sh = (h as f64 * scale).min(2048.0) as u32;
                let tiles_w = (sw as f64 / 512.0).ceil() as usize;
                let tiles_h = (sh as f64 / 512.0).ceil() as usize;
                85 + tiles_w * tiles_h * 170
            }
        }
    }

    pub fn process_single(&self, img_data: &[u8]) -> Result<(Vec<u8>, ImageDetail, usize, usize), SaverError> {
        let img = image::load_from_memory(img_data)
            .map_err(|e| SaverError::Failed(format!("image decode: {}", e)))?;

        let (w, h) = img.dimensions();
        let original_tokens = Self::estimate_tokens(w, h, ImageDetail::High);

        // Downscale if exceeds max dimension
        let resized = if w > self.config.max_dimension || h > self.config.max_dimension {
            let scale = self.config.max_dimension as f64 / w.max(h) as f64;
            let nw = (w as f64 * scale) as u32;
            let nh = (h as f64 * scale) as u32;
            img.resize(nw.max(1), nh.max(1), FilterType::Lanczos3)
        } else {
            img
        };

        let (rw, rh) = resized.dimensions();

        // Choose detail level
        let detail = match self.config.default_detail {
            ImageDetail::Auto => {
                let high_tokens = Self::estimate_tokens(rw, rh, ImageDetail::High);
                if high_tokens <= self.config.max_image_tokens_per_turn {
                    ImageDetail::High
                } else {
                    ImageDetail::Low
                }
            }
            d => d,
        };

        let processed_tokens = Self::estimate_tokens(rw, rh, detail);

        // Encode as JPEG
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        resized.write_to(&mut cursor, image::ImageFormat::Jpeg)
            .map_err(|e| SaverError::Failed(format!("jpeg encode: {}", e)))?;

        Ok((buf, detail, original_tokens, processed_tokens))
    }
}

#[async_trait::async_trait]
impl TokenSaver for VisionCompressSaver {
    fn name(&self) -> &str { "vision-compress" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 10 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        if input.images.is_empty() {
            return Ok(SaverOutput {
                messages: input.messages,
                tools: input.tools,
                images: input.images,
                skipped: false,
                cached_response: None,
            });
        }

        let mut total_original = 0usize;
        let mut total_processed = 0usize;
        let mut new_images = Vec::with_capacity(input.images.len());

        for img in &input.images {
            match self.process_single(&img.data) {
                Ok((data, detail, orig, proc)) => {
                    total_original += orig;
                    total_processed += proc;
                    new_images.push(ImageInput {
                        data,
                        mime: "image/jpeg".into(),
                        detail,
                        original_tokens: orig,
                        processed_tokens: proc,
                    });
                }
                Err(_) => {
                    // Keep original on error
                    total_original += img.original_tokens;
                    total_processed += img.original_tokens;
                    new_images.push(img.clone());
                }
            }
        }

        let saved = total_original.saturating_sub(total_processed);
        if saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "vision-compress".into(),
                tokens_before: total_original,
                tokens_after: total_processed,
                tokens_saved: saved,
                description: format!(
                    "{} images: {} → {} tokens ({:.1}% saved)",
                    new_images.len(), total_original, total_processed,
                    saved as f64 / total_original.max(1) as f64 * 100.0
                ),
            };
        }

        Ok(SaverOutput {
            messages: input.messages,
            tools: input.tools,
            images: new_images,
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
    fn test_low_detail_always_85() {
        assert_eq!(VisionCompressSaver::estimate_tokens(1920, 1080, ImageDetail::Low), 85);
        assert_eq!(VisionCompressSaver::estimate_tokens(100, 100, ImageDetail::Low), 85);
        assert_eq!(VisionCompressSaver::estimate_tokens(4000, 3000, ImageDetail::Low), 85);
    }

    #[test]
    fn test_high_detail_1920x1080() {
        let tokens = VisionCompressSaver::estimate_tokens(1920, 1080, ImageDetail::High);
        assert!(tokens > 85);
        assert!(tokens < 3000);
    }

    #[test]
    fn test_savings_ratio() {
        let high = VisionCompressSaver::estimate_tokens(1920, 1080, ImageDetail::High);
        let low = VisionCompressSaver::estimate_tokens(1920, 1080, ImageDetail::Low);
        let savings = (high - low) as f64 / high as f64;
        assert!(savings > 0.9, "expected >90% savings, got {:.1}%", savings * 100.0);
    }
}
