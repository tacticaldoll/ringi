## MODIFIED Requirements

### Requirement: Ringi Has A Native Naming Register
Ringi SHALL name its own domain in a clear deliberative-governance register (the arc
propose → review → verify → sanction → approve), documented in `docs/domain-language.md`. Brick
vocabulary (pacta `Pact`/`Registry`/`release`, suunta `Bearing`/`Course`, cadw
`TargetId`/`Ledger`/`Validator`) SHALL appear only in the thin seam adapters that call those
crates, and SHALL NOT name ringi's own domain types or modules. Ringi is an application, so
clarity outranks evocativeness.

#### Scenario: Ringi domain names use ringi's register
- **WHEN** a ringi domain type, module, or role is named
- **THEN** it uses ringi's deliberative register, not a brick's term nor a generic queue-runtime term

#### Scenario: Brick terms stay at the seam
- **WHEN** a brick's vocabulary appears in ringi
- **THEN** it appears only in the seam adapters that call that brick, not in ringi's own domain surface
