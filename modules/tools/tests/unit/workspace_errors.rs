use super::*;
use exoharness_sandbox::SandboxError;
#[test]
fn missing_owned_import_is_correctable_and_storage_failure_is_not_authorization() {
    use exoharness_artifacts::ArtifactError;
    use exoharness_storage_postgres::StoreError;
    // Owner-scoped lookups return the same NotFound for absent and foreign files.
    assert!(matches!(
        import_artifact_error(ArtifactError::Store(StoreError::NotFound)),
        ToolError::CorrectableInput(message) if message.contains("No files were changed")
            && message.contains("external task ID is not an artifact ID")
    ));
    for error in [ArtifactError::Storage, ArtifactError::Access, ArtifactError::Invalid] {
        assert!(matches!(import_artifact_error(error), ToolError::Storage));
    }
}
#[test]
fn only_confirmed_unchanged_atomic_edit_errors_are_correctable() {
    for error in [
        SandboxError::AmbiguousPatch,
        SandboxError::StaleRevision,
        SandboxError::PatchRejected("no matching hunk".into()),
    ] {
        assert!(
            matches!(edit_error(error),ToolError::CorrectableInput(message) if message.contains("No files were changed"))
        );
    }
    for error in [
        SandboxError::Invalid,
        SandboxError::WorkspaceBusy,
        SandboxError::Storage,
        SandboxError::RuntimeMismatch,
        SandboxError::Unavailable,
    ] {
        assert!(!matches!(edit_error(error), ToolError::CorrectableInput(_)));
    }
    for error in [SandboxError::OutcomeUnknown, SandboxError::CleanupPending] {
        assert!(matches!(edit_error(error), ToolError::Unknown));
    }
}
