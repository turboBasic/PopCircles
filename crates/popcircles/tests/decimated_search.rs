// The search over the shape the decimated table has — 2160 x 4320, the k=10 grid — built
// synthetically rather than decimated from anything, because `platform.md` "Testing" forbids a test that
// needs raster bytes to pass.
//
// Deselected, and `mise run test:search` is what runs it. The payload is 74.7 MB and the search walks a
// whole globe, which is a cost CI should not carry; what it pins that the unit tests cannot is that the
// bound actually fires at a size where pruning is the difference between seconds and hours.
//
// expect is what a test documents an invariant with; docs/ai/code.md allows it here.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::convert::Infallible;

use popcircles::circle;
use popcircles::geodesy::{LatLon, RadiusKm};
use popcircles::grid::Grid;
use popcircles::kernel::Kernel;
use popcircles::raster::Synthetic;
use popcircles::search::{Candidate, most_populous};
use popcircles::table::{Decimation, Table, build};

/// The decimated grid: a tenth of the registry raster's resolution in each direction.
const WIDTH: u32 = 4320;
const HEIGHT: u32 = 2160;

/// The planted cluster, inclusive, and its value per cell.
const CLUSTER_ROWS: (u32, u32) = (600, 639);
const CLUSTER_COLS: (u32, u32) = (1200, 1239);
const CLUSTER_VALUE: f32 = 1.0;

/// 200 km, which is 21.6 rows of this grid and about 28 columns at the cluster's latitude.
///
/// Comparable to the cluster's 40 cells a side, and that is the point rather than a detail: a radius much
/// larger than the cluster would capture all of it from any centre within reach, so every such centre would
/// tie and the answer would be whichever of them the tie-break reaches first — a plateau, not a maximum.
const RADIUS_KM: f64 = 200.0;

/// The side of the blocks the first level is tiled into.
const SPACING: u32 = 32;

/// Rows and columns around the cluster that a maximiser cannot lie outside of.
///
/// 60 rows is 5 degrees, or 556 km, so a centre further north or south than that is over 200 km from every
/// planted cell and its circle holds nothing. Inside that band the latitudes run 45.0 N to 31.7 N, where a
/// column is 6.55 to 7.88 km, so 80 columns is at least 524 km — again past the radius. So the maximum over
/// this window is the maximum over the globe, and it is small enough to scan exhaustively in the test.
const ROW_MARGIN: u32 = 60;
const COL_MARGIN: u32 = 80;

fn grid() -> Grid {
    Grid::new(
        WIDTH,
        HEIGHT,
        LatLon {
            lat: 90.0,
            lon: -180.0,
        },
        1.0 / 12.0,
        -1.0 / 12.0,
    )
    .expect("the decimated whole-globe grid is valid")
}

fn radius(km: f64) -> RadiusKm {
    RadiusKm::new(km).expect("a fixture radius is a length")
}

fn payload(grid: &Grid) -> Vec<f64> {
    let rows: Vec<Vec<f32>> = (0..grid.height())
        .map(|row| {
            (0..grid.width())
                .map(|col| {
                    let inside = (CLUSTER_ROWS.0..=CLUSTER_ROWS.1).contains(&row)
                        && (CLUSTER_COLS.0..=CLUSTER_COLS.1).contains(&col);
                    if inside { CLUSTER_VALUE } else { 0.0 }
                })
                .collect()
        })
        .collect();

    let source = Synthetic::new(*grid, -3.402_823e38, rows).expect("the rows are the grid's shape");
    let mut cells = Vec::new();
    build(source, Decimation::none(*grid), &mut (), |row| {
        cells.extend_from_slice(row);
        Ok::<(), Infallible>(())
    })
    .expect("neither a synthetic source nor this sink can fail");
    cells
}

/// The best centre in the window around the cluster, by `circle::population` and the search's own rule.
fn best_near_the_cluster(table: &Table<'_>, grid: &Grid) -> Candidate {
    let rows = CLUSTER_ROWS.0 - ROW_MARGIN..=CLUSTER_ROWS.1 + ROW_MARGIN;
    let cols = CLUSTER_COLS.0 - COL_MARGIN..=CLUSTER_COLS.1 + COL_MARGIN;

    let mut best: Option<Candidate> = None;
    for row in rows {
        let row = grid.row(row).expect("a row of the window");
        let kernel = Kernel::new(*grid, row, radius(RADIUS_KM)).expect("a grid that closes");
        for col in cols.clone() {
            let col = grid.col(col).expect("a column of the window");
            let candidate = Candidate {
                row,
                col,
                population: circle::population(table, &kernel, col).expect("valid grid indices"),
            };
            best = Some(match best {
                Some(held) => held.better(candidate),
                None => candidate,
            });
        }
    }
    best.expect("the window has cells")
}

#[test]
#[ignore = "74.7 MB of payload and a whole-globe search; mise run test:search"]
fn the_search_finds_a_planted_maximum_and_the_bound_prunes_most_of_the_globe() {
    let grid = grid();
    let cells = payload(&grid);
    let table = Table::new(grid, &cells).expect("the build emits the padded product");

    let spacing = std::num::NonZeroU32::new(SPACING).expect("the spacing is not zero");
    let result = most_populous(&table, radius(RADIUS_KM), Some(spacing), &mut ())
        .expect("a whole-globe grid and an ordinary radius");

    let expected = best_near_the_cluster(&table, &grid);
    assert_eq!(
        (
            result.centre.row.get(),
            result.centre.col.get(),
            result.centre.population
        ),
        (expected.row.get(), expected.col.get(), expected.population)
    );
    // The circle is smaller than the cluster, so it holds part of it — which is what makes the maximum
    // local rather than a plateau.
    assert!(
        result.centre.population > 0.0
            && result.centre.population < f64::from(CLUSTER_VALUE) * 40.0 * 40.0,
        "{} is not a partial cluster",
        result.centre.population
    );

    // Measured on this fixture: 16 757 of 19 288 blocks pruned, 86.9%, over six levels, with 174 kernels
    // built for 19 288 blocks. The assertion is well under the ratio because its job is that the bound
    // fires at all — a threshold set at the measured figure would fail on any later tightening of the
    // bound, which is the wrong direction to be brittle in.
    //
    // What holds the figure down to 86.9% rather than higher is the strict prune over empty ground: until
    // the first positive incumbent is found, a block whose bound is zero ties it and cannot be dropped, so
    // the first level's northern bands all survive. They are all pruned at the next level, once the cluster
    // has been reached. Narrowing that is the tie-plateau optimisation this issue left out of scope.
    assert!(
        result.stats.blocks_pruned * 3 >= result.stats.blocks_examined * 2,
        "only {} of {} blocks pruned",
        result.stats.blocks_pruned,
        result.stats.blocks_examined
    );
}
