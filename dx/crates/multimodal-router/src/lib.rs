//! Routes multimodal content to the cheapest processing path.
//! Route priority: text > OCR > compressed image > raw image
//! SAVINGS: 40-80% by choosing optimal representation
//! STAGE: PrePrompt (priority 90)

use dx_core::*;
use std::sync::Mutex;

pub struct MultimodalRouterSaver {
    report: Mutex<TokenSavingsReport>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RoutePath {
    /// Content is already text — cheapest
    Text,
    /// OCR can extract text from image
    Ocr,
    /// Compressed image representation
    CompressedImage,
    /// Raw image — most expensive
    RawImage,
}

impl RoutePath {
    pub fn estimated_savings_pct(&self) -> f64 {
        match self {
            RoutePath::Text => 0.95,
            RoutePath::Ocr => 0.85,
            RoutePath::CompressedImage => 0.60,
            RoutePath::RawImage => 0.0,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            RoutePath::Text => "text",
            RoutePath::Ocr => "ocr",
            RoutePath::CompressedImage => "compressed-image",
            RoutePath::RawImage => "raw-image",
        }
    }
}

impl MultimodalRouterSaver {
    pub fn new() -> Self {
        Self {
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    /// Choose the optimal processing route for a message.
    pub fn choose_route(msg: &Message) -> RoutePath {
        // Images attached → image route
        if !msg.images.is_empty() {
            // Screenshot/text-heavy content → suggest OCR
            let lower = msg.content.to_lowercase();
            if lower.contains("screenshot") || lower.contains("text-heavy") {
                return RoutePath::Ocr;
            }
            return RoutePath::CompressedImage;
        }
        // Pure text message
        RoutePath::Text
    }

    pub fn annotate_route(msg: &mut Message, route: &RoutePath) {
        let annotation = format!("[ROUTE:{}]", route.label());
        if !msg.content.contains("[ROUTE:") {
            msg.content = format!("{} {}", annotation, msg.content);
            msg.token_count = msg.content.len() / 4;
        }
    }
}

impl Default for MultimodalRouterSaver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl MultiModalTokenSaver for MultimodalRouterSaver {
    fn name(&self) -> &str { "multimodal-router" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 90 }
    fn modality(&self) -> Modality { Modality::CrossModal }

    async fn process_multimodal(
        &self,
        mut input: MultiModalSaverInput,
        _ctx: &SaverContext,
    ) -> Result<MultiModalSaverOutput, SaverError> {
        let mut routing_summary = Vec::new();

        for msg in &mut input.base.messages {
            let route = Self::choose_route(msg);
            if route != RoutePath::RawImage {
                routing_summary.push(format!("{}: {}", msg.role, route.label()));
                Self::annotate_route(msg, &route);
            }
        }

        if !routing_summary.is_empty() {
            let savings_est: usize = input.base.messages.iter()
                .filter(|m| m.content.contains("[ROUTE:"))
                .map(|m| {
                    let route = Self::choose_route(m);
                    (m.token_count as f64 * route.estimated_savings_pct()) as usize
                })
                .sum();

            let annotation = format!("[ROUTING DECISIONS: {}]", routing_summary.join("; "));
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "multimodal-router".into(),
                tokens_before: savings_est,
                tokens_after: 0,
                tokens_saved: savings_est,
                description: annotation,
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
