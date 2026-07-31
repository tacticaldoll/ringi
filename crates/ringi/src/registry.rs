//! The registry seam: ringi's one place that speaks pacta.
//!
//! Ringi owns no durable claim/lease mechanism of its own. Before invoking an Agent CLI for a
//! respondent, arbitrator, or condition-evaluator turn, it claims a pact keyed by that
//! invocation's [`crate::event::InvocationCoordinate`]; after the agent responds, it settles the
//! claim fulfilled or breached. A claim's lease durably distinguishes "attempted, not yet
//! confirmed settled" from "settled" — the checkpoint a single sequential process needs to
//! recover correctly from a crash between invoking the agent and committing the result. Per
//! `docs/naming.md`'s seam rule, pacta's vocabulary (`Pact`, `Claim`, `Retainer`, `Registry`,
//! `lifecycle`) is confined to this module and never names a ringi domain type: the public
//! surface here returns [`InvocationTicket`], never a raw `pacta::Retainer`.
//!
//! Adapted from a durable `SqliteRegistry` this repo built and conformance-proved before the
//! `reframe-ringi-deliberation` pivot deleted it as collateral with the old execution model
//! (see git history `6841606`, `707bfa3`) — the shape is proven, not new.

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use pacta::lifecycle::{self, State};
use pacta::{Claim, Pact, Registry, Retainer, Timestamp, Transition};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params, params_from_iter};
use uuid::Uuid;

use crate::event::InvocationCoordinate;

/// A fixed namespace for deriving a deterministic pact id from an invocation's coordinate, so
/// re-submitting the same coordinate always names the same pact.
const PACT_NAMESPACE: Uuid = Uuid::from_bytes([
    0x52, 0x69, 0x6e, 0x67, 0x69, 0x2d, 0x50, 0x61, 0x63, 0x74, 0x61, 0x2d, 0x53, 0x65, 0x61, 0x6d,
]);

/// The production lease: generously above the per-invocation timeout (60s today) so a normal
/// timeout-and-error path always settles before the lease would matter; the lease only bites the
/// crash case this module exists to cover. Test/conformance construction supplies its own lease
/// via [`SqliteRegistry::seeded`], since the lease is backend configuration, not part of the
/// pacta contract.
const PRODUCTION_LEASE_MILLIS: u64 = 5 * 60 * 1000;

/// The error a [`SqliteRegistry`] returns.
#[derive(Debug)]
pub enum RegistryError {
    /// The presented retainer is not the current holder of any claim.
    NotHeld,
    /// Persisted lifecycle data cannot be represented by pacta's lifecycle model.
    CorruptState(String),
    /// A pacta timestamp cannot be represented exactly by SQLite's signed integer.
    TimestampOutOfRange(u64),
    /// An underlying SQLite error.
    Sqlite(rusqlite::Error),
}

impl std::fmt::Display for RegistryError {
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

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for RegistryError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<lifecycle::NotCurrentHolder> for RegistryError {
    fn from(_: lifecycle::NotCurrentHolder) -> Self {
        Self::NotHeld
    }
}

/// Authority to settle one claimed invocation. Wraps a `pacta::Retainer` so no raw pacta type
/// crosses this module's boundary; only [`SqliteRegistry`] constructs or consumes one.
#[derive(Debug)]
pub struct InvocationTicket(Retainer);

/// A durable, file-backed `pacta::Registry` over SQLite, opening its own connection to the same
/// file `DossierStore` uses (two connections to one file, each with a `busy_timeout` — the
/// pattern the deleted implementation used, not a new design).
pub struct SqliteRegistry {
    conn: Mutex<Connection>,
    lease_millis: u64,
}

impl SqliteRegistry {
    /// Open (and provision) the durable registry at `path`, leasing claims for the production
    /// duration.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Self::init(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            lease_millis: PRODUCTION_LEASE_MILLIS,
        })
    }

    /// An in-memory registry holding `pacts`, each available to claim, leasing claims for
    /// `lease_millis`. Matches the `pacta-conformance` constructor shape
    /// (`fn(Vec<Pact>, u64) -> Self`), so it can be passed directly to
    /// `pacta_conformance::run`/`run_contention`.
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

    fn init(conn: &Connection) -> Result<(), RegistryError> {
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
        // Pacta requires durable claim selection to be full-scan-free.
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_pacts_claimable
             ON pacts (docket, state, lease_expiry, reclaimable_at)",
            [],
        )?;
        Ok(())
    }

    /// Claim a pact for `coordinate`, submitting it first if this is the first attempt.
    /// Idempotent: re-submitting the same coordinate names the same pact
    /// (`Uuid::new_v5` over its `idempotency_key()`), so this never creates a duplicate.
    pub fn claim_invocation(
        &self,
        coordinate: &InvocationCoordinate,
    ) -> Result<Option<InvocationTicket>, RegistryError> {
        let pact_id = Uuid::new_v5(&PACT_NAMESPACE, coordinate.idempotency_key().as_bytes());
        let docket = coordinate.dossier_id.to_string();

        {
            let conn = self.conn.lock().expect("registry mutex not poisoned");
            conn.execute(
                "INSERT OR IGNORE INTO pacts (id, docket, kind, clause, state)
                 VALUES (?, ?, ?, ?, 'available')",
                params![
                    pact_id.to_string(),
                    docket,
                    coordinate.role,
                    coordinate.idempotency_key().into_bytes(),
                ],
            )?;
        }

        let claim = self.claim(&[docket.as_str()], now_ms())?;
        Ok(claim.map(|claim| InvocationTicket(claim.retainer)))
    }

    /// Settle a ticket as fulfilled: the invocation succeeded.
    pub fn settle_fulfilled(&self, ticket: InvocationTicket) -> Result<(), RegistryError> {
        self.fulfill(&ticket.0)
    }

    /// Settle a ticket as breached: the invocation failed in a way that must never be retried
    /// under this exact coordinate. Terminal — a coordinate settled this way can never be
    /// claimed again; a caller wanting a retry must present a new coordinate (e.g. a bumped
    /// `attempt`). Available for a future retry policy; no current caller uses it (see
    /// `release_for_retry`, which today's callers use instead, since ringi has no
    /// give-up-after-N-tries policy yet).
    #[allow(dead_code)]
    pub fn settle_breached(&self, ticket: InvocationTicket) -> Result<(), RegistryError> {
        self.breach(&ticket.0)
    }

    /// Release a ticket for an immediate retry under the *same* coordinate: the invocation
    /// failed (a bad exit code, an infra error), but ringi has no policy yet that decides a
    /// failure is permanent, so the pact becomes claimable again right away rather than settling
    /// terminally — preserving today's behavior of simply re-running the command to retry.
    pub fn release_for_retry(&self, ticket: InvocationTicket) -> Result<(), RegistryError> {
        self.release(&ticket.0, now_ms())
    }

    /// Persist pacta's model state into ringi's row representation. Every lifecycle decision has
    /// already been made by pacta before this helper is called; it maps fields only.
    fn persist_state(conn: &Connection, pact_id: &str, state: &State) -> Result<(), RegistryError> {
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
            return Err(RegistryError::CorruptState(format!(
                "expected one pact row for id {pact_id}, updated {changed}"
            )));
        }
        Ok(())
    }

    fn held_state(retainer: &Retainer, lease_expiry: Option<i64>) -> Result<State, RegistryError> {
        let lease_expiry = lease_expiry.ok_or_else(|| {
            RegistryError::CorruptState("held row has no lease_expiry".to_string())
        })?;
        let lease_expiry = u64::try_from(lease_expiry).map_err(|_| {
            RegistryError::CorruptState(format!(
                "held row has negative lease_expiry {lease_expiry}"
            ))
        })?;
        Ok(State::Held {
            retainer: retainer.clone(),
            expiry: Timestamp::from_millis(lease_expiry),
        })
    }
}

fn millis(t: Timestamp) -> Result<i64, RegistryError> {
    i64::try_from(t.as_millis()).map_err(|_| RegistryError::TimestampOutOfRange(t.as_millis()))
}

fn now_ms() -> Timestamp {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_millis();
    Timestamp::from_millis(u64::try_from(millis).unwrap_or(u64::MAX))
}

impl Registry for SqliteRegistry {
    type Error = RegistryError;

    fn claim(&self, dockets: &[&str], now: Timestamp) -> Result<Option<Claim>, Self::Error> {
        if dockets.is_empty() {
            return Ok(None);
        }

        let mut conn = self.conn.lock().expect("registry mutex not poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now_ms = millis(now)?;

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
            RegistryError::CorruptState(format!("pact {id} has an invalid UUID: {error}"))
        })?;
        let retainer = Retainer::new(Uuid::new_v4());
        let next = lifecycle::on_claim(&retainer, now, self.lease_millis());
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
        let (pact_id, lease_expiry) = held.ok_or(RegistryError::NotHeld)?;
        let current = Self::held_state(retainer, lease_expiry)?;
        let next = transition(&current)?;
        Self::persist_state(&tx, &pact_id, &next)?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::revision::Digest;

    #[test]
    fn passes_registry_conformance() {
        pacta_conformance::run(SqliteRegistry::seeded);
    }

    #[test]
    fn passes_contention_conformance() {
        pacta_conformance::run_contention(SqliteRegistry::seeded);
    }

    fn coordinate(role: &str, turn: u32) -> InvocationCoordinate {
        InvocationCoordinate {
            dossier_id: Uuid::new_v4(),
            role: role.to_string(),
            input_digest: Digest("dig".into()),
            turn,
            attempt: 1,
        }
    }

    #[test]
    fn resubmitting_the_same_coordinate_is_idempotent() {
        let registry = SqliteRegistry::open(":memory:").expect("open");
        let coord = coordinate("respondent", 1);

        let first = registry
            .claim_invocation(&coord)
            .expect("claim should not error")
            .expect("a fresh coordinate should be claimable");
        registry
            .settle_fulfilled(first)
            .expect("settle should succeed");

        // Re-submitting after settlement finds the same pact, now settled -> no claim.
        let second = registry
            .claim_invocation(&coord)
            .expect("claim should not error");
        assert!(
            second.is_none(),
            "a settled coordinate must not be claimable again"
        );
    }

    #[test]
    fn a_successful_invocation_is_claimed_then_fulfilled() {
        let registry = SqliteRegistry::open(":memory:").expect("open");
        let coord = coordinate("respondent", 1);

        let ticket = registry
            .claim_invocation(&coord)
            .expect("claim should not error")
            .expect("a fresh coordinate should be claimable");
        registry
            .settle_fulfilled(ticket)
            .expect("settle fulfilled should succeed");

        assert!(registry.claim_invocation(&coord).unwrap().is_none());
    }

    #[test]
    fn a_failing_invocation_is_released_and_stays_retryable_under_the_same_coordinate() {
        let registry = SqliteRegistry::open(":memory:").expect("open");
        let coord = coordinate("respondent", 1);

        let ticket = registry
            .claim_invocation(&coord)
            .expect("claim should not error")
            .expect("a fresh coordinate should be claimable");
        registry
            .release_for_retry(ticket)
            .expect("release should succeed");

        // Unlike fulfill/breach, release is non-terminal: the same coordinate is claimable again.
        let retry = registry
            .claim_invocation(&coord)
            .expect("claim should not error");
        assert!(
            retry.is_some(),
            "a released coordinate must be claimable again immediately"
        );
    }

    #[test]
    fn a_breached_coordinate_cannot_be_claimed_again() {
        let registry = SqliteRegistry::open(":memory:").expect("open");
        let coord = coordinate("respondent", 1);

        let ticket = registry
            .claim_invocation(&coord)
            .expect("claim should not error")
            .expect("a fresh coordinate should be claimable");
        registry
            .settle_breached(ticket)
            .expect("settle breached should succeed");

        assert!(registry.claim_invocation(&coord).unwrap().is_none());
    }
}
