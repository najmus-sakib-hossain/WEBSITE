//! Selects keyframes from video at maximum 1 per 5 seconds.
//! SAVINGS: 90-98% on video frame tokens
//! STAGE: PrePrompt (priority 72)

use dx_core::*;
use image::DynamicImage;
use std::sync::Mutex;

pub struct VideoKeyframeSelectSaver {
    config: KeyframeConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct KeyframeConfig {
    pub keyframe_interval_secs: f64,
    pub max_keyframes: usize,
    pub fps: f64,
}

impl Default for KeyframeConfig {
    fn default() -> Self {
        Self {
            keyframe_interval_secs: 5.0,
            max_keyframes: 20,
            fps: 30.0,
        }
    }
}

impl VideoKeyframeSelectSaver {
    pub fn new(config: KeyframeConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(KeyframeConfig::default())
    }

    fn frame_interval(&self) -> usize {
        (self.config.fps * self.config.keyframe_interval_secs) as usize
    }

    fn select_indices(&self, total_frames: usize) -> Vec<usize> {
        let interval = self.frame_interval().max(1);
        let mut indices = Vec::new();
        let mut i = 0;
        while i < total_frames && indices.len() < self.config.max_keyframes {
            indices.push(i);
            i += interval;
        }
        indices
    }
}

#[async_trait::async_trait]
impl MultiModalTokenSaver for VideoKeyframeSelectSaver {
    fn modality(&self) -> Modality { Modality::Video }
}

#[async_trait::async_trait]
impl TokenSaver for VideoKeyframeSelectSaver {
    fn name(&self) -> &str { "video-keyframe-select" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 72 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let total = input.images.len();
        if total == 0 {
            return Ok(SaverOutput {
                messages: input.messages,
                tools: input.tools,
                images: input.images,
                skipped: false,
                cached_response: None,
            });
        }

        let keyframe_indices = self.select_indices(total);
        let kept = keyframe_indices.len();
        let saved_frames = total - kept;
        let tokens_saved = saved_frames * 170;

        let new_images: Vec<Vec<u8>> = keyframe_indices.iter()
            .map(|&i| input.images[i].clone())
            .collect();
        input.images = new_images;

        let duration = total as f64 / self.config.fps;
        let annotation = format!(
            "[VIDEO KEYFRAMES: {} selected from {} frames ({:.1}s) at {}s intervals]",
            kept, total, duration, self.config.keyframe_interval_secs
        );
        let mut msg = Message::default();
        msg.role = "system".into();
        msg.content = annotation;
        msg.token_count = msg.content.len() / 4;
        input.messages.push(msg);

        if tokens_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "video-keyframe-select".into(),
                tokens_before: total * 170,
                tokens_after: kept * 170,
                tokens_saved,
                description: format!("keyframe selection: {} → {} frames", total, kept),
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
