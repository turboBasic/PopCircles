# Data

Input datasets, one directory per kind, described for a machine in [`registry.toml`](registry.toml) and
for a person here. Generated products — summation tables, rendered maps — never live here: they are
gitignored (`*.bin`, `out/`) and reproducible from these inputs.

```text
data/
  population/   population rasters
  boundaries/   coastlines and country borders
  settlements/  populated places
```

A new kind gets its own directory, a row in [`registry.toml`](registry.toml) and an entry below. Keep
names lowercase and hyphenated, and put the grid resolution in the filename when a dataset comes in
several. **Every filename here is this project's own description of the contents, and matches no
publisher's**, so what identifies a dataset is the provenance recorded below and never its name — a file
downloaded from the source is renamed to its heading here before it is usable.

## Getting it

A large dataset is published rather than committed, so it is absent from a fresh clone until fetched:

```sh
mise run setup
mise run data:get   # fetch every registered dataset not already here, and verify it
```

`data:get` needs no account anywhere. It reads [`registry.toml`](registry.toml), checks each file
against the `sha256` recorded there **before** putting it in place, and downloads nothing already
present and correct. A download that fails verification leaves nothing behind. On success it prints the
attribution each licence requires, which is the moment a user acquires the obligation.

A small dataset is a committed Git blob and needs none of this — the coastline below is one, so a clone
draws a complete figure before fetching anything. Which of the two a dataset is, its row says.
Obtaining an independent copy from the publisher, to check a republished asset against its source, is
[`CONTRIBUTING.md`](../CONTRIBUTING.md#verifying-a-published-dataset).

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
[10.7927/H4PN93PB](https://doi.org/10.7927/H4PN93PB). SEDAC's own host is gone and the DOI is the route
that survived it. It is the WPP-adjusted variant rather than the unadjusted [Population Count][gpw-raw]:
the two differ only in values, and their catalogued maxima of 602 380 and 627 597 are what the measured
`Largest cell` identifies this copy by — the file's own `GDALMetadata` is generic and claims none of it.

GPWv4.11 is released under [CC BY 4.0][cc-by]. Reuse, including commercial, requires attribution, so any
published map or figure derived from this raster carries the citation:

> Center for International Earth Science Information Network — CIESIN — Columbia University. 2018.
> *Gridded Population of the World, Version 4 (GPWv4): Population Count Adjusted to Match 2015
> Revision of UN WPP Country Totals, Revision 11.* Palisades, NY: NASA Socioeconomic Data and
> Applications Center (SEDAC). <https://doi.org/10.7927/H4PN93PB>

[cc-by]: https://creativecommons.org/licenses/by/4.0/
[gpw-adj]: https://www.earthdata.nasa.gov/data/catalog/sedac-ciesin-sedac-gpwv4-apct-wpp-2015-r11-4.11
[gpw-raw]: https://doi.org/10.7927/H4JW8BX5

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

[ne]: https://www.naturalearthdata.com/downloads/110m-physical-vectors/
[ne-tag]: https://github.com/nvkelso/natural-earth-vector/releases/tag/v5.1.2
[ne-terms]: https://www.naturalearthdata.com/about/terms-of-use/

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
release asset is [`populated-places-1to10m.geojson`](https://github.com/turboBasic/PopulationCircles2026/releases/download/data-v2/populated-places-1to10m.geojson).

Natural Earth is in the public domain. Its [terms of use][ne-terms] require no attribution, so settlement
names add no credit to a figure.
