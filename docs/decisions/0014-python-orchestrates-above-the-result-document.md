---
id: 0014
status: accepted
date: 2026-10-21
scope: architecture
tags: [adr, popcircles]
---

# ADR 0014 — Python orchestrates combined execution above the result document

## Decision

Combined search and rendering is coordinated in Python above the published result-document boundary.
The coordinator invokes the Rust CLI and hands its published document to the renderer; it owns no
numeric computation or interpretation of raster, table or ledger contents. Rust builds and runs
independently of the coordinator and the plotting stack.

## Context

Issue #62 asks a checkout user to name a dataset and population share once and receive a figure while
retaining that invocation's result document. The existing search and renderer already work
independently; joining them needs an owner for sequencing and failure reporting, not another numeric
or rendering implementation.

The language split in ADR 0002, the build constraint in ADR 0003 and the document contract in ADR 0004
remain in force. The open question is which caller composes those halves without making a numeric
run require the presentation environment. The checkout audience accepts a combined command that is
unavailable to someone who installed only the standalone binary.

## Options

### Option 1 (SELECTED): Python coordinates above both tools

- Adopted because: the combined caller can use the existing rendering boundary without adding a dependency to Rust.
- Adopted because: one workflow can retain the run's document and identify failures at the search and rendering stages.
- Adopted despite: the combined command requires both toolchains and a checkout, unlike the standalone Rust binary.
- Adopted despite: the caller must follow the CLI's internal command and exit-code conventions as they change.

### Option 2: Rust invokes Python

- Rejected because: the numeric command would acquire interpreter discovery and presentation failure handling.
- Rejected despite: the installed binary would offer the most discoverable entry point for combined execution.

### Option 3: orchestration solely in mise

- Rejected because: document retention and stage-specific failure handling would become a second workflow outside the rendering caller.
- Rejected despite: mise already exposes both tools to checkout users and preserves their dependency direction equally well.

## Consequences

The combined caller may sequence tools but may not reach past the result document to supply rendering
input or reproduce Rust's validation of derived data. Independent search and document rendering remain
usable without that caller. Reversing this dependency requires a new ruling, not a convenience branch
inside the numeric command.

Issue #62's acceptance proof exercises the numeric build and run without a plotting stack; the
document boundary remains covered by the renderer's checked input tests. A one-command experience
for installed users without a checkout would reopen the audience trade.

## Links

- [Issue #62](https://github.com/turboBasic/PopCircles/issues/62) — the combined workflow and its acceptance evidence.
- [ADR 0002](0002-rust-computes-python-presents.md) — the language boundary.
- [ADR 0004](0004-the-published-document-is-the-only-contract.md) — the result contract.
