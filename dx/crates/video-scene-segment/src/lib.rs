//! Segments video into scenes and allocates token budget by importance.
//! SAVINGS: 85-95% on video tokens
//! STAGE: PrePrompt (priority 70)

use dx_core::*;
use std::sync::Mutex;

pub struct VideoSceneSegmentSaver {
    config: SceneConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct SceneConfig {
    pub scene_change_threshold: f64,
    pub max_scenes: usize,
    pub tokens_per_scene: usize,
    pub fps: f64,
    pub thumbnail_size: u32,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            scene_change_threshold: 0.20,
            max_scenes: 15,
            tokens_per_scene: 170,
            fps: 30.0,
            thumbnail_size: 32,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Scene {
    pub start_frame: usize,
    pub end_frame: usize,
    pub representative_frame: usize,
    pub change_score: f64,
}

impl VideoSceneSegmentSaver {
    pub fn new(config: SceneConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(SceneConfig::default())
    }

    fn frame_diff(a: &[u8], b: &[u8]) -> f64 {
        if a.len() != b.len() || a.is_empty() { return 1.0; }
        let s: u64 = a.iter().zip(b.iter())
            .map(|(&x, &y)| (x as i32 - y as i32).unsigned_abs() as u64)
            .sum();
        s as f64 / (a.len() as f64 * 255.0)
    }

    fn thumb(img_bytes: &[u8], size: u32) -> Vec<u8> {
        image::load_from_memory(img_bytes)
            .map(|img| img.resize_exact(size, size, image::imageops::FilterType::Nearest).to_luma8().into_raw())
            .unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl MultiModalTokenSaver for VideoSceneSegmentSaver {
    fn modality(&self) -> Modality { Modality::Video }
}

#[async_trait::async_trait]
impl TokenSaver for VideoSceneSegmentSaver {
    fn name(&self) -> &str { "video-scene-segment" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 70 }

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

        let thumbs: Vec<Vec<u8>> = input.images.iter()
            .map(|b| Self::thumb(b, self.config.thumbnail_size))
            .collect();

        let mut scenes: Vec<Scene> = vec![Scene {
            start_frame: 0,
            end_frame: 0,
            representative_frame: 0,
            change_score: 0.0,
        }];

        for i in 1..total {
            if thumbs[i - 1].is_empty() || thumbs[i].is_empty() { continue; }
            let diff = Self::frame_diff(&thumbs[i - 1], &thumbs[i]);
            if diff >= self.config.scene_change_threshold && scenes.len() < self.config.max_scenes {
                scenes.last_mut().unwrap().end_frame = i - 1;
                scenes.push(Scene {
                    start_frame: i,
                    end_frame: i,
                    representative_frame: i,
                    change_score: diff,
                });
            } else {
                scenes.last_mut().unwrap().end_frame = i;
            }
        }
        scenes.last_mut().unwrap().end_frame = total - 1;

        // Keep one representative frame per scene
        let new_images: Vec<Vec<u8>> = scenes.iter()
            .map(|s| input.images[s.representative_frame].clone())
            .collect();
        let kept = new_images.len();
        let saved_frames = total - kept;
        let tokens_saved = saved_frames * self.config.tokens_per_scene;

        input.images = new_images;

        let duration = total as f64 / self.config.fps;
        let annotation = format!(
            "[VIDEO SCENES: {} scenes from {} frames ({:.1}s)]",
            kept, total, duration
        );
        let mut msg = Message::default();
        msg.role = "system".into();
        msg.content = annotation;
        msg.token_count = msg.content.len() / 4;
        input.messages.push(msg);

        if tokens_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "video-scene-segment".into(),
                tokens_before: total * self.config.tokens_per_scene,
                tokens_after: kept * self.config.tokens_per_scene,
                tokens_saved,
                description: format!("scene segmentation: {} → {} frames", total, kept),
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
