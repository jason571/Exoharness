// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{AuthError, AuthStore, DEFAULT_AGENT_PROFILE_ID, UserAccount};
use exoharness_database::params;
impl AuthStore {
    /// Filter before paging; literal matching never treats '%' or '_' as wildcards.
    pub fn user_directory(
        &self,
        search: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<(Vec<(UserAccount, String)>, i64), AuthError> {
        if search.chars().count() > 128
            || !(1..=100).contains(&limit)
            || after.is_some_and(|v| v.len() > 128)
        {
            return Err(AuthError::Validation("用户列表筛选条件无效".into()));
        }
        let search = search.to_ascii_lowercase();
        let transaction = self.connection()?.transaction()?;
        transaction.execute_batch("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")?;
        let total=transaction.query_row("SELECT COUNT(*) FROM auth_users WHERE strpos(lower(username || ' ' || display_name || ' ' || COALESCE(email,'')),?1)>0",params![search],|row|row.get::<_,i64>(0))?;
        let mut statement=transaction.prepare("SELECT id,username,COALESCE(email,''),display_name,created_at,updated_at,COALESCE(NULLIF(btrim(default_agent_profile_id),''),?1) FROM auth_users WHERE strpos(lower(username || ' ' || display_name || ' ' || COALESCE(email,'')),?2)>0 AND (?3::TEXT IS NULL OR id>?3) ORDER BY id LIMIT ?4")?;
        let users = statement
            .query_map(
                params![DEFAULT_AGENT_PROFILE_ID, search, after, limit as i64],
                |row| {
                    Ok((
                        UserAccount {
                            id: row.get(0)?,
                            username: row.get(1)?,
                            email: row.get(2)?,
                            display_name: row.get(3)?,
                            created_at: row.get(4)?,
                            updated_at: row.get(5)?,
                        },
                        row.get(6)?,
                    ))
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        transaction.commit()?;
        Ok((users, total))
    }
}
