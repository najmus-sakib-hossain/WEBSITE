//! Deduplicates nearly-identical frames in live video streams.
//! SAVINGS: 70-95% on static/slow-changing video
//! STAGE: PrePrompt (priority 50)

use dx_core::*;
use image::{DynamicImage, GenericImageView};
use std::sync::Mutex;

pub struct LiveFrameDedupSaver {
    config: FrameDedupConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct FrameDedupConfig {
    pub change_threshold: f64,
    pub thumbnail_size: u32,
    pub max_skip_count: usize,
    pub min_keyframe_interval_secs: f64,
}

impl Default for FrameDedupConfig {
    fn default() -> Self {
        Self {
            change_threshold: 0.05,
            thumbnail_size: 64,
            max_skip_count: 30,
            min_keyframe_interval_secs: 0.5,
        }
    }
}

impl LiveFrameDedupSaver {
    pub fn new(config: FrameDedupConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(FrameDedupConfig::default())
    }

    /// Create small thumbnail for comparison.
    pub fn create_thumbnail(&self, img: &DynamicImage) -> Vec<u8> {
        let thumb = img.resize_exact(
            self.config.thumbnail_size,
            self.config.thumbnail_size,
            image::imageops::FilterType::Nearest,
        );
        thumb.to_luma8().into_raw()
    }

    /// Compute normalized pixel difference between two thumbnails.
    pub fn frame_difference(a: &[u8], b: &[u8]) -> f64 {
        if a.len() != b.len() || a.is_empty() { return 1.0; }
        let total: u64 = a.iter().zip(b.iter())
            .map(|(&x, &y)| (x as i32 - y as i32).unsigned_abs() as u64)
            .sum();
        total as f64 / (a.len() as f64 * 255.0)
    }

    /// Determine if frame should be kept as keyframe.
    pub fn should_keep_frame(&self, diff: f64, skip_count: usize) -> bool {
        diff >= self.config.change_threshold || skip_count >= self.config.max_skip_count
    }
}

#[async_trait::async_trait]
impl MultiModalTokenSaver for LiveFrameDedupSaver {
    fn modality(&self) -> Modality { Modality::Live }
}

#[async_trait::async_trait]
impl TokenSaver for LiveFrameDedupSaver {
    fn name(&self) -> &str { "live-frame-dedup" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 50 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut prev_thumb: Option<Vec<u8>> = None;
        let mut skip_count = 0usize;
        let mut total_saved = 0usize;
        let mut new_images = Vec::new();

        for img_bytes in &input.images {
            let img = image::load_from_memory(img_bytes.as_slice())
                .map_err(|e| SaverError::ProcessingError(e.to_string()))?;
            let thumb = self.create_thumbnail(&img);

            let keep = if let Some(ref prev) = prev_thumb {
                let diff = Self::frame_difference(&thumb, prev);
                self.should_keep_frame(diff, skip_count)
            } else {
                true
            };

            if keep {
                prev_thumb = Some(thumb);
                skip_count = 0;
                new_images.push(img_bytes.clone());
            } else {
                skip_count += 1;
                // Rough token estimate per dropped frame
                total_saved += img_bytes.len() / 4 / 100;
            }
        }

        input.images = new_images;

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "live-frame-dedup".into(),
                tokens_before: total_saved,
                tokens_after: 0,
                tokens_saved: total_saved,
                description: format!("deduped live frames, saved ~{} tokens", total_saved),
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
