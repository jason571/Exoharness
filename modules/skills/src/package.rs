use crate::{
    ImportRequest, Inspection, Result,
    errors::validation,
    metadata,
    types::{FileMetadata, MAX_FILE_BYTES, MAX_INSTRUCTION_BYTES, MAX_TOTAL_BYTES, PackageFiles},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use exoharness_contracts::capability::{
    CapabilityManifest, CapabilitySource, CapabilitySpec, MANIFEST_SCHEMA_VERSION, SkillSpec,
    ValidatedCapability,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub fn decode(request: &ImportRequest) -> Result<Inspection> {
    if request
        .package_kind
        .as_deref()
        .is_some_and(|kind| kind != "skill")
    {
        return Err(validation("请选择 Skill 文件、目录或 ZIP"));
    }
    let mut files = match (&request.archive_base64, request.files.is_empty()) {
        (Some(archive), true) => {
            crate::archive::read_zip(&decode_bytes(archive, MAX_TOTAL_BYTES)?)?
        }
        (None, false) => {
            if request.files.len() > crate::types::MAX_FILES {
                return Err(validation("Skill 文件数量超过限制"));
            }
            let mut total = 0usize;
            let raw = request
                .files
                .iter()
                .map(|file| {
                    let bytes = decode_bytes(&file.content_base64, MAX_FILE_BYTES)?;
                    total = total
                        .checked_add(bytes.len())
                        .ok_or_else(|| validation("Skill 包过大"))?;
                    if total > MAX_TOTAL_BYTES {
                        return Err(validation("Skill 包超过32 MiB"));
                    }
                    Ok((file.path.clone(), bytes))
                })
                .collect::<Result<Vec<_>>>()?;
            crate::normalization::normalize_files(raw)?
        }
        _ => return Err(validation("文件和 ZIP 只能选择一种，不能同时为空")),
    };
    if !files.contains_key(Path::new("SKILL.md")) {
        // A standalone Markdown file is a supported import, not a server path.
        if files.len() == 1
            && files.keys().next().is_some_and(|p| {
                p.components().count() == 1
                    && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("md"))
            })
        {
            let (_, bytes) = files.pop_first().expect("one file");
            files.insert(PathBuf::from("SKILL.md"), bytes);
        } else {
            files = crate::normalization::without_common_root(files)
                .unwrap_or_else(|original| original);
        }
    }
    let instructions = files
        .get(Path::new("SKILL.md"))
        .ok_or_else(|| validation("所选 Skill 目录的根目录需要包含 SKILL.md"))?;
    if instructions.len() > MAX_INSTRUCTION_BYTES {
        return Err(validation("SKILL.md 超过1 MiB"));
    }
    let text = std::str::from_utf8(instructions)
        .map_err(|_| validation("SKILL.md 必须使用 UTF-8 编码"))?;
    let (name, description) = metadata::identity(text)?;
    let file_name = request.file_name.clone().unwrap_or_else(|| {
        request
            .files
            .first()
            .and_then(|f| f.path.split('/').next())
            .unwrap_or("SKILL.md")
            .into()
    });
    let manifest = CapabilityManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        name,
        description,
        version: request.version.clone(),
        scope: request.scope.clone(),
        source: CapabilitySource::Uploaded { file_name },
        permissions: request.permissions.clone(),
        secret_refs: vec![],
        spec: CapabilitySpec::Skill(SkillSpec {
            entrypoint: "SKILL.md".into(),
            content_root: ".".into(),
            executable_scripts: false,
        }),
    };
    let source_digest = digest(&files);
    let checked = ValidatedCapability::new(manifest, Some(source_digest.clone()))
        .map_err(|e| validation(e.to_string()))?;
    let entries = files
        .iter()
        .map(|(path, bytes)| FileMetadata {
            path: path.clone(),
            size: bytes.len(),
            digest: format!("{:x}", Sha256::digest(bytes)),
        })
        .collect();
    Ok(Inspection {
        manifest: checked.manifest().clone(),
        source_digest,
        files: entries,
        content: files,
    })
}
fn decode_bytes(encoded: &str, limit: usize) -> Result<Vec<u8>> {
    if encoded.len() > limit.div_ceil(3) * 4 {
        return Err(validation("上传文件超过大小限制"));
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| validation("上传文件编码无效"))?;
    if bytes.len() > limit {
        return Err(validation("上传文件超过大小限制"));
    }
    Ok(bytes)
}
pub(crate) fn digest(files: &PackageFiles) -> String {
    let mut hash = Sha256::new();
    for (path, bytes) in files {
        let path = path.to_string_lossy();
        hash.update((path.len() as u64).to_be_bytes());
        hash.update(path.as_bytes());
        hash.update((bytes.len() as u64).to_be_bytes());
        hash.update(bytes);
    }
    format!("{:x}", hash.finalize())
}
