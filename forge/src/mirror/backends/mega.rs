//! Mega.nz backend — placeholder until a stable Rust crate is available.
//! megalib 0.9 is very new; this stub stores credentials and returns a handle
//! so the manifest round-trips correctly.
use crate::mirror::{auth::AuthStore, MirrorBackend, MirrorError, MirrorMetadata, MirrorTarget, MediaType};
use async_trait::async_trait;
use std::sync::Arc;

pub struct MegaBackend {
    auth: Arc<AuthStore>,
}

impl MegaBackend {
    pub fn new(auth: Arc<AuthStore>) -> Self { Self { auth } }
}

#[async_trait]
impl MirrorBackend for MegaBackend {
    fn name(&self) -> &'static str { "mega" }

    fn can_handle(&self, _: &MediaType) -> bool { true }

    async fn upload(&self, data: Vec<u8>, meta: &MirrorMetadata) -> Result<MirrorTarget, MirrorError> {
        // Ensure credentials exist before proceeding
        let _bundle = self
            .auth
            .load("mega")
            .map_err(|e| MirrorError::Upload(e.to_string()))?
            .ok_or(MirrorError::AuthMissing("mega"))?;

        // TODO: replace with megalib = "0.9" full AES-128-CTR encrypted upload
        let handle = format!(
            "forge-{}-{}",
            blake3::hash(meta.filename.as_bytes()).to_hex(),
            data.len()
        );

        tracing::warn!(
            "Mega: megalib integration pending — placeholder handle: {handle}"
        );
        Ok(MirrorTarget::Mega { handle })
    }
}
