//! Selects high-information regions from images rather than full image.
//! SAVINGS: 40-80% vs sending full image
//! STAGE: PrePrompt (priority 8)

use dx_core::*;
use image::{DynamicImage, GrayImage, GenericImageView};
use std::sync::Mutex;
use vision_compress::estimate_tokens;

pub struct VisionSelectSaver {
    max_crops: usize,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Debug, Clone)]
struct Region {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    score: f64,
}

impl VisionSelectSaver {
    pub fn new(max_crops: usize) -> Self {
        Self {
            max_crops,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(3)
    }

    /// Detect regions-of-interest via 4×4 grid edge analysis.
    pub fn detect_rois(&self, img: &DynamicImage) -> Vec<Region> {
        let gray = img.to_luma8();
        let (width, height) = gray.dimensions();
        let grid = 4usize;
        let cell_w = width / grid as u32;
        let cell_h = height / grid as u32;

        let mut regions = Vec::new();

        for gy in 0..grid {
            for gx in 0..grid {
                let x = gx as u32 * cell_w;
                let y = gy as u32 * cell_h;

                let score = self.cell_edge_density(&gray, x, y, cell_w, cell_h);
                regions.push(Region { x, y, w: cell_w, h: cell_h, score });
            }
        }

        regions.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        regions.truncate(self.max_crops * 2);
        let merged = self.merge_adjacent(regions);
        merged.into_iter().take(self.max_crops).collect()
    }

    fn cell_edge_density(&self, gray: &GrayImage, x: u32, y: u32, w: u32, h: u32) -> f64 {
        let mut edge_count = 0u64;
        let mut total = 0u64;
        for py in y..y.saturating_add(h).min(gray.height() - 1) {
            for px in x..x.saturating_add(w).min(gray.width() - 1) {
                let v = gray.get_pixel(px, py).0[0] as i32;
                let vr = gray.get_pixel(px + 1, py).0[0] as i32;
                let vd = gray.get_pixel(px, py + 1).0[0] as i32;
                let grad = (vr - v).abs() + (vd - v).abs();
                if grad > 20 { edge_count += 1; }
                total += 1;
            }
        }
        if total == 0 { 0.0 } else { edge_count as f64 / total as f64 }
    }

    fn are_adjacent(&self, a: &Region, b: &Region) -> bool {
        let ax2 = a.x + a.w;
        let ay2 = a.y + a.h;
        let bx2 = b.x + b.w;
        let by2 = b.y + b.h;
        let overlap_x = a.x < bx2 && ax2 > b.x;
        let overlap_y = a.y < by2 && ay2 > b.y;
        let touch_x = ax2 == b.x || bx2 == a.x;
        let touch_y = ay2 == b.y || by2 == a.y;
        (overlap_x && touch_y) || (overlap_y && touch_x)
    }

    fn merge_adjacent(&self, mut regions: Vec<Region>) -> Vec<Region> {
        if regions.len() <= 1 { return regions; }
        let mut merged = vec![regions.remove(0)];
        for r in regions {
            let adj = merged.iter_mut().find(|m| self.are_adjacent(m, &r));
            if let Some(m) = adj {
                let x = m.x.min(r.x);
                let y = m.y.min(r.y);
                let x2 = (m.x + m.w).max(r.x + r.w);
                let y2 = (m.y + m.h).max(r.y + r.h);
                m.x = x; m.y = y;
                m.w = x2 - x; m.h = y2 - y;
                m.score = m.score.max(r.score);
            } else {
                merged.push(r);
            }
        }
        merged
    }
}

#[async_trait::async_trait]
impl TokenSaver for VisionSelectSaver {
    fn name(&self) -> &str { "vision-select" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 8 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut total_saved = 0usize;
        let mut new_images = Vec::new();

        for img_data in &input.images {
            let decoded = image::load_from_memory(img_data.as_slice())
                .map_err(|e| SaverError::ProcessingError(e.to_string()))?;

            let full_tokens = estimate_tokens(
                decoded.width(),
                decoded.height(),
                vision_compress::Detail::High,
            );

            let rois = self.detect_rois(&decoded);
            if rois.is_empty() {
                new_images.push(img_data.clone());
                continue;
            }

            let mut crop_tokens = 0usize;
            let mut selected_crops = Vec::new();

            for roi in &rois {
                let crop = decoded.crop_imm(roi.x, roi.y, roi.w, roi.h);
                crop_tokens += estimate_tokens(crop.width(), crop.height(), vision_compress::Detail::Low);
                let mut buf = Vec::new();
                crop.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Jpeg)
                    .map_err(|e| SaverError::ProcessingError(e.to_string()))?;
                selected_crops.push(buf);
            }

            if crop_tokens < full_tokens {
                total_saved += full_tokens.saturating_sub(crop_tokens);
                new_images.extend(selected_crops);
            } else {
                new_images.push(img_data.clone());
            }
        }

        input.images = new_images;

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "vision-select".into(),
                tokens_before: total_saved,
                tokens_after: 0,
                tokens_saved: total_saved,
                description: format!("ROI crop selection saved {} tokens", total_saved),
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
