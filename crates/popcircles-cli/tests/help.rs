// Every help surface the binary has, pinned byte for byte. `--help` is the whole of what a user reads
// before running anything, and it is assembled by clap from attributes spread across every flag group
// and command variant — so it is the one artefact a module split can change without a single behavioural
// test noticing. These snapshots are the net: changes to a help surface are reviewed in its emitted diff.
//
// clap is built with `default-features = false` and no `wrap_help`, so this output does not depend on
// terminal width and the snapshots hold wherever they run. `--help` also carries no version string, so a
// version bump does not reach them.
//
// Snapshots live beside this file rather than in the library, which is what `commands.rs`'s header
// already notes about `FU-03`'s tripwire: it watches `crates/popcircles/src/snapshots/` and does not see
// a snapshot kept here. That is correct — a help string is not the wire format and does not bind
// `SCHEMA_VERSION`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;

/// The eleven surfaces, each named by the snapshot it writes. `help` itself is clap's own and takes no
/// arguments of ours, so it is not among them.
const SURFACES: [(&str, &[&str]); 16] = [
    ("binary", &[]),
    ("distance", &["distance"]),
    ("grid", &["grid"]),
    ("grid-describe", &["grid", "describe"]),
    ("country", &["country"]),
    ("country-list", &["country", "list"]),
    ("country-combinations", &["country", "combinations"]),
    ("country-prepare", &["country", "prepare"]),
    ("country-smallest", &["country", "smallest"]),
    ("table", &["table"]),
    ("table-build", &["table", "build"]),
    ("table-query", &["table", "query"]),
    ("population-at", &["population-at"]),
    ("most-populous", &["most-populous"]),
    ("smallest-for-share", &["smallest-for-share"]),
    ("sweep", &["sweep"]),
];

fn help_for(path: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_popcircles"))
        .args(path)
        .arg("--help")
        .output()
        .expect("the binary cargo just built runs");
    assert!(
        output.status.success(),
        "`{} --help` exited {:?}: {}",
        path.join(" "),
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "help goes to stdout: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("clap writes UTF-8")
}

#[test]
fn every_help_surface_holds_its_shape() {
    for (name, path) in SURFACES {
        insta::assert_snapshot!(name, help_for(path));
    }
}
