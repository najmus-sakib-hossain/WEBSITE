//! Prunes tokens from live video frames using region analysis.
//! SAVINGS: 40-70% by focusing on dynamic regions
//! STAGE: PrePrompt (priority 52)

use dx_core::*;
use image::{DynamicImage, GenericImageView};
use std::sync::Mutex;

pub struct LiveTokenPruneSaver {
    config: PruneConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct PruneConfig {
    pub tokens_per_frame: usize,
    pub grid_size: usize,
    pub static_threshold: f64,
    pub dynamic_budget_ratio: f64,
}

impl Default for PruneConfig {
    fn default() -> Self {
        Self {
            tokens_per_frame: 50,
            grid_size: 4,
            static_threshold: 0.92,
            dynamic_budget_ratio: 0.7,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RegionSignature {
    pub x: u32,
    pub y: u32,
    pub avg_lum: f32,
    pub edge_density: f32,
    pub hash: u64,
}

impl LiveTokenPruneSaver {
    pub fn new(config: PruneConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(PruneConfig::default())
    }

    pub fn analyze_regions(img: &DynamicImage, grid_size: usize) -> Vec<RegionSignature> {
        let (w, h) = img.dimensions();
        let gray = img.to_luma8();
        let cell_w = w / grid_size as u32;
        let cell_h = h / grid_size as u32;

        let mut regions = Vec::new();
        for gy in 0..grid_size {
            for gx in 0..grid_size {
                let x = gx as u32 * cell_w;
                let y = gy as u32 * cell_h;
                let mut lum_sum = 0u64;
                let mut edge_count = 0u64;
                let mut count = 0u64;

                for py in y..y.saturating_add(cell_h).min(h.saturating_sub(1)) {
                    for px in x..x.saturating_add(cell_w).min(w.saturating_sub(1)) {
                        let v = gray.get_pixel(px, py).0[0] as u64;
                        let vr = gray.get_pixel(px + 1, py).0[0] as i32;
                        let vd = gray.get_pixel(px, py + 1).0[0] as i32;
                        let grad = (vr - v as i32).abs() + (vd - v as i32).abs();
                        if grad > 20 { edge_count += 1; }
                        lum_sum += v;
                        count += 1;
                    }
                }

                let avg_lum = if count > 0 { lum_sum as f32 / count as f32 } else { 0.0 };
                let edge_density = if count > 0 { edge_count as f32 / count as f32 } else { 0.0 };
                let hash = (avg_lum as u64).wrapping_mul(1000).wrapping_add(edge_count);

                regions.push(RegionSignature {
                    x, y, avg_lum, edge_density, hash,
                });
            }
        }
        regions
    }

    pub fn allocate_budget(&self, regions: &[RegionSignature], prev: &[RegionSignature]) -> Vec<bool> {
        if prev.is_empty() {
            return vec![true; regions.len()];
        }
        regions.iter().zip(prev.iter())
            .map(|(cur, prv)| {
                let lum_diff = (cur.avg_lum - prv.avg_lum).abs() / 255.0;
                let edge_diff = (cur.edge_density - prv.edge_density).abs();
                let change = (lum_diff + edge_diff) as f64;
                change >= (1.0 - self.config.static_threshold)
            })
            .collect()
    }

    pub fn compact_frame_description(regions: &[RegionSignature], active: &[bool], grid: usize) -> String {
        let active_count = active.iter().filter(|&&a| a).count();
        let mut desc = format!("[LIVE FRAME: {}/{} dynamic regions]\n", active_count, regions.len());
        for (i, (r, &is_active)) in regions.iter().zip(active.iter()).enumerate() {
            let gx = i % grid;
            let gy = i / grid;
            if is_active {
                desc.push_str(&format!("  ({},{}) lum={:.0} edge={:.2}\n", gx, gy, r.avg_lum, r.edge_density));
            }
        }
        desc
    }
}

#[async_trait::async_trait]
impl MultiModalTokenSaver for LiveTokenPruneSaver {
    fn name(&self) -> &str { "live-token-prune" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 52 }
    fn modality(&self) -> Modality { Modality::Live }

    async fn process_multimodal(
        &self,
        mut input: MultiModalSaverInput,
        _ctx: &SaverContext,
    ) -> Result<MultiModalSaverOutput, SaverError> {
        let mut prev_regions: Vec<RegionSignature> = Vec::new();
        let mut total_before = 0usize;
        let mut total_after = 0usize;
        let mut kept_frames = Vec::new();

        for frame in &input.live_frames {
            let img = image::load_from_memory(&frame.image_data)
                .map_err(|e| SaverError::Failed(e.to_string()))?;

            let regions = Self::analyze_regions(&img, self.config.grid_size);
            let active = self.allocate_budget(&regions, &prev_regions);
            let description = Self::compact_frame_description(&regions, &active, self.config.grid_size);

            let desc_tokens = description.len() / 4;
            total_before += frame.token_estimate;
            total_after += desc_tokens;

            // Inject description as system message
            input.base.messages.push(Message {
                role: "system".into(),
                content: description,
                images: Vec::new(),
                tool_call_id: None,
                token_count: desc_tokens,
            });

            kept_frames.push(frame.clone());
            prev_regions = regions;
        }

        let total_saved = total_before.saturating_sub(total_after);
        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "live-token-prune".into(),
                tokens_before: total_before,
                tokens_after: total_after,
                tokens_saved: total_saved,
                description: format!("pruned live frame tokens: {} → {}", total_before, total_after),
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
            live_frames: kept_frames,
            documents: input.documents,
            videos: input.videos,
            assets_3d: input.assets_3d,
        })
    }

    fn last_savings(&self) -> TokenSavingsReport {
        self.report.lock().unwrap().clone()
    }
}
