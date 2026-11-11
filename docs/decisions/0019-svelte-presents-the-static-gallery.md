---
id: 0019
status: accepted
date: 2026-11-11
scope: architecture
tags: [adr, popcircles]
---

# ADR 0019 — Svelte presents the static Gallery

## Decision

Python continues to own population answers, result documents, and map rendering. It generates the
Gallery's data file and invokes a pinned, repository-scoped Svelte build; the resulting static page
presents that generated data. The site remains static and publishes through the existing Pages
handoff.

## Context

ADR 0002 assigns all presentation to Python and restricts Node to hosting CLI tools. The Gallery's
browser interactions are moving to Svelte, while its data and maps remain products of the existing
Python workflow. A reproducible build therefore needs a pinned Node toolchain without moving numeric
or map-rendering work across the established boundaries.

## Options

### Option 1 (SELECTED): compile Svelte as part of the local Gallery build

- Adopted because: the browser page can use Svelte while Python retains ownership of answers, result
  documents, map rendering, and the static publishing handoff.
- Adopted because: pinned repository-scoped tooling makes the compiled page reproducible.
- Adopted despite: contributors now install a Node toolchain alongside Rust and Python dependencies.

### Option 2: keep the Gallery page in Python and Jinja

- Rejected because: it would leave the requested browser presentation migration incomplete.

## Consequences

- Node may host the pinned Svelte compilation toolchain for this static browser presentation.
- Python remains responsible for population results, result documents, and map pixels; Svelte only
  presents generated Gallery data and existing maps.
- The built site remains static and is handed to the existing Pages workflow.

## Links

- Issue #271 — migrate the static Gallery page to Svelte.
- Supersedes [ADR 0002](0002-rust-computes-python-presents.md).
