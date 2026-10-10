//! Private content-addressed packages. Files are durable before registry commit.
use crate::{
    Inspection, Result,
    errors::validation,
    package::digest,
    types::{MAX_FILE_BYTES, MAX_FILES, MAX_TOTAL_BYTES, PackageFiles},
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub(crate) struct Packages {
    root: PathBuf,
}
mod removal;
impl Packages {
    pub fn open(root: PathBuf) -> Result<Self> {
        directory(&root)?;
        directory(&root.join("packages"))?;
        directory(&root.join("staging"))?;
        directory(&root.join("removing"))?;
        Ok(Self { root })
    }
    fn path(&self, id: &str, hash: &str) -> Result<PathBuf> {
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(validation("Skill 内容摘要无效"));
        }
        Ok(self
            .root
            .join("packages")
            .join(format!("{:x}", Sha256::digest(id.as_bytes())))
            .join(hash))
    }
    pub fn install(&self, inspected: &Inspection) -> Result<()> {
        let id = inspected.manifest.id();
        let destination = self.path(&id, &inspected.source_digest)?;
        directory(destination.parent().expect("package parent"))?;
        if destination.try_exists()? {
            self.read(&id, &inspected.source_digest)?;
            return Ok(());
        }
        let staging = temporary(&self.root.join("staging"), "skill-")?;
        for (path, bytes) in &inspected.content {
            let mut parent = staging.path().to_path_buf();
            for part in path.parent().unwrap_or(Path::new("")).components() {
                parent.push(part);
                directory(&parent)?;
            }
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o400);
            }
            let mut file = options.open(staging.path().join(path))?;
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        sync_tree(staging.path())?;
        match fs::rename(staging.path(), &destination) {
            Ok(()) => {
                fs::File::open(destination.parent().expect("parent"))?.sync_all()?;
            }
            Err(error) => {
                if !destination.try_exists()? {
                    return Err(error.into());
                }
                self.read(&id, &inspected.source_digest)?;
            }
        }
        Ok(())
    }
    pub fn read(&self, id: &str, expected: &str) -> Result<PackageFiles> {
        let path = self.path(id, expected)?;
        real_directory(&self.root)?;
        real_directory(&self.root.join("packages"))?;
        real_directory(path.parent().expect("parent"))?;
        real_directory(&path)?;
        let mut files = PackageFiles::new();
        let mut total = 0;
        collect(&path, &path, &mut files, &mut total, 0)?;
        if digest(&files) != expected {
            return Err(validation("Skill 文件内容已变化，未使用损坏的版本"));
        }
        Ok(files)
    }
}
fn directory(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => real_directory(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(path) {
                Ok(()) => real_directory(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    real_directory(path)
                }
                Err(error) => Err(error.into()),
            }
        }
        Err(error) => Err(error.into()),
    }
}
fn temporary(parent: &Path, prefix: &str) -> Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(prefix);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o700));
    }
    Ok(builder.tempdir_in(parent)?)
}
fn real_directory(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(validation("Skill 内容目录不安全"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(validation("Skill 内容目录需要私有权限0700"));
        }
    }
    Ok(())
}
fn collect(
    root: &Path,
    dir: &Path,
    files: &mut PackageFiles,
    total: &mut usize,
    depth: usize,
) -> Result<()> {
    if depth > 64 {
        return Err(validation("Skill 目录嵌套过深"));
    }
    for item in fs::read_dir(dir)? {
        let path = item?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(validation("Skill 文件不能是符号链接"));
        }
        if metadata.is_dir() {
            real_directory(&path)?;
            collect(root, &path, files, total, depth + 1)?;
            continue;
        }
        if !metadata.is_file() || files.len() >= MAX_FILES || metadata.len() > MAX_FILE_BYTES as u64
        {
            return Err(validation("Skill 文件类型、数量或大小无效"));
        }
        let mut content = Vec::new();
        fs::File::open(&path)?
            .take((MAX_FILE_BYTES + 1) as u64)
            .read_to_end(&mut content)?;
        *total = total
            .checked_add(content.len())
            .ok_or_else(|| validation("Skill 包过大"))?;
        if content.len() > MAX_FILE_BYTES || *total > MAX_TOTAL_BYTES {
            return Err(validation("Skill 文件超过限制"));
        }
        files.insert(
            path.strip_prefix(root)
                .map_err(|_| validation("路径超出 Skill 包"))?
                .into(),
            content,
        );
    }
    Ok(())
}
fn sync_tree(path: &Path) -> Result<()> {
    for item in fs::read_dir(path)? {
        let path = item?.path();
        if path.is_dir() {
            sync_tree(&path)?;
        }
    }
    fs::File::open(path)?.sync_all()?;
    Ok(())
}
