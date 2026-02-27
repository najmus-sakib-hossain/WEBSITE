//! Aggressively minifies tool schemas to reduce input tokens.
//! SAVINGS: 40-70% on tool schema tokens
//! STAGE: PromptAssembly (priority 20)

use dx_core::*;
use std::sync::Mutex;

pub struct SchemaMinifierSaver {
    config: MinifyConfig,
    report: Mutex<TokenSavingsReport>,
}

#[derive(Clone)]
pub struct MinifyConfig {
    pub strip_descriptions: bool,
    pub strip_defaults: bool,
    pub strip_examples: bool,
    pub strip_titles: bool,
    pub strip_meta: bool,
}

impl Default for MinifyConfig {
    fn default() -> Self {
        Self {
            strip_descriptions: true,
            strip_defaults: true,
            strip_examples: true,
            strip_titles: true,
            strip_meta: true,
        }
    }
}

impl MinifyConfig {
    pub fn conservative() -> Self {
        Self {
            strip_descriptions: false,
            strip_defaults: true,
            strip_examples: true,
            strip_titles: true,
            strip_meta: true,
        }
    }

    pub fn aggressive() -> Self {
        Self::default()
    }
}

impl SchemaMinifierSaver {
    pub fn new(config: MinifyConfig) -> Self {
        Self {
            config,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn aggressive() -> Self {
        Self::new(MinifyConfig::aggressive())
    }

    pub fn conservative() -> Self {
        Self::new(MinifyConfig::conservative())
    }

    pub fn minify(&self, value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let mut new_map = serde_json::Map::new();
                for (key, val) in map {
                    if self.config.strip_descriptions && key == "description" { continue; }
                    if self.config.strip_defaults && key == "default" { continue; }
                    if self.config.strip_examples && (key == "examples" || key == "example") { continue; }
                    if self.config.strip_titles && key == "title" { continue; }
                    if self.config.strip_meta && (key == "$schema" || key == "$id" || key == "$comment") { continue; }
                    if key == "additionalProperties" { continue; }
                    new_map.insert(key.clone(), self.minify(val));
                }
                serde_json::Value::Object(new_map)
            }
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(|v| self.minify(v)).collect())
            }
            other => other.clone(),
        }
    }

    pub fn minify_description(desc: &str) -> String {
        desc.split('.')
            .next()
            .unwrap_or(desc)
            .trim()
            .to_string()
    }
}

#[async_trait::async_trait]
impl TokenSaver for SchemaMinifierSaver {
    fn name(&self) -> &str { "schema-minifier" }
    fn stage(&self) -> SaverStage { SaverStage::PromptAssembly }
    fn priority(&self) -> u32 { 20 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut total_before = 0usize;
        let mut total_after = 0usize;

        for tool in &mut input.tools {
            let before_str = serde_json::to_string(&tool.parameters).unwrap_or_default();
            let before_tokens = before_str.len() / 4;
            total_before += before_tokens + tool.description.len() / 4;

            tool.parameters = self.minify(&tool.parameters);
            tool.description = Self::minify_description(&tool.description);

            let after_str = serde_json::to_string(&tool.parameters).unwrap_or_default();
            let after_tokens = after_str.len() / 4;
            total_after += after_tokens + tool.description.len() / 4;

            tool.token_count = after_tokens;
        }

        let saved = total_before.saturating_sub(total_after);
        if saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "schema-minifier".into(),
                tokens_before: total_before,
                tokens_after: total_after,
                tokens_saved: saved,
                description: format!(
                    "minified {} tool schemas: {} → {} tokens ({:.1}% saved)",
                    input.tools.len(), total_before, total_after,
                    saved as f64 / total_before.max(1) as f64 * 100.0
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strips_descriptions() {
        let saver = SchemaMinifierSaver::aggressive();
        let input = serde_json::json!({
            "type": "object",
            "description": "verbose description",
            "properties": { "path": { "type": "string", "description": "file path" } }
        });
        let result = saver.minify(&input);
        assert!(result.get("description").is_none());
        assert!(result["properties"]["path"].get("description").is_none());
    }

    #[test]
    fn test_preserves_required_fields() {
        let saver = SchemaMinifierSaver::aggressive();
        let input = serde_json::json!({ "type": "object", "properties": { "p": { "type": "string" } }, "required": ["p"] });
        let result = saver.minify(&input);
        assert!(result.get("type").is_some());
        assert!(result.get("required").is_some());
    }

    #[test]
    fn test_minify_description_first_sentence() {
        assert_eq!(
            SchemaMinifierSaver::minify_description("Read file contents. Supports line ranges."),
            "Read file contents"
        );
    }
}
