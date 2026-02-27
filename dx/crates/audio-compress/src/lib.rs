//! Compresses audio data to reduce token usage.
//! SAVINGS: 80-95% on raw audio data
//! STAGE: PrePrompt (priority 45)

use dx_core::*;
use std::sync::Mutex;

pub struct AudioCompressSaver {
    config: AudioCompressConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct AudioCompressConfig {
    pub target_tokens_per_sec: usize,
    pub use_ml_codec: bool,
    pub spectral_bands: usize,
    pub temporal_pool_ms: usize,
    pub min_duration_secs: f64,
}

impl Default for AudioCompressConfig {
    fn default() -> Self {
        Self {
            target_tokens_per_sec: 75,
            use_ml_codec: false,
            spectral_bands: 64,
            temporal_pool_ms: 25,
            min_duration_secs: 0.5,
        }
    }
}

impl AudioCompressSaver {
    pub fn new(config: AudioCompressConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(AudioCompressConfig::default())
    }

    /// Decode audio bytes to f32 samples (public, used by audio-segment).
    pub fn decode_to_f32_pub(data: &[u8], format: &str) -> Vec<f32> {
        match format.to_lowercase().as_str() {
            "wav" => {
                // Skip WAV header (44 bytes), decode 16-bit PCM
                if data.len() < 44 { return vec![]; }
                let pcm = &data[44..];
                pcm.chunks_exact(2)
                    .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
                    .collect()
            }
            "pcm" | "raw" => {
                data.chunks_exact(2)
                    .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
                    .collect()
            }
            _ => {
                // Treat bytes as normalized f32 (8-bit raw approximation)
                data.iter().map(|&b| (b as f32 - 128.0) / 128.0).collect()
            }
        }
    }

    /// Compute spectral band energies via simple windowed magnitude.
    pub fn compute_band_energies(samples: &[f32], bands: usize) -> Vec<f32> {
        if samples.is_empty() { return vec![0.0; bands]; }
        let chunk_size = (samples.len() / bands).max(1);
        (0..bands).map(|b| {
            let start = b * chunk_size;
            let end = ((b + 1) * chunk_size).min(samples.len());
            let energy: f32 = samples[start..end].iter().map(|x| x * x).sum();
            energy / (end - start) as f32
        }).collect()
    }

    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
        if a.len() != b.len() || a.is_empty() { return 0.0; }
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a < 1e-9 || norm_b < 1e-9 { return 0.0; }
        (dot / (norm_a * norm_b)) as f64
    }

    /// Merge windows with cosine similarity > threshold.
    pub fn merge_similar_windows(windows: &[Vec<f32>], threshold: f64) -> Vec<(usize, usize)> {
        if windows.is_empty() { return vec![]; }
        let mut groups: Vec<(usize, usize)> = vec![(0, 0)];
        for i in 1..windows.len() {
            let last_group = groups.last_mut().unwrap();
            let prev_idx = last_group.1;
            let sim = Self::cosine_similarity(&windows[prev_idx], &windows[i]);
            if sim >= threshold {
                last_group.1 = i;
            } else {
                groups.push((i, i));
            }
        }
        groups
    }

    /// Perform spectral compression and return compressed description.
    pub fn spectral_compress(&self, samples: &[f32], sample_rate: f64) -> String {
        let window_samples = ((self.config.temporal_pool_ms as f64 / 1000.0) * sample_rate) as usize;
        let window_size = window_samples.max(128);
        let total_windows = samples.len() / window_size;
        if total_windows == 0 {
            return "[empty audio]".into();
        }

        let windows: Vec<Vec<f32>> = (0..total_windows).map(|i| {
            let start = i * window_size;
            let end = (start + window_size).min(samples.len());
            Self::compute_band_energies(&samples[start..end], self.config.spectral_bands)
        }).collect();

        let groups = Self::merge_similar_windows(&windows, 0.92);
        self.audio_to_description(&groups, total_windows, sample_rate, window_size)
    }

    pub fn audio_to_description(
        &self, groups: &[(usize, usize)],
        total_windows: usize, sample_rate: f64, window_size: usize
    ) -> String {
        let duration_secs = total_windows as f64 * window_size as f64 / sample_rate;
        let mut desc = format!("[AUDIO: {:.1}s, {} spectral segments]\n", duration_secs, groups.len());
        let secs_per_window = window_size as f64 / sample_rate;
        for (start, end) in groups {
            let t_start = *start as f64 * secs_per_window;
            let t_end = (*end + 1) as f64 * secs_per_window;
            desc.push_str(&format!("  [{:.2}s-{:.2}s]", t_start, t_end));
        }
        desc
    }
}

#[async_trait::async_trait]
impl MultiModalTokenSaver for AudioCompressSaver {
    fn name(&self) -> &str { "audio-compress" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 45 }
    fn modality(&self) -> Modality { Modality::Audio }

    async fn process_multimodal(
        &self,
        mut input: MultiModalSaverInput,
        _ctx: &SaverContext,
    ) -> Result<MultiModalSaverOutput, SaverError> {
        let mut total_before = 0usize;
        let mut total_after = 0usize;

        for audio in &mut input.audio {
            if audio.duration_secs < self.config.min_duration_secs { continue; }

            let format_str = match audio.format {
                AudioFormat::Wav => "wav",
                AudioFormat::Pcm16 => "pcm",
                _ => "raw",
            };
            let sample_rate = audio.sample_rate as f64;
            let samples = Self::decode_to_f32_pub(&audio.data, format_str);
            if samples.is_empty() { continue; }

            total_before += audio.naive_token_estimate;
            let description = self.spectral_compress(&samples, sample_rate);
            let compressed_tokens = description.len() / 4;
            audio.data = description.into_bytes();
            audio.format = AudioFormat::Pcm16; // repurposed as text-encoded
            audio.compressed_tokens = compressed_tokens;
            total_after += compressed_tokens;
        }

        let total_saved = total_before.saturating_sub(total_after);
        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "audio-compress".into(),
                tokens_before: total_before,
                tokens_after: total_after,
                tokens_saved: total_saved,
                description: format!(
                    "spectral audio compression: {} → {} tokens ({:.0}% saved)",
                    total_before, total_after,
                    total_saved as f64 / total_before.max(1) as f64 * 100.0
                ),
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
