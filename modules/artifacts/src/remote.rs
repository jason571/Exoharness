//! Copy declared same-origin MCP outputs into permanent signed delivery.
//! No redirects, forwarded MCP credentials, arbitrary URL proxy or generation.
use crate::{ArtifactError, Artifacts, PublishedArtifact, Result, files, mime};
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};
use tokio::io::AsyncWriteExt;

pub struct RemoteFile<'a> {
    pub service_url: &'a str,
    pub url: &'a str,
    pub filename: &'a str,
    pub size: Option<u64>,
    pub sha256: Option<&'a str>,
}

pub fn same_service_origin(service: &str, resource: &str) -> bool {
    let (Ok(service), Ok(resource)) = (url::Url::parse(service), url::Url::parse(resource)) else {
        return false;
    };
    matches!(service.scheme(), "http" | "https")
        && matches!(resource.scheme(), "http" | "https")
        && service.host_str().is_some()
        && service.origin() == resource.origin()
        && service.username().is_empty()
        && service.password().is_none()
        && resource.username().is_empty()
        && resource.password().is_none()
        && resource.fragment().is_none()
}

impl Artifacts {
    /// The caller must supply an authorized, current MCP registration and a
    /// resource declared by that invocation. Ownership is checked before GET.
    pub async fn import_remote(
        self: &Arc<Self>,
        owner: &str,
        session: &str,
        run: &str,
        input: RemoteFile<'_>,
    ) -> Result<PublishedArtifact> {
        if !same_service_origin(input.service_url, input.url)
            || input
                .size
                .is_some_and(|size| size == 0 || size > files::MAX_BYTES)
        {
            return Err(ArtifactError::Invalid);
        }
        let expected_type = mime::content_type(input.filename)?;
        let store = self.store.clone();
        let (o, s, r) = (owner.to_owned(), session.to_owned(), run.to_owned());
        tokio::task::spawn_blocking(move || store.run(&o, &s, &r))
            .await
            .map_err(|_| ArtifactError::Storage)??;
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(300))
            .build()
            .map_err(|_| ArtifactError::Storage)?;
        let mut response = client
            .get(input.url)
            .send()
            .await
            .map_err(|_| ArtifactError::Storage)?;
        if response.status() != reqwest::StatusCode::OK
            || response
                .content_length()
                .is_some_and(|size| size == 0 || size > files::MAX_BYTES)
        {
            return Err(ArtifactError::Invalid);
        }
        if response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|actual| {
                let actual = actual.split(';').next().unwrap_or("").trim();
                !actual
                    .eq_ignore_ascii_case(expected_type.split(';').next().unwrap_or(expected_type))
                    && !actual.eq_ignore_ascii_case("application/octet-stream")
            })
        {
            return Err(ArtifactError::Invalid);
        }
        let temp = tempfile::tempfile().map_err(|_| ArtifactError::Storage)?;
        let mut writer =
            tokio::fs::File::from_std(temp.try_clone().map_err(|_| ArtifactError::Storage)?);
        let mut size = 0u64;
        let mut hash = Sha256::new();
        while let Some(bytes) = response.chunk().await.map_err(|_| ArtifactError::Storage)? {
            size = size
                .checked_add(bytes.len() as u64)
                .ok_or(ArtifactError::Invalid)?;
            if size > files::MAX_BYTES {
                return Err(ArtifactError::Invalid);
            }
            hash.update(&bytes);
            writer
                .write_all(&bytes)
                .await
                .map_err(|_| ArtifactError::Storage)?;
        }
        if size == 0
            || input.size.is_some_and(|expected| expected != size)
            || input
                .sha256
                .is_some_and(|expected| expected != format!("{:x}", hash.finalize()))
        {
            return Err(ArtifactError::Invalid);
        }
        writer.flush().await.map_err(|_| ArtifactError::Storage)?;
        drop(writer);
        let service = self.clone();
        let (o, s, r, name) = (
            owner.to_owned(),
            session.to_owned(),
            run.to_owned(),
            input.filename.to_owned(),
        );
        tokio::task::spawn_blocking(move || service.publish_file(&o, &s, &r, &name, temp))
            .await
            .map_err(|_| ArtifactError::Storage)?
    }
}
