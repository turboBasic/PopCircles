# Data

This folder contains the source data used by the project. The files are described for the program in
[`registry.toml`][registry] and explained here. Results made from the source data, such as lookup
tables and maps, are generated when needed and are not stored here.

```text
data/
  population/   population rasters
  boundaries/   coastlines and country borders
  settlements/  populated places
```

Each type of data has its own folder, a row in [`registry.toml`][registry], and an entry below.
Filenames use lowercase words separated by hyphens. If there are several resolutions, the filename
includes the resolution. **These filenames are project labels, not the publishers' original names.** The
information below identifies each source file. Rename a downloaded file to match its heading before
using it.

## Getting it

Large data files are kept in a download rather than in the source code repository. They are not included
when you first clone the project. Fetch them with:

```sh
mise run setup
mise run data:get   # fetch every registered dataset not already here, and verify it
```

`data:get` does not require an account. It reads [`registry.toml`][registry] and checks each
download against its recorded SHA-256 checksum before saving it. It skips files that are already
present and correct. If a download fails the check, it is not saved. After a successful download, the
command prints any credit required by the data's licence.

A small dataset is included in the repository, so it is already available after cloning. The coastline
below is one; maps can use it without downloading anything. The registry says whether each dataset is
included or must be fetched.
Obtaining an independent copy from the publisher, to check a republished asset against its source, is
[`CONTRIBUTING.md`][contributing-data].

## population-count-2020-30arcsec

| Property | Value |
| --- | --- |
| Grid | 43200 × 21600 (30 arc-second, 1/120°) |
| Extent | whole globe, origin (−180°, 90°) |
| CRS | EPSG:4326 (WGS 84) |
| Pixel type | Float32, single band, LZW compressed |
| Nodata | −3.40282306073709653e+38 (Float32, two ulps above −max) |
| Size | 428 465 215 bytes (409 MiB) |
| SHA-256 | `956993aa500774aed548c8e1af1a3a68fc164577be82ca799d4ae8568d445e9d` |
| Land cells | 222 669 928 of 933 120 000 (182 358 616 populated, 40 311 312 zero) |
| World total | 7 757 982 599.32 persons |
| Largest cell | 602 380.375 persons |

Every value above was measured from the file, not copied from a datasheet. The total is a compensated
(Neumaier) sum: over 933 120 000 additions into a running 7.8e9, where one ulp is 1.9e-6, a naive f64
accumulator lands 0.15 low — it gives 7 757 982 599.17, and the file is not what changed.

**It is [Gridded Population of the World, Version 4.11 (GPWv4.11): Population Count Adjusted to Match
the 2015 Revision of UN WPP Country Totals, Revision 11][gpw-adj]**, year **2020**, 30 arc-second
GeoTIFF — CIESIN, Columbia University, distributed by NASA SEDAC, DOI
[10.7927/H4PN93PB][gpw-doi]. SEDAC's own host is gone and the DOI is the route
that survived it. It is the WPP-adjusted variant rather than the unadjusted [Population Count][gpw-raw]:
the two differ only in values, and their catalogued maxima of 602 380 and 627 597 are what the measured
`Largest cell` identifies this copy by — the file's own `GDALMetadata` is generic and claims none of it.

GPWv4.11 is released under [CC BY 4.0][cc-by]. Reuse, including commercial, requires attribution, so any
published map or figure derived from this raster carries the citation:

> Center for International Earth Science Information Network — CIESIN — Columbia University. 2018.
> *Gridded Population of the World, Version 4 (GPWv4): Population Count Adjusted to Match 2015
> Revision of UN WPP Country Totals, Revision 11.* Palisades, NY: NASA Socioeconomic Data and
> Applications Center (SEDAC). <https://doi.org/10.7927/H4PN93PB>

## population-count-2020-10arcmin

| Property | Value |
| --- | --- |
| Grid | 2160 × 1080 (10 arc-minute, 1/6°) |
| Extent | whole globe, origin (−180°, 90°) |
| CRS | EPSG:4326 (WGS 84) |
| Pixel type | Float32, single band, LZW compressed |
| Nodata | −3.40282306073709653e+38 (Float32, two ulps above −max) |
| Size | 2,332,874 bytes |
| SHA-256 | `cf38289019a887369663ca18003bd4b39aa21e9ed574b0d66a61b3626ce9a767` |
| Measured world total | 7 757 982 599.77 persons |

This project derives the raster directly from [`population-count-2020-30arcsec`](#population-count-2020-30arcsec), whose exact SHA-256 is recorded beside the derived identity in [`registry.toml`][registry]. Each output cell sums its 20 × 20 source-cell block, retaining partial blocks at grid edges. Nodata and negative source values contribute zero. Aggregates are stored as Float32; the measured total differs from the source by 0.446 persons, within the conservative bound for per-cell Float32 rounding and f64 block summation.

The source is GPWv4.11's adjusted 2020 population count described above, with the same CC BY 4.0 terms and attribution. The derived GeoTIFF is published as [`population-count-2020-10arcmin.tif`][coarse-population-asset]. It is not resampled from another derived raster.

This is the built-in world-calculation default. Select the native 30 arc-second source explicitly with
`--dataset population-count-2020-30arcsec`. On an Apple M2 Pro, the GH-223 comparison built equal
2160 × 1080 tables from the coarse raster and from the native source at decimation 20. The coarse
table build plus three circle searches averaged 1.872 seconds, compared with 16.305 seconds from the
native source; all three measured radii and centres matched at 10%, 50% and 90% population shares.
This result describes those measured shares; the [issue records the timing method and full
comparison][coarse-comparison].

The measurements pass the pre-recorded default-adoption bar in #223; no answer-change condition
remains open for this default.

## coastline-1to110m

| Property | Value |
| --- | --- |
| Geometry | 134 LineStrings, 5128 vertices |
| Extent | whole globe, −180° to 180°, −85.609038° to 83.645130° |
| CRS | `urn:ogc:def:crs:OGC:1.3:CRS84`, the file's own declaration (EPSG:4326, axes as longitude then latitude) |
| Size | 136.6 KiB, one line |
| SHA-256 | `851f581ff5ffb844deed8ae1a9ce22e3c4bb3d74fa342cadb5d8e39b41ae7c3c` |

Measured from the file. The declared `bbox` reads `180.00000044181` at its eastern edge, 5 cm past the
antimeridian, and the vertices themselves stop at 180, so nothing relies on the declaration.

**It is [Natural Earth][ne]'s 1:110m physical coastline**, from the vector distribution repository at tag
**[v5.1.2][ne-tag]**, path `geojson/ne_110m_coastline.geojson` — stored here under the heading above,
which is the only difference between the two. Committed byte-for-byte as that tag serves it, which is
what makes the checksum above something a reader can check rather than a record of one download:

```sh
curl -sL https://raw.githubusercontent.com/nvkelso/natural-earth-vector/v5.1.2/geojson/ne_110m_coastline.geojson \
  | shasum -a 256
```

GeoJSON rather than the shapefile the same data is published as: `json` needs no reader beyond the
standard library, where a shapefile would put a driver in the dependency tree to draw a coastline with.

**Natural Earth is in the public domain.** Its [terms of use][ne-terms] ask for no permission, fee or
attribution, so a figure over this basemap carries the raster's citation alone. Crediting Natural Earth
too is welcomed by the project and is not done, for the reason the citation's wording is checked at all:
what a figure says about its sources should be what those sources ask for and nothing else.

## populated-places-1to10m

| Property | Value |
| --- | --- |
| Geometry | 7,342 Point features |
| Coordinates | longitude, latitude (WGS 84) |
| Size | 19,359,003 bytes |
| SHA-256 | `9b8e3de09048ef00dfc70357dbb9fa324493f214b5e0ae4daf1aa79a8d10116b` |

**Natural Earth 10m populated places**, in GeoJSON from its vector distribution repository at tag
**[v5.1.2][ne-tag]**, file `geojson/ne_10m_populated_places.geojson`. It is the publisher's 10m populated
places layer, preserved byte-for-byte; it has point coordinates and no raster grid, nodata value or cell
size. The `NAME` and `ADM0NAME` properties supply the settlement and country labels.

The upstream file is available anonymously, but its 19 MB size is above the repository's committed-file
limit, so this exact copy is published in a data-only release and fetched by `mise run data:get`. The
release asset is [`populated-places-1to10m.geojson`][populated-places-asset].

Natural Earth is in the public domain. Its [terms of use][ne-terms] require no attribution, so settlement
names add no credit to a figure.

## gisco-countries-2024-1m

| Property | Value |
| --- | --- |
| Geometry | 263 MultiPolygon country features |
| Coordinates | longitude, latitude (WGS 84) |
| CRS | EPSG:4326 (GeoJSON declares `urn:ogc:def:crs:EPSG::4326`) |
| Feature identifier | `CNTR_ID`, unique for all 263 features in this snapshot |
| Size | 58,878,461 bytes |
| SHA-256 | `556a81995108778335b8551f0d8929e86dcbcaab206ba512ba12928eafce505f` |

**GISCO Countries 2024, 1:1,000,000**, from the [publisher's country product][gisco-countries].
The publisher serves one worldwide GeoJSON file. The 2024 product has received later corrections;
the checksum above identifies the exact bytes selected here, and `CNTR_ID` identifies each feature
within that snapshot. The publisher offers no DOI or validated persistent identifier for these exact
bytes. The territorial interpretation follows [ADR 0015][adr-gisco]:
each published feature is retained as one identity, including multipart countries, separate
dependencies and the publisher's disputed-area treatment. The nine invalid source geometries remain
unchanged; country-mask work must decide their behavior explicitly.

GISCO's terms permit use and distribution only for non-commercial purposes, require acknowledgment
of EuroGeographics and UN-FAO, and require the source geometry to remain at 1:1,000,000 or smaller.
This input is unchanged at 1:1,000,000. It is not covered by this repository's MIT software licence.
The required English publication notice is **© EuroGeographics for the administrative boundaries**.

### Obtaining it

`mise run data:get` downloads the verified snapshot without an account from this project's
[data-only release][gisco-asset]. The release copy preserves the publisher's bytes unchanged because
the publisher's direct URL can serve corrected content under the same filename. To compare the
current publisher file with this snapshot:

```sh
curl -fL https://gisco-services.ec.europa.eu/distribution/v2/countries/geojson/CNTR_RG_01M_2024_4326.geojson \
  -o /tmp/CNTR_RG_01M_2024_4326.geojson
shasum -a 256 /tmp/CNTR_RG_01M_2024_4326.geojson
```

The published file is renamed to `gisco-countries-2024-1m.geojson` in `data/boundaries/`; no
extraction, simplification or reprojection is performed. Compare the resulting digest with the row in
[`registry.toml`][registry]. The registry's `fetch_url` is the stable project release asset;
the publisher URL above is provenance and a way to reproduce the selected bytes while it still
serves them.

## Prepared country combinations

The built-in combinations pair the 30 arc-second GPW population source with either GISCO Countries
2024 (`gpw-2020-gisco-2024`, the default) or GADM 4.1 (`gpw-2020-gadm-4.1`). Each pairing provides
factor 1 (30 arc-seconds) and factor 2 (1 arcminute); factor 2 is the default. `country fetch` installs
the selected prepared bundle. The manifests retain all 263 source feature IDs, including Vatican City,
which has no owned population cells in these bundles.

| Pairing and factor | Release assets | Bundle directory bytes |
| --- | --- | ---: |
| GISCO, factor 1 | [`data-v5`](https://github.com/turboBasic/PopCircles/releases/tag/data-v5) | 7,596,273,923 |
| GISCO, factor 2 | [`data-v6`](https://github.com/turboBasic/PopCircles/releases/tag/data-v6) | 1,901,567,393 |
| GADM, factor 1 | [`data-v7`](https://github.com/turboBasic/PopCircles/releases/tag/data-v7) | 7,177,194,888 |
| GADM, factor 2 | [`data-v8`](https://github.com/turboBasic/PopCircles/releases/tag/data-v8) | 1,796,924,439 |

The #213 release comparison found matching world radii at factors 1 and 2 for the tested 20%, 50%,
and 90% shares, and matching radii for the required GISCO country cases. Centres and achieved shares
shift slightly with resolution. GADM has more vertices for several microstates, but that geometry
count is not evidence of greater accuracy. The full case results, timings, input digests, and bundle
integrity evidence are recorded in the [#213 acceptance record](https://github.com/turboBasic/PopulationCircles2026/issues/213#issuecomment-6413726620).

## gadm41-adm0

| Property | Value |
| --- | --- |
| Geometry | 263 ADM0 MultiPolygon features |
| Coordinates | longitude, latitude (WGS 84) |
| CRS | EPSG:4326 |
| Feature identifier | `GID_0`, unique and retained from the GADM 4.1 source |
| Size | 1,155,550,194 bytes |
| SHA-256 | `4d005953e25108f475bcedb00f2ad4409f5d61d3c8d4ae7fd6e523d593c6c4b9` |

This is layer `ADM_0` from the [GADM 4.1 global levels archive][gadm-source], exported to GeoJSON
with only `GID_0` and `COUNTRY` properties. Coordinates are kept at 15 decimal places; no geometry
simplification or reprojection is performed. `GID_0` remains the source identity; it is not mapped to
GISCO identifiers.

GADM's [terms][gadm-license] permit academic and other non-commercial use; redistribution or
commercial use requires prior permission. The maintainer confirmed permission for this project's
redistribution of the source snapshot and derived prepared bundles on 2026-10-26. Austria's source
data is under CC BY-SA 2.0. The required attribution is **GADM database, version 4.1; Austria data
© Government of Austria, CC BY-SA 2.0.** This data is not covered by the repository's MIT software
licence.

### Obtaining and reproducing it

`mise run data:get` downloads the exact GeoJSON snapshot from this project's [data-only release][gadm-asset].
To reproduce the export from the publisher's global archive, extract `gadm_410-levels.gpkg` from
the ZIP, then run:

```sh
ogr2ogr -f GeoJSON -select GID_0,COUNTRY -lco COORDINATE_PRECISION=15 \
  /tmp/gadm41-adm0.geojson /tmp/gadm_410-levels.gpkg ADM_0
shasum -a 256 /tmp/gadm41-adm0.geojson
```

The digest above identifies the exact export. The registry's `fetch_url` is the stable project
asset; the publisher URL is the source archive used to reproduce it.

<!-- Links -->

[adr-gisco]: ../docs/decisions/0015-country-identities-follow-gisco-country-features.md
[cc-by]: https://creativecommons.org/licenses/by/4.0/
[coarse-population-asset]: https://github.com/turboBasic/PopCircles/releases/download/data-v4/population-count-2020-10arcmin.tif
[coarse-comparison]: https://github.com/turboBasic/PopulationCircles2026/issues/223#issuecomment-6404921257
[contributing-data]: ../CONTRIBUTING.md#verifying-a-published-dataset
[gadm-asset]: https://github.com/turboBasic/PopCircles/releases/download/data-v7/gadm41-adm0.geojson
[gadm-license]: https://gadm.org/license.html
[gadm-source]: https://geodata.ucdavis.edu/gadm/gadm4.1/gadm_410-levels.zip
[gisco-asset]: https://github.com/turboBasic/PopCircles/releases/download/data-v3/gisco-countries-2024-1m.geojson
[gisco-countries]: https://gisco-services.ec.europa.eu/distribution/v2/countries/countries-2024-files.html
[gpw-adj]: https://www.earthdata.nasa.gov/data/catalog/sedac-ciesin-sedac-gpwv4-apct-wpp-2015-r11-4.11
[gpw-doi]: https://doi.org/10.7927/H4PN93PB
[gpw-raw]: https://doi.org/10.7927/H4JW8BX5
[ne]: https://www.naturalearthdata.com/downloads/110m-physical-vectors/
[ne-tag]: https://github.com/nvkelso/natural-earth-vector/releases/tag/v5.1.2
[ne-terms]: https://www.naturalearthdata.com/about/terms-of-use/
[populated-places-asset]: https://github.com/turboBasic/PopCircles/releases/download/data-v2/populated-places-1to10m.geojson
[registry]: registry.toml
