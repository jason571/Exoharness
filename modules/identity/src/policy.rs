use crate::helpers::now_seconds;
use crate::types::AuthStore;
use crate::types::{
    AuthError, CapabilityShareGrant, DEFAULT_AGENT_PROFILE_ID, UserCapabilityPolicy,
};
use exoharness_database::OptionalExtension;
use exoharness_database::params;
use std::collections::HashMap;
impl AuthStore {
    pub fn user_agent_profile_id(&self, user_id: &str) -> Result<Option<String>, AuthError> {
        self.connection()?
            .query_row(
                "SELECT COALESCE(NULLIF(btrim(default_agent_profile_id), ''), ?2)
                 FROM auth_users WHERE id=?1",
                params![user_id, DEFAULT_AGENT_PROFILE_ID],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn user_capability_policy(
        &self,
        user_id: &str,
    ) -> Result<Option<UserCapabilityPolicy>, AuthError> {
        let connection = self.connection()?;
        if !connection
            .query_row(
                "SELECT 1::BIGINT FROM auth_users WHERE id=?1",
                params![user_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some()
        {
            return Ok(None);
        }
        let policy = connection
            .query_row(
                "SELECT install_skills, install_mcp, install_tools,
                    use_skills, use_mcp, use_tools
             FROM auth_user_capability_policy WHERE user_id=?1",
                params![user_id],
                |row| {
                    Ok(UserCapabilityPolicy {
                        install_skills: row.get::<_, i64>(0)? != 0,
                        install_mcp: row.get::<_, i64>(1)? != 0,
                        install_tools: row.get::<_, i64>(2)? != 0,
                        use_skills: row.get::<_, i64>(3)? != 0,
                        use_mcp: row.get::<_, i64>(4)? != 0,
                        use_tools: row.get::<_, i64>(5)? != 0,
                    })
                },
            )
            .optional()?
            .unwrap_or_default();
        Ok(Some(policy))
    }

    pub fn set_user_capability_policy(
        &self,
        user_id: &str,
        policy: UserCapabilityPolicy,
    ) -> Result<UserCapabilityPolicy, AuthError> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "INSERT INTO auth_user_capability_policy
                (user_id, install_skills, install_mcp, install_tools,
                 use_skills, use_mcp, use_tools, updated_at)
             SELECT id, ?2, ?3, ?4, ?5, ?6, ?7, ?8 FROM auth_users WHERE id=?1
             ON CONFLICT(user_id) DO UPDATE SET
                install_skills=excluded.install_skills,
                install_mcp=excluded.install_mcp,
                install_tools=excluded.install_tools,
                use_skills=excluded.use_skills,
                use_mcp=excluded.use_mcp,
                use_tools=excluded.use_tools,
                updated_at=excluded.updated_at",
            params![
                user_id,
                policy.install_skills as i64,
                policy.install_mcp as i64,
                policy.install_tools as i64,
                policy.use_skills as i64,
                policy.use_mcp as i64,
                policy.use_tools as i64,
                now_seconds()
            ],
        )?;
        if changed == 0 {
            return Err(AuthError::Validation("用户不存在".into()));
        }
        Ok(policy)
    }

    pub fn user_shared_capabilities(
        &self,
        user_id: &str,
    ) -> Result<HashMap<String, CapabilityShareGrant>, AuthError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT capability_id, revision, manifest_digest, source_digest
             FROM auth_user_capability_shares WHERE user_id=?1",
        )?;
        Ok(statement
            .query_map(params![user_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    CapabilityShareGrant {
                        revision: row.get(1)?,
                        manifest_digest: row.get(2)?,
                        source_digest: row.get(3)?,
                    },
                ))
            })?
            .collect::<Result<HashMap<_, _>, _>>()?)
    }

    pub fn capability_shared_users(
        &self,
        capability_id: &str,
        grant: &CapabilityShareGrant,
    ) -> Result<Vec<String>, AuthError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT user_id FROM auth_user_capability_shares
             WHERE capability_id=?1 AND revision=?2 AND manifest_digest=?3
               AND source_digest IS NOT DISTINCT FROM ?4 ORDER BY user_id",
        )?;
        Ok(statement
            .query_map(
                params![
                    capability_id,
                    grant.revision,
                    grant.manifest_digest,
                    grant.source_digest
                ],
                |row| row.get::<_, String>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn set_capability_share(
        &self,
        capability_id: &str,
        user_id: &str,
        grant: &CapabilityShareGrant,
        enabled: bool,
    ) -> Result<(), AuthError> {
        if self.user_capability_policy(user_id)?.is_none() {
            return Err(AuthError::Validation("用户不存在".into()));
        }
        let connection = self.connection()?;
        if enabled {
            connection.execute(
                "INSERT INTO auth_user_capability_shares
                    (capability_id, user_id, revision, manifest_digest, source_digest, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(capability_id, user_id)
                 DO UPDATE SET revision=excluded.revision, manifest_digest=excluded.manifest_digest,
                    source_digest=excluded.source_digest, created_at=excluded.created_at",
                params![
                    capability_id,
                    user_id,
                    grant.revision,
                    grant.manifest_digest,
                    grant.source_digest,
                    now_seconds()
                ],
            )?;
        } else {
            connection.execute(
                "DELETE FROM auth_user_capability_shares WHERE capability_id=?1 AND user_id=?2",
                params![capability_id, user_id],
            )?;
        }
        Ok(())
    }

    pub fn set_user_agent_profile_id(
        &self,
        user_id: &str,
        profile_id: &str,
    ) -> Result<String, AuthError> {
        let profile_id = profile_id.trim();
        if profile_id.is_empty()
            || profile_id.len() > 64
            || !profile_id.bytes().enumerate().all(|(index, byte)| {
                (index == 0 && byte.is_ascii_lowercase())
                    || (index > 0
                        && (byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'))
            })
        {
            return Err(AuthError::Validation(
                "助理 ID 必须匹配 [a-z][a-z0-9-]{0,63}".into(),
            ));
        }
        let updated = self.connection()?.execute(
            "UPDATE auth_users SET default_agent_profile_id=?2, updated_at=?3 WHERE id=?1",
            params![user_id, profile_id, now_seconds()],
        )?;
        if updated != 1 {
            return Err(AuthError::Validation("用户不存在".into()));
        }
        Ok(profile_id.to_owned())
    }
}
