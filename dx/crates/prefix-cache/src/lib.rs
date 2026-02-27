//! # prefix-cache
//!
//! Ensures byte-for-byte stable prompt prefixes so provider-side
//! prompt caching activates reliably.
//!
//! OpenAI caches prefixes ≥1024 tokens, discounts cached input 50%.
//! Anthropic cache_control marks static blocks for caching.
//! This crate guarantees the prefix is IDENTICAL across turns.
//!
//! SAVINGS: 50% on cached input tokens (provider pricing discount)
//! STAGE: PromptAssembly (priority 10)

use dx_core::*;
use std::collections::BTreeMap;

pub struct PrefixCacheSaver {
    last_prefix_hash: std::sync::Mutex<Option<blake3::Hash>>,
    cache_retention: Option<String>,
    report: std::sync::Mutex<TokenSavingsReport>,
}

impl PrefixCacheSaver {
    pub fn new() -> Self {
        Self {
            last_prefix_hash: std::sync::Mutex::new(None),
            cache_retention: Some("24h".to_string()),
            report: std::sync::Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_retention(mut self, retention: &str) -> Self {
        self.cache_retention = Some(retention.to_string());
        self
    }

    /// Sort tools alphabetically for deterministic ordering
    fn sort_tools(tools: &mut [ToolSchema]) {
        tools.sort_by(|a, b| a.name.cmp(&b.name));
    }

    /// Minify JSON schema with sorted keys (BTreeMap guarantees order)
    fn canonicalize_schema(schema: &serde_json::Value) -> serde_json::Value {
        match schema {
            serde_json::Value::Object(map) => {
                let sorted: BTreeMap<_, _> = map.iter()
                    .map(|(k, v)| (k.clone(), Self::canonicalize_schema(v)))
                    .collect();
                serde_json::to_value(sorted).unwrap_or_default()
            }
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(
                    arr.iter().map(Self::canonicalize_schema).collect()
                )
            }
            other => other.clone(),
        }
    }

    /// Compute prefix hash to detect whether cache will hit
    fn compute_hash(messages: &[Message], tools: &[ToolSchema]) -> blake3::Hash {
        let mut hasher = blake3::Hasher::new();

        // Hash system messages (stable prefix part)
        for msg in messages.iter().filter(|m| m.role == "system") {
            hasher.update(msg.content.as_bytes());
        }

        // Hash sorted tool schemas
        for tool in tools {
            hasher.update(tool.name.as_bytes());
            let schema_str = serde_json::to_string(&tool.parameters).unwrap_or_default();
            hasher.update(schema_str.as_bytes());
        }

        hasher.finalize()
    }
}

impl Default for PrefixCacheSaver {
    fn default() -> Self { Self::new() }
}

#[async_trait::async_trait]
impl TokenSaver for PrefixCacheSaver {
    fn name(&self) -> &str { "prefix-cache" }
    fn stage(&self) -> SaverStage { SaverStage::PromptAssembly }
    fn priority(&self) -> u32 { 10 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        // 1. Sort tools deterministically
        Self::sort_tools(&mut input.tools);

        // 2. Canonicalize each tool schema (sorted keys recursively)
        for tool in &mut input.tools {
            tool.parameters = Self::canonicalize_schema(&tool.parameters);
            let schema_str = serde_json::to_string(&tool.parameters).unwrap_or_default();
            tool.token_count = schema_str.len() / 4;
        }

        // 3. Ensure system messages come first (stable prefix)
        input.messages.sort_by(|a, b| {
            let a_system = if a.role == "system" { 0 } else { 1 };
            let b_system = if b.role == "system" { 0 } else { 1 };
            a_system.cmp(&b_system)
        });

        // 4. Compute and track prefix hash
        let new_hash = Self::compute_hash(&input.messages, &input.tools);
        let mut last = self.last_prefix_hash.lock().unwrap();
        let cache_hit = last.as_ref() == Some(&new_hash);
        *last = Some(new_hash);

        // 5. Update savings report on cache hit (50% pricing discount)
        if cache_hit {
            let cached_tokens: usize = input.messages.iter()
                .filter(|m| m.role == "system")
                .map(|m| m.token_count)
                .sum::<usize>()
                + input.tools.iter().map(|t| t.token_count).sum::<usize>();

            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "prefix-cache".into(),
                tokens_before: cached_tokens,
                tokens_after: cached_tokens,  // still sent, but 50% discounted
                tokens_saved: cached_tokens / 2, // 50% pricing discount
                description: format!(
                    "prefix cache hit: {} tokens at 50% discount", cached_tokens
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
    fn test_canonicalize_sorts_keys() {
        let input = serde_json::json!({
            "z_field": "last",
            "a_field": "first",
            "m_field": { "z": 1, "a": 2 }
        });
        let result = PrefixCacheSaver::canonicalize_schema(&input);
        let s = serde_json::to_string(&result).unwrap();
        assert!(s.find("a_field").unwrap() < s.find("m_field").unwrap());
        assert!(s.find("m_field").unwrap() < s.find("z_field").unwrap());
    }

    #[test]
    fn test_sort_tools_alphabetical() {
        let mut tools = vec![
            ToolSchema { name: "write".into(), description: "".into(), parameters: serde_json::json!({}), token_count: 0 },
            ToolSchema { name: "ask".into(), description: "".into(), parameters: serde_json::json!({}), token_count: 0 },
            ToolSchema { name: "read".into(), description: "".into(), parameters: serde_json::json!({}), token_count: 0 },
        ];
        PrefixCacheSaver::sort_tools(&mut tools);
        assert_eq!(tools[0].name, "ask");
        assert_eq!(tools[1].name, "read");
        assert_eq!(tools[2].name, "write");
    }

    #[test]
    fn test_hash_stability() {
        let msgs = vec![Message {
            role: "system".into(),
            content: "You are DX.".into(),
            images: vec![],
            tool_call_id: None,
            token_count: 5,
        }];
        let tools = vec![ToolSchema {
            name: "read".into(),
            description: "".into(),
            parameters: serde_json::json!({"type": "object"}),
            token_count: 5,
        }];

        let h1 = PrefixCacheSaver::compute_hash(&msgs, &tools);
        let h2 = PrefixCacheSaver::compute_hash(&msgs, &tools);
        assert_eq!(h1, h2);
    }
}
