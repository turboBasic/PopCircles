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
radius. GISCO edges are straight segments between stored longitude/latitude vertices; unwrap rings at
the projection seam and densify those segments to at most 0.01 degrees before projecting. Preserve
holes and multipart features. For world coverage, union projected source features before measuring so
overlaps count once. The measured fraction is the intersection area divided by the selected territory
area.

Approximate the circle boundary with the existing 720-sided buffer. In its azimuthal-equidistant
construction, each edge lies at most `r * (1 - cos(pi / 720))` inside the spherical cap of radius `r`;
add the existing 10 m bound for geographic seam and pole cuts. The adapter adds the area difference
between projections at 0.01-degree and 0.005-degree source-segment spacing to the spherical annulus
area, then divides by territory area. This is a checkable bound on the circle approximation's effect
on the share, capped at one (the largest possible absolute share error).

## Context

Issue #302 replaces effective-grid cell membership, which can report zero for visibly intersecting
small territories. The existing Python geometry stack already owns boundary loading and map geometry.

## Consequences

The share's meaning changes while the published field remains stable. Python callers enrich results
before saving or rendering them; documents without measurable source area remain without a share.
Population answers and the standalone Rust search remain independent of this calculation.
