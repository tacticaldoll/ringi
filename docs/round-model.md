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

**Corrected shape (was: "report in-flight from day one").** The original target claimed the
loop should report pending-retry attempts as `CoverageFinding`s *from day one*, before the
Reviewer exists. Reading suunta's actual coverage contract refutes the "day one" part: in a
synchronous, fixed-target loop there is nothing for coverage to do. `CoverageEffect::Covers`
omits a target from the residual **and does not surface it**, so covering a still-pending retry
makes `is_converged` true while the retry is outstanding — premature convergence that silently
drops the retry. And `Supersedes`/`Conflicts` only arise when the plan changes, which requires
a changing target set — i.e. **findings-as-targets (the Reviewer)**. So in-flight reporting has
no honest teeth until the target set can change: it lands **with/after** the Reviewer, not
before. The ② insight itself stands — suunta governs targets, identity governs attempts, and a
pending-retry *is* in-flight; only the sequencing ("day one, before findings") was wrong.

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

## Why this is not mirrorlane redux — bounded-context DDD, with the repo as the boundary

A grounding correction, so the "mirrorlane redux" red lines above are not misread.
`../mirrorlane`'s runtime *spine* was clean: a tiny, generic, domain-free `Step` + a
`Cached<S>: Step` decorator, with a standing genericity guard. **The Step/middleware/Tower
abstraction was not the disease — do not strawman it.**

mirrorlane died of **single-bounded-context domain-driven design**. One semantic frame
(runtime + strategy) was made to carry an unbounded owned domain — projection, skills, experts,
routing, providers — until it collapsed into a god crate. A god crate is precisely the symptom
that *one context's language cannot support the whole problem*.

The family's answer is not the negation of domain-driven design; it is its **mature form**:

- each brick is a **bounded context** — pacta (contract), suunta (navigation), shaahid
  (witness) — its register (see `docs/naming.md`) the *ubiquitous language*;
- the **seam** (`reconcile::seam`) is the *anti-corruption layer* / context map;
- the **repo is the boundary.** A module boundary inside one crate erodes (semantic drift
  precedes architectural drift); a separate published artifact is a *physical* boundary that
  cannot be reached across except through its API. "Compose, do not reimplement" is
  bounded-context DDD enforced by repo separation.

So **"mirrorlane redux" means re-collapsing mechanics into a single owned context** — not
"building an abstraction." The guard: mechanics live in *external, repo-bounded* bricks that
ringi composes; ringi owns only its own thin context (the Agent-CLI domain + the loop). And the
contexts are found by **force-then-extract**, never guessed — mirrorlane's fatal move was
guessing one context boundary (too big) up front.

The red line is therefore **not** "avoid domain modeling." It is: model via bounded contexts;
cut a new context only at a forced seam; keep ringi's own context thin. When a rich, cohesive
sub-domain appears (e.g. a review/critique domain), that is a candidate *repo to extract*, never
a rich model to grow inside ringi.

## Family-level outputs of this audit (BACKLOG sync)

- **Feedback to the family: none for identity.** The attempt-grain fits shaahid unchanged; the
  earlier "drift" worry dissolved. (Recorded so we do not re-raise it.)
- **Vision refinement:** Freigabe / Dychwel / Stoma are **Layer edge-policies on one middleware
  discipline**, not three separate bricks. Update the "ringi forces family growth" section.
- **The retry cluster** (retry Layer + consumer Policy) remains the first Shape-A instance ringi
  is expected to force out over pacta's seam.

---

## The first increment (smallest step toward this target)

Not "build the round loop." And — corrected from an earlier draft — **not the in-flight seam
either**. The in-flight seam has no honest teeth in a synchronous, fixed-target loop (see ②):
covering a pending-retry with `Covers` prematurely converges, and nothing supersedes until the
plan can change. The smallest aligned step is therefore the thing that *makes the plan change*:

1. **The Reviewer runner + findings-as-targets** (①/②). A Builder attempt produces a diff; a
   Reviewer attempt produces findings; each open finding becomes a suunta target ("resolve F1"),
   Unsatisfied until an attempt resolves it and a re-review certifies it Satisfied. This turns
   "reconcile a fixed step set" into "converge on a goal via review findings" — and it is what
   first gives coverage a changing target set to reason about.

Then **the in-flight seam** (②) — *conditional on a real trigger existing*: a pending-retry
attempt reported as in-flight, cancelled via suunta's `Supersedes` when a new finding supersedes
its target. Then **the first retry Layer** (③). Each a separate change, each a step toward this
target.

> **Provenance (two apply-time discoveries).** The in-flight-seam-first ordering was proposed as
> its own change (`report-in-flight-coverage`), then shelved when the `Covers` premature-
> convergence above surfaced — hence findings-as-targets first. After findings-as-targets landed,
> the seam was revisited and deferred **again**: reading the actual loop showed the "real trigger"
> does not yet exist. `drive_build` consumes a failed attempt's retry **within its round** (no
> attempt survives a plan change), and the loop builds **once per round, not once per target**, so
> there is no per-finding pending attempt for coverage to supersede. Building the seam now would be
> the ceremony the fidelity self-audit forbids; it waits for concurrency (phase 5) or a deliberate
> deferred-retry / per-target-attempt restructure. Recorded so it is not re-litigated a third time.

---

## Fidelity self-audit — where this could still drift

- **In-flight reporting could rot into ceremony.** If coverage never actually changes an
  outcome in practice, we must say so and reconsider suunta's depth here — not keep reporting
  in-flight for show. **This guard has fired once**: revisiting the seam after findings-as-targets
  showed the current synchronous, once-per-round loop has no attempt that survives a plan change,
  so the seam was deferred rather than built as decoration (see the first-increment provenance
  and `BACKLOG.md`). Composing suunta's coverage stays honest only once a real trigger is forced.
- **"Aligned shape" of the first Layer could slide into pre-building.** Guard: it must be *one*
  concrete instance with a *placed seam*, never a general trait, until a second forced instance
  exists.
- **Shape A / Shape B could blur.** The moment someone wants suunta/shaahid surfaced-handling
  to "compose like the Layers," stop — that is Shape B leaking into a framework.
