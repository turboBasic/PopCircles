#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn run(config: Option<&Path>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_popcircles"));
    command
        .current_dir(repository_root())
        .args(["country", "combinations"]);
    if let Some(path) = config {
        command.arg("--config").arg(path);
    }
    command.output().expect("the binary runs")
}

fn write_config(directory: &TempDir, filename: &str, contents: &str) -> PathBuf {
    let path = directory.path().join(filename);
    std::fs::write(&path, contents).expect("fixture config is written");
    path
}

#[test]
fn combination_listing_uses_builtin_and_user_configuration_without_opening_sources() {
    let builtin = run(None);
    assert!(
        builtin.status.success(),
        "{}",
        String::from_utf8_lossy(&builtin.stderr)
    );
    let builtin: Value = serde_json::from_slice(&builtin.stdout).unwrap();
    assert_eq!(builtin["document"], "country-combinations");
    assert_eq!(
        builtin["result"]["combinations"][0]["id"],
        "gpw-2020-gisco-2024"
    );
    assert_eq!(builtin["result"]["combinations"][0]["is_default"], true);

    let directory = TempDir::new().unwrap();
    let config = write_config(
        &directory,
        "user.toml",
        r#"
        [defaults]
        country_combination = "local-preview"

        [combinations.local-preview]
        population_source = "file:missing-population.tif"
        boundary_source = "file:missing-boundaries.geojson"
        default_resolution = 5

        [combinations.local-preview.resolutions.5]
        local = "not-prepared"
    "#,
    );
    let output = run(Some(&config));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output: Value = serde_json::from_slice(&output.stdout).unwrap();
    let combinations = output["result"]["combinations"].as_array().unwrap();
    assert_eq!(combinations.len(), 2);
    assert_eq!(combinations[0]["id"], "gpw-2020-gisco-2024");
    assert_eq!(combinations[0]["is_default"], false);
    assert_eq!(combinations[1]["id"], "local-preview");
    let expected_source = format!(
        "file:{}",
        directory.path().join("missing-population.tif").display()
    );
    assert_eq!(combinations[1]["population_source"], expected_source);
    assert_eq!(combinations[1]["default_resolution"], 5);
    assert_eq!(combinations[1]["is_default"], true);
}

#[test]
fn invalid_user_config_reports_its_path_and_combination() {
    let directory = TempDir::new().unwrap();
    let malformed = write_config(&directory, "broken.toml", "[defaults\n");
    let output = run(Some(&malformed));
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(malformed.to_str().unwrap()), "{stderr}");

    let conflict = write_config(
        &directory,
        "conflict.toml",
        r#"
        [combinations.gpw-2020-gisco-2024]
        population_source = "dataset:population-count-2020-30arcsec"
    "#,
    );
    let output = run(Some(&conflict));
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(conflict.to_str().unwrap()), "{stderr}");
    assert!(stderr.contains("gpw-2020-gisco-2024"), "{stderr}");
    assert!(stderr.contains("boundary_source"), "{stderr}");
}
