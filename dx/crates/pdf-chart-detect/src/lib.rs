//! Detects chart/table regions in PDF page images.
//! SAVINGS: Enables targeted processing of chart regions
//! STAGE: PrePrompt (priority 62)

use dx_core::*;
use image::{DynamicImage, GenericImageView};
use std::sync::Mutex;

pub struct PdfChartDetectSaver {
    config: ChartDetectConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct ChartDetectConfig {
    pub grid_density_threshold: f64,
    pub min_region_fraction: f64,
    pub annotation_token_budget: usize,
}

impl Default for ChartDetectConfig {
    fn default() -> Self {
        Self {
            grid_density_threshold: 0.15,
            min_region_fraction: 0.05,
            annotation_token_budget: 50,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DetectedRegion {
    pub kind: RegionKind,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RegionKind {
    Chart,
    Table,
    Text,
    Unknown,
}

impl PdfChartDetectSaver {
    pub fn new(config: ChartDetectConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(ChartDetectConfig::default())
    }

    /// Detect grid line patterns suggesting a table or chart.
    pub fn analyze_region(img: &DynamicImage, x: u32, y: u32, w: u32, h: u32) -> (f64, f64) {
        let gray = img.to_luma8();
        let mut horiz_edges = 0u64;
        let mut vert_edges = 0u64;
        let mut total = 0u64;

        for py in y..y.saturating_add(h).min(gray.height().saturating_sub(1)) {
            for px in x..x.saturating_add(w).min(gray.width().saturating_sub(1)) {
                let v = gray.get_pixel(px, py).0[0] as i32;
                let vr = gray.get_pixel(px + 1, py).0[0] as i32;
                let vd = gray.get_pixel(px, py + 1).0[0] as i32;
                let h_grad = (vd - v).abs();
                let v_grad = (vr - v).abs();
                if h_grad > 30 { horiz_edges += 1; }
                if v_grad > 30 { vert_edges += 1; }
                total += 1;
            }
        }

        if total == 0 { return (0.0, 0.0); }
        (horiz_edges as f64 / total as f64, vert_edges as f64 / total as f64)
    }

    pub fn classify_region(h_density: f64, v_density: f64) -> (RegionKind, f64) {
        let both = h_density > 0.05 && v_density > 0.05;
        let grid = h_density > 0.1 && v_density > 0.1;
        if grid {
            (RegionKind::Table, (h_density + v_density).min(1.0))
        } else if both {
            (RegionKind::Chart, (h_density + v_density) * 0.5)
        } else {
            (RegionKind::Text, 0.5)
        }
    }

    pub fn detect_regions(&self, img: &DynamicImage) -> Vec<DetectedRegion> {
        let (w, h) = img.dimensions();
        let grid = 3usize;
        let cw = w / grid as u32;
        let ch = h / grid as u32;
        let mut regions = Vec::new();

        for gy in 0..grid {
            for gx in 0..grid {
                let x = gx as u32 * cw;
                let y = gy as u32 * ch;
                let region_fraction = (cw * ch) as f64 / (w * h) as f64;
                if region_fraction < self.config.min_region_fraction { continue; }

                let (hd, vd) = Self::analyze_region(img, x, y, cw, ch);
                let (kind, confidence) = Self::classify_region(hd, vd);

                if kind != RegionKind::Text || confidence > self.config.grid_density_threshold {
                    regions.push(DetectedRegion { kind, x, y, w: cw, h: ch, confidence });
                }
            }
        }
        regions
    }

    pub fn format_annotation(regions: &[DetectedRegion]) -> String {
        let charts = regions.iter().filter(|r| r.kind == RegionKind::Chart).count();
        let tables = regions.iter().filter(|r| r.kind == RegionKind::Table).count();
        format!("[DETECTED: {} charts, {} tables]", charts, tables)
    }
}

#[async_trait::async_trait]
impl MultiModalTokenSaver for PdfChartDetectSaver {
    fn modality(&self) -> Modality { Modality::Document }
}

#[async_trait::async_trait]
impl TokenSaver for PdfChartDetectSaver {
    fn name(&self) -> &str { "pdf-chart-detect" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 62 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut annotations = Vec::new();

        for img_bytes in &input.images {
            if let Ok(img) = image::load_from_memory(img_bytes.as_slice()) {
                let regions = self.detect_regions(&img);
                if !regions.is_empty() {
                    annotations.push(Self::format_annotation(&regions));
                }
            }
        }

        for annotation in annotations {
            let mut msg = Message::default();
            msg.role = "system".into();
            msg.content = annotation;
            msg.token_count = msg.content.len() / 4;
            input.messages.push(msg);
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
