//! Persistent disk-backed response cache with zstd compression.
//! SAVINGS: 100% on repeated identical prompts
//! STAGE: CallElimination (priority 2)

use dx_core::*;
use redb::{Database, TableDefinition};
use std::path::PathBuf;
use std::sync::Mutex;

const CACHE_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("response_cache");

pub struct ResponseCacheSaver {
    db_path: PathBuf,
    enabled: bool,
    report: Mutex<TokenSavingsReport>,
}

impl ResponseCacheSaver {
    pub fn new(cache_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&cache_dir).ok();
        let db_path = cache_dir.join("response_cache.redb");
        Self {
            db_path,
            enabled: true,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_default_path() -> Self {
        let path = std::env::var("HOME")
            .map(|h| PathBuf::from(h).join(".cache").join("dx"))
            .unwrap_or_else(|_| PathBuf::from(".dx-cache"));
        Self::new(path)
    }

    pub fn disabled() -> Self {
        Self {
            db_path: PathBuf::new(),
            enabled: false,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn build_key(messages: &[Message], tools: &[ToolDefinition]) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        for msg in messages {
            let canonical = semantic_cache::canonicalize(&msg.content);
            hasher.update(msg.role.as_bytes());
            hasher.update(canonical.as_bytes());
        }
        for tool in tools {
            hasher.update(tool.name.as_bytes());
        }
        *hasher.finalize().as_bytes()
    }

    fn compress_value(data: &str) -> Vec<u8> {
        zstd::encode_all(data.as_bytes(), 3).unwrap_or_else(|_| data.as_bytes().to_vec())
    }

    fn decompress_value(data: &[u8]) -> Option<String> {
        let decompressed = zstd::decode_all(data).ok()?;
        String::from_utf8(decompressed).ok()
    }

    pub fn store(&self, key: &[u8; 32], response: &str) {
        if !self.enabled { return; }
        let compressed = Self::compress_value(response);
        let db = match Database::create(&self.db_path) {
            Ok(d) => d,
            Err(_) => return,
        };
        let tx = match db.begin_write() {
            Ok(t) => t,
            Err(_) => return,
        };
        {
            let mut table = match tx.open_table(CACHE_TABLE) {
                Ok(t) => t,
                Err(_) => return,
            };
            let _ = table.insert(key.as_slice(), compressed.as_slice());
        }
        let _ = tx.commit();
    }

    pub fn lookup(&self, key: &[u8; 32]) -> Option<String> {
        if !self.enabled { return None; }
        let db = Database::open(&self.db_path).ok()?;
        let tx = db.begin_read().ok()?;
        let table = tx.open_table(CACHE_TABLE).ok()?;
        let val = table.get(key.as_slice()).ok()??;
        Self::decompress_value(val.value())
    }
}

#[async_trait::async_trait]
impl TokenSaver for ResponseCacheSaver {
    fn name(&self) -> &str { "response-cache" }
    fn stage(&self) -> SaverStage { SaverStage::CallElimination }
    fn priority(&self) -> u32 { 2 }

    async fn process(&self, input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        if !self.enabled {
            return Ok(SaverOutput {
                messages: input.messages.clone(),
                tools: input.tools.clone(),
                images: input.images.clone(),
                skipped: false,
                cached_response: None,
            });
        }

        let key = Self::build_key(&input.messages, &input.tools);

        if let Some(cached) = self.lookup(&key) {
            let total_tokens: usize = input.messages.iter().map(|m| m.token_count).sum();
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "response-cache".into(),
                tokens_before: total_tokens,
                tokens_after: 0,
                tokens_saved: total_tokens,
                description: format!("cache hit, saved {} tokens", total_tokens),
            };
            return Ok(SaverOutput {
                messages: input.messages,
                tools: input.tools,
                images: input.images,
                skipped: true,
                cached_response: Some(cached),
            });
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
