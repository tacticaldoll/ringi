## ADDED Requirements

### Requirement: Convergence is computed mechanically by suunta

The deliberation loop SHALL derive a dossier's readiness for human decision by composing suunta, and SHALL NOT accept readiness as an output of any agent. Each turn the loop MUST build a suunta `Bearing` whose `Correction`s are the dossier's deliberation targets — every dissent and every risk, each keyed by a stable `Sigil` — and a `Sounding`, then treat `plan_residual(bearing, sounding).is_converged()` as the sole readiness signal. Ringi reports only the per-target verdict; suunta computes the residual and owns the convergence decision. The loop MUST NOT pre-filter satisfied targets out of the `Bearing`, since doing so reimplements convergence instead of composing it. The suunta seam MUST be confined to a dedicated module so its vocabulary does not name any ringi domain type.

#### Scenario: All targets satisfied yields readiness

- **WHEN** the loop evaluates a revision in which every dissent has a provenance-bound resolution and every risk has a provenance-bound resolution
- **THEN** the composed suunta residual reports `is_converged() == true`
- **AND** the loop transitions the dossier to `ReadyForDecision`

#### Scenario: An unmet target withholds readiness

- **WHEN** the loop evaluates a revision with at least one dissent or risk whose verdict is `Unsatisfied`
- **THEN** the composed suunta residual reports `is_converged() == false`
- **AND** the loop continues without transitioning to `ReadyForDecision`

### Requirement: Every target carries an explicit finding

The `Sounding`'s `Fix` SHALL contain exactly one `SatisfactionFinding` for every target in the `Bearing`. A target that is satisfied MUST be reported with an explicit `Satisfied` verdict; it MUST NOT be omitted, because suunta retains an unreported target in the residual and a fully-satisfied dossier would otherwise never converge.

#### Scenario: A satisfied target is reported, not omitted

- **WHEN** a dossier's only dissent has a provenance-bound resolution
- **THEN** the `Fix` contains a `Satisfied` finding for that dissent's `Sigil`
- **AND** the residual is empty and `is_converged() == true`

### Requirement: Unknown never converges

A target whose satisfaction cannot be positively certified SHALL be reported to suunta as `Satisfaction::Unknown`, which retains it in the residual. In v1 the structural mapping certifies only `Satisfied` (a provenance-bound resolution) or `Unsatisfied` (open); `Unknown` is a forward-compatibility guarantee verified at the seam. The loop MUST NOT report an uncertain target as `Satisfied`, so that unknown status can never produce convergence.

#### Scenario: Unknown target blocks convergence

- **WHEN** a `Sounding` reports every target `Satisfied` except one reported `Unknown`
- **THEN** the composed suunta residual reports `is_converged() == false`
- **AND** the dossier is not transitioned to `ReadyForDecision`

### Requirement: Final-turn convergence transitions

The loop SHALL evaluate readiness on the initial revision and on every freshly-produced successor, not only at the start of the next iteration. A revision that converges on the last permitted turn MUST still transition the dossier to `ReadyForDecision`.

#### Scenario: Convergence on the last permitted turn transitions

- **WHEN** a dossier with a turn limit of one converges after that single turn's successor is produced
- **THEN** the dossier transitions to `ReadyForDecision`
- **AND** it is not left in `Deliberating`

### Requirement: A never-deliberated dossier deliberates before it can converge

An un-deliberated initial revision — the root, with no parent — SHALL NOT be treated as converged even when its residual is empty, because an empty residual there means deliberation has not yet occurred rather than that every concern is resolved. Only a revision produced by arbitration (one carrying a parent) may be certified ready before a new turn runs; this is what lets a resumed, already-converged dossier transition without further turns.

#### Scenario: A freshly submitted empty dossier runs a turn before readiness

- **WHEN** a freshly submitted dossier whose initial revision has an empty residual is run
- **THEN** at least one deliberation turn runs before the dossier may transition to `ReadyForDecision`

#### Scenario: A resumed already-converged dossier transitions without a new turn

- **WHEN** a dossier is resumed whose latest revision was produced by arbitration and has an empty residual
- **THEN** it transitions to `ReadyForDecision` without invoking an agent

### Requirement: The arbitrator does not output or store readiness

Readiness SHALL NOT be a field of the arbitration output or of a revision, and no code path may transition a dossier to `ReadyForDecision` on the basis of an agent-authored value. Readiness MUST be recomputed from the residual rather than read from stored state.

#### Scenario: No stored or emitted readiness drives the transition

- **WHEN** an arbitration turn completes and its successor revision is applied and persisted
- **THEN** the successor carries no readiness field
- **AND** the loop computes readiness only from the composed suunta residual
