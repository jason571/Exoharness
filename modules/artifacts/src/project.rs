//! Immutable static-project publication. Manifest membership, not a raw path,
//! authorizes each asset. Nothing executes and no user directory is exposed.
use crate::{ArtifactError, Artifacts, Result, files};
use exoharness_coding::paths::WorkspacePath;
use exoharness_storage_postgres::StoredArtifact;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
};
const MANIFEST: &str = "project-preview.json";
#[derive(Serialize, Deserialize)]
struct Project {
    version: u8,
    entrypoint: String,
    members: BTreeMap<String, Member>,
}
#[derive(Serialize, Deserialize)]
struct Member {
    id: String,
    filename: String,
}
fn logical_path(path: &str) -> Result<()> {
    WorkspacePath::parse(path).map_err(|_| ArtifactError::Invalid)?;
    if path.contains(['%', '?', '#', '"', '\'']) || path.split('/').next() == Some("skill") {
        return Err(ArtifactError::Invalid);
    }
    Ok(())
}
impl Artifacts {
    pub fn publish_project(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        entrypoint: Option<&str>,
        mut sources: BTreeMap<String, File>,
    ) -> Result<Value> {
        if sources.is_empty() || sources.len() > 128 {
            return Err(ArtifactError::Invalid);
        }
        if let Some(entrypoint) = entrypoint {
            logical_path(entrypoint)?;
            if !(entrypoint.ends_with(".html") || entrypoint.ends_with(".htm"))
                || !sources.contains_key(entrypoint)
            {
                return Err(ArtifactError::Invalid);
            }
        }
        self.store.run(owner, session, run)?;
        let mut total = 0;
        for (path, file) in &sources {
            logical_path(path)?;
            let size = file.metadata().map_err(|_| ArtifactError::Storage)?.len();
            if !file
                .metadata()
                .map_err(|_| ArtifactError::Storage)?
                .is_file()
                || size > 8 * 1024 * 1024
            {
                return Err(ArtifactError::Invalid);
            }
            total += size;
            if total > 32 * 1024 * 1024 {
                return Err(ArtifactError::Invalid);
            }
            if entrypoint.is_some() {
                crate::mime::content_type(path.rsplit('/').next().ok_or(ArtifactError::Invalid)?)?;
            }
        }
        let mut archive = tempfile::tempfile().map_err(|_| ArtifactError::Storage)?;
        {
            let mut zip = zip::ZipWriter::new(&mut archive);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for (path, file) in &mut sources {
                file.seek(SeekFrom::Start(0))
                    .map_err(|_| ArtifactError::Storage)?;
                zip.start_file(path, options)
                    .map_err(|_| ArtifactError::Storage)?;
                std::io::copy(file, &mut zip).map_err(|_| ArtifactError::Storage)?;
                file.seek(SeekFrom::Start(0))
                    .map_err(|_| ArtifactError::Storage)?;
            }
            zip.finish().map_err(|_| ArtifactError::Storage)?;
        }
        archive
            .seek(SeekFrom::Start(0))
            .map_err(|_| ArtifactError::Storage)?;
        let Some(entrypoint) = entrypoint else {
            let count = sources.len();
            let archive = self.publish_file(owner, session, run, "project.zip", archive)?;
            let mut card =
                serde_json::to_value(&archive.artifact).map_err(|_| ArtifactError::Storage)?;
            card["url"] = json!(archive.url);
            card["download_url"] = json!(archive.download_url);
            card["preview_url"] = Value::Null;
            card["kind"] = json!("file");
            card["project"] = json!({"file_count":count,"immutable":true});
            return Ok(card);
        };
        let mut project = Project {
            version: 1,
            entrypoint: entrypoint.into(),
            members: BTreeMap::new(),
        };
        for (path, file) in sources {
            let filename = path
                .rsplit('/')
                .next()
                .ok_or(ArtifactError::Invalid)?
                .to_owned();
            let artifact = self.publish_file(owner, session, run, &filename, file)?;
            project.members.insert(
                path,
                Member {
                    id: artifact.artifact.id,
                    filename,
                },
            );
        }
        let mut manifest = tempfile::tempfile().map_err(|_| ArtifactError::Storage)?;
        manifest
            .write_all(&serde_json::to_vec(&project).map_err(|_| ArtifactError::Invalid)?)
            .map_err(|_| ArtifactError::Storage)?;
        manifest
            .seek(SeekFrom::Start(0))
            .map_err(|_| ArtifactError::Storage)?;
        let manifest = self.publish_file(owner, session, run, MANIFEST, manifest)?;
        let signature = self.signer.sign(&manifest.artifact.id, MANIFEST, 0);
        let mut base = self.base.clone();
        {
            let mut parts = base
                .path_segments_mut()
                .map_err(|_| ArtifactError::Invalid)?;
            parts
                .pop_if_empty()
                .push("projects")
                .push(&manifest.artifact.id)
                .push(&signature);
        }
        let mut preview = base.clone();
        {
            let mut parts = preview
                .path_segments_mut()
                .map_err(|_| ArtifactError::Invalid)?;
            for component in entrypoint.split('/') {
                parts.push(component);
            }
        }
        let archive = self.publish_file(owner, session, run, "website.zip", archive)?;
        let mut card =
            serde_json::to_value(&archive.artifact).map_err(|_| ArtifactError::Storage)?;
        card["url"] = json!(archive.url);
        card["download_url"] = json!(archive.download_url);
        card["preview_url"] = json!(preview.as_str());
        card["kind"] = json!("file");
        card["project"] = json!({"entrypoint":entrypoint,"file_count":project.members.len(),"immutable":true,"static_only":true});
        Ok(card)
    }

    pub fn open_project_signed(
        &self,
        id: &str,
        signature: &str,
        path: &str,
    ) -> Result<(StoredArtifact, File)> {
        logical_path(path)?;
        let (manifest, file) = self.open_signed(id, MANIFEST, 0, signature)?;
        if manifest.size_bytes > 512 * 1024 {
            return Err(ArtifactError::Invalid);
        }
        let mut text = Vec::new();
        file.take(512 * 1024 + 1)
            .read_to_end(&mut text)
            .map_err(|_| ArtifactError::Storage)?;
        let project: Project = serde_json::from_slice(&text).map_err(|_| ArtifactError::Invalid)?;
        if project.version != 1 || project.members.len() > 128 {
            return Err(ArtifactError::Invalid);
        }
        let member = project.members.get(path).ok_or(ArtifactError::Access)?;
        let record = self.store.delivery_artifact(&member.id)?;
        if record.run_id != manifest.run_id
            || record.filename != member.filename
            || record.size_bytes < 0
        {
            return Err(ArtifactError::Access);
        }
        let file = files::open(&self.root, &record.sha256, record.size_bytes as u64)?;
        Ok((record, file))
    }
    pub fn project_base(&self, id: &str, signature: &str) -> Result<String> {
        // Verify a bearer before it can be reflected into a CSP source.
        self.signer.verify(id, MANIFEST, 0, signature)?;
        let mut url = self.base.clone();
        url.path_segments_mut()
            .map_err(|_| ArtifactError::Invalid)?
            .pop_if_empty()
            .push("projects")
            .push(id)
            .push(signature)
            .push("");
        Ok(url.into())
    }
}
