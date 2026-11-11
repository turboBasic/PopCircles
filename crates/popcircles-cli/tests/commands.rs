// The four search commands run for real, against a cache this file builds. What only a real invocation
// can check is here and nothing else: that stdout is one JSON document, that stderr stays out of it, that
// the exit code is the class the failure falls in, and that the document names the table it was answered
// from. The wire format itself is snapshotted in the library, over `report`'s own types — see ADR 0004
// and `FU-03`, whose tripwire watches `crates/popcircles/src/snapshots/` and would not see a
// snapshot kept here.
//
// Synthetic caches need no raster. `Synthetic` is public and unconditional, so the fixture streams into a real cache
// through the writer a build uses, and the suite passes on a checkout with no raster in it.

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows both in tests. cast_precision_loss likewise — the largest cell is 648,
// which u32 -> f32 holds exactly.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::cast_precision_loss)]

use std::path::PathBuf;
use std::process::{Command, Output};

use popcircles::geodesy::LatLon;
use popcircles::grid::Grid;
use popcircles::raster::Synthetic;
use popcircles::table::cache::Cache;
use popcircles::table::{Decimation, build};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const WIDTH: u32 = 36;
const HEIGHT: u32 = 18;

/// The registry raster's sentinel, so the fixture reaches the table by the path a real raster takes.
const NODATA: f32 = -3.402_823e38;

/// Distinct at every cell, so a payload read at the wrong offset shows up as a wrong sum.
fn dense(row: u32, col: u32) -> f32 {
    (row * WIDTH + col + 1) as f32
}

/// The library's spelling of a digest, which is what a build prints and a flag takes back.
fn hexadecimal(digest: u64) -> String {
    format!("{digest:#018x}")
}

/// A cache built from a synthetic raster, under a directory that goes away with the test.
struct Fixture {
    directory: TempDir,
    digest: u64,
}

impl Fixture {
    /// Ten degrees a side and closing in longitude, which is `report.rs`'s fixture and `circle.rs`'s: the
    /// smallest whole-globe shape a kernel accepts.
    fn grid() -> Grid {
        Grid::new(
            WIDTH,
            HEIGHT,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            10.0,
            -10.0,
        )
        .expect("a 36 x 18 whole-globe grid is valid")
    }

    fn build() -> Self {
        Self::named("population-count-2020-30arcsec")
    }

    /// The same cache, published under a dataset name — which is what `table build --dataset` writes and
    /// what a document answered from such a table has to carry.
    fn named(dataset: &str) -> Self {
        Self::over(dense, Some(dataset))
    }

    /// Everyone in a four-cell patch and the sentinel everywhere else, which is the shape whose answer the
    /// arithmetic cannot separate: every circle holding the four holds the same 400 people, so every
    /// reaching probe's margin against a target of everyone is zero.
    ///
    /// `NODATA` rather than a plain zero outside the patch, so the fixture reaches the ambiguous case
    /// through the nodata-to-zero conversion a real raster takes rather than around it. The library's own
    /// tests build this shape with a helper private to them; this is the same four cells, rebuilt.
    fn clustered() -> Self {
        Self::over(
            |row, col| {
                let inside = (8..=9).contains(&row) && (15..=16).contains(&col);
                if inside { 100.0 } else { NODATA }
            },
            Some("population-count-2020-30arcsec"),
        )
    }

    fn over(cell: impl Fn(u32, u32) -> f32, dataset: Option<&str>) -> Self {
        let directory = TempDir::new().expect("a temporary directory");
        let fixture = Self {
            directory,
            digest: 0,
        };
        let grid = Self::grid();
        let rows: Vec<Vec<f32>> = (0..HEIGHT)
            .map(|row| (0..WIDTH).map(|col| cell(row, col)).collect())
            .collect();
        fixture.register_settlements(&rows);
        fixture.refresh_boundary_digest();
        let source = Synthetic::new(grid, NODATA, rows).expect("the rows are the grid's shape");

        let cache = Cache::new(fixture.directory.path().join("table"));
        let mut writer = cache.writer().expect("a writer under a fresh directory");
        let built = build(source, Decimation::none(grid), &mut (), |row| {
            writer.write_row(row)
        })
        .expect("a synthetic source and a fresh cache cannot fail");
        writer
            .publish(&built, dataset)
            .expect("the cache publishes");

        Self {
            digest: built.digest,
            ..fixture
        }
    }

    fn cache_base(&self) -> PathBuf {
        self.directory.path().join("table")
    }

    fn write_places(&self, places: &[(&str, &str, f64, f64)]) {
        let features: Vec<_> = places
            .iter()
            .map(|(name, country, lat, lon)| {
                serde_json::json!({
                    "type": "Feature",
                    "properties": {"NAME": name, "ADM0NAME": country},
                    "geometry": {"type": "Point", "coordinates": [lon, lat]},
                })
            })
            .collect();
        self.write_place_features(&features);
    }

    fn write_centres(&self, centres: &[[f64; 2]]) {
        let features: Vec<_> = centres
            .iter()
            .enumerate()
            .map(|(index, centre)| {
                serde_json::json!({
                    "type": "Feature",
                    "properties": {
                        "NAME": format!("Place {index}"),
                        "ADM0NAME": "Fixture",
                    },
                    "geometry": {"type": "Point", "coordinates": [centre[1], centre[0]]},
                })
            })
            .collect();
        self.write_place_features(&features);
    }

    fn write_place_features(&self, features: &[serde_json::Value]) {
        let path = self
            .directory
            .path()
            .join("data/settlements/populated-places-1to10m.geojson");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            path,
            serde_json::to_vec(&serde_json::json!({
                "type": "FeatureCollection",
                "features": features,
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn register_settlements(&self, rows: &[Vec<f32>]) {
        let data = self.directory.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::create_dir_all(data.join("population")).unwrap();
        std::fs::write(
            data.join("population/population-count-2020-30arcsec.tif"),
            synthetic_tiff_with(|row, col| rows[row as usize][col as usize]),
        )
        .unwrap();
        std::fs::create_dir_all(data.join("boundaries")).unwrap();
        std::fs::write(
            data.join("boundaries/gisco-countries-2024-1m.geojson"),
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"CNTR_ID":"XX","NAME_ENGL":"Fixture"},"geometry":{"type":"Polygon","coordinates":[[[-50,-50],[50,-50],[50,70],[-50,70],[-50,-50]]]}}]}"#,
        ).unwrap();
        std::fs::write(
            data.join("registry.toml"),
            r#"
[datasets.population-count-2020-30arcsec]
kind = "population-raster"
path = "data/population/population-count-2020-30arcsec.tif"
width = 36
height = 18
origin_lat = 90.0
origin_lon = -180.0
lon_step = 10.0
lat_step = -10.0
epsg = 4326
nodata = -3.402823e38

[datasets.gisco-countries-2024-1m]
kind = "boundary-vector"
path = "data/boundaries/gisco-countries-2024-1m.geojson"
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
format = "GeoJSON"
crs = "EPSG:4326"
axis_order = "longitude-latitude"
feature_count = 1
revision = "fixture"
source_id_field = "CNTR_ID"
identity_policy = "fixture"
attribution = "Fixture boundary credit"

... (+2631 lines) [see remaining: rtk proxy git -c core.fsmonitor=false show HEAD:crates/popcircles-cli/tests/commands.rs | tail -n +218]
