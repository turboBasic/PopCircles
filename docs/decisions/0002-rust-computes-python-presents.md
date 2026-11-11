---
id: 0002
status: superseded
superseded_by: 0019
date: 2026-08-13
scope: architecture
tags: [adr, popcircles]
---

# ADR 0002 — Rust owns everything numeric; Python owns presentation only

Superseded by [ADR 0019](0019-svelte-presents-the-static-gallery.md).

## Decision

Every numeric step — geodesy, raster ingest, the summation table, kernels, the search and the CLI —
is Rust. Presentation is Python and reads a published result document. The boundary is enforced by
the build: the search is a library crate, the CLI a second crate depending on it, and Python depends
on neither. Nothing crosses the boundary except the document.

## Context

The program answers a geometric question over a 21600 × 43200 population raster: 933 million cells,
a ~7.5 GB derived table, and a search that performs rectangle queries billions of times. The
correctness invariants that make an answer right — great-circle distance on a sphere, f64 throughout
the table, deterministic tie-breaks, antimeridian and pole handling — are all in that half. The other
half draws maps, and the maps are what anyone actually looks at.

The two halves have opposite requirements. The numeric half wants a small, auditable dependency tree
and predictable performance; the presentation half wants the geospatial plotting ecosystem, which
exists in Python and nowhere else. A single language would compromise one of them: Python cannot run
the search in a tolerable time, and Rust has no cartopy.

## Options

### Option 1 (SELECTED): Rust for the search, Python for presentation

- Adopted because: each half gets the ecosystem it needs, and neither pays for the other's.
- Adopted because: the numeric half's dependency tree stays small enough to justify crate by crate,
  which is what makes an audit of the correctness-critical path possible at all.
- Adopted because: a document boundary is testable — a rendering test's fixture is a dictionary, so
  a test that needs raster bytes cannot be written by accident.
- Adopted because: two toolchains are already pinned in `mise.toml`, so the cost is paid once.
- Adopted despite: two languages means two lint stacks, two type checkers and two test runners in
  every CI run, and a contributor has to be fluent in both to change an end-to-end behaviour.
- Adopted despite: a field the renderer needs must be published before it can be used, which is
  slower than reaching into a shared object.

### Option 2: One language throughout

- Rejected because: in Python, a search that is minutes in Rust is hours, and the table's f64
  arithmetic would be at the mercy of whatever numpy does.
- Rejected because: in Rust, the plotting and projection ecosystem does not exist at the maturity
  cartopy and PROJ offer, so the maps would be the compromise instead.
- Rejected despite: one language is genuinely simpler, and the prior art solving this problem was a
  single C++ codebase.

### Option 3: Rust core with Python bindings

- Rejected because: bindings make the renderer a client of the *domain types* rather than of a
  result, so every internal rename becomes a downstream break — the coupling this record exists to
  prevent, in a form the build cannot see.
- Rejected because: it adds a build step and an ABI to a project whose Python half needs neither.
- Rejected despite: it would remove the serialisation round-trip and any duplication of result
  shapes.

## Consequences

- A Python file that touches raster pixels is a defect, not a shortcut. So is a Rust dependency
  whose purpose is to draw something.
- The document is now load-bearing infrastructure rather than an output format. See ADR 0004.
- One-off numeric utilities are Rust even when a Python script would be quicker to write, so that
  there is never a second raster stack to keep correct.
- Reopening this means either the search becoming fast enough to not matter or the Rust plotting
  ecosystem maturing. Neither is close.

## Links

- No issue: the split predates the tracker, and `docs/ai/application.md` "Language split" states the
  rule it leaves behind.
