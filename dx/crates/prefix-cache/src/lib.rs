//! # prefix-cache
//!
//! Ensures byte-for-byte stable prompt prefixes so provider-side
//! prompt caching activates reliably.
//!
//! ## Verified Savings (TOKEN.md research)
//!
//! Provider caching discounts vary by model (as of early 2026):
//! - **GPT-5 family**: 90% off cached input tokens
//! - **GPT-4.1 family**: 75% off cached input tokens
//! - **GPT-4o / O-series**: 50% off cached input tokens
//! - **Anthropic Claude**: 90% off with cache_control
//!
//! Caching activates automatically for prefixes ≥1024 tokens.
//! Cache lifetime: 5-10 minutes of inactivity, up to 1 hour.
//!
//! This crate's job: guarantee the prefix is IDENTICAL byte-for-byte
//! across consecutive turns so the provider's cache hits reliably.
//!
//! SAVINGS: 50-90% on cached input tokens (provider discount)
//! STAGE: PromptAssembly (priority 10)

use dx_core::*;
use std::collections::BTreeMap;

/// Model family for provider-specific cache discount.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModelFamily {
    /// GPT-5 family: 90% discount on cached tokens
    Gpt5,
    /// GPT-4.1 family: 75% discount on cached tokens
    Gpt41,
    /// GPT-4o / O-series: 50% discount on cached tokens
    Gpt4o,
    /// Anthropic Claude: 90% discount with cache_control
    Claude,
    /// Unknown model: 50% discount assumed (conservative)
    Unknown,
}

impl ModelFamily {
    pub fn from_model_name(model: &str) -> Self {
        let m = model.to_lowercase();
        if m.contains("gpt-5") { ModelFamily::Gpt5 }
        else if m.contains("gpt-4.1") || m.contains("gpt4.1") { ModelFamily::Gpt41 }
        else if m.contains("gpt-4o") || m.starts_with("o1") || m.starts_with("o3") || m.starts_with("o4") { ModelFamily::Gpt4o }
        else if m.contains("claude") || m.contains("anthropic") { ModelFamily::Claude }
        else { ModelFamily::Unknown }
    }

    /// Fractional discount on cached input tokens (0.90 = 90% off).
    pub fn cache_discount(&self) -> f64 {
        match self {
            ModelFamily::Gpt5 => 0.90,
            ModelFamily::Gpt41 => 0.75,
            ModelFamily::Gpt4o => 0.50,
            ModelFamily::Claude => 0.90,
            ModelFamily::Unknown => 0.50,
        }
    }
}

pub struct PrefixCacheSaver {
    last_prefix_hash: std::sync::Mutex<Option<blake3::Hash>>,
    report: std::sync::Mutex<TokenSavingsReport>,
}

impl PrefixCacheSaver {
    pub fn new() -> Self {
        Self {
            last_prefix_hash: std::sync::Mutex::new(None),
            report: std::sync::Mutex::new(TokenSavingsReport::default()),
        }
    }

    /// Sort tools alphabetically — critical for cache stability.
    pub fn sort_tools(tools: &mut [ToolSchema]) {
        tools.sort_by(|a, b| a.name.cmp(&b.name));
    }

    /// Recursively sort JSON object keys via BTreeMap.
    /// Same logical schema → identical serialized bytes.
    pub fn canonicalize_schema(schema: &serde_json::Value) -> serde_json::Value {
        match schema {
            serde_json::Value::Object(map) => {
                let sorted: BTreeMap<_, _> = map.iter()
                    .map(|(k, v)| (k.clone(), Self::canonicalize_schema(v)))
                    .collect();
                serde_json::to_value(sorted).unwrap_or_default()
            }
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(Self::canonicalize_schema).collect())
            }
            other => other.clone(),
        }
    }

    /// Hash the stable prefix: system messages + sorted tool schemas.
    pub fn compute_prefix_hash(messages: &[Message], tools: &[ToolSchema]) -> blake3::Hash {
        let mut hasher = blake3::Hasher::new();
        for msg in messages.iter().filter(|m| m.role == "system") {
            hasher.update(b"\x00sys\x00");
            hasher.update(msg.content.as_bytes());
        }
        for tool in tools {
            hasher.update(b"\x00tool\x00");
            hasher.update(tool.name.as_bytes());
            let s = serde_json::to_string(&tool.parameters).unwrap_or_default();
            hasher.update(s.as_bytes());
        }
        hasher.finalize()
    }

    /// Sum tokens in the stable prefix (system messages + tools).
    pub fn count_prefix_tokens(messages: &[Message], tools: &[ToolSchema]) -> usize {
        messages.iter().filter(|m| m.role == "system").map(|m| m.token_count).sum::<usize>()
            + tools.iter().map(|t| t.token_count).sum::<usize>()
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

    async fn process(&self, mut input: SaverInput, ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        // 1. Sort tools deterministically for cache stability
        Self::sort_tools(&mut input.tools);

        // 2. Canonicalize each tool schema (sorted keys recursively)
        for tool in &mut input.tools {
            tool.parameters = Self::canonicalize_schema(&tool.parameters);
            let s = serde_json::to_string(&tool.parameters).unwrap_or_default();
            tool.token_count = (tool.name.len() + tool.description.len() + s.len()) / 4;
        }

        // 3. System messages MUST come first (stable prefix before dynamic content)
        input.messages.sort_by_key(|m| if m.role == "system" { 0u8 } else { 1u8 });

        // 4. Compute prefix hash, check for cache hit
        let current_hash = Self::compute_prefix_hash(&input.messages, &input.tools);
        let mut last = self.last_prefix_hash.lock().unwrap();
        let cache_hit = last.as_ref() == Some(&current_hash);
        *last = Some(current_hash);

        // 5. Provider caching requires ≥1024 tokens in the prefix
        let prefix_tokens = Self::count_prefix_tokens(&input.messages, &input.tools);
        let qualifies = prefix_tokens >= 1024;

        if cache_hit && qualifies {
            let family = ModelFamily::from_model_name(&ctx.model);
            let discount = family.cache_discount();
            let effective_saved = (prefix_tokens as f64 * discount) as usize;

            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "prefix-cache".into(),
                tokens_before: prefix_tokens,
                tokens_after: prefix_tokens,
                tokens_saved: effective_saved,
                description: format!(
                    "prefix cache hit [{}]: {} tokens × {:.0}% discount = {} effective saved",
                    if ctx.model.is_empty() { "unknown model" } else { &ctx.model },
                    prefix_tokens,
                    discount * 100.0,
                    effective_saved
                ),
            };
        } else {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "prefix-cache".into(),
                tokens_before: prefix_tokens,
                tokens_after: prefix_tokens,
                tokens_saved: 0,
                description: if !qualifies {
                    format!("prefix too short for caching ({} < 1024 tokens)", prefix_tokens)
                } else {
                    "prefix changed — cache miss this turn".into()
                },
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

    fn sys_msg(content: &str, tokens: usize) -> Message {
        Message { role: "system".into(), content: content.into(), images: vec![], tool_call_id: None, token_count: tokens }
    }

    fn tool(name: &str) -> ToolSchema {
        ToolSchema { name: name.into(), description: "desc".into(), parameters: serde_json::json!({"type": "object"}), token_count: 50 }
    }

    #[test]
    fn model_family_detection() {
        assert_eq!(ModelFamily::from_model_name("gpt-5"), ModelFamily::Gpt5);
        assert_eq!(ModelFamily::from_model_name("gpt-4.1-mini"), ModelFamily::Gpt41);
        assert_eq!(ModelFamily::from_model_name("gpt-4o"), ModelFamily::Gpt4o);
        assert_eq!(ModelFamily::from_model_name("claude-3-7-sonnet"), ModelFamily::Claude);
        assert_eq!(ModelFamily::from_model_name("unknown-xyz"), ModelFamily::Unknown);
    }

    #[test]
    fn cache_discounts_match_docs() {
        assert_eq!(ModelFamily::Gpt5.cache_discount(), 0.90);
        assert_eq!(ModelFamily::Gpt41.cache_discount(), 0.75);
        assert_eq!(ModelFamily::Gpt4o.cache_discount(), 0.50);
        assert_eq!(ModelFamily::Claude.cache_discount(), 0.90);
        assert_eq!(ModelFamily::Unknown.cache_discount(), 0.50);
    }

    #[test]
    fn hash_is_deterministic() {
        let msgs = vec![sys_msg("You are DX.", 5)];
        let tools = vec![tool("read")];
        assert_eq!(PrefixCacheSaver::compute_prefix_hash(&msgs, &tools),
                   PrefixCacheSaver::compute_prefix_hash(&msgs, &tools));
    }

    #[test]
    fn sort_tools_alphabetical() {
        let mut tools = vec![tool("write"), tool("ask"), tool("read")];
        PrefixCacheSaver::sort_tools(&mut tools);
        assert_eq!(tools[0].name, "ask");
        assert_eq!(tools[1].name, "read");
        assert_eq!(tools[2].name, "write");
    }

    #[test]
    fn canonicalize_sorts_json_keys() {
        let input = serde_json::json!({ "z": "last", "a": "first" });
        let out = PrefixCacheSaver::canonicalize_schema(&input);
        let s = serde_json::to_string(&out).unwrap();
        assert!(s.find('"' ).unwrap() < s.len()); // basic sanity
        let a_pos = s.find("\"a\"").unwrap();
        let z_pos = s.find("\"z\"").unwrap();
        assert!(a_pos < z_pos);
    }

    #[test]
    fn prefix_under_1024_does_not_qualify() {
        let msgs = vec![sys_msg("short", 100)];
        let tokens = PrefixCacheSaver::count_prefix_tokens(&msgs, &[]);
        assert!(tokens < 1024);
    }
}
