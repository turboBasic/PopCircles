// `smallest`'s predicate slack, measured against the arithmetic it bounds. The claim is that
// `circle::population`'s fold over a circle covering the whole grid stays within
// `predicate_slack_persons` of `Table::whole`'s single query — the two answers to the same population,
// one summed per row and one taken in four corners — at a magnitude where the sums actually round.
//
// The shape is the decimated grid, 4320 x 2160, with cells scaled so the total is a world's worth
// of people. Not the full-resolution shape: a `Table` there is 7.5 GB of payload, and the bound scales
// with the row count rather than jumping at some threshold, so measuring the formula at a tenth of the
// rows measures the formula. `full_resolution_table.rs` covers the same arithmetic one layer down and
// never holds a table for exactly this reason.
//
// Deselected, and `mise run test:fold` is what runs it: the payload is 74.7 MB and the build streams
// 9 331 200 cells, which is a cost the default suite should not carry per run.
//
// expect and unwrap are what a test documents an invariant with; docs/ai/code.md allows both here. The
// cast lints are allowed because each cast below is exact and says why at its site.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

use std::convert::Infallible;

use popcircles::circle;
use popcircles::geodesy::LatLon;
use popcircles::grid::Grid;
use popcircles::kernel::Kernel;
use popcircles::raster::Synthetic;
use popcircles::smallest::{ceiling_radius, predicate_slack_persons};
use popcircles::table::{Decimation, Table, build};

const WIDTH: u32 = 4320;
const HEIGHT: u32 = 2160;
const NODATA: f32 = -3.402_823e38;

/// The populated share, in thousandths, and the registry raster's is about a fifth.
const POPULATED_PER_MILLE: u64 = 195;

/// f32 stores this many significand bits, and a generated cell fills all of them, so no cell is a round
/// number the arithmetic could get right by luck.
const SIGNIFICAND_BITS: u32 = 23;
const SIGNIFICAND_MASK: u32 = (1 << SIGNIFICAND_BITS) - 1;

/// Exponent field 139 is the binade at 4096, and 9 331 200 cells a fifth populated at that magnitude come
/// to about 8e9 — a world's population, which is the magnitude the slack is a claim about.
const EXPONENT: u32 = 139;

/// splitmix64, so a cell depends on nothing but its position.
fn mix(value: u64) -> u64 {
    let mut z = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn cell(row: u32, col: u32) -> f32 {
    let hash = mix(u64::from(row) * u64::from(WIDTH) + u64::from(col));
    if hash % 1_000 >= POPULATED_PER_MILLE {
        return 0.0;
    }
    // The mask keeps 23 bits, so the narrowing is exact.
    let significand = ((hash >> 32) as u32) & SIGNIFICAND_MASK;
    f32::from_bits((EXPONENT << SIGNIFICAND_BITS) | significand)
}

#[test]
#[ignore = "builds a 9 331 200 cell table; run `mise run test:fold`"]
fn the_folds_error_at_magnitude_stays_inside_the_predicate_slack() {
    let grid = Grid::new(
        WIDTH,
        HEIGHT,
        LatLon {
            lat: 90.0,
            lon: -180.0,
        },
        1.0 / 12.0,
        -1.0 / 12.0,
    )
    .expect("the decimated shape is a valid grid");

    let rows: Vec<Vec<f32>> = (0..HEIGHT)
        .map(|row| (0..WIDTH).map(|col| cell(row, col)).collect())
        .collect();
    let source = Synthetic::new(grid, NODATA, rows).expect("the rows are the grid's shape");

    let mut payload: Vec<f64> = Vec::with_capacity((WIDTH as usize + 1) * (HEIGHT as usize + 1));
    build(source, Decimation::none(grid), &mut (), |row| {
        payload.extend_from_slice(row);
        Ok::<(), Infallible>(())
    })
    .expect("a synthetic source cannot fail and neither can this sink");

    let table = Table::new(grid, &payload).expect("the build emits the padded product");
    let (whole_rows, whole_cols) = table.whole();
    let total = table.population(whole_rows, whole_cols);
    let slack = predicate_slack_persons(&grid, total);

    // A circle at the ceiling covers every cell from any centre, so every one of these folds is a
    // summation of the same population the corner query answers in one subtraction. Centres at both poles
    // and the equator, and columns at both ends of the seam and in the middle, because the fold's error is
    // a function of the order the rows arrive in and that order is the centre's.
    let mut worst = 0.0f64;
    for row_index in [0, HEIGHT / 2, HEIGHT - 1] {
        let centre = grid.row(row_index).expect("a row of the grid");
        let kernel = Kernel::new(grid, centre, ceiling_radius()).expect("a grid that closes");
        for col_index in [0, WIDTH / 2, WIDTH - 1] {
            let col = grid.col(col_index).expect("a column of the grid");
            let folded = circle::population(&table, &kernel, col).expect("valid grid indices");
            worst = worst.max((folded - total).abs());
        }
    }

    println!(
        "total {total}, worst fold error {worst} persons, derived slack {slack} persons over {HEIGHT} \
         rows"
    );
    assert!(
        worst <= slack,
        "the fold is out by {worst} persons at a total of {total}, past the {slack} the slack claims"
    );

    // Measured 0.0 against a slack of 0.0218 persons at a total of 1.1e10 on 2026-08-14: at this shape the
    // per-row fold reproduces the corner query exactly, because each row's rectangle is a difference of
    // corners that telescopes. So the slack is a bound and not an observed error, and no tightening of it
    // can make the assertion above fail here — which is why the second assertion is about the derivation
    // rather than about the measurement. A bound inflated into a tolerance wide enough to hide a real
    // drift fails it; the ratio at this shape is 2e-12, and at the full-resolution row count 2e-11.
    assert!(
        slack < total * 1e-8,
        "a slack of {slack} at a total of {total} is wide enough to hide a drift rather than to bound one"
    );
}
