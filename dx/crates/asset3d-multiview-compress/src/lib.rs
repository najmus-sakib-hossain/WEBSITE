//! Reduces 3D asset viewpoints from 8-12 to 3-4 optimal views.
//! SAVINGS: 60-70% on 3D asset rendering tokens
//! STAGE: PrePrompt (priority 80)

use dx_core::*;
use std::sync::Mutex;

pub struct Asset3dMultiviewCompressSaver {
    config: MultiviewConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct MultiviewConfig {
    /// Maximum number of viewpoints to keep
    pub max_views: usize,
    /// Tokens per view estimate
    pub tokens_per_view: usize,
    /// Prefer canonical views (front, side, top, isometric)
    pub prefer_canonical: bool,
}

impl Default for MultiviewConfig {
    fn default() -> Self {
        Self {
            max_views: 4,
            tokens_per_view: 512,
            prefer_canonical: true,
        }
    }
}

/// Canonical view angles: front(0°), right(90°), top(270° elevation), isometric
const CANONICAL_AZIMUTHS: &[f32] = &[0.0, 90.0, 180.0, 270.0];

impl Asset3dMultiviewCompressSaver {
    pub fn new(config: MultiviewConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(MultiviewConfig::default())
    }

    /// Select the most informative subset of views.
    /// Views are assumed ordered: front, back, left, right, top, bottom, iso1, iso2...
    pub fn select_views(&self, total_views: usize) -> Vec<usize> {
        if total_views <= self.config.max_views {
            return (0..total_views).collect();
        }

        if self.config.prefer_canonical && total_views >= 4 {
            // front=0, right-side=total/4, top=total/2, isometric=3*total/4
            vec![
                0,
                total_views / 4,
                total_views / 2,
                3 * total_views / 4,
            ]
            .into_iter()
            .take(self.config.max_views)
            .collect()
        } else {
            // Evenly spaced
            let step = total_views / self.config.max_views;
            (0..self.config.max_views).map(|i| i * step).collect()
        }
    }
}

#[async_trait::async_trait]
impl MultiModalTokenSaver for Asset3dMultiviewCompressSaver {
    fn name(&self) -> &str { "asset3d-multiview-compress" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 80 }
    fn modality(&self) -> Modality { Modality::Asset3d }

    async fn process_multimodal(
        &self,
        mut input: MultiModalSaverInput,
        _ctx: &SaverContext,
    ) -> Result<MultiModalSaverOutput, SaverError> {
        let total_views = input.base.images.len();
        if total_views <= self.config.max_views {
            return Ok(MultiModalSaverOutput {
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
            });
        }

        let selected = self.select_views(total_views);
        let kept = selected.len();
        let new_images: Vec<ImageInput> = selected.iter()
            .map(|&i| input.base.images[i].clone())
            .collect();

        let saved_views = total_views - kept;
        let tokens_saved = saved_views * self.config.tokens_per_view;
        input.base.images = new_images;

        let annotation = format!(
            "[3D ASSET: {} views selected from {} (canonical: front/side/top/iso)]",
            kept, total_views
        );
        let ann_tokens = annotation.len() / 4;
        input.base.messages.push(Message {
            role: "system".into(),
            content: annotation,
            images: Vec::new(),
            tool_call_id: None,
            token_count: ann_tokens,
        });

        if tokens_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "asset3d-multiview-compress".into(),
                tokens_before: total_views * self.config.tokens_per_view,
                tokens_after: kept * self.config.tokens_per_view,
                tokens_saved,
                description: format!("multiview: {} → {} views", total_views, kept),
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
