use crate::{ArtifactError, PublishedArtifact, Result, files, mime, signing::Signer};
#[path = "project.rs"]
mod project;
#[path = "remote.rs"]
mod remote;
use exoharness_core::services::Service;
use exoharness_storage_postgres::{PostgresStore, StoredArtifact};
pub use remote::{RemoteFile, same_service_origin};
use sha2::{Digest, Sha256};
use std::{fs::File, path::PathBuf};
pub struct Artifacts {
    root: PathBuf,
    store: PostgresStore,
    signer: Signer,
    base: url::Url,
}
impl Service for Artifacts {
    const KEY: &'static str = "artifacts";
}
impl Artifacts {
    /// Same signed read capability as file delivery; no paths, raw inputs or credentials.
    pub fn metadata_signed(
        &self,
        id: &str,
        name: &str,
        expires: u64,
        signature: &str,
    ) -> Result<serde_json::Value> {
        let (record, _) = self.open_signed(id, name, expires, signature)?;
        let provenance = self
            .store
            .artifact_source(&record)?
            .map(|(invocation_id, args)| {
                let hashes: serde_json::Map<String, serde_json::Value> = args
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|s| {
                            (
                                key.clone(),
                                serde_json::json!(format!("{:x}", Sha256::digest(s.as_bytes()))),
                            )
                        })
                    })
                    .collect();
                serde_json::json!({"invocation_id":invocation_id,"input_string_sha256":hashes})
            });
        Ok(
            serde_json::json!({"artifact_id":record.id,"filename":record.filename,"content_type":record.content_type,"size_bytes":record.size_bytes,"sha256":record.sha256,"provenance":provenance}),
        )
    }
    pub fn open_owned(&self, owner: &str, id: &str) -> Result<(StoredArtifact, File)> {
        let record = self.store.owned_artifact(owner, id)?;
        if record.size_bytes < 0 {
            return Err(ArtifactError::Access);
        }
        let file = files::open(&self.root, &record.sha256, record.size_bytes as u64)?;
        Ok((record, file))
    }
    pub fn health(&self) -> Result<()> {
        files::private(&self.root)?;
        self.store.health()?;
        Ok(())
    }
    pub fn open(root: PathBuf, store: PostgresStore, public_base: &str) -> Result<Self> {
        let base = url::Url::parse(public_base).map_err(|_| ArtifactError::Invalid)?;
        if !matches!(base.scheme(), "http" | "https")
            || base.host_str().is_none()
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || base.path() != "/"
        {
            return Err(ArtifactError::Invalid);
        }
        if !root.try_exists().map_err(|_| ArtifactError::Storage)? {
            files::private(root.parent().ok_or(ArtifactError::Storage)?)?;
        }
        files::private(&root)?;
        files::private(&root.join("objects"))?;
        Ok(Self {
            signer: Signer::open(&root)?,
            root,
            store,
            base,
        })
    }
    pub fn public_host(&self) -> String {
        format!(
            "{}{}",
            self.base.host_str().expect("validated host"),
            self.base
                .port()
                .map(|p| format!(":{p}"))
                .unwrap_or_default()
        )
    }
    pub fn publish_file(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        filename: &str,
        source: File,
    ) -> Result<PublishedArtifact> {
        let content_type = mime::content_type(filename)?;
        // Check ownership before copying a large file; commit checks it again.
        self.store.run(owner, session, run)?;
        let (sha256, size) = files::save(&self.root, source)?;
        let identity = serde_json::to_vec(&(owner, run, filename, &sha256))
            .map_err(|_| ArtifactError::Invalid)?;
        let id = format!("artifact-{:x}", Sha256::digest(identity));
        let artifact = self.store.save_artifact(
            owner,
            session,
            &StoredArtifact {
                id,
                run_id: run.into(),
                filename: filename.into(),
                content_type: content_type.into(),
                relative_path: format!("objects/{sha256}.bin"),
                size_bytes: size as i64,
                sha256,
            },
        )?;
        let url = self.url(&artifact.id, filename, 0, false)?;
        let download_url = format!("{url}&download=true");
        let preview_url = content_type
            .starts_with("text/html")
            .then(|| self.url(&artifact.id, filename, 0, true))
            .transpose()?;
        Ok(PublishedArtifact {
            artifact,
            url,
            download_url,
            preview_url,
        })
    }
    pub fn url(&self, id: &str, name: &str, expires: u64, preview: bool) -> Result<String> {
        mime::content_type(name)?;
        let mut url = self.base.clone();
        {
            let mut paths = url
                .path_segments_mut()
                .map_err(|_| ArtifactError::Invalid)?;
            paths.pop_if_empty().push("artifacts").push(id).push(name);
            if preview {
                paths.push("preview");
            }
        }
        url.query_pairs_mut()
            .append_pair("expires", &expires.to_string())
            .append_pair("signature", &self.signer.sign(id, name, expires));
        Ok(url.into())
    }
    pub fn open_signed(
        &self,
        id: &str,
        name: &str,
        expires: u64,
        signature: &str,
    ) -> Result<(StoredArtifact, File)> {
        self.signer.verify(id, name, expires, signature)?;
        let record = self.store.delivery_artifact(id)?;
        if record.filename != name
            || record.content_type != mime::content_type(name)?
            || record.size_bytes < 0
        {
            return Err(ArtifactError::Access);
        }
        let file = files::open(&self.root, &record.sha256, record.size_bytes as u64)?;
        Ok((record, file))
    }
}
