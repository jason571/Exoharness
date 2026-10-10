//! Atomic admission across processes using durable Run and invocation states.
use crate::{
    PostgresStore, Result, StoreError,
    codec::{encoded, now_ms, number},
};
use exoharness_contracts::scheduler::SchedulerConfiguration;
use postgres::Transaction;
use serde_json::{Value, json};
pub(crate) fn lock(tx: &mut Transaction<'_>) -> Result<()> {
    tx.query_one("SELECT pg_advisory_xact_lock(hashtextextended(current_schema() || ':scheduler', 4271072042))", &[])?;
    Ok(())
}
pub(crate) fn configuration(tx: &mut Transaction<'_>) -> Result<SchedulerConfiguration> {
    let row = tx.query_one(
        "SELECT revision,max_active_runs_per_user,max_global_agent_runs,
        resource_pools_json,mcp_resource_assignments_json,updated_at
        FROM execution_scheduler_configuration WHERE singleton_id=1",
        &[],
    )?;
    let config = SchedulerConfiguration {
        revision: u64::try_from(row.get::<_, i64>(0)).map_err(|_| StoreError::Decode)?,
        max_active_runs_per_user: u32::try_from(row.get::<_, i64>(1))
            .map_err(|_| StoreError::Decode)?,
        max_global_agent_runs: u32::try_from(row.get::<_, i64>(2))
            .map_err(|_| StoreError::Decode)?,
        resource_pools: serde_json::from_str(&row.get::<_, String>(3))?,
        mcp_resource_assignments: serde_json::from_str(&row.get::<_, String>(4))?,
        updated_at: Some(row.get(5)),
    };
    config.validate().map_err(|_| StoreError::Decode)?;
    Ok(config)
}
pub(crate) fn run_capacity(tx: &mut Transaction<'_>, owner: &str) -> Result<bool> {
    lock(tx)?;
    let config = configuration(tx)?;
    let row = tx.query_one(
        "SELECT COUNT(*), COUNT(*) FILTER(WHERE s.owner_user_id=$1)
        FROM execution_runs r JOIN execution_sessions s ON s.id=r.session_id
        WHERE r.status IN ('running','waiting_authorization')",
        &[&owner],
    )?;
    Ok(
        row.get::<_, i64>(0) < i64::from(config.max_global_agent_runs)
            && row.get::<_, i64>(1) < i64::from(config.max_active_runs_per_user),
    )
}
fn pool_usage(
    tx: &mut Transaction<'_>,
    config: &SchedulerConfiguration,
    pool: &str,
) -> Result<(i64, i64)> {
    // Remapping cannot detach an in-flight reservation. Newly assigned active
    // calls also occupy the new pool until an authoritative receipt completes.
    let members: Vec<String> = config
        .mcp_resource_assignments
        .iter()
        .filter(|(_, p)| p.as_str() == pool)
        .map(|(id, _)| id.clone())
        .collect();
    let row = tx.query_one("SELECT COUNT(*), COUNT(*) FILTER(WHERE i.outcome_status='unknown')
        FROM execution_invocations i LEFT JOIN execution_resource_reservations p ON p.invocation_id=i.id
        WHERE i.status='submitted' AND (p.pool_id=$1 OR i.capability_id=ANY($2))", &[&pool, &members])?;
    Ok((row.get(0), row.get(1)))
}
pub(crate) fn pool_capacity(
    tx: &mut Transaction<'_>,
    capability: &str,
    invocation: &str,
) -> Result<bool> {
    lock(tx)?;
    let config = configuration(tx)?;
    let Some(pool_id) = config.mcp_resource_assignments.get(capability) else {
        return Ok(true);
    };
    let pool = config
        .resource_pools
        .iter()
        .find(|p| &p.id == pool_id)
        .ok_or(StoreError::Decode)?;
    let (active, unknown) = pool_usage(tx, &config, pool_id)?;
    if active >= i64::from(pool.max_concurrency) {
        if unknown > 0 {
            return Err(StoreError::OutcomeUnknown);
        }
        return Ok(false);
    }
    let members: Vec<String> = config
        .mcp_resource_assignments
        .iter()
        .filter(|(_, p)| *p == pool_id)
        .map(|(id, _)| id.clone())
        .collect();
    let ahead: i64 = tx
        .query_one(
            "SELECT COUNT(*) FROM execution_invocations q
        JOIN execution_runs r ON r.id=q.latest_run_id JOIN execution_invocations mine ON mine.id=$2
        WHERE q.capability_id=ANY($1) AND q.status='prepared' AND r.status='running'
        AND (q.created_at,q.id)<(mine.created_at,mine.id)",
            &[&members, &invocation],
        )?
        .get(0);
    Ok(ahead < i64::from(pool.max_concurrency) - active)
}
pub(crate) fn reserve_pool(
    tx: &mut Transaction<'_>,
    capability: &str,
    invocation: &str,
) -> Result<()> {
    if let Some(pool) = configuration(tx)?.mcp_resource_assignments.get(capability) {
        tx.execute(
            "INSERT INTO execution_resource_reservations(invocation_id,pool_id) VALUES($1,$2)
            ON CONFLICT(invocation_id) DO NOTHING",
            &[&invocation, pool],
        )?;
    }
    Ok(())
}
impl PostgresStore {
    pub fn scheduler_configuration(&self) -> Result<SchedulerConfiguration> {
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        let config = configuration(&mut tx)?;
        tx.commit()?;
        Ok(config)
    }
    pub fn save_scheduler_configuration(
        &self,
        mut next: SchedulerConfiguration,
    ) -> Result<SchedulerConfiguration> {
        next.validate().map_err(StoreError::Invalid)?;
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        lock(&mut tx)?;
        if configuration(&mut tx)?.revision != next.revision {
            return Err(StoreError::Conflict);
        }
        next.revision = next.revision.checked_add(1).ok_or(StoreError::Conflict)?;
        next.updated_at = Some(now_ms()?);
        tx.execute("UPDATE execution_scheduler_configuration SET revision=$1,max_active_runs_per_user=$2,
            max_global_agent_runs=$3,resource_pools_json=$4,mcp_resource_assignments_json=$5,updated_at=$6
            WHERE singleton_id=1", &[&number(next.revision)?,&i64::from(next.max_active_runs_per_user),
            &i64::from(next.max_global_agent_runs),&encoded(&next.resource_pools)?,
            &encoded(&next.mcp_resource_assignments)?,&next.updated_at])?;
        tx.commit()?;
        Ok(next)
    }
    pub fn scheduler_status(&self) -> Result<Value> {
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        lock(&mut tx)?;
        let config = configuration(&mut tx)?;
        let counts = tx.query_one(
            "SELECT COUNT(*) FILTER(WHERE status IN ('running','waiting_authorization')),
            COUNT(*) FILTER(WHERE status IN ('queued','pending')) FROM execution_runs",
            &[],
        )?;
        let mut pools = vec![];
        for pool in &config.resource_pools {
            let (active, unknown) = pool_usage(&mut tx, &config, &pool.id)?;
            pools.push(json!({"id":pool.id,"name":pool.name,"active":active,"unknown":unknown,"limit":pool.max_concurrency}));
        }
        tx.commit()?;
        Ok(
            json!({"ready":true,"active":counts.get::<_, i64>(0),"waiting":counts.get::<_, i64>(1),
            "global_limit":config.max_global_agent_runs,"per_user_limit":config.max_active_runs_per_user,
            "revision":config.revision,"pools":pools}),
        )
    }
}
