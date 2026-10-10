//! Artifact metadata uses the existing execution tables, never image Base64.
use crate::{PostgresStore, Result, StoreError, codec::now_ms, journal::lock_session};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredArtifact {
    pub id: String,
    pub run_id: String,
    pub filename: String,
    pub content_type: String,
    pub relative_path: String,
    pub size_bytes: i64,
    pub sha256: String,
}
fn row(row: &postgres::Row) -> StoredArtifact {
    StoredArtifact {
        id: row.get(0),
        run_id: row.get(1),
        filename: row.get(2),
        content_type: row.get(3),
        relative_path: row.get(4),
        size_bytes: row.get(5),
        sha256: row.get(6),
    }
}
const SELECT: &str = "SELECT a.id,a.run_id,a.filename,a.content_type,f.relative_path,f.size_bytes,f.sha256 FROM execution_run_artifacts a JOIN execution_artifact_files f ON f.artifact_id=a.id WHERE a.id=$1";
impl PostgresStore {
    /// Source evidence for a delivered artifact, without exposing original arguments.
    /// Only completed calls in its originating Run can establish this association.
    pub fn artifact_source(
        &self,
        artifact: &StoredArtifact,
    ) -> Result<Option<(String, serde_json::Value)>> {
        let row = self.connection()?.query_opt(
            "SELECT id,arguments_json FROM execution_invocations WHERE first_run_id=$1 AND status='completed' AND outcome_status='success' AND result_json::jsonb->>'artifact_id'=$2 ORDER BY completed_at DESC LIMIT 1",
            &[&artifact.run_id, &artifact.id],
        )?;
        row.map(|row| {
            let arguments: String = row.get(1);
            Ok((
                row.get(0),
                serde_json::from_str(&arguments)
                    .map_err(|_| StoreError::Invalid("invalid source evidence"))?,
            ))
        })
        .transpose()
    }
    /// Authenticated internal reuse, never possession of an arbitrary URL.
    pub fn owned_artifact(&self, owner: &str, id: &str) -> Result<StoredArtifact> {
        let mut connection = self.connection()?;
        let query = format!(
            "{SELECT} AND EXISTS(SELECT 1 FROM execution_runs r JOIN execution_sessions s ON s.id=r.session_id WHERE r.id=a.run_id AND s.owner_user_id=$2 AND s.archived=0)"
        );
        connection
            .query_opt(&query, &[&id, &owner])?
            .as_ref()
            .map(row)
            .ok_or(StoreError::NotFound)
    }
    pub fn save_artifact(
        &self,
        owner: &str,
        session: &str,
        artifact: &StoredArtifact,
    ) -> Result<StoredArtifact> {
        if artifact.size_bytes < 0
            || artifact.size_bytes > 8 * 1024 * 1024 * 1024
            || artifact.sha256.len() != 64
            || artifact.relative_path != format!("objects/{}.bin", artifact.sha256)
        {
            return Err(StoreError::Invalid("invalid artifact file proof"));
        }
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        lock_session(&mut tx, owner, session)?;
        if tx
            .query_opt(
                "SELECT id FROM execution_runs WHERE id=$1 AND session_id=$2 FOR UPDATE",
                &[&artifact.run_id, &session],
            )?
            .is_none()
        {
            return Err(StoreError::NotFound);
        }
        let source = format!("urn:exoharness:artifact:{}", artifact.id);
        tx.execute("INSERT INTO execution_run_artifacts(id,run_id,source_url,filename,content_type,created_at) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(id) DO NOTHING",&[&artifact.id,&artifact.run_id,&source,&artifact.filename,&artifact.content_type,&now_ms()?])?;
        tx.execute("INSERT INTO execution_artifact_files(artifact_id,relative_path,size_bytes,sha256) VALUES($1,$2,$3,$4) ON CONFLICT(artifact_id) DO NOTHING",&[&artifact.id,&artifact.relative_path,&artifact.size_bytes,&artifact.sha256])?;
        let saved = row(&tx.query_one(SELECT, &[&artifact.id])?);
        if &saved != artifact {
            return Err(StoreError::Conflict);
        }
        tx.commit()?;
        Ok(saved)
    }
    /// Used only after a signed delivery token was verified by Artifacts.
    pub fn delivery_artifact(&self, id: &str) -> Result<StoredArtifact> {
        self.connection()?
            .query_opt(SELECT, &[&id])?
            .as_ref()
            .map(row)
            .ok_or(StoreError::NotFound)
    }
}
