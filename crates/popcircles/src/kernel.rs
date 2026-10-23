// Step 2 of application.md "Approach": the columns a spherical cap of a given ground radius covers in
// each row of a grid. A span is an offset from a centre column rather than a pair of column indices,
// which is what makes one kernel serve every centre on its row and leaves the seam to one place.
//
// A cell is in the cap when its centre is within the cap's angular radius of the cap centre, measured
// through geodesy. That is the definition, and the spherical law of cosines below is an estimate the
// definition corrects rather than the answer: arccos loses precision exactly at the cap boundary, where
// its derivative is unbounded, and what the search needs is a membership rule a brute-force distance
// test reproduces — application.md "Correctness invariants".

use crate::geodesy::{LatLon, RadiusKm, angular_distance_rad, central_angle_rad};
use crate::grid::{Col, Grid, GridIndexError, Row};
use crate::table::ColSpan;

#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum KernelError {
    #[error(
        "a kernel is longitude-invariant only where the columns close; these span {lon_span} degrees"
    )]
    ColumnsDoNotClose { lon_span: f64 },
    #[error("the kernel index is outside its grid")]
    GridIndex(#[from] GridIndexError),
}

/// The columns one row of a kernel covers, as an offset from the centre column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    /// `half_width` columns either side of the centre column, and the centre column itself: `2h + 1`
    /// columns in all.
    Around { half_width: u32 },
    /// Every column of the row. A variant rather than the half width that would reach it, for
    /// [`crate::table::ColSpan::FullTurn`]'s reason: a row wide enough to close on itself must not be
    /// assembled from two pieces that double-count where they meet.
    FullTurn,
}

/// The cap being measured, in the terms one row's span needs, and the only place the geometry lives.
#[derive(Debug, Clone, Copy)]
struct Cap {
    angle_rad: f64,
    centre_lat: f64,
    lon_step: f64,
    width: u32,
}

impl Cap {
    /// The radius becomes an angle once, here, and is clamped at half a turn: no two points on a sphere
    /// are further apart than that, and an angle past π would leave `cos θ` in [`Cap::estimate`]
    /// oscillating instead of saturating — an estimate the walk would then pay for a column at a time.
    fn over(grid: &Grid, centre_lat: f64, radius_km: f64) -> Self {
        Self {
            angle_rad: central_angle_rad(radius_km).min(std::f64::consts::PI),
            centre_lat,
            lon_step: grid.lon_step(),
            width: grid.width(),
        }
    }

    /// The smallest half width that closes a row: `2h + 1 >= width` holds here at both parities, so
    /// reaching it is the full turn rather than a span a column short of one.
    const fn closing(&self) -> u32 {
        self.width / 2
    }

    /// Whether the cell `cells` columns along the row at `lat` is in the cap.
    ///
    /// The longitude is an **offset** — `cells` steps of the grid's own step — and never the difference
    /// of two [`Grid::centre_of`] longitudes, which is a whole multiple of that step only to within a
    /// rounding. Taking the offset is what makes a kernel exactly longitude-invariant rather than
    /// invariant to within the rounding that decides a boundary cell.
    fn contains(&self, lat: f64, cells: u32) -> bool {
        let centre = LatLon {
            lat: self.centre_lat,
            lon: 0.0,
        };
        let along = LatLon {
            lat,
            lon: f64::from(cells) * self.lon_step,
        };
        angular_distance_rad(centre, along) <= self.angle_rad
    }

    /// Whether the cap reaches the row at `lat` at all.
    ///
    /// The cell on the cap's own meridian is the row's closest to the centre, because for two fixed
    /// latitudes the great-circle distance grows with the longitude difference across the half turn. So
    /// a row this refuses holds no cell of the cap, and the rows it admits are contiguous.
    fn reaches(&self, lat: f64) -> bool {
        self.contains(lat, 0)
    }

    /// The columns the cap covers in a row it reaches.
    fn span(&self, lat: f64) -> Span {
        let closing = self.closing();
        let mut half_width = self.estimate(lat).min(closing);
        // The estimate is a starting point and `contains` is the definition, so this walk is what makes
        // the two agree. Either loop moves a step or two in practice, and no estimate can make the
        // answer wrong — only slow. What makes them terminate rather than merely stop is the same
        // monotonicity `reaches` rests on.
        while half_width < closing && self.contains(lat, half_width + 1) {
            half_width += 1;
        }
        while half_width > 0 && !self.contains(lat, half_width) {
            half_width -= 1;
        }

        if half_width == closing {
            Span::FullTurn
        } else {
            Span::Around { half_width }
        }
    }

    /// The half width the spherical law of cosines gives: `cos θ = sin φ₀ sin φ + cos φ₀ cos φ cos Δλ`,
    /// solved for Δλ and floored to whole columns.
    fn estimate(&self, lat: f64) -> u32 {
        let closing = self.closing();
        let (centre, row) = (self.centre_lat.to_radians(), lat.to_radians());
        let ratio = (self.angle_rad.cos() - centre.sin() * row.sin()) / (centre.cos() * row.cos());

        // The two degenerate branches, explicit because no arccos answers either: at or past 1 the cap
        // reaches no further than its own meridian in this row, and at or past −1 it covers the row.
        if ratio >= 1.0 {
            return 0;
        }
        if ratio <= -1.0 {
            return closing;
        }

        let cells = ratio.acos().to_degrees() / self.lon_step.abs();
        // Also where a ratio that is not a number lands, having taken neither branch above. Nothing
        // reachable through [`Kernel`] produces one — a grid's latitudes are finite and the cosine of a
        // representable latitude is never exactly zero, `cos(90°)` being 6.1e-17, so the denominator
        // does not collapse — and this is about what such an input would cost rather than what it
        // returns, since the walk in `span` corrects any estimate.
        if cells.is_nan() || cells >= f64::from(closing) {
            return closing;
        }
        // `cells` is finite and inside `[0, closing)`, so truncating is exact, and truncation is the
        // floor this wants: the last column the estimate claims. std offers no fallible f64 -> u32.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        (cells as u32)
    }
}

/// A cap of a given ground radius, decomposed into the columns it covers in each row it reaches.
///
/// It is built for a **row**, not for a cell: the shape depends on the centre's latitude and on nothing
/// else, so one kernel serves every centre on that row and the longitudes enter only when it is placed.
///
/// A grid whose columns do not close has no kernel. Invariance is the whole point of the type, and on a
/// grid that does not close a span would need clipping that depends on which column the centre is —
/// which is to say it would not be one shape but a different shape per centre.
#[derive(Debug, Clone, PartialEq)]
pub struct Kernel {
    grid: Grid,
    centre: Row,
    radius: RadiusKm,
    north: Row,
    spans: Vec<Span>,
}

impl Kernel {
    /// # Errors
    /// [`KernelError::ColumnsDoNotClose`] when the grid's columns do not close on themselves, which is
    /// the whole of what a caller holding a [`RadiusKm`] can be wrong about here. Zero is a radius: the
    /// cap is the centre cell alone. Returns [`KernelError::GridIndex`] if `centre` is outside the grid.
    pub fn new(grid: Grid, centre: Row, radius: RadiusKm) -> Result<Self, KernelError> {
        let centre_lat = grid.centre_lat(centre)?;
        if !grid.spans_full_turn() {
            return Err(KernelError::ColumnsDoNotClose {
                lon_span: f64::from(grid.width()) * grid.lon_step().abs(),
            });
        }

        let cap = Cap::over(&grid, centre_lat, radius.km());

        // North then south from the centre row, each stopping at the first row the cap does not reach:
        // the rows it reaches are contiguous, so a first miss is the end of the band and not a gap in
        // it. The grid's own ends stop the walk too, which is how a cap reaching past a pole — or past
        // the edge of a grid covering a band of latitude only — is clipped rather than refused.
        let mut north = centre;
        while let Some(previous) = north.get().checked_sub(1).and_then(|index| grid.row(index)) {
            if !cap.reaches(grid.centre_lat(previous)?) {
                break;
            }
            north = previous;
        }
        let mut south = centre;
        while let Some(next) = south.get().checked_add(1).and_then(|index| grid.row(index)) {
            if !cap.reaches(grid.centre_lat(next)?) {
                break;
            }
            south = next;
        }

        let spans = grid
            .rows()
            .skip(north.get() as usize)
            .take_while(|row| *row <= south)
            .map(|row| grid.centre_lat(row).map(|latitude| cap.span(latitude)))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            grid,
            centre,
            radius,
            north,
            spans,
        })
    }

    #[must_use]
    pub const fn centre(&self) -> Row {
        self.centre
    }

    /// The radius this kernel was built for, as the type it was built from: a caller wanting the number
    /// asks it for kilometres, and one widening it re-enters the constructor that checks the sum.
    #[must_use]
    pub const fn radius(&self) -> RadiusKm {
        self.radius
    }

    /// The grid this kernel was built over — the whole of it, not the band the cap reaches. A cap
    /// clipped at the grid's edge still belongs to the grid, and an evaluation checking a table against
    /// this needs the grid the table's own would have to equal.
    #[must_use]
    pub const fn grid(&self) -> &Grid {
        &self.grid
    }

    /// Every row the cap reaches, north to south, with the columns it covers there.
    pub fn rows(&self) -> impl Iterator<Item = (Row, Span)> + '_ {
        self.grid
            .rows()
            .skip(self.north.get() as usize)
            .zip(self.spans.iter().copied())
    }

    /// The kernel laid on a centre column: each row it reaches and the columns it covers there, in the
    /// terms [`crate::table::Table::population`] takes.
    ///
    /// Yielding a [`ColSpan`] rather than a shape of its own is deliberate. A second spelling of
    /// "wrapped columns" would have to be converted into that one somewhere, and the conversion is
    /// exactly where the double count [`ColSpan::FullTurn`] exists to prevent comes back.
    ///
    /// The offsets are symmetric about the centre, so which of a [`ColSpan::Through`]'s two fields the
    /// lower index lands in follows `lon_step`'s sign. The set of columns does not, and neither does its
    /// population.
    ///
    /// # Errors
    /// Returns [`GridIndexError`] if `centre` is outside the grid or a yielded placement is invalid.
    pub fn place(
        &self,
        centre: Col,
    ) -> Result<impl Iterator<Item = Result<(Row, ColSpan), GridIndexError>> + '_, GridIndexError>
    {
        self.grid.col_along(centre, 0)?;
        Ok(self.rows().map(move |(row, span)| {
            let cols = match span {
                Span::FullTurn => ColSpan::FullTurn,
                Span::Around { half_width } => {
                    let offset = i64::from(half_width);
                    ColSpan::Through {
                        west: self.grid.col_along(centre, -offset)?,
                        east: self.grid.col_along(centre, offset)?,
                    }
                }
            };
            Ok((row, cols))
        }))
    }
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows both in tests. float_cmp is here for the spans compared as values:
// a Span holds an integer count, and comparing two kernels' spans is exact by construction.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use proptest::prelude::*;

    use super::*;
    use crate::geodesy::{EARTH_RADIUS_KM, great_circle_km};

    /// A whole-globe grid, which is the shape that has kernels at all.
    fn globe(cells_per_degree: u32) -> Grid {
        let step = 1.0 / f64::from(cells_per_degree);
        Grid::new(
            360 * cells_per_degree,
            180 * cells_per_degree,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            step,
            -step,
        )
        .expect("a whole-globe grid at a whole number of cells per degree is valid")
    }

    fn row(grid: &Grid, index: u32) -> Row {
        grid.row(index).expect("a row of the fixture")
    }

    fn radius(km: f64) -> RadiusKm {
        RadiusKm::new(km).expect("a fixture radius is a length")
    }

    fn kernel(grid: Grid, centre: u32, radius_km: f64) -> Kernel {
        Kernel::new(grid, row(&grid, centre), radius(radius_km)).expect("a whole-globe grid")
    }

    #[test]
    fn a_cap_reaching_no_further_than_its_own_meridian_has_no_width() {
        // The ratio is exactly 1 here: the cap's edge touches this row on the cap's meridian and nowhere
        // else, so there is no arccos to take and the row is one cell wide.
        let cap = Cap {
            angle_rad: 1.0f64.to_radians(),
            centre_lat: 0.0,
            lon_step: 1.0,
            width: 360,
        };
        assert_eq!(cap.estimate(1.0), 0);
        assert_eq!(cap.span(1.0), Span::Around { half_width: 0 });
    }

    #[test]
    fn a_cap_swallowing_a_parallel_covers_the_whole_row() {
        // The other degenerate branch: a cap centred beside the pole and wide enough that every column
        // of this row is inside it, where the ratio is far past −1.
        let cap = Cap {
            angle_rad: 90.0f64.to_radians(),
            centre_lat: 89.0,
            lon_step: 1.0,
            width: 360,
        };
        assert_eq!(cap.estimate(88.0), 180);
        assert_eq!(cap.span(88.0), Span::FullTurn);
    }

    #[test]
    fn an_estimate_that_is_not_a_number_clamps_rather_than_running_away() {
        // Unreachable through `Kernel`, and constructed here directly for that reason. What it pins is
        // the cost of an impossible input rather than its answer: the walk would correct any estimate,
        // and an unbounded one would make it walk half a row to do so.
        let cap = Cap {
            angle_rad: f64::NAN,
            centre_lat: 0.0,
            lon_step: 1.0,
            width: 360,
        };
        assert_eq!(cap.estimate(0.0), 180);
    }

    #[test]
    fn every_rows_span_is_the_columns_a_distance_test_admits() {
        let grid = globe(1);
        for centre_lat in [0.0, 12.5, -47.5, 60.5, 89.5] {
            for radius_km in [250.0, 1000.0, 3000.0, 8000.0] {
                let cap = Cap::over(&grid, centre_lat, radius_km);
                for row in grid.rows() {
                    let lat = grid
                        .centre_lat(row)
                        .expect("indices belong to the fixture grid");
                    if !cap.reaches(lat) {
                        continue;
                    }

                    // Kilometres against kilometres, which is the comparison the answer has to be
                    // reproducible in and is independent of the angle the cap converted the radius to.
                    let inside = |cells: u32| {
                        great_circle_km(
                            LatLon {
                                lat: centre_lat,
                                lon: 0.0,
                            },
                            LatLon {
                                lat,
                                lon: f64::from(cells) * grid.lon_step(),
                            },
                        ) <= radius_km
                    };
                    let reach = (0..=cap.closing())
                        .take_while(|cells| inside(*cells))
                        .last()
                        .expect("the cap reaches this row, so its meridian cell is in");
                    let expected = if reach == cap.closing() {
                        Span::FullTurn
                    } else {
                        Span::Around { half_width: reach }
                    };

                    assert_eq!(
                        cap.span(lat),
                        expected,
                        "centre {centre_lat}, radius {radius_km} km, row {}",
                        row.get()
                    );
                }
            }
        }
    }

    #[test]
    fn a_row_that_closes_is_the_full_turn_at_either_parity_of_the_width() {
        // Four columns and five: `closing` is 2 either way, and 2 · 2 + 1 covers five exactly and
        // overruns four. What must not happen is a row reported as `Around` when its span has closed,
        // which is the double count `ColSpan::FullTurn` exists to prevent.
        for width in [4u32, 5] {
            let grid = Grid::new(
                width,
                90,
                LatLon {
                    lat: 90.0,
                    lon: -180.0,
                },
                360.0 / f64::from(width),
                -2.0,
            )
            .expect("a whole-globe grid of any width is valid");

            for radius_km in [100.0, 1000.0, 6000.0, 12_000.0, 20_016.0] {
                let cap = Cap::over(
                    &grid,
                    grid.centre_lat(grid.middle_row())
                        .expect("middle row belongs to the grid"),
                    radius_km,
                );
                for row in grid.rows() {
                    let lat = grid
                        .centre_lat(row)
                        .expect("indices belong to the fixture grid");
                    if !cap.reaches(lat) {
                        continue;
                    }
                    match cap.span(lat) {
                        Span::Around { half_width } => {
                            assert!(
                                2 * half_width + 1 < width,
                                "{width} columns, {radius_km} km"
                            );
                            assert!(
                                !cap.contains(lat, half_width + 1),
                                "{width} columns, {radius_km} km: the span stops a column short"
                            );
                        }
                        Span::FullTurn => assert!(
                            cap.contains(lat, cap.closing()),
                            "{width} columns, {radius_km} km: a row closed that the cap does not cover"
                        ),
                    }
                }
            }
        }
    }

    #[test]
    fn a_grid_whose_columns_do_not_close_has_no_kernel() {
        let window = Grid::new(
            600,
            600,
            LatLon {
                lat: 60.0,
                lon: -10.0,
            },
            1.0 / 120.0,
            -1.0 / 120.0,
        )
        .expect("a window grid is valid");
        assert!(matches!(
            Kernel::new(window, row(&window, 0), radius(500.0)),
            Err(KernelError::ColumnsDoNotClose { .. })
        ));
    }

    #[test]
    fn a_zero_radius_is_the_centre_cell_alone() {
        // Degenerate and legal: the centre's own cell is within zero of itself and no other cell is.
        let grid = globe(1);
        let centre = row(&grid, 100);
        let kernel = Kernel::new(grid, centre, radius(0.0)).expect("zero is a radius");
        assert_eq!(
            kernel.rows().collect::<Vec<(Row, Span)>>(),
            vec![(centre, Span::Around { half_width: 0 })]
        );
    }

    #[test]
    fn a_cap_over_the_pole_closes_the_rows_beside_it() {
        // Centre 87.5 N, radius 1000 km = 8.993 degrees. The pole is 2.5 degrees away, so a parallel up
        // to 6.49 degrees down the far side is swallowed whole: 84.5 N is 2.5 + 5.5 = 8 degrees off and
        // closes, 83.5 N is 9 and does not. Nothing here takes a branch of its own — the ordinary walk
        // and the ordinary estimate produce it.
        let grid = globe(1);
        let spans: Vec<(Row, Span)> = kernel(grid, 2, 1000.0).rows().collect();

        assert_eq!(spans[0], (row(&grid, 0), Span::FullTurn));
        assert_eq!(spans[5], (row(&grid, 5), Span::FullTurn));
        assert!(matches!(spans[6], (_, Span::Around { .. })));
        // The band ends at 79.5 N, 8 degrees south of the centre, and every row from there north is one
        // the cap reaches.
        assert_eq!(spans.len(), 11);
        assert!(matches!(spans[10], (_, Span::Around { .. })));
    }

    #[test]
    fn a_cap_larger_than_the_globe_closes_every_row() {
        // Half the circumference is 20 015.09 km, so this reaches every point on the sphere. Every row
        // closes as a variant, and none of them as a wrapped pair.
        let grid = globe(1);
        let kernel = kernel(grid, 90, 20_016.0);
        assert_eq!(kernel.rows().count(), grid.height() as usize);
        assert!(kernel.rows().all(|(_, span)| span == Span::FullTurn));
    }

    #[test]
    fn a_cap_running_off_a_band_grid_is_clipped_rather_than_refused() {
        // Closing in longitude, thirty degrees of latitude: the shape a regional grid has. The cap wants
        // to reach 86.5 N and the grid starts at 59.5, so the band's northern end is the grid's own.
        let band = Grid::new(
            360,
            30,
            LatLon {
                lat: 60.0,
                lon: -180.0,
            },
            1.0,
            -1.0,
        )
        .expect("a band grid that closes in longitude is valid");
        let kernel =
            Kernel::new(band, row(&band, 0), radius(3000.0)).expect("a band grid has kernels");

        let rows: Vec<Row> = kernel.rows().map(|(row, _)| row).collect();
        // 3000 km is 26.979 degrees, so the band runs from 59.5 N to 33.5 N: twenty-seven rows, of which
        // the first is the grid's first and not the cap's.
        assert_eq!(rows.first(), Some(&row(&band, 0)));
        assert_eq!(rows.len(), 27);
    }

    #[test]
    fn a_kernels_grid_is_the_one_it_was_built_over_and_not_the_band_it_covers() {
        let band = Grid::new(
            360,
            30,
            LatLon {
                lat: 60.0,
                lon: -180.0,
            },
            1.0,
            -1.0,
        )
        .expect("a band grid that closes in longitude is valid");
        let kernel =
            Kernel::new(band, row(&band, 0), radius(3000.0)).expect("a band grid has kernels");

        assert_eq!(*kernel.grid(), band);
        // Twenty-seven of the grid's thirty rows, so an accessor answering with the band the cap
        // reaches would fail the line above rather than agree with it.
        assert_eq!(kernel.rows().count(), 27);
    }

    #[test]
    fn a_finer_grids_row_is_no_centre_here() {
        let centre = row(&globe(4), 719);
        assert!(matches!(
            Kernel::new(globe(1), centre, radius(500.0)),
            Err(KernelError::GridIndex(GridIndexError::RowOutOfBounds {
                row: 719,
                height: 180,
            }))
        ));
    }

    /// An east-to-west whole globe, where a placed span's `west` field holds the eastern column: the set
    /// of columns is the same either way, which is what `place` claims and this is what tests it.
    fn westward() -> Grid {
        Grid::new(
            360,
            180,
            LatLon {
                lat: 90.0,
                lon: 180.0,
            },
            -1.0,
            -1.0,
        )
        .expect("an east-to-west whole-globe grid is valid")
    }

    fn col(grid: &Grid, index: u32) -> Col {
        grid.col(index).expect("a column of the fixture")
    }

    /// The columns a span names, expanded, so a wrapped one and a closed one are the same kind of thing.
    fn columns(grid: &Grid, span: ColSpan) -> Vec<u32> {
        match span {
            ColSpan::FullTurn => (0..grid.width()).collect(),
            ColSpan::Through { west, east } => {
                if west.get() <= east.get() {
                    (west.get()..=east.get()).collect()
                } else {
                    (west.get()..grid.width()).chain(0..=east.get()).collect()
                }
            }
        }
    }

    fn placed(grid: &Grid, kernel: &Kernel, centre: Col) -> BTreeSet<(u32, u32)> {
        kernel
            .place(centre)
            .expect("fixture centre belongs to the kernel grid")
            .map(|placed| placed.expect("placement indices belong to the grid"))
            .flat_map(|(row, cols)| {
                columns(grid, cols)
                    .into_iter()
                    .map(move |col| (row.get(), col))
            })
            .collect()
    }

    /// Every cell of the whole grid within `radius_km` of the centre cell, by the distance test and
    /// nothing else. The scan covers the grid rather than the kernel's band, which is the only way it can
    /// catch a cell the kernel left out.
    fn by_distance(grid: &Grid, centre: (Row, Col), radius_km: f64) -> BTreeSet<(u32, u32)> {
        let from = grid
            .centre_of(centre.0, centre.1)
            .expect("indices belong to the fixture grid");
        grid.rows()
            .flat_map(|row| grid.cols().map(move |col| (row, col)))
            .filter(|(row, col)| {
                great_circle_km(
                    from,
                    grid.centre_of(*row, *col)
                        .expect("indices belong to the fixture grid"),
                ) <= radius_km
            })
            .map(|(row, col)| (row.get(), col.get()))
            .collect()
    }

    #[test]
    fn a_placed_kernel_is_the_cells_a_distance_test_admits() {
        // Row 2 is 87.5 N, where a 1000 km cap covers the pole; row 45 is mid-latitude; row 90 straddles
        // the equator. Columns 0 and 359 put the seam inside the span, which is the case a span clipped
        // instead of wrapped would fail on.
        let grid = globe(1);
        for centre_row in [2u32, 45, 90] {
            for radius_km in [1000.0, 4000.0] {
                let kernel = kernel(grid, centre_row, radius_km);
                for centre_col in [0u32, 180, 359] {
                    let centre = (row(&grid, centre_row), col(&grid, centre_col));
                    assert_eq!(
                        placed(&grid, &kernel, centre.1),
                        by_distance(&grid, centre, radius_km),
                        "row {centre_row}, column {centre_col}, radius {radius_km} km"
                    );
                }
            }
        }
    }

    #[test]
    fn a_placed_kernel_on_a_westward_grid_is_the_same_cells() {
        // The compass names in a `ColSpan` follow the grid's column direction; the cells do not. Nothing
        // in the kernel knows which way the columns run, and this is what says that is sound.
        let grid = westward();
        let kernel = Kernel::new(grid, row(&grid, 60), radius(2500.0))
            .expect("a whole-globe grid has kernels");
        for centre_col in [0u32, 359] {
            let centre = (row(&grid, 60), col(&grid, centre_col));
            assert_eq!(
                placed(&grid, &kernel, centre.1),
                by_distance(&grid, centre, 2500.0),
                "column {centre_col}"
            );
        }
    }

    #[test]
    fn one_kernel_serves_every_centre_on_its_row() {
        // The claim the offsets are for: placing the same kernel anywhere along its row gives the same
        // shape, shifted. Asserted as the shifted set rather than as a count, because two different
        // shapes can share a cell count.
        let grid = globe(1);
        let kernel = kernel(grid, 70, 3000.0);
        let at_origin = placed(&grid, &kernel, col(&grid, 0));

        for centre_col in grid.cols() {
            let here = placed(&grid, &kernel, centre_col);
            let shifted: BTreeSet<(u32, u32)> = at_origin
                .iter()
                .map(|(row, column)| (*row, (column + centre_col.get()) % grid.width()))
                .collect();
            assert_eq!(here, shifted, "column {}", centre_col.get());
        }
    }

    #[test]
    fn a_closed_row_places_as_the_full_turn_and_never_as_a_pair() {
        let grid = globe(1);
        let closed = kernel(grid, 90, 20_016.0);
        assert!(
            closed
                .place(col(&grid, 17))
                .expect("fixture centre belongs to the kernel grid")
                .map(|placed| placed.expect("placement indices belong to the grid"))
                .all(|(_, cols)| cols == ColSpan::FullTurn)
        );

        // And the seam beside a partial span: at column 0 the span runs back past the first column onto
        // the last, so west holds the higher index. That is a wrap, not an inversion.
        let small = kernel(grid, 90, 500.0);
        let (_, cols) = small
            .place(col(&grid, 0))
            .expect("fixture centre belongs to the kernel grid")
            .map(|placed| placed.expect("placement indices belong to the grid"))
            .find(|(row, _)| *row == small.centre())
            .expect("the centre row is in the band");
        match cols {
            ColSpan::Through { west, east } => {
                assert!(west.get() > east.get(), "{west:?} to {east:?}");
                assert_eq!(columns(&grid, cols).len(), 2 * 4 + 1);
            }
            ColSpan::FullTurn => panic!("a 500 km cap does not close a row at the equator"),
        }
    }

    #[test]
    fn a_finer_grids_column_is_no_centre_here() {
        let centre = col(&globe(4), 1439);
        assert!(matches!(
            kernel(globe(1), 90, 500.0).place(centre),
            Err(GridIndexError::ColumnOutOfBounds {
                column: 1439,
                width: 360,
            })
        ));
    }

    /// The ground the kernel covers: each row's cell count times that row's cell area. It reads the area
    /// through the grid rather than recomputing a zone, so the earth model stays in one place.
    fn area_km2(grid: &Grid, kernel: &Kernel) -> f64 {
        kernel
            .rows()
            .map(|(row, span)| {
                let cells = match span {
                    Span::FullTurn => grid.width(),
                    Span::Around { half_width } => 2 * half_width + 1,
                };
                f64::from(cells)
                    * grid
                        .cell_area_km2(row)
                        .expect("indices belong to the fixture grid")
            })
            .sum()
    }

    #[test]
    fn a_kernels_area_is_the_spherical_caps() {
        // Against 2πR²(1 − cos θ), which is the cap's area on the sphere and owes nothing to this code.
        // The bounds are twice the error measured off this membership rule, and two things they cannot
        // be:
        //
        // - One figure for every case. The error scales with cell size over cap radius, so at one cell
        //   per degree a 1000 km cap at 60 N is 1.2e-2 out while an 8000 km cap is 8.5e-4, and a bound
        //   loose enough for the first would let the second be wrong by an order of magnitude.
        // - Monotone under refinement case by case. Row 45 at 3000 km goes 1.05e-4 at one cell per degree
        //   to 4.28e-4 at four: the discretisation error is not a bias that shrinks but a residue that
        //   resamples, so refining moves which cells straddle the boundary rather than reducing their
        //   effect. What does converge is the envelope over a fixed set of caps, which is the last
        //   assertion here.
        //
        // Each case is a row index on the one-degree grid, scaled by the resolution so the same cap is
        // measured at both, with the bound measured at each.
        let cases = [
            (90u32, 3000.0, [1.1e-2, 1.1e-3]),
            (45, 3000.0, [2.1e-4, 8.6e-4]),
            (30, 1000.0, [2.5e-2, 3.7e-3]),
            (2, 1000.0, [1.2e-2, 2.5e-3]),
            (120, 8000.0, [1.7e-3, 3.6e-5]),
        ];
        let mut worst = [0.0f64; 2];

        for (resolution, cells_per_degree) in [1u32, 4].into_iter().enumerate() {
            let grid = globe(cells_per_degree);
            for (row1, radius_km, bounds) in cases {
                let kernel = kernel(grid, row1 * cells_per_degree, radius_km);
                let exact = 2.0
                    * std::f64::consts::PI
                    * EARTH_RADIUS_KM
                    * EARTH_RADIUS_KM
                    * (1.0 - central_angle_rad(radius_km).cos());
                let relative = (area_km2(&grid, &kernel) - exact).abs() / exact;

                assert!(
                    relative < bounds[resolution],
                    "{cells_per_degree} cells per degree, row {row1}, {radius_km} km: off by \
                     {relative}, past {}",
                    bounds[resolution]
                );
                worst[resolution] = worst[resolution].max(relative);
            }
        }

        // The convergence claim the per-case figures cannot make: 1.2e-2 at one cell per degree against
        // 1.8e-3 at four, over the same five caps.
        assert!(
            worst[1] < worst[0],
            "refining did not shrink the envelope: {worst:?}"
        );
    }

    /// Whether `wider` covers every column `narrower` does, both being spans about one centre.
    fn covers(wider: Span, narrower: Span) -> bool {
        match (wider, narrower) {
            (Span::FullTurn, _) => true,
            (Span::Around { .. }, Span::FullTurn) => false,
            (Span::Around { half_width: wide }, Span::Around { half_width: narrow }) => {
                wide >= narrow
            }
        }
    }

    proptest! {
        /// The property #7's binary search over radius rests on: population is monotone in radius only
        /// if the cells are, and a span that narrowed anywhere as the radius grew would break it while
        /// still returning a plausible number.
        #[test]
        fn growing_the_radius_never_narrows_a_row_or_shortens_the_band(
            centre in 0u32..180,
            radius_km in 50.0f64..9000.0,
            growth in 1.0f64..2000.0,
        ) {
            let grid = globe(1);
            let grown: BTreeMap<Row, Span> = kernel(grid, centre, radius_km + growth).rows().collect();

            for (row, span) in kernel(grid, centre, radius_km).rows() {
                let wider = grown.get(&row).copied();
                prop_assert!(wider.is_some(), "row {} left the band", row.get());
                prop_assert!(
                    covers(wider.unwrap(), span),
                    "row {} narrowed from {:?} to {:?}",
                    row.get(),
                    span,
                    wider.unwrap()
                );
            }
        }
    }
}
