use std::path::Path;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use uuid::Uuid;

/// The error a store returns.
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
pub fn init(conn: &Connection) -> Result<(), StoreError> {
    // Dossier domain schema
    conn.execute(
        "CREATE TABLE IF NOT EXISTS dossiers (
                id    TEXT PRIMARY KEY,
                state TEXT NOT NULL
            )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS locked_settings (
                dossier_id  TEXT PRIMARY KEY,
                strategy    TEXT NOT NULL,
                max_turns   INTEGER NOT NULL,
                respondent  TEXT NOT NULL,
                arbitrator  TEXT NOT NULL
            )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS revisions (
                id                    TEXT PRIMARY KEY,
                dossier_id            TEXT NOT NULL,
                parent_digest         TEXT,
                content_digest        TEXT NOT NULL,
                original_proposal     TEXT NOT NULL,
                current_understanding TEXT NOT NULL,
                readiness             INTEGER NOT NULL
            )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS events (
                id              TEXT PRIMARY KEY,
                dossier_id      TEXT NOT NULL,
                timestamp       INTEGER NOT NULL,
                visibility      TEXT NOT NULL,
                payload_type    TEXT NOT NULL,
                payload_content TEXT,
                evaluator       TEXT,
                reasoning       TEXT,
                idempotency_key TEXT UNIQUE
            )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS dissents (
                id              TEXT PRIMARY KEY,
                revision_id     TEXT NOT NULL,
                claim           TEXT NOT NULL,
                resolved_reason TEXT
            )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS resolution_provenance (
                dissent_id TEXT NOT NULL,
                event_id   TEXT NOT NULL,
                PRIMARY KEY (dissent_id, event_id)
            )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS conditions (
                id          TEXT PRIMARY KEY,
                dossier_id  TEXT NOT NULL,
                predicate   TEXT NOT NULL,
                state       TEXT NOT NULL
            )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS decisions (
                id          TEXT PRIMARY KEY,
                dossier_id  TEXT NOT NULL,
                kind        TEXT NOT NULL,
                human_id    TEXT NOT NULL
            )",
        [],
    )?;

    Ok(())
}

/// The new store mapping the dossier domain.
pub struct DossierStore {
    conn: Connection,
}

impl DossierStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        init(&conn)?;
        Ok(Self { conn })
    }

    pub fn insert_dossier(&self, id: &str, state: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO dossiers (id, state) VALUES (?, ?)",
            params![id, state],
        )?;
        Ok(())
    }

    pub fn get_dossier_state(&self, id: &str) -> Result<Option<String>, StoreError> {
        let row = self
            .conn
            .query_row(
                "SELECT state FROM dossiers WHERE id = ?",
                params![id],
                |r| r.get::<_, String>(0),
            )
            .optional()?;
        Ok(row)
    }

    pub fn is_invocation_completed(
        &self,
        coordinate: &crate::event::InvocationCoordinate,
    ) -> Result<bool, StoreError> {
        let key = coordinate.idempotency_key();
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(1) FROM events WHERE idempotency_key = ?",
            params![key],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn get_latest_revision(
        &self,
        dossier_id: &str,
    ) -> Result<Option<crate::revision::Revision>, StoreError> {
        let row = self.conn.query_row(
            "SELECT id, parent_digest, content_digest, original_proposal, current_understanding, readiness
             FROM revisions WHERE dossier_id = ? ORDER BY _rowid_ DESC LIMIT 1",
            params![dossier_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            }
        ).optional()?;

        let Some((
            id_str,
            parent_digest,
            content_digest,
            original_proposal,
            current_understanding,
            readiness,
        )) = row
        else {
            return Ok(None);
        };

        let revision_id = Uuid::parse_str(&id_str).unwrap_or_default();

        let mut dissents_stmt = self
            .conn
            .prepare("SELECT id, claim, resolved_reason FROM dissents WHERE revision_id = ?")?;
        let dissents_iter = dissents_stmt.query_map(params![&id_str], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;

        let mut dissents = Vec::new();
        for dissent_res in dissents_iter {
            let (d_id, claim, resolved_reason) = dissent_res?;
            let dissent_uuid = Uuid::parse_str(&d_id).unwrap_or_default();

            let resolved_by = if let Some(reason) = resolved_reason {
                let mut prov_stmt = self
                    .conn
                    .prepare("SELECT event_id FROM resolution_provenance WHERE dissent_id = ?")?;
                let prov_iter =
                    prov_stmt.query_map(params![&d_id], |row| row.get::<_, String>(0))?;
                let mut provenance = Vec::new();
                for p_res in prov_iter {
                    provenance.push(crate::revision::EventRef {
                        event_id: Uuid::parse_str(&p_res?).unwrap_or_default(),
                    });
                }
                Some(crate::revision::Resolution { reason, provenance })
            } else {
                None
            };

            dissents.push(crate::revision::Dissent {
                id: dissent_uuid,
                claim,
                resolved_by,
            });
        }

        Ok(Some(crate::revision::Revision {
            revision_id,
            parent_digest: parent_digest.map(crate::revision::Digest),
            content_digest: crate::revision::Digest(content_digest),
            original_proposal,
            current_understanding,
            positions: vec![],
            dissents,
            unresolved_risks: vec![],
            readiness: readiness != 0,
        }))
    }

    pub fn commit_successor_revision(
        &mut self,
        dossier_id: &str,
        parent_revision_id: Option<&str>,
        new_revision: &crate::revision::Revision,
        events: &[crate::event::Event],
    ) -> Result<(), StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;

        // 1. Verify parent revision exists if specified
        if let Some(parent_id) = parent_revision_id {
            let count: i64 = tx.query_row(
                "SELECT COUNT(1) FROM revisions WHERE id = ? AND dossier_id = ?",
                params![parent_id, dossier_id],
                |r| r.get(0),
            )?;
            if count == 0 {
                return Err(StoreError::CorruptState(format!(
                    "Parent revision {} not found",
                    parent_id
                )));
            }
        }

        // 2. Insert events
        let mut stmt_events = tx.prepare(
            "INSERT INTO events (id, dossier_id, timestamp, visibility, payload_type, payload_content, evaluator, reasoning, idempotency_key) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )?;
        for event in events {
            let vis = match event.visibility {
                crate::event::EventVisibility::Public => "public",
                crate::event::EventVisibility::Sealed => "sealed",
            };
            let (p_type, p_content, eval, reason) = match &event.payload {
                crate::event::EventPayload::RawTranscript(c) => {
                    ("raw_transcript", Some(c.as_str()), None, None)
                }
                crate::event::EventPayload::Synthesis(c) => {
                    ("synthesis", Some(c.as_str()), None, None)
                }
                crate::event::EventPayload::PublicRecord(c) => {
                    ("public_record", Some(c.as_str()), None, None)
                }
                crate::event::EventPayload::SealedEvaluation {
                    evaluator,
                    reasoning,
                } => (
                    "sealed_evaluation",
                    None,
                    Some(evaluator.as_str()),
                    Some(reasoning.as_str()),
                ),
            };
            let idempotency_key = event.coordinate.as_ref().map(|c| c.idempotency_key());
            stmt_events.execute(params![
                event.id.to_string(),
                dossier_id,
                event.timestamp,
                vis,
                p_type,
                p_content,
                eval,
                reason,
                idempotency_key
            ])?;
        }
        drop(stmt_events);

        // 3. Verify event references in dissents
        for dissent in &new_revision.dissents {
            if let Some(res) = &dissent.resolved_by {
                for prov in &res.provenance {
                    let event_id_str = prov.event_id.to_string();
                    let count: i64 = tx.query_row(
                        "SELECT COUNT(1) FROM events WHERE id = ?",
                        params![event_id_str],
                        |r| r.get(0),
                    )?;
                    if count == 0 {
                        return Err(StoreError::CorruptState(format!(
                            "Broken event reference in dissent resolution: {}",
                            event_id_str
                        )));
                    }
                }
            }
        }

        // 4. Insert revision
        tx.execute(
            "INSERT INTO revisions (id, dossier_id, parent_digest, content_digest, original_proposal, current_understanding, readiness) 
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                new_revision.revision_id.to_string(),
                dossier_id,
                new_revision.parent_digest.as_ref().map(|d| d.0.clone()),
                new_revision.content_digest.0,
                new_revision.original_proposal,
                new_revision.current_understanding,
                new_revision.readiness as i64
            ],
        )?;

        // 5. Insert dissents and provenance
        let mut stmt_dissents = tx.prepare(
            "INSERT INTO dissents (id, revision_id, claim, resolved_reason) VALUES (?, ?, ?, ?)",
        )?;
        let mut stmt_prov =
            tx.prepare("INSERT INTO resolution_provenance (dissent_id, event_id) VALUES (?, ?)")?;
        for dissent in &new_revision.dissents {
            let reason = dissent.resolved_by.as_ref().map(|r| r.reason.as_str());
            stmt_dissents.execute(params![
                dissent.id.to_string(),
                new_revision.revision_id.to_string(),
                dissent.claim,
                reason
            ])?;
            if let Some(res) = &dissent.resolved_by {
                for prov in &res.provenance {
                    stmt_prov
                        .execute(params![dissent.id.to_string(), prov.event_id.to_string()])?;
                }
            }
        }
        drop(stmt_dissents);
        drop(stmt_prov);

        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dossier_state_persists_across_reopen() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ringi-dossier-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);

        {
            let store = DossierStore::open(&path).expect("open");
            store.insert_dossier("dossier-1", "draft").expect("insert");
        }

        let reopened = DossierStore::open(&path).expect("reopen");
        let state = reopened
            .get_dossier_state("dossier-1")
            .expect("get")
            .unwrap();
        assert_eq!(state, "draft");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn commit_successor_revision_rejects_broken_parent() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "ringi-dossier-commit-1-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        {
            let mut store = DossierStore::open(&path).expect("open");
            store.insert_dossier("dossier-1", "draft").unwrap();

            let revision = crate::revision::Revision {
                revision_id: Uuid::new_v4(),
                parent_digest: None,
                content_digest: crate::revision::Digest("dig".into()),
                original_proposal: "prop".into(),
                current_understanding: "und".into(),
                positions: vec![],
                dissents: vec![],
                unresolved_risks: vec![],
                readiness: false,
            };

            let result = store.commit_successor_revision(
                "dossier-1",
                Some("missing_parent"),
                &revision,
                &[],
            );
            assert!(matches!(result, Err(StoreError::CorruptState(_))));
        }

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn commit_successor_revision_rejects_broken_event_reference() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "ringi-dossier-commit-2-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        {
            let mut store = DossierStore::open(&path).expect("open");
            store.insert_dossier("dossier-1", "draft").unwrap();

            let mut revision = crate::revision::Revision {
                revision_id: Uuid::new_v4(),
                parent_digest: None,
                content_digest: crate::revision::Digest("dig".into()),
                original_proposal: "prop".into(),
                current_understanding: "und".into(),
                positions: vec![],
                dissents: vec![],
                unresolved_risks: vec![],
                readiness: false,
            };

            let dissent_id = Uuid::new_v4();
            let dissent = crate::revision::Dissent {
                id: dissent_id,
                claim: "bad idea".into(),
                resolved_by: Some(crate::revision::Resolution {
                    reason: "fixed".into(),
                    provenance: vec![crate::revision::EventRef {
                        event_id: Uuid::new_v4(), // not in events
                    }],
                }),
            };
            revision.dissents.push(dissent);

            let result = store.commit_successor_revision("dossier-1", None, &revision, &[]);
            assert!(matches!(result, Err(StoreError::CorruptState(_))));

            // Verify that the transaction rolled back: the revision shouldn't be inserted
            let count: i64 = store
                .conn
                .query_row("SELECT COUNT(1) FROM revisions", [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 0);
        }

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn commit_successor_revision_atomic_success() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "ringi-dossier-commit-3-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        {
            let mut store = DossierStore::open(&path).expect("open");
            store.insert_dossier("dossier-1", "draft").unwrap();

            let event = crate::event::Event {
                id: Uuid::new_v4(),
                timestamp: 12345,
                visibility: crate::event::EventVisibility::Public,
                payload: crate::event::EventPayload::PublicRecord("test event".into()),
                coordinate: None,
            };

            let mut revision = crate::revision::Revision {
                revision_id: Uuid::new_v4(),
                parent_digest: None,
                content_digest: crate::revision::Digest("dig".into()),
                original_proposal: "prop".into(),
                current_understanding: "und".into(),
                positions: vec![],
                dissents: vec![],
                unresolved_risks: vec![],
                readiness: false,
            };

            let dissent_id = Uuid::new_v4();
            let dissent = crate::revision::Dissent {
                id: dissent_id,
                claim: "bad idea".into(),
                resolved_by: Some(crate::revision::Resolution {
                    reason: "fixed".into(),
                    provenance: vec![crate::revision::EventRef { event_id: event.id }],
                }),
            };
            revision.dissents.push(dissent);

            let result = store.commit_successor_revision("dossier-1", None, &revision, &[event]);
            assert!(result.is_ok());

            let count: i64 = store
                .conn
                .query_row("SELECT COUNT(1) FROM events", [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 1);
            let count: i64 = store
                .conn
                .query_row("SELECT COUNT(1) FROM dissents", [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 1);
            let count: i64 = store
                .conn
                .query_row("SELECT COUNT(1) FROM resolution_provenance", [], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(count, 1);
        }

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn invocation_coordinate_idempotency_prevents_duplicates() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ringi-dossier-idemp-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);

        {
            let mut store = DossierStore::open(&path).expect("open");
            store.insert_dossier("dossier-1", "draft").unwrap();

            let coord = crate::event::InvocationCoordinate {
                dossier_id: Uuid::new_v4(),
                role: "respondent".into(),
                input_digest: crate::revision::Digest("dig".into()),
                turn: 1,
                attempt: 1,
            };

            let mut event1 = crate::event::Event::new_public(
                crate::event::EventPayload::PublicRecord("1".into()),
                1,
            );
            event1.coordinate = Some(coord.clone());

            let mut revision = crate::revision::Revision {
                revision_id: Uuid::new_v4(),
                parent_digest: None,
                content_digest: crate::revision::Digest("dig2".into()),
                original_proposal: "prop".into(),
                current_understanding: "und".into(),
                positions: vec![],
                dissents: vec![],
                unresolved_risks: vec![],
                readiness: false,
            };

            // First commit succeeds
            let result1 = store.commit_successor_revision("dossier-1", None, &revision, &[event1]);
            assert!(result1.is_ok());

            assert!(store.is_invocation_completed(&coord).unwrap());

            // Second commit with the same coordinate fails with UNIQUE constraint violation
            let mut event2 = crate::event::Event::new_public(
                crate::event::EventPayload::PublicRecord("2".into()),
                2,
            );
            event2.coordinate = Some(coord.clone());
            revision.revision_id = Uuid::new_v4();

            let result2 = store.commit_successor_revision("dossier-1", None, &revision, &[event2]);
            assert!(matches!(
                result2,
                Err(StoreError::Sqlite(rusqlite::Error::SqliteFailure(_, _)))
            ));
        }

        let _ = std::fs::remove_file(&path);
    }
}
