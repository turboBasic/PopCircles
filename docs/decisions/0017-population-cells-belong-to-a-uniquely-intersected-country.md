---
id: 0017
status: accepted
date: 2026-10-22
scope: data
tags: [adr, popcircles]
---

# ADR 0017 — Population cells belong to a uniquely intersected country

## Decision

A native population cell assigns its full sanitized population to a GISCO country only when its
footprint has positive-area intersection with exactly one feature. Cells with no qualifying feature
or multiple qualifying features remain in an explicit unassigned residual; feature order never breaks
a tie. Classify ownership at k=1 before coarser aggregation, and use the same ownership for country
totals and circle queries.

## Context

Issue #14 found 924,745 positive k=1 cells, totaling 26,859,928.94 population, outside all 263
center-based GISCO masks ([#13 audit](https://github.com/turboBasic/PopCircles/issues/13#issuecomment-6344579983)).
The issue required a rule for those cells; the selected one uses their footprint without changing
the cell-center masks documented in #13. This supersedes ADR 0016's consequence that unclaimed cells
are never assigned; its measured cropped-prefix route remains in force.

## Options

### Unique positive-area feature (SELECTED)

- Adopted because: a cell is assigned only where its footprint intersects one country with positive area.
- Adopted despite: cells outside all features or touching multiple features remain unassigned.

### Keep every cell outside the center masks unassigned

- Rejected because: center membership omits cells whose footprints uniquely intersect a country.
- Rejected despite: it preserves the existing geometric masks without a second ownership rule.

### Fractional cell-to-country overlap

- Rejected because: area fractions assume population is uniform within each source cell.
- Rejected despite: overlap areas could divide boundary cells among countries.

### Population-weight surface

- Rejected because: it requires additional dated data and a separate allocation model.
- Rejected despite: a finer surface could guide allocation within a cell.

## Consequences

Ownership results identify the source, grid and assignment-rule version that produced them. Outside
and multiply intersected positive cells are reported as separate residual components. Reopen this
ruling if a validated finer population surface replaces whole-cell ownership.

## Links

- Issue #14 — policy and reconciliation evidence.
- ADR 0015 — country identities follow GISCO features.
- ADR 0016 — measured cropped-prefix route.
