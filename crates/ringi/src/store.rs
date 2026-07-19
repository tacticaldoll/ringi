//! Durable persistence: `SqliteRegistry`, ringi's `pacta::Registry` over SQLite.
//!
//! This is the sans-I/O seam made real: pacta owns the `Registry` *contract* (synchronous,
//! clockless); ringi owns the *I/O* here. A backend is any type implementing the trait and
//! validated by `pacta-conformance` — `SqliteRegistry` passes its sequential and contention suites,
//! so it satisfies the contract by the same standard as the reference backend. Its native claim
//! query owns SQLite selection; one transactional `apply` port executes pacta's shared `lifecycle`
//! decisions, so ringi reimplements no transition policy.
//!
//! `Registry` is a brick term used here at the seam (a backend implementation), per the
//! naming worldview — not a term of ringi's own domain.

use std::path::Path;
use std::sync::Mutex;

use pacta::lifecycle::{self, State};
use pacta::{Claim, Pact, Registry, Retainer, Timestamp, Transition};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params, params_from_iter};
use uuid::Uuid;

use crate::reconcile::{Finding, Resume, RunJournal};

/// The error a [`SqliteRegistry`] returns.
#[derive(Debug)]
pub enum StoreError {
    /// The presented retainer is not the current holder (or the claim is not held).
    NotHeld,
    /// Persisted lifecycle data cannot be represented by pacta's lifecycle model.
    CorruptState(String),
    /// A pacta timestamp cannot be represented exactly by SQLite's signed integer.
    TimestampOutOfRange(u64),
    /// An underlying SQLite error.
    Sqlite(rusqlite::Error),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotHeld => write!(f, "retainer is not the current holder of any claim"),
            Self::CorruptState(message) => write!(f, "corrupt lifecycle state: {message}"),
            Self::TimestampOutOfRange(millis) => {
                write!(f, "timestamp {millis}ms is outside SQLite's exact range")
            }
            Self::Sqlite(error) => write!(f, "sqlite error: {error}"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotHeld => None,
            Self::CorruptState(_) => None,
            Self::TimestampOutOfRange(_) => None,
            Self::Sqlite(error) => Some(error),
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<lifecycle::NotCurrentHolder> for StoreError {
    fn from(_: lifecycle::NotCurrentHolder) -> Self {
        Self::NotHeld
    }
}

/// A durable [`Registry`] backend over SQLite. The connection is behind a `Mutex` so the
/// type is `Sync` (the trait requires it) while rusqlite's `Connection` is not.
pub struct SqliteRegistry {
    conn: Mutex<Connection>,
    lease_millis: u64,
}

impl SqliteRegistry {
    /// Open a durable, file-backed registry, leasing claims for `lease_millis`. Existing
    /// state persists across reopen — this is where "the store is the source of truth" lives.
    pub fn open(path: impl AsRef<Path>, lease_millis: u64) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Self::init(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            lease_millis,
        })
    }

    /// An in-memory registry seeded with `pacts`, each available to claim. Matches the
    /// `pacta-conformance` constructor shape and is used for tests.
    #[must_use]
    pub fn seeded(pacts: Vec<Pact>, lease_millis: u64) -> Self {
        let conn = Connection::open_in_memory().expect("open in-memory sqlite");
        Self::init(&conn).expect("init schema");
        for pact in pacts {
            conn.execute(
                "INSERT INTO pacts (id, docket, kind, clause, state) VALUES (?, ?, ?, ?, 'available')",
                params![pact.id.to_string(), pact.docket, pact.kind, pact.clause],
            )
            .expect("seed pact");
        }
        Self {
            conn: Mutex::new(conn),
            lease_millis,
        }
    }

    /// Open a durable, file-backed registry and idempotently seed `pacts` (their dockets carry
    /// the run id, so re-seeding the same run is a no-op). This is the backend `ringi run` drives
    /// over — durable where the reference `MemoryRegistry` is not.
    pub fn open_seeded(
        path: impl AsRef<Path>,
        pacts: Vec<Pact>,
        lease_millis: u64,
    ) -> Result<Self, StoreError> {
        let registry = Self::open(path, lease_millis)?;
        {
            let conn = registry.conn.lock().expect("registry mutex not poisoned");
            for pact in pacts {
                conn.execute(
                    "INSERT OR IGNORE INTO pacts (id, docket, kind, clause, state)
                     VALUES (?, ?, ?, ?, 'available')",
                    params![pact.id.to_string(), pact.docket, pact.kind, pact.clause],
                )?;
            }
        }
        Ok(registry)
    }

    fn init(conn: &Connection) -> Result<(), StoreError> {
        // The one user-scope DB holds both the Registry's lease/lifecycle state (`pacts`) and
        // ringi's own domain (`runs`, `findings`), per the seam design in BACKLOG.md.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS pacts (
                id             TEXT PRIMARY KEY,
                docket         TEXT NOT NULL,
                kind           TEXT NOT NULL,
                clause         BLOB NOT NULL,
                state          TEXT NOT NULL,
                retainer       TEXT,
                lease_expiry   INTEGER,
                reclaimable_at INTEGER
            )",
            [],
        )?;
        // Pacta requires durable claim selection to be full-scan-free. This index begins with the
        // requested docket and lifecycle state, then carries both time boundaries used by the
        // native eligibility predicate. Existing databases gain it idempotently when opened.
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_pacts_claimable
             ON pacts (docket, state, lease_expiry, reclaimable_at)",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS runs (
                run_id     TEXT PRIMARY KEY,
                task       TEXT NOT NULL,
                workspace  TEXT NOT NULL,
                state      TEXT NOT NULL,
                rounds     INTEGER NOT NULL DEFAULT 0,
                next_round INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS findings (
                run_id     TEXT NOT NULL,
                finding_id TEXT NOT NULL,
                PRIMARY KEY (run_id, finding_id)
            )",
            [],
        )?;
        // The durable witness ledger: which rounds' build attempts have succeeded. A build Deed
        // is determined by (run_id, round), so this is all that is needed to reconstruct the
        // ledger on resume (see `RunStore::load_resume`).
        conn.execute(
            "CREATE TABLE IF NOT EXISTS deeds (
                run_id TEXT NOT NULL,
                round  INTEGER NOT NULL,
                PRIMARY KEY (run_id, round)
            )",
            [],
        )?;
        Ok(())
    }

    /// Persist pacta's model state into ringi's existing row representation. This maps fields only:
    /// every lifecycle decision has already been made by pacta before this helper is called.
    fn persist_state(conn: &Connection, pact_id: &str, state: &State) -> Result<(), StoreError> {
        let changed = match state {
            State::Available => conn.execute(
                "UPDATE pacts SET state = 'available', retainer = NULL, lease_expiry = NULL,
                                  reclaimable_at = NULL WHERE id = ?",
                params![pact_id],
            )?,
            State::Held { retainer, expiry } => {
                let expiry = millis(*expiry)?;
                conn.execute(
                    "UPDATE pacts SET state = 'held', retainer = ?, lease_expiry = ?,
                                      reclaimable_at = NULL WHERE id = ?",
                    params![retainer.id().to_string(), expiry, pact_id],
                )?
            }
            State::Deferred { reclaimable_at } => {
                let reclaimable_at = millis(*reclaimable_at)?;
                conn.execute(
                    "UPDATE pacts SET state = 'deferred', retainer = NULL, lease_expiry = NULL,
                                      reclaimable_at = ? WHERE id = ?",
                    params![reclaimable_at, pact_id],
                )?
            }
            State::Settled => conn.execute(
                "UPDATE pacts SET state = 'settled', retainer = NULL, lease_expiry = NULL,
                                  reclaimable_at = NULL WHERE id = ?",
                params![pact_id],
            )?,
        };
        if changed != 1 {
            return Err(StoreError::CorruptState(format!(
                "expected one pact row for id {pact_id}, updated {changed}"
            )));
        }
        Ok(())
    }

    fn held_state(retainer: &Retainer, lease_expiry: Option<i64>) -> Result<State, StoreError> {
        let lease_expiry = lease_expiry
            .ok_or_else(|| StoreError::CorruptState("held row has no lease_expiry".to_string()))?;
        let lease_expiry = u64::try_from(lease_expiry).map_err(|_| {
            StoreError::CorruptState(format!("held row has negative lease_expiry {lease_expiry}"))
        })?;
        Ok(State::Held {
            retainer: retainer.clone(),
            expiry: Timestamp::from_millis(lease_expiry),
        })
    }
}

fn millis(t: Timestamp) -> Result<i64, StoreError> {
    i64::try_from(t.as_millis()).map_err(|_| StoreError::TimestampOutOfRange(t.as_millis()))
}

impl Registry for SqliteRegistry {
    type Error = StoreError;

    fn claim(&self, dockets: &[&str], now: Timestamp) -> Result<Option<Claim>, Self::Error> {
        if dockets.is_empty() {
            return Ok(None);
        }

        let mut conn = self.conn.lock().expect("registry mutex not poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now_ms = millis(now)?;

        // Claimable: available, a lapsed hold, or a deferred pact whose instant has passed.
        let placeholders = vec!["?"; dockets.len()].join(",");
        let sql = format!(
            "SELECT id, docket, kind, clause FROM pacts
             WHERE docket IN ({placeholders})
               AND (state = 'available'
                    OR (state = 'held' AND lease_expiry < ?)
                    OR (state = 'deferred' AND reclaimable_at <= ?))
             LIMIT 1"
        );
        let mut args: Vec<Value> = dockets
            .iter()
            .map(|d| Value::Text((*d).to_string()))
            .collect();
        args.push(Value::Integer(now_ms));
        args.push(Value::Integer(now_ms));

        let row = tx
            .query_row(&sql, params_from_iter(args.iter()), |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Vec<u8>>(3)?,
                ))
            })
            .optional()?;

        let Some((id, docket, kind, clause)) = row else {
            tx.commit()?;
            return Ok(None);
        };

        let pact_id = Uuid::parse_str(&id).map_err(|error| {
            StoreError::CorruptState(format!("pact {id} has an invalid UUID: {error}"))
        })?;
        // Mint a fresh retainer only on a successful claim; the rotation is what makes a
        // prior holder unable to settle after a reclaim. Pacta produces the held state.
        let retainer = Retainer::new(Uuid::new_v4());
        let next = lifecycle::on_claim(&retainer, now, self.lease_millis);
        let State::Held { expiry, .. } = &next else {
            unreachable!("on_claim always produces held state")
        };
        let expiry = *expiry;
        Self::persist_state(&tx, &id, &next)?;
        tx.commit()?;

        Ok(Some(Claim::new(
            Pact::new(pact_id, docket, kind, clause),
            retainer,
            expiry,
        )))
    }

    fn lease_millis(&self) -> u64 {
        self.lease_millis
    }

    fn apply(&self, retainer: &Retainer, transition: &Transition<'_>) -> Result<(), Self::Error> {
        let mut conn = self.conn.lock().expect("registry mutex not poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let held = tx
            .query_row(
                "SELECT id, lease_expiry FROM pacts WHERE retainer = ? AND state = 'held' LIMIT 1",
                params![retainer.id().to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
            .optional()?;
        let (pact_id, lease_expiry) = held.ok_or(StoreError::NotHeld)?;
        let current = Self::held_state(retainer, lease_expiry)?;
        let next = transition(&current).map_err(StoreError::from)?;
        Self::persist_state(&tx, &pact_id, &next)?;
        tx.commit()?;
        Ok(())
    }
}

/// The lifecycle state of a persisted run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    /// Recorded but not yet driven to a terminal outcome — in progress, or interrupted.
    Running,
    /// The run converged.
    Converged,
    /// The run reached the round limit without converging.
    Failed,
}

impl RunState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Converged => "converged",
            Self::Failed => "failed",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "converged" => Self::Converged,
            "failed" => Self::Failed,
            _ => Self::Running,
        }
    }
}

/// A persisted run's record, as read back from the store.
#[derive(Debug, Clone)]
pub struct RunRecord {
    /// Stable identity of the run.
    pub run_id: String,
    /// The task the run was given.
    pub task: String,
    /// The workspace the run operated in.
    pub workspace: String,
    /// The run's lifecycle state.
    pub state: RunState,
    /// Rounds the loop ran (0 while still running).
    pub rounds: usize,
    /// Ids of findings still open at the terminal outcome.
    pub open_findings: Vec<String>,
}

/// Ringi's durable domain store, over the **same** SQLite DB as the Registry: it records a run's
/// identity and outcome so they survive the process. Recording is done here in the driver layer,
/// never inside the composition root (`run-assembly` stays non-persisting). Kept on its own
/// connection for now; unifying it with the Registry's connection onto one owner is a later
/// refinement (see the change design).
pub struct RunStore {
    conn: Connection,
}

impl RunStore {
    /// Open (and provision) the durable store at `path`. A `busy_timeout` guards against
    /// incidental locking from the Registry's separate connection to the same file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        SqliteRegistry::init(&conn)?;
        Ok(Self { conn })
    }

    /// Record a run as started (state `running`), before its loop is driven.
    pub fn create_run(&self, run_id: &str, task: &str, workspace: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO runs (run_id, task, workspace, state, rounds)
             VALUES (?, ?, ?, 'running', 0)",
            params![run_id, task, workspace],
        )?;
        Ok(())
    }

    /// Record a run's terminal outcome and its open findings.
    pub fn complete_run(
        &self,
        run_id: &str,
        converged: bool,
        rounds: usize,
        open_findings: &[String],
    ) -> Result<(), StoreError> {
        let state = if converged {
            RunState::Converged
        } else {
            RunState::Failed
        };
        self.conn.execute(
            "UPDATE runs SET state = ?, rounds = ? WHERE run_id = ?",
            params![
                state.as_str(),
                i64::try_from(rounds).unwrap_or(i64::MAX),
                run_id
            ],
        )?;
        // Replace any checkpointed findings with the terminal open set, so `findings` always
        // reflects what is currently open (a converged run clears stale checkpoint findings).
        self.conn
            .execute("DELETE FROM findings WHERE run_id = ?", params![run_id])?;
        for id in open_findings {
            self.conn.execute(
                "INSERT OR IGNORE INTO findings (run_id, finding_id) VALUES (?, ?)",
                params![run_id, id],
            )?;
        }
        Ok(())
    }

    /// Read a persisted run, or `None` if the id is unknown.
    pub fn get_run(&self, run_id: &str) -> Result<Option<RunRecord>, StoreError> {
        let row = self
            .conn
            .query_row(
                "SELECT task, workspace, state, rounds FROM runs WHERE run_id = ?",
                params![run_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((task, workspace, state, rounds)) = row else {
            return Ok(None);
        };
        let mut stmt = self
            .conn
            .prepare("SELECT finding_id FROM findings WHERE run_id = ? ORDER BY finding_id")?;
        let open_findings = stmt
            .query_map(params![run_id], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(RunRecord {
            run_id: run_id.to_string(),
            task,
            workspace,
            state: RunState::parse(&state),
            rounds: usize::try_from(rounds).unwrap_or(0),
            open_findings,
        }))
    }

    /// A [`RunJournal`] handle bound to `run_id`, so the round loop can record its progress.
    #[must_use]
    pub fn journal(&self, run_id: &str) -> RunJournalHandle<'_> {
        RunJournalHandle {
            store: self,
            run_id: run_id.to_string(),
        }
    }

    /// Record that `round`'s build attempt succeeded (idempotent).
    fn record_deed(&self, run_id: &str, round: usize) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO deeds (run_id, round) VALUES (?, ?)",
            params![run_id, i64::try_from(round).unwrap_or(i64::MAX)],
        )?;
        Ok(())
    }

    /// Record a checkpoint: the next round to run and the currently-open findings (replacing the
    /// prior open set, so `findings` always reflects what is open now).
    fn set_checkpoint(
        &self,
        run_id: &str,
        next_round: usize,
        open_findings: &[String],
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE runs SET next_round = ? WHERE run_id = ?",
            params![i64::try_from(next_round).unwrap_or(i64::MAX), run_id],
        )?;
        self.conn
            .execute("DELETE FROM findings WHERE run_id = ?", params![run_id])?;
        for id in open_findings {
            self.conn.execute(
                "INSERT OR IGNORE INTO findings (run_id, finding_id) VALUES (?, ?)",
                params![run_id, id],
            )?;
        }
        Ok(())
    }

    /// Load a resume point for a still-running run: the next round, the succeeded rounds (to
    /// rebuild the ledger), and the open findings. Returns `None` if the run is unknown or is
    /// not in a resumable (still-running) state.
    pub fn load_resume(&self, run_id: &str) -> Result<Option<Resume>, StoreError> {
        let row = self
            .conn
            .query_row(
                "SELECT state, next_round FROM runs WHERE run_id = ?",
                params![run_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
            )
            .optional()?;
        let Some((state, next_round)) = row else {
            return Ok(None);
        };
        if RunState::parse(&state) != RunState::Running {
            return Ok(None);
        }

        let mut deeds_stmt = self
            .conn
            .prepare("SELECT round FROM deeds WHERE run_id = ? ORDER BY round")?;
        let built_rounds = deeds_stmt
            .query_map(params![run_id], |r| r.get::<_, i64>(0))?
            .map(|r| r.map(|v| usize::try_from(v).unwrap_or(0)))
            .collect::<Result<Vec<_>, _>>()?;

        let mut findings_stmt = self
            .conn
            .prepare("SELECT finding_id FROM findings WHERE run_id = ? ORDER BY finding_id")?;
        // Only the finding id is persisted; the loop keys targets by id, so an empty summary is
        // immaterial on resume.
        let open_findings = findings_stmt
            .query_map(params![run_id], |r| {
                Ok(Finding {
                    id: r.get::<_, String>(0)?,
                    summary: String::new(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Some(Resume {
            start_round: usize::try_from(next_round).unwrap_or(0),
            built_rounds,
            open_findings,
        }))
    }
}

/// A [`RunJournal`] bound to one run, writing the round loop's progress into the durable store.
/// Journalling failures are durability faults and surface as a panic — the run cannot claim to be
/// resumable if its progress was not recorded.
pub struct RunJournalHandle<'a> {
    store: &'a RunStore,
    run_id: String,
}

impl RunJournal for RunJournalHandle<'_> {
    fn build_succeeded(&self, round: usize) {
        self.store
            .record_deed(&self.run_id, round)
            .expect("journal a succeeded build");
    }

    fn checkpoint(&self, next_round: usize, open_findings: &[Finding]) {
        let ids: Vec<String> = open_findings.iter().map(|f| f.id.clone()).collect();
        self.store
            .set_checkpoint(&self.run_id, next_round, &ids)
            .expect("journal a checkpoint");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_registry_conformance() {
        pacta_conformance::run(SqliteRegistry::seeded);
    }

    #[test]
    fn passes_registry_contention_conformance() {
        pacta_conformance::run_contention(SqliteRegistry::seeded);
    }

    #[test]
    fn independent_connections_issue_only_one_claim() {
        let path = temp_db("cross-connection-claim");
        let pact = Pact::new(Uuid::new_v4(), "d".into(), "step".into(), Vec::new());
        let first = SqliteRegistry::open_seeded(&path, vec![pact], 1_000).expect("open and seed");
        let second = SqliteRegistry::open(&path, 1_000).expect("open second connection");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));

        let a = {
            let barrier = std::sync::Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                first.claim(&["d"], Timestamp::from_millis(0))
            })
        };
        let b = {
            let barrier = std::sync::Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                second.claim(&["d"], Timestamp::from_millis(0))
            })
        };

        let claims = [
            a.join().expect("first thread"),
            b.join().expect("second thread"),
        ]
        .into_iter()
        .flat_map(|result| result.expect("claim must not leak SQLite contention"))
        .collect::<Vec<_>>();
        assert_eq!(
            claims.len(),
            1,
            "one SQLite transaction must win the single eligible pact"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn claim_query_uses_the_claimable_index() {
        let registry = SqliteRegistry::seeded(Vec::new(), 1_000);
        let conn = registry.conn.lock().expect("registry mutex not poisoned");
        let mut statement = conn
            .prepare(
                "EXPLAIN QUERY PLAN
                 SELECT id, docket, kind, clause FROM pacts
                 WHERE docket IN (?)
                   AND (state = 'available'
                        OR (state = 'held' AND lease_expiry < ?)
                        OR (state = 'deferred' AND reclaimable_at <= ?))
                 LIMIT 1",
            )
            .expect("prepare query plan");
        let details = statement
            .query_map(params!["d", 0_i64, 0_i64], |row| row.get::<_, String>(3))
            .expect("explain query")
            .collect::<Result<Vec<_>, _>>()
            .expect("query-plan rows");
        assert!(
            details
                .iter()
                .any(|detail| detail.contains("USING INDEX idx_pacts_claimable")),
            "claim selection must use its index, query plan: {details:?}"
        );
        assert!(
            details.iter().all(|detail| !detail.contains("SCAN pacts")),
            "claim selection must not scan the pact table, query plan: {details:?}"
        );
    }

    #[test]
    fn out_of_range_timestamp_is_rejected_without_claiming() {
        let registry = SqliteRegistry::seeded(
            vec![Pact::new(
                Uuid::new_v4(),
                "d".into(),
                "step".into(),
                Vec::new(),
            )],
            1_000,
        );
        let out_of_range = Timestamp::from_millis((i64::MAX as u64) + 1);
        assert!(matches!(
            registry.claim(&["d"], out_of_range),
            Err(StoreError::TimestampOutOfRange(value)) if value == out_of_range.as_millis()
        ));
        assert!(
            registry
                .claim(&["d"], Timestamp::from_millis(0))
                .expect("valid timestamp")
                .is_some(),
            "the rejected timestamp must leave the pact available"
        );
    }

    #[test]
    fn apply_rejects_a_stranger_even_when_the_transition_accepts_any_state() {
        let registry = SqliteRegistry::seeded(
            vec![Pact::new(
                Uuid::new_v4(),
                "d".into(),
                "step".into(),
                Vec::new(),
            )],
            1_000,
        );
        let claim = registry
            .claim(&["d"], Timestamp::from_millis(0))
            .expect("claim")
            .expect("claimable");
        let stranger = Retainer::new(Uuid::new_v4());
        let accept_any = |_state: &State| Ok::<State, lifecycle::NotCurrentHolder>(State::Settled);

        assert!(matches!(
            registry.apply(&stranger, &accept_any),
            Err(StoreError::NotHeld)
        ));
        registry
            .fulfill(&claim.retainer)
            .expect("stranger attempt must leave the holder's row unchanged");
    }

    #[test]
    fn corrupt_held_state_is_not_reported_as_lost_authority() {
        let registry = SqliteRegistry::seeded(
            vec![Pact::new(
                Uuid::new_v4(),
                "d".into(),
                "step".into(),
                Vec::new(),
            )],
            1_000,
        );
        let claim = registry
            .claim(&["d"], Timestamp::from_millis(0))
            .expect("claim")
            .expect("claimable");
        registry
            .conn
            .lock()
            .expect("registry mutex not poisoned")
            .execute(
                "UPDATE pacts SET lease_expiry = NULL WHERE retainer = ?",
                params![claim.retainer.id().to_string()],
            )
            .expect("corrupt fixture");

        let error = registry
            .fulfill(&claim.retainer)
            .expect_err("corrupt held row must fail");
        assert!(
            matches!(error, StoreError::CorruptState(_)),
            "corrupt persisted data must not masquerade as NotHeld: {error}"
        );
    }

    #[test]
    fn state_persists_across_reopen() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ringi-registry-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let pact = Pact::new(Uuid::new_v4(), "d".into(), "step".into(), Vec::new());
        {
            let reg = SqliteRegistry::open(&path, 1_000).expect("open");
            // Seed one pact by hand (open() does not seed) and claim it.
            reg.conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO pacts (id, docket, kind, clause, state) VALUES (?, 'd', 'step', X'', 'available')",
                    params![pact.id.to_string()],
                )
                .unwrap();
            let claim = reg
                .claim(&["d"], Timestamp::from_millis(0))
                .unwrap()
                .expect("claimable");
            // Hold it, then drop the registry (simulating a crash before settlement).
            let _ = claim;
        }
        // Reopen the same file: the held state survived, so once the lease lapses the pact is
        // reclaimable — the store, not memory, was the source of truth.
        let reopened = SqliteRegistry::open(&path, 1_000).expect("reopen");
        let reclaimed = reopened
            .claim(&["d"], Timestamp::from_millis(5_000))
            .unwrap();
        assert!(
            reclaimed.is_some(),
            "a held pact must survive reopen and be reclaimable after its lease lapses"
        );

        let _ = std::fs::remove_file(&path);
    }

    fn temp_db(tag: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "ringi-runstore-{}-{tag}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn a_completed_run_round_trips_across_reopen() {
        let path = temp_db("complete");
        {
            let store = RunStore::open(&path).expect("open");
            store.create_run("run-1", "do it", "/ws").expect("create");
            store
                .complete_run("run-1", true, 3, &["F1".to_string()])
                .expect("complete");
        }
        // A fresh process (new connection) reads the persisted run — the store is the truth.
        let reopened = RunStore::open(&path).expect("reopen");
        let record = reopened.get_run("run-1").expect("query").expect("present");
        assert_eq!(record.state, RunState::Converged);
        assert_eq!(record.rounds, 3);
        assert_eq!(record.task, "do it");
        assert_eq!(record.open_findings, vec!["F1".to_string()]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn an_interrupted_run_reads_as_running() {
        let path = temp_db("interrupted");
        {
            let store = RunStore::open(&path).expect("open");
            store.create_run("run-2", "t", "/ws").expect("create");
            // No complete_run: the process "crashed" mid-run.
        }
        let reopened = RunStore::open(&path).expect("reopen");
        let record = reopened.get_run("run-2").expect("query").expect("present");
        assert_eq!(
            record.state,
            RunState::Running,
            "an interrupted run is observably not-complete, never absent"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn an_unknown_run_id_is_none() {
        let path = temp_db("unknown");
        let store = RunStore::open(&path).expect("open");
        assert!(
            store.get_run("nope").expect("query").is_none(),
            "an unknown run id is not found, never a fabricated record"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_checkpoint_and_deeds_reconstruct_a_resume_point() {
        let path = temp_db("resume");
        {
            let store = RunStore::open(&path).expect("open");
            store.create_run("run-r", "t", "/ws").expect("create");
            let journal = store.journal("run-r");
            journal.build_succeeded(0);
            journal.checkpoint(
                1,
                &[Finding {
                    id: "F1".to_string(),
                    summary: "x".to_string(),
                }],
            );
            journal.build_succeeded(1);
            journal.checkpoint(
                2,
                &[Finding {
                    id: "F1".to_string(),
                    summary: "x".to_string(),
                }],
            );
        }
        // A fresh process reconstructs the resume point from the durable checkpoint + deeds.
        let reopened = RunStore::open(&path).expect("reopen");
        let resume = reopened
            .load_resume("run-r")
            .expect("query")
            .expect("resumable");
        assert_eq!(resume.start_round, 2);
        assert_eq!(resume.built_rounds, vec![0, 1]);
        assert_eq!(
            resume
                .open_findings
                .iter()
                .map(|f| f.id.clone())
                .collect::<Vec<_>>(),
            vec!["F1".to_string()]
        );

        // A finished run is not resumable; nor is an unknown id.
        reopened.complete_run("run-r", true, 2, &[]).expect("done");
        assert!(
            reopened.load_resume("run-r").expect("query").is_none(),
            "a finished run is not resumable"
        );
        assert!(
            reopened.load_resume("nope").expect("query").is_none(),
            "an unknown run is not resumable"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_failed_run_records_its_open_findings() {
        let path = temp_db("failed");
        let store = RunStore::open(&path).expect("open");
        store.create_run("run-3", "t", "/ws").expect("create");
        store
            .complete_run("run-3", false, 16, &["F1".to_string(), "F2".to_string()])
            .expect("complete");
        let record = store.get_run("run-3").expect("query").expect("present");
        assert_eq!(record.state, RunState::Failed);
        assert_eq!(record.rounds, 16);
        assert_eq!(
            record.open_findings,
            vec!["F1".to_string(), "F2".to_string()]
        );
        let _ = std::fs::remove_file(&path);
    }
}
