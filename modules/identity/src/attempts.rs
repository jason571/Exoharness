// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Persisted admission limits; process restart cannot reset a brute-force window.
use crate::{AuthError, AuthStore, helpers::now_seconds};
use exoharness_database::params;
use sha2::{Digest, Sha256};
impl AuthStore {
    pub fn reserve_login_attempt(&self, bucket: &str) -> Result<(), AuthError> {
        let digest = format!("{:x}", Sha256::digest(bucket.as_bytes()));
        let now = now_seconds();
        let transaction = self.connection()?.transaction()?;
        transaction.execute(
            "SELECT pg_advisory_xact_lock(hashtext(?1))",
            params![digest],
        )?;
        transaction.execute(
            "DELETE FROM auth_login_attempts WHERE window_start < ?1",
            [now - 300],
        )?;
        let (attempts,start) = transaction.query_row("INSERT INTO auth_login_attempts(bucket,window_start,attempts) VALUES(?1,?2,1) ON CONFLICT(bucket) DO UPDATE SET attempts=auth_login_attempts.attempts+1 RETURNING attempts,window_start",params![digest,now],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?)))?;
        transaction.commit()?;
        if attempts > 10 {
            return Err(AuthError::RateLimited {
                message: "登录尝试过于频繁，请稍后再试".into(),
                retry_after: (start + 300 - now).max(1),
            });
        }
        Ok(())
    }
}
