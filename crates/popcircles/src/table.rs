// The computing half of the summation table. The file, the header and
// everything that serialises are kept in `table/cache.rs`, so nothing here can be reached without a grid and a
// slice.

pub mod cache;

use crate::bracket::Bracket;
use crate::domain::FeatureId;
use crate::geodesy::LatLon;
use crate::grid::{BOUNDARY_TOLERANCE_DEG, Col, Grid, GridError, Row};
use crate::ownership::{CellOwner, CellOwnership, OWNERSHIP_RULE_VERSION, OwnershipError};
use crate::progress::Progress;
use crate::raster::{CellTallies, RasterError, RasterSource};

#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum TableError {
    #[error("a padded table over this grid holds {expected} cells; the payload holds {found}")]
    PayloadLength { expected: usize, found: usize },

    #[error(
        "a decimation factor must divide both grid dimensions; {factor} does not divide {width} x {height}"
    )]
    Decimation {
        factor: u32,
        width: u32,
        height: u32,
    },

    #[error("the decimated grid is not a grid")]
    DecimatedGrid(#[source] GridError),
}

/// Why a build stopped. The sink's own failure stays its own type, so this module needs no vocabulary
/// for whatever a caller is emitting rows into.
#[derive(Debug, thiserror::Error)]
pub enum BuildError<E> {
    #[error("the raster could not be read")]
    Raster(#[from] RasterError),

    #[error("a completed table row could not be taken")]
    Sink(#[source] E),
}

/// A band of rows, inclusive at both ends.
///
/// Inclusive because a [`Row`] is only ever minted by the grid that contains it, so a half-open band
/// would need a southern end no grid will mint. Any pair of rows is a band, so the constructor orders
/// them rather than refusing one order: there is nothing here for a caller to get wrong and nothing
/// downstream to re-check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowBand {
    north: Row,
    south: Row,
}

impl RowBand {
    #[must_use]
    pub fn new(a: Row, b: Row) -> Self {
        Self {
            north: a.min(b),
            south: a.max(b),
        }
    }

    #[must_use]
    pub const fn north(self) -> Row {
        self.north
    }

    #[must_use]
    pub const fn south(self) -> Row {
        self.south
    }
}

/// The columns a query covers.
///
/// The full turn is a **variant**, not a `west == east` pair: on a grid whose columns close on
/// themselves, a span given as two indices cannot say whether it means one column or all of them, and
/// the answer a caller wants is the one it did not encode. Splitting the wrapped case into two
/// rectangles is then this module's business rather than every caller's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColSpan {
    /// `west` through `east` inclusive, running east past the last column and on from the first when
    /// `west > east`.
    Through { west: Col, east: Col },
    /// Every column of the grid, exactly once.
    FullTurn,
}

/// A latitude and longitude window a caller wants the population of.
///
/// Four degrees rather than two [`LatLon`] corners, because the **span** `east - west` is what decides
/// which [`ColSpan`] a window means and a pair of corners cannot say: −180 and 180 reduce to the same
/// column, so a window given as corners cannot tell one column from the whole turn. A span of a full
/// turn or more is [`ColSpan::FullTurn`]; a negative span wraps the antimeridian and is ordinary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    pub north: f64,
    pub south: f64,
    pub west: f64,
    pub east: f64,
}

/// A summed-area table over a grid, borrowing its payload.
///
/// The payload is padded to `(height + 1) x (width + 1)` with a zero first row and column, so element `(r, c)` holds the population of every cell strictly north of grid row `r`
/// and strictly west of grid column `c`, and a rectangle touching the north or west edge needs no
/// branch of its own.
///
/// It borrows a slice instead of taking a storage generic, which is the same decision: a `Vec` and a
/// mapping are both a slice by the time a query sees one, and the checked cast belongs where it can
/// fail rather than behind an accessor.
#[derive(Debug, Clone, Copy)]
pub struct Table<'a> {
    grid: Grid,
    cells: &'a [f64],
}

impl<'a> Table<'a> {
    /// # Errors
    /// [`TableError::PayloadLength`] when `cells` is not the padded product of the grid's dimensions.
    /// That is the whole of what a slice can be wrong about here, and checking it once is what lets
    /// every query below index without a bound of its own.
    pub fn new(grid: Grid, cells: &'a [f64]) -> Result<Self, TableError> {
        let expected = padded_len(&grid);
        if cells.len() != expected {
            return Err(TableError::PayloadLength {
                expected,
                found: cells.len(),
            });
        }
        Ok(Self { grid, cells })
    }

    #[must_use]
    pub const fn grid(&self) -> &Grid {
        &self.grid
    }

    /// The population of a rectangle: four lookups, and two of those for a span that wraps.
    ///
    /// # Panics
    /// If an index was minted by a larger grid; [`Row`] says why that is a stop.
    #[must_use]
    pub fn population(&self, rows: RowBand, cols: ColSpan) -> f64 {
        let width = self.grid.width();
        // The band is ordered, so its southern end is the only one that can run off the grid.
        assert!(
            rows.south.get() < self.grid.height(),
            "row {} is not a row of a {}-row table",
            rows.south.get(),
            self.grid.height()
        );

        match cols {
            // One rectangle over every column, never two pieces meeting at a seam. The full turn is
            // the case a split double-counts, and here there is no split to get wrong.
            ColSpan::FullTurn => self.rectangle(rows, 0, width),
            ColSpan::Through { west, east } => {
                assert!(
                    west.get() < width && east.get() < width,
                    "({}, {}) are not columns of a {}-column table",
                    west.get(),
                    east.get(),
                    width
                );
                if west.get() <= east.get() {
                    self.rectangle(rows, west.get(), east.get() + 1)
                } else {
                    self.rectangle(rows, west.get(), width)
                        + self.rectangle(rows, 0, east.get() + 1)
                }
            }
        }
    }

    /// Every row and every column, which is the table's whole extent.
    ///
    /// Not a [`Window`] of 90 to −90: the grid's outer southern boundary lies in no cell, by
    /// [`Grid::cell_containing`]'s convention, so the whole-globe question is this rather than a window
    /// a caller has to nudge off the pole.
    #[must_use]
    pub fn whole(&self) -> (RowBand, ColSpan) {
        // Seeded with the middle row, the one row [`Grid`] mints without an `Option`, and widened by
        // every row the grid hands out. That is what makes the extent an expression over the grid's own
        // mints rather than a pair of indices this module asserts are rows.
        let middle = self.grid.middle_row();
        let rows = self
            .grid
            .rows()
            .fold(RowBand::new(middle, middle), |band, row| {
                RowBand::new(band.north().min(row), band.south().max(row))
            });
        (rows, ColSpan::FullTurn)
    }

    /// The rows and columns a [`Window`] covers, or `None` when a corner falls outside the grid.
    ///
    /// Here rather than in a caller because it is coordinate arithmetic and one branch — the full turn —
    /// and a caller that assembled a [`ColSpan`] itself from a pair of longitudes would get that branch
    /// wrong at −180 to 180, which is the case a whole-globe question is always asked in.
    ///
    /// A coordinate on a cell's boundary belongs to the cell south or east of it, and the grid's own
    /// outer southern and eastern boundaries belong to no cell at all — both are
    /// [`Grid::cell_containing`]'s convention rather than this method's, and [`whole`](Self::whole) is
    /// what a caller wanting the extent asks for instead.
    #[must_use]
    pub fn covering(&self, window: Window) -> Option<(RowBand, ColSpan)> {
        // A latitude alone has no cell, so pair each with a longitude every grid has: its own origin's.
        let at = |lat: f64, lon: f64| self.grid.cell_containing(LatLon { lat, lon });
        let west_edge = self.grid.origin().lon;
        let (north, _) = at(window.north, west_edge)?;
        let (south, _) = at(window.south, west_edge)?;
        let rows = RowBand::new(north, south);

        if window.east - window.west >= 360.0 - BOUNDARY_TOLERANCE_DEG {
            return Some((rows, ColSpan::FullTurn));
        }
        let (_, west) = at(window.north, window.west)?;
        let (_, east) = at(window.north, window.east)?;
        Some((rows, ColSpan::Through { west, east }))
    }

    /// `first` and `last` are grid columns, half-open, and both are already padded indices.
    fn rectangle(&self, rows: RowBand, first: u32, last: u32) -> f64 {
        let north = rows.north.get();
        let south = rows.south.get() + 1;
        self.corner(south, last) - self.corner(north, last) - self.corner(south, first)
            + self.corner(north, first)
    }

    fn corner(&self, row: u32, col: u32) -> f64 {
        let stride = self.grid.width() as usize + 1;
        self.cells[row as usize * stride + col as usize]
    }
}

/// Saturating rather than wrapping, so a grid too large for this host's addresses is reported as a
/// payload of the wrong length instead of as a smaller shape some payload happens to fit.
fn padded_len(grid: &Grid) -> usize {
    let cells = (u64::from(grid.width()) + 1) * (u64::from(grid.height()) + 1);
    usize::try_from(cells).unwrap_or(usize::MAX)
}

/// How coarse a table is, and the grid that makes it: a factor of k folds every k by k block of source
/// cells into one table cell.
///
/// It is built against the grid it will be applied to, because the factor's whole constraint is a
/// relation to that grid — k has to divide both dimensions, so that no block is partial and the coarser
/// grid covers the same ground. Refusing it here is what keeps a half-filled block from being a case
/// the build, the query and the cache each need an answer for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decimation {
    factor: u32,
    source: Grid,
    grid: Grid,
}

impl Decimation {
    /// # Errors
    /// [`TableError::Decimation`] when `factor` does not divide both of the grid's dimensions, zero
    /// included; [`TableError::DecimatedGrid`] when the coarser grid is not one this crate accepts.
    pub fn new(source: Grid, factor: u32) -> Result<Self, TableError> {
        // A factor of zero needs no case of its own: the grid's dimensions are never zero, and no
        // non-zero number is a multiple of zero, so the rule below refuses it without dividing by it.
        if !source.width().is_multiple_of(factor) || !source.height().is_multiple_of(factor) {
            return Err(TableError::Decimation {
                factor,
                width: source.width(),
                height: source.height(),
            });
        }

        let grid = Grid::new(
            source.width() / factor,
            source.height() / factor,
            source.origin(),
            source.lon_step() * f64::from(factor),
            source.lat_step() * f64::from(factor),
        )
        .map_err(TableError::DecimatedGrid)?;

        Ok(Self {
            factor,
            source,
            grid,
        })
    }

    /// One table cell per source cell. Total rather than fallible, because 1 divides everything and the
    /// coarser grid is the source's own.
    #[must_use]
    pub const fn none(source: Grid) -> Self {
        Self {
            factor: 1,
            source,
            grid: source,
        }
    }

    #[must_use]
    pub const fn factor(&self) -> u32 {
        self.factor
    }

    /// The grid the table is over — the source's own when the factor is one.
    #[must_use]
    pub const fn grid(&self) -> &Grid {
        &self.grid
    }

    #[must_use]
    pub const fn source(&self) -> &Grid {
        &self.source
    }
}

/// What a build produced beside the rows it emitted, and everything a cache header needs of them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuiltTable {
    /// The identity of the cells this table was built from — ADR 0005, and the only thing
    /// that says two tables are the same table. Of the source's cells, not the table's: a decimated
    /// table and a full one over the same raster carry the same digest and differ by the factor beside
    /// it, so a mismatch in either is reported as itself.
    pub digest: u64,
    pub tallies: CellTallies,
    /// The whole raster's population: the table's last cell, so it carries the compensation the rest
    /// of the table carries rather than being summed a second way.
    pub total: f64,
    /// What the rows that were emitted are over, so a cache header needs nothing the build did not
    /// already settle.
    pub decimation: Decimation,
}

/// A prefix table containing only the cells uniquely owned by one feature.
#[derive(Debug, Clone, PartialEq)]
pub struct CountryTable {
    source_grid: Grid,
    effective_grid: Grid,
    grid: Grid,
    north_row: u32,
    west_col: u32,
    total: f64,
    unassigned: f64,
    identity: CountryTableIdentity,
    cells: Vec<f64>,
}

/// Inputs that must match before a derived country table can be reused.
#[derive(Debug, Clone, PartialEq)]
pub struct CountryTableIdentity {
    feature_id: FeatureId,
    source_digest: u64,
    boundary_source_digest: String,
    ownership_digest: u64,
    source_grid: Grid,
    effective_grid: Grid,
    decimation: u32,
    ownership_rule_version: u32,
    aggregation_order_version: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedCountryTableIdentity {
    pub feature_id: FeatureId,
    pub source_digest: u64,
    pub boundary_source_digest: String,
    pub ownership_digest: u64,
    pub source_grid: Grid,
    pub effective_grid: Grid,
    pub decimation: u32,
    pub ownership_rule_version: u32,
    pub aggregation_order_version: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedCountryTable {
    pub source_grid: Grid,
    pub effective_grid: Grid,
    pub grid: Grid,
    pub north_row: u32,
    pub west_col: u32,
    pub total: f64,
    pub unassigned: f64,
    pub identity: CountryTableIdentity,
    pub cells: Vec<f64>,
}

impl CountryTableIdentity {
    #[must_use]
    pub const fn feature_id(&self) -> &FeatureId {
        &self.feature_id
    }

    #[must_use]
    pub const fn source_digest(&self) -> u64 {
        self.source_digest
    }

    #[must_use]
    pub fn boundary_source_digest(&self) -> &str {
        &self.boundary_source_digest
    }

    #[must_use]
    pub const fn ownership_digest(&self) -> u64 {
        self.ownership_digest
    }

    #[must_use]
    pub const fn source_grid(&self) -> &Grid {
        &self.source_grid
    }

    #[must_use]
    pub const fn effective_grid(&self) -> &Grid {
        &self.effective_grid
    }

    #[must_use]
    pub const fn decimation(&self) -> u32 {
        self.decimation
    }

    #[must_use]
    pub const fn ownership_rule_version(&self) -> u32 {
        self.ownership_rule_version
    }

    #[must_use]
    pub const fn aggregation_order_version(&self) -> u32 {
        self.aggregation_order_version
    }
}

impl From<PreparedCountryTableIdentity> for CountryTableIdentity {
    fn from(parts: PreparedCountryTableIdentity) -> Self {
        Self {
            feature_id: parts.feature_id,
            source_digest: parts.source_digest,
            boundary_source_digest: parts.boundary_source_digest,
            ownership_digest: parts.ownership_digest,
            source_grid: parts.source_grid,
            effective_grid: parts.effective_grid,
            decimation: parts.decimation,
            ownership_rule_version: parts.ownership_rule_version,
            aggregation_order_version: parts.aggregation_order_version,
        }
    }
}

/// The row-major source-cell order used to accumulate each effective cell.
pub const COUNTRY_AGGREGATION_ORDER_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the derived country table identity does not match the requested source or rules")]
pub struct CountryTableIdentityError;

#[derive(Debug, thiserror::Error)]
pub enum PreparedCountryTableError {
    #[error("prepared country table has {found} prefix cells; expected {expected}")]
    PayloadLength { expected: usize, found: usize },
    #[error("prepared country table identity does not match its grids or decimation")]
    Identity,
    #[error("prepared country table crop is outside its effective grid")]
    CropBounds,
    #[error("prepared country table crop grid is invalid")]
    CropGrid(#[source] GridError),
    #[error("prepared country table total is not the final prefix value")]
    Total,
    #[error("prepared country table prefix encodes a negative cell population")]
    NegativeCell,
}

impl CountryTable {
    /// Restores a prefix table after its wire metadata and payload integrity have been checked.
    ///
    /// # Errors
    /// Rejects a payload with the wrong padded length, inconsistent identity or crop, a negative
    /// cell population, or a total that differs from the final prefix cell.
    pub fn from_prepared(
        prepared: PreparedCountryTable,
    ) -> Result<Self, PreparedCountryTableError> {
        let PreparedCountryTable {
            source_grid,
            effective_grid,
            grid,
            north_row,
            west_col,
            total,
            unassigned,
            identity,
            cells,
        } = prepared;
        let decimation = Decimation::new(source_grid, identity.decimation)
            .map_err(|_| PreparedCountryTableError::Identity)?;
        if identity.source_grid != source_grid
            || identity.effective_grid != effective_grid
            || *decimation.grid() != effective_grid
        {
            return Err(PreparedCountryTableError::Identity);
        }
        let height = grid.height();
        let width = grid.width();
        if north_row
            .checked_add(height)
            .is_none_or(|end| end > effective_grid.height())
            || width > effective_grid.width()
            || west_col >= effective_grid.width()
        {
            return Err(PreparedCountryTableError::CropBounds);
        }
        let expected_grid = country_crop_grid(effective_grid, north_row, height, west_col, width)
            .map_err(|error| match error {
            CountryTableBuildError::CropGrid(source) => PreparedCountryTableError::CropGrid(source),
            _ => PreparedCountryTableError::CropBounds,
        })?;
        if grid != expected_grid {
            return Err(PreparedCountryTableError::CropBounds);
        }
        let expected = padded_len(&grid);
        if cells.len() != expected {
            return Err(PreparedCountryTableError::PayloadLength {
                expected,
                found: cells.len(),
            });
        }
        let last = cells[expected - 1];
        if !total.is_finite()
            || !unassigned.is_finite()
            || !cells.iter().all(|cell| cell.is_finite() && *cell >= 0.0)
            || total.to_bits() != last.to_bits()
        {
            return Err(PreparedCountryTableError::Total);
        }
        let stride = width as usize + 1;
        for row in 0..=height as usize {
            for col in 1..=width as usize {
                let index = row * stride + col;
                if cells[index] < cells[index - 1]
                    || (row > 0 && cells[index] < cells[index - stride])
                {
                    return Err(PreparedCountryTableError::NegativeCell);
                }
            }
        }
        for row in 1..=height as usize {
            for col in 1..=width as usize {
                let index = row * stride + col;
                let northwest = cells[index - stride - 1];
                let northeast = cells[index - stride];
                let southwest = cells[index - 1];
                let southeast = cells[index];
                let population = (southeast - northeast) - (southwest - northwest);
                let rounding_bound =
                    8.0 * f64::EPSILON * southeast.max(northwest).max(northeast).max(southwest);
                if population < -rounding_bound {
                    return Err(PreparedCountryTableError::NegativeCell);
                }
            }
        }
        Ok(Self {
            source_grid,
            effective_grid,
            grid,
            north_row,
            west_col,
            total,
            unassigned,
            identity,
            cells,
        })
    }

    #[must_use]
    pub const fn source_grid(&self) -> &Grid {
        &self.source_grid
    }

    #[must_use]
    pub const fn effective_grid(&self) -> &Grid {
        &self.effective_grid
    }

    #[must_use]
    pub const fn grid(&self) -> &Grid {
        &self.grid
    }

    #[must_use]
    pub const fn north_row(&self) -> u32 {
        self.north_row
    }

    #[must_use]
    pub const fn west_col(&self) -> u32 {
        self.west_col
    }

    #[must_use]
    pub const fn total(&self) -> f64 {
        self.total
    }

    /// The positive population in cells with no unique owner, counted over the source grid.
    #[must_use]
    pub const fn unassigned(&self) -> f64 {
        self.unassigned
    }

    #[must_use]
    pub const fn identity(&self) -> &CountryTableIdentity {
        &self.identity
    }

    /// Immutable padded prefix payload for a portable prepared-data writer.
    #[must_use]
    pub fn prefix_cells(&self) -> &[f64] {
        &self.cells
    }

    /// # Errors
    /// Returns [`CountryTableIdentityError`] when any source, grid or rule field differs.
    pub fn require_identity(
        &self,
        expected: &CountryTableIdentity,
    ) -> Result<(), CountryTableIdentityError> {
        if &self.identity == expected {
            Ok(())
        } else {
            Err(CountryTableIdentityError)
        }
    }

    #[must_use]
    pub fn table(&self) -> Table<'_> {
        Table {
            grid: self.grid,
            cells: &self.cells,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CountryTableBuildError {
    #[error("the raster could not be read")]
    Raster(#[from] RasterError),
    #[error(transparent)]
    Ownership(#[from] OwnershipError),
    #[error("ownership and raster use different effective grids")]
    GridMismatch,
    #[error("feature {0} owns no cells on this grid")]
    NoOwnedCells(String),
    #[error("the country crop is not a valid grid")]
    CropGrid(#[source] GridError),
    #[error("the buffered raster has {found} rows; its grid has {expected}")]
    BufferedRowCount { expected: usize, found: usize },
    #[error("buffered raster row {row} has {found} cells; its grid has {expected}")]
    BufferedRowWidth {
        row: usize,
        expected: usize,
        found: usize,
    },
    #[error("a country table worker panicked")]
    WorkerPanicked,
}

/// Builds one country's cropped prefix and the source-wide unassigned residual.
///
/// # Errors
/// Returns an error when the raster and ownership grids differ, the source fails, or the feature
/// owns no cells at this resolution.
pub fn build_country<S: RasterSource>(
    source: S,
    ownership: &CellOwnership,
    feature_id: &FeatureId,
    boundary_source_digest: &str,
) -> Result<CountryTable, CountryTableBuildError> {
    let decimation = Decimation::none(source.grid());
    build_country_decimated(
        source,
        ownership,
        feature_id,
        decimation,
        boundary_source_digest,
    )
}

/// Builds one country's cropped prefix after classifying ownership on the native source grid.
///
/// Source cells are folded into their effective cell in row-major order before the country prefix is
/// built. The unassigned residual and source digest cover the native grid.
///
/// # Errors
/// Returns an error when the raster, ownership and decimation grids differ, the source fails, or the
/// feature owns no cells at this resolution.
pub fn build_country_decimated<S: RasterSource>(
    mut source: S,
    ownership: &CellOwnership,
    feature_id: &FeatureId,
    decimation: Decimation,
    boundary_source_digest: &str,
) -> Result<CountryTable, CountryTableBuildError> {
    let source_grid = source.grid();
    if ownership.grid() != &source_grid || decimation.source() != &source_grid {
        return Err(CountryTableBuildError::GridMismatch);
    }
    let effective_grid = *decimation.grid();
    let factor = decimation.factor();
    let Some((north_row, height, west_col, width)) = country_crop(ownership, feature_id, factor)
    else {
        return Err(CountryTableBuildError::NoOwnedCells(
            feature_id.as_str().to_owned(),
        ));
    };
    let grid = country_crop_grid(effective_grid, north_row, height, west_col, width)?;
    let crop_width = width as usize;
    let stride = crop_width + 1;
    let mut cells = Vec::with_capacity(padded_len(&grid));
    let mut acc = vec![0.0; stride];
    let mut corr = vec![0.0; stride];
    let mut block = vec![0.0; crop_width];
    let mut block_corr = vec![0.0; crop_width];
    let mut residual = 0.0;
    let mut residual_corr = 0.0;
    let mut digest = FNV_OFFSET_BASIS;
    cells.extend_from_slice(&acc);

    while let Some(row) = source.next_row() {
        let row = row?;
        for value in row.values {
            digest = (digest ^ u64::from(value.to_bits())).wrapping_mul(FNV_PRIME);
        }
        let intervals =
            ownership
                .row_intervals(row.row.get())
                .ok_or(OwnershipError::CellOutsideGrid {
                    row: row.row.get(),
                    col: 0,
                })?;
        for interval in intervals {
            if let CellOwner::Unassigned(_) = interval.owner() {
                for value in &row.values[interval.start() as usize..interval.end() as usize] {
                    let (sum, dropped) = two_sum(residual, f64::from(*value));
                    (residual, residual_corr) = two_sum(sum, residual_corr + dropped);
                }
            }
        }

        for interval in intervals {
            if !matches!(interval.owner(), CellOwner::Feature(owner) if owner == feature_id) {
                continue;
            }
            for column in interval.start()..interval.end() {
                let effective_col = column / factor;
                let crop_col = if effective_col >= west_col {
                    effective_col - west_col
                } else {
                    effective_grid.width() - west_col + effective_col
                };
                if crop_col < width {
                    let index = crop_col as usize;
                    let (sum, dropped) =
                        two_sum(block[index], f64::from(row.values[column as usize]));
                    (block[index], block_corr[index]) = two_sum(sum, block_corr[index] + dropped);
                }
            }
        }
        if !(row.row.get() + 1).is_multiple_of(factor) {
            continue;
        }
        let effective_row = row.row.get() / factor;
        if (north_row..north_row + height).contains(&effective_row) {
            append_country_prefix_row(&mut cells, &mut acc, &mut corr, &block, &block_corr);
        }
        block.fill(0.0);
        block_corr.fill(0.0);
    }

    Ok(CountryTable {
        source_grid,
        effective_grid,
        grid,
        north_row,
        west_col,
        total: acc[crop_width],
        unassigned: residual + residual_corr,
        identity: CountryTableIdentity {
            feature_id: feature_id.clone(),
            source_digest: digest,
            boundary_source_digest: boundary_source_digest.to_owned(),
            ownership_digest: country_ownership_digest(ownership),
            source_grid,
            effective_grid,
            decimation: factor,
            ownership_rule_version: OWNERSHIP_RULE_VERSION,
            aggregation_order_version: COUNTRY_AGGREGATION_ORDER_VERSION,
        },
        cells,
    })
}

/// Builds one country's cropped prefix with two scoped execution contexts.
///
/// The worker computes each horizontal prefix row independently; the caller retains row order when
/// folding those rows vertically, so parallel scheduling does not change the arithmetic order.
///
/// # Errors
/// Returns an error when the buffered rows, ownership or decimation do not match the source grid,
/// the feature owns no cells, or the worker fails.
pub fn build_country_decimated_parallel(
    source_grid: Grid,
    rows: &[Vec<f32>],
    ownership: &CellOwnership,
    feature_id: &FeatureId,
    decimation: Decimation,
    boundary_source_digest: &str,
) -> Result<CountryTable, CountryTableBuildError> {
    if ownership.grid() != &source_grid || decimation.source() != &source_grid {
        return Err(CountryTableBuildError::GridMismatch);
    }
    if rows.len() != source_grid.height() as usize {
        return Err(CountryTableBuildError::BufferedRowCount {
            expected: source_grid.height() as usize,
            found: rows.len(),
        });
    }
    for (row, values) in rows.iter().enumerate() {
        if values.len() != source_grid.width() as usize {
            return Err(CountryTableBuildError::BufferedRowWidth {
                row,
                expected: source_grid.width() as usize,
                found: values.len(),
            });
        }
    }

    let effective_grid = *decimation.grid();
    let factor = decimation.factor();
    let Some((north_row, height, west_col, width)) = country_crop(ownership, feature_id, factor)
    else {
        return Err(CountryTableBuildError::NoOwnedCells(
            feature_id.as_str().to_owned(),
        ));
    };
    let grid = country_crop_grid(effective_grid, north_row, height, west_col, width)?;
    let crop_width = width as usize;
    let stride = crop_width + 1;
    let (source_digest, residual, mut cells) = std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            fill_country_horizontal_rows(
                rows,
                ownership,
                feature_id,
                decimation,
                (north_row, height, west_col, width),
                stride,
            )
        });
        let source_metadata = source_digest_and_residual(rows, ownership);
        match worker.join() {
            Ok(Ok(cells)) => source_metadata.map(|(digest, residual)| (digest, residual, cells)),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(CountryTableBuildError::WorkerPanicked),
        }
    })?;

    let mut acc = vec![0.0; stride];
    let mut corr = vec![0.0; stride];
    for row in 1..=height as usize {
        let offset = row * stride;
        for column in 1..stride {
            let (sum, dropped) = two_sum(acc[column], cells[offset + column]);
            (acc[column], corr[column]) = two_sum(sum, corr[column] + dropped);
            cells[offset + column] = acc[column];
        }
    }

    Ok(CountryTable {
        source_grid,
        effective_grid,
        grid,
        north_row,
        west_col,
        total: acc[crop_width],
        unassigned: residual,
        identity: CountryTableIdentity {
            feature_id: feature_id.clone(),
            source_digest,
            boundary_source_digest: boundary_source_digest.to_owned(),
            ownership_digest: country_ownership_digest(ownership),
            source_grid,
            effective_grid,
            decimation: factor,
            ownership_rule_version: OWNERSHIP_RULE_VERSION,
            aggregation_order_version: COUNTRY_AGGREGATION_ORDER_VERSION,
        },
        cells,
    })
}

fn source_digest_and_residual(
    rows: &[Vec<f32>],
    ownership: &CellOwnership,
) -> Result<(u64, f64), CountryTableBuildError> {
    let mut digest = FNV_OFFSET_BASIS;
    let mut residual = 0.0;
    let mut residual_corr = 0.0;
    for (source_row, values) in ownership.grid().rows().zip(rows) {
        for value in values {
            digest = (digest ^ u64::from(value.to_bits())).wrapping_mul(FNV_PRIME);
        }
        let intervals =
            ownership
                .row_intervals(source_row.get())
                .ok_or(OwnershipError::CellOutsideGrid {
                    row: source_row.get(),
                    col: 0,
                })?;
        for interval in intervals {
            if let CellOwner::Unassigned(_) = interval.owner() {
                for value in &values[interval.start() as usize..interval.end() as usize] {
                    let (sum, dropped) = two_sum(residual, f64::from(*value));
                    (residual, residual_corr) = two_sum(sum, residual_corr + dropped);
                }
            }
        }
    }
    Ok((digest, residual + residual_corr))
}

#[allow(clippy::cast_possible_truncation)]
fn fill_country_horizontal_rows(
    rows: &[Vec<f32>],
    ownership: &CellOwnership,
    feature_id: &FeatureId,
    decimation: Decimation,
    crop: (u32, u32, u32, u32),
    stride: usize,
) -> Result<Vec<f64>, CountryTableBuildError> {
    let effective_grid = *decimation.grid();
    let factor = decimation.factor();
    let (north_row, height, west_col, width) = crop;
    let crop_width = width as usize;
    let mut output = Vec::with_capacity((height as usize + 1) * stride);
    output.resize(stride, 0.0);
    let mut block = vec![0.0; crop_width];
    let mut block_corr = vec![0.0; crop_width];
    for effective_row in north_row..north_row + height {
        for source_row in effective_row * factor..(effective_row + 1) * factor {
            let values = &rows[source_row as usize];
            let intervals =
                ownership
                    .row_intervals(source_row)
                    .ok_or(OwnershipError::CellOutsideGrid {
                        row: source_row,
                        col: 0,
                    })?;
            for interval in intervals {
                if !matches!(interval.owner(), CellOwner::Feature(owner) if owner == feature_id) {
                    continue;
                }
                for column in interval.start()..interval.end() {
                    let effective_col = column / factor;
                    let crop_col = if effective_col >= west_col {
                        effective_col - west_col
                    } else {
                        effective_grid.width() - west_col + effective_col
                    };
                    if crop_col < width {
                        let index = crop_col as usize;
                        let (sum, dropped) =
                            two_sum(block[index], f64::from(values[column as usize]));
                        (block[index], block_corr[index]) =
                            two_sum(sum, block_corr[index] + dropped);
                    }
                }
            }
        }
        country_horizontal_prefix_row(&mut output, &block, &block_corr);
        block.fill(0.0);
        block_corr.fill(0.0);
    }
    Ok(output)
}

fn country_horizontal_prefix_row(output: &mut Vec<f64>, block: &[f64], block_corr: &[f64]) {
    output.push(0.0);
    let mut prefix = 0.0;
    let mut prefix_corr = 0.0;
    for (&sum, &correction) in block.iter().zip(block_corr) {
        let (sum, dropped) = two_sum(prefix, sum + correction);
        (prefix, prefix_corr) = two_sum(sum, prefix_corr + dropped);
        output.push(prefix);
    }
}

fn country_crop_grid(
    effective_grid: Grid,
    north_row: u32,
    height: u32,
    west_col: u32,
    width: u32,
) -> Result<Grid, CountryTableBuildError> {
    Grid::new(
        width,
        height,
        crate::geodesy::LatLon {
            lat: effective_grid.origin().lat + f64::from(north_row) * effective_grid.lat_step(),
            lon: effective_grid.origin().lon + f64::from(west_col) * effective_grid.lon_step(),
        },
        effective_grid.lon_step(),
        effective_grid.lat_step(),
    )
    .map_err(CountryTableBuildError::CropGrid)
}

fn append_country_prefix_row(
    cells: &mut Vec<f64>,
    acc: &mut [f64],
    corr: &mut [f64],
    block: &[f64],
    block_corr: &[f64],
) {
    let offset = cells.len();
    country_horizontal_prefix_row(cells, block, block_corr);
    for column in 1..acc.len() {
        let (sum, dropped) = two_sum(acc[column], cells[offset + column]);
        (acc[column], corr[column]) = two_sum(sum, corr[column] + dropped);
        cells[offset + column] = acc[column];
    }
}

fn country_ownership_digest(ownership: &CellOwnership) -> u64 {
    let mut digest = FNV_OFFSET_BASIS;
    for row in 0..ownership.grid().height() {
        for interval in ownership.row_intervals(row).unwrap_or_default() {
            feed_digest(&mut digest, &row.to_le_bytes());
            feed_digest(&mut digest, &interval.start().to_le_bytes());
            feed_digest(&mut digest, &interval.end().to_le_bytes());
            match interval.owner() {
                CellOwner::Feature(feature) => {
                    feed_digest(&mut digest, &[1]);
                    feed_feature_id(&mut digest, feature);
                }
                CellOwner::Unassigned(crate::ownership::UnassignedReason::OutsideFeatures) => {
                    feed_digest(&mut digest, &[2]);
                }
                CellOwner::Unassigned(crate::ownership::UnassignedReason::MultipleFeatures(
                    features,
                )) => {
                    feed_digest(&mut digest, &[3]);
                    feed_digest(&mut digest, &(features.len() as u64).to_le_bytes());
                    for feature in features {
                        feed_feature_id(&mut digest, feature);
                    }
                }
            }
            feed_digest(&mut digest, &[0xff]);
        }
    }
    digest
}

fn feed_feature_id(digest: &mut u64, feature: &FeatureId) {
    let bytes = feature.as_str().as_bytes();
    feed_digest(digest, &(bytes.len() as u64).to_le_bytes());
    feed_digest(digest, bytes);
}

fn feed_digest(digest: &mut u64, bytes: &[u8]) {
    for &byte in bytes {
        *digest = (*digest ^ u64::from(byte)).wrapping_mul(FNV_PRIME);
    }
}

fn country_crop(
    ownership: &CellOwnership,
    feature_id: &FeatureId,
    factor: u32,
) -> Option<(u32, u32, u32, u32)> {
    let source_grid = ownership.grid();
    let mut north = source_grid.height();
    let mut south = 0;
    let mut columns = Vec::new();
    for row in 0..source_grid.height() {
        let intervals = ownership.row_intervals(row)?;
        for interval in intervals {
            if matches!(interval.owner(), CellOwner::Feature(owner) if owner == feature_id) {
                let effective_row = row / factor;
                north = north.min(effective_row);
                south = south.max(effective_row);
                columns.push((interval.start() / factor, interval.end().div_ceil(factor)));
            }
        }
    }
    if columns.is_empty() {
        return None;
    }
    columns.sort_unstable();
    let mut merged: Vec<(u32, u32)> = Vec::new();
    for (start, end) in columns {
        if let Some(previous) = merged.last_mut()
            && start <= previous.1
        {
            previous.1 = previous.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    let mut largest_gap = 0;
    let mut west_col = 0;
    for index in 0..merged.len() {
        let current = merged[index];
        let next = merged[(index + 1) % merged.len()];
        let effective_width = source_grid.width() / factor;
        let gap = if index + 1 == merged.len() {
            effective_width - current.1 + next.0
        } else {
            next.0 - current.1
        };
        let candidate_west = next.0 % (source_grid.width() / factor);
        if gap > largest_gap || (gap == largest_gap && candidate_west < west_col) {
            largest_gap = gap;
            west_col = candidate_west;
        }
    }
    Some((
        north,
        south - north + 1,
        west_col,
        source_grid.width() / factor - largest_gap,
    ))
}

// The digest in full, because one whose word width or order is left to the
// implementation is a number that happens to match today rather than an identity: FNV-1a, 64-bit,
// standard offset basis and prime, over each sanitised cell's `f32` bits widened to `u64`, in
// row-major order.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Streams a raster into the padded rows of a summation table, handing each row to `emit` as it is
/// completed.
///
/// What stays resident is one accumulator row, one correction row and the row the source lends — 864 KB
/// at the registry raster's width, for a table of 7.5 GB — so the build's memory is the grid's width
/// rather than its area. The accumulator holds the correctly rounded column sum and the correction
/// holds exactly what f64 could not, renormalised after every row; that is why the accumulator can be
/// emitted as it stands, where Neumaier's deferred correction would need a third row resident to add
/// the two together into. A decimated build adds one row of the coarser width to gather into.
///
/// # Errors
/// [`BuildError::Raster`] when the source fails mid-stream, [`BuildError::Sink`] when `emit` does.
///
/// # Panics
/// If `decimation` was built against a grid other than this source's. The types cannot catch that —
/// both are grids — and the alternative to stopping is a table of a shape nobody asked for.
pub fn build<S, P, F, E>(
    mut source: S,
    decimation: Decimation,
    progress: &mut P,
    mut emit: F,
) -> Result<BuiltTable, BuildError<E>>
where
    S: RasterSource,
    P: Progress,
    F: FnMut(&[f64]) -> Result<(), E>,
{
    let grid = source.grid();
    assert_eq!(
        *decimation.source(),
        grid,
        "the decimation was built against a different grid than this source declares"
    );
    let width = grid.width() as usize;
    let rows = u64::from(grid.height());
    let factor = decimation.factor() as usize;

    let mut acc = vec![0.0f64; width + 1];
    let mut corr = vec![0.0f64; width + 1];
    // Empty at factor 1, where the accumulator is already the row to emit: that is what holds a
    // full-resolution build to two rows resident rather than three.
    let mut coarse = if factor == 1 {
        Vec::new()
    } else {
        vec![0.0f64; decimation.grid().width() as usize + 1]
    };
    let mut digest = FNV_OFFSET_BASIS;

    // The boundary this step is, at the one granularity the library knows and a caller does not: the shape
    // going in and the shape coming out. Through the facade, so nothing here names a stream.
    log::info!(
        "streaming {} x {} cells into a {} x {} table",
        grid.width(),
        grid.height(),
        decimation.grid().width(),
        decimation.grid().height()
    );
    // Box 7's first granularity. Bound rather than discarded, and it outlives both `?`s below.
    let _bracket = Bracket::open(module_path!(), "table build");

    // The zero row, which is the padding itself: emitting it here is what lets a rectangle touching
    // the north edge subtract four corners like any other.
    emit(decimated(&acc, &mut coarse, factor)).map_err(BuildError::Sink)?;
    progress.advance(0, rows);

    let mut done = 0u64;
    while let Some(row) = source.next_row() {
        let values = row?.values;

        // The row prefix lives in two scalars rather than a row of its own: each prefix is consumed by
        // the column it belongs to in the same step, so nothing here ever needs a whole prefix row.
        let mut prefix = 0.0f64;
        let mut prefix_corr = 0.0f64;
        for (index, &cell) in values.iter().enumerate() {
            digest = (digest ^ u64::from(cell.to_bits())).wrapping_mul(FNV_PRIME);

            // The one widening in the build, and the ground rule behind it: f32 -> f64 is exact, and
            // no accumulator below is ever anything but f64.
            let (sum, dropped) = two_sum(prefix, f64::from(cell));
            (prefix, prefix_corr) = two_sum(sum, prefix_corr + dropped);

            let column = index + 1;
            let (sum, dropped) = two_sum(acc[column], prefix);
            (acc[column], corr[column]) = two_sum(sum, corr[column] + dropped);
        }

        done += 1;
        // Every k-th row, because the factor divides the height and so the last row always completes a
        // block: no partial block ever reaches this.
        if done.is_multiple_of(factor as u64) {
            emit(decimated(&acc, &mut coarse, factor)).map_err(BuildError::Sink)?;
        }
        progress.advance(done, rows);
    }

    Ok(BuiltTable {
        digest,
        tallies: source.finish(),
        total: acc[width],
        decimation,
    })
}

/// The row to emit: the accumulator itself, or every k-th cell of it.
///
/// A decimated summation table **is** the full one's every k-th row and column, because the prefix sum
/// over k by k blocks and the prefix sum over cells agree at every block corner. So decimation needs no
/// block sum of its own, and the one thing forbidden — a block rounded before the table sees it
/// — has nothing to round: every source cell is folded into the f64 accumulators exactly as it would be
/// at full resolution, and the coarser table is what is read back out.
fn decimated<'r>(acc: &'r [f64], coarse: &'r mut [f64], factor: usize) -> &'r [f64] {
    if factor == 1 {
        return acc;
    }
    for (index, cell) in coarse.iter_mut().enumerate() {
        *cell = acc[index * factor];
    }
    coarse
}

/// Knuth's two-sum: the rounded sum, and exactly what the rounding dropped, for any two magnitudes.
///
/// The unconditional form rather than Neumaier's comparison of magnitudes — the same arithmetic, and
/// the error term is what both are for, but this one costs no branch per cell over 933 120 000 of them.
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let sum = a + b;
    let b_rounded = sum - a;
    (sum, (a - (sum - b_rounded)) + (b - b_rounded))
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests. float_cmp joins them because every value
// below is a small integer, exact in f64: what these assertions pin is the indexing and the wrapping,
// so a tolerance would let an off-by-one in the padding pass. cast_precision_loss likewise — the
// generated cells are integers below 2^20, where u32 -> f32 is exact.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss
)]
mod tests {
    use std::convert::Infallible;
    use std::ops::RangeInclusive;

    use proptest::prelude::*;

    use super::*;
    use crate::domain::{Geometry, Polygon};
    use crate::geodesy::LatLon;
    use crate::ownership::CellOwnership;
    use crate::raster::Synthetic;

    // The cells and the padded table over them are both written out, rather than one derived from the
    // other: the code that derives one is 1.3's builder, and a fixture it computed would assert
    // nothing about the indexing here.
    static CELLS: [[f64; 4]; 3] = [
        [1.0, 2.0, 3.0, 4.0],
        [5.0, 6.0, 7.0, 8.0],
        [9.0, 10.0, 11.0, 12.0],
    ];

    static PAYLOAD: [[f64; 5]; 4] = [
        [0.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 3.0, 6.0, 10.0],
        [0.0, 6.0, 14.0, 24.0, 36.0],
        [0.0, 15.0, 33.0, 54.0, 78.0],
    ];

    fn grid() -> Grid {
        // Four columns of 90 degrees close on themselves, which is what makes the wrapped and
        // full-turn spans below cases this grid actually has.
        Grid::new(
            4,
            3,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            90.0,
            -60.0,
        )
        .expect("a 4 x 3 whole-globe grid is valid")
    }

    fn rectangle(west: f64, south: f64, east: f64, north: f64) -> Geometry {
        Geometry::Polygon(Polygon {
            rings: vec![vec![
                LatLon {
                    lat: south,
                    lon: west,
                },
                LatLon {
                    lat: south,
                    lon: east,
                },
                LatLon {
                    lat: north,
                    lon: east,
                },
                LatLon {
                    lat: north,
                    lon: west,
                },
                LatLon {
                    lat: south,
                    lon: west,
                },
            ]],
        })
    }

    #[test]
    fn a_country_prefix_and_the_unassigned_residual_reconcile_to_the_world() {
        let grid = grid();
        let a = FeatureId::new("A").expect("fixture ID is valid");
        let b = FeatureId::new("B").expect("fixture ID is valid");
        let features = vec![
            (a.clone(), rectangle(-180.0, -30.0, 0.0, 30.0)),
            (b.clone(), rectangle(0.0, -30.0, 180.0, 30.0)),
        ];
        let (ownership, _) =
            CellOwnership::rasterize_features(&features, grid).expect("fixture ownership is valid");
        let rows = vec![
            vec![1.0, 2.0, 3.0, 4.0],
            vec![5.0, 6.0, 7.0, 8.0],
            vec![9.0, 10.0, 11.0, 12.0],
        ];
        let source = |rows: Vec<Vec<f32>>| {
            Synthetic::new(grid, -9999.0, rows).expect("fixture raster has the grid's shape")
        };

        let table_a = build_country(source(rows.clone()), &ownership, &a, "fixture-gisco-v1")
            .expect("A owns cells on the fixture grid");
        let table_b = build_country(source(rows), &ownership, &b, "fixture-gisco-v1")
            .expect("B owns cells on the fixture grid");

        assert_eq!((table_a.grid().width(), table_a.grid().height()), (2, 1));
        assert_eq!(table_a.total(), 11.0);
        assert_eq!(table_b.total(), 15.0);
        assert_eq!(table_a.unassigned(), 52.0);
        assert_eq!(table_b.unassigned(), 52.0);
        let restored = CountryTable::from_prepared(PreparedCountryTable {
            source_grid: *table_a.source_grid(),
            effective_grid: *table_a.effective_grid(),
            grid: *table_a.grid(),
            north_row: table_a.north_row(),
            west_col: table_a.west_col(),
            total: table_a.total(),
            unassigned: table_a.unassigned(),
            identity: table_a.identity().clone(),
            cells: table_a.prefix_cells().to_vec(),
        })
        .expect("the prepared prefix restores its table");
        assert_eq!(restored, table_a);
        let invalid = CountryTable::from_prepared(PreparedCountryTable {
            source_grid: *table_a.source_grid(),
            effective_grid: *table_a.effective_grid(),
            grid: *table_a.grid(),
            north_row: table_a.north_row(),
            west_col: table_a.west_col(),
            total: 5.0,
            unassigned: table_a.unassigned(),
            identity: table_a.identity().clone(),
            cells: vec![0.0, 10.0, 5.0, 0.0, 10.0, 5.0],
        });
        assert!(matches!(
            invalid,
            Err(PreparedCountryTableError::NegativeCell)
        ));
        let crop = country_crop_grid(*table_a.effective_grid(), 0, 2, 0, 2)
            .expect("the 2 by 2 fixture crop is valid");
        let invalid_2d = CountryTable::from_prepared(PreparedCountryTable {
            source_grid: *table_a.source_grid(),
            effective_grid: *table_a.effective_grid(),
            grid: crop,
            north_row: 0,
            west_col: 0,
            total: 22.0,
            unassigned: table_a.unassigned(),
            identity: table_a.identity().clone(),
            cells: vec![0.0, 0.0, 0.0, 0.0, 10.0, 20.0, 0.0, 15.0, 22.0],
        });
        assert!(matches!(
            invalid_2d,
            Err(PreparedCountryTableError::NegativeCell)
        ));
        let invalid_overflow = CountryTable::from_prepared(PreparedCountryTable {
            source_grid: *table_a.source_grid(),
            effective_grid: *table_a.effective_grid(),
            grid: crop,
            north_row: 0,
            west_col: 0,
            total: 1.6e308,
            unassigned: table_a.unassigned(),
            identity: table_a.identity().clone(),
            cells: vec![0.0, 0.0, 0.0, 0.0, 1e308, 1.5e308, 0.0, 1.5e308, 1.6e308],
        });
        assert!(matches!(
            invalid_overflow,
            Err(PreparedCountryTableError::NegativeCell)
        ));
        let table = table_a.table();
        let (rows, cols) = table.whole();
        assert_eq!(table.population(rows, cols), 11.0);
        assert_eq!(
            table_a.total() + table_b.total() + table_a.unassigned(),
            78.0
        );
    }

    #[test]
    fn parallel_country_prefix_matches_sequential_for_overlap_and_decimation() {
        let grid = Grid::new(
            36,
            18,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            10.0,
            -10.0,
        )
        .expect("fixture grid is valid");
        let a = FeatureId::new("A").expect("fixture ID is valid");
        let b = FeatureId::new("B").expect("fixture ID is valid");
        let empty = FeatureId::new("EMPTY").expect("fixture ID is valid");
        let features = vec![
            (a.clone(), rectangle(-180.0, -30.0, 10.0, 30.0)),
            (b.clone(), rectangle(-10.0, -30.0, 180.0, 30.0)),
        ];
        let (ownership, _) =
            CellOwnership::rasterize_features(&features, grid).expect("ownership is valid");
        let rows: Vec<Vec<f32>> = (0..grid.height())
            .map(|row| {
                (0..grid.width())
                    .map(|col| (row * grid.width() + col + 1) as f32)
                    .collect()
            })
            .collect();
        let decimation = Decimation::new(grid, 3).expect("three divides the grid");

        for feature in [&a, &b] {
            let sequential = build_country_decimated(
                Synthetic::new(grid, -9999.0, rows.clone()).expect("fixture shape is valid"),
                &ownership,
                feature,
                decimation,
                "fixture-boundaries",
            )
            .expect("feature owns cells");
            let parallel = build_country_decimated_parallel(
                grid,
                &rows,
                &ownership,
                feature,
                decimation,
                "fixture-boundaries",
            )
            .expect("feature owns cells");
            assert_eq!(parallel.cells, sequential.cells);
            assert_eq!(parallel.total(), sequential.total());
            assert_eq!(parallel.unassigned(), sequential.unassigned());
            assert_eq!(parallel.identity(), sequential.identity());
        }
        let disjoint_features = vec![
            (a.clone(), rectangle(-180.0, -30.0, -20.0, 30.0)),
            (b.clone(), rectangle(20.0, -30.0, 180.0, 30.0)),
        ];
        let (disjoint_ownership, _) = CellOwnership::rasterize_features(&disjoint_features, grid)
            .expect("disjoint ownership is valid");
        for feature in [&a, &b] {
            let sequential = build_country_decimated(
                Synthetic::new(grid, -9999.0, rows.clone()).expect("fixture shape is valid"),
                &disjoint_ownership,
                feature,
                decimation,
                "fixture-boundaries",
            )
            .expect("feature owns cells");
            let parallel = build_country_decimated_parallel(
                grid,
                &rows,
                &disjoint_ownership,
                feature,
                decimation,
                "fixture-boundaries",
            )
            .expect("feature owns cells");
            assert_eq!(parallel.cells, sequential.cells);
            assert_eq!(parallel.total(), sequential.total());
            assert_eq!(parallel.unassigned(), sequential.unassigned());
            assert_eq!(parallel.identity(), sequential.identity());
        }
        assert!(matches!(
            build_country_decimated_parallel(
                grid,
                &rows,
                &ownership,
                &empty,
                decimation,
                "fixture-boundaries",
            ),
            Err(CountryTableBuildError::NoOwnedCells(_))
        ));
    }

    #[test]
    fn native_ownership_aggregates_before_decimation_and_identity_rejects_changes() {
        let grid = Grid::new(
            360,
            180,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            1.0,
            -1.0,
        )
        .expect("one-degree globe grid is valid");
        let feature = FeatureId::new("A").expect("fixture ID is valid");
        let (ownership, _) = CellOwnership::rasterize_features(
            &[(feature.clone(), rectangle(-2.0, -2.0, 2.0, 2.0))],
            grid,
        )
        .expect("fixture ownership is valid");
        let rows: Vec<Vec<f32>> = (0..grid.height())
            .map(|row| {
                (0..grid.width())
                    .map(|col| (row * grid.width() + col + 1) as f32)
                    .collect()
            })
            .collect();
        let source = |rows: Vec<Vec<f32>>| {
            Synthetic::new(grid, -9999.0, rows).expect("fixture raster has the grid's shape")
        };
        let fine = build_country(
            source(rows.clone()),
            &ownership,
            &feature,
            "fixture-gisco-v1",
        )
        .expect("feature owns source cells");
        let decimation = Decimation::new(grid, 10).expect("ten divides the globe grid");
        let coarse = build_country_decimated(
            source(rows.clone()),
            &ownership,
            &feature,
            decimation,
            "fixture-gisco-v1",
        )
        .expect("native ownership can be aggregated");

        let effective = *decimation.grid();
        let mut expected = vec![0.0; padded_len(coarse.grid())];
        let crop_width = coarse.grid().width() as usize;
        let stride = crop_width + 1;
        for crop_row in 0..coarse.grid().height() {
            let effective_row = coarse.north_row() + crop_row;
            let mut row_sum = vec![0.0; crop_width];
            for source_row in effective_row * 10..(effective_row + 1) * 10 {
                for source_col in 0..grid.width() {
                    if !matches!(
                        ownership
                            .owner_at(&grid, source_row, source_col)
                            .expect("source cell is in the grid"),
                        CellOwner::Feature(owner) if owner == &feature
                    ) {
                        continue;
                    }
                    let effective_col = source_col / 10;
                    let crop_col = if effective_col >= coarse.west_col() {
                        effective_col - coarse.west_col()
                    } else {
                        effective.width() - coarse.west_col() + effective_col
                    };
                    if crop_col < coarse.grid().width() {
                        row_sum[crop_col as usize] +=
                            f64::from(rows[source_row as usize][source_col as usize]);
                    }
                }
            }
            let mut prefix = 0.0;
            for (col, value) in row_sum.into_iter().enumerate() {
                prefix += value;
                let index = (crop_row as usize + 1) * stride + col + 1;
                expected[index] = expected[crop_row as usize * stride + col + 1] + prefix;
            }
        }
        let expected_total = expected[expected.len() - 1];
        assert_eq!(coarse.cells, expected);
        assert_eq!(coarse.total(), fine.total());
        assert_eq!(coarse.total(), expected_total);
        assert_eq!(coarse.unassigned(), fine.unassigned());
    }

    #[test]
    fn country_identity_rejects_changed_raster_boundary_ownership_grid_and_rule() {
        let grid = Grid::new(
            360,
            180,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            1.0,
            -1.0,
        )
        .expect("one-degree globe grid is valid");
        let feature = FeatureId::new("A").expect("fixture ID is valid");
        let (ownership, _) = CellOwnership::rasterize_features(
            &[(feature.clone(), rectangle(-2.0, -2.0, 2.0, 2.0))],
            grid,
        )
        .expect("fixture ownership is valid");
        let rows: Vec<Vec<f32>> = (0..grid.height())
            .map(|row| {
                (0..grid.width())
                    .map(|col| (row * grid.width() + col + 1) as f32)
                    .collect()
            })
            .collect();
        let source = |rows: Vec<Vec<f32>>| {
            Synthetic::new(grid, -9999.0, rows).expect("fixture raster has the grid's shape")
        };
        let decimation = Decimation::new(grid, 10).expect("ten divides the globe grid");
        let coarse = build_country_decimated(
            source(rows.clone()),
            &ownership,
            &feature,
            decimation,
            "fixture-gisco-v1",
        )
        .expect("feature owns source cells");

        let mut changed_source = rows.clone();
        changed_source[0][0] += 1.0;
        let changed = build_country_decimated(
            source(changed_source),
            &ownership,
            &feature,
            decimation,
            "fixture-gisco-v1",
        )
        .expect("changed source still builds");
        assert!(coarse.require_identity(coarse.identity()).is_ok());
        assert!(coarse.require_identity(changed.identity()).is_err());

        let (changed_ownership, _) = CellOwnership::rasterize_features(
            &[(feature.clone(), rectangle(-3.0, -2.0, 2.0, 2.0))],
            grid,
        )
        .expect("changed geometry still rasterizes");
        let changed = build_country_decimated(
            source(rows.clone()),
            &changed_ownership,
            &feature,
            decimation,
            "fixture-gisco-v1",
        )
        .expect("changed ownership still builds");
        assert!(coarse.require_identity(changed.identity()).is_err());

        let mut changed_boundary = coarse.identity().clone();
        changed_boundary.boundary_source_digest = "fixture-gisco-v2".to_owned();
        assert!(coarse.require_identity(&changed_boundary).is_err());

        let mut changed_grid = coarse.identity().clone();
        changed_grid.effective_grid = grid;
        assert!(coarse.require_identity(&changed_grid).is_err());
        let mut changed_rule = coarse.identity().clone();
        changed_rule.ownership_rule_version += 1;
        assert!(coarse.require_identity(&changed_rule).is_err());
    }

    fn table() -> Table<'static> {
        Table::new(grid(), PAYLOAD.as_flattened()).expect("the payload is the padded product")
    }

    fn band(north: u32, south: u32) -> RowBand {
        let grid = grid();
        RowBand::new(
            grid.row(north).expect("a row of the fixture"),
            grid.row(south).expect("a row of the fixture"),
        )
    }

    fn through(west: u32, east: u32) -> ColSpan {
        let grid = grid();
        ColSpan::Through {
            west: grid.col(west).expect("a column of the fixture"),
            east: grid.col(east).expect("a column of the fixture"),
        }
    }

    /// The reference: the named cells added up directly. `cols` is a list rather than a range so a
    /// wrapped span's reference is as easy to write as a contiguous one's.
    fn direct(rows: RangeInclusive<usize>, cols: &[usize]) -> f64 {
        rows.flat_map(|row| cols.iter().map(move |col| CELLS[row][*col]))
            .sum()
    }

    #[test]
    fn an_interior_rectangle_is_the_sum_of_its_cells() {
        assert_eq!(
            table().population(band(1, 2), through(1, 2)),
            direct(1..=2, &[1, 2])
        );
    }

    #[test]
    fn a_span_wrapped_across_the_antimeridian_covers_both_pieces() {
        assert_eq!(
            table().population(band(0, 1), through(3, 0)),
            direct(0..=1, &[3, 0])
        );
    }

    #[test]
    fn the_full_turn_is_the_row_band_itself() {
        // Against the band's own total, not against two rectangles: a full turn assembled from a pair
        // of spans is the double count this enum exists to make unwritable, and comparing one
        // assembly with another would agree with it.
        assert_eq!(
            table().population(band(1, 2), ColSpan::FullTurn),
            direct(1..=2, &[0, 1, 2, 3])
        );
    }

    #[test]
    fn a_span_of_one_column_is_one_column() {
        // The other half of that: `west == east` says one column and cannot be read as all of them.
        assert_eq!(
            table().population(band(0, 2), through(2, 2)),
            direct(0..=2, &[2])
        );
    }

    #[test]
    fn a_payload_that_is_not_the_padded_product_is_refused() {
        let full = PAYLOAD.as_flattened();
        let short = &full[..full.len() - 1];
        let long: Vec<f64> = full.iter().copied().chain([0.0]).collect();

        assert_eq!(
            Table::new(grid(), short).unwrap_err(),
            TableError::PayloadLength {
                expected: 20,
                found: 19
            }
        );
        assert_eq!(
            Table::new(grid(), &long).unwrap_err(),
            TableError::PayloadLength {
                expected: 20,
                found: 21
            }
        );
    }

    #[test]
    fn the_whole_table_is_every_row_and_the_full_turn() {
        let table = table();
        let (rows, cols) = table.whole();
        assert_eq!((rows, cols), (band(0, 2), ColSpan::FullTurn));
        assert_eq!(table.population(rows, cols), direct(0..=2, &[0, 1, 2, 3]));
    }

    #[test]
    fn a_window_spanning_a_full_turn_is_every_column_and_not_one() {
        // The case a caller reducing two longitudes to columns gets wrong: -180 and 180 are the same
        // column, so corners alone would ask for the westernmost strip and call it the globe. The
        // southern latitude is off the pole because the grid's outer boundary lies in no cell, which is
        // why `whole` exists above.
        let table = table();
        let world = Window {
            north: 90.0,
            south: -89.0,
            west: -180.0,
            east: 180.0,
        };
        let (rows, cols) = table
            .covering(world)
            .expect("both latitudes are on a whole-globe grid");
        assert_eq!((rows, cols), (band(0, 2), ColSpan::FullTurn));
    }

    #[test]
    fn a_window_across_the_antimeridian_wraps_and_one_of_zero_width_is_a_column() {
        let table = table();
        // The fixture's columns are 90 degrees wide from -180, so 170E is the last column and 170W the
        // first; 31N is inside the first row, where 30N is the second row's northern boundary and so the
        // second row's own.
        let wrapped = Window {
            north: 90.0,
            south: 31.0,
            west: 170.0,
            east: -170.0,
        };
        let (rows, cols) = table
            .covering(wrapped)
            .expect("both corners are on the grid");
        assert_eq!((rows, cols), (band(0, 0), through(3, 0)));

        let single = Window {
            north: 90.0,
            south: -89.0,
            west: 0.0,
            east: 0.0,
        };
        let (_, cols) = table
            .covering(single)
            .expect("both corners are on the grid");
        assert_eq!(cols, through(2, 2));
    }

    #[test]
    fn a_window_off_the_grid_has_no_rows() {
        // A window grid is where this bites: a latitude outside it has no row, and the answer is that
        // there is no rectangle rather than the nearest one.
        let grid = Grid::new(
            4,
            1,
            LatLon {
                lat: 10.0,
                lon: -180.0,
            },
            90.0,
            -10.0,
        )
        .expect("a one-row band is a valid grid");
        let payload = [0.0f64; 10];
        let table = Table::new(grid, &payload).expect("the payload is the padded product");
        assert!(
            table
                .covering(Window {
                    north: 90.0,
                    south: 0.0,
                    west: -180.0,
                    east: 180.0,
                })
                .is_none()
        );
    }

    /// The registry raster's sentinel, so the sanitising the digest depends on is the sanitising the
    /// real reader does.
    const NODATA: f32 = -3.402_823e38;

    fn built(decimation: Decimation, rows: Vec<Vec<f32>>) -> (Vec<f64>, BuiltTable) {
        let source = Synthetic::new(*decimation.source(), NODATA, rows)
            .expect("the rows are the grid's shape");
        let mut payload = Vec::new();
        let built = build(source, decimation, &mut (), |row| {
            payload.extend_from_slice(row);
            Ok::<(), Infallible>(())
        })
        .expect("neither a synthetic source nor this sink can fail");
        (payload, built)
    }

    /// One ulp at `value`'s magnitude, which is the unit the decimation budget below is in.
    fn ulp(value: f64) -> f64 {
        value.abs().next_up() - value.abs()
    }

    /// A fixture cell, distinct at every position and mostly a repeating binary fraction, so no partial
    /// sum of these is exact in f64 and the budget below is a claim about arithmetic rather than about
    /// integers f64 happens to add exactly. The conversion is through `u16` because f32 holds every one
    /// of those exactly, so the inexactness is the third and nothing else.
    fn a_third_of(index: u32) -> f32 {
        f32::from(u16::try_from(index + 1).unwrap()) / 3.0
    }

    /// 3 and 4 both divide 12; only 3 divides 6.
    fn divisible() -> Grid {
        Grid::new(
            12,
            6,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            30.0,
            -30.0,
        )
        .expect("a 12 x 6 whole-globe grid is valid")
    }

    #[test]
    fn the_digest_is_fnv_1a_over_the_sanitised_cells() {
        // Computed outside this crate, from the three cells the sanitised row holds: 1.0 is
        // 0x3f80_0000, the sentinel becomes 0.0 and so contributes 0x0000_0000, and 2.5 is
        // 0x4020_0000. Folding those three words into the offset basis with the FNV prime gives the
        // value below. A digest checked against whatever this build produces would pin nothing, which
        // is the whole reason the definition is spelled out above.
        let grid = Grid::new(
            3,
            1,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            120.0,
            -180.0,
        )
        .expect("a 3 x 1 whole-globe grid is valid");
        let (_, built) = built(Decimation::none(grid), vec![vec![1.0, NODATA, 2.5]]);

        assert_eq!(built.digest, 0x3a5d_5e3b_082f_2fb7);
        assert_eq!(built.total, 3.5);
        assert_eq!(built.tallies.nodata, 1);
        assert_eq!(built.tallies.populated, 2);
    }

    #[test]
    fn a_factor_that_does_not_divide_the_grid_is_refused() {
        // 5 divides neither dimension; 4 divides the width alone, and a block half off the south edge
        // is not a smaller block — it is a row of cells the table has nowhere to put.
        assert_eq!(
            Decimation::new(divisible(), 5).unwrap_err(),
            TableError::Decimation {
                factor: 5,
                width: 12,
                height: 6
            }
        );
        assert_eq!(
            Decimation::new(divisible(), 4).unwrap_err(),
            TableError::Decimation {
                factor: 4,
                width: 12,
                height: 6
            }
        );
    }

    #[test]
    fn a_decimated_table_agrees_with_the_full_one_over_the_same_ground() {
        const FACTOR: u32 = 3;
        let grid = divisible();
        let cells: Vec<Vec<f32>> = (0..grid.height())
            .map(|row| {
                (0..grid.width())
                    .map(|col| a_third_of(row * grid.width() + col))
                    .collect()
            })
            .collect();

        let decimation = Decimation::new(grid, FACTOR).expect("3 divides both 12 and 6");
        let (fine_payload, _) = built(Decimation::none(grid), cells.clone());
        let (coarse_payload, coarse_built) = built(decimation, cells);

        assert_eq!(coarse_built.decimation.factor(), FACTOR);
        let fine = Table::new(grid, &fine_payload).expect("the build emits the padded product");
        let coarse = Table::new(*decimation.grid(), &coarse_payload)
            .expect("a decimated build emits the coarser padded product");

        // Every rectangle of the coarse table against the ground it stands for, half-open in both axes
        // so the factor multiplies the bounds directly. It holds bit for bit here, because a decimated
        // table is the full one subsampled and so was summed in the same order; the budget is what the
        // claim is, and the margin is what this construction happens to give.
        let (rows, cols) = (grid.height() / FACTOR, grid.width() / FACTOR);
        for r1 in 0..rows {
            for r2 in r1 + 1..=rows {
                for c1 in 0..cols {
                    for c2 in c1 + 1..=cols {
                        let coarse_band = RowBand::new(
                            coarse.grid().row(r1).unwrap(),
                            coarse.grid().row(r2 - 1).unwrap(),
                        );
                        let coarse_span = ColSpan::Through {
                            west: coarse.grid().col(c1).unwrap(),
                            east: coarse.grid().col(c2 - 1).unwrap(),
                        };
                        let fine_band = RowBand::new(
                            grid.row(r1 * FACTOR).unwrap(),
                            grid.row(r2 * FACTOR - 1).unwrap(),
                        );
                        let fine_span = ColSpan::Through {
                            west: grid.col(c1 * FACTOR).unwrap(),
                            east: grid.col(c2 * FACTOR - 1).unwrap(),
                        };

                        let over_ground = fine.population(fine_band, fine_span);
                        let difference =
                            (coarse.population(coarse_band, coarse_span) - over_ground).abs();
                        assert!(
                            difference <= 4.0 * ulp(over_ground),
                            "[{r1}, {r2}) x [{c1}, {c2}) decimated by {FACTOR} is out by {difference}"
                        );
                    }
                }
            }
        }
    }

    proptest! {
        /// Exactly, not within a tolerance: the cells are integers below 2^20, so every partial sum of
        /// them is exact in f64 and any difference here is the traversal — an index, the padding
        /// offset, or a seam — rather than arithmetic. The ulp budget is the other claim and
        /// is tested at full magnitude, where no direct sum exists to compare against.
        #[test]
        fn every_rectangle_matches_a_direct_sum(
            (width, height, cells) in (1u32..=5, 1u32..=4).prop_flat_map(|(width, height)| {
                (
                    Just(width),
                    Just(height),
                    prop::collection::vec(
                        prop::collection::vec(0u32..(1 << 20), width as usize),
                        height as usize,
                    ),
                )
            })
        ) {
            let grid = Grid::new(
                width,
                height,
                LatLon { lat: 90.0, lon: -180.0 },
                360.0 / f64::from(width),
                -180.0 / f64::from(height),
            )
            .expect("a whole-globe grid of any shape is valid");

            let values: Vec<Vec<f32>> = cells
                .iter()
                .map(|row| row.iter().map(|cell| *cell as f32).collect())
                .collect();
            let (payload, built) = built(Decimation::none(grid), values.clone());
            let table = Table::new(grid, &payload).expect("the build emits the padded product");

            let direct = |rows: &[u32], cols: &[u32]| -> f64 {
                let mut total = 0.0;
                for row in rows {
                    for col in cols {
                        total += f64::from(values[*row as usize][*col as usize]);
                    }
                }
                total
            };

            let every_row: Vec<u32> = (0..height).collect();
            let every_col: Vec<u32> = (0..width).collect();
            prop_assert_eq!(built.total, direct(&every_row, &every_col));

            for north in 0..height {
                for south in north..height {
                    let rows: Vec<u32> = (north..=south).collect();
                    let band = RowBand::new(grid.row(north).unwrap(), grid.row(south).unwrap());
                    prop_assert_eq!(
                        table.population(band, ColSpan::FullTurn),
                        direct(&rows, &every_col)
                    );

                    for west in 0..width {
                        for east in 0..width {
                            // west > east is the wrapped case, and on a grid one column wide it is
                            // also the full width: both are ordinary spans here rather than cases a
                            // caller has to spot.
                            let cols: Vec<u32> = if west <= east {
                                (west..=east).collect()
                            } else {
                                (west..width).chain(0..=east).collect()
                            };
                            let span = ColSpan::Through {
                                west: grid.col(west).unwrap(),
                                east: grid.col(east).unwrap(),
                            };
                            prop_assert_eq!(table.population(band, span), direct(&rows, &cols));
                        }
                    }
                }
            }
        }
    }
}
