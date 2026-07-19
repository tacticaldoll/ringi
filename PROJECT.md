# Project Contract

Fill this file in during the first project-specific OpenSpec change. Keep it
short and concrete; it is the orientation layer for humans and AI agents.

## Purpose

Describe what this project is for in one or two paragraphs.

## Core Contract

Name the behavior that must be protected first. Examples:

- a data lifecycle that must never lose or duplicate information
- a protocol compatibility promise
- a user-facing workflow that must remain coherent
- a security or privacy invariant

## Terminology

Define project-specific terms here. Prefer one canonical term over synonyms.

## First Project Change

Start each derived project with an OpenSpec change named
`initial-project-shape` unless a more specific first change is clearer.

That change should:

- replace placeholder project metadata
- define the first project-specific specs
- choose the crate layout
- add the real Rust crate or crates
- make the Rust Definition of Done runnable from the workspace root

Before the first real crate exists, Rust build, test, lint, and format commands
are not yet a meaningful Definition of Done. The first project-specific change
is responsible for making them meaningful.

## Change Prioritization

When comparing possible changes, prefer the one that protects the core contract
earliest:

1. Correctness, data integrity, lifecycle safety, and security foundations.
2. Specified feature completeness for concepts already declared in OpenSpec.
3. Operator and developer ergonomics.
4. Scale-out, integrations, and optional platform features.

Do not add scale-out or integration scope merely because a correctness change
enables it. Keep enabling contract changes separate and small.
