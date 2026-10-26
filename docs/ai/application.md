# Application

All circle calculations use great-circle distance on a sphere. A circle drawn on an equirectangular map
is not a true circle on Earth; viral maps of the 50% circle illustrate this error. [`platform.md`](platform.md)
governs tooling and repository structure, and takes precedence where its rules conflict with this file.

## What this program does

Given a share of world population, the program finds the smallest circle **on the globe** that contains
it. “Smallest” means the shortest great-circle radius, not the smallest area on a projected map. The
input is an equirectangular population raster with 30 arc-second cells (21600 × 43200), about 1 km wide
at the equator. The program also finds the most populous circle of a fixed radius and renders maps.

For **one country**, the program finds the smallest circle containing a share of that country's
population; 100% means the country's full population. The circle's centre may lie outside the country.
This differs from finding the smallest circle that encloses the country's land. The program does not
solve that geometric problem. Country searches add boundary data and a grid mask, but use the same
search. Country identity follows GISCO's published features and territorial interpretation
([ADR 0015](../decisions/0015-country-identities-follow-gisco-country-features.md)).

Country population queries use masked prefix tables cropped to each country's bounds. Keep only one
country table alive at a time ([ADR 0016](../decisions/0016-country-queries-use-cropped-prefixes.md)).
This supports repeated searches within one country and fixed-circle queries across all countries. A
source cell contributes its sanitized population to a GISCO feature only when its footprint overlaps
exactly one feature by positive area. Cells with no match or multiple matches remain in an explicit
unassigned residual; feature order never breaks ties. Classify ownership at k=1 before aggregating to
coarser grids, and use that ownership for both country totals and circle queries
([ADR 0017](../decisions/0017-population-cells-belong-to-a-uniquely-intersected-country.md)).

World-circle commands report country composition using the registered population raster and GISCO
boundaries. They load the inputs once, build one cropped country prefix at a time, and limit parallel
work within each country. They query only final circles; a sweep reuses each country's prefix for all
its answers.

## Approach

The numerical pipeline lives in `crates/popcircles/`:

- `geodesy` owns the earth model, longitude wrapping, great-circle distance, arc angles, checked radii,
  `LatBand`, and the spherical zone formula.
- `grid` owns raster geometry: validated `Grid` values, pixel centres and their inverse, row latitude,
  cell edges and area, and column traversal across the seam.
- `raster` defines `RasterSource`, which yields one row at a time with nodata converted to zero. It also
  tracks how each cell in a drained raster was classified and provides `Synthetic` for in-memory tests.
- `progress` defines the one-method sink used by long-running steps. `bracket` pairs debug log records
  around an expensive step and closes the pair from `Drop`.
- `table` builds the padded prefix-sum table with compensated summation, queries rectangles from a
  borrowed payload, and folds source cells by the requested decimation factor.
- `kernel` defines spherical-cap membership and places each cap as per-row pixel spans. Its row width
  is an offset from the centre column.
- `circle` sums one rectangle per row in the order given by a placed kernel.
- `search` uses branch and bound over candidate centres. It bounds rectangles of centres, includes the
  two-hop slack needed to bound ground distance, then prunes or subdivides each rectangle.
- `smallest` searches radius in whole kilometres. It validates the requested share, handles a circle
  covering the whole grid, tracks comparison slack, and resumes from a ledger.
- `report` defines the versioned result format: its earth model, result kind, provenance, and one payload
  type for each question. Consumers read the declared kind before the `result` payload.

The call flow is `RasterSource` → table build → placed kernel → circle query → centre search → radius
search. Other modules use these interfaces without inspecting files or cache contents.

`bracket`, `circle`, `geodesy`, `grid`, `kernel`, `progress`, `report`, `search`, `smallest`, `raster`,
and `table` perform no I/O. A `log` call creates a record; the CLI's subscriber decides where to write
it. I/O stays in three modules: `raster/geotiff.rs` reads files, decodes GeoTIFF data, and validates
tags; `table/cache.rs` validates headers, publishes files atomically, and memory-maps tables; and
`smallest/cache.rs` reads and publishes ledger files.

1. **Summation table.** Convert the raster into a 2D prefix-sum table. Four lookups then give the
   population in any axis-aligned pixel rectangle. Build it once, cache it on disk, and never commit
   it. At full resolution the table uses about 7.5 GB (933 million cells × 8 bytes), so memory-map it
   instead of loading it all into memory. A decimated table uses the same code path on a coarser grid
   for fast iteration and tests.
2. **Circular kernels.** A circle's pixel span changes with latitude. Represent it as one rectangle per
   row, then reuse those rectangles at every longitude on that row. Kernel construction is the only step
   that computes geodesic distance.
3. **Most populous circle of a given radius.** Scan cell centres at coarse spacing, then refine around
   the best candidates. For each rectangle of candidate centres, bound the population using a circle
   large enough to cover every centre in that rectangle. Refine to individual cells and round bounds
   outward so pruning cannot discard a tied best result. Refinement reaches individual cells, so the
   search finds the exact maximum over cell centres with zero spatial tolerance. Floating-point
   summation still introduces uncertainty: each circle sums one rectangle query per row, so the 4-ulp
   query error from step 1 accumulates. Step 4 reports that accumulated slack. Adversarial inputs can
   increase runtime while the bounds preserve accuracy.
4. **Smallest circle for a given population.** Search integer-kilometre radii with step 3. Save every
   tested radius in a ledger so a resumed run does not repeat work. First double the radius until one
   reaches the target, then bisect the passing and failing radii. This order avoids the broad plateau
   that a large circle creates when it covers most of the globe. If a circle covers the whole grid,
   answer with step 1's whole-grid sum; this makes a target of the full population exact.

   Report the smallest tested radius that reaches the target and the radius below it that does not. If
   summation error makes a comparison uncertain, report the unresolved radius range. The initial doubling
   leaves untested radii between probes, so the report must include that uncertainty
   ([ADR 0007](../decisions/0007-a-result-states-what-it-could-not-separate.md)).

   The same search works with a `CountryTable`. Its total sets the share denominator, each probe counts
   only cells owned by that country, and candidate centres stay on its effective grid. The smallest
   circle for a 100% target can be smaller than a globe-covering circle if a lower radius already
   reaches the country total. Use the globe-covering ceiling only when no smaller radius reaches that
   total. The ledger records the full country identity so a resumed run cannot reuse another country's
   probes.
5. **Rendering.** Python renders published documents in a separate layer from the Rust search. It reads
   two committed inputs: the basemap and dataset registry. The six modules in `population_circles`
   divide the work:

   - `circle_document` validates a document and turns it into an immutable answer with separate geometry,
     caption, and provenance. It rejects unknown schema versions, unsupported result kinds, non-spherical
     earth models, and documents without a dataset. The renderer needs the dataset to provide the
     required credit.
   - `dataset_registry` reads `data/registry.toml`. It rejects rows whose dataset key does not match the
     file stem and supplies the attribution required for each figure.
   - `circle_geometry` builds the circle cap and owns the only PROJ definition.
   - `map_frame` defines the display projection, visible extent, and graticule.
   - `centre_caption` formats the centre for Markdown and figure titles.
   - `render_map` draws figures and resolves datasets only from keys in the answer. For a country
     result, it selects the outline by canonical ID and checks the boundary revision and digest. It
     derives the map extent from the outline and result circle. Population and boundary credits come
     from registry rows. The renderer measures country captions and credits before allocating footer
     space.

   Build circles as azimuthal-equidistant buffers and pass them to PROJ's polygon transform
   ([ADR 0008](../decisions/0008-a-circle-is-projected-never-drawn.md)). Keep the buffer vertices and
   transformed polygon separate. The buffer defines the circle; the polygon is the geometry PROJ
   returns for drawing. A latitude/longitude ring can fill the wrong side of the world across the
   antimeridian or around a pole. The transform returns one of
   three shapes based on how many poles the cap contains: a closed path around no pole, a path closed
   along one pole, or the whole world with the missed region cut out. Size the cap using the document's
   `earth_model`; Python code must not name the sphere. The committed basemap (`data/README.md`) keeps
   rendering offline. Tests verify this by disabling network access while drawing a figure.

The numerical library is `crates/popcircles/`; the CLI is `crates/popcircles-cli/`. Keep `clap` and
`anyhow` in the CLI manifest; do not add them to the library manifest.

## Architecture

The search library has two clients: the CLI and the renderer. The renderer depends on serialised
results; the CLI depends on the domain; the domain depends on neither client. Reversing one of these
dependencies changes the architecture.

Python coordinates workflows that combine Rust search and Python rendering
([ADR 0014](../decisions/0014-python-orchestrates-above-the-result-document.md)).

- **Use domain types.** Latitude, longitude, pixel index, radius, and population are different
  quantities. A bare `f64` or `usize` can hide a units error. `Row` and `Col` distinguish index axes,
  while grid operations check indices against the receiving grid; an in-range index carries no grid
  identity. Prefer types that cannot represent invalid values over checks repeated at each use.
- **Keep I/O out of the domain.** Domain modules compute and return values; they do not read files,
  write files, print, or format. A `log` call hands a record to the CLI, whose subscriber chooses the
  stream, level, and format. Modules such as `geodesy`, `grid`, `ingest`, `table`, `kernels`, and
  `search` take and return domain types. Keep paths, file formats, stdout, progress reporting, and CLI
  flags outside them. Otherwise, pure function tests need a filesystem and a module gains a second
  responsibility. Pass a narrow progress sink or raster source into long-running steps so tests can
  use fixtures, decimated rasters, and memory-mapped tables through the same interface. Keep those
  interfaces focused: search needs population for a rectangle, not a general raster API.
- **Keep the CLI thin.** Parse arguments, resolve paths and datasets, build a config, call the library,
  serialise the result, and map errors to exit codes. Put coordinate and population arithmetic,
  candidate ordering, and tolerances in the library.
- **Keep rendering downstream of results.** The renderer reads the serialised result (see
  [Language split](#language-split)) and does not access the raster, table, or domain types. Treat the
  serialised result as the contract between Rust and Python. Add fields there when rendering needs
  them; do not let the renderer cross that boundary.
- **Test behaviour at module boundaries.** A coarser grid, cached table, or masked raster must preserve
  the invariants below. A new dataset, mask, or output format should implement an existing interface
  when a real caller needs it. Do not add extension points in advance.

## Correctness invariants

Changes to geodesy, raster ingest, the summation table, kernels, search, and their caches must preserve
these invariants. Tests should pin invariants, not only example outputs. For example,
the summation table must match a naive sum on small inputs, and a circle's population must never decrease
as its radius grows. [`platform.md`](platform.md) "Testing" explains how to organize those tests.

- **Ground distance.** Measure great-circle arcs on a sphere. `crates/popcircles/src/geodesy.rs` owns
  the radius and formula; do not duplicate them. Never treat pixel or degree distance as ground distance
  or calculate Euclidean distance in pixel or degree coordinates. These errors can produce plausible
  wrong answers, so tests must catch them.
- **Antimeridian and poles.** Circles can cross the antimeridian or cover a pole. Every raster traversal
  must handle longitude wrapping; a kernel row spanning the full raster width is a normal case.
- **Determinism.** The same raster and parameters must produce the same answer, including ties. Sort
  candidates and apply a stated tie-breaking rule rather than relying on iteration order.
- **Nodata.** Convert negative or sentinel cells to zero once during ingest. Later stages should not
  distinguish nodata sentinels from population values. `data/registry.toml` records each sentinel.
- **Precision.** Sum about 8 × 10⁹ people across about 9 × 10⁸ cells using `f64` throughout the table.
  A smaller element type would reduce table size but requires a documented error analysis first.
- **Cache invalidation.** Each cached table or result records its source raster and parameters. Reject
  it when either differs from the requested inputs.
- **Population totals belong to datasets.** Calculate the world total from the raster; never hardcode a
  value from another source. The measured total in `data/README.md` is a sanity check. Published results
  must carry the attribution required by the dataset's registry entry.

### Country query requirements

Issues #13 and #14 use the same country membership rule for totals and circle queries. The country
total is the denominator for share targets, and circle queries count only cells owned by that country,
even when the circle centre lies outside it. A derived country table's identity includes the decoded
population values and grid, the resolution and order of source-cell aggregation, the GISCO snapshot and
feature identifier, and the membership rule. Any change to these inputs requires a new identity and a
rebuild.

Set an arithmetic error budget for each route from its operations and population magnitudes. Candidate
bounds include query error. Smallest-circle comparisons also include total error and predicate slack.
Do not derive these limits from an observed maximum. Keep the existing uncertainty reporting
([ADR 0007](../decisions/0007-a-result-states-what-it-could-not-separate.md)).

A published world circle reports its country composition by querying each uniquely-owned country table
at the circle's final centre and radius. Include every positive contribution and every country whose
effective-grid domain intersects the circle. Sort by contribution, then canonical identifier, and
reconcile the sum against the circle's world population. Calculate the remainder for that circle;
source-wide unassigned totals cannot replace it. Bound row queries using the world and country table
totals, then include the error from reducing country results.

The k=10 centre-point masks in #72 were provisional. Their country totals did not reconcile with the
reconstructed world total. The k=1 gap was smaller but still material ([measurements and
reconciliation](https://github.com/turboBasic/PopCircles/issues/72#issuecomment-6340314761)). GISCO
membership is now classified independently from each effective-grid cell centre at k=1 and k=10. A
centre on a boundary belongs to every matching feature. Subtract holes, combine multipart components,
handle antimeridian crossings, and use even-odd membership for self-intersecting rings without changing
coordinates. Degenerate components contribute no cells and produce a feature-specific diagnostic.
Preserve source feature identities and geometric overlaps; GISCO features are not a disjoint partition.
Issue #14 still defines source-cell aggregation order, positive-population reconciliation, and the
difference between unassigned population and arithmetic error.

## Language split

Rust owns numeric work: geodesy, raster ingest, tables, kernels, search, and the CLI. One-off utilities
must reuse Rust's raster reader rather than introduce a second raster stack. Python renders figures
and coordinates combined workflows by invoking the Rust CLI and passing its published JSON to the
renderer. Python must not read raster pixels, cache headers, or ledger contents. Move any Python helper
that needs raster pixels into Rust.
