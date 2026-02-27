//! In-memory semantic cache. Canonicalizes prompts to maximize cache hits.
//! SAVINGS: 100% per cache hit (entire API call skipped)
//! STAGE: CallElimination (priority 1)

use dx_core::*;
use moka::sync::Cache;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Clone)]
pub struct CachedEntry {
    pub response: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
}

pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub total_tokens_saved: usize,
}

pub struct SemanticCacheSaver {
    cache: Cache<blake3::Hash, CachedEntry>,
    stats: Mutex<CacheStats>,
    report: Mutex<TokenSavingsReport>,
}

impl SemanticCacheSaver {
    pub fn new(max_entries: u64, ttl: Duration) -> Self {
        Self {
            cache: Cache::builder()
                .max_capacity(max_entries)
                .time_to_live(ttl)
                .build(),
            stats: Mutex::new(CacheStats { hits: 0, misses: 0, total_tokens_saved: 0 }),
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(10_000, Duration::from_secs(86400))
    }

    /// Canonicalize prompt text to maximize cache hits.
    pub fn canonicalize(text: &str) -> String {
        let mut s = text.to_string();

        // Strip ISO timestamps
        s = regex_lite::Regex::new(r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}[^\s]*")
            .unwrap().replace_all(&s, "[T]").to_string();

        // Strip Unix timestamps
        s = regex_lite::Regex::new(r"\b1[6-7]\d{8}\b")
            .unwrap().replace_all(&s, "[TS]").to_string();

        // Normalize home directory paths
        s = regex_lite::Regex::new(r"(/home/\w+/|/Users/\w+/|C:\\Users\\\w+\\)")
            .unwrap().replace_all(&s, "~/").to_string();

        // Normalize UUIDs
        s = regex_lite::Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
            .unwrap().replace_all(&s, "[ID]").to_string();

        // Normalize SHA hashes (40-char hex)
        s = regex_lite::Regex::new(r"\b[0-9a-f]{40}\b")
            .unwrap().replace_all(&s, "[SHA]").to_string();

        // Collapse whitespace
        s = regex_lite::Regex::new(r"\s+")
            .unwrap().replace_all(&s, " ").to_string();

        s.trim().to_string()
    }

    pub fn hash(text: &str) -> blake3::Hash {
        blake3::hash(text.as_bytes())
    }

    pub fn build_key(messages: &[Message]) -> blake3::Hash {
        let mut hasher = blake3::Hasher::new();
        for msg in messages.iter().filter(|m| m.role == "user") {
            let canonical = Self::canonicalize(&msg.content);
            hasher.update(canonical.as_bytes());
        }
        hasher.finalize()
    }

    pub fn store(&self, messages: &[Message], response: &str, input_tokens: usize, output_tokens: usize) {
        let key = Self::build_key(messages);
        self.cache.insert(key, CachedEntry {
            response: response.to_string(),
            input_tokens,
            output_tokens,
        });
    }

    pub fn hit_rate(&self) -> f64 {
        let stats = self.stats.lock().unwrap();
        let total = stats.hits + stats.misses;
        if total == 0 { return 0.0; }
        stats.hits as f64 / total as f64
    }
}

#[async_trait::async_trait]
impl TokenSaver for SemanticCacheSaver {
    fn name(&self) -> &str { "semantic-cache" }
    fn stage(&self) -> SaverStage { SaverStage::CallElimination }
    fn priority(&self) -> u32 { 1 }

    async fn process(&self, input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let key = Self::build_key(&input.messages);

        if let Some(cached) = self.cache.get(&key) {
            let mut stats = self.stats.lock().unwrap();
            stats.hits += 1;
            let tokens_saved = cached.input_tokens + cached.output_tokens;
            stats.total_tokens_saved += tokens_saved;

            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "semantic-cache".into(),
                tokens_before: tokens_saved,
                tokens_after: 0,
                tokens_saved,
                description: format!(
                    "cache hit (rate: {:.1}%), saved {} tokens",
                    self.hit_rate() * 100.0, tokens_saved
                ),
            };

            return Ok(SaverOutput {
                messages: input.messages,
                tools: input.tools,
                images: input.images,
                skipped: true,
                cached_response: Some(cached.response.clone()),
            });
        }

        {
            let mut stats = self.stats.lock().unwrap();
            stats.misses += 1;
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
    fn test_canonicalize_timestamps() {
        let input = "Error at 2026-02-27T14:30:00Z in /home/alice/project/src/main.rs";
        let result = SemanticCacheSaver::canonicalize(input);
        assert!(!result.contains("2026"));
        assert!(!result.contains("alice"));
        assert!(result.contains("[T]"));
        assert!(result.contains("~/"));
    }

    #[test]
    fn test_canonicalize_uuids() {
        let input = "request 550e8400-e29b-41d4-a716-446655440000 failed";
        let result = SemanticCacheSaver::canonicalize(input);
        assert!(result.contains("[ID]"));
        assert!(!result.contains("550e8400"));
    }

    #[test]
    fn test_same_content_different_timestamps_same_hash() {
        let a = "Error at 2026-01-01T00:00:00Z: connection refused";
        let b = "Error at 2026-12-31T23:59:59Z: connection refused";
        let ha = SemanticCacheSaver::hash(&SemanticCacheSaver::canonicalize(a));
        let hb = SemanticCacheSaver::hash(&SemanticCacheSaver::canonicalize(b));
        assert_eq!(ha, hb);
    }

    #[test]
    fn test_different_content_different_hash() {
        let a = "compile error in main.rs";
        let b = "runtime error in utils.rs";
        let ha = SemanticCacheSaver::hash(&SemanticCacheSaver::canonicalize(a));
        let hb = SemanticCacheSaver::hash(&SemanticCacheSaver::canonicalize(b));
        assert_ne!(ha, hb);
    }
}
