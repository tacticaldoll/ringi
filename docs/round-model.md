# Round Model — Vision-Aligned Target (Fidelity Audit)

**Status**: target, not a change. This is the shape every round-model / policy increment steps
toward, so alignment is a matter of *seam placement and shape*, never of pre-building. Read it
before opening a change in this area; each increment must be the smallest step toward this
target.

**The paradox it resolves**: the vision itself forbids "aligning early" by building the general
product (force-then-extract, no central framework, don't pre-build). So *aligning with the
vision* cannot mean "build the middleware product now." It means: **the first concrete instance
sits in the vision's shape, with the extraction seam already placed — so later work is lifting,
not rewriting.** The vision is honored by shape and seam, not by premature generality.

---

## The three layers, top of the decision tree first

The round model is not one decision. It is three, at three layers, and they resolve **upstream
to downstream**. Settle identity first; convergence and disposition fall out of it.

```
   ① identity      (shaahid)  ── what counts as "the same work"?      ← UPSTREAM
        │  decides
   ② convergence   (suunta)   ── when is the run done?
        │  decides
   ③ disposition   (Layers)   ── what to do with what a brick surfaces? ← DOWNSTREAM
```

---

## ① Identity — the exactly-once unit is an *attempt*, not a step-across-rounds

**Decision.** The durable, exactly-once unit is a single **agent invocation (an attempt)**, not
"the step across all its rounds." Each attempt has a stable coordinate identity:

```
   Seal        = <run>:<target>:<round>:<attempt>     (a coordinate, not content)
   Fingerprint = hash(the attempt's input)            (prompt + findings it addresses)
```

**Why this is the vision-aligned choice (and fits shaahid as-is — no feedback needed).**

- Exactly-once matters *per invocation*: a crash between "Builder ran" and "settled" must not
  re-run *that* invocation (it may have already changed the workspace). shaahid guards exactly
  this — a reclaim re-presents the same coordinate → `Attach` → don't re-run.
- Across rounds we *want* to run again (new invocation addressing findings) — genuinely new
  work, new side effects. A new round = a new coordinate = a fresh `Seal` → `Create`. Clean.
- The `DriftedFingerprint` alarm stays *meaningful*: it fires only when the same coordinate
  carries a different input hash — i.e. non-deterministic prompt construction under a fixed
  attempt, a real bug worth catching. `SplitSeal` still catches the same input under two
  coordinates.

**Audit result: the identity model fits shaahid unchanged.** Earlier I flagged "same-step
evolving content" as a possible family feedback (like `release`). On reflection it is *not* — it
was an artifact of picking the wrong unit (step-across-rounds). At the *attempt* grain there is
no drift to work around and no Seal hack (`step#round`) to invent. **Not every stress is a
feedback; this one dissolved.** No change to shaahid is warranted.

**The red line here.** Do not identify a work unit by "the step" and then fight
`DriftedFingerprint` with a synthetic round counter. That is a workaround; the attempt-grain is
the honest model.

---

## ② Convergence — suunta governs *targets*; identity governs *attempts* (different layers)

**Decision.** suunta reasons about **targets** (the desired end state); shaahid/pacta reason
about **attempts** (the work toward them). They are different layers and must not be conflated.

```
   suunta Bearing = desired targets = { goal G } ∪ { open findings as corrections }
        · a finding is a target ("resolve F1"), Unsatisfied until an attempt resolves it
          AND a re-review certifies it Satisfied
        · run converged  ⟺  residual empty  AND  surfaced empty

   pacta + shaahid = execution of attempts against those targets
        · each attempt = a pact (claim/settle) + a deed (exactly-once within the attempt)
```

**The fidelity teeth — and the honesty about them.** suunta has two teeth:

- **Tooth 1 — conservative retention + `UnknownRetained`.** Fires whenever a satisfaction
  verdict is genuinely `Unknown` (reviewer agent times out / cannot decide). Real, but modest.
- **Tooth 2 — coverage (`Supersedes`/`Conflicts`) over in-flight corrections.** This is
  suunta's *distinctive* value, and it stays **dormant in a synchronous, settle-within-a-cycle
  loop.** It wakes only when an in-flight correction outlives a plan change.

**Aligned shape: report in-flight to suunta from day one.** Even while v1 runs one agent at a
time, a **pending-retry** attempt is a claimed-but-unsettled correction — it *is* in-flight.
The loop must report it as a `CoverageFinding` so that when a new finding supersedes a
pending target's work, suunta's `Supersedes` fires and the loop cancels the obsolete work. This
is the one seam that lets Tooth 2 wake without full concurrency.

**Honest fidelity note.** If ringi were to stay forever strictly synchronous *and* never report
in-flight, we would have chosen suunta for a value that never fires — a fidelity failure we
would have to declare. Reporting in-flight is how the choice of suunta stays honest.
Concurrency (parallel builders + mid-flight plan change) is where Tooth 2 fully earns its keep,
so it is not a "phase 5 nicety" — it is what makes composing suunta *true*.

---

## ③ Disposition — policy is an embeddable Layer (the Tower shape), on the execution axis only

**Decision.** The disposition of what a brick surfaces splits into **two shapes**, and only one
of them is a product-shaped thing:

```
  Shape A — execution Layer (wraps the act of executing)     ← the Tower middleware we wanted
      Executor (bare work)
        ⊃ Retry      (pacta breach → retry per Policy)
        ⊃ Approval   (Freigabe: gate before execute)
        ⊃ Circuit    (Stoma: trip on repeated failure)
        ⊃ Compensate (Dychwel: undo on failure)
        ⊃ Idempotency(shaahid: skip if witnessed)
      → composable, reorderable, one concern each; pacta's Middleware<E> is the seed slot

  Shape B — disposition fold (decide over a list of surfaced findings)
      match suunta.surfaced { Superseded(i) => …, Conflicting(i) => … }
      match shaahid.contradictions { DriftedFingerprint => …, SplitSeal => … }
      → NOT a service wrapper. This is policy CONTENT. Forever consumer. A few lines of match.
        Do NOT abstract it into a product.
```

**Vision refinement (record, do not build).** Shape A collapses the family's three planned
future bricks — **Freigabe (gate), Dychwel (compensate), Stoma (circuit)** — into **one Layer
discipline + three edge-policies**. They are all "wrap the execute." So the family's future is
not "three more bricks"; it is "one middleware pattern (mechanism) + a few policies (edge)."
Fewer, thinner, more aligned. This is the good kind of alignment: *seeing clearly so we build
less.*

**Aligned shape for the first Layer.** When it is forced, the first Layer:
- sits on pacta's `Executor`/`Middleware<E>` seam (retry — the most-forced by a real client);
- splits mechanism (how Layers compose / when to retry) from edge-policy (the backoff *content*,
  which stays consumer);
- is sibling-blind at the brick boundary and hand-written as **one concrete instance** — not a
  general trait, not a central engine.

**The red lines.**
- ❌ No general Layer trait / composition engine / central policy runtime that all bricks'
  surfaced streams route through. That is the central framework the vision forbids (mirrorlane
  redux).
- ❌ Do not abstract Shape B. It is policy content and stays consumer `match`.
- ❌ Do not build Freigabe/Dychwel/Stoma. Force them out with real instances first.

---

## Family-level outputs of this audit (BACKLOG sync)

- **Feedback to the family: none for identity.** The attempt-grain fits shaahid unchanged; the
  earlier "drift" worry dissolved. (Recorded so we do not re-raise it.)
- **Vision refinement:** Freigabe / Dychwel / Stoma are **Layer edge-policies on one middleware
  discipline**, not three separate bricks. Update the "ringi forces family growth" section.
- **The retry cluster** (retry Layer + consumer Policy) remains the first Shape-A instance ringi
  is expected to force out over pacta's seam.

---

## The first increment (smallest step toward this target)

Not "build the round loop." The smallest aligned step is **the in-flight seam** (② ), because it
is upstream of disposition and it is what keeps the suunta choice honest:

1. The reconcile loop reports **pending-retry** attempts to suunta as `CoverageFinding`s, and
   acts on `Supersedes`/`Conflicts` by cancelling the obsolete pending work.
2. Acceptance: a step pending retry, when a new target supersedes it, is cancelled via suunta's
   coverage rather than run to completion — Tooth 2 fires in a single-line loop.

Then the Reviewer runner + findings-as-targets (①/②), then the first retry Layer (③) — each a
separate change, each a step toward this target.

---

## Fidelity self-audit — where this could still drift

- **In-flight reporting could rot into ceremony.** If coverage never actually changes an
  outcome in practice, we must say so and reconsider suunta's depth here — not keep reporting
  in-flight for show.
- **"Aligned shape" of the first Layer could slide into pre-building.** Guard: it must be *one*
  concrete instance with a *placed seam*, never a general trait, until a second forced instance
  exists.
- **Shape A / Shape B could blur.** The moment someone wants suunta/shaahid surfaced-handling
  to "compose like the Layers," stop — that is Shape B leaking into a framework.
