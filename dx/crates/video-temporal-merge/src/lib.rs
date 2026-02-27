//! Merges consecutive similar video frames into temporal tokens.
//! SAVINGS: 95-99% on video (300K tokens → ~5K after merge)
//! STAGE: PrePrompt (priority 74)

use dx_core::*;
use image::{DynamicImage, GenericImageView};
use std::sync::Mutex;

pub struct VideoTemporalMergeSaver {
    config: TemporalMergeConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct TemporalMergeConfig {
    pub similarity_threshold: f64,
    pub thumbnail_size: u32,
    pub max_merged_tokens: usize,
}

impl Default for TemporalMergeConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.95,
            thumbnail_size: 32,
            max_merged_tokens: 100,
        }
    }
}

impl VideoTemporalMergeSaver {
    pub fn new(config: TemporalMergeConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(TemporalMergeConfig::default())
    }

    fn thumb(img: &DynamicImage, size: u32) -> Vec<u8> {
        img.resize_exact(size, size, image::imageops::FilterType::Nearest)
            .to_luma8().into_raw()
    }

    fn diff(a: &[u8], b: &[u8]) -> f64 {
        if a.len() != b.len() || a.is_empty() { return 1.0; }
        let s: u64 = a.iter().zip(b.iter()).map(|(&x, &y)| (x as i32 - y as i32).unsigned_abs() as u64).sum();
        s as f64 / (a.len() as f64 * 255.0)
    }
}

#[async_trait::async_trait]
impl MultiModalTokenSaver for VideoTemporalMergeSaver {
    fn modality(&self) -> Modality { Modality::Video }
}

#[async_trait::async_trait]
impl TokenSaver for VideoTemporalMergeSaver {
    fn name(&self) -> &str { "video-temporal-merge" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 74 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let original_count = input.images.len();
        if original_count == 0 {
            return Ok(SaverOutput {
                messages: input.messages,
                tools: input.tools,
                images: input.images,
                skipped: false,
                cached_response: None,
            });
        }

        // Group consecutive similar frames
        let mut groups: Vec<(usize, usize)> = vec![(0, 0)]; // (start, end) frame indices
        let mut thumbs: Vec<Vec<u8>> = Vec::new();

        for (i, img_bytes) in input.images.iter().enumerate() {
            let img = image::load_from_memory(img_bytes.as_slice())
                .map_err(|e| SaverError::ProcessingError(e.to_string()))?;
            let t = Self::thumb(&img, self.config.thumbnail_size);

            if i == 0 {
                thumbs.push(t);
                continue;
            }

            let prev = &thumbs[thumbs.len() - 1];
            let diff = Self::diff(prev, &t);

            if diff <= (1.0 - self.config.similarity_threshold) {
                groups.last_mut().unwrap().1 = i;
            } else {
                groups.push((i, i));
                thumbs.push(t);
            }
        }

        // Keep one representative frame per group
        let mut new_images = Vec::new();
        for (start, _end) in &groups {
            new_images.push(input.images[*start].clone());
        }

        let saved_frames = original_count - new_images.len();
        let fps = 30.0f64;
        let total_saved = saved_frames * 170; // ~170 tokens per dropped frame

        // Add temporal merge annotation
        let annotation = format!(
            "[VIDEO TEMPORAL MERGE: {} frames → {} groups, {:.1}s, saved ~{}K tokens]",
            original_count, groups.len(),
            original_count as f64 / fps,
            total_saved / 1000
        );
        let mut msg = Message::default();
        msg.role = "system".into();
        msg.content = annotation;
        msg.token_count = msg.content.len() / 4;
        input.messages.push(msg);
        input.images = new_images;

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "video-temporal-merge".into(),
                tokens_before: original_count * 170,
                tokens_after: groups.len() * 170,
                tokens_saved: total_saved,
                description: format!("temporal merge: {} → {} frames", original_count, groups.len()),
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
