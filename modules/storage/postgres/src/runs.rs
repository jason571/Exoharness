use crate::codec::{digest, now_ms, number, parsed, value};
use crate::journal::{append, lock_session};
use crate::{
    ExecutionEvent, ExecutionSnapshot, PostgresStore, RecoveryAction, RecoveryDecision, Result,
    RunRecord, RunState, StoreError,
};
use postgres::Transaction;
use serde_json::Value;

pub(crate) fn status(state: RunState) -> &'static str {
    match state {
        RunState::Queued => "queued",
        RunState::Running => "running",
        RunState::AwaitingApproval => "waiting_authorization",
        RunState::Paused => "recoverable",
        RunState::Succeeded => "succeeded",
        RunState::Failed => "failed",
        RunState::Cancelled => "cancelled",
    }
}
fn state(label: &str) -> Result<RunState> {
    match label {
        "queued" | "pending" => Ok(RunState::Queued),
        "running" => Ok(RunState::Running),
        "waiting_authorization" => Ok(RunState::AwaitingApproval),
        "recoverable" => Ok(RunState::Paused),
        "succeeded" => Ok(RunState::Succeeded),
        "failed" => Ok(RunState::Failed),
        "cancelled" => Ok(RunState::Cancelled),
        _ => Err(StoreError::Decode),
    }
}
pub(crate) fn read_run(tx: &mut Transaction<'_>, session: &str, run: &str) -> Result<RunRecord> {
    let row=tx.query_opt("SELECT r.turn_id,r.status,r.last_sequence,r.model_snapshot_json,r.capability_snapshot_json,a.profile_snapshot_json FROM execution_runs r LEFT JOIN execution_run_agent_profiles a ON a.run_id=r.id WHERE r.id=$1 AND r.session_id=$2 FOR UPDATE OF r",&[&run,&session])?.ok_or(StoreError::NotFound)?;
    let control = super::control::load(tx, run)?;
    let policy = &control.policy;
    let profile: Value = parsed(
        &row.get::<_, Option<String>>(5)
            .ok_or(StoreError::MigrationRequired)?,
    )?;
    let snapshot = ExecutionSnapshot {
        version: serde_json::from_value(policy["version"].clone())?,
        assistant_id: serde_json::from_value(profile["id"].clone())?,
        assistant_revision: serde_json::from_value(profile["revision"].clone())?,
        model: parsed(
            &row.get::<_, Option<String>>(3)
                .ok_or(StoreError::MigrationRequired)?,
        )?,
        capabilities: parsed(
            &row.get::<_, Option<String>>(4)
                .ok_or(StoreError::MigrationRequired)?,
        )?,
        max_consecutive_identical_calls: serde_json::from_value(
            policy["max_consecutive_identical_calls"].clone(),
        )?,
        requires_execution: serde_json::from_value(policy["requires_execution"].clone())?,
    };
    super::sessions::validate_snapshot(&snapshot)?;
    if digest(&value(&snapshot)?)? != control.snapshot_digest {
        return Err(StoreError::Conflict);
    }
    let recovery = control.recovery;
    Ok(RunRecord {
        id: run.into(),
        session_id: session.into(),
        turn_id: row.get(0),
        state: state(&row.get::<_, String>(1))?,
        version: row.get::<_, i64>(2) as u64,
        snapshot,
        recovery,
    })
}
impl PostgresStore {
    pub fn run(&self, owner: &str, session: &str, run: &str) -> Result<RunRecord> {
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        lock_session(&mut tx, owner, session)?;
        let result = read_run(&mut tx, session, run)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn transition_run(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        expected_version: u64,
        next: RunState,
        mut recovery: Option<RecoveryDecision>,
    ) -> Result<RunRecord> {
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        lock_session(&mut tx, owner, session)?;
        let current = read_run(&mut tx, session, run)?;
        if current.version != expected_version || !current.state.permits(next) {
            return Err(StoreError::Conflict);
        }
        if (next == RunState::Paused) != recovery.is_some() {
            return Err(StoreError::Invalid(
                "only paused runs require a recovery decision",
            ));
        }
        let unknown:i64=tx.query_one("SELECT COUNT(*) FROM execution_invocations WHERE turn_id=$1 AND status='submitted'",&[&current.turn_id])?.get(0);
        if next == RunState::Paused
            && unknown > 0
            && recovery.as_ref().map(|decision| decision.action) == Some(RecoveryAction::Resume)
        {
            recovery=Some(RecoveryDecision {action:RecoveryAction::ReconcileExternal,code:"outcome_unknown".into(),message:"Confirm the recorded external outcome before resuming; no call will be resubmitted".into()});
        }
        if next == RunState::Running && current.state == RunState::Paused {
            if unknown > 0 {
                return Err(StoreError::OutcomeUnknown);
            }
            if current.recovery.as_ref().map(|decision| decision.action)
                != Some(RecoveryAction::Resume)
            {
                return Err(StoreError::Conflict);
            }
            if tx.query_opt("SELECT id FROM execution_runs WHERE session_id=$1 AND id<>$2 AND status IN ('queued','pending','running','waiting_authorization')",&[&session,&run])?.is_some() {return Err(StoreError::Conflict);}
        }
        if next == RunState::Running && !super::scheduler::run_capacity(&mut tx, owner)? {
            return Err(StoreError::CapacityWaiting);
        }
        if next == RunState::Succeeded {
            let unfinished:i64=tx.query_one("SELECT COUNT(*) FROM execution_invocations WHERE turn_id=$1 AND status IN ('submitted','prepared')",&[&current.turn_id])?.get(0);
            if unfinished > 0 {
                return Err(StoreError::OutcomeUnknown);
            }
            let successes:i64=tx.query_one("SELECT COUNT(*) FROM execution_invocations WHERE turn_id=$1 AND status='completed' AND outcome_status='success'",&[&current.turn_id])?.get(0);
            if current.snapshot.requires_execution && successes == 0 {
                return Err(StoreError::MissingExecution);
            }
        }
        let mut persisted = super::control::load(&mut tx, run)?;
        persisted.recovery = recovery.clone();
        super::control::save(&mut tx, run, &persisted)?;
        let finished = if matches!(
            next,
            RunState::Succeeded | RunState::Failed | RunState::Cancelled
        ) {
            Some(now_ms()?)
        } else {
            None
        };
        tx.execute("UPDATE execution_runs SET status=$2,cancel_requested=$3,started_at=CASE WHEN $2='running' THEN COALESCE(started_at,$4) ELSE started_at END,finished_at=$5 WHERE id=$1 AND last_sequence=$6",&[&run,&status(next),&(if next==RunState::Cancelled {1_i64}else {0_i64}),&now_ms()?,&finished,&number(expected_version)?])?;
        tx.execute(
            "UPDATE execution_turns SET status=$2,updated_at=$3 WHERE id=$1",
            &[&current.turn_id, &status(next), &now_ms()?],
        )?;
        append(
            &mut tx,
            session,
            run,
            ExecutionEvent::RunTransition {
                run_id: run.into(),
                from: current.state,
                to: next,
                recovery,
            },
        )?;
        let result = read_run(&mut tx, session, run)?;
        tx.commit()?;
        Ok(result)
    }
}
