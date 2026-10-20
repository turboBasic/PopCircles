// The four search commands run for real, against a cache this file builds. What only a real invocation
// can check is here and nothing else: that stdout is one JSON document, that stderr stays out of it, that
// the exit code is the class the failure falls in, and that the document names the table it was answered
// from. The wire format itself is snapshotted in the library, over `report`'s own types — see ADR 0004
// and `FU-03`, whose tripwire watches `crates/popcircles/src/snapshots/` and would not see a
// snapshot kept here.
//
// No raster anywhere. `Synthetic` is public and unconditional, so the fixture streams into a real cache
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
        let grid = Self::grid();
        let rows: Vec<Vec<f32>> = (0..HEIGHT)
            .map(|row| (0..WIDTH).map(|col| cell(row, col)).collect())
            .collect();
        let source = Synthetic::new(grid, NODATA, rows).expect("the rows are the grid's shape");

        let cache = Cache::new(directory.path().join("table"));
        let mut writer = cache.writer().expect("a writer under a fresh directory");
        let built = build(source, Decimation::none(grid), &mut (), |row| {
            writer.write_row(row)
        })
        .expect("a synthetic source and a fresh cache cannot fail");
        writer
            .publish(&built, dataset)
            .expect("the cache publishes");

        Self {
            directory,
            digest: built.digest,
        }
    }

    fn cache_base(&self) -> PathBuf {
        self.directory.path().join("table")
    }

    fn ledger(&self, name: &str) -> PathBuf {
        self.directory.path().join(name)
    }

    /// The ten flags every command reading this cache takes, with `digest` so a case can pass one that
    /// names another table.
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
        run_with(&self.flags(digest), command, rest)
    }

    /// The command with this fixture's own digest, which is the case every success below is.
    fn run_ok(&self, command: &str, rest: &[&str]) -> Output {
        self.run(command, &hexadecimal(self.digest), rest)
    }

    /// The command with this fixture's own digest, at a level of the caller's choosing.
    fn run_at_level(&self, level: &str, command: &str, rest: &[&str]) -> Output {
        let flags = self.flags_at(&hexadecimal(self.digest), level);
        run_with(&flags, command, rest)
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
        run_with(&flags, command, rest)
    }
}

fn run_with(flags: &[String], command: &str, rest: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_popcircles"))
        .arg(command)
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
        assert!(output.stdout.is_empty());
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
    // Box 6's two ends and nothing between them: the table that answered, then the answer.
    assert_eq!(narration.lines().count(), 2, "{narration}");
    assert!(narration.contains("opened from"), "{narration}");
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
    let output = fixture.run(
        "population-at",
        "0x0000000000000001",
        &["--lat", "48", "--lon", "11", "--radius-km", "1200"],
    );
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    // Nothing on stdout, which is what makes a consumer's parse failure impossible rather than confusing.
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(!output.stderr.is_empty(), "a failure says why");
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
    // the name reaching a document at all: the four search commands take no `--dataset`, so what they
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
