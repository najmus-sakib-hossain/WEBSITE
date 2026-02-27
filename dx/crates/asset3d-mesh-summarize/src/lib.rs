//! Summarizes 3D mesh data as compact text description.
//! SAVINGS: 99%+ vs sending raw mesh data
//! STAGE: PrePrompt (priority 84)

use dx_core::*;
use std::sync::Mutex;

pub struct Asset3dMeshSummarizeSaver {
    config: MeshSummarizeConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct MeshSummarizeConfig {
    pub max_output_tokens: usize,
    pub include_topology: bool,
}

impl Default for MeshSummarizeConfig {
    fn default() -> Self {
        Self {
            max_output_tokens: 150,
            include_topology: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MeshStats {
    pub vertex_count: usize,
    pub face_count: usize,
    pub edge_count: usize,
    pub is_manifold: bool,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub material_count: usize,
}

impl Asset3dMeshSummarizeSaver {
    pub fn new(config: MeshSummarizeConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(MeshSummarizeConfig::default())
    }

    /// Parse simple OBJ-format mesh statistics.
    pub fn parse_obj_stats(text: &str) -> MeshStats {
        let mut stats = MeshStats::default();
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];

        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() { continue; }
            match parts[0] {
                "v" if parts.len() >= 4 => {
                    stats.vertex_count += 1;
                    for (i, coord) in parts[1..4].iter().enumerate() {
                        if let Ok(v) = coord.parse::<f32>() {
                            min[i] = min[i].min(v);
                            max[i] = max[i].max(v);
                        }
                    }
                }
                "f" => { stats.face_count += 1; }
                "usemtl" => { stats.material_count += 1; }
                _ => {}
            }
        }

        if stats.vertex_count > 0 {
            stats.bounds_min = min;
            stats.bounds_max = max;
        }

        // Euler characteristic heuristic: manifold if V - E + F = 2
        stats.edge_count = (stats.face_count * 3) / 2;
        let euler = stats.vertex_count as i64 - stats.edge_count as i64 + stats.face_count as i64;
        stats.is_manifold = euler == 2;
        stats
    }

    pub fn format_summary(stats: &MeshStats) -> String {
        format!(
            "[3D MESH: {}V {}F {}E | bounds: [{:.2},{:.2},{:.2}]-[{:.2},{:.2},{:.2}] | {} | {} materials]",
            stats.vertex_count, stats.face_count, stats.edge_count,
            stats.bounds_min[0], stats.bounds_min[1], stats.bounds_min[2],
            stats.bounds_max[0], stats.bounds_max[1], stats.bounds_max[2],
            if stats.is_manifold { "manifold" } else { "non-manifold" },
            stats.material_count
        )
    }
}

#[async_trait::async_trait]
impl MultiModalTokenSaver for Asset3dMeshSummarizeSaver {
    fn name(&self) -> &str { "asset3d-mesh-summarize" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 84 }
    fn modality(&self) -> Modality { Modality::Asset3d }

    async fn process_multimodal(
        &self,
        mut input: MultiModalSaverInput,
        _ctx: &SaverContext,
    ) -> Result<MultiModalSaverOutput, SaverError> {
        let mut total_before = 0usize;
        let mut total_after = 0usize;

        for asset in &mut input.assets_3d {
            let content = String::from_utf8_lossy(&asset.data).into_owned();
            let stats = Self::parse_obj_stats(&content);
            if stats.vertex_count == 0 && stats.face_count == 0 { continue; }

            total_before += asset.naive_token_estimate;
            let summary = Self::format_summary(&stats);
            let summary_tokens = summary.len() / 4;
            total_after += summary_tokens;

            if summary_tokens < asset.naive_token_estimate {
                asset.data = summary.into_bytes();
            }
        }

        let total_saved = total_before.saturating_sub(total_after);
        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "asset3d-mesh-summarize".into(),
                tokens_before: total_before,
                tokens_after: total_after,
                tokens_saved: total_saved,
                description: format!("mesh summary: {} -> {} tokens", total_before, total_after),
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
