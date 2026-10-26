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
    let builtin_combinations = builtin["result"]["combinations"].as_array().unwrap();
    assert_eq!(builtin_combinations.len(), 2);
    let gisco = builtin_combinations
        .iter()
        .find(|combination| combination["id"] == "gpw-2020-gisco-2024")
        .unwrap();
    assert_eq!(gisco["is_default"], true);
    assert_eq!(gisco["default_resolution"], 2);
    assert_eq!(gisco["available_resolutions"], serde_json::json!([1, 2]));
    let gadm = builtin_combinations
        .iter()
        .find(|combination| combination["id"] == "gpw-2020-gadm-4.1")
        .unwrap();
    assert_eq!(gadm["is_default"], false);
    assert_eq!(gadm["default_resolution"], 2);
    assert!(
        gadm["boundary_source"]
            .as_str()
            .unwrap()
            .ends_with("boundaries/gadm41-adm0.geojson")
    );
    assert_eq!(gadm["available_resolutions"], serde_json::json!([1, 2]));

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

        [combinations.local-preview.population_metadata]
        width = 2
        height = 2
        origin_lat = 1.0
        origin_lon = 0.0
        lon_step = 1.0
        lat_step = -1.0
        epsg = 4326
        nodata = -9999.0

        [combinations.local-preview.boundary_metadata]
        source_id_field = "CNTR_ID"

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
    assert_eq!(combinations.len(), 3);
    let gisco = combinations
        .iter()
        .find(|combination| combination["id"] == "gpw-2020-gisco-2024")
        .unwrap();
    assert_eq!(gisco["is_default"], false);
    let preview = combinations
        .iter()
        .find(|combination| combination["id"] == "local-preview")
        .unwrap();
    let expected_source = format!(
        "file:{}",
        directory.path().join("missing-population.tif").display()
    );
    assert_eq!(preview["population_source"], expected_source);
    assert_eq!(preview["default_resolution"], 5);
    assert_eq!(preview["is_default"], true);
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
