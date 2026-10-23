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
        Self::over(dense, None)
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
            None,
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

[datasets.populated-places-1to10m]
kind = "settlement-vector"
path = "data/settlements/populated-places-1to10m.geojson"
"#,
        )
        .unwrap();
        self.write_places(&[
            ("Resolved centre", "Fixture", 45.0, 15.0),
            ("Requested point", "Fixture", 48.0, 11.0),
        ]);
    }

    fn register_two_countries(&self) {
        let data = self.directory.path().join("data");
        std::fs::write(
            data.join("boundaries/gisco-countries-2024-1m.geojson"),
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"CNTR_ID":"AA","NAME_ENGL":"Alpha"},"geometry":{"type":"Polygon","coordinates":[[[-180,-90],[0,-90],[0,90],[-180,90],[-180,-90]]]}},{"type":"Feature","properties":{"CNTR_ID":"BB","NAME_ENGL":"Beta"},"geometry":{"type":"Polygon","coordinates":[[[0,-90],[180,-90],[180,90],[0,90],[0,-90]]]}}]}"#,
        )
        .unwrap();
        let registry = data.join("registry.toml");
        let contents = std::fs::read_to_string(&registry).unwrap();
        std::fs::write(
            registry,
            contents.replace("feature_count = 1", "feature_count = 2"),
        )
        .unwrap();
    }

    fn register_path_like_country(&self) {
        let feature = serde_json::json!({
            "type": "Feature",
            "properties": {"CNTR_ID": "../victim", "NAME_ENGL": "Path-like ID"},
            "geometry": {"type": "Polygon", "coordinates": [[[-50,-50],[50,-50],[50,70],[-50,70],[-50,-50]]]},
        });
        std::fs::write(
            self.directory
                .path()
                .join("data/boundaries/gisco-countries-2024-1m.geojson"),
            serde_json::to_vec(&serde_json::json!({
                "type": "FeatureCollection",
                "features": [feature],
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn ledger(&self, name: &str) -> PathBuf {
        self.directory.path().join(name)
    }

    /// Explicit cache inputs, with `digest` so a case can pass one that names another table.
    ///
    /// `--log-level error` because `info` is the default and narrates: it is what keeps the success cases
    /// silent, and with it `one_document_naming_the_fixture`'s emptiness assertion is strictly stronger
    /// than dropping the assertion would be. `narration_follows_the_level_and_stdout_does_not` is where
    /// the narration itself is checked.
    fn flags(&self, digest: &str) -> Vec<String> {
        self.flags_at(digest, "error")
    }

    /// The same at a level the caller names. Substituted rather than appended, because a global argument is
    /// refused a second occurrence and no case wants two.
    fn flags_at(&self, digest: &str, level: &str) -> Vec<String> {
        [
            "--width",
            "36",
            "--height",
            "18",
            "--origin-lat",
            "90",
            "--origin-lon",
            "-180",
            "--lon-step",
            "10",
            "--lat-step",
            "-10",
        ]
        .iter()
        .map(|flag| (*flag).to_string())
        .chain([
            "--cache".to_string(),
            self.cache_base().to_string_lossy().into_owned(),
            "--digest".to_string(),
            digest.to_string(),
            "--log-level".to_string(),
            level.to_string(),
        ])
        .collect()
    }

    fn run(&self, command: &str, digest: &str, rest: &[&str]) -> Output {
        run_with(self.directory.path(), &self.flags(digest), command, rest)
    }

    /// The command with this fixture's own digest, which is the case every success below is.
    fn run_ok(&self, command: &str, rest: &[&str]) -> Output {
        self.run(command, &hexadecimal(self.digest), rest)
    }

    fn run_inferred(&self, command: &str, rest: &[&str]) -> Output {
        let mut flags = self.flags(&hexadecimal(self.digest));
        let at = flags.iter().position(|flag| flag == "--digest").unwrap();
        flags.drain(at..at + 2);
        run_with(self.directory.path(), &flags, command, rest)
    }

    /// The command with this fixture's own digest, at a level of the caller's choosing.
    fn run_at_level(&self, level: &str, command: &str, rest: &[&str]) -> Output {
        let flags = self.flags_at(&hexadecimal(self.digest), level);
        run_with(self.directory.path(), &flags, command, rest)
    }

    /// The same invocation with one number substituted, so a case declaring another grid differs from a
    /// success case in that number and nothing else — not in the digest, the dimensions or the steps.
    fn run_at_origin_lon(&self, lon: &str, command: &str, rest: &[&str]) -> Output {
        let mut flags = self.flags(&hexadecimal(self.digest));
        let at = flags
            .iter()
            .position(|flag| flag == "--origin-lon")
            .expect("the flag list declares an origin longitude");
        flags[at + 1] = lon.to_string();
        run_with(self.directory.path(), &flags, command, rest)
    }
}

fn run_with(directory: &std::path::Path, flags: &[String], command: &str, rest: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_popcircles"))
        .current_dir(directory)
        .args(command.split_whitespace())
        .args(flags)
        .args(rest)
        .output()
        .expect("the binary cargo just built runs")
}

/// What every success case asserts, and it is the whole of what running the binary adds over a unit test:
/// stdout is exactly one JSON document, stderr is empty so stdout stays machine-readable, the exit code is
/// zero, and `provenance.digest` names the table the fixture built.
///
/// The emptiness now holds because [`Fixture::flags`] asks for `error`, where it used to hold because
/// nothing emitted. What moved is the reason, not the claim.
fn one_document_naming_the_fixture(fixture: &Fixture, output: &Output) -> serde_json::Value {
    assert!(
        output.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");

    // `from_slice` refuses trailing content, so a second document on stdout fails here rather than being
    // read as the first.
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is one JSON document");
    assert_eq!(document["schema_version"], 1);
    assert_eq!(
        document["provenance"]["digest"],
        serde_json::Value::from(hexadecimal(fixture.digest))
    );
    // And no dataset, because this fixture is published through the library rather than by `table build`:
    // the absent case of that field, end to end through the binary rather than over a constructed value.
    assert!(document["provenance"]["dataset"].is_null(), "{document}");
    document
}

#[test]
fn population_at_answers_from_the_cache() {
    let fixture = Fixture::build();
    let output = fixture.run_ok(
        "population-at",
        &["--lat", "48", "--lon", "11", "--radius-km", "1200"],
    );
    let document = one_document_naming_the_fixture(&fixture, &output);
    // The figure `report.rs`'s snapshot pins, reached this time through the parser and a real cache.
    assert_eq!(document["result"]["population"], 820.0);
    assert_eq!(document["result"]["requested"]["lat"], 48.0);
    assert_eq!(document["result"]["centre"]["lat"], 45.0);
    assert_eq!(
        document["result"]["nearest_settlement"]["name"],
        "Resolved centre"
    );
    assert_eq!(document["result"]["nearest_settlement"]["distance_km"], 0.0);
}

#[test]
fn every_circle_command_publishes_country_accounting_from_registered_inputs() {
    let fixture = Fixture::build();
    let population_at = successful_document(&fixture.run_ok(
        "population-at",
        &["--lat", "45", "--lon", "15", "--radius-km", "0"],
    ));
    assert_eq!(
        population_at["result"]["country_breakdown"]["entries"][0]["id"],
        "XX"
    );

    let most_populous =
        successful_document(&fixture.run_ok("most-populous", &["--radius-km", "0"]));
    assert!(most_populous["result"]["country_breakdown"].is_object());

    let smallest_args = ["--share", "10", "--ledger", "smallest.json"];
    let smallest = successful_document(&fixture.run_ok("smallest-for-share", &smallest_args));
    assert!(smallest["result"]["circle"]["country_breakdown"].is_object());
    let warm = successful_document(&fixture.run_ok("smallest-for-share", &smallest_args));
    for field in ["radius_km", "centre", "population", "country_breakdown"] {
        assert_eq!(
            smallest["result"]["circle"][field],
            warm["result"]["circle"][field]
        );
    }

    let sweep = successful_document(&fixture.run_ok(
        "sweep",
        &[
            "--from",
            "10",
            "--to",
            "20",
            "--step",
            "10",
            "--ledger",
            "sweep.json",
        ],
    ));
    assert!(
        sweep["result"]["records"]
            .as_array()
            .unwrap()
            .iter()
            .all(|record| record["country_breakdown"].is_object())
    );
}

#[test]
fn all_four_commands_match_the_country_breakdown_oracle() {
    let fixture = Fixture::over(
        |row, col| match (row, col) {
            (4, 19) => 30.0,
            (4, 18 | 20 | 10) => 10.0,
            (4, 25 | 30) => 35.0,
            _ => NODATA,
        },
        None,
    );
    let boundary = fixture
        .directory
        .path()
        .join("data/boundaries/gisco-countries-2024-1m.geojson");
    std::fs::write(
        &boundary,
        r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"CNTR_ID":"A","NAME_ENGL":"Alpha"},"geometry":{"type":"MultiPolygon","coordinates":[[[[10,40],[20,40],[20,50],[10,50],[10,40]]],[[[70,40],[80,40],[80,50],[70,50],[70,40]]],[[[120,40],[130,40],[130,50],[120,50],[120,40]]]]}},{"type":"Feature","properties":{"CNTR_ID":"B","NAME_ENGL":"Beta"},"geometry":{"type":"MultiPolygon","coordinates":[[[[0,40],[10,40],[10,50],[0,50],[0,40]]],[[[-80,40],[-70,40],[-70,50],[-80,50],[-80,40]]]]}}]}"#,
    )
    .unwrap();
    let registry = fixture.directory.path().join("data/registry.toml");
    let text = std::fs::read_to_string(&registry).unwrap();
    std::fs::write(
        &registry,
        text.replace("feature_count = 1", "feature_count = 2"),
    )
    .unwrap();

    let population_at = successful_document(&fixture.run_ok(
        "population-at",
        &["--lat", "45", "--lon", "15", "--radius-km", "800"],
    ));
    assert_oracle(&population_at["result"]["country_breakdown"]);

    let most_populous =
        successful_document(&fixture.run_ok("most-populous", &["--radius-km", "800"]));
    assert_oracle(&most_populous["result"]["country_breakdown"]);

    let smallest = successful_document(&fixture.run_ok(
        "smallest-for-share",
        &["--share", "38", "--ledger", "oracle-smallest.json"],
    ));
    assert_oracle(&smallest["result"]["circle"]["country_breakdown"]);

    let sweep = successful_document(&fixture.run_ok(
        "sweep",
        &[
            "--from",
            "38",
            "--to",
            "38",
            "--step",
            "1",
            "--ledger",
            "oracle-sweep.json",
        ],
    ));
    assert_oracle(&sweep["result"]["records"][0]["country_breakdown"]);
}

fn assert_oracle(breakdown: &serde_json::Value) {
    let entries = breakdown["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2, "{breakdown}");
    assert_eq!(entries[0]["id"], "A");
    assert_eq!(entries[0]["population_inside"], 30.0);
    assert_eq!(entries[0]["country_total_population"], 100.0);
    assert_eq!(entries[0]["fraction_of_country_population"], 0.3);
    assert_eq!(entries[0]["fraction_of_circle_population"], 0.6);
    assert_eq!(entries[1]["id"], "B");
    assert_eq!(entries[1]["population_inside"], 10.0);
    assert_eq!(entries[1]["country_total_population"], 20.0);
    assert_eq!(entries[1]["fraction_of_country_population"], 0.5);
    assert_eq!(entries[1]["fraction_of_circle_population"], 0.2);
    assert_eq!(breakdown["reconciliation"]["unassigned_population"], 10.0);
}

#[test]
fn country_list_reads_only_registered_boundaries_and_sorts_the_json_result() {
    let directory = TempDir::new().unwrap();
    let boundaries = directory.path().join("data/boundaries");
    std::fs::create_dir_all(&boundaries).unwrap();
    std::fs::write(
        boundaries.join("gisco-countries-2024-1m.geojson"),
        r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"CNTR_ID":"BB","NAME_ENGL":"Beta"},"geometry":{"type":"Polygon","coordinates":[]}},{"type":"Feature","properties":{"CNTR_ID":"AA","NAME_ENGL":"Alpha"},"geometry":{"type":"Polygon","coordinates":[]}}]}"#,
    )
    .unwrap();
    std::fs::write(
        directory.path().join("data/registry.toml"),
        r#"
[datasets.gisco-countries-2024-1m]
kind = "boundary-vector"
path = "data/boundaries/gisco-countries-2024-1m.geojson"
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
format = "GeoJSON"
crs = "EPSG:4326"
axis_order = "longitude-latitude"
feature_count = 2
revision = "fixture"
source_id_field = "CNTR_ID"
identity_policy = "fixture"
attribution = "Fixture boundary credit"
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_popcircles"))
        .current_dir(directory.path())
        .args(["country", "list"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["schema_version"], 1);
    assert_eq!(document["document"], "country-list");
    assert_eq!(document["result"]["countries"][0]["id"], "AA");
    assert_eq!(document["result"]["countries"][1]["id"], "BB");
    insta::assert_json_snapshot!("country-list-command", document);
}

#[test]
fn country_smallest_publishes_identity_and_refuses_a_mismatched_ledger() {
    let fixture = Fixture::build();
    fixture.register_two_countries();
    let invoke = |country: &str| {
        run_with(
            fixture.directory.path(),
            &["--log-level".to_owned(), "error".to_owned()],
            "country smallest",
            &[
                "--country",
                country,
                "--share",
                "10",
                "--dataset",
                "population-count-2020-30arcsec",
                "--decimate",
                "1",
                "--spacing",
                "4",
                "--ledger",
                "shared-country.json",
            ],
        )
    };
    let first = invoke("AA");
    assert!(
        first.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(first.status.code(), Some(0), "{first:?}");
    let mut document: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(document["document"], "country-smallest");
    assert_eq!(document["schema_version"], 1);
    assert_eq!(document["result"]["country"]["id"], "AA");
    assert_eq!(document["result"]["country"]["name"], "Alpha");
    assert_eq!(
        document["result"]["country"]["boundary_dataset"],
        "gisco-countries-2024-1m"
    );
    assert_eq!(
        document["result"]["country"]["boundary_revision"],
        "fixture"
    );
    assert_eq!(
        document["result"]["country"]["boundary_digest"],
        "a".repeat(64)
    );
    document["result"]["ledger"]["path"] = "shared-country.json".into();
    insta::assert_json_snapshot!("country-smallest-command", document);

    let second = invoke("BB");
    assert_eq!(second.status.code(), Some(3), "{second:?}");
    assert!(
        String::from_utf8_lossy(&second.stderr).contains("does not describe the country wanted"),
        "{second:?}"
    );
    assert!(second.stdout.is_empty(), "{second:?}");
}

#[test]
fn country_id_cannot_redirect_the_default_ledger_path() {
    let fixture = Fixture::build();
    fixture.register_path_like_country();
    let output = run_with(
        fixture.directory.path(),
        &["--log-level".to_owned(), "error".to_owned()],
        "country smallest",
        &[
            "--country",
            "../victim",
            "--share",
            "10",
            "--dataset",
            "population-count-2020-30arcsec",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ledger = document["result"]["ledger"]["path"].as_str().unwrap();
    assert_eq!(ledger, "out/countries/2e2e2f76696374696d.json");
    assert!(fixture.directory.path().join(ledger).is_file());
    assert!(!fixture.directory.path().join("out/victim.json").exists());
}

#[test]
fn country_smallest_leaves_world_command_bytes_unchanged() {
    let fixture = Fixture::build();
    let args = ["--lat", "45", "--lon", "15", "--radius-km", "0"];
    let before = fixture.run_ok("population-at", &args);
    let country = run_with(
        fixture.directory.path(),
        &["--log-level".to_owned(), "error".to_owned()],
        "country smallest",
        &[
            "--country",
            "XX",
            "--share",
            "10",
            "--dataset",
            "population-count-2020-30arcsec",
            "--ledger",
            "country.json",
        ],
    );
    assert!(country.status.success(), "{country:?}");
    let after = fixture.run_ok("population-at", &args);
    assert_eq!(before.stdout, after.stdout);
    assert_eq!(before.stderr, after.stderr);
    let mut document: serde_json::Value = serde_json::from_slice(&after.stdout).unwrap();
    document["provenance"]["cache"]["header"] = "out/table.header.json".into();
    document["provenance"]["cache"]["payload"] = "out/table.payload.bin".into();
    insta::assert_json_snapshot!("country-smallest-world-compatibility", document);
}

#[test]
fn country_sweep_is_ordered_resumable_and_keeps_country_ledgers_separate() {
    let fixture = Fixture::build();
    fixture.register_two_countries();
    let invoke = |countries: &[&str], ledger_dir: &str| {
        let flags = vec!["--log-level".to_owned(), "info".to_owned()];
        let mut rest: Vec<&str> = countries
            .iter()
            .flat_map(|country| ["--country", *country])
            .collect();
        rest.extend([
            "--from",
            "10",
            "--to",
            "20",
            "--step",
            "10",
            "--dataset",
            "population-count-2020-30arcsec",
            "--spacing",
            "4",
            "--ledger-dir",
            ledger_dir,
        ]);
        run_with(fixture.directory.path(), &flags, "country sweep", &rest)
    };

    let cold = invoke(&["AA", "BB"], "out/cold-ledgers");
    assert_eq!(cold.status.code(), Some(0), "{cold:?}");
    let mut cold_doc: serde_json::Value = serde_json::from_slice(&cold.stdout).unwrap();

    let partial = invoke(&["AA"], "out/country-ledgers");
    assert_eq!(partial.status.code(), Some(0), "{partial:?}");
    let partial_doc: serde_json::Value = serde_json::from_slice(&partial.stdout).unwrap();
    assert_eq!(partial_doc["document"], "country-sweep");
    assert!(String::from_utf8_lossy(&partial.stderr).contains("AA"));

    let resumed = invoke(&["BB", "AA"], "out/country-ledgers");
    assert_eq!(resumed.status.code(), Some(0), "{resumed:?}");
    let mut resumed_doc: serde_json::Value = serde_json::from_slice(&resumed.stdout).unwrap();
    let countries = resumed_doc["result"]["countries"].as_array().unwrap();
    assert_eq!(countries.len(), 2);
    assert_eq!(countries[0]["country"]["id"], "AA");
    assert_eq!(countries[1]["country"]["id"], "BB");
    for country in countries {
        let records = country["records"].as_array().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["share"], 0.1);
        assert_eq!(records[1]["share"], 0.2);
    }
    let aa_ledger = countries[0]["ledger"]["path"].as_str().unwrap();
    let bb_ledger = countries[1]["ledger"]["path"].as_str().unwrap();
    assert_ne!(aa_ledger, bb_ledger);
    assert!(fixture.directory.path().join(aa_ledger).is_file());
    assert!(fixture.directory.path().join(bb_ledger).is_file());
    let aa_state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture.directory.path().join(aa_ledger)).unwrap())
            .unwrap();
    let bb_state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture.directory.path().join(bb_ledger)).unwrap())
            .unwrap();
    assert_eq!(aa_state["identity"]["value"]["feature_id"], "AA");
    assert_eq!(bb_state["identity"]["value"]["feature_id"], "BB");
    let stderr = String::from_utf8_lossy(&resumed.stderr);
    assert!(stderr.contains("AA") && stderr.contains("BB"), "{stderr}");

    let warm = invoke(&["AA", "BB"], "out/country-ledgers");
    assert_eq!(warm.status.code(), Some(0), "{warm:?}");
    let mut warm_doc: serde_json::Value = serde_json::from_slice(&warm.stdout).unwrap();
    for document in [&mut cold_doc, &mut resumed_doc, &mut warm_doc] {
        for country in document["result"]["countries"].as_array_mut().unwrap() {
            country["ledger"].as_object_mut().unwrap().remove("path");
            for record in country["records"].as_array_mut().unwrap() {
                record["circle"].as_object_mut().unwrap().remove("stats");
            }
        }
    }
    assert_eq!(cold_doc, resumed_doc);
    assert_eq!(warm_doc, resumed_doc);
}

#[test]
fn country_input_errors_are_clear_and_non_circle_commands_do_not_read_them() {
    let fixture = Fixture::build();
    let boundary = fixture
        .directory
        .path()
        .join("data/boundaries/gisco-countries-2024-1m.geojson");
    std::fs::remove_file(&boundary).unwrap();
    let missing = fixture.run_ok(
        "population-at",
        &["--lat", "45", "--lon", "15", "--radius-km", "0"],
    );
    assert_eq!(missing.status.code(), Some(3), "{missing:?}");
    assert_eq!(missing.stdout, [] as [u8; 0]);
    assert!(String::from_utf8_lossy(&missing.stderr).contains("mise run data:get"));

    std::fs::write(&boundary, "not GeoJSON").unwrap();
    let malformed = fixture.run_ok(
        "population-at",
        &["--lat", "45", "--lon", "15", "--radius-km", "0"],
    );
    assert_eq!(malformed.status.code(), Some(2), "{malformed:?}");
    assert_eq!(malformed.stdout, [] as [u8; 0]);

    let table = fixture.run_ok("table query", &[]);
    assert!(table.status.success(), "{table:?}");
}

#[test]
fn most_populous_answers_from_the_cache() {
    let fixture = Fixture::build();
    let output = fixture.run_ok("most-populous", &["--radius-km", "3000", "--spacing", "4"]);
    let document = one_document_naming_the_fixture(&fixture, &output);
    assert!(
        document["result"]["stats"]["blocks_pruned"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn smallest_for_share_answers_from_the_cache_and_writes_its_ledger() {
    let fixture = Fixture::build();
    let ledger = fixture.ledger("radii.json");
    let output = fixture.run_ok(
        "smallest-for-share",
        &[
            "--share",
            "25",
            "--spacing",
            "4",
            "--ledger",
            &ledger.to_string_lossy(),
        ],
    );
    let document = one_document_naming_the_fixture(&fixture, &output);
    // The ledger block is a claim about a file, so the file is what checks it.
    assert!(ledger.is_file(), "the ledger was not written");
    assert!(document["result"]["ledger"]["radii"].as_u64().unwrap() > 0);
}

#[test]
fn sweep_answers_every_share_over_one_ledger() {
    let fixture = Fixture::build();
    let ledger = fixture.ledger("sweep-radii.json");
    let output = fixture.run_ok(
        "sweep",
        &[
            "--from",
            "10",
            "--to",
            "30",
            "--step",
            "10",
            "--spacing",
            "4",
            "--ledger",
            &ledger.to_string_lossy(),
        ],
    );
    let document = one_document_naming_the_fixture(&fixture, &output);
    let records = document["result"]["records"]
        .as_array()
        .expect("the sweep publishes records");
    assert_eq!(records.len(), 3);
    // Ascending and exact, which is the contract `SweepDocument::new` holds and the reason for the percent
    // flags — read here off a document a real invocation produced.
    let shares: Vec<f64> = records
        .iter()
        .map(|record| record["target"]["share"].as_f64().unwrap())
        .collect();
    assert_eq!(shares, vec![0.1, 0.2, 0.3]);
}

#[test]
fn most_populous_labels_its_own_result_centre() {
    let fixture = Fixture::build();
    let most_args = ["--radius-km", "3000", "--spacing", "4"];
    let before =
        one_document_naming_the_fixture(&fixture, &fixture.run_ok("most-populous", &most_args));
    let centre = &before["result"]["centre"];
    fixture.write_centres(&[[
        centre["lat"].as_f64().unwrap(),
        centre["lon"].as_f64().unwrap(),
    ]]);
    let after =
        one_document_naming_the_fixture(&fixture, &fixture.run_ok("most-populous", &most_args));
    assert_fields_unchanged(
        &before["result"],
        &after["result"],
        &[
            "centre",
            "row",
            "col",
            "radius_km",
            "population",
            "total_population",
            "share_of_total",
        ],
    );
    assert_eq!(after["result"]["nearest_settlement"]["name"], "Place 0");
    assert_eq!(after["result"]["nearest_settlement"]["distance_km"], 0.0);
}

#[test]
fn smallest_for_share_labels_its_own_result_centre() {
    let fixture = Fixture::build();
    let before = one_document_naming_the_fixture(
        &fixture,
        &fixture.run_ok(
            "smallest-for-share",
            &[
                "--share",
                "25",
                "--spacing",
                "4",
                "--ledger",
                "smallest-before.json",
            ],
        ),
    );
    let circle = &before["result"]["circle"];
    fixture.write_centres(&[[
        circle["centre"]["lat"].as_f64().unwrap(),
        circle["centre"]["lon"].as_f64().unwrap(),
    ]]);
    let after = one_document_naming_the_fixture(
        &fixture,
        &fixture.run_ok(
            "smallest-for-share",
            &[
                "--share",
                "25",
                "--spacing",
                "4",
                "--ledger",
                "smallest-after.json",
            ],
        ),
    );
    assert_fields_unchanged(
        &before["result"]["circle"],
        &after["result"]["circle"],
        &[
            "centre",
            "row",
            "col",
            "radius_km",
            "population",
            "target",
            "share_achieved",
        ],
    );
    assert_eq!(
        after["result"]["circle"]["nearest_settlement"]["name"],
        "Place 0"
    );
    assert_eq!(
        after["result"]["circle"]["nearest_settlement"]["distance_km"],
        0.0
    );
}

#[test]
fn sweep_labels_each_record_at_its_own_result_centre() {
    let fixture = Fixture::build();
    let before = one_document_naming_the_fixture(
        &fixture,
        &fixture.run_ok(
            "sweep",
            &[
                "--from",
                "25",
                "--to",
                "50",
                "--step",
                "25",
                "--spacing",
                "4",
                "--ledger",
                "sweep-before.json",
            ],
        ),
    );
    let centres: Vec<_> = before["result"]["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| {
            [
                record["centre"]["lat"].as_f64().unwrap(),
                record["centre"]["lon"].as_f64().unwrap(),
            ]
        })
        .collect();
    fixture.write_centres(&centres);
    let after = one_document_naming_the_fixture(
        &fixture,
        &fixture.run_ok(
            "sweep",
            &[
                "--from",
                "25",
                "--to",
                "50",
                "--step",
                "25",
                "--spacing",
                "4",
                "--ledger",
                "sweep-after.json",
            ],
        ),
    );
    let before_records = before["result"]["records"].as_array().unwrap();
    let after_records = after["result"]["records"].as_array().unwrap();
    assert_eq!(before_records.len(), after_records.len());
    for (before, after) in before_records.iter().zip(after_records) {
        assert_fields_unchanged(
            before,
            after,
            &[
                "centre",
                "row",
                "col",
                "radius_km",
                "population",
                "target",
                "share_achieved",
            ],
        );
        assert_eq!(after["nearest_settlement"]["distance_km"], 0.0);
    }
}

fn assert_fields_unchanged(before: &serde_json::Value, after: &serde_json::Value, fields: &[&str]) {
    for field in fields {
        assert_eq!(before[*field], after[*field], "{field}");
    }
}

#[test]
fn missing_or_remote_settlements_do_not_affect_non_circle_commands() {
    let fixture = Fixture::build();
    register_fixture(&fixture);
    std::fs::write(
        fixture.directory.path().join("data/tiny.tif"),
        synthetic_tiff(),
    )
    .unwrap();
    let built = build_command(&fixture, "out/no-places", &["--origin-lon", "-180"]);
    assert_eq!(built.status.code(), Some(0), "{built:?}");
    let places_path = fixture
        .directory
        .path()
        .join("data/settlements/populated-places-1to10m.geojson");
    std::fs::remove_file(&places_path).unwrap();

    let missing = fixture.run_ok(
        "population-at",
        &["--lat", "48", "--lon", "11", "--radius-km", "1200"],
    );
    assert_eq!(missing.status.code(), Some(3), "{missing:?}");
    assert!(missing.stdout.is_empty(), "{missing:?}");
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("mise run data:get"),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );

    let grid = Command::new(env!("CARGO_BIN_EXE_popcircles"))
        .current_dir(fixture.directory.path())
        .args([
            "grid",
            "describe",
            "--width",
            "36",
            "--height",
            "18",
            "--origin-lat",
            "90",
            "--origin-lon",
            "-180",
            "--lon-step",
            "10",
            "--lat-step",
            "-10",
        ])
        .output()
        .unwrap();
    assert_eq!(grid.status.code(), Some(0), "{grid:?}");
    let table_query = fixture.run_ok("table query", &[]);
    assert_eq!(table_query.status.code(), Some(0), "{table_query:?}");
    let distance = Command::new(env!("CARGO_BIN_EXE_popcircles"))
        .current_dir(fixture.directory.path())
        .args(["distance", "0", "0", "1", "1"])
        .output()
        .unwrap();
    assert_eq!(distance.status.code(), Some(0), "{distance:?}");

    fixture.write_places(&[("Remote", "Fixture", 0.0, 0.0)]);
    let ocean = fixture.run_ok(
        "population-at",
        &["--lat", "48", "--lon", "11", "--radius-km", "1200"],
    );
    let ocean = one_document_naming_the_fixture(&fixture, &ocean);
    assert!(ocean["result"].get("nearest_settlement").is_none());
}

fn search_parameters<'a>(command: &str, ledger: &'a str) -> Vec<&'a str> {
    match command {
        "most-populous" => vec!["--radius-km", "3000"],
        "smallest-for-share" => vec!["--share", "100", "--ledger", ledger],
        "sweep" => vec![
            "--from", "25", "--to", "100", "--step", "75", "--ledger", ledger,
        ],
        _ => panic!("a searching command"),
    }
}

const CACHE_READERS: [&str; 5] = [
    "table query",
    "population-at",
    "most-populous",
    "smallest-for-share",
    "sweep",
];

fn cached_parameters<'a>(command: &str, ledger: &'a str) -> Vec<&'a str> {
    match command {
        "table query" => vec![],
        "population-at" => vec!["--lat", "48", "--lon", "11", "--radius-km", "1200"],
        _ => search_parameters(command, ledger),
    }
}

fn successful_document(output: &Output) -> serde_json::Value {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    serde_json::from_slice(&output.stdout).expect("stdout is one JSON document")
}

#[test]
fn every_cache_reader_infers_the_same_digest_and_answers() {
    for dataset in [None, Some("population-count-2020-30arcsec")] {
        let fixture = Fixture::over(dense, dataset);
        for command in CACHE_READERS {
            let inferred_path = fixture.ledger(&format!("{command}-inferred.json"));
            let explicit_path = fixture.ledger(&format!("{command}-explicit.json"));
            let inferred_path = inferred_path.to_string_lossy();
            let explicit_path = explicit_path.to_string_lossy();
            let parameters = cached_parameters(command, &inferred_path);
            let mut inferred = successful_document(&fixture.run_inferred(command, &parameters));
            let mut explicit = successful_document(
                &fixture.run_ok(command, &cached_parameters(command, &explicit_path)),
            );
            if command == "table query" {
                assert_eq!(inferred["result"]["digest"], hexadecimal(fixture.digest));
            } else {
                assert_eq!(
                    inferred["provenance"]["digest"],
                    hexadecimal(fixture.digest)
                );
                assert_eq!(
                    inferred["provenance"]["dataset"],
                    serde_json::json!(dataset)
                );
            }

            if matches!(command, "smallest-for-share" | "sweep") {
                let warm = successful_document(&fixture.run_inferred(command, &parameters));
                assert_same_answers(&warm, &inferred);
                for answer in answers(&warm) {
                    assert_eq!(answer["stats"]["radii_evaluated"], 0);
                    assert!(answer["stats"]["radii_reused"].as_u64().unwrap() > 0);
                }
                inferred["result"]["ledger"]["path"] = serde_json::Value::Null;
                explicit["result"]["ledger"]["path"] = serde_json::Value::Null;
            }
            assert_same_bits(&inferred, &explicit);
        }
    }
}

#[test]
fn inferred_digests_still_refuse_another_tables_ledger() {
    let fixture = Fixture::build();
    let other = Fixture::clustered();
    assert_ne!(fixture.digest, other.digest);
    for command in ["smallest-for-share", "sweep"] {
        let ledger = fixture.ledger(&format!("{command}.json"));
        let path = ledger.to_string_lossy();
        let parameters = cached_parameters(command, &path);
        successful_document(&fixture.run_inferred(command, &parameters));
        let before = std::fs::read(&ledger).unwrap();
        let output = other.run_inferred(command, &parameters);
        assert_eq!(output.status.code(), Some(3), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        let complaint = String::from_utf8_lossy(&output.stderr);
        assert!(complaint.contains("ledger"), "{complaint}");
        assert!(complaint.contains("digest"), "{complaint}");
        assert_eq!(std::fs::read(&ledger).unwrap(), before);
    }
}

fn answers(document: &serde_json::Value) -> Vec<&serde_json::Value> {
    let result = &document["result"];
    if let Some(records) = result["records"].as_array() {
        records.iter().collect()
    } else {
        vec![result.get("circle").unwrap_or(result)]
    }
}

fn assert_same_bits(actual: &serde_json::Value, expected: &serde_json::Value) {
    match (actual, expected) {
        (serde_json::Value::Number(a), serde_json::Value::Number(b)) => {
            assert_eq!(a.as_f64().unwrap().to_bits(), b.as_f64().unwrap().to_bits());
        }
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            assert_eq!(a.len(), b.len());
            for (key, value) in a {
                assert_same_bits(value, &b[key]);
            }
        }
        _ => assert_eq!(actual, expected),
    }
}

fn assert_same_answers(actual: &serde_json::Value, expected: &serde_json::Value) {
    let actual = answers(actual);
    let expected = answers(expected);
    assert_eq!(actual.len(), expected.len());
    for (a, b) in actual.iter().zip(expected) {
        for field in [
            "centre",
            "row",
            "col",
            "radius_km",
            "population",
            "total_population",
            "share_of_total",
            "target",
            "share_achieved",
            "short_below",
            "covers_whole_grid",
            "predicate_slack_persons",
            "tolerance_persons",
            "ambiguity",
        ] {
            assert_same_bits(&a[field], &b[field]);
        }
    }
}

#[test]
fn commands_derive_spacing_and_reuse_ledgers_across_policies() {
    for fixture in [Fixture::build(), Fixture::clustered()] {
        for command in ["most-populous", "smallest-for-share", "sweep"] {
            let auto_ledger = fixture.ledger(&format!("{command}-auto.json"));
            let explicit_ledger = fixture.ledger(&format!("{command}-explicit.json"));
            let auto_path = auto_ledger.to_string_lossy();
            let explicit_path = explicit_ledger.to_string_lossy();
            let auto_flags = search_parameters(command, &auto_path);
            let mut explicit_flags = search_parameters(command, &explicit_path);
            explicit_flags.extend(["--spacing", "4"]);
            let automatic =
                one_document_naming_the_fixture(&fixture, &fixture.run_ok(command, &auto_flags));
            let explicit = one_document_naming_the_fixture(
                &fixture,
                &fixture.run_ok(command, &explicit_flags),
            );
            assert_same_answers(&automatic, &explicit);
            if command == "most-populous" {
                continue;
            }
            let mut override_auto_ledger = auto_flags.clone();
            override_auto_ledger.extend(["--spacing", "4"]);
            let explicit_warm = one_document_naming_the_fixture(
                &fixture,
                &fixture.run_ok(command, &override_auto_ledger),
            );
            let auto_warm = one_document_naming_the_fixture(
                &fixture,
                &fixture.run_ok(command, &search_parameters(command, &explicit_path)),
            );
            for warm in [&explicit_warm, &auto_warm] {
                assert_same_answers(warm, &automatic);
                for answer in answers(warm) {
                    assert_eq!(answer["stats"]["radii_evaluated"], 0);
                    assert!(answer["stats"]["radii_reused"].as_u64().unwrap() > 0);
                    for value in answer["stats"]["searched"].as_object().unwrap().values() {
                        assert_eq!(value, 0);
                    }
                }
            }
        }
    }
}

#[test]
fn zero_spacing_is_refused_before_cache_access_on_every_search_command() {
    let fixture = Fixture::build();
    for command in ["most-populous", "smallest-for-share", "sweep"] {
        let mut parameters = search_parameters(command, "unused-ledger.json");
        parameters.extend(["--spacing", "0"]);
        let output = fixture.run(command, "0x0000000000000001", &parameters);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(output.stdout, [] as [u8; 0]);
        assert!(String::from_utf8_lossy(&output.stderr).contains("--spacing"));
    }
}

/// One invocation at two levels, which is what makes the flag rather than the code path the thing being
/// checked.
#[test]
fn narration_follows_the_level_and_stdout_does_not() {
    let fixture = Fixture::build();
    let circle = ["--lat", "48", "--lon", "11", "--radius-km", "1200"];

    let narrated = fixture.run_at_level("info", "population-at", &circle);
    assert_eq!(narrated.status.code(), Some(0), "{narrated:?}");
    let narration = String::from_utf8_lossy(&narrated.stderr);
    assert_eq!(narration.lines().count(), 3, "{narration}");
    assert!(narration.contains("opened from"), "{narration}");
    assert!(
        narration.contains("country accounting 1/1: XX"),
        "{narration}"
    );
    assert!(narration.contains("holds 820"), "{narration}");

    let silent = fixture.run_at_level("warn", "population-at", &circle);
    assert_eq!(silent.status.code(), Some(0), "{silent:?}");
    assert!(
        silent.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&silent.stderr)
    );

    // Byte for byte, which is the property box 6 exists to protect: the level governs stderr and reaches
    // the document not at all.
    assert_eq!(narrated.stdout, silent.stdout);
}

/// The `warn` and its absence, one command at one level over two caches, so what differs is the answer
/// rather than the flags.
#[test]
fn an_unseparated_answer_warns_on_stderr_and_a_separated_one_says_nothing() {
    let clustered = Fixture::clustered();
    let ledger = clustered.ledger("clustered-radii.json");
    let warned = clustered.run_at_level(
        "warn",
        "smallest-for-share",
        &[
            "--share",
            "100",
            "--spacing",
            "4",
            "--ledger",
            &ledger.to_string_lossy(),
        ],
    );
    assert_eq!(warned.status.code(), Some(0), "{warned:?}");
    let record = String::from_utf8_lossy(&warned.stderr);
    // One record, not one per probe: the span is a property of the answer.
    assert_eq!(record.lines().count(), 1, "{record}");
    assert!(record.contains("1572"), "{record}");
    assert!(record.contains("2048"), "{record}");
    // And the document is still exactly one document, which is what keeps the record a second surface
    // rather than a second answer.
    let document: serde_json::Value =
        serde_json::from_slice(&warned.stdout).expect("stdout is one JSON document");
    assert_eq!(document["result"]["circle"]["radius_km"], 1572);

    // The ordinary answer at the same level over the dense cache: 485 people clear of the target, so there
    // is nothing to disclose and stderr stays empty.
    let dense = Fixture::build();
    let quiet = dense.run_at_level(
        "warn",
        "smallest-for-share",
        &[
            "--share",
            "25",
            "--spacing",
            "4",
            "--ledger",
            &dense.ledger("dense-radii.json").to_string_lossy(),
        ],
    );
    one_document_naming_the_fixture(&dense, &quiet);
}

#[test]
fn a_digest_naming_another_table_is_missing_data_and_prints_nothing() {
    let fixture = Fixture::build();
    for command in CACHE_READERS {
        let ledger = fixture.ledger("unused.json");
        let path = ledger.to_string_lossy();
        let output = fixture.run(
            command,
            "0x0000000000000001",
            &cached_parameters(command, &path),
        );
        assert_eq!(output.status.code(), Some(3), "{output:?}");
        // Nothing on stdout, which is what makes a consumer's parse failure impossible rather than confusing.
        assert!(output.stdout.is_empty(), "{output:?}");
        let complaint = String::from_utf8_lossy(&output.stderr);
        assert!(complaint.contains("digest"), "{complaint}");
        assert!(!ledger.exists());
    }
}

#[test]
fn a_grid_the_cache_was_not_built_over_is_missing_data_and_prints_nothing() {
    // The whole of the grid attestation in one invocation: every flag a success case passes, with the columns half a
    // turn from where the table's are. The digest is the one the build reported, the width, the height,
    // the steps and the factor all agree, and before this the command answered with a population.
    let fixture = Fixture::build();
    let output = fixture.run_at_origin_lon(
        "0",
        "population-at",
        &["--lat", "48", "--lon", "11", "--radius-km", "1200"],
    );
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    // And the refusal names the number that differed rather than sending its reader to a hex editor.
    let complaint = String::from_utf8_lossy(&output.stderr);
    assert!(complaint.contains("origin longitude"), "{complaint}");
}

#[test]
fn a_coordinate_off_the_grid_is_bad_input() {
    let fixture = Fixture::build();
    // The outer southern boundary lies in no cell, and the cache opens cleanly first — so this is the
    // coordinate being refused rather than the table.
    let output = fixture.run_ok(
        "population-at",
        &["--lat", "-90", "--lon", "0", "--radius-km", "1200"],
    );
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
}

#[test]
fn a_build_naming_no_registered_dataset_is_refused_before_any_raster_is_opened() {
    // The one case that reaches the registry, and it is here rather than in a unit test because what it
    // asserts is the exit code and the sentence a person sees. From the repository root, because the
    // registry path is resolved against the working directory and a test's is its own package.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO_BIN_EXE_popcircles"))
        .current_dir(&root)
        .args(["table", "build", "--dataset", "nonesuch"])
        .output()
        .expect("the binary cargo just built runs");

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let complaint = String::from_utf8_lossy(&output.stderr);
    assert!(
        complaint.contains("not a registered population raster"),
        "{complaint}"
    );
    assert!(
        complaint.contains("population-count-2020-30arcsec"),
        "{complaint}"
    );
}

#[test]
fn a_document_answered_from_a_named_table_names_that_dataset() {
    // The other side of the assertion in `one_document_naming_the_fixture`, and the only thing that pins
    // the name reaching a document at all: the four search commands publish the header name, so what they
    // publish comes off the header of the table they opened. A command that stopped reading it would leave
    // every other case in this file green.
    let fixture = Fixture::named("population-count-2020-30arcsec");
    let output = fixture.run_ok(
        "population-at",
        &["--lat", "48", "--lon", "11", "--radius-km", "1200"],
    );

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is one JSON document");
    assert_eq!(
        document["provenance"]["dataset"],
        serde_json::Value::from("population-count-2020-30arcsec")
    );
}

struct TiffTag {
    id: u16,
    kind: u16,
    count: u32,
    bytes: Vec<u8>,
}

fn tiff_tag(id: u16, kind: u16, count: usize, bytes: Vec<u8>) -> TiffTag {
    TiffTag {
        id,
        kind,
        count: u32::try_from(count).expect("fixture tag count fits TIFF"),
        bytes,
    }
}

fn tiff_short(id: u16, value: u16) -> TiffTag {
    tiff_tag(id, 3, 1, value.to_le_bytes().to_vec())
}

fn tiff_long(id: u16, value: u32) -> TiffTag {
    tiff_tag(id, 4, 1, value.to_le_bytes().to_vec())
}

fn tiff_doubles(id: u16, values: &[f64]) -> TiffTag {
    tiff_tag(
        id,
        12,
        values.len(),
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect(),
    )
}

fn assemble_tiff(mut tags: Vec<TiffTag>, pixels: &[u8]) -> Vec<u8> {
    tags.sort_by_key(|entry| entry.id);
    let ifd_end = 8 + 2 + tags.len() * 12 + 4;
    let mut cursor = ifd_end;
    let offsets: Vec<_> = tags
        .iter()
        .map(|entry| {
            if entry.bytes.len() <= 4 {
                None
            } else {
                if !cursor.is_multiple_of(2) {
                    cursor += 1;
                }
                let offset = u32::try_from(cursor).expect("fixture TIFF offset fits");
                cursor += entry.bytes.len();
                Some(offset)
            }
        })
        .collect();
    let strip = tags
        .iter()
        .position(|entry| entry.id == 273)
        .expect("fixture has strip offset");
    tags[strip].bytes = u32::try_from(cursor)
        .expect("fixture TIFF offset fits")
        .to_le_bytes()
        .to_vec();

    let mut out = Vec::new();
    out.extend_from_slice(b"II");
    out.extend_from_slice(&42u16.to_le_bytes());
    out.extend_from_slice(&8u32.to_le_bytes());
    out.extend_from_slice(
        &u16::try_from(tags.len())
            .expect("fixture has few tags")
            .to_le_bytes(),
    );
    for (entry, offset) in tags.iter().zip(offsets) {
        out.extend_from_slice(&entry.id.to_le_bytes());
        out.extend_from_slice(&entry.kind.to_le_bytes());
        out.extend_from_slice(&entry.count.to_le_bytes());
        if let Some(offset) = offset {
            out.extend_from_slice(&offset.to_le_bytes());
        } else {
            let mut inline = entry.bytes.clone();
            inline.resize(4, 0);
            out.extend_from_slice(&inline);
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    for entry in &tags {
        if entry.bytes.len() > 4 {
            if !out.len().is_multiple_of(2) {
                out.push(0);
            }
            out.extend_from_slice(&entry.bytes);
        }
    }
    assert_eq!(out.len(), cursor);
    out.extend_from_slice(pixels);
    out
}

fn synthetic_tiff() -> Vec<u8> {
    synthetic_tiff_with(dense)
}

fn synthetic_tiff_with(cell: impl Fn(u32, u32) -> f32) -> Vec<u8> {
    let mut pixels = Vec::new();
    for row in 0..HEIGHT {
        for col in 0..WIDTH {
            pixels.extend_from_slice(&cell(row, col).to_le_bytes());
        }
    }
    let mut nodata = b"-3.40282306073709653e+38".to_vec();
    nodata.push(0);
    let keys: Vec<u16> = [1, 1, 0, 3, 1024, 0, 1, 2, 1025, 0, 1, 1, 2048, 0, 1, 4326].into();
    let tags = vec![
        tiff_long(256, WIDTH),
        tiff_long(257, HEIGHT),
        tiff_short(258, 32),
        tiff_short(259, 1),
        tiff_short(262, 1),
        tiff_long(273, 0),
        tiff_short(277, 1),
        tiff_long(278, HEIGHT),
        tiff_long(279, u32::try_from(pixels.len()).expect("fixture fits TIFF")),
        tiff_short(284, 1),
        tiff_doubles(33550, &[10.0, 10.0, 0.0]),
        tiff_doubles(33922, &[0.0, 0.0, 0.0, -180.0, 90.0, 0.0]),
        tiff_short(339, 3),
        tiff_tag(
            34735,
            3,
            keys.len(),
            keys.iter().flat_map(|value| value.to_le_bytes()).collect(),
        ),
        tiff_tag(42113, 2, nodata.len(), nodata),
    ];
    assemble_tiff(tags, &pixels)
}

fn register_fixture(fixture: &Fixture) {
    std::fs::create_dir_all(fixture.directory.path().join("data")).unwrap();
    std::fs::create_dir_all(fixture.directory.path().join("data/boundaries")).unwrap();
    std::fs::write(
        fixture.directory.path().join("data/boundaries/gisco-countries-2024-1m.geojson"),
        r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"CNTR_ID":"XX","NAME_ENGL":"Fixture"},"geometry":{"type":"Polygon","coordinates":[[[-50,-50],[50,-50],[50,70],[-50,70],[-50,-50]]]}}]}"#,
    ).unwrap();
    std::fs::write(
        fixture.directory.path().join("data/registry.toml"),
        r#"
[datasets.tiny]
kind = "population-raster"
path = "data/tiny.tif"
width = 36
height = 18
origin_lat = 90.0
origin_lon = 0.0
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

[datasets.populated-places-1to10m]
kind = "settlement-vector"
path = "data/settlements/populated-places-1to10m.geojson"
"#,
    )
    .unwrap();
    fixture.write_places(&[("Fixture", "Fixture", 0.0, 0.0)]);
}

fn named_command(fixture: &Fixture, command: &str, rest: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_popcircles"))
        .current_dir(fixture.directory.path())
        .args(command.split_whitespace())
        .args(["--dataset", "tiny", "--log-level", "error"])
        .args(rest)
        .output()
        .unwrap()
}

#[test]
fn circle_commands_use_registered_country_inputs_alongside_dataset_grid_defaults() {
    let fixture = Fixture::build();
    register_fixture(&fixture);
    std::fs::write(
        fixture.directory.path().join("data/tiny.tif"),
        synthetic_tiff(),
    )
    .unwrap();
    assert!(
        fixture
            .directory
            .path()
            .join("data/population/population-count-2020-30arcsec.tif")
            .exists()
    );
    let cache = fixture.cache_base();
    let cache = cache.to_str().unwrap();
    let described = named_command(&fixture, "grid describe", &[]);
    assert!(described.status.success(), "{described:?}");
    let overridden = named_command(&fixture, "grid describe", &["--origin-lon", "-180"]);
    let explicit = run_with(
        fixture.directory.path(),
        &fixture.flags(&hexadecimal(fixture.digest))[..12],
        "grid describe",
        &[],
    );
    assert_eq!(overridden.stdout, explicit.stdout);
    assert_ne!(described.stdout, overridden.stdout);
    for command in CACHE_READERS {
        let ledger = fixture.ledger(&format!("named-{command}.json"));
        let ledger = ledger.to_str().unwrap();
        let params = cached_parameters(command, ledger);
        let mut rest = vec!["--cache", cache, "--origin-lon", "-180"];
        rest.extend_from_slice(&params);
        let output = named_command(&fixture, command, &rest);
        assert!(output.status.success(), "{command}: {output:?}");
        let explicit_ledger = fixture.ledger(&format!("explicit-{command}.json"));
        let explicit = fixture.run_inferred(
            command,
            &cached_parameters(command, explicit_ledger.to_str().unwrap()),
        );
        let mut named: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let mut explicit: serde_json::Value = serde_json::from_slice(&explicit.stdout).unwrap();
        named["result"]["ledger"] = serde_json::Value::Null;
        explicit["result"]["ledger"] = serde_json::Value::Null;
        assert_eq!(named, explicit, "{command}");
        assert!(named["provenance"]["dataset"].is_null());
        let registry = fixture.directory.path().join("data/registry.toml");
        let text = std::fs::read_to_string(&registry).unwrap();
        std::fs::write(
            &registry,
            text.replace("origin_lon = 0.0", "origin_lon = -180.0"),
        )
        .unwrap();
        let mut defaults = vec!["--cache", cache];
        defaults.extend_from_slice(&params);
        let dataset_only = named_command(&fixture, command, &defaults);
        assert!(dataset_only.status.success(), "{command}: {dataset_only:?}");
        std::fs::write(&registry, text).unwrap();
        let mut wrong = vec!["--cache", cache];
        wrong.extend_from_slice(&params);
        let output = named_command(&fixture, command, &wrong);
        assert_eq!(output.status.code(), Some(3), "{command}: {output:?}");
        assert_eq!(output.stdout, [] as [u8; 0]);
    }
}

#[test]
fn dataset_and_explicit_grid_refusals_keep_their_error_classes() {
    let fixture = Fixture::build();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_popcircles"))
            .current_dir(fixture.directory.path())
            .args(args)
            .output()
            .unwrap()
    };
    let missing = run(&["grid", "describe", "--width", "36"]);
    assert_eq!(missing.status.code(), Some(2));
    let complaint = String::from_utf8_lossy(&missing.stderr);
    for flag in [
        "--height",
        "--origin-lat",
        "--origin-lon",
        "--lon-step",
        "--lat-step",
    ] {
        assert!(complaint.contains(flag), "{complaint}");
    }
    let explicit = Command::new(env!("CARGO_BIN_EXE_popcircles"))
        .current_dir(fixture.directory.path())
        .args(["grid", "describe"])
        .args(&fixture.flags(&hexadecimal(fixture.digest))[..12])
        .output()
        .unwrap();
    assert!(explicit.status.success(), "{explicit:?}");
    std::fs::remove_file(fixture.directory.path().join("data/registry.toml")).unwrap();
    let absent = named_command(&fixture, "grid describe", &[]);
    assert_eq!(absent.status.code(), Some(3));
    register_fixture(&fixture);
    for name in ["unknown", "vector"] {
        let output = run(&["grid", "describe", "--dataset", name]);
        assert_eq!(output.status.code(), Some(2));
        let complaint = String::from_utf8_lossy(&output.stderr);
        assert!(
            complaint.contains("the registered ones are tiny"),
            "{complaint}"
        );
    }
    for flags in [
        vec!["--width", "0"],
        vec!["--lat-step", "0"],
        vec!["--origin-lat", "100"],
    ] {
        let output = named_command(&fixture, "grid describe", &flags);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
    }
    for factor in ["0", "7"] {
        let output = named_command(&fixture, "table query", &["--decimate", factor]);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert_eq!(output.stdout, [] as [u8; 0]);
    }
    std::fs::write(
        fixture.directory.path().join("data/registry.toml"),
        "malformed [",
    )
    .unwrap();
    let malformed = named_command(&fixture, "grid describe", &[]);
    assert_eq!(malformed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&malformed.stderr).contains("not the TOML document"));
}

fn build_command(fixture: &Fixture, cache: &str, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_popcircles"))
        .current_dir(fixture.directory.path())
        .args([
            "table",
            "build",
            "--dataset",
            "tiny",
            "--cache",
            cache,
            "--log-level",
            "error",
        ])
        .args(extra)
        .output()
        .unwrap()
}

fn each_bad_registry_value_is_refused_and_corrected_alone(
    fixture: &Fixture,
    registry: &std::path::Path,
    matching: &str,
) {
    let cases = [
        (
            "grid",
            matching.replace("origin_lon = -180.0", "origin_lon = 0.0"),
            vec!["--origin-lon", "-180"],
        ),
        (
            "epsg",
            matching.replace("epsg = 4326", "epsg = 3857"),
            vec!["--epsg", "4326"],
        ),
        (
            "nodata",
            matching.replace("nodata = -3.402823e38", "nodata = 0.0"),
            vec!["--nodata", "-3.402823e38"],
        ),
    ];
    for (field, incorrect, correction) in cases {
        std::fs::write(registry, incorrect).unwrap();
        let refused = build_command(fixture, &format!("out/{field}-refused"), &[]);
        assert_ne!(refused.status.code(), Some(0), "{field}: {refused:?}");
        assert_eq!(refused.stdout, [] as [u8; 0]);
        let corrected = build_command(fixture, &format!("out/{field}-corrected"), &correction);
        assert!(
            corrected.status.success(),
            "{field}: {}",
            String::from_utf8_lossy(&corrected.stderr)
        );
    }
}

#[test]
fn dataset_build_and_all_cached_commands_agree_with_explicit_inputs() {
    let fixture = Fixture::build();
    register_fixture(&fixture);
    let registry = fixture.directory.path().join("data/registry.toml");
    let text = std::fs::read_to_string(&registry).unwrap();
    std::fs::write(
        &registry,
        text.replace("origin_lon = 0.0", "origin_lon = -180.0"),
    )
    .unwrap();
    std::fs::write(
        fixture.directory.path().join("data/tiny.tif"),
        synthetic_tiff(),
    )
    .unwrap();
    let built = build_command(&fixture, "out/short-form", &["--decimate", "2"]);
    let built = successful_document(&built);
    let cache = fixture.directory.path().join("out/short-form");
    let cache = cache.to_str().unwrap();
    let mut flags = fixture.flags(built["result"]["digest"].as_str().unwrap());
    let at = flags.iter().position(|flag| flag == "--cache").unwrap();
    flags[at + 1] = cache.to_string();
    flags.extend(["--decimate".to_string(), "2".to_string()]);

    for command in CACHE_READERS {
        let named_ledger = fixture.ledger(&format!("short-{command}.json"));
        let explicit_ledger = fixture.ledger(&format!("long-{command}.json"));
        let mut rest = vec!["--cache", cache, "--decimate", "2"];
        rest.extend(cached_parameters(command, named_ledger.to_str().unwrap()));
        let mut named = successful_document(&named_command(&fixture, command, &rest));
        let mut explicit = successful_document(&run_with(
            fixture.directory.path(),
            &flags,
            command,
            &cached_parameters(command, explicit_ledger.to_str().unwrap()),
        ));
        if matches!(command, "smallest-for-share" | "sweep") {
            named["result"]["ledger"]["path"] = serde_json::Value::Null;
            explicit["result"]["ledger"]["path"] = serde_json::Value::Null;
        }
        assert_same_bits(&named, &explicit);
        if command == "table query" {
            assert_eq!(named["result"]["digest"], built["result"]["digest"]);
        } else {
            assert_eq!(named["provenance"]["dataset"], "tiny");
            assert_eq!(named["provenance"]["digest"], built["result"]["digest"]);
        }
    }
}

#[test]
fn table_build_uses_registered_defaults_and_explicit_raster_overrides() {
    let fixture = Fixture::build();
    register_fixture(&fixture);
    std::fs::write(
        fixture.directory.path().join("data/tiny.tif"),
        synthetic_tiff(),
    )
    .unwrap();
    let registry = fixture.directory.path().join("data/registry.toml");
    let baseline = std::fs::read_to_string(&registry).unwrap();
    let matching = baseline.replace("origin_lon = 0.0", "origin_lon = -180.0");
    std::fs::write(&registry, &matching).unwrap();
    let defaults = build_command(&fixture, "out/default", &[]);
    assert!(defaults.status.success(), "{defaults:?}");
    let report: serde_json::Value = serde_json::from_slice(&defaults.stdout).unwrap();
    assert_eq!(report["result"]["grid"]["origin"]["lon"], -180.0);

    each_bad_registry_value_is_refused_and_corrected_alone(&fixture, &registry, &matching);

    let wrong = matching
        .replace("origin_lon = -180.0", "origin_lon = 0.0")
        .replace("epsg = 4326", "epsg = 3857")
        .replace("nodata = -3.402823e38", "nodata = 0.0");
    std::fs::write(&registry, &wrong).unwrap();
    let built = build_command(
        &fixture,
        "out/overridden",
        &[
            "--origin-lon",
            "-180",
            "--epsg",
            "4326",
            "--nodata",
            "-3.402823e38",
        ],
    );
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&built.stdout).unwrap();
    assert_eq!(
        report["result"]["digest"],
        format!("{:#018x}", fixture.digest)
    );
    std::fs::write(&registry, &matching).unwrap();
    let query = Command::new(env!("CARGO_BIN_EXE_popcircles"))
        .current_dir(fixture.directory.path())
        .args([
            "population-at",
            "--dataset",
            "tiny",
            "--origin-lon",
            "-180",
            "--cache",
            "out/overridden",
            "--log-level",
            "error",
            "--lat",
            "45",
            "--lon",
            "5",
            "--radius-km",
            "1000",
        ])
        .output()
        .unwrap();
    assert!(query.status.success(), "{query:?}");
    let query: serde_json::Value = serde_json::from_slice(&query.stdout).unwrap();
    assert_eq!(query["provenance"]["digest"], report["result"]["digest"]);
    assert_eq!(query["provenance"]["dataset"], "tiny");

    let bad_dataset = build_command(
        &fixture,
        "out/not-registered",
        &["--raster", "data/tiny.tif"],
    );
    assert_eq!(bad_dataset.status.code(), Some(2));
    assert_eq!(bad_dataset.stdout, [] as [u8; 0]);
}
