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

`country list` prints native IDs and English names from the selected combination's boundary source,
with `iso_alpha2` when that feature has a unique code. It accepts `--combination` and `--resolution`,
... (+348 lines) [see remaining: rtk proxy git -c core.fsmonitor=false show HEAD:USAGE.md | tail -n +175]
