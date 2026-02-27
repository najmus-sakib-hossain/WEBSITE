//! Prunes tool definitions to only those relevant for current task.
//! SAVINGS: 20-60% on tool definition tokens
//! STAGE: PromptAssembly (priority 15)

use dx_core::*;
use std::collections::HashMap;
use std::sync::Mutex;

pub struct ToolRouterSaver {
    core_tools: Vec<String>,
    keyword_map: HashMap<String, Vec<String>>,
    report: Mutex<TokenSavingsReport>,
}

impl ToolRouterSaver {
    pub fn new() -> Self {
        let mut keyword_map: HashMap<String, Vec<String>> = HashMap::new();
        keyword_map.insert("web".into(), vec!["browser".into(), "fetch".into(), "http".into(), "url".into()]);
        keyword_map.insert("database".into(), vec!["sql".into(), "query".into(), "insert".into(), "select".into()]);
        keyword_map.insert("git".into(), vec!["commit".into(), "push".into(), "branch".into(), "merge".into()]);
        keyword_map.insert("docker".into(), vec!["container".into(), "image".into(), "compose".into()]);
        keyword_map.insert("test".into(), vec!["test".into(), "spec".into(), "jest".into(), "cargo test".into()]);

        Self {
            core_tools: vec![
                "read".into(), "write".into(), "patch".into(), "exec".into(),
                "list_files".into(), "search".into(),
            ],
            keyword_map,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn select_tools<'a>(&self, available: &'a [ToolDefinition], messages: &[Message]) -> Vec<&'a ToolDefinition> {
        let context: String = messages.iter()
            .filter(|m| m.role == "user" || m.role == "system")
            .map(|m| m.content.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");

        let mut selected_names: std::collections::HashSet<&str> = self.core_tools
            .iter().map(|s| s.as_str()).collect();

        for (category, kws) in &self.keyword_map {
            if kws.iter().any(|kw| context.contains(kw.as_str())) {
                selected_names.insert(category.as_str());
            }
        }

        available.iter().filter(|t| {
            self.core_tools.iter().any(|core| t.name.contains(core.as_str()))
                || self.keyword_map.keys().any(|cat| {
                    if selected_names.contains(cat.as_str()) {
                        t.name.to_lowercase().contains(cat.as_str())
                            || t.description.as_deref().unwrap_or("").to_lowercase().contains(cat.as_str())
                    } else {
                        false
                    }
                })
        }).collect()
    }
}

impl Default for ToolRouterSaver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TokenSaver for ToolRouterSaver {
    fn name(&self) -> &str { "tool-router" }
    fn stage(&self) -> SaverStage { SaverStage::PromptAssembly }
    fn priority(&self) -> u32 { 15 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        if input.tools.len() <= 6 {
            return Ok(SaverOutput {
                messages: input.messages,
                tools: input.tools,
                images: input.images,
                skipped: false,
                cached_response: None,
            });
        }

        let before_count = input.tools.len();
        let selected = self.select_tools(&input.tools, &input.messages);
        let selected_names: std::collections::HashSet<&str> = selected.iter().map(|t| t.name.as_str()).collect();

        let before_tokens: usize = input.tools.iter().map(|t| t.definition_tokens).sum();
        input.tools.retain(|t| selected_names.contains(t.name.as_str()));
        let after_tokens: usize = input.tools.iter().map(|t| t.definition_tokens).sum();

        let saved = before_tokens.saturating_sub(after_tokens);
        if saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "tool-router".into(),
                tokens_before: before_tokens,
                tokens_after: after_tokens,
                tokens_saved: saved,
                description: format!(
                    "pruned {}/{} tools, saved {} tokens",
                    before_count - input.tools.len(), before_count, saved
                ),
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
