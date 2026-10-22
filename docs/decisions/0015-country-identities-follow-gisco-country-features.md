---
id: 0015
status: accepted
date: 2026-10-22
scope: data
tags: [adr, popcircles]
---

# ADR 0015 — Country identities follow GISCO country features

## Decision

Each feature in the selected GISCO Countries layer is a separate country identity, keyed by its
source identifier within the pinned snapshot. Its published geometry defines its extent, including
overseas components and separately published unsettled territories. Features are never merged by
sovereignty, split into alternative claims, or identified by display name.

## Context

Country population results need one reproducible answer to what a country includes. The owner
reopened this non-final record for non-commercial use, so commercial-use restrictions no longer
exclude a source. GISCO's worldwide 1:1m layer balances a single downloadable input with finer island
geometry than Natural Earth; its measured snapshot retains 263 uniquely identified features.

This accepts GISCO's interpretation, not a universal de facto rule: its geometry includes Taiwan
within China and Kosovo within Serbia, while sampled Crimean locations belong to Ukraine. Separately
published dependencies and unsettled areas remain separate. The measured comparison and complete
feature inventory are issue #12's evidence, not an inference from ISO codes or recognition status.

## Options

### Option 1 (SELECTED): GISCO Countries features

- Adopted because: a worldwide feature model supplies unique source identifiers and a single, detailed input without assembling national claims.
- Adopted because: non-commercial processing with attribution is permitted, and dependencies and unsettled areas have explicit feature identities.
- Adopted despite: Taiwan and Kosovo are not separate identities, and the source interpretation differs from Natural Earth's de facto treatment.
- Adopted despite: nine selected geometries are invalid, microstates are generalized, and neither map scale nor vertex count proves cell accuracy.
- Adopted despite: data retain non-commercial terms, and the publisher updates version-labelled URLs, requiring an exact snapshot digest.

### Option 2: GADM country features

- Rejected because: the native global delivery and country packages add extraction and conversion work, while redistribution requires permission.
- Rejected despite: non-commercial use is permitted and native sample geometries retain substantially finer microstate detail.

### Option 3: geoBoundaries gbOpen ADM0

- Rejected because: national files need assembly with heterogeneous source licences and territorial coverage; the composite changes disputes and simplifies geometry.
- Rejected despite: pinned boundary identifiers and source metadata support reproducibility, and several measured national geometries are more detailed.

### Option 4: Natural Earth country features

- Rejected because: its cartographic country geometry retains less island detail than the selected layer and poorly represents the measured Vatican extent.
- Rejected despite: its public-domain, tagged GeoJSON is the easiest input to distribute and explicitly models de facto countries and dependencies.

### Option 5: OSM-derived country boundaries

- Rejected because: consistent worldwide country identities and coastline clipping need an extraction pipeline or a commissioned export.
- Rejected despite: detailed geometry, historical snapshots, and commercial reuse are available; ODbL affects derived databases rather than application code.

## Consequences

Source identifiers, including non-ISO unsettled-area codes, remain valid identities. France includes
the overseas components encoded in its feature; Greenland and French Polynesia retain their separate
features. No feature is dropped for lacking an ISO code, and overlaps or unassigned land are not
resolved by input order. This interpretation does not guarantee a partition of populated land.

Masking must explicitly handle invalid geometry without silent repair, and cannot claim precise
microstate membership from generalized polygons. Population reconciliation and aggregation order
remain downstream obligations. Reopen this ruling if those measured limitations prevent the required
accounting, or if a different territorial interpretation is wanted.

## Links

- Issue #12 — non-commercial reassessment, exact snapshot and territorial evidence.
- ADR 0009 — verified input delivery.
