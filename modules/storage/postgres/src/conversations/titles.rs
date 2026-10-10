// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Session naming is a local projection of the first user message, not another
//! model request. Explicit user names always take precedence.
use crate::{Result, codec};
use postgres::Transaction;

pub(crate) fn first_turn(
    tx: &mut Transaction<'_>,
    owner: &str,
    session: &str,
    prompt: &str,
) -> Result<()> {
    let normalized = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = normalized.chars();
    let mut title = chars.by_ref().take(60).collect::<String>();
    if chars.next().is_some() {
        title.push('…');
    }
    tx.execute("UPDATE execution_sessions SET title=$3,title_source='auto',updated_at=$4 WHERE id=$1 AND owner_user_id=$2 AND title_source='placeholder'", &[&session,&owner,&title,&codec::now_ms()?])?;
    Ok(())
}
