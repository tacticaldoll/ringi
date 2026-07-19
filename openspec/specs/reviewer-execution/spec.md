# reviewer-execution Specification

## Purpose

How ringi scrutinizes a build result: a read-only Reviewer agent, run through the agent seam,
produces structured findings. Findings are quality opinions that become suunta targets to
resolve — never a completion decision and never a permission. Completion is decided by objective
verification (see `verification`), not by the Reviewer.

## Requirements

### Requirement: A Reviewer Agent Produces Findings Through The Agent Seam
Ringi SHALL scrutinize a build result by running a read-only Reviewer agent through the
`AgentAdapter` seam: a Reviewer runner SHALL turn the current state into a Reviewer
`AgentRequest` and run it through an `AgentAdapter`, returning a set of structured findings. The
runner SHALL depend only on the `AgentAdapter` seam, never on a specific CLI, so any adapter
(scripted first, subprocess later) can back it. The Reviewer SHALL NOT edit the workspace.

#### Scenario: A review produces findings
- **WHEN** the loop runs the Reviewer over a build result through the adapter
- **THEN** it returns the review's structured findings without modifying the workspace

#### Scenario: A clean review produces no findings
- **WHEN** the Reviewer finds nothing to raise
- **THEN** it returns an empty set of findings

### Requirement: Reviewer Output Is A Quality Opinion, Never A Permission
Reviewer findings SHALL be treated as quality opinions only: they SHALL become suunta targets to
resolve, and SHALL NOT decide whether the run is complete and SHALL NOT authorize any action.
Completion is decided by objective verification (see `verification`), never by the Reviewer.

#### Scenario: Findings do not complete the run
- **WHEN** the Reviewer returns no findings but the goal is not objectively verified
- **THEN** the run is not complete, because completion is Verify's verdict, not the Reviewer's
