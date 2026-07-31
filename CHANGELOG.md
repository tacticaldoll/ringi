# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

_0.1.0 is in development; it has not been released._

### Added

- Project shape: the `ringi` app skeleton (a `clap` command surface, stubbed) and the
  self-driving foundation (`PROJECT.md`, `AGENTS.md`, `BACKLOG.md`, OpenSpec scaffolding).
  Behavior is built bet-first — first the minimal composition loop over the pacta family.

### Changed

- Replace the suunta and shaahid `release/0.1.0` Git dependencies with their published 0.1.1
  facade crates. Their public behavior is unchanged; the existing convergence, exactly-once,
  reclaim, restart, and agent-backed composition tests remain the compatibility gate.
- Upgrade pacta from 0.1.2 to 0.2.2 and migrate `SqliteRegistry` to the hardened backend-author
  surface: native atomic claim, lease accessor, and one transactional `apply` port over pacta's
  shared lifecycle decisions. The durable backend now passes sequential and contention conformance,
  including an independent-connection claim fence, with only an additive claim-selection index —
  no table or stored-row-format change — and no reconcile-loop behavior change.
