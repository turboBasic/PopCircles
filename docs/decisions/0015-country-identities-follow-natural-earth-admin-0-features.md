---
id: 0015
status: accepted
date: 2026-10-21
scope: data
tags: [adr, popcircles]
---

# ADR 0015 — Country identities follow Natural Earth Admin-0 country features

## Decision

Each feature in the selected Natural Earth Admin-0 Countries layer is a separate country identity.
Its published classification and default de facto geometry define its extent; features are never
merged by sovereign state or identified by display name.

## Context

Issue #12 had to settle whether a country means a sovereign state, map unit, or a separately published
dependency or disputed feature. Natural Earth distinguishes those concepts in its 1:10m countries and
map-units layers; its vector repository tag v5.1.2 pins the selected input. The 10m countries file has
258 features and 22 `ISO_A3 = -99` values, while the 110m file omits Vatican. The 10m file also has one
invalid Egypt polygon, which downstream geometry handling must surface rather than repair implicitly.

## Options

### Option 1 (SELECTED): Natural Earth 1:10m Admin-0 Countries features

- Adopted because: the public-domain worldwide layer identifies countries, dependencies, and disputed features separately, and its pinned revision can identify all 258 features.
- Adopted because: its default de facto treatment is explicit and has data for Vatican that the 110m layer omits.
- Adopted despite: its cartographic generalization does not promise 30-arcsec cell accuracy, and one polygon is invalid.
- Adopted despite: 22 features have no ISO alpha-3 code and require the source's own feature identity.

### Option 2: geoBoundaries gbOpen ADM0

- Rejected because: it represents countries as they represent themselves and its country files do not specially identify disputed areas; its global composite would supply a different territorial treatment.
- Rejected despite: it is worldwide, CC BY 4.0 compliant, and publishes per-boundary identifiers and source metadata.

### Option 3: GISCO Countries 2024

- Rejected because: its published use terms prohibit commercial use without separate permission from EuroGeographics.
- Rejected despite: the Countries product is worldwide, not Europe-only, and provides a 2024 1:10m GeoJSON product.

### Option 4: OSM-derived boundaries

- Rejected because: its ODbL database attribution and share-alike obligations add licensing conditions this project has not chosen for its country input.
- Rejected despite: it is globally detailed and permits commercial reuse subject to the ODbL.

## Consequences

Country identity follows the pinned feature identifier, including when ISO is absent. Separately
published dependencies and disputed features remain separate; overseas parts included in a source
country feature stay with it. This does not create alternative claims or guarantee a disjoint partition
of populated land. Any later source or territorial-policy change requires a new ruling. Masking must
make invalid geometry handling explicit and must not claim accuracy beyond the input's documented
limits.

## Links

- Issue #12 — candidate evaluation and measured input evidence.
- `docs/ai/application.md` — current country interpretation.
