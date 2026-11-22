---
id: 0021
status: accepted
date: 2026-11-21
scope: architecture
tags: [adr, popcircles]
---

# ADR 0021 — Python reads terrain rasters for map presentation

## Decision

Python may use Rasterio to read registered terrain rasters for map presentation and Matplotlib to prepare their visual relief. Rust retains population raster access and calculations, cache and ledger contents, and authors published result documents; those documents remain the Rust-to-Python contract. Rust CLI builds and runs without Rasterio or Matplotlib.

## Context

Issue #355 adds relief and water context to Gallery maps while keeping published population answers and circles unchanged. The selected Rasterio and Matplotlib path is bounded to presentation and addresses the terrain reprojection and visual relief needed by the eight approved maps ([issue #355](https://github.com/turboBasic/PopCircles/issues/355)).

The existing Rust build boundary excludes system geospatial prerequisites ([ADR 0003](0003-the-build-takes-no-non-rust-prerequisite.md)); the result document remains the contract between Rust search and Python rendering ([ADR 0004](0004-the-published-document-is-the-only-contract.md)).

## Options

### Option 1 (SELECTED): Rasterio terrain access with Matplotlib presentation

- Adopted because: Rasterio supplies raster reprojection and resampling while Matplotlib supplies presentation relief within Python's existing rendering layer.
- Adopted despite: map rendering now requires a native geospatial Python stack and registered terrain inputs.

### Option 2: Keep all raster access in Rust

- Rejected because: it would move terrain presentation into the population-oriented Rust boundary or require a separate terrain preparation tool.
- Rejected despite: one raster access stack would be easier to distribute and audit.

### Option 3: Use GDAL for terrain and visual relief

- Rejected because: the selected path assigns raster transformation to Rasterio and visual relief to the existing Matplotlib rendering layer.
- Rejected despite: GDAL offers a mature, integrated terrain processing toolset.

## Consequences

Python raster access is limited to registered terrain used to draw maps; the renderer must not open population rasters, cache headers, or ledger contents. Rust remains independently buildable and runnable without the Python rendering stack. Reopen this boundary only if a future requirement needs Python raster access beyond terrain presentation or changes Rust's standalone build constraint.

## Links

- [Issue #355](https://github.com/turboBasic/PopCircles/issues/355) — terrain and map context for Gallery country maps.
- [ADR 0003](0003-the-build-takes-no-non-rust-prerequisite.md) — Rust build prerequisites.
- [ADR 0004](0004-the-published-document-is-the-only-contract.md) — result document boundary.
