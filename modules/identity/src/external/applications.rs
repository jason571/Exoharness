// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{Application, ApplicationInput, IssuedApplication, validation};
use crate::{
    AuthError, AuthStore,
    helpers::{now_seconds, token_digest},
};
use exoharness_database::{OptionalExtension, params};

impl AuthStore {
    pub fn external_application(&self, id: &str) -> Result<Option<Application>, AuthError> {
        let record = self.connection()?.query_row(
            "SELECT configuration_json, revision FROM auth_external_applications WHERE client_id=?1 AND deleted_at IS NULL",
            [id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
        ).optional()?;
        record
            .map(|(raw, revision)| {
                Ok(Application {
                    input: serde_json::from_str(&raw)
                        .map_err(|_| validation::invalid("invalid stored application"))?,
                    revision,
                })
            })
            .transpose()
    }
    pub fn external_applications(&self) -> Result<Vec<Application>, AuthError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare("SELECT configuration_json, revision FROM auth_external_applications WHERE deleted_at IS NULL ORDER BY client_id")?;
        statement
            .query_map((), |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
            .map(|row| {
                let (raw, revision) = row?;
                Ok(Application {
                    input: serde_json::from_str(&raw)
                        .map_err(|_| validation::invalid("invalid stored application"))?,
                    revision,
                })
            })
            .collect()
    }
    pub fn create_external_application(
        &self,
        input: ApplicationInput,
    ) -> Result<IssuedApplication, AuthError> {
        self.create_external_application_with_key(input, validation::token("exo_")?, String::new())
    }
    pub fn create_external_application_with_key(
        &self,
        mut input: ApplicationInput,
        secret: String,
        encrypted: String,
    ) -> Result<IssuedApplication, AuthError> {
        validation::application(&mut input)?;
        if !(32..=256).contains(&secret.len()) {
            return Err(validation::invalid("invalid application secret"));
        }
        let encoded = serde_json::to_string(&input)
            .map_err(|_| validation::invalid("invalid application"))?;
        let changed = self.connection()?.execute(
            "INSERT INTO auth_external_applications(client_id,configuration_json,secret_digest,revision,updated_at,encrypted_key) VALUES(?1,?2,?3,1,?4,?5) ON CONFLICT DO NOTHING",
            params![input.client_id, encoded, token_digest(&secret), now_seconds(),encrypted],
        )?;
        if changed != 1 {
            let deleted = self.connection()?.query_row(
                "SELECT deleted_at IS NOT NULL FROM auth_external_applications WHERE client_id=?1",
                [&input.client_id], |r| r.get::<_,bool>(0),
            ).optional()?.unwrap_or(false);
            return Err(AuthError::Conflict(if deleted {
                "deleted application ID cannot be reused".into()
            } else {
                "application already exists".into()
            }));
        }
        Ok(IssuedApplication {
            application: Application { input, revision: 1 },
            client_secret: secret,
        })
    }
    /// Configuration changes rotate the application revision and invalidate
    /// existing user grants. Secret rotation is an explicit administrator action.
    pub fn update_external_application(
        &self,
        input: ApplicationInput,
        expected: i64,
        rotate_secret: bool,
    ) -> Result<IssuedApplication, AuthError> {
        let key = if rotate_secret {
            Some((validation::token("exo_")?, String::new()))
        } else {
            None
        };
        self.update_external_application_with_key(input, expected, key)
    }
    pub fn update_external_application_with_key(
        &self,
        mut input: ApplicationInput,
        expected: i64,
        key: Option<(String, String)>,
    ) -> Result<IssuedApplication, AuthError> {
        validation::application(&mut input)?;
        let (secret, encrypted) = key.unwrap_or_default();
        if !secret.is_empty() && !(32..=256).contains(&secret.len()) {
            return Err(validation::invalid("invalid application secret"));
        }
        let encoded = serde_json::to_string(&input)
            .map_err(|_| validation::invalid("invalid application"))?;
        let next = expected
            .checked_add(1)
            .filter(|_| expected > 0)
            .ok_or_else(|| validation::invalid("invalid revision"))?;
        let changed = self.connection()?.execute(
            "UPDATE auth_external_applications SET configuration_json=?2, revision=?3, updated_at=?4, secret_digest=CASE WHEN ?5<>'' THEN ?5 ELSE secret_digest END, encrypted_key=CASE WHEN ?5<>'' THEN ?7 ELSE encrypted_key END WHERE client_id=?1 AND revision=?6 AND deleted_at IS NULL",
            params![input.client_id, encoded, next, now_seconds(), if !secret.is_empty() { token_digest(&secret) } else { String::new() }, expected,encrypted],
        )?;
        if changed != 1 {
            return Err(AuthError::Conflict("application revision changed".into()));
        }
        Ok(IssuedApplication {
            application: Application {
                input,
                revision: next,
            },
            client_secret: secret,
        })
    }
    pub fn external_encrypted_key(&self, id: &str) -> Result<Option<String>, AuthError> {
        Ok(self
            .connection()?
            .query_row(
                "SELECT encrypted_key FROM auth_external_applications WHERE client_id=?1 AND deleted_at IS NULL",
                [id],
                |r| r.get(0),
            )
            .optional()?)
    }
    pub fn audit_external_key(
        &self,
        client: &str,
        actor: &str,
        action: &str,
    ) -> Result<(), AuthError> {
        if !matches!(action, "create" | "rotate" | "view" | "delete") {
            return Err(validation::invalid("invalid key audit action"));
        }
        self.connection()?.execute("INSERT INTO auth_external_key_audit(client_id,actor_id,action,created_at) VALUES(?1,?2,?3,?4)",params![client,actor,action,now_seconds()])?;
        Ok(())
    }
    pub fn authenticate_external_application(
        &self,
        id: &str,
        secret: &str,
    ) -> Result<Option<Application>, AuthError> {
        if !validation::identifier(id) || !(32..=256).contains(&secret.len()) {
            return Ok(None);
        }
        let record = self.connection()?.query_row(
            "SELECT configuration_json,revision,secret_digest FROM auth_external_applications WHERE client_id=?1 AND deleted_at IS NULL", [id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?)),
        ).optional()?;
        let Some((raw, revision, digest)) = record else {
            return Ok(None);
        };
        let input: ApplicationInput = serde_json::from_str(&raw)
            .map_err(|_| validation::invalid("invalid stored application"))?;
        Ok(
            (input.enabled && validation::secret_matches(&token_digest(secret), &digest))
                .then_some(Application { input, revision }),
        )
    }

    /// Delete access atomically while retaining identity and execution history.
    /// The client ID remains reserved to prevent reassigning old user mappings.
    pub fn delete_external_application(
        &self,
        client: &str,
        expected: i64,
        actor: &str,
    ) -> Result<bool, AuthError> {
        let next = expected
            .checked_add(1)
            .filter(|_| expected > 0)
            .ok_or_else(|| validation::invalid("invalid revision"))?;
        let tx = self.connection()?.transaction()?;
        let record = tx.query_row(
            "SELECT configuration_json,revision FROM auth_external_applications WHERE client_id=?1 AND deleted_at IS NULL FOR UPDATE",
            [client], |r| Ok((r.get::<_,String>(0)?, r.get::<_,i64>(1)?)),
        ).optional()?;
        let Some((raw, revision)) = record else {
            return Ok(false);
        };
        if revision != expected {
            return Err(AuthError::Conflict("application revision changed".into()));
        }
        let mut input: ApplicationInput = serde_json::from_str(&raw)
            .map_err(|_| validation::invalid("invalid stored application"))?;
        input.enabled = false;
        let encoded = serde_json::to_string(&input)
            .map_err(|_| validation::invalid("invalid application"))?;
        let now = now_seconds();
        tx.execute("UPDATE auth_external_applications SET configuration_json=?2,revision=?3,updated_at=?4,deleted_at=?4,secret_digest='',encrypted_key='' WHERE client_id=?1",params![client,encoded,next,now])?;
        tx.execute(
            "UPDATE auth_external_grants SET revoked_at=COALESCE(revoked_at,?2) WHERE client_id=?1",
            params![client, now],
        )?;
        tx.execute("INSERT INTO auth_external_key_audit(client_id,actor_id,action,created_at) VALUES(?1,?2,'delete',?3)",params![client,actor,now])?;
        tx.commit()?;
        Ok(true)
    }
}
