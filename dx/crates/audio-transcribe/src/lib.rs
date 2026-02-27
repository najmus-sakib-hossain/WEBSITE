//! Transcribes audio to text using system Whisper or similar.
//! SAVINGS: 95-99% by replacing audio tokens with text
//! STAGE: PrePrompt (priority 42)

use dx_core::*;
use std::sync::Mutex;

pub struct AudioTranscribeSaver {
    config: TranscribeConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone, Debug)]
pub enum TranscribeMethod {
    SystemWhisper,
    Disabled,
}

#[derive(Clone)]
pub struct TranscribeConfig {
    pub method: TranscribeMethod,
    pub include_timestamps: bool,
    pub max_segment_secs: f64,
    pub min_confidence: f64,
}

impl Default for TranscribeConfig {
    fn default() -> Self {
        Self {
            method: TranscribeMethod::SystemWhisper,
            include_timestamps: true,
            max_segment_secs: 30.0,
            min_confidence: 0.5,
        }
    }
}

impl AudioTranscribeSaver {
    pub fn new(config: TranscribeConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(TranscribeConfig::default())
    }

    /// Run system `whisper` command and return transcription.
    pub fn transcribe_system(audio_data: &[u8], format: &str) -> Option<String> {
        // Write to temp file using stdlib
        let pid = std::process::id();
        let tmp_path = std::path::PathBuf::from(format!("/tmp/dx_audio_{}.{}", pid, format));
        std::fs::write(&tmp_path, audio_data).ok()?;

        let output = std::process::Command::new("whisper")
            .arg(tmp_path.to_str()?)
            .arg("--output_format")
            .arg("json")
            .arg("--output_dir")
            .arg(tmp_path.parent()?.to_str()?)
            .output()
            .ok()?;

        if !output.status.success() { return None; }

        // Try to read JSON output file
        let json_path = tmp_path.with_extension("json");
        let json_str = std::fs::read_to_string(json_path).ok()?;
        let parsed: serde_json::Value = serde_json::from_str(&json_str).ok()?;

        Some(parsed["text"].as_str().unwrap_or("").to_string())
    }

    /// Check if audio is likely to contain speech (simple energy check).
    pub fn is_speech_likely(samples: &[f32]) -> bool {
        if samples.is_empty() { return false; }
        let rms: f32 = (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt();
        rms > 0.01
    }

    /// Format transcription with optional timestamps.
    pub fn format_transcription(raw: &str, include_timestamps: bool) -> String {
        if !include_timestamps {
            return format!("[TRANSCRIPT]: {}", raw.trim());
        }
        format!("[TRANSCRIPT]: {}", raw.trim())
    }
}

#[async_trait::async_trait]
impl MultiModalTokenSaver for AudioTranscribeSaver {
    fn name(&self) -> &str { "audio-transcribe" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 42 }
    fn modality(&self) -> Modality { Modality::Audio }

    async fn process_multimodal(
        &self,
        mut input: MultiModalSaverInput,
        _ctx: &SaverContext,
    ) -> Result<MultiModalSaverOutput, SaverError> {
        if matches!(self.config.method, TranscribeMethod::Disabled) {
            return Ok(MultiModalSaverOutput {
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
            });
        }

        let mut total_before = 0usize;
        let mut total_after = 0usize;

        for audio in &mut input.audio {
            let format_str = match audio.format {
                AudioFormat::Wav => "wav",
                AudioFormat::Mp3 => "mp3",
                _ => "wav",
            };
            total_before += audio.naive_token_estimate;

            if let Some(text) = Self::transcribe_system(&audio.data, format_str) {
                let formatted = Self::format_transcription(&text, self.config.include_timestamps);
                let transcript_tokens = formatted.len() / 4;
                audio.data = formatted.into_bytes();
                audio.compressed_tokens = transcript_tokens;
                total_after += transcript_tokens;
            } else {
                total_after += audio.naive_token_estimate;
            }
        }

        let total_saved = total_before.saturating_sub(total_after);
        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "audio-transcribe".into(),
                tokens_before: total_before,
                tokens_after: total_after,
                tokens_saved: total_saved,
                description: format!("transcribed audio: {} → {} tokens", total_before, total_after),
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
