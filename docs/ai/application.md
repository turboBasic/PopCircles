# Application

Read this when working on the search itself: what the program answers, how it is meant to get there,
how the code doing it is arranged, what must hold for an answer to be right, and whose code may not
be reused doing it.
[`platform.md`](platform.md) wins wherever it and this file would conflict.

## What this program does

Given a share of world population, find the smallest circle **on the globe** that contains it —
smallest by great-circle radius, not by area on a projected map. The input is a ~1 km resolution
equirectangular population raster (30 arc-second grid, 21600 × 43200). Secondary outputs: the most
populous circle of a fixed radius, and rendered maps of the results.

The same question restricted to **one country** is a second supported goal: the smallest circle
containing a share of that country's population, with 100% meaning everyone in it. The circle's
centre may lie outside the country. This is not the smallest circle enclosing a country's landmass —
that is a geometric problem this program does not solve, and conflating the two is a spec error.
Restricting to a country adds boundary data and a mask over the grid; the search itself is the same.
Country identity follows GISCO's published country features and territorial interpretation
([ADR 0015](../decisions/0015-country-identities-follow-gisco-country-features.md)).

Country population queries use masked prefix tables cropped to the selected domain bounds, processed
sequentially ([ADR 0016](../decisions/0016-country-queries-use-cropped-prefixes.md)). This route
serves both repeated country searches and the fixed-circle all-country workload. A native source-cell
footprint assigns its full sanitized population to a GISCO feature only when positive-area contact
qualifies exactly one feature. No match or multiple matches remain in an explicit unassigned residual;
feature order never breaks ties. Classify at k=1 before coarser aggregation, and use the same ownership
for country totals and circle queries ([ADR 0017](../decisions/0017-population-cells-belong-to-a-uniquely-intersected-country.md)).

World-circle commands publish country composition from the registered population raster and GISCO
boundaries. They prepare those inputs once, build one cropped country prefix at a time, and query only
settled circles; a sweep reuses each country prefix across its answers.

"On the globe" is the whole point. A circle drawn on an equirectangular image is not a circle on the
Earth, and the well-known viral maps of the 50% circle got this wrong.

## Provenance and the copying rule

This project is inspired by [alexmijo/PopulationCircles](https://github.com/alexmijo/PopulationCircles),
which solved the same problem in C++ and produced the published maps.

**That repository has no licence.** Default copyright applies, so its source is not ours to reuse:

- **Never copy, paste, transliterate, or line-by-line port** source from that project — or from any
  local checkout of it — into this repo. Mechanical translation of its C++ into Rust is copying.
- **Algorithms and mathematics are free to use.** Summed-area tables, per-latitude circular kernels,
  and binary search over radius are well-known published techniques, and the problem itself has prior
  art (the Valeriepieris circle; Danny Quah's 3300 km result). Implementing a documented technique
  from its description is independent work.
- Practical line: consulting that repo is allowed, and useful — it is the record of what the published
  maps were actually computed with, and answers questions no description does. What may not cross over
  is expression: take a fact, a constant or a convention from it, and write the code itself from the
  reference or from a description of the technique.
- Third-party files vendored upstream carry their own permissive licences and are irrelevant here —
  pull such a dependency from its own origin through Cargo or uv, never from that repo.

A port cannot be un-published, so this is non-negotiable by the test in
`docs/ai-instructions.md` "Non-negotiables": a request to lift upstream code gets the protocol that
section sets out, not a quiet workaround.

## Approach

Steps 1 to 4 and the ground they stand on are one
library crate, `crates/popcircles/`, with `geodesy` holding the earth model, longitude wrapping,
great-circle distance, the angle an arc subtends and the checked radius a circle is asked for,
`grid` holding the raster's geometry — the checked
`Grid`, pixel centres and their inverse, a row's own latitude, cell edges, cell area, and a column
stepped along the seam — `raster` holding the boundary a raster crosses: the
`RasterSource` trait that hands out one row at a time with nodata already turned into zero, the
tallies saying where every cell of a drained raster went, and an in-memory `Synthetic` a later step's
tests are written against instead of a file; `progress` holding the one-method sink a long-running
step reports through; `bracket` holding the guard whose `Drop` closes an expensive step's `debug` pair;
`table` holding the summation table — the padded prefix-sum layout, the
compensated build that streams a raster into it, the rectangle query over a borrowed payload, and the
factor a coarser table folds at; `kernel` holding the spherical cap — the membership rule a span
means, the per-row half width as an offset from a centre column, and the placement that turns one into
the columns a query takes; `circle` holding the fold between the last two — one rectangle per row a
placed kernel names, added in the order it yields them; `search` holding the branch and bound over
candidate centres — the rectangle of centres a bound speaks for, the two-hop slack that bounds the ground
distance across one, and the level loop that prunes a rectangle or halves it; `smallest` holding the
search over radius — the checked share a circle is asked for, the radius at which a circle is the whole
grid, the climb and bisection over whole kilometres, the slack inside which the comparison between two
radii is uncertain, and the ledger seam a resumed run reads; and `report` holding the wire format — the
versioned envelope, the earth model every distance in a document was measured on, the kind a payload
type declares so a consumer branches before reading under `result`, the provenance a document names its
table and its dataset by, and one payload type per question a
command answers, which owns what a consumer of that format needs to know. The build is the
`RasterSource` trait's first caller, the circle is `place`'s, the search is the circle's, and the search
over radius is the search's.
bracket, circle, geodesy, grid, kernel, progress, report, search, smallest, `raster` itself and `table`
itself are pure computation with no I/O — a `log` record is not I/O here, because none reaches a stream until the CLI's own
subscriber writes it;
the file, the decoder and the tag validation are `crates/popcircles/src/raster/geotiff.rs`, the header,
the atomic publication and the mapping are `crates/popcircles/src/table/cache.rs`, the ledger document and
its own publication are `crates/popcircles/src/smallest/cache.rs`, and nothing above any of those modules
names what is inside it.

1. **Summation table.** Convert the raster into a 2D prefix-sum table so the population of any
   axis-aligned pixel rectangle is four lookups. Built once, cached to disk, never committed. At full
   resolution the table is ~7.5 GB (933M cells × 8 bytes), so it is read by mmap rather than held
   resident, and a decimated table (coarser grid, same code path) exists for fast iteration and for
   tests.
2. **Circular kernels.** A circle of a given ground radius covers a different pixel span at each
   latitude, so decompose it into per-row rectangles — a kernel — reusable for every longitude at
   that latitude. Building kernels is the only step that computes geodesic distance.
3. **Most populous circle of a given radius.** Scan the globe at a coarse step, then refine around
   the best candidates, pruning a rectangle of candidate centres by the population of a circle wide
   enough to cover every one of them. The answer is the maximum over the grid's cell centres exactly:
   refinement runs to single cells, the bound is rounded outward and pruning discards no tie, so the
   reported tolerance is zero and what separates it from the truth is the arithmetic beneath it — a
   circle's population is a sum of one query per row, so step 1's 4 ulp per query composes rather than
   carrying over, and step 4's reported slack is that composition. Adversarial input costs time here, not
   accuracy.
4. **Smallest circle for a given population.** A search over integer radius in km driving step 3, with
   every radius tried kept in a ledger so a rerun resumes instead of repeating work. The order climbs
   before it bisects — doubling until one radius reaches the target, then halving the bracket that closed
   — because step 3's strict prune makes a radius covering most of the globe a plateau it refines cell by
   cell. The radius at which a circle is the whole grid is answered by step 1's whole-extent query rather
   than searched, which is what makes a target of the entire population exact rather than a rounding away.
   The answer is the smallest radius reaching the target, reported with the radius below it that did not.
   Where the summation slack cannot separate a probed radius from the target, the result names the span of
   probed radii it cannot separate rather than asserting a minimality it has not proved — a floor on that
   span, since the climb doubles and the radii between two probes were never measured
   ([ADR 0007](../decisions/0007-a-result-states-what-it-could-not-separate.md)).
5. **Rendering.** Python, from the published document and nothing else, kept out of the Rust search
   path entirely — the document is where a *result* comes from, alongside two committed inputs it reads
   for itself, the basemap and the dataset registry. Six modules in the `population_circles` package:
   `circle_document` is the boundary, turning a document into a frozen answer whose geometric circle,
   caption and provenance are separate, and refusing a schema version it does not know, a kind it
   cannot draw, an earth model that is not a sphere, or a document naming no dataset — a figure whose
   credit cannot be read is not drawn;
   `dataset_registry` is the second boundary, turning `data/registry.toml` into the same shape and
   refusing a row whose key is not its file's stem, which is where the attribution a figure owes is read
   from; `circle_geometry` builds the cap and holds the one place a PROJ definition is spelled;
   `map_frame` is what a figure is drawn *in* — the display projection, what it can show, and the
   graticule; `centre_caption` formats the answer's centre for Markdown and figure titles;
   `render_map` is the figure, the only one here that writes anything, and the only one that
   resolves a dataset — by the key the answer carries, never by one of its own. It takes the answer for
   its title and caption, including any published country lines and their accounting credit, and passes
   only its circle to the geometry it draws; `render` is handed the population attribution rather than
   reaching for it. Country captions are measured before the footer area is allocated, so a complete
   list does not overlap the map. A
   circle is **an azimuthal-equidistant buffer handed to PROJ's polygon transform**, never a ring of
   latitudes and longitudes — the ring fills the complement at the antimeridian and the wrong hemisphere
   over a pole, measurably, which is why the buffer's own vertices and the polygon PROJ returns are two
   objects carrying two different assertions
   ([ADR 0008](../decisions/0008-a-circle-is-projected-never-drawn.md)). Three shapes come out of that
   transform, not one, and which is right is settled by how many poles the cap holds: none is a walk that
   closes, one is a walk closed along that pole, and both is the world with the region the cap misses cut
   out of it. The radius a cap is sized on is the document's own `earth_model`, so no Python file names
   the sphere. The basemap is committed (`data/README.md`), so a complete figure needs no network and the
   suite draws one on every run — proved by taking sockets away for the duration rather than by reading
   the imports.

A module per subject, and two crates: the library `crates/popcircles/` and the binary
`crates/popcircles-cli/`. A dependency forced that boundary and is what a further split takes too —
`clap` and `anyhow` live in the CLI's manifest, and the library's may not grow them.
[`platform.md`](platform.md) "Structure" needs nothing for this: `crates/` is already a root
there, and that section carries roots rather than an inventory.

## Architecture

The search is a library; the CLI and the renderer are clients of it. Dependencies point one way —
view depends on results, CLI depends on domain, domain depends on neither — and a change that reverses
an arrow is an architecture change, not a refactor.

Combined execution is coordinated above this boundary
([ADR 0014](../decisions/0014-python-orchestrates-above-the-result-document.md)).

- **Model the domain in types, not primitives.** A latitude, a longitude, a pixel index, a radius in
  km and a population count are five different things, and a bare `f64` or `usize` standing in for
  one is where a wrong-units bug lives undetected. `Grid` is the existing example: the constructor
  rejects the invalid shape, so no later stage revalidates and no caller can build the impossible
  value. Prefer a type whose invalid states do not construct over a check repeated at every use.
- **The domain computes and returns; it does not read, write, print, or format.** A diagnostic emitted
  through the `log` facade is not an exception to that — the record is a value handed to whatever the
  binary installed, and choosing a stream, a level and a format stays the CLI's. geodesy, grid,
  ingest, table, kernels and search take domain types and give back domain types. Paths, file formats,
  stdout, progress reporting and CLI flags are not domain concerns — a module that grows one has taken
  a second responsibility (SRP), and the test that used to be a pure function call now needs a
  filesystem. Where a long-running step must report progress or a raster must be sourced, take the
  sink or the source as a parameter (DIP) so a fixture, a decimated raster and an mmap-backed table
  are the same seam. Keep those abstractions narrow (ISP): the search needs "population of this
  rectangle", not a general raster API.
- **The CLI is a shell.** Parse arguments, resolve paths and datasets, build a config value, call the
  library, serialise the result, map errors to exit codes. Any arithmetic on coordinates or
  populations, any candidate ordering, any tolerance, belongs below it — if the CLI acquires a branch
  the library should have owned, move the branch down rather than duplicating the logic.
- **The view is downstream of results, never of the domain.** Rendering reads the serialised result
  (see [Language split](#language-split)) and knows nothing of the raster, the table or the domain
  types. That serialised shape is the contract between the two halves: extend it additively, and
  treat a renderer that needs a new field as a reason to publish the field, not to reach past the
  boundary.
- **Substitutability is behavioural.** An implementation swapped in behind one of those abstractions —
  a coarser grid, a cached table, a masked raster — must preserve the invariants below, not merely
  typecheck (LSP). OCP is the weakest of the five here: a new dataset, mask or output format should
  arrive as a new implementation of an existing seam, but do not build extension points ahead of a
  second caller.

## Correctness invariants

Properties any numeric change must preserve — geodesy, raster ingest, the summation table, kernels,
the search, and anything that caches their results. Each is a claim a test can pin, and pinning the
invariant matters more than pinning the output: a summation table agrees with a naive sum on small
inputs; a circle's contained population is monotonic in radius. [`platform.md`](platform.md) "Testing"
holds how those tests are organised.

- **Ground distance.** Distances are great-circle arcs on a sphere. `crates/popcircles/src/geodesy.rs`
  states the radius and the formula and is the only place either appears; a second copy anywhere is a
  defect to fix there. **Never treat pixel or degree distance as ground distance, and never measure
  Euclidean in pixel or degree space.** That is a correctness bug, not an approximation: it produces a
  plausible wrong answer rather than a failure, which is why it needs a test and not just a review.
- **Antimeridian and poles.** A circle may wrap longitude or cover a pole. Every raster traversal
  handles wrapping, and a kernel row spanning the full width is a normal case rather than an edge
  case. A traversal correct only away from the seam is not correct.
- **Determinism.** Same raster and same parameters give the same answer, tie-breaks included. Order
  the candidates rather than relying on iteration order, and break ties on a stated rule.
- **Nodata.** Negative or sentinel cells are zero population, converted once at ingest so that no
  later stage has to know a sentinel from a count. `data/registry.toml` holds each dataset's value.
- **Precision.** Populations sum to ~8 × 10⁹ over ~9 × 10⁸ cells; f64 throughout the summation table.
  Narrowing the element type is not the way out of the table's size — that size is a stated cost of
  the approach — and needs a documented error analysis first.
- **Cache invalidation.** A cached table or result file records what raster and what parameters
  produced it, and is rejected rather than silently reused when those differ.
- **Population totals are dataset properties.** Derive the world total from the raster; never hardcode
  a figure taken from elsewhere. `data/README.md` records the measured total as a sanity check, not as
  a value to embed, and anything published from the raster carries the attribution its registry entry
  states.

### Country query handoff

Issues #13 and #14 consume a domain membership rule and answer from that same domain: its total is the
target for shares, and a circle query counts only member cells even when its centre lies outside the
domain. A derived country table is identified by the decoded population values and grid, the resolution
and order in which source cells are aggregated, the exact GISCO snapshot and feature identifier, and
the membership rule that classified those cells. A change to any of them requires a new identity and
a rebuild.

The population source supplies route-specific arithmetic budgets. Candidate bounds include query
error; smallest-target comparisons use the separate total error and predicate slack. Those limits
must follow the route's actual operations and magnitudes, not a measured maximum. The existing
uncertainty reporting remains in force ([ADR 0007](../decisions/0007-a-result-states-what-it-could-not-separate.md)).

A published world circle accounts for countries by querying each uniquely-owned country table at its
settled centre and radius. The composition retains every positive contribution and every country whose
effective-grid domain intersects the circle, orders by contribution then canonical identifier, and
reconciles against the circle's world population. Its remainder is circle-specific; source-wide
unassigned totals are not a substitute. Its bound uses the world table total and each country table total
for the row queries, then covers the reduction of country results.

The k=10 center-point masks in #72 were provisional, and their all-country totals did not reconcile
with the reconstructed world total; the k=1 gap was smaller but remained material ([measurements and
reconciliation](https://github.com/turboBasic/PopulationCircles2026/issues/72#issuecomment-6340314761)).
GISCO membership is now classified from each effective-grid cell center, independently at k=1 and
k=10. A center on a boundary belongs to every matching feature; holes are subtracted, multipart
components are combined, antimeridian crossings are handled, and self-intersecting rings use even-odd
membership on unchanged coordinates. Structurally degenerate components contribute no cells and emit
a feature-specific diagnostic; source feature identities and geometric overlaps remain represented.
The source is not treated as a disjoint partition. Issue #14 still owns the source-aggregation order,
positive-population reconciliation, and the distinction between unassigned population and arithmetic
error.

## Language split

Rust owns everything numeric: geodesy, raster ingest, the table, kernels, the search, the CLI —
including one-off utilities that reuse the reader, so there is no second raster stack to keep
correct. Python owns rendering, figures and combined execution; its coordinator invokes the Rust CLI
and passes the published JSON to the renderer. It reads no raster pixels, cache headers or ledger
contents. A Python helper that touches raster pixels is a sign the work belongs in Rust.
