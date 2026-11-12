// What a summation table build costs, and how much of that is the payload write rather than the
// compensated arithmetic — the question left open when the first figures came from a scratch crate
// outside this tree.
//
// No raster: the source is generated a row at a time, so this measures the build and not a decoder, and
// it runs on a checkout with no raster in it. The generated mix is the registry raster's own, because a
// build over all-nodata rows and a build over dense counts are not the same measurement.
//
// expect is what a benchmark documents an invariant with, the same licence `docs/ai/code.md` grants a
// test: a shape this file declares is either valid or a mistake here.
#![allow(clippy::expect_used)]

use std::convert::Infallible;
use std::hint::black_box;
use std::time::Instant;

use popcircles::geodesy::LatLon;
use popcircles::grid::Grid;
use popcircles::raster::{CellTallies, RasterError, RasterRow, RasterSource, sanitise_row};
use popcircles::table::cache::Cache;
use popcircles::table::{Decimation, build};
use serde_json::json;

#[path = "support/options.rs"]
mod options;

/// The registry raster's sentinel, so the generated rows travel the path a real strip travels.
const NODATA: f32 = -3.402_823e38;

/// The registry raster's measured mix, in parts per thousand: 710 450 072 nodata and 40 311 312 zero
/// cells of 933 120 000 (`data/README.md`). A build over rows that are all counts is a different
/// measurement, and a faster one — `sanitise_row` writes a zero over three quarters of a real raster.
const NODATA_PER_MILLE: u64 = 761;
const ZERO_PER_MILLE: u64 = 43;

/// A raster generated one row at a time, so a full-resolution pass needs no 3.7 GB of fixture.
struct Generated {
    grid: Grid,
    next: u32,
    values: Vec<f32>,
    tallies: CellTallies,
}

impl Generated {
    fn over(grid: Grid) -> Self {
        Self {
            grid,
            next: 0,
            values: vec![0.0; grid.width() as usize],
            tallies: CellTallies::default(),
        }
    }
}

impl RasterSource for Generated {
    fn grid(&self) -> Grid {
        self.grid
    }

    fn next_row(&mut self) -> Option<Result<RasterRow<'_>, RasterError>> {
        let row = self.grid.row(self.next)?;
        self.next += 1;
        for (value, col) in self.values.iter_mut().zip(0u32..) {
            *value = cell(row.get(), col);
        }
        sanitise_row(&mut self.values, NODATA, &mut self.tallies);
        Some(Ok(RasterRow {
            row,
            values: &self.values,
        }))
    }

    fn finish(self) -> CellTallies {
        self.tallies
    }
}

/// One cell, from its index alone: a deterministic bit mix rather than a generator with state, so a row
/// costs no allocation and two runs measure the same input.
fn cell(row: u32, col: u32) -> f32 {
    let mut hash = (u64::from(row) << 32) | u64::from(col);
    hash = hash.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    hash ^= hash >> 29;
    hash = hash.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    hash ^= hash >> 32;

    let bucket = hash % 1000;
    if bucket < NODATA_PER_MILLE {
        return NODATA;
    }
    if bucket < NODATA_PER_MILLE + ZERO_PER_MILLE {
        return 0.0;
    }
    // 0 to 8191 through u16, which f32 holds exactly, so no cast lint is silenced to get a count. The
    // divisor puts the mean near 42.7 persons, which is the registry's 7.76e9 over its 182 million
    // populated cells — the magnitude the compensated accumulator is asked to hold.
    let scaled = u16::try_from((hash >> 40) & 0x1fff).unwrap_or(u16::MAX);
    f32::from(scaled) / 96.0
}

/// A path under the workspace root, whatever directory cargo chose to run this from: cargo runs a
/// benchmark with the *package* directory as the working directory, so a bare `out/` would put 7.5 GB
/// under `crates/popcircles/`, which is not the gitignored one.
fn workspace(relative: &str) -> std::path::PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", "..", relative]
        .iter()
        .collect()
}

fn grid(width: u32, height: u32) -> Grid {
    let step = 360.0 / f64::from(width);
    Grid::new(
        width,
        height,
        LatLon {
            lat: 90.0,
            lon: -180.0,
        },
        step,
        -step,
    )
    .expect("a whole-globe grid this file declares is valid")
}

/// Streams the shape once, discarding every row: the compensated arithmetic with no sink behind it.
fn discarding(source: Grid, factor: u32) -> (f64, f64) {
    let decimation = Decimation::new(source, factor).expect("the factor divides the shape");
    let started = Instant::now();
    let built = build(
        Generated::over(source),
        decimation,
        &mut (),
        |row: &[f64]| {
            black_box(row);
            Ok::<(), Infallible>(())
        },
    )
    .expect("neither a generated source nor this sink can fail");
    let seconds = started.elapsed().as_secs_f64();
    (seconds, built.total)
}

/// The same stream through the cache writer, so the difference is the payload write and nothing else.
///
/// Removes both files before returning: 7.5 GB left under `out/` is gitignored but not free.
fn through_the_cache(source: Grid, factor: u32, base: &std::path::Path) -> (f64, u64) {
    let decimation = Decimation::new(source, factor).expect("the factor divides the shape");
    let cache = Cache::new(base);
    let started = Instant::now();
    let mut writer = cache
        .writer()
        .expect("a temporary under a directory that exists");
    let built = build(Generated::over(source), decimation, &mut (), |row| {
        writer.write_row(row)
    })
    .expect("a generated source, and a sink that only fails on I/O");
    writer
        .publish(&built, None)
        .expect("the payload is published");
    let seconds = started.elapsed().as_secs_f64();
    let bytes = std::fs::metadata(cache.payload_path())
        .expect("the payload was just published")
        .len();
    for path in [cache.header_path(), cache.payload_path()] {
        let _ = std::fs::remove_file(path);
    }
    (seconds, bytes)
}

fn main() {
    let options = match options::Options::from_env() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    if !options.is_empty() {
        if let Err(error) = selected(&options) {
            eprintln!("{error}");
            std::process::exit(2);
        }
        return;
    }
    println!("table build — a generated raster at the registry's mix of nodata, zero and counts\n");
    println!(
        "{:>13}  {:>4}  {:>13}  {:>13}  {:>9}  {:>12}",
        "source", "fold", "cells in", "cells out", "seconds", "cells/s"
    );

    let shapes = [
        (grid(4320, 2160), 1u32),
        (grid(43200, 21600), 10),
        (grid(43200, 21600), 1),
    ];
    for (source, factor) in shapes {
        let cells_in = u64::from(source.width()) * u64::from(source.height());
        let (seconds, total) = discarding(source, factor);
        let out = Decimation::new(source, factor).expect("the factor divides the shape");
        let cells_out = u64::from(out.grid().width()) * u64::from(out.grid().height());
        // The rate is what the figure is for, and a cell count near 1e9 is exact in f64, so the two
        // widenings below lose nothing.
        #[allow(clippy::cast_precision_loss)]
        let rate = cells_in as f64 / seconds;
        println!(
            "{:>6} x {:<6}  {factor:>4}  {cells_in:>13}  {cells_out:>13}  {seconds:>9.3}  {rate:>12.3e}",
            source.width(),
            source.height(),
        );
        black_box(total);
    }

    // The write is measured only at full resolution, because 7.5 GB is the figure expected to
    // dominate and a 74.7 MB one says nothing about it. The temporary goes under `out/`, which is
    // gitignored, and both files are removed again.
    let base = workspace("out/bench-table-build");
    std::fs::create_dir_all(workspace("out")).expect("out/ is this repository's scratch directory");
    let (seconds, bytes) = through_the_cache(grid(43200, 21600), 1, &base);
    println!(
        "\nthe same 933 120 000 cells through the cache writer: {seconds:.3} s for {bytes} payload bytes"
    );
}

fn selected(options: &options::Options) -> Result<(), String> {
    options.only(&["width", "height", "factor", "mode", "samples"])?;
    let width: u32 = options.required("width")?;
    let height: u32 = options.required("height")?;
    let factor: u32 = options.required("factor")?;
    let mode = options.text("mode").unwrap_or("discard");
    let samples: u32 = options.optional("samples", 3)?;
    if width == 0 || height == 0 || factor == 0 || samples == 0 {
        return Err("width, height, factor and samples must be positive".to_owned());
    }
    if !matches!(mode, "discard" | "write") {
        return Err("mode must be `discard` or `write`".to_owned());
    }
    let source = grid(width, height);
    Decimation::new(source, factor).map_err(|error| error.to_string())?;
    let mut raw = Vec::with_capacity(usize::try_from(samples).unwrap_or(usize::MAX));
    let mut payload_bytes = None;
    for _ in 0..samples {
        let seconds = if mode == "discard" {
            discarding(source, factor).0
        } else {
            std::fs::create_dir_all(workspace("out")).map_err(|error| {
                format!("could not create the repository scratch directory: {error}")
            })?;
            let (seconds, bytes) =
                through_the_cache(source, factor, &workspace("out/bench-table-build"));
            payload_bytes = Some(bytes);
            seconds
        };
        raw.push(seconds);
    }
    println!(
        "{}",
        json!({
            "samples_seconds": raw,
            "input_identity": "synthetic:generated-raster-v1",
            "parameters": {
                "width": width,
                "height": height,
                "factor": factor,
                "mode": mode,
                "samples": samples,
            },
            "payload_bytes": payload_bytes,
        })
    );
    Ok(())
}
