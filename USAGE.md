# Usage

Examples of common commands and their output, plus how to choose options that affect search speed and
detail.
[`README.md`][readme] has the overview, the measured result and the release artifacts.
[`PERFORMANCE.md`][performance] explains speed, memory and precision trade-offs, table and ledger reuse,
and search spacing.

## Distances, grids and tables

### Distance along Earth's surface

A great-circle distance follows the curve of the Earth. It is the shortest route between two points
on a sphere.

Wiesbaden to Rome:

```sh
mise run cli -- distance 50.0782 8.2398 41.9028 12.4964
# {"schema_version":1,"tool":"popcircles","result":{"great_circle_km":966.3013398709427, …}}
```

### The population map's grid

The population map is divided into cells. Its grid is described by six values: the number of cells
across and down, the location of its top-left corner, and the size and direction of each step between
cells. A wrong value can produce a believable but incorrect result. This command checks the grid shape
and explains what it means on Earth, including whether it covers the whole globe and the area of a cell.
It does not read a file, so you can check a grid before building a table or starting a search.

For the GPWv4.11 population raster — a map made of cells — without reading the file itself (see
[`data/README.md`][data-readme]):

```sh
mise run cli -- grid describe --dataset population-count-2020-30arcsec
# {"schema_version":1,"tool":"popcircles","result":{"middle_row_cell_area_km2":0.8586351267048113, …}}
```

Grid-taking commands accept `--dataset` for registry defaults. World calculations use the population
dataset selected by the effective configuration unless `--dataset` is supplied. The built-in world
default is `population-count-2020-10arcmin`; pass `--dataset population-count-2020-30arcsec` to use the
native 30 arc-second raster. Each explicit grid flag overrides its own default, including zero; the
resolved geometry is checked before use. For an explicit-only grid, supply all six flags:

```sh
mise run cli -- grid describe --width 36 --height 18 \
  --origin-lat 90 --origin-lon -180 \
  --lon-step 10 --lat-step -10
```

### Building a population lookup table

A lookup table lets searches quickly add up population in an area. Build it once from the raster; later
commands can reuse the saved table. Fetch the raster first with `mise run data:get`.

```sh
mise run cli -- table build --dataset population-count-2020-30arcsec \
  --decimate 10 --cache out/gpw-5arcmin
```

`--decimate` combines source cells into larger cells. Higher values usually make searches faster but
locate circle centres less precisely. See [Choosing the inputs][inputs] for a starting value and
[PERFORMANCE.md][performance-resolution] for the speed and memory trade-offs.

To count population in a rectangle, give its north, south, west and east edges:

```sh
mise run cli -- table query --dataset population-count-2020-30arcsec \
  --decimate 10 --cache out/gpw-5arcmin \
  --north 55.06 --south 47.27 --west 5.87 --east 15.04
# the box around Germany; the result includes nearby areas outside Germany
# {…,"population":103837251.96947795}}
```

A rectangle follows latitude and longitude lines, not a country's borders. To count only people
inside a country's borders, use the country search described below. The table uses grid settings from
`data/registry.toml` unless you supply overrides. For the full command options, run
`mise run cli -- table --help`.

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

Circle searches use the population map, a list of populated places, and country borders. Fetch them with
`mise run data:get` before searching; if a file is missing, the command explains how to fetch it.
Distance, grid and table commands do not need the places or country borders.

**The examples below use a coarser map.** Each cell is 10 times wider and taller than a source cell, so
the search runs faster but places the circle centre less precisely. For the half-world example, this
coarser map gives the same radius as the full-resolution data. See [Validation][validation].

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

This finds the circle of the chosen radius that contains the most people. Here it is farther west than
the previous example and contains 16% of the world's population rather than 10%:

```sh
search most-populous --radius-km 1000
# {…,"result":{"centre":{"lat":25.125,"lon":79.70833333333331},"radius_km":1000.0,
#  "population":1254363867.9300776,"share_of_total":0.1616868627727305,"tolerance_persons":0.0,
#  …}}
```

### The smallest circle holding a share

The question the program is named for — here, half the world's population. `--ledger` chooses a file
for saving search results. Reuse the same path to resume a run or search for another share with the
same table; a ledger made for a different table is rejected.

```sh
search smallest-for-share --share 50 --ledger out/radii.json
# {…,"result":{"ledger":{"path":"out/radii.json","radii":24},"circle":{"radius_km":3360,
#  "centre":{"lat":28.79166666666667,"lon":100.625},"population":3879165388.019252,
#  "target":{"share":0.5,"persons":3878991299.6618357,"total":7757982599.323671},
#  "short_below":{"radius_km":3359,"population":3878869485.4163485},"covers_whole_grid":false,
#  "predicate_slack_persons":0.01196060136531932,…}}}
```

3360 km, centred in western China. `short_below` is the other end of the bracket the search proved: that
radius was measured too and falls short of the target, so minimality is readable from the result. The
share is given in **whole percent** so results use clean values such as `30%` instead of a long decimal
caused by computer rounding.

This answer is separated by 174 088 people against a `predicate_slack_persons` of 0.012, so it carries no
`ambiguity` block. A share whose target sits on a flat stretch — anything near everyone, or a plateau of
ocean — grows one, naming the probed radii the arithmetic cannot tell apart, and the run says so once on
stderr as well.

### A sweep of shares over one ledger

Use `sweep` to request several shares at once. This example asks for 10%, 30% and 50%, using the ledger
from the previous command:

```sh
search sweep --from 10 --to 50 --step 20 --ledger out/radii.json
# {…,"result":{"ledger":{"path":"out/radii.json","radii":43},"shares":{"from_percent":10,"to_percent":50,
#  "step_percent":20},"records":[
#   {"radius_km":702,…,"target":{"share":0.1,…},"stats":{"radii_evaluated":9,"radii_reused":11,…}},
#   {"radius_km":2129,…,"target":{"share":0.3,…},"stats":{"radii_evaluated":10,"radii_reused":14,…}},
#   {"radius_km":3360,…,"target":{"share":0.5,…},"stats":{"radii_evaluated":0,"radii_reused":24,…}}]}}
```

`records` are listed from the smallest requested share to the largest. Here the 50% record has
`radii_evaluated: 0`: its results were already saved, so no new radius searches were needed for it.
See [How the ledger avoids repeated searches][performance-ledger] for what is saved and how targets
share that work.

## Countries

`country list` prints country IDs and English names from the selected combination's boundary source.
It accepts `--combination` and `--resolution`, which default to the configured combination and that
combination's default resolution. For a search, `--country` accepts an exact ID or a name that matches
one country in the selected boundary source. If the ID or name is unknown or ambiguous, the command
lists possible matches.

```sh
mise run cli -- country list --config my-data.toml
# {…,"document":"country-list","result":{"selection":{"combination_id":"local-gpw",
#  "resolution":10,"population_source":{"name":"population.tif",…},
#  "boundary_source":{"name":"gisco-countries-2024-1m",…}},"countries":[…]}}
```

Country searches read only the prepared tables for the selected combination and resolution. They never
prepare or fetch data implicitly. If prepared data is missing or invalid, follow the command's guidance
to run `country prepare` when the raw sources are local, or `country fetch` when a remote prepared
bundle is configured. Known source and resolution mismatches are refused. The result's `selection`
names the combination, resolution, population and boundary sources, and fingerprints when available;
supplied attribution is carried to rendered maps.

One country uses the same share search, restricted to its uniquely owned population cells. Override the
configured choices with `--combination` and `--resolution`:

```sh
mise run cli -- country smallest --country DE --share 50 \
  --combination gpw-2020-gisco-2024 --resolution 10
# {…,"document":"country-smallest","result":{"country":{"id":"DE","name":"Germany",
#  "boundary_dataset":"gisco-countries-2024-1m",…},"ledger":{"path":"out/countries/4445.json",…},
#  "circle":{"radius_km":…,"target":{"share":0.5,…},…}}}
```

Several countries and shares run in one command. Repeat `--country` for each selector; the result orders
countries by canonical ID and each country's records by requested share:

```sh
mise run cli -- country sweep --country DE --country FR --from 10 --to 90 --step 10 \
  --combination gpw-2020-gisco-2024 --resolution 10
# {…,"schema_version":1,"document":"country-sweep","result":{"shares":{"from_percent":10,…},
#  "countries":[{"country":{"id":"DE","name":"Germany",…},"ledger":{"path":"out/countries/4445.json",…},
#  "records":[{"share":0.1,"circle":{"target":{"share":0.1,…},…}},…]},…]}}
```

Each country keeps its own resumable ledger under `out/countries`; pass `--ledger-dir` to choose another
directory. Repeating a run resumes that country's saved radii. Progress and diagnostics go to stderr,
while stdout contains one JSON document. Choose an affordable prepared resolution for sweeps; use a finer
prepared resolution for a more precise selected answer when needed.

A sweep skips features with no uniquely owned population cells at that resolution, reports each ID and
reason on stderr, and measures the elapsed time and cropped grid for every completed feature.

## Maps

From the checkout root, one command searches a named dataset and saves its map:

```sh
mise run data:get   # fetch the source raster explicitly before the first build
mise run map -- --dataset population-count-2020-30arcsec --share 50 \
  --decimate 10 --cache out/gpw-5arcmin --output out/half-world.png
```

`map` checks and reuses a compatible cache, even if the source raster has since been removed. An
absent or incompatible cache is built from the named raster; a corrupt or unreadable cache stops the
run. No data is downloaded implicitly. Omit `--decimate` for full resolution.

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
[`results/`][results], so
`mise run render -- --input results/world-most-populous-1000km-5arcmin.json --output out/map.png` draws a
figure on a clone that has fetched no raster at all. `mise run results:build` regenerates the world documents.

`--projection` chooses how the round Earth appears on the flat map. Use `plate-carree` for the familiar
rectangular world map, or `orthographic` for a globe-like view centred on the circle. Every figure
includes the credit required by the population data's CC BY licence. The required wording is in
[`data/README.md`][data-population].

Circles are drawn on the curved Earth, then transformed to fit the flat map. A circle that crosses the
180° longitude line appears at both edges of a rectangular map. A circle that covers a pole closes across
the top or bottom edge. Drawing circles directly on a flat map can shade the wrong region in either
case, so the project checks these cases. A circle that covers both poles appears as the whole map, with
the area outside the circle cut out.

**Nothing here reaches the network.** The coastlines are Natural Earth 110m, committed under
[`data/boundaries/`][data-readme], so a figure is the same on a fresh clone as on a warm one and CI
draws complete ones on every pull request. `--no-coastlines` draws the cap over a bare graticule, which is
a choice about the figure rather than a way to avoid a download.

When a world document includes a country breakdown, the caption lists each country with its population
inside and percentages of both country and circle populations, followed by accounting details and
boundary credit. The figure grows to fit those lines.

A country-result document names its population dataset and decimation, and needs the registered GISCO
vector at `data/boundaries/gisco-countries-2024-1m.geojson`. The renderer selects the canonical feature
ID from the document and checks its boundary revision and digest against the registry before drawing.
It derives the orthographic frame from the selected outline and the result circle, then shows the country
name and ID with population and boundary credits from their registry rows. Rendering still reads no
population raster, table or ledger. For example, the committed Germany result renders with:

```sh
mise run render -- --input results/germany-half-5arcmin.json --output out/germany-half.png
```

Fetch the boundary vector with `mise run data:get` when it is absent. Existing world documents need no
country boundary file.

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

### Population datasets and country combinations

`data/registry.toml` names the declared input datasets. The built-in `data/config.toml` selects the
10-arcminute world population dataset and the `gpw-2020-gisco-2024` country combination. The native
30 arc-second raster remains available with `--dataset population-count-2020-30arcsec`. In the GH-223
comparison on an Apple M2 Pro, the coarse dataset matched native radius and centre answers at 10%, 50%
and 90%, and average table-build-plus-search time was 8.71× faster (1.872 s vs 16.305 s). These are
three measured shares, not a guarantee for every query; the [issue records the method and full
comparison][coarse-comparison]. Use `--config PATH` to layer a user TOML file over the built-in
choices; relative file sources and prepared-data locations in that file resolve beside it.

For example, save this as `my-data.toml` to add a combination using a local population file and the
declared GISCO boundary dataset:

```toml
[defaults]
country_combination = "local-gpw"

[combinations.local-gpw]
population_source = "file:population.tif"
boundary_source = "dataset:gisco-countries-2024-1m"
default_resolution = 10

[combinations.local-gpw.resolutions.10]
local = "out/local-gpw/10"
```

Select that file with `--config my-data.toml`. A `dataset:<name>` source must name a compatible entry
in `data/registry.toml`; `file:<path>` names a local input. To override a built-in combination, keep
its ID and source pair, then change its default resolution or add/replace a resolution location. A
source pair cannot be rebound under an existing ID; add a new ID for a different pair. Global defaults
can select another registered population dataset or configured combination. Optional `provenance`
and `attribution` text can be recorded on a new combination.

List the effective built-in and user combinations with:

```sh
mise run cli -- country combinations --config my-data.toml
# {…,"document":"country-combinations","result":{"combinations":[…]}}
```

The list shows configured source pairs and resolutions. It does not fetch inputs or check whether
source or prepared-data files exist.

Prepare a combination before using its country tables:

```sh
mise run cli -- country prepare
mise run cli -- country prepare --combination local-gpw --resolution 10
```

The command defaults to the configured country combination and its default resolution. It reads
`dataset:<name>` inputs using `data/registry.toml`, or reads `file:<path>` inputs using metadata in the
combination: a local population GeoTIFF needs its grid, EPSG code and nodata sentinel, and a local
boundary GeoJSON needs its string identifier property. Preparation validates and fingerprints both
input files, applies the same cell ownership and aggregation rules as country searches, and writes a
versioned manifest plus portable country-prefix files under the selected resolution's `local`
directory. It checks the prepared output through the shared reader before reporting success. A
registered file whose bytes differ from its registry SHA-256 is refused. Existing prepared output is
preserved unless `--replace` is passed explicitly. Fetching remote inputs and selecting prepared
combinations in searches are separate actions.

To fetch an already prepared bundle, set `remote` beside the local install directory. The remote URL
names a directory that serves `manifest.json` and the payload files named by that manifest:

```toml
[combinations.local-gpw.resolutions.10]
local = "out/local-gpw/10"
remote = "https://data.example.org/prepared/local-gpw/10"
```

Then run the explicit fetch command:

```sh
mise run cli -- country fetch
mise run cli -- country fetch --combination local-gpw --resolution 10
```

Fetch defaults to the configured combination and its default resolution. It accepts direct HTTPS
locations without credentials or query strings, validates the manifest identity and every payload,
and checks the prepared tables before installing them. An already valid local bundle is reused without
network access. If the target is invalid or belongs to a different configured identity, pass
`--replace`; failed downloads or validation leave the previous bundle intact:

```sh
mise run cli -- country fetch --combination local-gpw --resolution 10 --replace
```

Fetch never runs implicitly during a search. Use `country prepare` when the raw population and boundary
sources are available locally; use `country fetch` when the configured prepared bundle is published
at its remote location.

**Choose the detail level with `--decimate`.** Use `10` for quick exploration, as in the examples above,
or `2` for a practical balance between detail and speed. Use `1` for full resolution. Build and search
with the same value. Country sweeps default to `10`; the other searches default to `1`.

A finer grid needs more memory and may take much longer to search. See the
[resolution trade-offs and measurements][performance-resolution] before choosing full resolution.
The program chooses spacing automatically; most users can leave `--spacing` unset.

### What limits precision

- Smallest-circle searches report whole kilometres. For example, the result shows that 3360 km holds
  half the population and 3359 km does not.
- Centres are placed at map cell centres. A coarser grid gives fewer possible locations.
- If numerical uncertainty prevents a firm answer, the result includes an `ambiguity` block, as
  described in the smallest-circle example above.

See [How resolution affects precision][performance-precision] for the explanation and arithmetic limits.

<!-- Links -->

[data-population]: data/README.md#population-count-2020-30arcsec
[data-readme]: data/README.md
[inputs]: #choosing-the-inputs
[coarse-comparison]: https://github.com/turboBasic/PopulationCircles2026/issues/223#issuecomment-6404921257
[performance]: PERFORMANCE.md
[performance-ledger]: PERFORMANCE.md#how-the-ledger-avoids-repeated-searches
[performance-precision]: PERFORMANCE.md#how-resolution-affects-precision
[performance-resolution]: PERFORMANCE.md#choosing-grid-resolution
[readme]: README.md
[results]: results/
[validation]: README.md#validation
