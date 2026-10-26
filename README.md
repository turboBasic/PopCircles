# PopCircles 👫🌍

[![CI][ci-badge]][ci] [![Rust 1.99+][rust-badge]][rust] [![License: MIT][license-badge]][license]

PopCircles searches a map of world population to find how many people live inside a circle, which circle
of a chosen radius contains the most people, or the smallest circle containing a chosen share of the
population. You can search worldwide or by country, then draw maps of the results. The map is divided
into cells about 1 km wide at the equator, and distances follow Earth's curved surface rather than a
flat map. The project began with one question: *what is the smallest circle that contains half the
world's population?*

**Status:** Search and map generation work; broader validation is still in progress. Long searches can
resume after interruption. See [Validation][validation] for the half-world result.

## Getting started

```sh
mise run setup      # toolchains, dependencies, git hooks
mise run ci         # lint, typecheck, test — the same checks CI runs
```

Input datasets are listed in [`data/`][data] and described for the program in
[`data/registry.toml`][data-registry]. Large files are downloaded separately, so a fresh clone is
small. The coastline map is included in the repository because it is small:

```sh
mise run data:get   # fetch what is missing and verify it — no account needed
```

`data:get` reads the registry and checks each file against the recorded checksum before putting it in
place, so a truncated download is refused rather than parsed:
[`data/README.md`][data-getting-it].

## Install

Download the binary for your platform from [GitHub Releases][releases]:

| Asset | For |
| --- | --- |
| `popcircles-aarch64-apple-darwin` | macOS on Apple silicon |
| `popcircles-x86_64-unknown-linux-gnu` | Linux on x86-64 |

For download verification, see [the release verification instructions][release-verification].
macOS binaries are not notarized. If Gatekeeper blocks a verified browser download, clear
its quarantine attribute:

```sh
xattr -d com.apple.quarantine popcircles-aarch64-apple-darwin
```

## Usage

[`USAGE.md`][usage] shows commands and their output for measuring distances, looking up population,
searching for circles, and drawing maps. It also explains how to choose `--decimate` for faster searches.
Run `mise run cli -- --help` for the full command and option reference.

Questions about using the project or early ideas belong in [Discussions][discussions]. Report
reproducible bugs or work ready to be scoped in [Issues][issues].

## Validation

The 2020 raster counts 7 757 982 599 people. In that dataset, the smallest circle containing half the
population has a radius of **3 360 km** and is centred at [28.84 N, 100.66 E][map-centre] (192 km from Xichang, China).
The full-resolution result was recorded on 2026-08-19 in
[`results/world-half-30arcsec.json`][results-world-half]. We tested whole-kilometre radii: the 3 360 km
circle contains 655 480 people more than half the total; the 3 359 km circle contains 75 397 fewer.

Earlier published results include Danny Quah's roughly 3 300 km circle and the
[Valeriepieris circle][valeriepieris]. Four differences help explain the 60 km gap:

- the earlier results used older population data, with about 4% fewer people than this 2020 raster;
- "half" here means half of the raster's total, which differs from half of today's estimated population;
- this project measures distance along Earth's curved surface, using a sphere with a radius of 6 371 km;
- the search checks the centre of each data cell. At full resolution, cell centres are about 926 m apart.

At full resolution, the lookup table can exceed available memory, so the search spends much of its time
reading from disk. See [the performance details](PERFORMANCE.md#when-the-table-exceeds-available-memory).
To validate against the real raster, run `mise run test:validate`; it skips if the raster is missing.

## The maps

The [gallery][gallery] shows maps generated from result files in `results/`. Each map caption gives its
population share, radius and centre, identifies the table used to calculate it, and includes the required
data credit. The gallery is rebuilt when a change is merged, and each map links to its result file.

## Data

The population raster is [GPWv4.11 UN WPP-adjusted population count for 2020][gpw] at 30 arc-second
resolution — CIESIN / Columbia University, distributed by NASA SEDAC, DOI [10.7927/H4PN93PB][gpw-doi],
[CC BY 4.0][cc-by]. Published maps and figures based on this raster must include its citation;
[`data/README.md`][data-population] holds the citation and the grid details.

The full-resolution raster is not included in a clone. The [`data-v1` release][data-tag] carries the
project's copy; run `mise run data:get` to download it and verify it against the recorded checksum. No
account is needed.

## Layout

| Path | What it is |
| --- | --- |
| `crates/popcircles/` | Rust library — the search |
| `crates/popcircles-cli/` | The `popcircles` binary — a client of the library |
| `data/` | Input datasets, with a registry in [`data/registry.toml`][data-registry] |
| `python/` | Python project — `population_circles` coordinates search and renders maps, `repo_tools` lints this repository |
| `docs/ai-instructions.md` | Project rules and links to contributor guidance |
| `docs/ai/` | Contributor guidance on tools, code and application design |
| `docs/follow-ups.md` | Planned follow-up work and the conditions for doing it |

The `docs/ai/` files describe project conventions for contributors. [`ACCESSIBILITY.md`][accessibility]
covers accessibility priorities and limits. [`CONTRIBUTING.md`][contributing] explains setup and how to
contribute; [`USAGE.md`][usage] shows how to run the commands.

## Inspiration

The question and original full-resolution maps come from
[alexmijo/PopulationCircles][alexmijo-project], a C++ project. PopCircles independently implements the
same approach. The original repository has no licence, so none of its code is copied or ported here.

## License

[MIT][license].

<!-- Links -->

[accessibility]: ACCESSIBILITY.md
[alexmijo-project]: https://github.com/alexmijo/PopulationCircles
[cc-by]: https://creativecommons.org/licenses/by/4.0/
[ci]: https://github.com/turboBasic/PopCircles/actions/workflows/ci.yml?query=branch%3Amain
[ci-badge]: https://github.com/turboBasic/PopCircles/actions/workflows/ci.yml/badge.svg?branch=main
[contributing]: CONTRIBUTING.md
[data]: data/README.md
[data-getting-it]: data/README.md#getting-it
[data-population]: data/README.md#population-count-2020-30arcsec
[data-registry]: data/registry.toml
[data-tag]: https://github.com/turboBasic/PopCircles/releases/tag/data-v1
[discussions]: https://github.com/turboBasic/PopCircles/discussions
[gallery]: https://turbobasic.github.io/PopCircles/
[gpw]: https://www.earthdata.nasa.gov/data/catalog/sedac-ciesin-sedac-gpwv4-apct-wpp-2015-r11-4.11
[gpw-doi]: https://doi.org/10.7927/H4PN93PB
[issues]: https://github.com/turboBasic/PopCircles/issues
[license]: LICENSE
[license-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[map-centre]: https://www.openstreetmap.org/?mlat=28.8375&mlon=100.66250000000002
[release-verification]: CONTRIBUTING.md#verifying-a-downloaded-release
[releases]: https://github.com/turboBasic/PopCircles/releases
[results-world-half]: results/world-half-30arcsec.json
[rust]: https://www.rust-lang.org/
[rust-badge]: https://img.shields.io/badge/rust-1.99%2B-orange.svg
[usage]: USAGE.md
[valeriepieris]: https://en.wikipedia.org/wiki/Valeriepieris_circle
[validation]: #validation
