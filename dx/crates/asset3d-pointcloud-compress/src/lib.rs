//! Compresses 3D point clouds to text via voxel grid downsampling.
//! SAVINGS: 99%+ by replacing raw points with text summary
//! STAGE: PrePrompt (priority 82)

use dx_core::*;
use std::sync::Mutex;

pub struct Asset3dPointcloudCompressSaver {
    config: PointcloudConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct PointcloudConfig {
    pub voxel_grid_size: f32,
    pub max_output_tokens: usize,
    pub include_clusters: bool,
}

impl Default for PointcloudConfig {
    fn default() -> Self {
        Self {
            voxel_grid_size: 0.05,
            max_output_tokens: 200,
            include_clusters: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PointCloudStats {
    pub point_count: usize,
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
    pub min_z: f32,
    pub max_z: f32,
    pub density_estimate: f32,
}

impl Asset3dPointcloudCompressSaver {
    pub fn new(config: PointcloudConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(PointcloudConfig::default())
    }

    /// Parse simple XYZ point cloud text format.
    pub fn parse_xyz(text: &str) -> Vec<[f32; 3]> {
        let mut points = Vec::new();
        for line in text.lines() {
            let nums: Vec<f32> = line.split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if nums.len() >= 3 {
                points.push([nums[0], nums[1], nums[2]]);
            }
        }
        points
    }

    pub fn compute_stats(points: &[[f32; 3]]) -> PointCloudStats {
        if points.is_empty() { return PointCloudStats::default(); }
        let mut s = PointCloudStats {
            point_count: points.len(),
            min_x: points[0][0], max_x: points[0][0],
            min_y: points[0][1], max_y: points[0][1],
            min_z: points[0][2], max_z: points[0][2],
            density_estimate: 0.0,
        };
        for p in points {
            s.min_x = s.min_x.min(p[0]); s.max_x = s.max_x.max(p[0]);
            s.min_y = s.min_y.min(p[1]); s.max_y = s.max_y.max(p[1]);
            s.min_z = s.min_z.min(p[2]); s.max_z = s.max_z.max(p[2]);
        }
        let vol = (s.max_x - s.min_x) * (s.max_y - s.min_y) * (s.max_z - s.min_z);
        s.density_estimate = if vol > 0.0 { points.len() as f32 / vol } else { 0.0 };
        s
    }

    /// Voxel grid downsampling — count unique voxels.
    pub fn voxel_downsample(points: &[[f32; 3]], voxel_size: f32) -> usize {
        use std::collections::HashSet;
        let voxels: HashSet<(i64, i64, i64)> = points.iter().map(|p| (
            (p[0] / voxel_size) as i64,
            (p[1] / voxel_size) as i64,
            (p[2] / voxel_size) as i64,
        )).collect();
        voxels.len()
    }

    pub fn format_summary(stats: &PointCloudStats, voxel_count: usize, voxel_size: f32) -> String {
        format!(
            "[POINT CLOUD: {} points, {} voxels@{:.3}m | bounds: x[{:.2},{:.2}] y[{:.2},{:.2}] z[{:.2},{:.2}] | density: {:.1}pts/m³]",
            stats.point_count, voxel_count, voxel_size,
            stats.min_x, stats.max_x,
            stats.min_y, stats.max_y,
            stats.min_z, stats.max_z,
            stats.density_estimate
        )
    }
}

#[async_trait::async_trait]
impl MultiModalTokenSaver for Asset3dPointcloudCompressSaver {
    fn modality(&self) -> Modality { Modality::Asset3d }
}

#[async_trait::async_trait]
impl TokenSaver for Asset3dPointcloudCompressSaver {
    fn name(&self) -> &str { "asset3d-pointcloud-compress" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 82 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut total_saved = 0usize;

        for msg in &mut input.messages {
            if msg.modality.as_deref() != Some("asset3d") { continue; }
            let ct = msg.content_type.as_deref().unwrap_or("");
            if !ct.contains("xyz") && !ct.contains("pcd") && !ct.contains("pointcloud") { continue; }

            let original_tokens = msg.token_count;
            let points = Self::parse_xyz(&msg.content);
            if points.is_empty() { continue; }

            let stats = Self::compute_stats(&points);
            let voxel_count = Self::voxel_downsample(&points, self.config.voxel_grid_size);
            let summary = Self::format_summary(&stats, voxel_count, self.config.voxel_grid_size);

            let summary_tokens = summary.len() / 4;
            if summary_tokens < original_tokens {
                total_saved += original_tokens - summary_tokens;
                msg.content = summary;
                msg.token_count = summary_tokens;
            }
        }

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "asset3d-pointcloud-compress".into(),
                tokens_before: total_saved,
                tokens_after: 0,
                tokens_saved: total_saved,
                description: format!("point cloud compression saved {} tokens", total_saved),
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
