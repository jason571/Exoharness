use exoharness_storage_postgres::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("{0}")]
    Validation(String),
    #[error("Skill content store is unavailable or unsafe")]
    Io,
    #[error("{0}")]
    Store(#[from] StoreError),
}
impl From<std::io::Error> for SkillError {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}
pub type Result<T> = std::result::Result<T, SkillError>;
pub(crate) fn validation(message: impl Into<String>) -> SkillError {
    SkillError::Validation(message.into())
}
