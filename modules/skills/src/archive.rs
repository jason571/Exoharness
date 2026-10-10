// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{
    Result,
    errors::validation,
    types::{MAX_FILE_BYTES, MAX_FILES, MAX_TOTAL_BYTES, PackageFiles},
};
use std::io::{Cursor, Read};
use zip::ZipArchive;
const MAX_COMPRESSION_RATIO: u64 = 200;

pub fn read_zip(bytes: &[u8]) -> Result<PackageFiles> {
    if bytes.is_empty() || bytes.len() > MAX_TOTAL_BYTES {
        return Err(validation(format!(
            "ZIP size must be in 1..={MAX_TOTAL_BYTES} bytes"
        )));
    }
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|error| validation(error.to_string()))?;
    if archive.len() > MAX_FILES {
        return Err(validation(format!(
            "package contains more than {MAX_FILES} entries"
        )));
    }

    let mut raw_files = Vec::new();
    let mut total = 0_usize;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| validation(error.to_string()))?;
        if entry.encrypted() {
            return Err(validation("encrypted ZIP entries are not accepted"));
        }
        if entry.is_symlink() {
            return Err(validation("symbolic links are not accepted in packages"));
        }
        if entry.is_dir() {
            continue;
        }
        let original_name = entry.name().to_owned();
        if original_name
            .split('/')
            .any(|part| part == "." || part == "..")
        {
            return Err(validation("ZIP contains a non-canonical relative path"));
        }
        let path = entry
            .enclosed_name()
            .ok_or_else(|| validation(format!("unsafe ZIP path {:?}", entry.name())))?;
        let path = path
            .to_str()
            .ok_or_else(|| validation("ZIP paths must be valid UTF-8"))?
            .to_owned();
        if path.contains('\\') || path.contains('\0') {
            return Err(validation(format!("unsafe ZIP path {path:?}")));
        }
        let size = usize::try_from(entry.size())
            .map_err(|_| validation(format!("ZIP entry {path:?} is too large")))?;
        if size > MAX_FILE_BYTES {
            return Err(validation(format!(
                "ZIP entry {path:?} exceeds {MAX_FILE_BYTES} bytes"
            )));
        }
        if entry.size() > 0
            && (entry.compressed_size() == 0
                || entry.size() / entry.compressed_size().max(1) > MAX_COMPRESSION_RATIO)
        {
            return Err(validation(format!(
                "ZIP entry {path:?} exceeds the compression ratio limit"
            )));
        }
        total = total
            .checked_add(size)
            .ok_or_else(|| validation("expanded ZIP is too large"))?;
        if total > MAX_TOTAL_BYTES {
            return Err(validation(format!(
                "expanded ZIP exceeds {MAX_TOTAL_BYTES} bytes"
            )));
        }
        let mut content = Vec::with_capacity(size);
        entry
            .by_ref()
            .take((MAX_FILE_BYTES + 1) as u64)
            .read_to_end(&mut content)?;
        if content.len() != size || content.len() > MAX_FILE_BYTES {
            return Err(validation(format!(
                "ZIP entry {path:?} size does not match its metadata"
            )));
        }
        raw_files.push((path, content));
    }
    super::normalization::normalize_files(raw_files)
}
