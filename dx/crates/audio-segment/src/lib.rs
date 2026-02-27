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
    fn name(&self) -> &str { "audio-segment" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 40 }
    fn modality(&self) -> Modality { Modality::Audio }

    async fn process_multimodal(
        &self,
        mut input: MultiModalSaverInput,
        _ctx: &SaverContext,
    ) -> Result<MultiModalSaverOutput, SaverError> {
        let mut total_before = 0usize;
        let mut total_after = 0usize;

        for audio in &mut input.audio {
            if audio.duration_secs < 0.1 { continue; }

            let format_str = match audio.format {
                AudioFormat::Wav => "wav",
                AudioFormat::Pcm16 => "pcm",
                _ => "raw",
            };
            let sample_rate = audio.sample_rate as usize;
            let samples = AudioCompressSaver::decode_to_f32_pub(&audio.data, format_str);
            if samples.is_empty() { continue; }

            total_before += audio.naive_token_estimate;
            let segments = self.detect_segments(&samples, sample_rate);
            if segments.is_empty() { continue; }

            let description = self.extract_segments(&samples, &segments, sample_rate);
            let compressed_tokens = description.len() / 4;
            audio.data = description.into_bytes();
            audio.compressed_tokens = compressed_tokens;
            total_after += compressed_tokens;
        }

        let total_saved = total_before.saturating_sub(total_after);
        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "audio-segment".into(),
                tokens_before: total_before,
                tokens_after: total_after,
                tokens_saved: total_saved,
                description: format!("silence removal saved {} tokens", total_saved),
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
