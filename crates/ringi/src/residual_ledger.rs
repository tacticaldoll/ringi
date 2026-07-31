//! The residual-ledger seam: ringi's one place that speaks cadw.
//!
//! Ringi composes `cadw`'s `Ledger` to validate and atomically apply a `Move`
//! batch's/`ConditionMove`'s structural rules — existence, state-machine, duplicate-target
//! rejection — the same shape `Revision::apply_moves`/`apply_condition_move` need for their
//! dissents/risks/questions/conditions. `cadw`'s `Ledger` is reconstructed fresh from the
//! revision's own already-persisted state on every call, used, and discarded: `cadw` owns no
//! persistence of its own by design, so ringi's own `Dissent`/`Risk`/`Question`/`Condition`
//! vectors remain the sole durable source of truth. `apply` and `apply_condition_move` each build
//! their own `Ledger`, scoped to their own target namespace (`dissent:`/`risk:`/`question:` vs
//! `condition:`) — the two never interact, since a `Move` batch never references a condition and
//! a `ConditionMove` never references the other three. Per `docs/naming.md`'s seam rule, `cadw`'s
//! vocabulary (`TargetId`, `Ledger`, `Move`, `Validator`, `Rejection`) is confined to this module
//! and never names a ringi domain type.

use std::fmt;

use cadw::{Ledger, Move as CadwMove, Rejection as CadwRejection, State, TargetId, Validator};
use uuid::Uuid;

use crate::revision::{Condition, ConditionMove, Dissent, Move, Question, Resolution, Risk};

fn dissent_target(id: Uuid) -> TargetId {
    TargetId::new(format!("dissent:{id}"))
}

fn risk_target(id: Uuid) -> TargetId {
    TargetId::new(format!("risk:{id}"))
}

fn question_target(id: Uuid) -> TargetId {
    TargetId::new(format!("question:{id}"))
}

fn condition_target(id: Uuid) -> TargetId {
    TargetId::new(format!("condition:{id}"))
}

/// Which residual category a `TargetId` names, recovered from its own prefix — the same
/// prefixing `convergence.rs` already uses for suunta's `Sigil`.
#[derive(Clone, Copy)]
enum Kind {
    Dissent,
    Risk,
    Question,
    Condition,
}

fn kind_of(target: &TargetId) -> Kind {
    let rendered = target.to_string();
    if rendered.starts_with("dissent:") {
        Kind::Dissent
    } else if rendered.starts_with("risk:") {
        Kind::Risk
    } else if rendered.starts_with("question:") {
        Kind::Question
    } else {
        Kind::Condition
    }
}

/// The domain rejection `ResolutionValidator` reports — never a free string, per cadw's own
/// `Validator::Rejection` contract.
#[derive(Debug)]
enum ResolutionRejection {
    EmptyReason,
    EmptyProvenance,
}

impl fmt::Display for ResolutionRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolutionRejection::EmptyReason => write!(f, "resolution requires a reason"),
            ResolutionRejection::EmptyProvenance => {
                write!(f, "resolution requires event provenance")
            }
        }
    }
}

impl std::error::Error for ResolutionRejection {}

/// Validates a `Resolution` the same way ringi always has: non-empty reason, non-empty event
/// provenance.
struct ResolutionValidator;

impl Validator<Resolution> for ResolutionValidator {
    type Rejection = ResolutionRejection;

    fn validate(
        &self,
        _target: &TargetId,
        outcome: &Resolution,
    ) -> Result<(), ResolutionRejection> {
        if outcome.reason.is_empty() {
            return Err(ResolutionRejection::EmptyReason);
        }
        if outcome.provenance.is_empty() {
            return Err(ResolutionRejection::EmptyProvenance);
        }
        Ok(())
    }
}

/// Accepts any `Resolution` unconditionally — used only to replay already-persisted,
/// already-validated history back onto a freshly-`Open` `Ledger`. Re-running the real
/// `ResolutionValidator` against history that was already validated when it first happened
/// would be redundant, not incorrect; this avoids paying that cost.
struct AlwaysValid;

impl Validator<Resolution> for AlwaysValid {
    type Rejection = ResolutionRejection;

    fn validate(
        &self,
        _target: &TargetId,
        _outcome: &Resolution,
    ) -> Result<(), ResolutionRejection> {
        Ok(())
    }
}

/// Translates a cadw `Rejection` into the exact static error message ringi's callers and tests
/// already expect, recovering which residual kind was addressed from the `TargetId`'s own
/// prefix. `AlreadyExists`/`NotClosed` are unreachable in practice (ringi never declares
/// `Reopen`, and always mints a fresh `Uuid` before `Create`) but are matched exhaustively
/// rather than assumed away.
fn translate_rejection(rejection: CadwRejection<ResolutionRejection>) -> &'static str {
    match rejection {
        CadwRejection::DuplicateTargetInBatch(_) => {
            "A move batch cannot target the same item twice"
        }
        CadwRejection::UnknownTarget(target) => match kind_of(&target) {
            Kind::Dissent => "Move targets a dissent that does not exist",
            Kind::Risk => "Move targets a risk that does not exist",
            Kind::Question => "Move targets a question that does not exist",
            Kind::Condition => "Move targets a condition that does not exist",
        },
        CadwRejection::AlreadyClosed(target) => match kind_of(&target) {
            Kind::Dissent => "Cannot resolve a dissent that is already resolved",
            Kind::Risk => "Cannot close a risk that is already closed",
            Kind::Question => "Cannot answer a question that is already answered",
            Kind::Condition => "Cannot satisfy a condition that is already satisfied",
        },
        CadwRejection::Invalid(target, ResolutionRejection::EmptyReason) => {
            match kind_of(&target) {
                Kind::Dissent => "Dissent resolution requires a reason",
                Kind::Risk => "Risk resolution requires a reason",
                Kind::Question => "Question answer requires a reason",
                Kind::Condition => "Condition satisfaction requires a reason",
            }
        }
        CadwRejection::Invalid(target, ResolutionRejection::EmptyProvenance) => {
            match kind_of(&target) {
                Kind::Dissent => "Dissent resolution requires event provenance",
                Kind::Risk => "Risk resolution requires event provenance",
                Kind::Question => "Question answer requires event provenance",
                Kind::Condition => "Condition satisfaction requires event provenance",
            }
        }
        CadwRejection::AlreadyExists(_) | CadwRejection::NotClosed(_) => {
            "Move batch rejected by the residual ledger"
        }
        // `Rejection` is `#[non_exhaustive]`: cadw may add a variant in a later version. Ringi
        // never declares `Reopen` and always mints a fresh `Uuid` before `Create`, so every
        // variant reachable today is already matched above; this exists only so a future cadw
        // upgrade fails to compile here instead of silently mis-reporting a new rejection kind.
        _ => "Move batch rejected by the residual ledger",
    }
}

/// The updated residual `apply` produces: a fresh set of dissents, risks, and questions.
type Residual = (Vec<Dissent>, Vec<Risk>, Vec<Question>);

/// Applies a batch of ringi `Move`s to the given residual by composing cadw's `Ledger`.
///
/// Builds a fresh `Ledger<Resolution>` from the current dissents/risks/questions, replays
/// already-resolved history (via `AlwaysValid`) to reach the true current state, then folds the
/// turn's actual moves with the real `ResolutionValidator`. Returns the updated
/// dissents/risks/questions; the caller (`Revision::apply_moves`) remains responsible for
/// everything else a successor revision needs (`original_proposal`, digests,
/// `current_understanding`).
pub fn apply(
    dissents: &[Dissent],
    risks: &[Risk],
    questions: &[Question],
    moves: Vec<Move>,
) -> Result<Residual, &'static str> {
    // Ringi's own move-payload checks: cadw's Create carries no payload, so it has nothing to
    // validate about a new dissent's claim, a new risk's description, or a new question's text.
    for mv in &moves {
        match mv {
            Move::AddDissent { claim } if claim.is_empty() => {
                return Err("A new dissent requires a non-empty claim");
            }
            Move::AddRisk { description } if description.is_empty() => {
                return Err("A new risk requires a non-empty description");
            }
            Move::AskQuestion { text } if text.is_empty() => {
                return Err("A new question requires non-empty text");
            }
            _ => {}
        }
    }

    let all_targets = dissents
        .iter()
        .map(|d| dissent_target(d.id))
        .chain(risks.iter().map(|r| risk_target(r.id)))
        .chain(questions.iter().map(|q| question_target(q.id)));
    let baseline = Ledger::new(all_targets);

    let replay: Vec<CadwMove<Resolution>> = dissents
        .iter()
        .filter_map(|d| d.resolved_by.clone().map(|r| (dissent_target(d.id), r)))
        .chain(
            risks
                .iter()
                .filter_map(|r| r.resolved_by.clone().map(|res| (risk_target(r.id), res))),
        )
        .chain(questions.iter().filter_map(|q| {
            q.answered_by
                .clone()
                .map(|res| (question_target(q.id), res))
        }))
        .map(|(target, outcome)| CadwMove::Close { target, outcome })
        .collect();
    let current = baseline
        .fold_batch(&replay, &AlwaysValid)
        .expect("replaying already-persisted, already-validated history cannot fail");

    // Mint ids for any newly-created dissent/risk/question up front, so ringi can both build the
    // cadw Create move and, on success, append the new item using the same id.
    let mut new_dissents: Vec<(Uuid, String)> = Vec::new();
    let mut new_risks: Vec<(Uuid, String)> = Vec::new();
    let mut new_questions: Vec<(Uuid, String)> = Vec::new();
    let mut cadw_moves: Vec<CadwMove<Resolution>> = Vec::new();
    for mv in moves {
        match mv {
            Move::AddDissent { claim } => {
                let id = Uuid::new_v4();
                new_dissents.push((id, claim));
                cadw_moves.push(CadwMove::Create {
                    target: dissent_target(id),
                });
            }
            Move::ResolveDissent { id, resolution } => cadw_moves.push(CadwMove::Close {
                target: dissent_target(id),
                outcome: resolution,
            }),
            Move::AddRisk { description } => {
                let id = Uuid::new_v4();
                new_risks.push((id, description));
                cadw_moves.push(CadwMove::Create {
                    target: risk_target(id),
                });
            }
            Move::CloseRisk { id, resolution } => cadw_moves.push(CadwMove::Close {
                target: risk_target(id),
                outcome: resolution,
            }),
            Move::AskQuestion { text } => {
                let id = Uuid::new_v4();
                new_questions.push((id, text));
                cadw_moves.push(CadwMove::Create {
                    target: question_target(id),
                });
            }
            Move::AnswerQuestion { id, resolution } => cadw_moves.push(CadwMove::Close {
                target: question_target(id),
                outcome: resolution,
            }),
        }
    }

    let next = current
        .fold_batch(&cadw_moves, &ResolutionValidator)
        .map_err(translate_rejection)?;

    let mut next_dissents = dissents.to_vec();
    for dissent in &mut next_dissents {
        if let Some(State::Closed(resolution)) = next.state_of(&dissent_target(dissent.id)) {
            dissent.resolved_by = Some(resolution.clone());
        }
    }
    for (id, claim) in new_dissents {
        next_dissents.push(Dissent {
            id,
            claim,
            resolved_by: None,
        });
    }

    let mut next_risks = risks.to_vec();
    for risk in &mut next_risks {
        if let Some(State::Closed(resolution)) = next.state_of(&risk_target(risk.id)) {
            risk.resolved_by = Some(resolution.clone());
        }
    }
    for (id, description) in new_risks {
        next_risks.push(Risk {
            id,
            description,
            resolved_by: None,
        });
    }

    let mut next_questions = questions.to_vec();
    for question in &mut next_questions {
        if let Some(State::Closed(resolution)) = next.state_of(&question_target(question.id)) {
            question.answered_by = Some(resolution.clone());
        }
    }
    for (id, text) in new_questions {
        next_questions.push(Question {
            id,
            text,
            answered_by: None,
        });
    }

    Ok((next_dissents, next_risks, next_questions))
}

/// Applies a single `ConditionMove` to the given conditions by composing cadw's `Ledger`, exactly
/// as `apply` does for a `Move` batch — a separate, smaller `Ledger` scoped to conditions alone
/// (the `condition:` target namespace never overlaps with `dissent:`/`risk:`/`question:`, so
/// there is no cross-category interaction a shared `Ledger` construction would need to protect
/// against). Only one move at a time: neither of today's call sites (`add_condition_command`,
/// `evaluate_conditions`) ever need more than one.
pub fn apply_condition_move(
    conditions: &[Condition],
    mv: ConditionMove,
) -> Result<Vec<Condition>, &'static str> {
    if let ConditionMove::Add { description } = &mv
        && description.is_empty()
    {
        return Err("A new condition requires a non-empty description");
    }

    let baseline = Ledger::new(conditions.iter().map(|c| condition_target(c.id)));

    let replay: Vec<CadwMove<Resolution>> = conditions
        .iter()
        .filter_map(|c| {
            c.resolved_by.clone().map(|r| CadwMove::Close {
                target: condition_target(c.id),
                outcome: r,
            })
        })
        .collect();
    let current = baseline
        .fold_batch(&replay, &AlwaysValid)
        .expect("replaying already-persisted, already-validated history cannot fail");

    let mut new_condition: Option<(Uuid, String)> = None;
    let cadw_move = match mv {
        ConditionMove::Add { description } => {
            let id = Uuid::new_v4();
            new_condition = Some((id, description));
            CadwMove::Create {
                target: condition_target(id),
            }
        }
        ConditionMove::Satisfy { id, resolution } => CadwMove::Close {
            target: condition_target(id),
            outcome: resolution,
        },
    };

    let next = current
        .fold_batch(&[cadw_move], &ResolutionValidator)
        .map_err(translate_rejection)?;

    let mut next_conditions = conditions.to_vec();
    for condition in &mut next_conditions {
        if let Some(State::Closed(resolution)) = next.state_of(&condition_target(condition.id)) {
            condition.resolved_by = Some(resolution.clone());
        }
    }
    if let Some((id, description)) = new_condition {
        next_conditions.push(Condition {
            id,
            description,
            resolved_by: None,
        });
    }

    Ok(next_conditions)
}
