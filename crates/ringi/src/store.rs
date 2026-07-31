//! Durable persistence: `SqliteRegistry`, ringi's `pacta::Registry` over SQLite.
//!
//! This is the sans-I/O seam made real: pacta owns the `Registry` *contract* (synchronous,
//! clockless); ringi owns the *I/O* here. A backend is any type implementing the trait and
//! validated by `pacta-conformance` — `SqliteRegistry` passes that suite, so it satisfies the
//! contract by the same standard as the reference backend. It reimplements no lifecycle
//! policy; it stores and honors the lease/lapse/reclaim state pacta defines.
//!
//! `Registry` is a brick term used here at the seam (a backend implementation), per the
//! naming worldview — not a term of ringi's own domain.

use std::path::Path;
use std::sync::Mutex;

use pacta::{Claim, Pact, Registry, Retainer, Timestamp};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use uuid::Uuid;

/// The error a [`SqliteRegistry`] returns.
#[derive(Debug)]
pub enum StoreError {
    /// The presented retainer is not the current holder (or the claim is not held).
    NotHeld,
    /// An underlying SQLite error.
    Sqlite(rusqlite::Error),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotHeld => write!(f, "retainer is not the current holder of any claim"),
            Self::Sqlite(error) => write!(f, "sqlite error: {error}"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotHeld => None,
            Self::Sqlite(error) => Some(error),
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
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

    fn init(conn: &Connection) -> Result<(), StoreError> {
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
        Ok(())
    }
}

fn millis(t: Timestamp) -> i64 {
    // Timestamps are non-negative wall-clock offsets; the cast is lossless in range.
    i64::try_from(t.as_millis()).unwrap_or(i64::MAX)
}

impl Registry for SqliteRegistry {
    type Error = StoreError;

    fn claim(&self, dockets: &[&str], now: Timestamp) -> Result<Option<Claim>, Self::Error> {
        let conn = self.conn.lock().expect("registry mutex not poisoned");
        let now_ms = millis(now);

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

        let row = conn
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
            return Ok(None);
        };

        // Mint a fresh retainer only on a successful claim; the rotation is what makes a
        // prior holder unable to settle after a reclaim.
        let retainer = Retainer::new(Uuid::new_v4());
        let expiry = now.plus_millis(self.lease_millis);
        conn.execute(
            "UPDATE pacts SET state = 'held', retainer = ?, lease_expiry = ?, reclaimable_at = NULL
             WHERE id = ?",
            params![retainer.id().to_string(), millis(expiry), id],
        )?;

        let pact_id = Uuid::parse_str(&id).map_err(|_| StoreError::NotHeld)?;
        Ok(Some(Claim::new(
            Pact::new(pact_id, docket, kind, clause),
            retainer,
            expiry,
        )))
    }

    fn heartbeat(&self, retainer: &Retainer, now: Timestamp) -> Result<(), Self::Error> {
        let conn = self.conn.lock().expect("registry mutex not poisoned");
        let now_ms = millis(now);
        // Refuse to revive a lapsed lease: require the lease still valid at `now`.
        let changed = conn.execute(
            "UPDATE pacts SET lease_expiry = ?
             WHERE retainer = ? AND state = 'held' AND lease_expiry >= ?",
            params![
                millis(now.plus_millis(self.lease_millis)),
                retainer.id().to_string(),
                now_ms
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::NotHeld);
        }
        Ok(())
    }

    fn fulfill(&self, retainer: &Retainer) -> Result<(), Self::Error> {
        self.settle(retainer)
    }

    fn breach(&self, retainer: &Retainer) -> Result<(), Self::Error> {
        self.settle(retainer)
    }

    fn release(&self, retainer: &Retainer, reclaimable_at: Timestamp) -> Result<(), Self::Error> {
        let conn = self.conn.lock().expect("registry mutex not poisoned");
        let changed = conn.execute(
            "UPDATE pacts SET state = 'deferred', retainer = NULL, lease_expiry = NULL,
                              reclaimable_at = ?
             WHERE retainer = ? AND state = 'held'",
            params![millis(reclaimable_at), retainer.id().to_string()],
        )?;
        if changed == 0 {
            return Err(StoreError::NotHeld);
        }
        Ok(())
    }
}

impl SqliteRegistry {
    // fulfill and breach share one terminal settlement; a stale retainer (rotated away by a
    // reclaim) matches no held row, so it is rejected without needing time.
    fn settle(&self, retainer: &Retainer) -> Result<(), StoreError> {
        let conn = self.conn.lock().expect("registry mutex not poisoned");
        let changed = conn.execute(
            "UPDATE pacts SET state = 'settled', retainer = NULL, lease_expiry = NULL
             WHERE retainer = ? AND state = 'held'",
            params![retainer.id().to_string()],
        )?;
        if changed == 0 {
            return Err(StoreError::NotHeld);
        }
        Ok(())
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
}
