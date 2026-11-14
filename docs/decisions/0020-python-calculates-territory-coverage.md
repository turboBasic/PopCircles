---
id: 0020
status: accepted
date: 2026-11-14
scope: architecture
tags: [adr, popcircles]
---

# ADR 0020 — Python calculates geometric territory coverage

## Decision

Python calculates `territory_share` from each published circle and the registered GISCO source
geometry. This narrowly supersedes ADR 0014's ban on Python numeric interpretation; Python still does
not read population rasters, tables, or ledgers. Rust omits the field because it has no source geometry.

Transform circle and territory geometry to a Mollweide equal-area CRS built with the result's spherical
radius. GISCO polygon edges are the straight segments between their stored longitude/latitude vertices;
preserve holes and multipart features. Unwrap rings at the antimeridian before splitting them into the
world window. For world coverage, union projected source features before measuring so overlaps count
once. The measured fraction is the intersection area divided by the selected territory area.

Approximate the circle boundary with the existing 720-sided buffer. In its azimuthal-equidistant
construction, every edge lies at most `r * (1 - cos(pi / 720))` inside the spherical cap of radius
`r`. Therefore the missed circle area, and the absolute share error, are bounded by the area of the
spherical annulus of that width divided by the measured territory area. The adapter computes this
bound, capped at one (the largest possible absolute share error), and checks it is finite.

## Context

Issue #302 replaces effective-grid cell membership, which can report zero for visibly intersecting
small territories. The existing Python geometry stack already owns boundary loading and map geometry.

## Consequences

The share's meaning changes while the published field remains stable. Python callers enrich results
before saving or rendering them; documents without measurable source area remain without a share.
Population answers and the standalone Rust search remain independent of this calculation.
