#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn country_ids(root: &Path) -> Vec<String> {
    let listed = Command::new(env!("CARGO_BIN_EXE_popcircles"))
        .current_dir(root)
        .args(["--log-level", "error", "country", "list"])
        .output()
        .expect("country list process starts");
    assert!(listed.status.success(), "{listed:?}");
    let listed: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    let ids: Vec<String> = listed["result"]["countries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|country| country["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids.len(), 263);
    ids
}

fn run_sweep(root: &Path, ids: &[String], ledger_dir: &Path) -> (Output, f64) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_popcircles"));
    command
        .current_dir(root)
        .args(["--log-level", "info", "country", "sweep"]);
    for id in ids {
        command.args(["--country", id]);
    }
    command.args([
        "--from",
        "10",
        "--to",
        "100",
        "--step",
        "10",
        "--dataset",
        "population-count-2020-30arcsec",
        "--decimate",
        "10",
        "--ledger-dir",
    ]);
    command.arg(ledger_dir);
    let started = Instant::now();
    let output = command.output().expect("country sweep process starts");
    (output, started.elapsed().as_secs_f64())
}

fn outcome_inventory(
    countries: &[serde_json::Value],
    ids: &[String],
    stderr: &str,
) -> (BTreeSet<String>, BTreeSet<String>, String) {
    let successful: BTreeSet<String> = countries
        .iter()
        .map(|country| country["country"]["id"].as_str().unwrap().to_owned())
        .collect();
    let no_owned: BTreeSet<String> = ids
        .iter()
        .filter(|id| {
            stderr
                .lines()
                .any(|line| line.starts_with(&format!("{id}: no owned population cells")))
        })
        .cloned()
        .collect();
    let mut outcomes = String::new();
    for id in &successful {
        let _ = writeln!(outcomes, "{id}\tresult");
    }
    for id in &no_owned {
        let _ = writeln!(outcomes, "{id}\tno-owned-cells");
    }
    (successful, no_owned, outcomes)
}

fn check_results(
    countries: &[serde_json::Value],
    ids: &[String],
    successful: &BTreeSet<String>,
    no_owned: &BTreeSet<String>,
) {
    assert!(successful.is_disjoint(no_owned));
    assert_eq!(
        successful.union(no_owned).cloned().collect::<BTreeSet<_>>(),
        ids.iter().cloned().collect()
    );

    for country in countries {
        let id = country["country"]["id"].as_str().unwrap();
        let records = country["records"].as_array().unwrap();
        assert_eq!(records.len(), 10, "{id}");
        let mut previous_radius = 0;
        for record in records {
            let circle = &record["circle"];
            let radius = circle["radius_km"].as_u64().unwrap();
            assert!(
                radius >= previous_radius,
                "{id}: {previous_radius} > {radius}"
            );
            previous_radius = radius;
            if record["share"] == 1.0 {
                assert!(
                    circle["population"].as_f64().unwrap()
                        >= circle["target"]["persons"].as_f64().unwrap(),
                    "{id}: {circle}"
                );
            }
        }
    }
}

#[test]
#[ignore = "reads the registered raster and GISCO boundaries; run `mise run test:validate`"]
fn every_registered_country_answers_the_decimated_share_sweep() {
    let root = repository_root();
    let raster = root.join("data/population/population-count-2020-30arcsec.tif");
    let boundary = root.join("data/boundaries/gisco-countries-2024-1m.geojson");
    if !raster.is_file() || !boundary.is_file() {
        eprintln!(
            "skipping country validation: registered raster or GISCO boundary is absent; run `mise run data:get`"
        );
        return;
    }

    let ids = country_ids(&root);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    let run_dir = root.join(format!("tmp/gh-18/country-sweep-{timestamp}"));
    let ledger_dir = run_dir.join("ledgers");
    fs::create_dir_all(&ledger_dir).expect("fresh run directory is created");
    let (output, elapsed) = run_sweep(&root, &ids, &ledger_dir);
    fs::write(run_dir.join("country-sweep.json"), &output.stdout).unwrap();
    fs::write(run_dir.join("country-sweep.stderr"), &output.stderr).unwrap();
    fs::write(
        run_dir.join("country-sweep-wall-time-seconds.txt"),
        format!("{elapsed:.6}\n"),
    )
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["document"], "country-sweep");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let countries = document["result"]["countries"].as_array().unwrap();
    let (successful, no_owned, outcomes) = outcome_inventory(countries, &ids, &stderr);
    fs::write(run_dir.join("country-outcomes.txt"), outcomes).unwrap();
    check_results(countries, &ids, &successful, &no_owned);
    eprintln!("country validation artifacts: {}", run_dir.display());
}
