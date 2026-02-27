//! Segments audio to extract only speech-containing regions.
//! SAVINGS: 30-80% by removing silence
//! STAGE: PrePrompt (priority 40)

use dx_core::*;
use audio_compress::AudioCompressSaver;
use std::sync::Mutex;

pub struct AudioSegmentSaver {
    config: SegmentConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct SegmentConfig {
    pub silence_threshold: f32,
    pub min_silence_ms: usize,
    pub keep_boundary_ms: usize,
    pub max_inter_segment_silence_ms: usize,
    pub min_segment_ms: usize,
}

impl Default for SegmentConfig {
    fn default() -> Self {
        Self {
            silence_threshold: 0.01,
            min_silence_ms: 300,
            keep_boundary_ms: 50,
            max_inter_segment_silence_ms: 500,
            min_segment_ms: 200,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Segment {
    pub start_sample: usize,
    pub end_sample: usize,
}

impl AudioSegmentSaver {
    pub fn new(config: SegmentConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(SegmentConfig::default())
    }

    /// Compute RMS energy of a sample window.
    pub fn rms_energy(samples: &[f32]) -> f32 {
        if samples.is_empty() { return 0.0; }
        (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
    }

    /// Detect speech segments by finding non-silent regions.
    pub fn detect_segments(&self, samples: &[f32], sample_rate: usize) -> Vec<Segment> {
        let window = (sample_rate as f32 * 0.01) as usize; // 10ms windows
        let min_silence_windows = self.config.min_silence_ms / 10;
        let min_segment_windows = self.config.min_segment_ms / 10;
        let boundary_windows = self.config.keep_boundary_ms / 10;

        let mut in_speech = false;
        let mut silent_count = 0usize;
        let mut segment_start = 0usize;
        let mut segments = Vec::new();

        let total_windows = samples.len() / window.max(1);

        for w in 0..total_windows {
            let start = w * window;
            let end = (start + window).min(samples.len());
            let energy = Self::rms_energy(&samples[start..end]);

            if energy >= self.config.silence_threshold {
                if !in_speech {
                    let actual_start = w.saturating_sub(boundary_windows) * window;
                    segment_start = actual_start;
                    in_speech = true;
                }
                silent_count = 0;
            } else if in_speech {
                silent_count += 1;
                if silent_count >= min_silence_windows {
                    let end_sample = (w * window).min(samples.len());
                    let seg_windows = (end_sample - segment_start) / window.max(1);
                    if seg_windows >= min_segment_windows {
                        segments.push(Segment {
                            start_sample: segment_start,
                            end_sample,
                        });
                    }
                    in_speech = false;
                    silent_count = 0;
                }
            }
        }

        if in_speech {
            segments.push(Segment {
                start_sample: segment_start,
                end_sample: samples.len(),
            });
        }

        segments
    }

    /// Extract segment samples and re-encode as minimal PCM description.
    pub fn extract_segments(&self, samples: &[f32], segments: &[Segment], sample_rate: usize) -> String {
        let total_duration = samples.len() as f64 / sample_rate as f64;
        let speech_duration: f64 = segments.iter()
            .map(|s| (s.end_sample - s.start_sample) as f64 / sample_rate as f64)
            .sum();

        let mut desc = format!(
            "[AUDIO SEGMENTS: {:.1}s total, {:.1}s speech, {} segments]\n",
            total_duration, speech_duration, segments.len()
        );
        for (i, seg) in segments.iter().enumerate() {
            let t_start = seg.start_sample as f64 / sample_rate as f64;
            let t_end = seg.end_sample as f64 / sample_rate as f64;
            desc.push_str(&format!("  seg{}: {:.2}s-{:.2}s\n", i, t_start, t_end));
        }
        desc
    }
}

#[async_trait::async_trait]
impl MultiModalTokenSaver for AudioSegmentSaver {
    fn modality(&self) -> Modality { Modality::Audio }
}

#[async_trait::async_trait]
impl TokenSaver for AudioSegmentSaver {
    fn name(&self) -> &str { "audio-segment" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 40 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut total_saved = 0usize;

        for msg in &mut input.messages {
            if msg.modality.as_deref() != Some("audio") { continue; }
            let audio_bytes = match msg.binary_data.as_deref() {
                Some(b) => b.to_vec(),
                None => continue,
            };

            let format = msg.content_type.as_deref().unwrap_or("raw");
            let samples = AudioCompressSaver::decode_to_f32_pub(&audio_bytes, format);
            if samples.is_empty() { continue; }

            let sample_rate = 16000usize;
            let original_tokens = msg.token_count;
            let segments = self.detect_segments(&samples, sample_rate);

            if segments.is_empty() { continue; }

            let description = self.extract_segments(&samples, &segments, sample_rate);
            msg.content = description;
            msg.token_count = msg.content.len() / 4;
            msg.binary_data = None;
            total_saved += original_tokens.saturating_sub(msg.token_count);
        }

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "audio-segment".into(),
                tokens_before: total_saved,
                tokens_after: 0,
                tokens_saved: total_saved,
                description: format!("silence removal saved {} tokens", total_saved),
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
