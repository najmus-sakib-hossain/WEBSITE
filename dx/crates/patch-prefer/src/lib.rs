//! Instructs the model to use patch/diff output instead of full file rewrites.
//! SAVINGS: 90-98% on file editing outputs
//! STAGE: PromptAssembly (priority 25)

use dx_core::*;
use similar::TextDiff;
use std::sync::Mutex;

pub struct PatchPreferSaver {
    min_file_tokens: usize,
    report: Mutex<TokenSavingsReport>,
}

impl PatchPreferSaver {
    pub fn new() -> Self {
        Self {
            min_file_tokens: 50,
            report: Mutex::new(TokenSavingsReport::default()),
        }
    }

    const PATCH_INSTRUCTION: &str =
        "\nWhen editing files, use the 'patch' tool with unified diff format instead of 'write' with full file content. This is mandatory for files over 50 lines.";

    /// Calculate savings estimate for patch vs full rewrite.
    pub fn savings_estimate(original: &str, new_content: &str) -> (usize, usize, f64) {
        let full_tokens = new_content.len() / 4;
        let diff = TextDiff::from_lines(original, new_content);
        let unified = diff.unified_diff().context_radius(3).to_string();
        let patch_tokens = unified.len() / 4;
        let savings_pct = if full_tokens > 0 {
            (full_tokens.saturating_sub(patch_tokens)) as f64 / full_tokens as f64 * 100.0
        } else {
            0.0
        };
        (full_tokens, patch_tokens, savings_pct)
    }

    /// Check if content change ratio suggests patch would have been better.
    pub fn should_have_been_patch(original: &str, new_content: &str) -> bool {
        let diff = TextDiff::from_lines(original, new_content);
        let changed: usize = diff.iter_all_changes()
            .filter(|c| c.tag() != similar::ChangeTag::Equal)
            .count();
        let total = new_content.lines().count();
        total > 10 && (changed as f64 / total.max(1) as f64) < 0.3
    }
}

impl Default for PatchPreferSaver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TokenSaver for PatchPreferSaver {
    fn name(&self) -> &str { "patch-prefer" }
    fn stage(&self) -> SaverStage { SaverStage::PromptAssembly }
    fn priority(&self) -> u32 { 25 }

    async fn process(&self, mut input: SaverInput, _ctx: &SaverContext) -> Result<SaverOutput, SaverError> {
        if let Some(sys_msg) = input.messages.iter_mut().find(|m| m.role == "system") {
            if !sys_msg.content.contains("patch") && !sys_msg.content.contains("diff") {
                sys_msg.content.push_str(Self::PATCH_INSTRUCTION);
                sys_msg.token_count = sys_msg.content.len() / 4;
            }
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
