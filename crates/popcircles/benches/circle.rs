// What evaluating one circle costs — the innermost thing the search does, once per candidate centre, as
// one rectangle query per row the kernel spans.
//
// **Two figures, not one, and the second is the one a full-resolution run actually pays.** A table that
// fits in memory answers a four-corner query in nanoseconds; the full-resolution table is 7.5 GB, read by
// mmap on a machine that cannot hold it, and there the same query is a page fault. A benchmark reporting
// only the resident figure describes a run that never happens, so this reports the resident one from a
// table it builds and the mapped one from the cache under `out/` when a full-resolution table is there.
//
// The mapped half is skipped with a message rather than failed when that cache is absent: it needs
// `mise run data:get` and a build, and neither is something a benchmark should do behind a caller's back.
//
// expect is what a benchmark documents an invariant with, the same licence `docs/ai/code.md` grants a
// test.
#![allow(clippy::expect_used)]

use std::hint::black_box;
use std::time::Instant;

use popcircles::circle;
use popcircles::geodesy::{LatLon, RadiusKm};
use popcircles::grid::Grid;
use popcircles::kernel::Kernel;
use popcircles::table::cache::{Cache, Identity};
use popcircles::table::{Decimation, Table};

/// The 5 arcmin shape, whose table is 74.7 MB and therefore resident.
const RESIDENT_SHAPE: (u32, u32) = (4320, 2160);

/// Where a full-resolution table is looked for, and the two facts needed to open one: `README.md`'s own
/// worked example writes this cache, and the digest is the registry raster's, whose provenance
/// `data/README.md` records. Literals rather than anything parsed, the shape `tests/registry_raster.rs`
/// uses: #8 owns how a user supplies a spec, and a benchmark is not a second command surface.
///
/// Resolved through [`workspace`] rather than used as it stands: cargo runs a benchmark with the *package*
/// directory as the working directory, so the bare path would name `crates/popcircles/out/` and the skip
/// below would fire on a machine that has the table.
const MAPPED_CACHE: &str = "out/gpw-30arcsec";
const REGISTRY_DIGEST: u64 = 0xf17a_a802_a689_0f0c;
const REGISTRY_SHAPE: (u32, u32) = (43200, 21600);

/// Centres sampled per axis, and the two are two orders of magnitude apart for a reason the figures
/// themselves give: a mapped query costs a thousand times a resident one, so the same count that takes a
/// twentieth of a second resident would take a quarter of an hour mapped. Both are printed with the count
/// they used, and neither is a cap on coverage — a per-query figure is what is wanted, and it converges
/// long before the globe is exhausted.
const RESIDENT_CENTRES: u32 = 64;
const MAPPED_CENTRES: u32 = 8;

/// The radii both halves are measured at: one whose kernel spans a few hundred rows, one whose kernel
/// spans a third of the globe, because the per-circle cost scales with the second and the per-query cost
/// should not.
const RADII_KM: [f64; 2] = [200.0, 3300.0];

/// A path under the workspace root, whatever directory cargo chose to run this from.
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

fn radius(km: f64) -> RadiusKm {
    RadiusKm::new(km).expect("a benchmark radius is a length")
}

/// One circle per sampled centre, evenly spaced in both axes over the whole globe.
///
/// Evenly spaced rather than clustered so the mapped figure meets the page pattern a search meets: a
/// kernel walks rows in order, and consecutive rows of the table are a row-stride apart — 345 KB at full
/// resolution.
///
/// Returns the seconds, the four-corner queries those circles cost between them, and the circles.
fn evaluate(table: &Table<'_>, radius: RadiusKm, centres: u32) -> (f64, u64, u64) {
    let grid = *table.grid();
    let row_stride = usize::try_from((grid.height() / centres).max(1)).unwrap_or(1);
    let col_stride = usize::try_from((grid.width() / centres).max(1)).unwrap_or(1);

    let mut queries = 0u64;
    let mut circles = 0u64;
    let started = Instant::now();
    for row_index in (0..grid.height()).step_by(row_stride) {
        let row = grid.row(row_index).expect("an index below the height");
        let kernel = Kernel::new(grid, row, radius).expect("a whole-globe grid closes");
        let spanned = u64::try_from(kernel.rows().count()).unwrap_or(u64::MAX);
        for col_index in (0..grid.width()).step_by(col_stride) {
            let col = grid.col(col_index).expect("an index below the width");
            let population = circle::population(table, &kernel, col);
            let _ = black_box(population);
            queries += spanned;
            circles += 1;
        }
    }
    (started.elapsed().as_secs_f64(), queries, circles)
}

fn report(label: &str, seconds: f64, queries: u64, circles: u64) {
    // Both counts are well under 2^53, so the widenings are exact.
    #[allow(clippy::cast_precision_loss)]
    let per_query_ns = seconds * 1e9 / queries as f64;
    #[allow(clippy::cast_precision_loss)]
    let per_circle_ms = seconds * 1e3 / circles as f64;
    println!(
        "{label:>24}  {seconds:>9.3}  {circles:>8}  {queries:>12}  {per_query_ns:>12.1}  \
         {per_circle_ms:>13.3}"
    );
}

/// A table of the resident shape, whose cells are a separable ramp: the figure being measured is the
/// four-corner arithmetic and the memory it touches, neither of which depends on the values. A ramp is
/// what the prefix sum of a constant raster is, so every rectangle query returns a non-negative
/// population as a real table's would.
fn resident_cells(grid: &Grid) -> Vec<f64> {
    let width = grid.width() as usize + 1;
    let height = grid.height() as usize + 1;
    let mut cells = vec![0.0f64; width * height];
    for row in 1..height {
        for col in 1..width {
            // Both indices are under 2^24, which f64 holds exactly.
            #[allow(clippy::cast_precision_loss)]
            let value = (row * col) as f64;
            cells[row * width + col] = value;
        }
    }
    cells
}

fn header() {
    println!(
        "{:>24}  {:>9}  {:>8}  {:>12}  {:>12}  {:>13}",
        "table", "seconds", "circles", "queries", "ns / query", "ms / circle"
    );
}

fn main() {
    println!("circle evaluation — one rectangle query per row a kernel spans\n");
    header();

    let shape = grid(RESIDENT_SHAPE.0, RESIDENT_SHAPE.1);
    let cells = resident_cells(&shape);
    let table = Table::new(shape, &cells).expect("the padded shape is the grid's");
    for radius_km in RADII_KM {
        let (seconds, queries, circles) = evaluate(&table, radius(radius_km), RESIDENT_CENTRES);
        report(
            &format!("resident, {radius_km:.0} km"),
            seconds,
            queries,
            circles,
        );
    }

    mapped();
}

/// The full-resolution table through mmap, when one is there.
fn mapped() {
    let shape = grid(REGISTRY_SHAPE.0, REGISTRY_SHAPE.1);
    let identity = Identity {
        digest: REGISTRY_DIGEST,
        decimation: Decimation::none(shape),
    };
    let mapped = match Cache::new(workspace(MAPPED_CACHE)).open(&identity) {
        Ok(mapped) => mapped,
        Err(error) => {
            println!(
                "\nskipped the mapped figure — {error}\n\
                 it needs a full-resolution table, which `mise run data:get` and then \
                 `table build --decimate 1 --cache {MAPPED_CACHE}` produce; \
                 README.md's Usage carries the whole command."
            );
            return;
        }
    };
    let cells = mapped.cells().expect("the header describes this payload");
    let table = Table::new(shape, cells).expect("the padded shape is the grid's");

    for radius_km in RADII_KM {
        let (seconds, queries, circles) = evaluate(&table, radius(radius_km), MAPPED_CENTRES);
        report(
            &format!("mapped, {radius_km:.0} km"),
            seconds,
            queries,
            circles,
        );
    }
}
