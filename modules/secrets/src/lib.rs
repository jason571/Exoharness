// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Local secrets are private files, never serializable manifest values.
mod cipher;
mod filesystem;
pub use cipher::SecretCipher;
mod store;
mod value;
pub use store::SecretStore;
pub use value::SecretValue;
#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("密钥引用或内容无效")]
    Invalid,
    #[error("密钥目录或文件权限不安全")]
    Unsafe,
    #[error("密钥尚未配置")]
    Missing,
    #[error("密钥存储暂时不可用")]
    Storage,
}
impl From<std::io::Error> for SecretError {
    fn from(error: std::io::Error) -> Self {
        if error.kind() == std::io::ErrorKind::NotFound {
            Self::Missing
        } else {
            Self::Storage
        }
    }
}
pub type Result<T> = std::result::Result<T, SecretError>;
