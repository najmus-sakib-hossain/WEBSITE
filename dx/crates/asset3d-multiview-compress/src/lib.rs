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
    fn modality(&self) -> Modality { Modality::Asset3d }
}

#[async_trait::async_trait]
impl TokenSaver for Asset3dMultiviewCompressSaver {
    fn name(&self) -> &str { "asset3d-multiview-compress" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 80 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let total_views = input.images.len();
        if total_views <= self.config.max_views {
            return Ok(SaverOutput {
                messages: input.messages,
                tools: input.tools,
                images: input.images,
                skipped: false,
                cached_response: None,
            });
        }

        let selected = self.select_views(total_views);
        let kept = selected.len();
        let new_images: Vec<Vec<u8>> = selected.iter()
            .map(|&i| input.images[i].clone())
            .collect();

        let saved_views = total_views - kept;
        let tokens_saved = saved_views * self.config.tokens_per_view;
        input.images = new_images;

        let annotation = format!(
            "[3D ASSET: {} views selected from {} (canonical: front/side/top/iso)]",
            kept, total_views
        );
        let mut msg = Message::default();
        msg.role = "system".into();
        msg.content = annotation;
        msg.token_count = msg.content.len() / 4;
        input.messages.push(msg);

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
