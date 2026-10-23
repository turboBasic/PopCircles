use std::error::Error;
use std::fmt;

use crate::geodesy::{LatBand, LatLon, wrap_lon, zone_area_km2};

// A step arrives as a decimal from a geotransform and need not round-trip to the rational it
// means, so a span computed from one lands a few ulps off the whole number it is meant to hit —
// past -90 for a grid that in fact ends at the pole, short of 360 for one that in fact closes on
// itself. The size is what keeps this from swallowing a real discrepancy: being out by one cell
// costs 1/120° on the finest grid here, eight orders of magnitude above this.
//
// The registry raster is what fixes the scale rather than a guess: its step is 1/120 + 5.4e-16 and
// its origin latitude 90 + 1.16e-11, so the width times the step overshoots a full turn by 2.3e-11.
// pub(crate) because the raster reader compares a file's geotransform against a declared grid and
// has to allow exactly the same rounding this does — a second constant there would be two answers to
// one question.
pub(crate) const BOUNDARY_TOLERANCE_DEG: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridError {
    EmptyDimension { width: u32, height: u32 },
    NonFiniteOrigin { origin: LatLon },
    OriginLatOutOfRange { origin_lat: f64 },
    NonFiniteStep { lon_step: f64, lat_step: f64 },
    ZeroStep { lon_step: f64, lat_step: f64 },
    LatStepNotSouthward { lat_step: f64 },
    RunsPastSouthPole { south_edge: f64 },
    RunsPastAFullTurn { lon_span: f64 },
}

impl fmt::Display for GridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::EmptyDimension { width, height } => {
                write!(f, "grid has an empty dimension: {width} x {height}")
            }
            Self::NonFiniteOrigin { origin } => {
                write!(
                    f,
                    "grid origin is not finite: ({}, {})",
                    origin.lat, origin.lon
                )
            }
            Self::OriginLatOutOfRange { origin_lat } => {
                write!(f, "grid origin latitude {origin_lat} is outside [-90, 90]")
            }
            Self::NonFiniteStep { lon_step, lat_step } => {
                write!(f, "grid step is not finite: ({lon_step}, {lat_step})")
            }
            Self::ZeroStep { lon_step, lat_step } => {
                write!(f, "grid step is zero: ({lon_step}, {lat_step})")
            }
            Self::LatStepNotSouthward { lat_step } => write!(
                f,
                "grid lat_step {lat_step} is not negative; rows run north to south"
            ),
            Self::RunsPastSouthPole { south_edge } => {
                write!(f, "grid rows reach {south_edge}, past the south pole")
            }
            Self::RunsPastAFullTurn { lon_span } => {
                write!(f, "grid columns span {lon_span} degrees, past a full turn")
            }
        }
    }
}

impl Error for GridError {}

/// An index does not fit the grid receiving it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GridIndexError {
    #[error("row {row} is outside this {height}-row grid")]
    RowOutOfBounds { row: u32, height: u32 },
    #[error("column {column} is outside this {width}-column grid")]
    ColumnOutOfBounds { column: u32, width: u32 },
    #[error("cell ({row}, {column}) is outside this {width} x {height} grid")]
    CellOutOfBounds {
        row: u32,
        column: u32,
        width: u32,
        height: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LonSpan {
    pub west: f64,
    pub east: f64,
}

/// An index minted by one grid can be passed to another because the type carries no grid identity.
/// Index-consuming methods check it against the receiving grid and return [`GridIndexError`] when it
/// does not fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Row(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Col(u32);

impl Row {
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl Col {
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Rows run north to south, so `lat_step` is negative — the GDAL north-up convention, kept so a
/// geotransform read from a raster needs no sign flip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    width: u32,
    height: u32,
    origin: LatLon,
    lon_step: f64,
    lat_step: f64,
}

impl Grid {
    /// # Errors
    /// [`GridError`], whose variants name the cases, when the arguments cannot describe a north-up
    /// grid that stays on the globe.
    pub fn new(
        width: u32,
        height: u32,
        origin: LatLon,
        lon_step: f64,
        lat_step: f64,
    ) -> Result<Self, GridError> {
        if width == 0 || height == 0 {
            return Err(GridError::EmptyDimension { width, height });
        }
        if !origin.lat.is_finite() || !origin.lon.is_finite() {
            return Err(GridError::NonFiniteOrigin { origin });
        }
        if !(-90.0..=90.0).contains(&origin.lat) {
            return Err(GridError::OriginLatOutOfRange {
                origin_lat: origin.lat,
            });
        }
        if !lon_step.is_finite() || !lat_step.is_finite() {
            return Err(GridError::NonFiniteStep { lon_step, lat_step });
        }
        if lon_step == 0.0 || lat_step == 0.0 {
            return Err(GridError::ZeroStep { lon_step, lat_step });
        }
        if lat_step > 0.0 {
            return Err(GridError::LatStepNotSouthward { lat_step });
        }

        // u32 -> f64 is exact: f64 represents every integer below 2^53.
        let south_edge = origin.lat + f64::from(height) * lat_step;
        if south_edge < -90.0 - BOUNDARY_TOLERANCE_DEG {
            return Err(GridError::RunsPastSouthPole { south_edge });
        }

        // The longitude axis needs its own bound, and for a different reason than the latitude one:
        // past a full turn two columns address the same meridian, so `cell_containing` cannot invert
        // `centre_of` and the round trip fails on a plausible answer rather than on an error.
        let lon_span = f64::from(width) * lon_step.abs();
        if lon_span > 360.0 + BOUNDARY_TOLERANCE_DEG {
            return Err(GridError::RunsPastAFullTurn { lon_span });
        }

        Ok(Self {
            width,
            height,
            // Reducing the origin once here is what keeps `centre_of` and `cell_containing` talking
            // about the same longitudes: the inverse reduces modulo a turn and so is indifferent to
            // an origin a few turns out, while the forward direction would carry that offset into
            // every coordinate it returns and hand a renderer a longitude of 540.
            origin: LatLon {
                lat: origin.lat,
                lon: wrap_lon(origin.lon),
            },
            lon_step,
            lat_step,
        })
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub const fn origin(&self) -> LatLon {
        self.origin
    }

    #[must_use]
    pub const fn lon_step(&self) -> f64 {
        self.lon_step
    }

    #[must_use]
    pub const fn lat_step(&self) -> f64 {
        self.lat_step
    }

    /// The row this grid contains at `row`, or `None` when it has no such row.
    #[must_use]
    pub const fn row(&self, row: u32) -> Option<Row> {
        if row < self.height {
            Some(Row(row))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn col(&self, col: u32) -> Option<Col> {
        if col < self.width {
            Some(Col(col))
        } else {
            None
        }
    }

    /// The row halfway down the grid. The one mint that returns a [`Row`] rather than an
    /// `Option<Row>`, because the constructor rejects a zero height, so the middle index always
    /// names a row and a caller has no absent case to invent a number for.
    #[must_use]
    pub const fn middle_row(&self) -> Row {
        Row(self.height / 2)
    }

    /// Every row north to south, and every column west to east on a north-up grid. These exist so a
    /// traversal needs no index it has to check: the iterator is the mint.
    pub fn rows(&self) -> impl Iterator<Item = Row> {
        (0..self.height).map(Row)
    }

    pub fn cols(&self) -> impl Iterator<Item = Col> {
        (0..self.width).map(Col)
    }

    /// The centre of a cell, not its corner: an index addresses a sample, and every distance in the
    /// search is measured between sample centres.
    ///
    /// The longitude is reduced only as far as the origin was, so a grid whose columns cross the
    /// antimeridian returns one past 180 rather than folded — folding it here is what would put west
    /// east of east in a [`LonSpan`]. Distance does not care, since it works on a difference;
    /// anything presenting a coordinate reduces with [`crate::geodesy::wrap_lon`].
    ///
    /// # Errors
    /// Returns [`GridIndexError::CellOutOfBounds`] if either index is outside this grid.
    pub fn centre_of(&self, row: Row, col: Col) -> Result<LatLon, GridIndexError> {
        if row.0 >= self.height || col.0 >= self.width {
            return Err(GridIndexError::CellOutOfBounds {
                row: row.0,
                column: col.0,
                width: self.width,
                height: self.height,
            });
        }
        // u32 -> f64 is exact below 2^53, so this carries no rounding of its own.
        Ok(LatLon {
            lat: self.origin.lat + (f64::from(row.0) + 0.5) * self.lat_step,
            lon: self.origin.lon + (f64::from(col.0) + 0.5) * self.lon_step,
        })
    }

    /// The latitude every cell centre in a row shares.
    ///
    /// It takes no column for the reason [`Grid::cell_area_km2`] takes none, and it exists because a
    /// circular kernel is built from a row's latitude alone: without it such a caller has to name a
    /// column it has no use for, and a column it invented is a column it could invent wrongly.
    /// [`Grid::centre_of`] reads it rather than recomputing it, so a row has one latitude and not two
    /// that agree.
    ///
    /// # Errors
    /// Returns [`GridIndexError::RowOutOfBounds`] if `row` is outside this grid.
    pub fn centre_lat(&self, row: Row) -> Result<f64, GridIndexError> {
        if row.0 >= self.height {
            return Err(GridIndexError::RowOutOfBounds {
                row: row.0,
                height: self.height,
            });
        }
        Ok(self.origin.lat + (f64::from(row.0) + 0.5) * self.lat_step)
    }

    /// # Errors
    /// Returns [`GridIndexError::RowOutOfBounds`] if `row` is outside this grid.
    pub fn lat_bounds(&self, row: Row) -> Result<LatBand, GridIndexError> {
        if row.0 >= self.height {
            return Err(GridIndexError::RowOutOfBounds {
                row: row.0,
                height: self.height,
            });
        }
        Ok(self.lat_band_unchecked(row))
    }

    fn lat_band_unchecked(&self, row: Row) -> LatBand {
        let north = self.origin.lat + f64::from(row.0) * self.lat_step;
        // lat_step is negative by construction, so north is the larger of the two.
        LatBand::new(north, north + self.lat_step)
    }

    /// # Errors
    /// Returns [`GridIndexError::ColumnOutOfBounds`] if `col` is outside this grid.
    pub fn lon_bounds(&self, col: Col) -> Result<LonSpan, GridIndexError> {
        if col.0 >= self.width {
            return Err(GridIndexError::ColumnOutOfBounds {
                column: col.0,
                width: self.width,
            });
        }
        let a = self.origin.lon + f64::from(col.0) * self.lon_step;
        let b = a + self.lon_step;
        // lon_step's sign is not constrained, so order by value rather than by index.
        Ok(LonSpan {
            west: a.min(b),
            east: a.max(b),
        })
    }

    /// The column `cells` along from `col`, counting round the seam: through rising indices when
    /// `cells` is positive, falling when it is negative.
    ///
    /// Total rather than fallible, because the offset is reduced modulo the width and so always names a
    /// column this grid has. Whether that column also sits `cells` steps *east on the ground* turns on
    /// [`Grid::spans_full_turn`] and on `lon_step`'s sign, and this asks neither: the caller for which
    /// the ground matters is [`crate::kernel::Kernel`], which settles both before it places a span.
    ///
    /// # Errors
    /// Returns [`GridIndexError::ColumnOutOfBounds`] if `col` is outside this grid.
    pub fn col_along(&self, col: Col, cells: i64) -> Result<Col, GridIndexError> {
        if col.0 >= self.width {
            return Err(GridIndexError::ColumnOutOfBounds {
                column: col.0,
                width: self.width,
            });
        }
        let width = i64::from(self.width);
        // The offset is reduced before it is added rather than after, so both terms stay below 2^32 and
        // no offset a caller can pass overflows the sum.
        let index = (i64::from(col.0) + cells.rem_euclid(width)).rem_euclid(width);
        // rem_euclid by a positive modulus lands in [0, width), and the width came from a u32, so this
        // conversion is exact. std offers no fallible i64 -> u32, which is why it is a cast.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Ok(Col(index as u32))
    }

    /// The cell containing a coordinate, or `None` when it falls outside the grid.
    ///
    /// Longitude is reduced modulo a full turn first, so on a whole-globe grid every longitude
    /// lands in a column and the antimeridian is not a boundary. On a window grid a longitude
    /// outside the window has no column, and that is the `None`.
    #[must_use]
    pub fn cell_containing(&self, at: LatLon) -> Option<(Row, Col)> {
        if !at.lat.is_finite() || !at.lon.is_finite() {
            return None;
        }

        let row = Row(index_from_offset(
            (at.lat - self.origin.lat) / self.lat_step,
            self.height,
        )?);

        let cells_from_origin = (at.lon - self.origin.lon) / self.lon_step;
        // Reducing by the width rather than by 360/step is what makes "every longitude has a
        // column" exact on a closed grid: 360/step can exceed the width by a rounding sliver, and
        // longitudes falling in that sliver would otherwise have no column at all.
        let modulus = if self.spans_full_turn() {
            f64::from(self.width)
        } else {
            360.0 / self.lon_step.abs()
        };
        let col = Col(index_from_offset(
            cells_from_origin.rem_euclid(modulus),
            self.width,
        )?);

        Some((row, col))
    }

    /// The ground area of any cell in a row. Every cell in a row has the same area, so this takes no
    /// column.
    ///
    /// It reads the row's edges through [`Grid::lat_bounds`] rather than recomputing them, which is
    /// what stops the area and the coordinate of a row from ever disagreeing about where the row is.
    ///
    /// # Errors
    /// Returns [`GridIndexError::RowOutOfBounds`] if `row` is outside this grid.
    pub fn cell_area_km2(&self, row: Row) -> Result<f64, GridIndexError> {
        Ok(zone_area_km2(self.lat_bounds(row)?, self.lon_step.abs()))
    }

    pub(crate) fn middle_row_cell_area_km2(&self) -> f64 {
        zone_area_km2(
            self.lat_band_unchecked(self.middle_row()),
            self.lon_step.abs(),
        )
    }

    /// Whether the columns close on themselves, as a whole-globe raster's do.
    #[must_use]
    pub fn spans_full_turn(&self) -> bool {
        (f64::from(self.width) * self.lon_step.abs() - 360.0).abs() <= BOUNDARY_TOLERANCE_DEG
    }
}

/// `offset` is a position in cells from the origin along one axis; the index is the cell it falls
/// in, or `None` when that is off the end.
fn index_from_offset(offset: f64, limit: u32) -> Option<u32> {
    let floored = offset.floor();
    if !(0.0..f64::from(limit)).contains(&floored) {
        return None;
    }
    // The range check above proves floored is an integral f64 in [0, limit) and limit is a u32, so
    // this conversion is exact. std offers no fallible f64 -> u32, which is why it is a cast.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(floored as u32)
}

// expect is warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows it in tests. float_cmp is here because these
// assertions pin that the constructor stored its arguments verbatim — bit-exact equality is the
// property, not an approximation of one.
#[cfg(test)]
#[allow(clippy::expect_used, clippy::float_cmp)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::geodesy::EARTH_RADIUS_KM;

    const GPW_STEP: f64 = 1.0 / 120.0;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    // Minting a pair is three lines at every call site otherwise, and every use below wants a pair
    // the grid does contain.
    fn cell(grid: &Grid, row: u32, col: u32) -> (Row, Col) {
        (
            grid.row(row).expect("row is inside the grid"),
            grid.col(col).expect("column is inside the grid"),
        )
    }

    fn last_row(grid: &Grid) -> Row {
        grid.row(grid.height() - 1).expect("the grid has rows")
    }

    // The decimated grid application.md "Approach" calls for: same code path, coarse enough to
    // enumerate exhaustively in a test.
    fn decimated() -> Grid {
        Grid::new(
            360,
            180,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            1.0,
            -1.0,
        )
        .expect("a 1 degree whole-globe grid is valid")
    }

    fn gpw() -> Grid {
        Grid::new(
            43200,
            21600,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            GPW_STEP,
            -GPW_STEP,
        )
        .expect("the GPW registry row is a valid grid")
    }

    #[test]
    fn gpw_registry_row_constructs() {
        let grid = gpw();
        assert_eq!(grid.width(), 43200);
        assert_eq!(grid.height(), 21600);
        assert_eq!(grid.origin().lat, 90.0);
        assert_eq!(grid.origin().lon, -180.0);
        assert_eq!(grid.lat_step(), -GPW_STEP);
    }

    fn err(width: u32, height: u32, origin_lat: f64, lon_step: f64, lat_step: f64) -> GridError {
        Grid::new(
            width,
            height,
            LatLon {
                lat: origin_lat,
                lon: -180.0,
            },
            lon_step,
            lat_step,
        )
        .expect_err("expected this grid to be rejected")
    }

    #[test]
    fn rejects_empty_dimension() {
        assert_eq!(
            err(0, 21600, 90.0, GPW_STEP, -GPW_STEP),
            GridError::EmptyDimension {
                width: 0,
                height: 21600
            }
        );
        assert_eq!(
            err(43200, 0, 90.0, GPW_STEP, -GPW_STEP),
            GridError::EmptyDimension {
                width: 43200,
                height: 0
            }
        );
    }

    #[test]
    fn rejects_non_finite_origin() {
        assert!(matches!(
            err(43200, 21600, f64::NAN, GPW_STEP, -GPW_STEP),
            GridError::NonFiniteOrigin { .. }
        ));
        assert!(matches!(
            Grid::new(
                43200,
                21600,
                LatLon {
                    lat: 90.0,
                    lon: f64::INFINITY
                },
                GPW_STEP,
                -GPW_STEP
            ),
            Err(GridError::NonFiniteOrigin { .. })
        ));
    }

    #[test]
    fn rejects_origin_lat_out_of_range() {
        assert_eq!(
            err(43200, 21600, 90.5, GPW_STEP, -GPW_STEP),
            GridError::OriginLatOutOfRange { origin_lat: 90.5 }
        );
        assert_eq!(
            err(43200, 21600, -90.5, GPW_STEP, -GPW_STEP),
            GridError::OriginLatOutOfRange { origin_lat: -90.5 }
        );
    }

    #[test]
    fn rejects_non_finite_step() {
        assert!(matches!(
            err(43200, 21600, 90.0, f64::NAN, -GPW_STEP),
            GridError::NonFiniteStep { .. }
        ));
        assert!(matches!(
            err(43200, 21600, 90.0, GPW_STEP, f64::NEG_INFINITY),
            GridError::NonFiniteStep { .. }
        ));
    }

    #[test]
    fn rejects_zero_step() {
        assert!(matches!(
            err(43200, 21600, 90.0, 0.0, -GPW_STEP),
            GridError::ZeroStep { .. }
        ));
        assert!(matches!(
            err(43200, 21600, 90.0, GPW_STEP, 0.0),
            GridError::ZeroStep { .. }
        ));
    }

    #[test]
    fn rejects_northward_lat_step() {
        assert_eq!(
            err(43200, 21600, -90.0, GPW_STEP, GPW_STEP),
            GridError::LatStepNotSouthward { lat_step: GPW_STEP }
        );
    }

    #[test]
    fn rejects_rows_past_the_south_pole() {
        // One row too many at the GPW step overruns by a whole cell, well past the rounding the
        // tolerance admits.
        assert!(matches!(
            err(43200, 21601, 90.0, GPW_STEP, -GPW_STEP),
            GridError::RunsPastSouthPole { .. }
        ));
    }

    #[test]
    fn rejects_columns_past_a_full_turn() {
        // The failure this rules out is not an off-globe coordinate but a lost bijection: at 43201
        // columns the last one repeats the first meridian, and centre_of would hand back a longitude
        // that cell_containing resolves to a different column.
        assert!(matches!(
            err(43201, 21600, 90.0, GPW_STEP, -GPW_STEP),
            GridError::RunsPastAFullTurn { .. }
        ));
        // A negative lon_step spans the same turn westward, so the bound is on the magnitude.
        assert!(matches!(
            err(43201, 21600, 90.0, -GPW_STEP, -GPW_STEP),
            GridError::RunsPastAFullTurn { .. }
        ));
    }

    #[test]
    fn a_step_a_hair_too_large_still_reaches_the_pole() {
        // What BOUNDARY_TOLERANCE_DEG is for, on both axes at once: this grid ends at -90 and closes
        // on itself in intent, but computes a south edge just past the pole and a span just past a
        // turn. Rejecting it would fail a real raster on a rounding artefact.
        let step = f64::from_bits((GPW_STEP).to_bits() + 1);
        assert!(90.0 + f64::from(21600u32) * -step < -90.0);
        Grid::new(
            43200,
            21600,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            step,
            -step,
        )
        .expect("a step one ulp large is rounding, not an overrun");
    }

    #[test]
    fn a_partial_grid_north_of_the_pole_constructs() {
        // A country mask is a window on the globe, not the whole globe: rows stopping short of -90
        // are the normal case, not an edge case.
        Grid::new(
            1200,
            600,
            LatLon {
                lat: 60.0,
                lon: -10.0,
            },
            GPW_STEP,
            -GPW_STEP,
        )
        .expect("a window grid inside the globe is valid");
    }

    #[test]
    fn an_origin_off_the_seam_is_reduced_at_construction() {
        for lon in [180.0, 540.0, -540.0] {
            let grid = Grid::new(360, 180, LatLon { lat: 90.0, lon }, 1.0, -1.0)
                .expect("a whole-globe grid is valid at any origin longitude");
            assert_eq!(grid.origin().lon, -180.0, "origin lon {lon}");
        }
        // A window keeps the longitude it was given when that is already canonical, so reducing the
        // origin does not move a mask off the region it covers.
        let window = Grid::new(
            600,
            600,
            LatLon {
                lat: 60.0,
                lon: 170.0,
            },
            GPW_STEP,
            -GPW_STEP,
        )
        .expect("a window grid is valid");
        assert_eq!(window.origin().lon, 170.0);
    }

    #[test]
    fn an_index_off_the_end_is_never_minted() {
        // The accessors have no bound of their own, so this is the only thing standing between a
        // caller's arithmetic and a latitude off the globe or a negative cell area.
        let grid = decimated();
        assert!(grid.row(179).is_some() && grid.col(359).is_some());
        for off_the_end in [180, 181, u32::MAX] {
            assert_eq!(grid.row(off_the_end), None);
        }
        for off_the_end in [360, 361, u32::MAX] {
            assert_eq!(grid.col(off_the_end), None);
        }
    }

    // Row and column types carry no grid identity, so each mismatch reports the receiving grid's error.
    #[test]
    fn a_finer_grids_row_returns_row_errors_for_row_operations() {
        let (row, _) = cell(&gpw(), 21599, 0);
        let grid = decimated();
        let error = GridIndexError::RowOutOfBounds {
            row: 21599,
            height: 180,
        };
        assert_eq!(grid.centre_lat(row), Err(error));
        assert_eq!(grid.lat_bounds(row), Err(error));
        assert_eq!(grid.cell_area_km2(row), Err(error));
    }

    #[test]
    fn a_finer_grids_column_returns_column_errors_for_column_operations() {
        let (_, col) = cell(&gpw(), 0, 43199);
        let grid = decimated();
        let error = GridIndexError::ColumnOutOfBounds {
            column: 43199,
            width: 360,
        };
        assert_eq!(grid.lon_bounds(col), Err(error));
        assert_eq!(grid.col_along(col, 1), Err(error));
    }

    #[test]
    fn a_finer_grids_cell_returns_cell_error() {
        let (row, col) = cell(&gpw(), 21599, 43199);
        assert_eq!(
            decimated().centre_of(row, col),
            Err(GridIndexError::CellOutOfBounds {
                row: 21599,
                column: 43199,
                width: 360,
                height: 180,
            })
        );
    }

    #[test]
    fn stepping_along_the_columns_wraps_at_the_seam() {
        let grid = decimated();
        let (_, first) = cell(&grid, 0, 0);
        let (_, last) = cell(&grid, 0, grid.width() - 1);

        // No step is no move, and whole turns are the same: an offset is a position on a ring.
        for col in grid.cols() {
            assert_eq!(
                grid.col_along(col, 0).expect("indices belong to the grid"),
                col
            );
            for turns in [-2i64, -1, 1, 2] {
                assert_eq!(
                    grid.col_along(col, turns * i64::from(grid.width()))
                        .expect("indices belong to the grid"),
                    col
                );
            }
        }

        // The seam in both directions, which is the case an index clamped instead of wrapped would get
        // wrong: there is no column before the first or after the last, only the other end.
        assert_eq!(
            grid.col_along(first, -1)
                .expect("indices belong to the grid"),
            last
        );
        assert_eq!(
            grid.col_along(last, 1).expect("indices belong to the grid"),
            first
        );
        assert_eq!(
            grid.col_along(first, -359)
                .expect("indices belong to the grid"),
            grid.col(1).expect("a column")
        );

        // An offset far past a turn is still a column, so a caller needs no reduction of its own.
        assert_eq!(
            grid.col_along(first, 3 * 360 + 5)
                .expect("indices belong to the grid"),
            grid.col(5).expect("a column")
        );
        // The extreme the reduction order exists for: reducing after the addition would overflow here,
        // and i64::MIN is 352 columns along a 360-column ring.
        assert_eq!(
            grid.col_along(first, i64::MIN)
                .expect("indices belong to the grid"),
            grid.col(352).expect("a column")
        );
    }

    #[test]
    fn a_finer_grids_column_cannot_be_stepped_along_this_one() {
        let (_, col) = cell(&gpw(), 0, 43199);
        assert_eq!(
            decimated().col_along(col, 1),
            Err(GridIndexError::ColumnOutOfBounds {
                column: 43199,
                width: 360,
            })
        );
    }

    #[test]
    fn the_iterators_cover_the_grid_exactly_once_in_order() {
        let grid = decimated();
        let rows: Vec<u32> = grid.rows().map(Row::get).collect();
        let cols: Vec<u32> = grid.cols().map(Col::get).collect();
        assert_eq!(rows, (0..grid.height()).collect::<Vec<u32>>());
        assert_eq!(cols, (0..grid.width()).collect::<Vec<u32>>());
    }

    #[test]
    fn the_middle_row_is_a_row_of_every_grid() {
        // Including the one-row grid, which is where a mint returning no Option has to be right:
        // there the middle is the only row there is.
        for grid in [
            decimated(),
            gpw(),
            Grid::new(
                1,
                1,
                LatLon {
                    lat: 90.0,
                    lon: -180.0,
                },
                360.0,
                -180.0,
            )
            .expect("a single-cell whole-globe grid is valid"),
        ] {
            let middle = grid.middle_row();
            assert_eq!(grid.row(middle.get()), Some(middle));
        }
    }

    #[test]
    fn centres_sit_half_a_cell_inside_each_corner() {
        let grid = gpw();
        let half = GPW_STEP / 2.0;

        let (row, col) = cell(&grid, 0, 0);
        let nw = grid
            .centre_of(row, col)
            .expect("indices belong to the grid");
        assert!(close(nw.lat, 90.0 - half) && close(nw.lon, -180.0 + half));

        let (row, col) = cell(&grid, 0, 43199);
        let ne = grid
            .centre_of(row, col)
            .expect("indices belong to the grid");
        assert!(close(ne.lat, 90.0 - half) && close(ne.lon, 180.0 - half));

        let (row, col) = cell(&grid, 21599, 0);
        let sw = grid
            .centre_of(row, col)
            .expect("indices belong to the grid");
        assert!(close(sw.lat, -90.0 + half) && close(sw.lon, -180.0 + half));

        let (row, col) = cell(&grid, 21599, 43199);
        let se = grid
            .centre_of(row, col)
            .expect("indices belong to the grid");
        assert!(close(se.lat, -90.0 + half) && close(se.lon, 180.0 - half));
    }

    #[test]
    fn a_rows_latitude_is_the_one_every_cell_in_it_has() {
        // Bit for bit, over every cell of the grid: what this pins is that the row latitude has one
        // definition rather than two that agree to within a rounding, which is the whole reason
        // centre_of reads centre_lat instead of repeating the expression.
        let grid = decimated();
        for row in grid.rows() {
            let lat = grid.centre_lat(row).expect("indices belong to the grid");
            for col in grid.cols() {
                assert_eq!(
                    grid.centre_of(row, col)
                        .expect("indices belong to the grid")
                        .lat,
                    lat,
                    "row {}",
                    row.get()
                );
            }
        }
    }

    #[test]
    fn polar_rows_stay_on_the_globe() {
        let grid = gpw();
        for index in [0, grid.height() - 1] {
            let (row, col) = cell(&grid, index, 0);
            let centre = grid
                .centre_of(row, col)
                .expect("indices belong to the grid");
            assert!(centre.lat.abs() < 90.0, "row {index} centre left the globe");
            assert_eq!(grid.cell_containing(centre), Some((row, col)));
        }
    }

    #[test]
    fn the_antimeridian_is_not_a_boundary() {
        let grid = gpw();
        let east = cell(&grid, 0, grid.width() - 1);
        let west = cell(&grid, 0, 0);

        // The two columns either side of the seam are the first and the last, and a longitude in
        // each lands in it — the seam is where the index wraps, not where the grid stops.
        assert_eq!(
            grid.cell_containing(
                grid.centre_of(east.0, east.1)
                    .expect("indices belong to the grid")
            ),
            Some(east)
        );
        assert_eq!(
            grid.cell_containing(
                grid.centre_of(west.0, west.1)
                    .expect("indices belong to the grid")
            ),
            Some(west)
        );

        // -180 and +180 are the same meridian, so both land in a column, and +180 wraps to the
        // first rather than falling off the last.
        let equator = cell(&grid, 10800, 0);
        assert_eq!(
            grid.cell_containing(LatLon {
                lat: 0.0,
                lon: -180.0
            }),
            Some(equator)
        );
        assert_eq!(
            grid.cell_containing(LatLon {
                lat: 0.0,
                lon: 180.0
            }),
            Some(equator)
        );
    }

    #[test]
    fn bounds_bracket_the_centre() {
        let grid = gpw();
        for (row, col) in
            [(0u32, 0u32), (10800, 21600), (21599, 43199)].map(|(row, col)| cell(&grid, row, col))
        {
            let centre = grid
                .centre_of(row, col)
                .expect("indices belong to the grid");
            let band = grid.lat_bounds(row).expect("indices belong to the grid");
            let span = grid.lon_bounds(col).expect("indices belong to the grid");
            assert!(band.north() > centre.lat && centre.lat > band.south());
            assert!(span.west < centre.lon && centre.lon < span.east);
            assert!(close(band.north() - band.south(), GPW_STEP));
            assert!(close(span.east - span.west, GPW_STEP));
        }
    }

    #[test]
    fn adjacent_rows_share_an_edge() {
        // The bands tile the globe with no gap and no overlap, which is what lets 3.1's sum
        // telescope.
        let grid = decimated();
        for (row, next) in grid.rows().zip(grid.rows().skip(1)) {
            assert!(close(
                grid.lat_bounds(row)
                    .expect("indices belong to the grid")
                    .south(),
                grid.lat_bounds(next)
                    .expect("indices belong to the grid")
                    .north()
            ));
        }
        let (first, _) = cell(&grid, 0, 0);
        assert!(close(
            grid.lat_bounds(first)
                .expect("indices belong to the grid")
                .north(),
            90.0
        ));
        assert!(close(
            grid.lat_bounds(last_row(&grid))
                .expect("the last row belongs to the grid")
                .south(),
            -90.0
        ));
    }

    #[test]
    fn every_grid_row_has_an_ordered_band_with_positive_area() {
        let grid = decimated();
        for row in grid.rows() {
            let band = grid.lat_bounds(row).expect("indices belong to the grid");
            assert!(band.north() > band.south());
            assert!(zone_area_km2(band, grid.lon_step().abs()) > 0.0);
        }
    }

    #[test]
    fn a_latitude_off_the_grid_has_no_cell() {
        let grid = gpw();
        for lat in [90.5, -90.5, f64::NAN, f64::INFINITY] {
            assert_eq!(grid.cell_containing(LatLon { lat, lon: 0.0 }), None);
        }
    }

    #[test]
    fn a_latitude_past_a_pole_is_rejected_not_folded() {
        // The failure this rules out: 91 folding to 89 and quietly answering with a cell on the
        // wrong side of the pole. Rejection is the whole point, so assert it is not the fold.
        let grid = gpw();
        let folded = grid.cell_containing(LatLon {
            lat: 89.0,
            lon: 0.0,
        });
        assert!(folded.is_some());
        assert_eq!(
            grid.cell_containing(LatLon {
                lat: 91.0,
                lon: 0.0
            }),
            None
        );
        assert_eq!(
            grid.cell_containing(LatLon {
                lat: -91.0,
                lon: 0.0
            }),
            None
        );
    }

    #[test]
    fn a_grid_whose_origin_is_a_whole_turn_away_is_the_same_grid() {
        let base = gpw();
        for k in -3..=3 {
            let shifted = Grid::new(
                43200,
                21600,
                LatLon {
                    lat: 90.0,
                    lon: -180.0 + 360.0 * f64::from(k),
                },
                GPW_STEP,
                -GPW_STEP,
            )
            .expect("shifting the origin by whole turns keeps the grid valid");
            // Same grid, not merely a grid that agrees once wrapped: the constructor reduces the
            // origin, so there is no offset left for a coordinate to carry.
            assert_eq!(shifted, base, "k={k}");
            for (row, col) in [(0u32, 0u32), (10800, 21600), (21599, 43199)]
                .map(|(row, col)| cell(&shifted, row, col))
            {
                let here = shifted
                    .centre_of(row, col)
                    .expect("indices belong to the grid");
                let there = base
                    .centre_of(row, col)
                    .expect("indices belong to the grid");
                assert!(
                    here == there,
                    "k={k} ({}, {}): {here:?} against {there:?}",
                    row.get(),
                    col.get()
                );
            }
        }
    }

    // Nothing constrains lon_step's sign, so an east-to-west raster is a grid this code accepts and
    // no test exercised. These pin that the sign changes which meridian a column sits on and nothing
    // else: the seam still wraps, the span stays ordered west before east, and area is unsigned.
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

    #[test]
    fn a_westward_grid_still_inverts_its_own_centres() {
        let grid = westward();
        assert!(grid.spans_full_turn());
        for index in [0, 1, 180, 359] {
            let (row, col) = cell(&grid, 0, index);
            assert_eq!(
                grid.cell_containing(
                    grid.centre_of(row, col)
                        .expect("indices belong to the grid")
                ),
                Some((row, col))
            );
        }
    }

    #[test]
    fn a_westward_grids_columns_run_the_other_way() {
        let grid = westward();
        let (row, first) = cell(&grid, 0, 0);
        let (_, second) = cell(&grid, 0, 1);
        // Column 0 sits east of column 1, which is the whole of what the sign changes.
        assert!(
            grid.centre_of(row, first)
                .expect("indices belong to the grid")
                .lon
                > grid
                    .centre_of(row, second)
                    .expect("indices belong to the grid")
                    .lon
        );
        // Ordered by value regardless, so a span is never inverted.
        for col in grid.cols() {
            let span = grid.lon_bounds(col).expect("indices belong to the grid");
            assert!(span.west < span.east);
            assert!(close(span.east - span.west, 1.0));
        }
    }

    #[test]
    fn a_westward_grid_has_the_same_cell_areas() {
        let westward = westward();
        let eastward = decimated();
        for (west, east) in westward.rows().zip(eastward.rows()) {
            assert!(close(
                westward
                    .cell_area_km2(west)
                    .expect("indices belong to the grid"),
                eastward
                    .cell_area_km2(east)
                    .expect("indices belong to the grid")
            ));
        }
    }

    #[test]
    fn a_longitude_outside_a_window_grid_has_no_cell() {
        // The counterpart of "every longitude has a column": that holds because the globe closes,
        // and a window does not close, so its outside is a real None.
        let window = Grid::new(
            600,
            600,
            LatLon {
                lat: 60.0,
                lon: -10.0,
            },
            GPW_STEP,
            -GPW_STEP,
        )
        .expect("a window grid is valid");
        assert!(!window.spans_full_turn());
        assert_eq!(
            window.cell_containing(LatLon {
                lat: 57.0,
                lon: 120.0
            }),
            None
        );
        assert!(
            window
                .cell_containing(LatLon {
                    lat: 57.0,
                    lon: -7.0
                })
                .is_some()
        );
    }

    #[test]
    fn whole_globe_cell_areas_sum_to_the_sphere() {
        // The zone formula telescopes: summing a whole globe leaves sin(90) - sin(-90) = 2, so this
        // is an identity rather than an approximation, and a loose tolerance here would hide a real
        // disagreement rather than absorb rounding. Naive summation over 21600 rows costs about
        // 5e-14 relative, four orders inside the bound.
        let exact = 4.0 * std::f64::consts::PI * EARTH_RADIUS_KM * EARTH_RADIUS_KM;
        for grid in [gpw(), decimated()] {
            let total: f64 = grid
                .rows()
                .map(|row| {
                    f64::from(grid.width())
                        * grid.cell_area_km2(row).expect("indices belong to the grid")
                })
                .sum();
            let relative = (total - exact).abs() / exact;
            assert!(
                relative < 1e-9,
                "{} x {}: {total} km2 against 4piR^2 = {exact} km2, off by {relative}",
                grid.width(),
                grid.height()
            );
        }
    }

    #[test]
    fn cell_areas_are_positive_and_symmetric_about_the_equator() {
        let grid = decimated();
        let last = grid.height() - 1;
        for row in grid.rows() {
            let area = grid.cell_area_km2(row).expect("indices belong to the grid");
            assert!(area > 0.0, "row {} has area {area}", row.get());
            let (mirror, _) = cell(&grid, last - row.get(), 0);
            let mirrored = grid
                .cell_area_km2(mirror)
                .expect("indices belong to the grid");
            assert!(
                (area - mirrored).abs() / area < 1e-12,
                "row {} and its mirror disagree: {area} against {mirrored}",
                row.get()
            );
        }
    }

    #[test]
    fn cell_areas_grow_towards_the_equator() {
        // A cell spans a fixed angle, so its ground width shrinks with the cosine of latitude. The
        // largest cells are the two straddling the equator, and nothing on the way there dips.
        for grid in [gpw(), decimated()] {
            let middle = grid.height() / 2;
            for index in 1..middle {
                let (row, _) = cell(&grid, index, 0);
                let (previous_row, _) = cell(&grid, index - 1, 0);
                let previous = grid
                    .cell_area_km2(previous_row)
                    .expect("indices belong to the grid");
                let current = grid.cell_area_km2(row).expect("indices belong to the grid");
                assert!(
                    current > previous,
                    "row {index} ({current}) is not larger than row {} ({previous})",
                    index - 1
                );
            }
            let largest = grid
                .rows()
                .map(|row| grid.cell_area_km2(row).expect("indices belong to the grid"))
                .fold(f64::NEG_INFINITY, f64::max);
            let (middle_row, _) = cell(&grid, middle, 0);
            assert!(
                (grid
                    .cell_area_km2(middle_row)
                    .expect("indices belong to the grid")
                    - largest)
                    .abs()
                    / largest
                    < 1e-12
            );
        }
    }

    proptest! {
        #[test]
        fn centre_round_trips_on_the_full_grid(row in 0u32..21600, col in 0u32..43200) {
            let grid = gpw();
            let at = cell(&grid, row, col);
            prop_assert_eq!(grid.cell_containing(grid.centre_of(at.0, at.1).expect("indices belong to the grid")), Some(at));
        }

        #[test]
        fn centre_round_trips_on_the_decimated_grid(row in 0u32..180, col in 0u32..360) {
            let grid = decimated();
            let at = cell(&grid, row, col);
            prop_assert_eq!(grid.cell_containing(grid.centre_of(at.0, at.1).expect("indices belong to the grid")), Some(at));
        }

        #[test]
        fn a_whole_turn_of_longitude_finds_the_same_cell(
            row in 0u32..21600,
            col in 0u32..43200,
            k in -3i32..=3,
        ) {
            let grid = gpw();
            let at = cell(&grid, row, col);
            let centre = grid.centre_of(at.0, at.1).expect("indices belong to the grid");
            let shifted = LatLon { lat: centre.lat, lon: centre.lon + 360.0 * f64::from(k) };
            prop_assert_eq!(grid.cell_containing(shifted), Some(at));
        }

        #[test]
        fn every_longitude_has_a_column(lon in -3600.0f64..3600.0) {
            let grid = gpw();
            // Bound out of the assert: prop_assert! stringifies its expression into a format
            // string, so a struct literal's braces break the macro.
            let at = LatLon { lat: 0.0, lon };
            prop_assert!(grid.cell_containing(at).is_some());
        }
    }
}
