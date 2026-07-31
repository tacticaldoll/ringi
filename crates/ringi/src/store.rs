//! The durable SQLite store for the dossier domain: `dossiers` (whole-`SubmittedDossier` JSON,
//! including its locked settings and conditions), `revisions`, `dissents`/`risks` with their
//! resolution provenance, and `events`. This is not the pacta claim/lease state — that table and
//! connection live in `registry.rs`, opened separately against the same file.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use uuid::Uuid;

/// The error a store returns.
#[derive(Debug)]
pub enum StoreError {
    /// Persisted dossier data violates a referential-integrity invariant (a broken parent or
    /// event reference).
    CorruptState(String),
    /// An underlying SQLite error.
    Sqlite(rusqlite::Error),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CorruptState(message) => write!(f, "corrupt lifecycle state: {message}"),
            Self::Sqlite(error) => write!(f, "sqlite error: {error}"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CorruptState(_) => None,
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
        "CREATE TABLE IF NOT EXISTS revisions (
                id                    TEXT PRIMARY KEY,
                dossier_id            TEXT NOT NULL,
                parent_digest         TEXT,
                content_digest        TEXT NOT NULL,
                original_proposal     TEXT NOT NULL,
                current_understanding TEXT NOT NULL
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
    // A dissent is carried forward across revisions, each holding its own snapshot, so the
    // logical id is unique only within a revision — the key is (id, revision_id).
    conn.execute(
        "CREATE TABLE IF NOT EXISTS dissents (
                id              TEXT NOT NULL,
                revision_id     TEXT NOT NULL,
                claim           TEXT NOT NULL,
                resolved_reason TEXT,
                PRIMARY KEY (id, revision_id)
            )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS resolution_provenance (
                dissent_id  TEXT NOT NULL,
                revision_id TEXT NOT NULL,
                event_id    TEXT NOT NULL,
                PRIMARY KEY (dissent_id, revision_id, event_id)
            )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS risks (
                id              TEXT NOT NULL,
                revision_id     TEXT NOT NULL,
                description     TEXT NOT NULL,
                resolved_reason TEXT,
                PRIMARY KEY (id, revision_id)
            )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS risk_resolution_provenance (
                risk_id     TEXT NOT NULL,
                revision_id TEXT NOT NULL,
                event_id    TEXT NOT NULL,
                PRIMARY KEY (risk_id, revision_id, event_id)
            )",
        [],
    )?;
    // A question mirrors a risk: carried forward across revisions, its logical id unique only
    // within a revision.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS questions (
                id              TEXT NOT NULL,
                revision_id     TEXT NOT NULL,
                text            TEXT NOT NULL,
                resolved_reason TEXT,
                PRIMARY KEY (id, revision_id)
            )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS question_resolution_provenance (
                question_id TEXT NOT NULL,
                revision_id TEXT NOT NULL,
                event_id    TEXT NOT NULL,
                PRIMARY KEY (question_id, revision_id, event_id)
            )",
        [],
    )?;
    Ok(())
}

/// An event's SQL row shape, shared by every place that inserts one, so the
/// visibility/payload/idempotency-key mapping is written once.
struct EventRow<'a> {
    visibility: &'static str,
    payload_type: &'static str,
    payload_content: Option<&'a str>,
    evaluator: Option<&'a str>,
    reasoning: Option<&'a str>,
    idempotency_key: Option<String>,
}

impl<'a> From<&'a crate::event::Event> for EventRow<'a> {
    fn from(event: &'a crate::event::Event) -> Self {
        let visibility = match event.visibility {
            crate::event::EventVisibility::Public => "public",
            crate::event::EventVisibility::Sealed => "sealed",
        };
        let (payload_type, payload_content, evaluator, reasoning) = match &event.payload {
            crate::event::EventPayload::RawTranscript(c) => {
                ("raw_transcript", Some(c.as_str()), None, None)
            }
            crate::event::EventPayload::Synthesis(c) => ("synthesis", Some(c.as_str()), None, None),
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
        Self {
            visibility,
            payload_type,
            payload_content,
            evaluator,
            reasoning,
            idempotency_key,
        }
    }
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
            "INSERT OR REPLACE INTO dossiers (id, state) VALUES (?, ?)",
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

    /// Every event recorded for `dossier_id`, in commit order. `coordinate` is always `None`:
    /// the stored `idempotency_key` is a one-way string derived from an `InvocationCoordinate`
    /// (see `EventRow`), not a serialization of it, so the original coordinate cannot be
    /// reconstructed — callers needing event *content* (an archive, an audit view) do not need it
    /// back.
    pub fn events_for_dossier(
        &self,
        dossier_id: &str,
    ) -> Result<Vec<crate::event::Event>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, visibility, payload_type, payload_content, evaluator, reasoning
             FROM events WHERE dossier_id = ? ORDER BY _rowid_ ASC",
        )?;
        let rows = stmt.query_map(params![dossier_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })?;

        let mut events = Vec::new();
        for row in rows {
            events.push(Self::parse_event_row(row?)?);
        }
        Ok(events)
    }

    /// The event recorded for `dossier_id` under `coordinate`'s exact `idempotency_key`, if one
    /// has already been persisted — the durable record a retry uses to recover an already-
    /// succeeded invocation's result instead of re-invoking it. `coordinate` is not restored on
    /// the returned event (see `events_for_dossier`'s note); callers already know the coordinate
    /// they looked up.
    pub fn find_event_for_coordinate(
        &self,
        dossier_id: &str,
        coordinate: &crate::event::InvocationCoordinate,
    ) -> Result<Option<crate::event::Event>, StoreError> {
        let key = coordinate.idempotency_key();
        let row = self
            .conn
            .query_row(
                "SELECT id, timestamp, visibility, payload_type, payload_content, evaluator, reasoning
                 FROM events WHERE dossier_id = ? AND idempotency_key = ?",
                params![dossier_id, key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                    ))
                },
            )
            .optional()?;
        row.map(Self::parse_event_row).transpose()
    }

    /// Persists a single event immediately, independent of any revision commit — used to
    /// durably record a turn-step's result (e.g. the respondent's answer) as soon as it succeeds,
    /// so a later step's failure cannot discard it. Shares `EventRow`'s mapping with
    /// `commit_successor_revision`, which still owns the multi-event, revision-atomic case.
    pub fn record_event(
        &mut self,
        dossier_id: &str,
        event: &crate::event::Event,
    ) -> Result<(), StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row = EventRow::from(event);
        tx.execute(
            "INSERT INTO events (id, dossier_id, timestamp, visibility, payload_type, payload_content, evaluator, reasoning, idempotency_key)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                event.id.to_string(),
                dossier_id,
                event.timestamp,
                row.visibility,
                row.payload_type,
                row.payload_content,
                row.evaluator,
                row.reasoning,
                row.idempotency_key
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Parses one `events` row (as selected by `events_for_dossier`/`find_event_for_coordinate`)
    /// into an `Event`. `coordinate` is always `None` — see `events_for_dossier`'s doc.
    fn parse_event_row(
        row: (
            String,
            i64,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    ) -> Result<crate::event::Event, StoreError> {
        let (id, timestamp, visibility, payload_type, payload_content, evaluator, reasoning) = row;
        let id = Uuid::parse_str(&id).map_err(|error| {
            StoreError::CorruptState(format!("event {id} has an invalid UUID: {error}"))
        })?;
        let visibility = match visibility.as_str() {
            "public" => crate::event::EventVisibility::Public,
            "sealed" => crate::event::EventVisibility::Sealed,
            other => {
                return Err(StoreError::CorruptState(format!(
                    "event {id} has an unknown visibility: {other}"
                )));
            }
        };
        let payload = match payload_type.as_str() {
            "raw_transcript" => {
                crate::event::EventPayload::RawTranscript(payload_content.ok_or_else(|| {
                    StoreError::CorruptState(format!("event {id} is missing payload_content"))
                })?)
            }
            "synthesis" => {
                crate::event::EventPayload::Synthesis(payload_content.ok_or_else(|| {
                    StoreError::CorruptState(format!("event {id} is missing payload_content"))
                })?)
            }
            "public_record" => {
                crate::event::EventPayload::PublicRecord(payload_content.ok_or_else(|| {
                    StoreError::CorruptState(format!("event {id} is missing payload_content"))
                })?)
            }
            "sealed_evaluation" => crate::event::EventPayload::SealedEvaluation {
                evaluator: evaluator.ok_or_else(|| {
                    StoreError::CorruptState(format!("event {id} is missing evaluator"))
                })?,
                reasoning: reasoning.ok_or_else(|| {
                    StoreError::CorruptState(format!("event {id} is missing reasoning"))
                })?,
            },
            other => {
                return Err(StoreError::CorruptState(format!(
                    "event {id} has an unknown payload_type: {other}"
                )));
            }
        };
        Ok(crate::event::Event {
            id,
            timestamp: u64::try_from(timestamp).map_err(|_| {
                StoreError::CorruptState(format!("event {id} has a negative timestamp"))
            })?,
            visibility,
            payload,
            coordinate: None,
        })
    }

    pub fn get_latest_revision(
        &self,
        dossier_id: &str,
    ) -> Result<Option<crate::revision::Revision>, StoreError> {
        let row = self
            .conn
            .query_row(
                "SELECT id, parent_digest, content_digest, original_proposal, current_understanding
             FROM revisions WHERE dossier_id = ? ORDER BY _rowid_ DESC LIMIT 1",
                params![dossier_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()?;

        let Some((id_str, parent_digest, content_digest, original_proposal, current_understanding)) =
            row
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
                let mut prov_stmt = self.conn.prepare(
                    "SELECT event_id FROM resolution_provenance WHERE dissent_id = ? AND revision_id = ?",
                )?;
                let prov_iter =
                    prov_stmt.query_map(params![&d_id, &id_str], |row| row.get::<_, String>(0))?;
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

        let mut risks_stmt = self
            .conn
            .prepare("SELECT id, description, resolved_reason FROM risks WHERE revision_id = ?")?;
        let risks_iter = risks_stmt.query_map(params![&id_str], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;

        let mut risks = Vec::new();
        for risk_res in risks_iter {
            let (r_id, description, resolved_reason) = risk_res?;
            let risk_uuid = Uuid::parse_str(&r_id).unwrap_or_default();

            let resolved_by = if let Some(reason) = resolved_reason {
                let mut prov_stmt = self.conn.prepare(
                    "SELECT event_id FROM risk_resolution_provenance WHERE risk_id = ? AND revision_id = ?",
                )?;
                let prov_iter =
                    prov_stmt.query_map(params![&r_id, &id_str], |row| row.get::<_, String>(0))?;
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

            risks.push(crate::revision::Risk {
                id: risk_uuid,
                description,
                resolved_by,
            });
        }

        let mut questions_stmt = self
            .conn
            .prepare("SELECT id, text, resolved_reason FROM questions WHERE revision_id = ?")?;
        let questions_iter = questions_stmt.query_map(params![&id_str], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;

        let mut questions = Vec::new();
        for question_res in questions_iter {
            let (q_id, text, resolved_reason) = question_res?;
            let question_uuid = Uuid::parse_str(&q_id).unwrap_or_default();

            let answered_by = if let Some(reason) = resolved_reason {
                let mut prov_stmt = self.conn.prepare(
                    "SELECT event_id FROM question_resolution_provenance WHERE question_id = ? AND revision_id = ?",
                )?;
                let prov_iter =
                    prov_stmt.query_map(params![&q_id, &id_str], |row| row.get::<_, String>(0))?;
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

            questions.push(crate::revision::Question {
                id: question_uuid,
                text,
                answered_by,
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
            risks,
            questions,
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
            let row = EventRow::from(event);
            stmt_events.execute(params![
                event.id.to_string(),
                dossier_id,
                event.timestamp,
                row.visibility,
                row.payload_type,
                row.payload_content,
                row.evaluator,
                row.reasoning,
                row.idempotency_key
            ])?;
        }
        drop(stmt_events);

        // 3. Verify event references in dissent and risk resolutions
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
        for risk in &new_revision.risks {
            if let Some(res) = &risk.resolved_by {
                for prov in &res.provenance {
                    let event_id_str = prov.event_id.to_string();
                    let count: i64 = tx.query_row(
                        "SELECT COUNT(1) FROM events WHERE id = ?",
                        params![event_id_str],
                        |r| r.get(0),
                    )?;
                    if count == 0 {
                        return Err(StoreError::CorruptState(format!(
                            "Broken event reference in risk resolution: {}",
                            event_id_str
                        )));
                    }
                }
            }
        }
        for question in &new_revision.questions {
            if let Some(res) = &question.answered_by {
                for prov in &res.provenance {
                    let event_id_str = prov.event_id.to_string();
                    let count: i64 = tx.query_row(
                        "SELECT COUNT(1) FROM events WHERE id = ?",
                        params![event_id_str],
                        |r| r.get(0),
                    )?;
                    if count == 0 {
                        return Err(StoreError::CorruptState(format!(
                            "Broken event reference in question answer: {}",
                            event_id_str
                        )));
                    }
                }
            }
        }

        // 4. Insert revision
        tx.execute(
            "INSERT INTO revisions (id, dossier_id, parent_digest, content_digest, original_proposal, current_understanding)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![
                new_revision.revision_id.to_string(),
                dossier_id,
                new_revision.parent_digest.as_ref().map(|d| d.0.clone()),
                new_revision.content_digest.0,
                new_revision.original_proposal,
                new_revision.current_understanding
            ],
        )?;

        // 5. Insert dissents and provenance
        let mut stmt_dissents = tx.prepare(
            "INSERT INTO dissents (id, revision_id, claim, resolved_reason) VALUES (?, ?, ?, ?)",
        )?;
        let mut stmt_prov = tx.prepare(
            "INSERT INTO resolution_provenance (dissent_id, revision_id, event_id) VALUES (?, ?, ?)",
        )?;
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
                    stmt_prov.execute(params![
                        dissent.id.to_string(),
                        new_revision.revision_id.to_string(),
                        prov.event_id.to_string()
                    ])?;
                }
            }
        }
        drop(stmt_dissents);
        drop(stmt_prov);

        // 6. Insert risks and their resolution provenance (mirrors dissents)
        let mut stmt_risks = tx.prepare(
            "INSERT INTO risks (id, revision_id, description, resolved_reason) VALUES (?, ?, ?, ?)",
        )?;
        let mut stmt_risk_prov = tx.prepare(
            "INSERT INTO risk_resolution_provenance (risk_id, revision_id, event_id) VALUES (?, ?, ?)",
        )?;
        for risk in &new_revision.risks {
            let reason = risk.resolved_by.as_ref().map(|r| r.reason.as_str());
            stmt_risks.execute(params![
                risk.id.to_string(),
                new_revision.revision_id.to_string(),
                risk.description,
                reason
            ])?;
            if let Some(res) = &risk.resolved_by {
                for prov in &res.provenance {
                    stmt_risk_prov.execute(params![
                        risk.id.to_string(),
                        new_revision.revision_id.to_string(),
                        prov.event_id.to_string()
                    ])?;
                }
            }
        }
        drop(stmt_risks);
        drop(stmt_risk_prov);

        // 7. Insert questions and their answer provenance (mirrors risks)
        let mut stmt_questions = tx.prepare(
            "INSERT INTO questions (id, revision_id, text, resolved_reason) VALUES (?, ?, ?, ?)",
        )?;
        let mut stmt_question_prov = tx.prepare(
            "INSERT INTO question_resolution_provenance (question_id, revision_id, event_id) VALUES (?, ?, ?)",
        )?;
        for question in &new_revision.questions {
            let reason = question.answered_by.as_ref().map(|r| r.reason.as_str());
            stmt_questions.execute(params![
                question.id.to_string(),
                new_revision.revision_id.to_string(),
                question.text,
                reason
            ])?;
            if let Some(res) = &question.answered_by {
                for prov in &res.provenance {
                    stmt_question_prov.execute(params![
                        question.id.to_string(),
                        new_revision.revision_id.to_string(),
                        prov.event_id.to_string()
                    ])?;
                }
            }
        }
        drop(stmt_questions);
        drop(stmt_question_prov);

        tx.commit()?;
        Ok(())
    }

    /// Atomically persists a condition-evaluation outcome: the dossier's updated state (with
    /// the judged condition's `is_met` flipped, if the verdict was `True`) and the evaluator's
    /// sealed reasoning as one event, in a single transaction.
    pub fn record_condition_evaluation(
        &mut self,
        dossier_id: &str,
        updated_dossier_json: &str,
        event: &crate::event::Event,
    ) -> Result<(), StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;

        tx.execute(
            "INSERT OR REPLACE INTO dossiers (id, state) VALUES (?, ?)",
            params![dossier_id, updated_dossier_json],
        )?;

        let row = EventRow::from(event);
        tx.execute(
            "INSERT INTO events (id, dossier_id, timestamp, visibility, payload_type, payload_content, evaluator, reasoning, idempotency_key)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                event.id.to_string(),
                dossier_id,
                event.timestamp,
                row.visibility,
                row.payload_type,
                row.payload_content,
                row.evaluator,
                row.reasoning,
                row.idempotency_key
            ],
        )?;

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
    fn record_event_is_found_by_its_coordinate_and_not_by_a_different_one() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "ringi-dossier-record-event-{}.sqlite",
            std::process::id()
        ));
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
            let mut event = crate::event::Event::new_public(
                crate::event::EventPayload::PublicRecord("the claim".into()),
                1,
            );
            event.coordinate = Some(coord.clone());

            store.record_event("dossier-1", &event).unwrap();

            let found = store
                .find_event_for_coordinate("dossier-1", &coord)
                .unwrap()
                .expect("event should be found by its own coordinate");
            assert_eq!(
                found.payload,
                crate::event::EventPayload::PublicRecord("the claim".into())
            );

            let other_coord = crate::event::InvocationCoordinate { turn: 2, ..coord };
            assert!(
                store
                    .find_event_for_coordinate("dossier-1", &other_coord)
                    .unwrap()
                    .is_none(),
                "a different coordinate must not match"
            );
        }

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
                risks: vec![],
                questions: vec![],
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
                risks: vec![],
                questions: vec![],
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
                risks: vec![],
                questions: vec![],
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
    fn risks_round_trip_with_id_reason_and_provenance() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ringi-dossier-risks-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let open_risk_id = Uuid::new_v4();
        let closed_risk_id = Uuid::new_v4();
        {
            let mut store = DossierStore::open(&path).expect("open");
            store.insert_dossier("dossier-1", "draft").unwrap();

            let event = crate::event::Event::new_public(
                crate::event::EventPayload::PublicRecord("mitigation evidence".into()),
                7,
            );
            let event_id = event.id;

            let revision = crate::revision::Revision {
                revision_id: Uuid::new_v4(),
                parent_digest: None,
                content_digest: crate::revision::Digest("dig".into()),
                original_proposal: "prop".into(),
                current_understanding: "und".into(),
                positions: vec![],
                dissents: vec![],
                risks: vec![
                    crate::revision::Risk {
                        id: open_risk_id,
                        description: "open risk".into(),
                        resolved_by: None,
                    },
                    crate::revision::Risk {
                        id: closed_risk_id,
                        description: "closed risk".into(),
                        resolved_by: Some(crate::revision::Resolution {
                            reason: "mitigated".into(),
                            provenance: vec![crate::revision::EventRef { event_id }],
                        }),
                    },
                ],
                questions: vec![],
            };

            store
                .commit_successor_revision("dossier-1", None, &revision, &[event])
                .expect("commit");
        }

        let store = DossierStore::open(&path).expect("reopen");
        let reloaded = store
            .get_latest_revision("dossier-1")
            .expect("get")
            .expect("some");

        assert_eq!(reloaded.risks.len(), 2);
        let open = reloaded
            .risks
            .iter()
            .find(|r| r.id == open_risk_id)
            .expect("open risk present");
        assert!(open.resolved_by.is_none());
        let closed = reloaded
            .risks
            .iter()
            .find(|r| r.id == closed_risk_id)
            .expect("closed risk present");
        let res = closed.resolved_by.as_ref().expect("resolved");
        assert_eq!(res.reason, "mitigated");
        assert_eq!(res.provenance.len(), 1);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn questions_round_trip_with_id_reason_and_provenance() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "ringi-dossier-questions-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let open_question_id = Uuid::new_v4();
        let answered_question_id = Uuid::new_v4();
        {
            let mut store = DossierStore::open(&path).expect("open");
            store.insert_dossier("dossier-1", "draft").unwrap();

            let event = crate::event::Event::new_public(
                crate::event::EventPayload::PublicRecord("supplier answer".into()),
                7,
            );
            let event_id = event.id;

            let revision = crate::revision::Revision {
                revision_id: Uuid::new_v4(),
                parent_digest: None,
                content_digest: crate::revision::Digest("dig".into()),
                original_proposal: "prop".into(),
                current_understanding: "und".into(),
                positions: vec![],
                dissents: vec![],
                risks: vec![],
                questions: vec![
                    crate::revision::Question {
                        id: open_question_id,
                        text: "open question".into(),
                        answered_by: None,
                    },
                    crate::revision::Question {
                        id: answered_question_id,
                        text: "answered question".into(),
                        answered_by: Some(crate::revision::Resolution {
                            reason: "Acme Corp".into(),
                            provenance: vec![crate::revision::EventRef { event_id }],
                        }),
                    },
                ],
            };

            store
                .commit_successor_revision("dossier-1", None, &revision, &[event])
                .expect("commit");
        }

        let store = DossierStore::open(&path).expect("reopen");
        let reloaded = store
            .get_latest_revision("dossier-1")
            .expect("get")
            .expect("some");

        assert_eq!(reloaded.questions.len(), 2);
        let open = reloaded
            .questions
            .iter()
            .find(|q| q.id == open_question_id)
            .expect("open question present");
        assert!(open.answered_by.is_none());
        let answered = reloaded
            .questions
            .iter()
            .find(|q| q.id == answered_question_id)
            .expect("answered question present");
        let res = answered.answered_by.as_ref().expect("answered");
        assert_eq!(res.reason, "Acme Corp");
        assert_eq!(res.provenance.len(), 1);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn multi_revision_carries_dissent_forward_and_loads_latest_snapshot() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ringi-dossier-multi-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let dissent_id = Uuid::new_v4();
        {
            let mut store = DossierStore::open(&path).expect("open");
            store.insert_dossier("dossier-1", "draft").unwrap();

            // Revision A: the dissent is open.
            let rev_a = crate::revision::Revision {
                revision_id: Uuid::new_v4(),
                parent_digest: None,
                content_digest: crate::revision::Digest("a".into()),
                original_proposal: "p".into(),
                current_understanding: "u1".into(),
                positions: vec![],
                dissents: vec![crate::revision::Dissent {
                    id: dissent_id,
                    claim: "concern".into(),
                    resolved_by: None,
                }],
                risks: vec![],
                questions: vec![],
            };
            store
                .commit_successor_revision("dossier-1", None, &rev_a, &[])
                .expect("commit A");

            // Revision B carries the SAME dissent id forward, now resolved. With a bare
            // `id` primary key this would collide; the composite key (id, revision_id)
            // stores each snapshot independently.
            let event = crate::event::Event::new_public(
                crate::event::EventPayload::PublicRecord("evidence".into()),
                9,
            );
            let event_id = event.id;
            let rev_b = crate::revision::Revision {
                revision_id: Uuid::new_v4(),
                parent_digest: Some(crate::revision::Digest("a".into())),
                content_digest: crate::revision::Digest("b".into()),
                original_proposal: "p".into(),
                current_understanding: "u2".into(),
                positions: vec![],
                dissents: vec![crate::revision::Dissent {
                    id: dissent_id,
                    claim: "concern".into(),
                    resolved_by: Some(crate::revision::Resolution {
                        reason: "addressed".into(),
                        provenance: vec![crate::revision::EventRef { event_id }],
                    }),
                }],
                risks: vec![],
                questions: vec![],
            };
            store
                .commit_successor_revision(
                    "dossier-1",
                    Some(&rev_a.revision_id.to_string()),
                    &rev_b,
                    &[event],
                )
                .expect("commit B");
        }

        let store = DossierStore::open(&path).expect("reopen");
        let latest = store
            .get_latest_revision("dossier-1")
            .expect("get")
            .expect("some");

        // The latest snapshot is B's: exactly one dissent, resolved with its provenance —
        // not duplicated by A's open copy.
        assert_eq!(latest.current_understanding, "u2");
        assert_eq!(latest.dissents.len(), 1);
        let res = latest.dissents[0].resolved_by.as_ref().expect("resolved");
        assert_eq!(res.provenance.len(), 1);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn commit_successor_revision_rejects_broken_risk_event_reference() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "ringi-dossier-risk-ref-{}.sqlite",
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
                risks: vec![crate::revision::Risk {
                    id: Uuid::new_v4(),
                    description: "heat".into(),
                    resolved_by: Some(crate::revision::Resolution {
                        reason: "mitigated".into(),
                        provenance: vec![crate::revision::EventRef {
                            event_id: Uuid::new_v4(), // not in events
                        }],
                    }),
                }],
                questions: vec![],
            };

            let result = store.commit_successor_revision("dossier-1", None, &revision, &[]);
            assert!(matches!(result, Err(StoreError::CorruptState(_))));

            let count: i64 = store
                .conn
                .query_row("SELECT COUNT(1) FROM revisions", [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 0);
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
                risks: vec![],
                questions: vec![],
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
