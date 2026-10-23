# PopulationCircles2026

[![CI][ci-badge]][ci] [![Rust 1.99+][rust-badge]][rust] [![License: MIT][license-badge]][license]

Finds the smallest circle on Earth containing a given share of the world's population, from a
~1 km resolution population raster — smallest measured on the globe, not on a projected map, which
is what makes the answer differ from the familiar viral versions. Also renders the results as maps.

**Status: early.** Grid geometry, spherical geodesy, raster ingest, the summation table with its
on-disk cache, the circular kernels a circle is measured through, the population inside one such
circle, the most populous circle of a fixed radius, and the smallest circle holding a given share of a
population — resumable across runs — are implemented and tested, with a command line over that search
and maps rendered from what it publishes. The answer for half the world has been run against the real
raster at full resolution and checked against the published prior art ([Validation](#validation)).
Per-country listing, searches and resumable sweeps are available; per-country maps and broader validation
remain.

## Getting started

```sh
mise run setup      # toolchains, dependencies, git hooks
mise run ci         # lint, typecheck, test — the same checks CI runs
```

Input datasets live in [`data/`](data/README.md), described for a machine in
[`data/registry.toml`](data/registry.toml). The rasters are published rather than carried, so cloning
stays fast and cheap; the coastline basemap is a committed blob, because a hundred kilobytes every clone
needs is cheaper carried than fetched:

```sh
mise run data:get   # fetch what is missing and verify it — no account needed
```

`data:get` reads the registry and checks each file against the recorded checksum before putting it in
place, so a truncated download is refused rather than parsed:
[`data/README.md`](data/README.md#getting-it).

## Usage

[`USAGE.md`](USAGE.md) has the worked commands and their real output — a great-circle distance, the
summation table and the queries answered from it, the four search commands, combined search and
rendering, and what a run says about itself — together with how to choose `--decimate` and override spacing
with `--spacing`. `mise run cli -- --help` is the full command and flag reference.

## Validation

Half the world's population — half of this raster's own 7 757 982 599 persons — is held by a circle of
**3 360 km** centred at [28.84 N, 100.66 E](https://www.openstreetmap.org/?mlat=28.8375&mlon=100.66250000000002) (192 km from Xichang, China).
[`results/world-half-30arcsec.json`](results/world-half-30arcsec.json) is that answer as the program
printed it on 2026-08-19 from the full 30 arc-second grid, and it is a bracket rather than an estimate:
3 360 km reaches half by 655 480 persons and 3 359 km falls 75 397 short. Both were computed, and the
summation slack between them is 0.12 of a person.

The published prior art is Danny Quah's ~3 300 km, and the [Valeriepieris circle][valeriepieris] before
it. **The 60 km is explained rather than tuned away**, by four things and no defect:

- the raster is 2020 and the published figures are earlier, over a world 4% smaller;
- half of *this* dataset is not half of the world, and the target is always the dataset's own total;
- distances here are great-circle arcs on a sphere of 6 371.0088 km, published in every document's
  `earth_model`;
- the answer is the best cell centre, not the best point — and at 30 arc-seconds that mesh is 926 m.

What the search spends its time on was measured on the same run: at full resolution it is **6.5% CPU** and
the rest is page faults against a 7.5 GB table, which `FU-17` in
[`docs/follow-ups.md`](docs/follow-ups.md) holds. `mise run test:validate` is the end-to-end run against the real
raster, and it skips with a message when the raster has not been fetched.

## The maps

[**The gallery**][gallery] draws every committed document that carries a circle, rebuilt from the tree on
every merge. Each figure is captioned from its own document — the share, the radius, the centre, the table
that answered it and the credit the raster requires — and the document it was drawn from sits beside it.
Nothing there is committed; a figure is deployed rather than carried
([ADR 0012](docs/decisions/0012-a-figure-is-deployed-to-a-site-not-attached-to-a-tag.md)).

## Releases

A [release][releases] attaches two binaries, each named by its target triple with a `.sha256` beside it:

| Asset | For |
| --- | --- |
| `popcircles-aarch64-apple-darwin` | macOS on Apple silicon |
| `popcircles-x86_64-unknown-linux-gnu` | Linux on x86-64 |

Each binary has a `.sha256` checksum and the release includes `provenance.sigstore.json`. The checksum
detects accidental damage; verify the attestation to confirm that GitHub's release workflow produced
the downloaded bytes from the expected source commit. Release authorization trusts the current SSH
signing keys registered to the `turboBasic` GitHub account. This governs prospective signed releases;
historical tags remain unchanged. Signing a release tag is documented in
[`CONTRIBUTING.md`](CONTRIBUTING.md#releasing).

Set `TAG` to the release tag and `COMMIT` to its source commit. To find the commit, inspect the tag's
commit target, for example with `git ls-remote` and the annotated tag's peeled ref:

```sh
git ls-remote https://github.com/turboBasic/PopulationCircles2026.git \
  "refs/tags/$TAG" "refs/tags/$TAG^{}"
```

The peeled `^{}` line is the commit. Download the matching binary and `provenance.sigstore.json`
from the release, then verify it:

```sh
gh attestation verify ./popcircles-aarch64-apple-darwin \
  --repo turboBasic/PopulationCircles2026 \
  --signer-workflow turboBasic/PopulationCircles2026/.github/workflows/release.yml \
  --source-ref "refs/tags/$TAG" \
  --source-digest "$COMMIT" \
  --bundle ./provenance.sigstore.json
```

Use `popcircles-x86_64-unknown-linux-gnu` for Linux. The same verification command and policy apply
to either `.sha256` file; substitute that downloaded checksum file as the first argument.

Neither carries a Developer ID signature and neither is notarized, which costs you something only on
macOS and only depending on how you fetched it. **A browser download** is tagged
`com.apple.quarantine`, and Gatekeeper then kills the binary with "Apple could not verify … is free of
malware" — offering to move it to the Bin, which you do not want. Clearing the attribute is the whole of
the fix:

```sh
xattr -d com.apple.quarantine popcircles-aarch64-apple-darwin
```

**A download with `curl`, `wget` or `gh release download` is not tagged** and runs as it is. Nothing here
depends on Gatekeeper being disabled: the macOS binary is ad-hoc signed, which is what the linker does by
default and what lets it run on Apple silicon at all.

**What a release promises is the wire format, and only that.** The `schema_version` the JSON documents
carry is a contract across releases, so a renderer or any other consumer may rely on it. The summation
table cache and the ledger are internal by contrast, and **any release may invalidate either**: one it
did not write is refused and rebuilt rather than migrated, which at full resolution costs a pass over
the raster rather than a download.

## Data

The population raster is [GPWv4.11 UN WPP-adjusted population count for 2020][gpw] at 30
arc-second resolution — CIESIN / Columbia University, distributed by NASA SEDAC, DOI
[10.7927/H4PN93PB][gpw-doi], [CC BY 4.0][cc-by]. Attribution is required of anything published from
it; [`data/README.md`](data/README.md#population-count-2020-30arcsec) holds the citation and the grid
details.

**It is not in a plain clone.** The [`data-v1` release][data-tag] carries it, `mise run data:get` fetches
from there, and that copy needs no account. Obtaining an *independent* copy from NASA Earthdata instead —
which is what makes the published checksum worth checking — is
[`CONTRIBUTING.md`](CONTRIBUTING.md#verifying-a-published-dataset).

## Layout

| Path | What it is |
| --- | --- |
| `crates/popcircles/` | Rust library — the search |
| `crates/popcircles-cli/` | The `popcircles` binary — a client of the library |
| `data/` | Input datasets, with a registry in [`data/registry.toml`](data/registry.toml) |
| `python/` | Python project — `population_circles` coordinates search and renders maps, `repo_tools` lints this repository |
| `docs/ai-instructions.md` | The instruction router: project invariants, and what to read for a task |
| `docs/ai/` | Per-task conventions: [platform](docs/ai/platform.md), [code](docs/ai/code.md), [application](docs/ai/application.md) |
| `docs/decisions/` | Architecture decision records — one ruling each, one page each |
| `docs/follow-ups.md` | The register of pending obligations |

Those documents are the conventions for humans as much as for AI tools. They are split across files so
each subject can be corrected and reviewed on its own, not so that any of them is optional — all of
them apply to every change. [`CONTRIBUTING.md`](CONTRIBUTING.md) covers setup and the task loop, and
[`USAGE.md`](USAGE.md) is the worked reference for running the thing.

## Inspiration

The problem, and the demonstration that it is tractable at full raster resolution, come from
[alexmijo/PopulationCircles] — a C++ project that produced the published maps of these circles.
This is an independent implementation: the approach is reused, the code is not. That repository
carries no licence, so none of its source is copied or ported here;
see [the application doc](docs/ai/application.md#provenance-and-the-copying-rule).

Prior art on the 50% circle specifically: the [Valeriepieris circle][valeriepieris].

## License

[MIT](LICENSE).

<!-- Link references: badges at the top of this file. -->

[alexmijo/PopulationCircles]: https://github.com/alexmijo/PopulationCircles
[cc-by]: https://creativecommons.org/licenses/by/4.0/
[ci]: https://github.com/turboBasic/PopulationCircles2026/actions/workflows/ci.yml?query=branch%3Amain
[ci-badge]: https://github.com/turboBasic/PopulationCircles2026/actions/workflows/ci.yml/badge.svg?branch=main
[data-tag]: https://github.com/turboBasic/PopulationCircles2026/releases/tag/data-v1
[gallery]: https://turbobasic.github.io/PopulationCircles2026/
[gpw]: https://www.earthdata.nasa.gov/data/catalog/sedac-ciesin-sedac-gpwv4-apct-wpp-2015-r11-4.11
[gpw-doi]: https://doi.org/10.7927/H4PN93PB
[license]: LICENSE
[license-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[releases]: https://github.com/turboBasic/PopulationCircles2026/releases
[rust]: https://www.rust-lang.org/
[rust-badge]: https://img.shields.io/badge/rust-1.99%2B-orange.svg
[valeriepieris]: https://en.wikipedia.org/wiki/Valeriepieris_circle
