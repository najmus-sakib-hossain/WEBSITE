//! Converts text-heavy images to plain text via OCR, eliminating image tokens.
//! SAVINGS: 90% on text-heavy screenshots
//! STAGE: PrePrompt (priority 5)

use dx_core::*;
use image::GrayImage;
use std::sync::Mutex;

pub struct OcrExtractSaver {
    config: OcrConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct OcrConfig {
    pub enabled: bool,
    pub text_detection_threshold: f64,
    pub min_confidence: f64,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            text_detection_threshold: 0.08,
            min_confidence: 0.6,
        }
    }
}

impl OcrExtractSaver {
    pub fn new(config: OcrConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(OcrConfig::default())
    }

    /// Compute gradient-based edge density. Text images have 0.05-0.20.
    pub fn edge_density(gray: &GrayImage) -> f64 {
        let (w, h) = gray.dimensions();
        if w < 3 || h < 3 { return 0.0; }

        let mut edges = 0u64;
        let total = ((w - 2) * (h - 2)) as u64;

        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let c = gray.get_pixel(x, y)[0] as i32;
                let r = gray.get_pixel(x + 1, y)[0] as i32;
                let b = gray.get_pixel(x, y + 1)[0] as i32;
                let gradient = (c - r).abs() + (c - b).abs();
                if gradient > 30 {
                    edges += 1;
                }
            }
        }

        if total == 0 { 0.0 } else { edges as f64 / total as f64 }
    }

    pub fn is_text_heavy(&self, img_data: &[u8]) -> bool {
        let img = match image::load_from_memory(img_data) {
            Ok(i) => i,
            Err(_) => return false,
        };
        let gray = img.to_luma8();
        Self::edge_density(&gray) > self.config.text_detection_threshold
    }

    pub fn try_system_tesseract(img_data: &[u8]) -> Option<String> {
        use std::process::Command;

        let temp_path = std::env::temp_dir().join("dx_ocr_temp.png");
        std::fs::write(&temp_path, img_data).ok()?;

        let output = Command::new("tesseract")
            .args([temp_path.to_str()?, "stdout", "--psm", "6"])
            .output()
            .ok()?;

        std::fs::remove_file(&temp_path).ok();

        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if text.len() > 20 { Some(text) } else { None }
        } else {
            None
        }
    }

    pub fn try_ocr(&self, img_data: &[u8]) -> Option<String> {
        Self::try_system_tesseract(img_data)
    }
}

#[async_trait::async_trait]
impl TokenSaver for OcrExtractSaver {
    fn name(&self) -> &str { "ocr-extract" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 5 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        if !self.config.enabled || input.images.is_empty() {
            return Ok(SaverOutput {
                messages: input.messages,
                tools: input.tools,
                images: input.images,
                skipped: false,
                cached_response: None,
            });
        }

        let mut remaining_images = Vec::new();
        let mut total_saved = 0usize;
        let original_count = input.images.len();

        for img in &input.images {
            if self.is_text_heavy(&img.data) {
                if let Some(text) = self.try_ocr(&img.data) {
                    let text_tokens = text.len() / 4;
                    let image_tokens = img.original_tokens.max(85);

                    input.messages.push(Message {
                        role: "user".into(),
                        content: format!("[Extracted text from screenshot]\n{}", text),
                        images: vec![],
                        tool_call_id: None,
                        token_count: text_tokens + 5,
                    });

                    total_saved += image_tokens.saturating_sub(text_tokens);
                    continue;
                }
            }
            remaining_images.push(img.clone());
        }

        if total_saved > 0 {
            let extracted = original_count - remaining_images.len();
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "ocr-extract".into(),
                tokens_before: total_saved + remaining_images.len() * 85,
                tokens_after: remaining_images.len() * 85,
                tokens_saved: total_saved,
                description: format!("OCR replaced {} images with text", extracted),
            };
        }

        input.images = remaining_images;

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
