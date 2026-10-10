// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use exoharness_storage_postgres::StoredArtifact;
use serde::Serialize;
#[derive(Serialize)]
pub struct PublishedArtifact {
    pub artifact: StoredArtifact,
    pub url: String,
    pub download_url: String,
    pub preview_url: Option<String>,
}
#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    #[error("文件名、大小或媒体地址不符合交付要求")]
    Invalid,
    #[error("文件链接无效或已过期")]
    Access,
    #[error("交付文件存储暂不可用或内容已变化")]
    Storage,
    #[error("{0}")]
    Store(#[from] exoharness_storage_postgres::StoreError),
}
