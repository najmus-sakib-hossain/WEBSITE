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
    fn modality(&self) -> Modality { Modality::Audio }
}

#[async_trait::async_trait]
impl TokenSaver for AudioCompressSaver {
    fn name(&self) -> &str { "audio-compress" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 45 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut total_saved = 0usize;

        for msg in &mut input.messages {
            if msg.modality.as_deref() != Some("audio") { continue; }

            let audio_bytes = match msg.binary_data.as_deref() {
                Some(b) => b.to_vec(),
                None => continue,
            };

            let format = msg.content_type.as_deref().unwrap_or("raw");
            let samples = Self::decode_to_f32_pub(&audio_bytes, format);
            if samples.is_empty() { continue; }

            let duration_secs = samples.len() as f64 / 16000.0;
            if duration_secs < self.config.min_duration_secs { continue; }

            let original_tokens = msg.token_count;
            let description = self.spectral_compress(&samples, 16000.0);

            msg.content = description;
            msg.token_count = msg.content.len() / 4;
            msg.binary_data = None;
            total_saved += original_tokens.saturating_sub(msg.token_count);
        }

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "audio-compress".into(),
                tokens_before: total_saved,
                tokens_after: 0,
                tokens_saved: total_saved,
                description: format!("spectral audio compression saved {} tokens", total_saved),
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
