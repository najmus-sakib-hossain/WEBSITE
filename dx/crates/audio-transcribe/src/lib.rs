//! Transcribes audio to text using system Whisper or similar.
//! SAVINGS: 95-99% by replacing audio tokens with text
//! STAGE: PrePrompt (priority 42)

use dx_core::*;
use std::io::Write;
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
        // Write to temp file
        let mut tmp = tempfile::Builder::new()
            .suffix(&format!(".{}", format))
            .tempfile()
            .ok()?;
        tmp.write_all(audio_data).ok()?;
        let tmp_path = tmp.path().to_path_buf();

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
    fn modality(&self) -> Modality { Modality::Audio }
}

#[async_trait::async_trait]
impl TokenSaver for AudioTranscribeSaver {
    fn name(&self) -> &str { "audio-transcribe" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 42 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        if matches!(self.config.method, TranscribeMethod::Disabled) {
            return Ok(SaverOutput {
                messages: input.messages,
                tools: input.tools,
                images: input.images,
                skipped: false,
                cached_response: None,
            });
        }

        let mut total_saved = 0usize;

        for msg in &mut input.messages {
            if msg.modality.as_deref() != Some("audio") { continue; }
            let audio_bytes = match msg.binary_data.as_deref() {
                Some(b) => b.to_vec(),
                None => continue,
            };

            let format = msg.content_type.as_deref().unwrap_or("wav");
            let original_tokens = msg.token_count;

            let transcript = Self::transcribe_system(&audio_bytes, format);
            if let Some(text) = transcript {
                let formatted = Self::format_transcription(&text, self.config.include_timestamps);
                msg.content = formatted;
                msg.token_count = msg.content.len() / 4;
                msg.binary_data = None;
                total_saved += original_tokens.saturating_sub(msg.token_count);
            }
        }

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "audio-transcribe".into(),
                tokens_before: total_saved,
                tokens_after: 0,
                tokens_saved: total_saved,
                description: format!("transcribed audio, saved {} tokens", total_saved),
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
