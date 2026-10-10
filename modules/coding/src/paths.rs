/// Portable logical workspace path, NOT a resolved or trusted host path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePath(String);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("path must be a nonempty relative workspace path without traversal")]
pub struct InvalidPath;

impl WorkspacePath {
    pub fn parse(input: &str) -> Result<Self, InvalidPath> {
        if input.is_empty()
            || input.contains(['\\', ':', '\0'])
            || input.chars().any(char::is_control)
            || input
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(InvalidPath);
        }
        Ok(Self(input.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
