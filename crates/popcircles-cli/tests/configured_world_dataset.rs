#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

fn run(arguments: &[&str], cache: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_popcircles"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .args(["table", "build", "--cache"])
        .arg(cache)
        .args(arguments)
        .output()
        .expect("the CLI starts")
}

#[test]
fn configured_world_dataset_rejects_an_invalid_override_before_io() {
    let directory = TempDir::new().unwrap();
    let output = run(
        &["--dataset", "missing-dataset"],
        &directory.path().join("override"),
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not a registered population raster"),
        "{stderr}"
    );
    assert!(!directory.path().join("override.header").exists());
}
