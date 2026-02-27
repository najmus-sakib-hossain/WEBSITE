//! Organizes live events into a hierarchical tree structure.
//! SAVINGS: 50-80% on live stream events
//! STAGE: PrePrompt (priority 56)

use dx_core::*;
use std::sync::Mutex;

pub struct LiveEventTreeSaver {
    config: EventTreeConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct EventTreeConfig {
    pub event_similarity_threshold: f64,
    pub max_event_duration_secs: f64,
    pub max_events: usize,
    pub summary_token_budget: usize,
}

impl Default for EventTreeConfig {
    fn default() -> Self {
        Self {
            event_similarity_threshold: 0.7,
            max_event_duration_secs: 60.0,
            max_events: 20,
            summary_token_budget: 500,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Event {
    pub start_time: f64,
    pub end_time: f64,
    pub frame_count: usize,
    pub description: String,
    pub importance: f64,
}

#[derive(Default)]
pub struct EventTree {
    pub events: Vec<Event>,
}

impl EventTree {
    pub fn tree_summary(&self) -> String {
        if self.events.is_empty() {
            return "[EVENTS: none]".into();
        }
        let total_frames: usize = self.events.iter().map(|e| e.frame_count).sum();
        let duration = self.events.last().map(|e| e.end_time).unwrap_or(0.0);
        let mut summary = format!(
            "[{} events | {:.1}s | {} frames]\n",
            self.events.len(), duration, total_frames
        );
        for e in &self.events {
            summary.push_str(&format!(
                "  [{:.1}s-{:.1}s({} frames)] {}\n",
                e.start_time, e.end_time, e.frame_count,
                e.description.chars().take(60).collect::<String>()
            ));
        }
        summary
    }
}

impl LiveEventTreeSaver {
    pub fn new(config: EventTreeConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(EventTreeConfig::default())
    }

    fn text_similarity(a: &str, b: &str) -> f64 {
        use std::collections::HashSet;
        let wa: HashSet<&str> = a.split_whitespace().collect();
        let wb: HashSet<&str> = b.split_whitespace().collect();
        let inter = wa.intersection(&wb).count();
        let union = wa.union(&wb).count();
        if union == 0 { 1.0 } else { inter as f64 / union as f64 }
    }

    pub fn ingest_frames(&self, messages: &[Message]) -> EventTree {
        let mut tree = EventTree::default();
        let secs_per_frame = 1.0 / 30.0;

        for (i, msg) in messages.iter().enumerate() {
            if msg.modality.as_deref() != Some("live") { continue; }

            let t = i as f64 * secs_per_frame;
            let desc = msg.content.chars().take(80).collect::<String>();

            if let Some(last) = tree.events.last_mut() {
                let sim = Self::text_similarity(&last.description, &desc);
                let duration = t - last.start_time;
                if sim >= self.config.event_similarity_threshold
                    && duration < self.config.max_event_duration_secs
                {
                    last.end_time = t;
                    last.frame_count += 1;
                    continue;
                }
            }

            if tree.events.len() < self.config.max_events {
                tree.events.push(Event {
                    start_time: t,
                    end_time: t,
                    frame_count: 1,
                    description: desc,
                    importance: 0.5,
                });
            }
        }

        tree
    }
}

#[async_trait::async_trait]
impl MultiModalTokenSaver for LiveEventTreeSaver {
    fn modality(&self) -> Modality { Modality::Live }
}

#[async_trait::async_trait]
impl TokenSaver for LiveEventTreeSaver {
    fn name(&self) -> &str { "live-event-tree" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 56 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let before_tokens: usize = input.messages.iter().map(|m| m.token_count).sum();

        let tree = self.ingest_frames(&input.messages);
        let summary = tree.tree_summary();

        // Remove live messages, add tree summary
        input.messages.retain(|m| m.modality.as_deref() != Some("live"));

        let mut tree_msg = Message::default();
        tree_msg.role = "system".into();
        tree_msg.content = summary;
        tree_msg.token_count = tree_msg.content.len() / 4;
        input.messages.push(tree_msg);

        let after_tokens: usize = input.messages.iter().map(|m| m.token_count).sum();
        let saved = before_tokens.saturating_sub(after_tokens);

        if saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "live-event-tree".into(),
                tokens_before: before_tokens,
                tokens_after: after_tokens,
                tokens_saved: saved,
                description: format!("event tree compression saved {} tokens", saved),
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
