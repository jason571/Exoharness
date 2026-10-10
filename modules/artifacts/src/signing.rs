use crate::{ArtifactError, Result};
use ring::hmac;
use std::{fs, io::Write, path::Path};
pub(crate) struct Signer(hmac::Key);
impl Signer {
    pub fn open(root: &Path) -> Result<Self> {
        let path = root.join("delivery-key");
        if !path.try_exists().map_err(|_| ArtifactError::Storage)? {
            let mut bytes = [0u8; 32];
            getrandom::fill(&mut bytes).map_err(|_| ArtifactError::Storage)?;
            let mut staged =
                tempfile::NamedTempFile::new_in(root).map_err(|_| ArtifactError::Storage)?;
            staged
                .write_all(&bytes)
                .and_then(|_| staged.as_file().sync_all())
                .map_err(|_| ArtifactError::Storage)?;
            match staged.persist_noclobber(&path) {
                Ok(_) => (),
                Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(_) => return Err(ArtifactError::Storage),
            };
            fs::File::open(root)
                .and_then(|f| f.sync_all())
                .map_err(|_| ArtifactError::Storage)?;
        }
        let meta = fs::symlink_metadata(&path).map_err(|_| ArtifactError::Storage)?;
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len() != 32 {
            return Err(ArtifactError::Storage);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o077 != 0 {
                return Err(ArtifactError::Storage);
            }
        }
        let bytes = fs::read(path).map_err(|_| ArtifactError::Storage)?;
        Ok(Self(hmac::Key::new(hmac::HMAC_SHA256, &bytes)))
    }
    fn message(id: &str, name: &str, expires: u64) -> Vec<u8> {
        serde_json::to_vec(&("artifact-delivery-v1", id, name, expires)).expect("string tuple")
    }
    pub fn sign(&self, id: &str, name: &str, expires: u64) -> String {
        hmac::sign(&self.0, &Self::message(id, name, expires))
            .as_ref()
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect()
    }
    pub fn verify(&self, id: &str, name: &str, expires: u64, signature: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| ArtifactError::Access)?
            .as_secs();
        if signature.len() != 64
            || !signature.bytes().all(|c| c.is_ascii_hexdigit())
            || (expires != 0 && now >= expires)
        {
            return Err(ArtifactError::Access);
        }
        let bytes = (0..64)
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&signature[i..i + 2], 16).map_err(|_| ArtifactError::Access)
            })
            .collect::<Result<Vec<_>>>()?;
        hmac::verify(&self.0, &Self::message(id, name, expires), &bytes)
            .map_err(|_| ArtifactError::Access)
    }
}
