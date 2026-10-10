use crate::{Receipt, Result, SandboxError, Workspace};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};
pub(crate) fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
pub(crate) fn private(path: &Path) -> Result<()> {
    if !path.try_exists().map_err(|_| SandboxError::Storage)? {
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(path).map_err(|_| SandboxError::Storage)?;
    }
    let meta = fs::symlink_metadata(path).map_err(|_| SandboxError::Storage)?;
    if !meta.is_dir()
        || meta.file_type().is_symlink()
        || fs::canonicalize(path).map_err(|_| SandboxError::Storage)? != path
    {
        return Err(SandboxError::Invalid);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(SandboxError::Invalid);
        }
    }
    Ok(())
}
pub(crate) fn relative(path: &str) -> Result<PathBuf> {
    if path.is_empty()
        || path.len() > 4096
        || path
            .chars()
            .any(|c| c.is_control() || c == '\\' || c == ':')
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err(SandboxError::Invalid);
    }
    let path = PathBuf::from(path);
    if path
        .components()
        .any(|p| !matches!(p, Component::Normal(_)))
    {
        return Err(SandboxError::Invalid);
    }
    Ok(path)
}
pub(crate) fn identity(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(SandboxError::Invalid);
    }
    Ok(())
}
/// An OS lease is released on process death. A PID file is not sufficient:
/// two providers must never reconcile or execute in the same namespace.
pub(crate) fn lease(root: &Path) -> Result<fs::File> {
    use fs2::FileExt;
    let path = root.join("provider.lock");
    if path.try_exists().map_err(|_| SandboxError::Storage)? {
        private_file(&path)?;
    }
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(&path).map_err(|_| SandboxError::Storage)?;
    private_file(&path)?;
    file.try_lock_exclusive()
        .map_err(|_| SandboxError::Unavailable)?;
    Ok(file)
}
fn private_file(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path).map_err(|_| SandboxError::Storage)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(SandboxError::Invalid);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(SandboxError::Invalid);
        }
    }
    Ok(())
}
pub(crate) fn reserved_digest(root: &Path, id: &str) -> Result<()> {
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(SandboxError::Invalid);
    }
    private_file(&root.join("jobs").join(id))
}
pub(crate) fn workspace(
    root: &Path,
    id: &str,
    files: BTreeMap<String, Vec<u8>>,
) -> Result<Workspace> {
    identity(id)?;
    if files.is_empty()
        || files.len() > 1024
        || files.values().map(Vec::len).sum::<usize>() > 32 * 1024 * 1024
    {
        return Err(SandboxError::Invalid);
    }
    let normalized = files
        .into_iter()
        .map(|(path, bytes)| {
            if bytes.len() > 4 * 1024 * 1024 {
                return Err(SandboxError::Invalid);
            }
            Ok((relative(&path)?, bytes))
        })
        .collect::<Result<Vec<_>>>()?;
    let id = digest(id);
    let parent = root.join("workspaces");
    private(&parent)?;
    let path = parent.join(&id);
    if path.try_exists().map_err(|_| SandboxError::Storage)? {
        return Err(SandboxError::OutcomeUnknown);
    }
    let mut builder = tempfile::Builder::new();
    builder.prefix(".workspace-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o700));
    }
    let staged = builder
        .tempdir_in(&parent)
        .map_err(|_| SandboxError::Storage)?;
    for (relative, bytes) in normalized {
        let mut directory = staged.path().to_path_buf();
        for part in relative.parent().unwrap_or(Path::new("")).components() {
            directory.push(part);
            private(&directory)?;
        }
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(staged.path().join(&relative))
            .map_err(|_| SandboxError::Storage)?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| SandboxError::Storage)?;
    }
    fs::rename(staged.path(), &path).map_err(|_| SandboxError::Storage)?;
    fs::File::open(&parent)
        .and_then(|f| f.sync_all())
        .map_err(|_| SandboxError::Storage)?;
    Ok(Workspace {
        root: path,
        store: root.into(),
        id,
    })
}
pub(crate) fn verify(root: &Path, workspace: &Workspace, entry: &str) -> Result<()> {
    if workspace.store != root || workspace.root != root.join("workspaces").join(&workspace.id) {
        return Err(SandboxError::Invalid);
    }
    private(root)?;
    private(&root.join("workspaces"))?;
    private(&workspace.root)?;
    let relative = relative(entry)?;
    let mut path = workspace.root.clone();
    for part in relative.components() {
        path.push(part);
        let meta = fs::symlink_metadata(&path).map_err(|_| SandboxError::Invalid)?;
        if meta.file_type().is_symlink() {
            return Err(SandboxError::Invalid);
        }
    }
    if !fs::metadata(path)
        .map_err(|_| SandboxError::Invalid)?
        .is_file()
    {
        return Err(SandboxError::Invalid);
    }
    Ok(())
}
pub(crate) fn reserve(root: &Path, id: &str) -> Result<()> {
    identity(id)?;
    let jobs = root.join("jobs");
    private(&jobs)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(jobs.join(digest(id))).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            SandboxError::OutcomeUnknown
        } else {
            SandboxError::Storage
        }
    })?;
    file.write_all(b"reserved; reconcile before resubmission\n")
        .and_then(|_| file.sync_all())
        .map_err(|_| SandboxError::Storage)?;
    fs::File::open(jobs)
        .and_then(|f| f.sync_all())
        .map_err(|_| SandboxError::Storage)?;
    Ok(())
}
pub(crate) fn save_receipt(root: &Path, receipt: &Receipt) -> Result<()> {
    let parent = root.join("jobs");
    private(&parent)?;
    let mut staged = tempfile::NamedTempFile::new_in(&parent).map_err(|_| SandboxError::Storage)?;
    serde_json::to_writer(&mut staged, receipt).map_err(|_| SandboxError::Storage)?;
    staged
        .as_file()
        .sync_all()
        .map_err(|_| SandboxError::Storage)?;
    staged
        .persist(parent.join(format!("{}.receipt.json", digest(&receipt.invocation_id))))
        .map_err(|_| SandboxError::Storage)?;
    fs::File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|_| SandboxError::Storage)?;
    Ok(())
}
pub(crate) fn receipt(root: &Path, id: &str) -> Result<Option<Receipt>> {
    use std::io::Read;
    identity(id)?;
    let parent = root.join("jobs");
    private(&parent)?;
    let path = parent.join(format!("{}.receipt.json", digest(id)));
    let meta = match fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(SandboxError::Storage),
    };
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 32 * 1024 * 1024 {
        return Err(SandboxError::Invalid);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(SandboxError::Invalid);
        }
    }
    let mut data = Vec::new();
    fs::File::open(path)
        .map_err(|_| SandboxError::Storage)?
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut data)
        .map_err(|_| SandboxError::Storage)?;
    let receipt: Receipt = serde_json::from_slice(&data).map_err(|_| SandboxError::Storage)?;
    if receipt.invocation_id != id {
        return Err(SandboxError::Invalid);
    }
    Ok(Some(receipt))
}
