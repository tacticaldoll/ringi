# Design

## Context

See proposal.md for motivation. The reconcile/round loop that `reconcile-loop` and
`resumable-runs` describe (`reconcile.rs`, `run.rs`, `verify.rs`, `config.rs`, `ringi run`,
`ringi resume`) was itself removed by the dossier pivot. This change only removes the shaahid
claims from those specs.

## Goals / Non-Goals

**Goals:**
- No shipped spec names shaahid or a guarantee that rested on it.
- Each edited requirement keeps its remaining text unchanged.

**Non-Goals:**
- Retiring the run-era capabilities as a whole. That is a separate decision, left to its own
  change.
- Specifying a replacement exactly-once mechanism.
- Fixing the three pre-existing requirement-less placeholder specs (`arbitration-strategy`,
  `dossier-assembly`, `durable-dossiers`) that make `openspec validate --specs --strict` fail.

## Decisions

- **Remove, do not replace, the exactly-once guarantees.** The code has no shaahid witness and
  its idempotency is the invocation coordinate, which `durable-registry` and
  `resumable-dossiers` already specify. Inventing a replacement for a loop that no longer exists
  would specify code that does not exist.
- **Replace the resume requirement whole.** A MODIFIED delta that omits an existing scenario
  fails `openspec validate --strict` ("omits scenario(s) the current spec still has"). The
  requirement is therefore REMOVED and ADDED under a new name, "An Interrupted Run Resumes From
  Its Recorded Checkpoint", with only the witness-ledger sentence and its scenario dropped. Sync
  places it where the old requirement stood.
- **Edit the Purpose sections at sync.** Deltas cannot carry a Purpose change for an existing
  capability, so sync edits the `reconcile-loop` and `resumable-runs` Purposes directly to drop
  the shaahid sentences.
- **Name cadw in naming-worldview.** `residual_ledger.rs` imports cadw `TargetId`, `Ledger`, and
  `Validator`, so those are real brick vocabulary at a seam.
- **Delete `sync_specs.py` in this change.** It is inert, but it would overwrite specs from a
  missing change directory if run.

## Risks / Trade-offs

- [`openspec validate --specs --strict` still fails after this change] → The failures are the
  three pre-existing empty placeholders, unchanged from main; a follow-up change retires them.
