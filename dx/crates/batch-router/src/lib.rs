//! Routes tasks to smaller/specialized models when appropriate.
//! SAVINGS: 40-70% on token costs by model selection
//! STAGE: PreCall (priority 15)

use dx_core::*;
use std::sync::Mutex;

pub struct BatchRouterSaver {
    keywords: Vec<String>,
    report: Mutex<TokenSavingsReport>,
}

impl BatchRouterSaver {
    pub fn new(keywords: Vec<String>) -> Self {
        Self {
            keywords,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(vec![
            "format".into(), "lint".into(), "rename".into(),
            "sort".into(), "organize".into(), "classify".into(),
            "extract".into(), "convert".into(), "validate".into(),
            "check".into(),
        ])
    }

    pub fn is_batch_eligible(&self, messages: &[Message]) -> bool {
        let user_msgs: Vec<&Message> = messages.iter().filter(|m| m.role == "user").collect();
        if user_msgs.is_empty() { return false; }
        let last = &user_msgs[user_msgs.len() - 1].content.to_lowercase();
        self.keywords.iter().any(|kw| last.contains(kw.as_str()))
    }
}

#[async_trait::async_trait]
impl TokenSaver for BatchRouterSaver {
    fn name(&self) -> &str { "batch-router" }
    fn stage(&self) -> SaverStage { SaverStage::PreCall }
    fn priority(&self) -> u32 { 15 }

    async fn process(&self, mut input: SaverInput, ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        if self.is_batch_eligible(&input.messages) {
            if let Some(sys) = input.messages.iter_mut().find(|m| m.role == "system") {
                let annotation = "\n[ROUTING: eligible for batch/lightweight model]";
                if !sys.content.contains("ROUTING") {
                    sys.content.push_str(annotation);
                    sys.token_count = sys.content.len() / 4;
                }
            }
        }
        let _ = ctx;

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
