# PopCircles 👫🌍

[![CI][ci-badge]][ci] [![Rust 1.99+][rust-badge]][rust] [![License: MIT][license-badge]][license]

PopCircles answers questions about population inside circles: how many people live within a
chosen circle, which circle of a given size contains the most people, and the smallest circle that
contains a chosen share of the population. These searches work for the whole world and for individual
countries. The project also draws maps of the results. It uses a population map with cells about 1 km
wide at the equator and measures distance along Earth's curved surface, rather than on a flat map.
It began with one question: *what is the smallest circle that contains half the world's population?*

**Status: Features work; the user experience still needs polish.** You can measure population inside a
circle, find the most populated circle of a chosen size, or find the smallest circle containing a
chosen share of the population, for the world or an individual country. Long searches can resume where
they left off. Maps can be drawn from world and country results. The half-world result has been checked
using the full-resolution population data and compared with published results ([Validation][validation]).
Broader validation is still in progress.

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

Half the world's population — half of this dataset's 7 757 982 599 people — is held by a circle of
**3 360 km** centred at [28.84 N, 100.66 E][map-centre] (192 km from Xichang, China).
[`results/world-half-30arcsec.json`][results-world-half] records the full-resolution result
from 2026-08-19. The radius is checked in 1 km steps: 3 360 km reaches half the population by 655 480
people, while 3 359 km falls short by 75 397. Both circles were measured.

Earlier published results include Danny Quah's roughly 3 300 km circle and the
[Valeriepieris circle][valeriepieris]. The 60 km difference has four known causes:

- this dataset counts people in 2020, while the earlier results used older data from a world with 4%
  fewer people;
- half of this dataset's population is not exactly half of the world's population today;
- this project measures distance along Earth's curved surface, using a sphere with a radius of 6 371 km;
- the search checks the centre of each data cell. At full resolution, cell centres are about 926 m apart.

What the search spends its time on was measured on the same run: at full resolution it is **6.5% CPU** and
the rest is page faults against a 7.5 GB table, which `FU-17` in [`docs/follow-ups.md`][follow-ups] holds.
`mise run test:validate` is the end-to-end run against the real raster, and it skips with a message when
the raster has not been fetched.

## The maps

[**The gallery**][gallery] draws every committed document that carries a circle, rebuilt from the tree on
every merge. Each figure is captioned from its own document — the share, the radius, the centre, the table
that answered it and the credit the raster requires — and the document it was drawn from sits beside it.
Nothing there is committed; a figure is deployed rather than carried.

## Data

The population raster is [GPWv4.11 UN WPP-adjusted population count for 2020][gpw] at 30 arc-second
resolution — CIESIN / Columbia University, distributed by NASA SEDAC, DOI [10.7927/H4PN93PB][gpw-doi],
[CC BY 4.0][cc-by]. Attribution is required of anything published from it;
[`data/README.md`][data-population] holds the citation and the grid details.

**It is not in a plain clone.** The [`data-v1` release][data-tag] carries it, `mise run data:get` fetches
from there, and that copy needs no account. Obtaining an *independent* copy from NASA Earthdata instead —
which is what makes the published checksum worth checking — is [`CONTRIBUTING.md`][contributing-data].

## Layout

| Path | What it is |
| --- | --- |
| `crates/popcircles/` | Rust library — the search |
| `crates/popcircles-cli/` | The `popcircles` binary — a client of the library |
| `data/` | Input datasets, with a registry in [`data/registry.toml`][data-registry] |
| `python/` | Python project — `population_circles` coordinates search and renders maps, `repo_tools` lints this repository |
| `docs/ai-instructions.md` | The instruction router: project invariants, and what to read for a task |
| `docs/ai/` | Per-task conventions: [platform][platform], [code][code], [application][application] |
| `docs/follow-ups.md` | The register of pending obligations |

Those documents are the conventions for humans as much as for AI tools. They are split across files so
each subject can be corrected and reviewed on its own, not so that any of them is optional — all of
them apply to every change. [`ACCESSIBILITY.md`][accessibility] describes current accessibility
priorities and limitations. [`CONTRIBUTING.md`][contributing] covers setup and the task loop, and
[`USAGE.md`][usage] is the worked reference for running the thing.

## Inspiration

The problem, and the demonstration that it is tractable at full raster resolution, come from
[alexmijo/PopulationCircles][alexmijo-project] — a C++ project that produced the published maps of
these circles. This is an independent implementation: the approach is reused, the code is not.
That repository carries no licence, so none of its source is copied or ported here;
see [the application doc][application-copying].

Prior art on the 50% circle specifically: the [Valeriepieris circle][valeriepieris].

## License

[MIT][license].

<!-- Links -->

[accessibility]: ACCESSIBILITY.md
[alexmijo-project]: https://github.com/alexmijo/PopulationCircles
[application]: docs/ai/application.md
[application-copying]: docs/ai/application.md#provenance-and-the-copying-rule
[cc-by]: https://creativecommons.org/licenses/by/4.0/
[ci]: https://github.com/turboBasic/PopCircles/actions/workflows/ci.yml?query=branch%3Amain
[ci-badge]: https://github.com/turboBasic/PopCircles/actions/workflows/ci.yml/badge.svg?branch=main
[code]: docs/ai/code.md
[contributing]: CONTRIBUTING.md
[contributing-data]: CONTRIBUTING.md#verifying-a-published-dataset
[data]: data/README.md
[data-getting-it]: data/README.md#getting-it
[data-population]: data/README.md#population-count-2020-30arcsec
[data-registry]: data/registry.toml
[data-tag]: https://github.com/turboBasic/PopCircles/releases/tag/data-v1
[discussions]: https://github.com/turboBasic/PopCircles/discussions
[follow-ups]: docs/follow-ups.md
[gallery]: https://turbobasic.github.io/PopCircles/
[gpw]: https://www.earthdata.nasa.gov/data/catalog/sedac-ciesin-sedac-gpwv4-apct-wpp-2015-r11-4.11
[gpw-doi]: https://doi.org/10.7927/H4PN93PB
[issues]: https://github.com/turboBasic/PopCircles/issues
[license]: LICENSE
[license-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[map-centre]: https://www.openstreetmap.org/?mlat=28.8375&mlon=100.66250000000002
[platform]: docs/ai/platform.md
[release-verification]: CONTRIBUTING.md#verifying-a-downloaded-release
[releases]: https://github.com/turboBasic/PopCircles/releases
[results-world-half]: results/world-half-30arcsec.json
[rust]: https://www.rust-lang.org/
[rust-badge]: https://img.shields.io/badge/rust-1.99%2B-orange.svg
[usage]: USAGE.md
[valeriepieris]: https://en.wikipedia.org/wiki/Valeriepieris_circle
[validation]: #validation
