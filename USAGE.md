# Usage

Worked commands and their real output, and how to choose the two flags that decide what a run costs.
[`README.md`](README.md) has the overview, the measured result and the release artifacts.

## Distances, grids and tables

### A great-circle distance

Wiesbaden to Rome:

```sh
mise run cli -- distance 50.0782 8.2398 41.9028 12.4964
# {"schema_version":1,"tool":"popcircles","result":{"great_circle_km":966.3013398709427, …}}
```

### Grid geometry

Six numbers describe the image every other command reads — its width and height in cells, the corner it
starts at, and how far one cell steps in each direction — and a wrong sign or a truncated step gives a
plausible wrong answer rather than an error. This command checks those numbers on their own: it refuses a
shape that cannot exist, and says what they mean on the ground, including whether the grid wraps the whole
globe and how large one of its cells is. It takes no file and no time, which is the point — run it before a
table build, not after a search that disagreed with you.

For the GPWv4.11 raster (see [`data/README.md`](data/README.md)), without reading the file itself:

```sh
mise run cli -- grid describe --dataset population-count-2020-30arcsec
# {"schema_version":1,"tool":"popcircles","result":{"middle_row_cell_area_km2":0.8586351267048113, …}}
```

Grid-taking commands accept `--dataset` for registry defaults. Each explicit grid flag overrides its
own default, including zero; the resolved geometry is checked before use. Without `--dataset`, supply
all six flags; no registry is read. For an explicit-only grid:

```sh
mise run cli -- grid describe --width 36 --height 18 \
  --origin-lat 90 --origin-lon -180 \
  --lon-step 10 --lat-step -10
```

### The summation table, and the queries over it

A table over that raster, decimated to 5 arcmin so the cache stays small, and then population queries
answered from it by mmap — the payload is never resident. The `query` function is this file's doing rather
than the CLI's, because the same grid and table go to every query and only the window changes:

```sh
mise run cli -- table build --dataset population-count-2020-30arcsec --decimate 10 --cache out/gpw-5arcmin
# {"schema_version":1, …,"result":{"digest":"0xf17aa802a6890f0c","total_population":7757982599.323671, …}}
```

`table build` reads the raster path and defaults from `data/registry.toml`. It accepts individual
grid overrides, plus `--nodata` and `--epsg`; each supplied value replaces only that registry value.
The source path remains registered, and the GeoTIFF is checked
against the resolved specification before any cache is published.

```sh
query() {
  mise run cli -- table query --dataset population-count-2020-30arcsec \
    --decimate 10 --cache out/gpw-5arcmin "$@";
}

query
# every cell there is
# {…,"rows":{"north":0,"south":2159},"columns":{"west":0,"east":4319,"full_turn":true},"population":7757982599.323671}}

query --north 55.06 --south 47.27 --west 5.87 --east 15.04
# the box around Germany
# {…,"rows":{"north":419,"south":512},"columns":{"west":2230,"east":2340,"full_turn":false},"population":103837251.96947795}}

query --north 52.38 --south 44.39 --west 22.14 --east 40.23
# the box around Ukraine
# {…,"rows":{"north":451,"south":547},"columns":{"west":2425,"east":2642,"full_turn":false},"population":75109401.82679437}}

query --north -12 --south -21 --west 176 --east -178
# Fiji, across the antimeridian
# {…,"rows":{"north":1224,"south":1332},"columns":{"west":4272,"east":24,"full_turn":false},"population":919250.7823575613}}
```

The digest names the cells a table was built from. Cached commands read it from the header; pass
`--digest 0xf17aa802a6890f0c` to assert that expectation and refuse a different table. Omitting it selects
the cache at `--cache` without verifying the source raster or recomputing the decoded digest. The requested
grid and decimation are still checked against the header. The Fiji window wraps
the antimeridian, which needs nothing said about it because `west` is above `east`; the whole extent,
though, is what a query with no window covers, because −180 and 180 reduce to the same column and so
no pair of coordinates can mean the globe.

For an explicit override and digest expectation:

```sh
mise run cli -- table query --dataset population-count-2020-30arcsec --origin-lon -180 \
  --decimate 10 --cache out/gpw-5arcmin --digest 0xf17aa802a6890f0c
```

**A window is a box, not a country.** The middle two are the bounding boxes of Germany and Ukraine,
and each holds a good deal more than the country it is named for: the German box takes in the
Netherlands, Belgium, Czechia, Austria and Switzerland whole, and parts of Poland, France and Denmark.
Counting a country needs a mask over the grid, which is a later step — and treating a box, or a
circle, as a country is the spec error
[the application doc](docs/ai/application.md#what-this-program-does) names.

Both cache files land under `out/`, which is gitignored because a generated table is never committed. A
build killed part way leaves a `.payload.bin.tmp`, and the next build into that `--cache` refuses rather than
deleting it — that file is what stops two builds writing one payload. Remove it and build again.
Building needs the raster, so `mise run data:get` first: it fetches from the
[`data-v1` release](https://github.com/turboBasic/PopulationCircles2026/releases/tag/data-v1), which needs
no account, and verifies what it got against the registry's checksum before placing it. Getting an
independent copy from the publisher instead is the slower route, and is
[`CONTRIBUTING.md`](CONTRIBUTING.md#verifying-a-published-dataset).

## Circles

The four search commands read the same table, so they take the same flags — collected into a shell
function again, and the outputs below are from one run against the 5 arcmin table above:

```sh
search() {
  local command="$1"
  shift
  mise run cli -- "$command" --dataset population-count-2020-30arcsec \
    --decimate 10 --cache out/gpw-5arcmin "$@"
}
```

Circle searches also read the registered settlement places. Fetch it with `mise run data:get` before
searching; if it is missing, circle commands stop with that command in the error. Distance, grid and
table commands do not need the places file.

**Every figure below is a decimated table's.** The 5 arcmin grid is a tenth of the raster's resolution in
each direction, so a radius here is good to about the width of one of its cells — which for half the world
turns out to be no error at all. [Validation](README.md#validation) is where that was checked at full
resolution.

### The population inside a circle you name

A thousand kilometres around Dhaka is a tenth of everyone:

```sh
search population-at --lat 23.8103 --lon 90.4125 --radius-km 1000
# {…,"result":{"requested":{"lat":23.8103,…},"centre":{"lat":23.79166666666667,"lon":90.375},
#  "radius_km":1000.0,"population":769799773.1688497,"share_of_total":0.0992267981157833}}
```

The centre is not the coordinate asked for: it is the centre of the cell containing it, and both are
published because they are different questions.

### The most populous circle of a radius

Found by branch and bound over every cell centre — a good deal further west than the last one, and holding
16% of the world rather than 10%:

```sh
search most-populous --radius-km 1000
# {…,"result":{"centre":{"lat":25.125,"lon":79.70833333333331},"radius_km":1000.0,
#  "population":1254363867.9300776,"share_of_total":0.1616868627727305,"tolerance_persons":0.0,
#  …}}
```

`--spacing` is optional: the library derives an initial block side from this radius and grid. A positive
`--spacing` overrides that choice exactly and changes runtime only; the answer stays the same.

### The smallest circle holding a share

The question the program is named for — here, half the world's population:

```sh
search smallest-for-share --share 50 --ledger out/radii.json
# {…,"result":{"ledger":{"path":"out/radii.json","radii":24},"circle":{"radius_km":3360,
#  "centre":{"lat":28.79166666666667,"lon":100.625},"population":3879165388.019252,
#  "target":{"share":0.5,"persons":3878991299.6618357,"total":7757982599.323671},
#  "short_below":{"radius_km":3359,"population":3878869485.4163485},"covers_whole_grid":false,
#  "predicate_slack_persons":0.01196060136531932,…}}}
```

3360 km, centred in western China. `short_below` is the other end of the bracket the search proved: that
radius was measured too and falls short of the target, so minimality is readable off the document rather
than taken on trust. The share is given in **whole percent**, because a fraction stepped in f64 publishes
`0.30000000000000004` as a third share and a renderer then labels a chart with it.

This answer is separated by 174 088 people against a `predicate_slack_persons` of 0.012, so it carries no
`ambiguity` block. A share whose target sits on a flat stretch — anything near everyone, or a plateau of
ocean — grows one, naming the probed radii the arithmetic cannot tell apart, and the run says so once on
stderr as well.

### A sweep of shares over one ledger

Every radius tried goes in the ledger, so a sweep of several shares pays for each radius once. Here the
50% record costs no search at all, because the run above already settled its radii:

```sh
search sweep --from 10 --to 50 --step 20 --ledger out/radii.json
# {…,"result":{"ledger":{"path":"out/radii.json","radii":43},"shares":{"from_percent":10,"to_percent":50,
#  "step_percent":20},"records":[
#   {"radius_km":702,…,"target":{"share":0.1,…},"stats":{"radii_evaluated":9,"radii_reused":11,…}},
#   {"radius_km":2129,…,"target":{"share":0.3,…},"stats":{"radii_evaluated":10,"radii_reused":14,…}},
#   {"radius_km":3360,…,"target":{"share":0.5,…},"stats":{"radii_evaluated":0,"radii_reused":24,…}}]}}
```

`records` ascend by requested share, which is part of the format rather than an accident of iteration.
A ledger describing another table is refused rather than resumed from, which is why there is no way to
turn it off.

## Maps

From the checkout root, one command searches a named dataset and saves its map:

```sh
mise run data:get   # fetch the source raster explicitly before the first build
mise run map -- --dataset population-count-2020-30arcsec --share 50 \
  --decimate 10 --cache out/gpw-5arcmin --output out/half-world.png
```

`map` checks and reuses a compatible cache, even if the source raster has since been removed. An
absent or incompatible cache is built from the named raster; a corrupt or unreadable cache stops the
run. No data is downloaded implicitly. Omit `--decimate` for full resolution; `--spacing` overrides
Rust's automatic search spacing.

The image's sibling `out/half-world.json` contains this invocation's search stdout. `--document`
selects another path. The default radii ledger is beside that document, with a `.radii.json` suffix;
`--ledger` selects another. Image, document and ledger paths must differ. The current JSON is saved
before rendering and retained if document validation or rendering fails, with its path in the error.
A failed search or document write leaves an existing document untouched and draws no new image.

Launch failures retain uv/mise's own interpreter diagnostic. Application errors name the cache probe,
table build, search, document publication or rendering stage and preserve child diagnostics. A missing
raster names `mise run data:get`; a rejected document names the saved JSON. On success, both image
and document destinations are printed. This combined command requires the checkout and its Python
plotting environment; the Rust search remains independently usable.

The tools also work separately. Rendering reads the published search document, basemap and registry
without opening the raster, table or ledger:

```sh
search most-populous --radius-km 1000 > out/most-populous.json
mise run render -- --input out/most-populous.json --output out/most-populous.png
mise run render -- --input out/most-populous.json --output out/globe.png --projection orthographic
```

**The documents this project publishes its answers as are in the tree**, under
[`results/`](results/), so
`mise run render -- --input results/world-most-populous-1000km-5arcmin.json --output out/map.png` draws a
figure on a clone that has fetched no raster at all. `mise run results:build` regenerates them.

`--projection` takes `plate-carree`, which is what the viral maps used, or `orthographic` centred on the
circle. Every figure carries the CC BY citation the raster's licence requires, and a test fails if that
wording drifts from [`data/README.md`](data/README.md#population-count-2020-30arcsec).

**The circle is a spherical cap projected by PROJ, not a ring of coordinates.** So one crossing the
antimeridian comes out in two pieces at either edge of the map, and one covering a pole closes across the
top of it — where drawing the ring directly fills the complement in the first case and the wrong
hemisphere in the second. Both of those look like maps, which is why they are tested rather than eyeballed.
A circle wide enough to hold both poles is the third case, and it comes out as the whole map with the one
region it misses cut out.

**Nothing here reaches the network.** The coastlines are Natural Earth 110m, committed under
[`data/boundaries/`](data/README.md), so a figure is the same on a fresh clone as on a warm one and CI
draws complete ones on every pull request. `--no-coastlines` draws the cap over a bare graticule, which is
a choice about the figure rather than a way to avoid a download.

Circle documents carry their nearest settlement, so a figure title can name it without opening the
fetched places file. Markdown centre fragments use the same decision and link the circle's centre to
OpenStreetMap. After `mise run data:get` and a circle search, format a document with:

```sh
uv run python -m population_circles.centre_caption --input results/world-half-30arcsec.json
# [28.84 N, 100.66 E](https://www.openstreetmap.org/?mlat=28.8375&mlon=100.66250000000002) (192 km from Xichang, China)
```

Older documents and centres without a nearby settlement still produce a linked coordinate. Rendering
and formatting need the document, basemap and dataset registry; they do not need the settlement file.

## Watching a run

Every command takes `--log-level`, the only control over what a run says about itself: `error`, `warn`,
`info` or `debug`, and `info` unless you say otherwise. At `info` a run names the table it resolved and the
answer it reached; at `debug` each expensive step is bracketed by a begin and an end record carrying one
operation name, so its duration is the difference between the two elapsed figures on the left. `RUST_LOG`
does nothing here — the flag is the only way in.

The progress meter is a **second mechanism, and the flag does not govern it.** A log says what happened;
the meter says how far a run has got. So `--log-level error` still draws a meter, and a `debug` run piped
to a file draws none — the meter is silent when stderr is not a terminal. Both write to stderr, and
stdout stays exactly one JSON document at every level.

`mise run cli -- --help` has the full command and flag reference.

## Choosing the inputs

**Choose the resolution with `--decimate`.** It coarsens the image before searching — bigger cells,
fewer of them, so everything is faster and the circle's centre lands less precisely. Initial spacing is
automatic; `--spacing` optionally overrides how coarsely the first pass sweeps for candidate centres and
changes runtime only.

**Build at `--decimate 2` and search without a spacing flag.** That resolution gives the same radius as
the full raster, a centre within a kilometre of it, and a 1.8 GB table instead of 7.0 GB.

The rest of this section explains what costs time, what limits precision, the measured numbers, and when
to override spacing.

### The five words you need

- **Raster** — the population data as a giant image. One pixel ("cell") holds how many people live in it.
  The full one is 43200 × 21600 cells, about 933 million of them, each roughly 1 km across at the equator.
- **Decimation** — coarsening that image before searching, by averaging blocks of cells together.
  `--decimate 10` turns 10 × 10 cells into one, leaving a 4320 × 2160 image. Fewer cells, faster
  everything, blurrier answer.
- **Summation table** — a running-totals table built once from the raster. It is what makes the search
  possible at all: with it, the population of *any* rectangle is four lookups instead of adding up a
  million cells. It costs 8 bytes per cell on disk, which is where the sizes below come from.
- **Spacing** — how coarsely the search sweeps the globe on its first pass. The program tests candidate
  circle centres in square blocks of this many cells, then splits the promising blocks and looks closer.
- **Pruning** — throwing away a whole block without testing the centres inside it. The program can prove
  that no centre in a block can beat the best one found so far, so it skips all of them. This is what
  makes the search finish; without it you would test 933 million centres one at a time.

### What costs time

Three things, in order of how much they matter:

1. **Whether the summation table fits in RAM.** This is a cliff, not a slope. Below it the program reads
   the table at memory speed; above it every lookup may wait for the disk, and the search spends **94% of
   its time waiting** rather than calculating. On a 16 GB machine the 7 GB full-resolution table is over
   the cliff and the 1.8 GB one is comfortably under it — which is the whole reason the recommendation
   above is not "use everything".
2. **How many cells the image has.** Four times the cells is roughly four times the search, as long as you
   stay under the cliff.
3. **The radius you are looking for.** A big circle covers more rows of the image, and the program does one
   rectangle lookup per row. A 3300 km circle spans 356 rows of the 5 arcmin image but 7130 rows of the
   full one.

Spacing changes the work the search does, never its answer. The automatic choice and the earlier
fixed-spacing measurements are described below.

### What limits precision

Also three things, and the surprise is that only one of them matters here:

- **The radius is reported in whole kilometres**, because that is the step the search takes. You get a
  proved bracket rather than an estimate: "3360 km holds half, 3359 km does not", both actually computed.
- **The centre is the best cell, not the best point.** Nothing between cell centres is ever tested, so a
  coarser image can only place the centre within one of its own cells.
- **Arithmetic error is irrelevant.** Adding billions of numbers loses a little precision, and the program
  publishes exactly how much: **0.06 of a person** out of 3.9 billion at 1 arcmin. That is eleven correct
  significant figures. It is never the thing limiting your answer.

Historical measurements on the real raster with explicit spacing, for half the world's population — every
row an actual run, end to end, on a 16 GB M2 Pro (6 performance and 4 efficiency cores, internal SSD) under
macOS 26.6:

| `--decimate` | Cell size | Table | Build | Search | Radius | Centre error |
| --- | --- | --- | --- | --- | --- | --- |
| 10 | 9.3 km | 71 MB | 11 s | 0.9 s | 3360 km | 6.3 km |
| 4 | 3.7 km | 445 MB | 15 s | 5.0 s | 3360 km | 1.8 km |
| **2** | **1.9 km** | **1.8 GB** | **16 s** | **18 s** | **3360 km** | **0.6 km** |
| 1 | 0.9 km | 7.0 GB | 21 s | 78 min | 3360 km | — (the reference) |

The full-resolution row is 24 probed radii at 100–275 s each, and
[`results/world-half-30arcsec.json`](results/world-half-30arcsec.json) is the document it printed. A sweep
is cheaper than its shares suggest: ten of them over one ledger cost 7.0 s at `--decimate 10`, because 28
radii answer all ten.

**All four agree on the radius.** Resolution buys you centre placement and nothing else, and it buys it at
about 4× the cost per halving of the cell. The last step — 1 arcmin to full — costs roughly 250× the time
for half a kilometre of centre, because it is the step that goes over the RAM cliff.

### Automatic spacing and overrides

Omit `--spacing` for normal searches. For each radius the library chooses the largest positive block side
whose conservative distance bound, added to that radius with outward rounding, stays strictly below half
the sphere's circumference. The side is capped at the grid's maximum dimension. The bound uses the grid's
angular steps and assumes the equator's widest longitude distance, so the choice depends on both the radius
and resolution rather than a fitted fraction of the image width.

At or above half the circumference, no block side can satisfy that strict ceiling. Automatic mode uses the
grid's maximum dimension instead, producing one initial block. Below the ceiling it always remains
positive, even when only single-cell blocks fit. A smallest-circle search derives the side separately at
each radius it probes.

Use a positive `--spacing N` to reproduce or investigate a particular initial tiling. The override is used
exactly, including values beyond the automatic ceiling. One means an exhaustive scan of cell centres;
blocks whose widened radius covers the sphere cannot prune at that level. Spacing changes runtime, never
the answer, and ledgers can be reused across automatic and explicit runs.

The earlier fixed-spacing sweep at 3300 km on the 4320-wide image counted these circles:

| `--spacing` | Circles tested |
| --- | --- |
| 8 | 4447 |
| 32 | 910 |
| 128 | 424 |
| 256 | 390 |
| 1024 | 371 |
| 4319 | 379 |

In that sweep the *percentage* of blocks pruned fell the whole way, from 97% to 77%, while
the program got faster. A fine first pass prunes nearly everything it looks at — but only because it
created so many blocks to look at. Percentage pruned alone does not measure the work; tested circles and
constructed kernels both contribute.

On 2026-10-20, a release comparison on the same 5 arcmin table derived sides 2139, 2073, 1803 and 1297
at 200, 800, 3300 and 8000 km. Three alternating-order repetitions gave automatic median times of 0.024,
0.021, 0.057 and 0.088 s, against 0.009, 0.014, 0.034 and 0.074 s at explicit spacing 256. Every answer
agreed bitwise. At 3300 km automatic mode tested fewer circles (356 against 390) but built more kernels
(561 against 252): different block extents change the widened radius and require rebuilding the held
kernel. The geometric ceiling is a guard against whole-sphere bounds, not a promise of the fastest tiling.
The [full comparison on #71](https://github.com/turboBasic/PopulationCircles2026/issues/71) records all
four radii, eight explicit spacings, counters and timings; no constant was fitted to these measurements.
