// The ground step 3 of application.md "Approach" stands on: the population inside a circle of a given
// ground radius, anywhere on the globe. The geometry is the kernel's and the arithmetic is the table's,
// so what is here is the fold between them and nothing else — no distance, no latitude, no angle.

use crate::grid::Col;
use crate::kernel::Kernel;
use crate::table::{RowBand, Table};

/// The population inside the circle `kernel` describes, centred on `centre` in the kernel's own row.
///
/// One rectangle per row the cap reaches, added in the order [`Kernel::place`] yields them, north to
/// south. The order is fixed rather than incidental: a rerun has to give the same answer down to the last
/// bit, so nothing here sorts the rows, splits them across threads, or merges two of them into one query.
///
/// A grid whose columns do not close needs no case: [`Kernel::new`] refuses one, so holding a kernel is
/// the proof, and the assertion below extends it to the table.
///
/// # Panics
/// If the kernel was built over a grid other than the table's. Both are a [`crate::grid::Grid`], so the
/// types cannot catch it, and it is a wiring mistake rather than an input — the same reason
/// [`crate::table::build`] stops on a decimation minted against a foreign grid. Also if `centre` was
/// minted by a larger grid; [`crate::grid::Col`] says why that is a stop.
#[must_use]
pub fn population(table: &Table<'_>, kernel: &Kernel, centre: Col) -> f64 {
    assert_eq!(
        *table.grid(),
        *kernel.grid(),
        "the kernel was built over a different grid than this table"
    );

    kernel
        .place(centre)
        .map(|(row, cols)| table.population(RowBand::new(row, row), cols))
        .sum()
}

// expect is warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows it in tests. float_cmp is the point rather than a concession: every
// fixture cell is a small integer, so each side of an assertion below is an exact f64 and a tolerance
// would let a dropped row or a doubled one pass. cast_precision_loss likewise — the largest cell is 648,
// which u32 -> f32 holds exactly.
#[cfg(test)]
#[allow(clippy::expect_used, clippy::float_cmp, clippy::cast_precision_loss)]
mod tests {
    use std::convert::Infallible;

    use proptest::prelude::*;

    use super::*;
    use crate::geodesy::{LatLon, RadiusKm, great_circle_km};
    use crate::grid::{Grid, Row};
    use crate::kernel::Span;
    use crate::raster::Synthetic;
    use crate::table::{ColSpan, Decimation, build};

    const WIDTH: u32 = 36;
    const HEIGHT: u32 = 18;

    /// The registry raster's sentinel, so the fixture reaches the table by the path a real raster takes.
    const NODATA: f32 = -3.402_823e38;

    /// Ten degrees a side, closing in longitude: the smallest shape that has a kernel, a seam and a pole
    /// at once, and small enough that a whole-grid reference scan at every centre is a second of work.
    fn grid() -> Grid {
        Grid::new(
            WIDTH,
            HEIGHT,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            10.0,
            -10.0,
        )
        .expect("a 36 x 18 whole-globe grid is valid")
    }

    fn radius(km: f64) -> RadiusKm {
        RadiusKm::new(km).expect("a fixture radius is a length")
    }

    /// Distinct at every position, so a cell counted twice or not at all moves the total, and no larger
    /// than 648, so every partial sum of them is exact in f64 and the assertions are equalities.
    fn cell(row: u32, col: u32) -> f32 {
        (row * WIDTH + col + 1) as f32
    }

    fn cells() -> Vec<Vec<f32>> {
        (0..HEIGHT)
            .map(|row| (0..WIDTH).map(|col| cell(row, col)).collect())
            .collect()
    }

    /// The padded payload a real build emits over these rows, rather than one written out by hand: the
    /// fixture is then the path the search uses and not a second construction of it.
    fn payload_over(rows: Vec<Vec<f32>>) -> Vec<f64> {
        let grid = grid();
        let source = Synthetic::new(grid, NODATA, rows).expect("the rows are the grid's shape");
        let mut payload = Vec::new();
        build(source, Decimation::none(grid), &mut (), |row| {
            payload.extend_from_slice(row);
            Ok::<(), Infallible>(())
        })
        .expect("neither a synthetic source nor this sink can fail");
        payload
    }

    /// Every cell of the whole grid within `radius_km` of the centre cell, added up directly.
    ///
    /// The scan covers the grid and not the kernel's band, which is the only way it can see a cell the
    /// kernel left out; and it tests kilometres against kilometres, so it owes nothing to the angle the
    /// kernel converted the radius to.
    fn by_distance(grid: &Grid, centre: (Row, Col), radius_km: f64) -> f64 {
        let from = grid.centre_of(centre.0, centre.1);
        grid.rows()
            .flat_map(|row| grid.cols().map(move |col| (row, col)))
            .filter(|(row, col)| great_circle_km(from, grid.centre_of(*row, *col)) <= radius_km)
            .map(|(row, col)| f64::from(cell(row.get(), col.get())))
            .sum()
    }

    #[test]
    fn a_circles_population_is_the_sum_of_the_cells_a_distance_test_admits() {
        // Every cell of the fixture as a centre, at four radii: 1500 km spans a column or two, 20 016 km
        // is past half the circumference and closes every row, and the two between cover the pole from
        // one side and not the other. Exhaustive in longitude is what says the seam is not a special
        // case, and exhaustive in latitude is what says the pole is not either.
        let grid = grid();
        let payload = payload_over(cells());
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        for radius_km in [1500.0, 4000.0, 8000.0, 20_016.0] {
            for centre_row in grid.rows() {
                // One kernel per row, placed at all 36 columns, which is the reuse the type exists for.
                let kernel =
                    Kernel::new(grid, centre_row, radius(radius_km)).expect("a whole-globe grid");
                for centre_col in grid.cols() {
                    assert_eq!(
                        population(&table, &kernel, centre_col),
                        by_distance(&grid, (centre_row, centre_col), radius_km),
                        "row {}, column {}, radius {radius_km} km",
                        centre_row.get(),
                        centre_col.get()
                    );
                }
            }
        }
    }

    /// The fixture's rows with the value at column c moved to `col_along(c, k)` — the same direction and
    /// the same k a centre column is moved by below, which is the whole content of the claim.
    fn shifted_cells(k: i64) -> Vec<Vec<f32>> {
        let grid = grid();
        (0..HEIGHT)
            .map(|row| {
                (0..WIDTH)
                    .map(|col| {
                        let source =
                            grid.col_along(grid.col(col).expect("a column of the fixture"), -k);
                        cell(row, source.get())
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn shifting_the_raster_and_the_centre_together_leaves_the_population_alone() {
        // Exactly, and only because of the fixture: cells no larger than 648 make every partial sum in the
        // table exact in f64, so the shifted table's corners are the unshifted ones rearranged rather than
        // rounded. At full-resolution magnitudes the same shift moves the answer by up to 2 ulp, about
        // 1.9e-6 persons, because rotating a row rotates the sequence its prefix accumulates and the
        // four-corner difference inherits the last bits of that. It is inside the table's 4 ulp per
        // rectangle query, and it is the table's arithmetic rather than a fault in the wrapping — not
        // something to chase by changing how the table is built.
        let grid = grid();
        let payload = payload_over(cells());
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        for radius_km in [1500.0, 4000.0] {
            for centre_row in [0u32, 9] {
                let row = grid.row(centre_row).expect("a row of the fixture");
                // One kernel for both tables: a shift in longitude is exactly what it is invariant to.
                let kernel = Kernel::new(grid, row, radius(radius_km)).expect("a whole-globe grid");

                for k in [1i64, 17, 35] {
                    let shifted_payload = payload_over(shifted_cells(k));
                    let shifted = Table::new(grid, &shifted_payload)
                        .expect("the build emits the padded product");

                    for centre_col in [0u32, 1, 35] {
                        let centre = grid.col(centre_col).expect("a column of the fixture");
                        assert_eq!(
                            population(&shifted, &kernel, grid.col_along(centre, k)),
                            population(&table, &kernel, centre),
                            "row {centre_row}, column {centre_col}, radius {radius_km} km, shift {k}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn evaluating_the_same_circle_twice_gives_the_same_bits() {
        // Compared as bits rather than as values, which is what box 3 asks for: the fold adds one
        // rectangle per row in the order `place` yields them, and nothing in this crate is parallel, so
        // there is no thread for a summation order to depend on.
        let grid = grid();
        let payload = payload_over(cells());
        let table = Table::new(grid, &payload).expect("the build emits the padded product");
        let kernel =
            Kernel::new(grid, grid.middle_row(), radius(4000.0)).expect("a whole-globe grid");
        let centre = grid.col(7).expect("a column of the fixture");

        assert_eq!(
            population(&table, &kernel, centre).to_bits(),
            population(&table, &kernel, centre).to_bits()
        );
    }

    /// The fixture and a table over it, which every case below wants and none of them varies.
    fn fixture() -> (Grid, Vec<f64>) {
        (grid(), payload_over(cells()))
    }

    #[test]
    fn a_cap_over_the_pole_counts_the_polar_row_once() {
        // Row 0 is 85 N and a 2000 km cap is 17.99 degrees, so the far side of that parallel is 10
        // degrees away and the whole row is inside: the case a fold assembling a closed row from two
        // pieces would double-count. The span is asserted as well as the population, so a later change
        // that stops closing the row fails here rather than passing on a case it no longer covers.
        let (grid, payload) = fixture();
        let table = Table::new(grid, &payload).expect("the build emits the padded product");
        let row = grid.row(0).expect("a row of the fixture");
        let kernel = Kernel::new(grid, row, radius(2000.0)).expect("a whole-globe grid");

        assert_eq!(kernel.rows().next(), Some((row, Span::FullTurn)));
        for centre_col in [0u32, 18] {
            let centre = grid.col(centre_col).expect("a column of the fixture");
            assert_eq!(
                population(&table, &kernel, centre),
                by_distance(&grid, (row, centre), 2000.0),
                "column {centre_col}"
            );
        }
    }

    #[test]
    fn a_cap_across_the_antimeridian_counts_both_sides() {
        // Row 9 is 5 S and a 1500 km cap reaches a column either side, so placing it on column 0 runs
        // back onto column 35: west holds the higher index, which is a wrap and not an inversion.
        let (grid, payload) = fixture();
        let table = Table::new(grid, &payload).expect("the build emits the padded product");
        let row = grid.row(9).expect("a row of the fixture");
        let centre = grid.col(0).expect("a column of the fixture");
        let kernel = Kernel::new(grid, row, radius(1500.0)).expect("a whole-globe grid");

        let (_, cols) = kernel
            .place(centre)
            .find(|(placed, _)| *placed == row)
            .expect("the centre row is in the band");
        match cols {
            ColSpan::Through { west, east } => {
                assert!(west.get() > east.get(), "{west:?} {east:?}");
            }
            ColSpan::FullTurn => panic!("a 1500 km cap does not close a row at 5 S on this grid"),
        }

        assert_eq!(
            population(&table, &kernel, centre),
            by_distance(&grid, (row, centre), 1500.0)
        );
    }

    #[test]
    fn a_cap_larger_than_the_globe_is_the_whole_table() {
        // Past half the circumference, so every row closes and the circle is the world. Compared against
        // the table's own extent rather than against a second sum of its own: a fold that double-counted a
        // seam or read a row twice would agree with another fold making the same mistake, and cannot agree
        // with this.
        let (grid, payload) = fixture();
        let table = Table::new(grid, &payload).expect("the build emits the padded product");
        let kernel =
            Kernel::new(grid, grid.middle_row(), radius(20_016.0)).expect("a whole-globe grid");
        let (rows, cols) = table.whole();

        assert_eq!(
            population(
                &table,
                &kernel,
                grid.col(23).expect("a column of the fixture")
            ),
            table.population(rows, cols)
        );
    }

    #[test]
    fn a_zero_radius_is_the_centre_cell() {
        // Degenerate and legal: the centre cell is within zero of itself and no other cell is.
        let (grid, payload) = fixture();
        let table = Table::new(grid, &payload).expect("the build emits the padded product");
        let row = grid.row(4).expect("a row of the fixture");
        let centre = grid.col(29).expect("a column of the fixture");
        let kernel = Kernel::new(grid, row, radius(0.0)).expect("zero is a radius");

        assert_eq!(
            population(&table, &kernel, centre),
            f64::from(cell(row.get(), centre.get()))
        );
    }

    proptest! {
        /// The invariant application.md "Correctness invariants" states directly, and the one #7's binary
        /// search over radius rests on. #4's proptest pinned its geometric half — no row's span narrows as
        /// the radius grows — and what this adds is that the fold reads every row the wider kernel has: a
        /// row dropped for some radii leaves every span monotone while the population is not.
        ///
        /// Over 1.2's fixture and not a generated payload, deliberately. Monotonicity is a property of the
        /// cell set, while `Table::population` is a four-corner subtraction: on cells whose partial sums
        /// round, the wider circle can come back a hair low and the failure would read as a dropped row
        /// while being the table's arithmetic. Cells no larger than 648 make every rectangle exact, which
        /// is what turns `>=` from an approximation into the claim.
        ///
        /// The radius range reaches caps that close a row, so `Span::FullTurn` is inside the domain rather
        /// than beside it: at 85 N — row 0 — the far side of the parallel is 1112 km away.
        #[test]
        fn growing_the_radius_never_shrinks_the_population(
            centre_row in 0u32..HEIGHT,
            centre_col in 0u32..WIDTH,
            radius_km in 50.0f64..9000.0,
            growth in 1.0f64..3000.0,
        ) {
            let grid = grid();
            let payload = payload_over(cells());
            let table = Table::new(grid, &payload).expect("the build emits the padded product");
            let row = grid.row(centre_row).expect("a row of the fixture");
            let centre = grid.col(centre_col).expect("a column of the fixture");

            let inner = Kernel::new(grid, row, radius(radius_km)).expect("a whole-globe grid");
            let outer = Kernel::new(grid, row, radius(radius_km + growth)).expect("a whole-globe grid");

            let (narrow, wide) = (
                population(&table, &inner, centre),
                population(&table, &outer, centre),
            );
            prop_assert!(
                wide >= narrow,
                "{radius_km} km holds {narrow} and {} km holds {wide}",
                radius_km + growth
            );
        }
    }

    #[test]
    #[should_panic(expected = "built over a different grid")]
    fn a_kernel_from_another_grid_is_no_circle_here() {
        let grid = grid();
        let coarser = Grid::new(
            WIDTH / 2,
            HEIGHT / 2,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            20.0,
            -20.0,
        )
        .expect("an 18 x 9 whole-globe grid is valid");
        let payload = payload_over(cells());
        let table = Table::new(grid, &payload).expect("the build emits the padded product");
        let kernel =
            Kernel::new(coarser, coarser.middle_row(), radius(3000.0)).expect("a whole-globe grid");

        let _ = population(
            &table,
            &kernel,
            grid.col(0).expect("a column of the fixture"),
        );
    }
}
