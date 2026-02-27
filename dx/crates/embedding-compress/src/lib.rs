//! Removes near-duplicate embedding/retrieval messages using Jaccard similarity.
//! SAVINGS: 20-50% on retrieval-heavy conversations
//! STAGE: PrePrompt (priority 35)

use dx_core::*;
use std::collections::HashSet;
use std::sync::Mutex;

pub struct EmbeddingCompressSaver {
    similarity_threshold: f64,
    report: Mutex<TokenSavingsReport>,
}

impl EmbeddingCompressSaver {
    pub fn new(similarity_threshold: f64) -> Self {
        Self {
            similarity_threshold,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(0.85)
    }

    /// Jaccard similarity on word sets.
    pub fn text_similarity(a: &str, b: &str) -> f64 {
        let words_a: HashSet<&str> = a.split_whitespace().collect();
        let words_b: HashSet<&str> = b.split_whitespace().collect();
        let intersection = words_a.intersection(&words_b).count();
        let union = words_a.union(&words_b).count();
        if union == 0 { 1.0 } else { intersection as f64 / union as f64 }
    }
}

#[async_trait::async_trait]
impl TokenSaver for EmbeddingCompressSaver {
    fn name(&self) -> &str { "embedding-compress" }
    fn stage(&self) -> SaverStage { SaverStage::PrePrompt }
    fn priority(&self) -> u32 { 35 }

    async fn process(&self, input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        let mut kept: Vec<Message> = Vec::new();
        let mut total_saved = 0usize;

        for msg in input.messages {
            // Only deduplicate tool/context messages
            if msg.role != "tool" && msg.tool_call_id.is_none() {
                kept.push(msg);
                continue;
            }

            let is_dup = kept.iter().any(|k| {
                (k.role == "tool" || k.tool_call_id.is_some())
                    && Self::text_similarity(&k.content, &msg.content) >= self.similarity_threshold
            });

            if is_dup {
                total_saved += msg.token_count;
            } else {
                kept.push(msg);
            }
        }

        if total_saved > 0 {
            let mut report = self.report.lock().unwrap();
            *report = TokenSavingsReport {
                technique: "embedding-compress".into(),
                tokens_before: total_saved,
                tokens_after: 0,
                tokens_saved: total_saved,
                description: format!("removed {} near-duplicate tool tokens", total_saved),
            };
        }

        Ok(SaverOutput {
            messages: kept,
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
    fn test_identical_similarity() {
        assert!((EmbeddingCompressSaver::text_similarity("hello world foo", "hello world foo") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_different_similarity() {
        let s = EmbeddingCompressSaver::text_similarity("apple banana cherry", "dog cat fish");
        assert!(s < 0.1);
    }
}
