// What building a circular kernel costs. It is the only step that computes geodesic distance, so this is
// where the trigonometry in the search lives: one kernel per row of the grid, reused for every longitude
// at that row.
//
// Needs no raster and no table — a kernel is the grid and a radius, and nothing else.
//
// expect is what a benchmark documents an invariant with, the same licence `docs/ai/code.md` grants a
// test.
#![allow(clippy::expect_used)]

use std::hint::black_box;
use std::time::Instant;

use popcircles::geodesy::{LatLon, RadiusKm};
use popcircles::grid::Grid;
use popcircles::kernel::Kernel;
use serde_json::json;

#[path = "support/options.rs"]
mod options;

/// Rows sampled per shape rather than every row, and printed as such: a full-resolution grid has 21 600
/// rows and a 8 000 km kernel spans 17 000 of them, so the whole sweep would be a quarter of an hour for
/// a rate three digits of which never move. The sample is evenly spaced over the grid, so it covers the
/// pole, the equator and the latitudes between at the same weight the search meets them.
const SAMPLED_ROWS: u32 = 128;

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

fn radius(km: f64) -> RadiusKm {
    RadiusKm::new(km).expect("a benchmark radius is a length")
}

/// Builds `SAMPLED_ROWS` kernels evenly spaced over the grid, and reports the seconds and the rows those
/// kernels covered — the second figure being what the cost actually scales with.
fn sweep(grid: &Grid, radius: RadiusKm, sampled_rows: u32) -> (f64, u64) {
    let stride = (grid.height() / sampled_rows).max(1);
    let mut rows_covered = 0u64;
    let started = Instant::now();
    for index in (0..grid.height()).step_by(stride as usize) {
        let row = grid.row(index).expect("an index below the height");
        let kernel = Kernel::new(*grid, row, radius).expect("a whole-globe grid closes");
        rows_covered += kernel.rows().count() as u64;
        black_box(&kernel);
    }
    (started.elapsed().as_secs_f64(), rows_covered)
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
    println!(
        "kernel construction — {SAMPLED_ROWS} kernels per shape, evenly spaced over the grid\n"
    );
    println!(
        "{:>15}  {:>9}  {:>9}  {:>11}  {:>11}  {:>13}",
        "shape", "radius km", "seconds", "rows spanned", "µs / kernel", "rows/s"
    );

    let shapes = [grid(4320, 2160), grid(43200, 21600)];
    for shape in shapes {
        for radius_km in [200.0, 800.0, 3300.0, 8000.0] {
            let (seconds, rows_covered) = sweep(&shape, radius(radius_km), SAMPLED_ROWS);
            // Every widening here is of a count under 2^24, which f64 holds exactly.
            #[allow(clippy::cast_precision_loss)]
            let per_kernel = seconds * 1e6 / f64::from(SAMPLED_ROWS);
            #[allow(clippy::cast_precision_loss)]
            let rows_per_second = rows_covered as f64 / seconds;
            println!(
                "{:>6} x {:<6}  {radius_km:>9.0}  {seconds:>9.4}  {rows_covered:>11}  \
                 {per_kernel:>11.1}  {rows_per_second:>13.3e}",
                shape.width(),
                shape.height(),
            );
        }
    }
}

fn selected(options: &options::Options) -> Result<(), String> {
    options.only(&["width", "height", "radius-km", "rows", "samples"])?;
    let width: u32 = options.required("width")?;
    let height: u32 = options.required("height")?;
    let km: f64 = options.required("radius-km")?;
    let rows: u32 = options.optional("rows", SAMPLED_ROWS)?;
    let samples: u32 = options.optional("samples", 3)?;
    if width == 0 || height == 0 || rows == 0 || samples == 0 {
        return Err("width, height, rows and samples must be positive".to_owned());
    }
    let radius = RadiusKm::new(km).map_err(|error| error.to_string())?;
    let shape = grid(width, height);
    let mut raw = Vec::with_capacity(usize::try_from(samples).unwrap_or(usize::MAX));
    for _ in 0..samples {
        let (seconds, _) = sweep(&shape, radius, rows);
        raw.push(seconds);
    }
    println!(
        "{}",
        json!({
            "samples_seconds": raw,
            "parameters": {
                "width": width,
                "height": height,
                "radius_km": km,
                "sampled_rows": rows,
                "samples": samples,
            },
        })
    );
    Ok(())
}
