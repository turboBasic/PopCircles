// Step 3 of application.md "Approach": the most populous circle of a fixed ground radius, as a branch
// and bound over a refining grid of candidate centres.
//
// The multiresolution grid the technique needs is the grid of *centres*, not a second summation table: a
// rectangle of candidate cells carries one bound for every centre in it, and a rectangle that survives
// halves. So what is here is the rectangle, the slack it spans, and the level loop over the two — the
// circles themselves are `circle::population`'s and the geometry is `kernel`'s.

use std::num::NonZeroU32;

use crate::bracket::Bracket;
use crate::circle;
use crate::geodesy::{RadiusKm, arc_km};
use crate::grid::{Col, Grid, Row};
use crate::kernel::{Kernel, KernelError};
use crate::progress::Progress;
use crate::table::{RowBand, Table};

/// Why a search could not run.
///
/// One variant, and no arm for the widened radius: [`RadiusKm::widened_by`] is total, because the slack a
/// block can carry is bounded by the sphere and cannot push a finite radius out of range. An error arm for
/// it would be one every caller has to handle and no input can produce.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum SearchError {
    #[error("the search could not build a kernel")]
    Kernel(#[from] KernelError),
}

/// The relative amount [`slack_km`] inflates its answer by, so that the figure it returns dominates the
/// mathematical bound rather than approximating it.
///
/// 32 ε is 7.1e-15, which at a 20 000 km bound is 0.14 µm of ground. That is enormous next to the handful
/// of rounding steps in the expression it corrects — two `to_radians`, a `cos`, a multiply and an add — and
/// negligible next to a 30 arc-second cell's ~900 m, so the only cell the inflation can newly admit is one
/// whose distance sits within 0.14 µm of the boundary. Which is the cell it exists to admit.
const SLACK_MARGIN: f64 = 32.0 * f64::EPSILON;

/// The row `index` names.
///
/// A panic rather than a `Result` for [`crate::table::Table::population`]'s reason: every index reaching
/// this lies between two rows the grid itself minted, so a miss is a wiring mistake in this module and
/// not an input a caller could do anything with.
fn row_of(grid: &Grid, index: u32) -> Row {
    match grid.row(index) {
        Some(row) => row,
        None => panic!("row {index} is not a row of a {}-row grid", grid.height()),
    }
}

/// The column `index` names, a stop for [`row_of`]'s reason.
fn col_of(grid: &Grid, index: u32) -> Col {
    match grid.col(index) {
        Some(col) => col,
        None => panic!(
            "column {index} is not a column of a {}-column grid",
            grid.width()
        ),
    }
}

/// A rectangle of candidate centres: the unit a bound speaks for, and the unit that halves.
///
/// The columns are `first` and `last` as indices rather than a [`crate::table::ColSpan`]'s compass pair,
/// and **a block never wraps the seam**: blocks tile the index space from column 0, so the last block of
/// a row is short rather than wrapped. The circles a block's centres carry do wrap, which is exactly why
/// the rectangle of centres must not — a wrapped block would have two spellings of "the cells I speak
/// for" and the bound would have to pick one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    rows: RowBand,
    first: Col,
    last: Col,
}

impl Block {
    /// Any pair of columns is a block, so the constructor orders them rather than refusing one order —
    /// [`RowBand::new`]'s reason, and it leaves nothing here for a caller to get wrong.
    #[must_use]
    pub fn new(rows: RowBand, a: Col, b: Col) -> Self {
        Self {
            rows,
            first: a.min(b),
            last: a.max(b),
        }
    }

    #[must_use]
    pub const fn rows(self) -> RowBand {
        self.rows
    }

    #[must_use]
    pub const fn first(self) -> Col {
        self.first
    }

    #[must_use]
    pub const fn last(self) -> Col {
        self.last
    }

    #[must_use]
    pub const fn is_cell(self) -> bool {
        self.rows.north().get() == self.rows.south().get() && self.first.get() == self.last.get()
    }

    /// The grid tiled into blocks `spacing` cells on a side, row-major, the last of each extent short
    /// rather than overhanging.
    ///
    /// # Panics
    /// If `grid` is not the grid the returned blocks are read against — it is, by construction here, and
    /// the panic is `row_of`'s.
    #[must_use]
    pub fn tile(grid: &Grid, spacing: NonZeroU32) -> Vec<Self> {
        // Saturating, because a spacing larger than the grid is a legal way to ask for one block and
        // `north + step` would otherwise wrap on a spacing near u32::MAX.
        let step = spacing.get();
        let mut blocks = Vec::new();
        let mut north = 0u32;
        while north < grid.height() {
            let south = north.saturating_add(step - 1).min(grid.height() - 1);
            let mut first = 0u32;
            while first < grid.width() {
                let last = first.saturating_add(step - 1).min(grid.width() - 1);
                blocks.push(Self::new(
                    RowBand::new(row_of(grid, north), row_of(grid, south)),
                    col_of(grid, first),
                    col_of(grid, last),
                ));
                first = last + 1;
            }
            north = south + 1;
        }
        blocks
    }

    /// The cell whose circle stands for the block's: the floor midpoint of each extent.
    ///
    /// Floor rather than round, so an extent of even length takes the north-western of its two middles.
    /// Which one is arbitrary; that it is the same one every run is not.
    ///
    /// # Panics
    /// If `grid` is not the grid this block's indices were minted by; `row_of` says why that is a stop.
    #[must_use]
    pub fn probe(self, grid: &Grid) -> (Row, Col) {
        let (north, south) = (self.rows.north().get(), self.rows.south().get());
        let (first, last) = (self.first.get(), self.last.get());
        (
            row_of(grid, north + (south - north) / 2),
            col_of(grid, first + (last - first) / 2),
        )
    }

    /// The block halved in each extent longer than one cell — four children, or two when one extent is
    /// already a single row or column, or none at all for a single cell.
    ///
    /// Nothing for a single cell is what terminates the refinement: every split shortens the longer
    /// extent, so a block reaches one cell in a bounded number of rounds and then stops producing work.
    ///
    /// # Panics
    /// If `grid` is not the grid this block's indices were minted by; `row_of` says why that is a stop.
    pub fn split(self, grid: &Grid) -> impl Iterator<Item = Self> {
        let halves = |low: u32, high: u32| {
            if low == high {
                vec![(low, high)]
            } else {
                let mid = low + (high - low) / 2;
                vec![(low, mid), (mid + 1, high)]
            }
        };

        let children = if self.is_cell() {
            Vec::new()
        } else {
            let rows = halves(self.rows.north().get(), self.rows.south().get());
            let cols = halves(self.first.get(), self.last.get());
            rows.into_iter()
                .flat_map(|(north, south)| {
                    cols.iter().map(move |&(first, last)| {
                        Self::new(
                            RowBand::new(row_of(grid, north), row_of(grid, south)),
                            col_of(grid, first),
                            col_of(grid, last),
                        )
                    })
                })
                .collect()
        };
        children.into_iter()
    }
}

/// A candidate centre and the population of its circle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub row: Row,
    pub col: Col,
    pub population: f64,
}

impl Candidate {
    /// The better of two candidates: more population wins, and equal population keeps the smaller
    /// `(row, col)`.
    ///
    /// A comparison rather than "keep the first one seen", and the difference is the answer's
    /// independence from the order candidates arrive in. Pruning changes that order with every initial
    /// spacing, so a rule reading it would give a different centre for the same raster and radius. This
    /// one is symmetric — `a.better(b)` and `b.better(a)` are the same candidate — which is what lets the
    /// search fold in whatever order the traversal happens to produce.
    #[must_use]
    pub fn better(self, other: Self) -> Self {
        // A population that is not a number takes neither comparison and falls through to the position,
        // so a table holding one yields a deterministic answer rather than an order-dependent one. It
        // cannot arise from a sanitised raster; what matters here is that it has no way to be undefined.
        if other.population > self.population {
            other
        } else if other.population < self.population {
            self
        } else if (other.row, other.col) < (self.row, self.col) {
            other
        } else {
            self
        }
    }
}

/// An upper bound on the ground distance from `block`'s probe to any cell centre in it.
///
/// Two hops and the triangle inequality on the sphere. Take the probe at `(φ₀, λ₀)` and a target cell
/// centre at `(φ, λ)`: go along the probe's own parallel to `(φ₀, λ)`, then along that meridian to
/// `(φ, λ)`. The first leg is a path of length `R · Δλ · cos φ₀` — the parallel is not a great circle away
/// from the equator, so the geodesic between its ends is no longer than the arc along it — and the second
/// leg is exactly `R · Δφ`. A geodesic between the ends is no longer than any path joining them, so the
/// sum bounds it.
///
/// The cosine is the **probe's** because the longitude leg stays at the probe's latitude. That is what
/// makes the bound tighten toward the poles rather than needing the block's worst latitude, and it is why
/// a block of the same index extent bounds a much shorter distance at 80° than at the equator.
///
/// Offsets are between **cell centres**, not out to the block's outer boundary: the candidates a bound
/// speaks for are cell centres, so measuring to a cell edge would loosen it for nothing.
///
/// The answer is inflated by `SLACK_MARGIN`, and the reason is that the inequality above is not strict
/// in two configurations. A block one column wide has `Δλ = 0`, so the two-hop path *is* the meridian
/// geodesic; a block one row tall on the equator has `Δφ = 0` and the parallel *is* a great circle. In
/// both the mathematical margin is zero, so a figure computed one ulp light would exclude a cell that is
/// genuinely within reach — and a block holding the maximum would be pruned.
///
/// Where the bound stops discriminating: once `radius + slack` reaches half the circumference the widened
/// circle is the whole sphere, every bound equals the raster's total and no block is ever pruned. That is
/// the ceiling on a useful initial spacing rather than a correctness limit.
///
/// # Panics
/// If `grid` is not the grid `block`'s indices were minted by; `row_of` says why that is a stop.
#[must_use]
pub fn slack_km(grid: &Grid, block: Block) -> f64 {
    let (probe_row, probe_col) = block.probe(grid);
    let probe_lat = grid.centre_lat(probe_row);

    let north = grid.centre_lat(block.rows().north());
    let south = grid.centre_lat(block.rows().south());
    let delta_lat_deg = (north - probe_lat).abs().max((south - probe_lat).abs());

    // The probe is a cell of its own block, so neither difference underflows.
    let west = probe_col.get() - block.first().get();
    let east = block.last().get() - probe_col.get();
    let delta_lon_deg = f64::from(west.max(east)) * grid.lon_step().abs();

    let two_hop_rad =
        delta_lat_deg.to_radians() + delta_lon_deg.to_radians() * probe_lat.to_radians().cos();
    arc_km(two_hop_rad) * (1.0 + SLACK_MARGIN)
}

/// The largest initial side whose conservative equatorial bound stays below half the circumference.
///
/// Bounding the longitude cosine by one makes this independent of a tile's latitude. Outward rounding
/// also covers the subtraction of rounded cell latitudes in [`slack_km`], even when a tiny step is lost
/// beside a nonzero origin. At or above half the circumference no side can discriminate: use one block
/// instead. Below it, a single-cell block always fits because its slack is exactly zero.
#[must_use]
pub fn initial_spacing(grid: &Grid, radius: RadiusKm) -> NonZeroU32 {
    let maximum = grid.width().max(grid.height());
    let half = arc_km(std::f64::consts::PI);
    if radius.km() >= half {
        return NonZeroU32::new(maximum).unwrap_or(NonZeroU32::MIN);
    }

    let fits = |side: u32| {
        let rows = side.min(grid.height()) / 2;
        let cols = side.min(grid.width()) / 2;
        if rows == 0 && cols == 0 {
            return true;
        }
        // Each centre multiplies an exact half-integer by the step and adds the origin, then slack
        // subtracts two centres. Eight epsilons of their absolute scale cover that rounding; the
        // next representable values also cover subnormal steps without a relative-error assumption.
        let latitude = if rows == 0 {
            0.0
        } else {
            let scale = (grid.origin().lat.abs()
                + (f64::from(grid.height()) * grid.lat_step().abs()).next_up())
            .next_up();
            let headroom = (8.0 * f64::EPSILON * scale).next_up();
            ((f64::from(rows) * grid.lat_step().abs()).next_up() + headroom).next_up()
        };
        let longitude = if cols == 0 {
            0.0
        } else {
            (f64::from(cols) * grid.lon_step().abs()).next_up()
        };
        let angle = (latitude.to_radians().next_up() + longitude.to_radians().next_up()).next_up();
        let slack = (arc_km(angle).next_up() * (1.0 + SLACK_MARGIN)).next_up();
        radius.widened_by(slack).km().next_up() < half
    };

    let mut low = 1;
    let mut high = maximum;
    while low < high {
        let distance = high - low;
        let middle = low + distance / 2 + distance % 2;
        if fits(middle) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    NonZeroU32::new(low).unwrap_or(NonZeroU32::MIN)
}

/// What a search did beside finding the answer, and the only window onto whether the bound is working.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SearchStats {
    pub levels: u32,
    pub blocks_examined: u64,
    pub blocks_pruned: u64,
    /// Blocks whose exact circle was evaluated — the ones the bound could not rule out.
    pub circles_evaluated: u64,
    /// How many kernels were constructed. It is here because it is what says the sort is buying the reuse
    /// it is there for: one per (row, radius) rather than one per block.
    pub kernels_built: u64,
}

/// The most populous circle a search found, and what "most" is exact to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MostPopulous {
    pub centre: Candidate,
    pub radius: RadiusKm,
    /// The population slack the search pruned with, in persons.
    ///
    /// Zero: refinement runs to single-cell blocks, the prune is strict and [`slack_km`]'s bound is
    /// inflated outward, so nothing here trades accuracy for time. What separates this figure from the
    /// true one is the table's own arithmetic — 4 ulp per rectangle query, about 4e-6 persons
    /// at a world total — which is the floor beneath this field rather than something it reports.
    ///
    /// A field rather than a constant because issue #6 asks the *result* to report it, and #7 reads
    /// results rather than this crate's constants.
    pub tolerance_persons: f64,
    pub stats: SearchStats,
}

/// One kernel, rebuilt only when the row or the radius changes.
///
/// A single entry rather than a map: the search walks a level's blocks sorted by probe row, so every block
/// of a row asks for the same kernel in succession and one entry catches all of it. A map would hold every
/// row's spans at once — 7128 rows of them at full resolution — to serve a locality the sort already gives.
#[derive(Debug)]
struct HeldKernel {
    grid: Grid,
    row: Row,
    radius_bits: u64,
    kernel: Kernel,
    built: u64,
}

impl HeldKernel {
    fn new(grid: Grid, row: Row, radius: RadiusKm) -> Result<Self, KernelError> {
        Ok(Self {
            grid,
            row,
            radius_bits: radius.km().to_bits(),
            kernel: Kernel::new(grid, row, radius)?,
            built: 1,
        })
    }

    /// The radius is compared by bits rather than by value, which is the comparison that matches what the
    /// cache is for: the widened radius is recomputed per block, and two blocks of the same row and the
    /// same extent produce the same bits — exactly when the kernel is reusable.
    fn get(&mut self, row: Row, radius: RadiusKm) -> Result<&Kernel, KernelError> {
        let bits = radius.km().to_bits();
        if self.row != row || self.radius_bits != bits {
            self.kernel = Kernel::new(self.grid, row, radius)?;
            self.row = row;
            self.radius_bits = bits;
            self.built += 1;
        }
        Ok(&self.kernel)
    }
}

/// The most populous circle of ground radius `radius` centred on a cell of the table's grid.
///
/// The maximum is over the **table's** cell centres, of the population the table holds within the radius.
/// On a whole-globe table that is the globe-wide answer; on a table spanning a band of latitude it is that
/// band's, because [`Kernel`] clips a cap at a grid's edge rather than refusing it. The claim is worded
/// that way rather than as "on the globe" so a band table is answered honestly instead of refused by a
/// guard the layer below does not have.
///
/// `None` derives the first level's block side from this radius and grid with [`initial_spacing`].
/// `Some(side)` uses that exact side, even beyond the automatic ceiling. Spacing changes runtime only:
/// every bound is admissible and refinement runs to single cells; a side of one scans every centre.
///
/// `progress` is advanced once per block, `(blocks done, blocks in this level)`, both absolute within the
/// level being walked. Per level rather than overall because the number of levels is not known until the
/// search runs, so a global total would be a figure this function had to revise.
///
/// # Errors
/// [`SearchError::Kernel`], the only variant there is, when the table's grid has no kernels at all —
/// which is a grid whose columns do not close.
///
/// # Panics
/// If the table's grid yields no block to examine. A [`Grid`] has at least one cell and the first block
/// examined cannot be pruned — there is no incumbent to prune it against — so this is an invariant of the
/// two rather than a case a caller can reach.
pub fn most_populous<P: Progress>(
    table: &Table<'_>,
    radius: RadiusKm,
    spacing: Option<NonZeroU32>,
    progress: &mut P,
) -> Result<MostPopulous, SearchError> {
    let grid = *table.grid();
    let mut level = Block::tile(
        &grid,
        spacing.unwrap_or_else(|| initial_spacing(&grid, radius)),
    );
    let mut best: Option<Candidate> = None;
    let mut stats = SearchStats::default();

    let seed = grid.middle_row();
    let mut exact = HeldKernel::new(grid, seed, radius)?;
    let mut widest = HeldKernel::new(grid, seed, radius)?;

    while !level.is_empty() {
        // The traversal decision, and it buys two things at once: every kernel band slides north to south
        // so each table row is read forward, and consecutive blocks share a probe row so the kernel above
        // is reused rather than rebuilt. Splitting yields children in each parent's order, which is not
        // the level's order, so the sort is per level rather than once.
        level.sort_unstable_by_key(|block| {
            let (row, col) = block.probe(&grid);
            (row.get(), col.get())
        });
        stats.levels += 1;

        // Box 7's second granularity. Kernel placement gets no pair of its own: there is no discrete
        // placement step to open one around — `HeldKernel::get` builds lazily inside the block loop below,
        // 15 891 times in the measured run — so what that entry wanted rides on this pair's end record as
        // the delta over the level.
        let mut bracket = Bracket::open(module_path!(), format!("level {}", stats.levels));
        // Read from the two counters and not from `stats.kernels_built`, which is assigned once after this
        // loop exits and so is zero at every level. They stand at 2 before the first level opens, one seed
        // kernel each, which is what a delta keeps the first level from claiming.
        let kernels_before = exact.built + widest.built;

        // Saturating rather than casting, so a host where a Vec's length does not fit a u64 reports a
        // capped total instead of a wrapped one.
        let total = u64::try_from(level.len()).unwrap_or(u64::MAX);
        let mut done = 0u64;
        let mut survivors: Vec<Block> = Vec::new();

        for block in &level {
            let (probe_row, probe_col) = block.probe(&grid);
            let widened = radius.widened_by(slack_km(&grid, *block));
            let ceiling = circle::population(table, widest.get(probe_row, widened)?, probe_col);
            stats.blocks_examined += 1;

            // Strictly under, never equal. A block whose bound merely ties the incumbent may hold a
            // centre that ties it too and wins on position, and that centre is the answer — so a `<=`
            // here would drop the tie before the tie-break ever saw it, and the result would depend on
            // the spacing. Under a strict incumbent, every centre in the block is beaten outright.
            if best.is_some_and(|held| ceiling < held.population) {
                stats.blocks_pruned += 1;
            } else {
                let population =
                    circle::population(table, exact.get(probe_row, radius)?, probe_col);
                let candidate = Candidate {
                    row: probe_row,
                    col: probe_col,
                    population,
                };
                best = Some(match best {
                    Some(held) => held.better(candidate),
                    None => candidate,
                });
                stats.circles_evaluated += 1;
                survivors.push(*block);
            }

            done += 1;
            progress.advance(done, total);
        }

        bracket.figure("kernels", exact.built + widest.built - kernels_before);

        level = survivors
            .into_iter()
            .flat_map(|block| block.split(&grid).collect::<Vec<Block>>())
            .collect();
    }

    stats.kernels_built = exact.built + widest.built;
    match best {
        Some(centre) => Ok(MostPopulous {
            centre,
            radius,
            tolerance_persons: 0.0,
            stats,
        }),
        None => panic!(
            "a grid has at least one cell, so a tiling has at least one block and the first cannot be \
             pruned"
        ),
    }
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows both in tests. float_cmp is for the exactly-zero and tightness
// assertions, where the value being exactly what it is is the property.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss
)]
mod tests {
    use std::collections::BTreeSet;
    use std::convert::Infallible;

    use proptest::prelude::*;

    use super::*;
    use crate::geodesy::{LatLon, great_circle_km};
    use crate::kernel::Span;
    use crate::raster::Synthetic;
    use crate::table::{ColSpan, Decimation, build};

    const WIDTH: u32 = 36;
    const HEIGHT: u32 = 18;

    /// Ten degrees a side, the same whole-globe shape `circle.rs` measures against, and small enough
    /// that a tiling can be compared against every cell of the grid.
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

    fn spacing(cells: u32) -> NonZeroU32 {
        NonZeroU32::new(cells).expect("a fixture spacing is not zero")
    }

    fn spacing_ceiling(grid: &Grid, side: u32, radius: RadiusKm) -> bool {
        let row_offset = side.min(grid.height()) / 2;
        let col_offset = side.min(grid.width()) / 2;
        if row_offset == 0 && col_offset == 0 {
            return true;
        }
        let lat = if row_offset == 0 {
            0.0
        } else {
            let extent = (f64::from(grid.height()) * grid.lat_step().abs()).next_up();
            let scale = (grid.origin().lat.abs() + extent).next_up();
            let error = (8.0 * f64::EPSILON * scale).next_up();
            ((f64::from(row_offset) * grid.lat_step().abs()).next_up() + error).next_up()
        };
        let lon = if col_offset == 0 {
            0.0
        } else {
            (f64::from(col_offset) * grid.lon_step().abs()).next_up()
        };
        let angle = (lat.to_radians().next_up() + lon.to_radians().next_up()).next_up();
        let bound = (arc_km(angle).next_up() * (1.0 + 32.0 * f64::EPSILON)).next_up();
        (radius.km() + bound).next_up() < arc_km(std::f64::consts::PI)
    }

    fn check_initial_spacing(grid: &Grid, radius: RadiusKm) {
        let half = arc_km(std::f64::consts::PI);
        let selected = initial_spacing(grid, radius);
        let maximum = grid.width().max(grid.height());
        if radius.km() >= half {
            assert_eq!(selected.get(), maximum);
            assert_eq!(Block::tile(grid, selected).len(), 1);
            return;
        }
        let expected = (1..=maximum)
            .filter(|&side| spacing_ceiling(grid, side, radius))
            .max()
            .unwrap();
        assert_eq!(selected.get(), expected, "{grid:?}, {radius:?}");
        for block in Block::tile(grid, selected) {
            assert!(
                radius.widened_by(slack_km(grid, block)).km() < half,
                "{grid:?}, {radius:?}, {block:?}"
            );
        }
    }

    #[test]
    fn initial_spacing_is_the_largest_feasible_side_on_small_grids() {
        let half = arc_km(std::f64::consts::PI);
        let fine = Grid::new(
            72,
            36,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            5.0,
            -5.0,
        )
        .unwrap();
        let mut shapes = vec![grid(), equatorial_grid(), fine];
        for (width, height, lat, lon_step, lat_step) in [
            (24, 5, 70.0, -15.0, -2.0),
            (1, 8, 90.0, 360.0, -20.0),
            (8, 1, 10.0, 45.0, -20.0),
            (1, 1, 90.0, 360.0, -180.0),
            (7, 9, 60.0, 1e-14, -1e-14),
            (7, 9, -60.0, f64::MIN_POSITIVE, -f64::MIN_POSITIVE),
        ] {
            shapes.push(
                Grid::new(width, height, LatLon { lat, lon: 0.0 }, lon_step, lat_step).unwrap(),
            );
        }
        for grid in &shapes {
            for km in [
                0.0,
                200.0,
                8000.0,
                half - 1e-10,
                half.next_down(),
                half,
                half.next_up(),
                f64::MAX,
            ] {
                check_initial_spacing(grid, radius(km));
            }
        }
        assert_ne!(
            initial_spacing(&grid(), radius(200.0)),
            initial_spacing(&grid(), radius(8000.0))
        );
        assert_ne!(
            initial_spacing(&shapes[0], radius(8000.0)),
            initial_spacing(&shapes[2], radius(8000.0))
        );
    }

    #[test]
    fn initial_spacing_handles_both_sides_of_each_ceiling_boundary() {
        let grid = grid();
        let half = arc_km(std::f64::consts::PI);
        for side in 2..=grid.width() {
            let angle = (f64::from(side.min(grid.height()) / 2) * grid.lat_step().abs()
                + f64::from(side / 2) * grid.lon_step().abs())
            .to_radians();
            let boundary = half - arc_km(angle) * (1.0 + SLACK_MARGIN);
            if boundary >= 0.0 {
                for km in [
                    boundary - 1e-8,
                    boundary.next_down(),
                    boundary,
                    boundary.next_up(),
                    boundary + 1e-8,
                ] {
                    check_initial_spacing(&grid, radius(km.max(0.0)));
                }
            }
        }
        assert_eq!(initial_spacing(&grid, radius(half.next_down())).get(), 1);
    }

    #[test]
    fn initial_spacing_searches_u32_dimensions_without_allocating_tiles() {
        let grid = Grid::new(
            u32::MAX,
            1,
            LatLon {
                lat: 0.0,
                lon: -180.0,
            },
            360.0 / f64::from(u32::MAX),
            -1.0,
        )
        .unwrap();
        let selected = initial_spacing(&grid, radius(8000.0)).get();
        assert!(spacing_ceiling(&grid, selected, radius(8000.0)));
        assert!(!spacing_ceiling(&grid, selected + 1, radius(8000.0)));
        assert_eq!(initial_spacing(&grid, radius(0.0)).get(), u32::MAX);
        assert_eq!(initial_spacing(&grid, radius(f64::MAX)).get(), u32::MAX);
    }

    /// Every cell a block speaks for, expanded.
    fn cells_of(block: Block) -> Vec<(u32, u32)> {
        (block.rows().north().get()..=block.rows().south().get())
            .flat_map(|row| (block.first().get()..=block.last().get()).map(move |col| (row, col)))
            .collect()
    }

    fn every_cell() -> BTreeSet<(u32, u32)> {
        (0..HEIGHT)
            .flat_map(|row| (0..WIDTH).map(move |col| (row, col)))
            .collect()
    }

    #[test]
    fn a_tiling_covers_every_cell_exactly_once() {
        // Both halves matter and neither implies the other: the set says nothing was missed, and the
        // count says nothing was covered twice. Spacings 5 and 18 are the ones that do not divide the
        // grid evenly in one direction or the other, which is where a short final block is either
        // right or an off-by-one.
        let grid = grid();
        for cells in [1u32, 4, 5, 18, 40] {
            let blocks = Block::tile(&grid, spacing(cells));
            let covered: Vec<(u32, u32)> = blocks.iter().copied().flat_map(cells_of).collect();

            assert_eq!(
                covered.iter().copied().collect::<BTreeSet<(u32, u32)>>(),
                every_cell(),
                "spacing {cells} misses or invents a cell"
            );
            assert_eq!(
                covered.len(),
                (WIDTH * HEIGHT) as usize,
                "spacing {cells} covers a cell twice"
            );
        }
    }

    #[test]
    fn a_spacing_past_the_grid_is_one_block() {
        // The saturating arm, and the shape a caller asking for "the whole globe as one candidate" gets.
        let grid = grid();
        let blocks = Block::tile(&grid, spacing(u32::MAX));
        assert_eq!(blocks.len(), 1);
        assert_eq!(cells_of(blocks[0]).len(), (WIDTH * HEIGHT) as usize);
    }

    #[test]
    fn a_single_cell_block_splits_into_nothing() {
        // The termination condition, asserted directly rather than inferred from the round count below.
        let grid = grid();
        for block in Block::tile(&grid, spacing(1)) {
            assert!(block.is_cell());
            assert_eq!(block.split(&grid).count(), 0);
        }
    }

    #[test]
    fn splitting_reaches_single_cells_in_five_rounds_and_loses_no_cell() {
        // An extent of 18 halves 18, 9, 5, 3, 2, 1 — five rounds — and the whole-globe fixture at
        // spacing 18 is two such blocks. The round count is stated because an off-by-one in `halves`
        // that still terminates would take six, and because a `mid` computed the other way round would
        // leave a child of the parent's own length and never terminate at all.
        let grid = grid();
        let mut level = Block::tile(&grid, spacing(18));
        let mut rounds = 0;

        while !level.iter().all(|block| block.is_cell()) {
            level = level
                .iter()
                .flat_map(|block| {
                    if block.is_cell() {
                        vec![*block]
                    } else {
                        block.split(&grid).collect()
                    }
                })
                .collect();
            rounds += 1;
            assert!(rounds <= 8, "splitting is not converging: {rounds} rounds");
        }

        assert_eq!(rounds, 5);
        // Splitting is a partition at every round, not merely a shrinking: the leaves are the grid.
        let leaves: Vec<(u32, u32)> = level.iter().copied().flat_map(cells_of).collect();
        assert_eq!(
            leaves.iter().copied().collect::<BTreeSet<(u32, u32)>>(),
            every_cell()
        );
        assert_eq!(leaves.len(), (WIDTH * HEIGHT) as usize);
    }

    #[test]
    fn a_probe_on_an_even_extent_is_the_north_western_middle() {
        // Four rows and four columns, so both extents have two middles and the choice is visible. Rows
        // 1 and 2 are the middles of 0..3, and 1 is the northern.
        let grid = grid();
        let block = Block::new(
            RowBand::new(row_of(&grid, 0), row_of(&grid, 3)),
            col_of(&grid, 0),
            col_of(&grid, 3),
        );
        let (row, col) = block.probe(&grid);
        assert_eq!((row.get(), col.get()), (1, 1));

        // And an odd extent has one middle, which is the case the floor must not shift.
        let odd = Block::new(
            RowBand::new(row_of(&grid, 4), row_of(&grid, 8)),
            col_of(&grid, 10),
            col_of(&grid, 14),
        );
        let (row, col) = odd.probe(&grid);
        assert_eq!((row.get(), col.get()), (6, 12));
    }

    #[test]
    fn a_probe_is_a_cell_of_the_block_it_stands_for() {
        // The property every bound rests on and the one a midpoint formula can lose at a short final
        // block: the probe is inside its own rectangle.
        let grid = grid();
        for cells in [1u32, 4, 5, 18] {
            for block in Block::tile(&grid, spacing(cells)) {
                let (row, col) = block.probe(&grid);
                assert!(
                    cells_of(block).contains(&(row.get(), col.get())),
                    "spacing {cells}: probe {:?} is outside its block",
                    (row.get(), col.get())
                );
            }
        }
    }

    /// A whole globe with a row centred on the equator, which needs an odd row count: centres sit at
    /// `90 − (r + 0.5)·20`, so row 4 is at 0.0 exactly. That row is one of the two configurations where
    /// the two-hop bound is not strict.
    fn equatorial_grid() -> Grid {
        Grid::new(
            WIDTH,
            9,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            10.0,
            -20.0,
        )
        .expect("a 36 x 9 whole-globe grid is valid")
    }

    fn block_of(grid: &Grid, north: u32, south: u32, first: u32, last: u32) -> Block {
        Block::new(
            RowBand::new(row_of(grid, north), row_of(grid, south)),
            col_of(grid, first),
            col_of(grid, last),
        )
    }

    /// The furthest cell of a block from its probe, by the distance test and nothing else.
    fn furthest_km(grid: &Grid, block: Block) -> f64 {
        let (probe_row, probe_col) = block.probe(grid);
        let from = grid.centre_of(probe_row, probe_col);
        cells_of(block)
            .into_iter()
            .map(|(row, col)| {
                great_circle_km(from, grid.centre_of(row_of(grid, row), col_of(grid, col)))
            })
            .fold(0.0f64, f64::max)
    }

    #[test]
    fn the_slack_dominates_every_cell_of_every_block() {
        // Exhaustive over cells, which is the only form of this claim worth having: a bound checked at a
        // corner is a bound checked where the author expected the maximum to be.
        for grid in [grid(), equatorial_grid()] {
            for cells in [4u32, 5, 18] {
                for block in Block::tile(&grid, spacing(cells)) {
                    let slack = slack_km(&grid, block);
                    let (probe_row, probe_col) = block.probe(&grid);
                    let from = grid.centre_of(probe_row, probe_col);

                    for (row, col) in cells_of(block) {
                        let to = grid.centre_of(row_of(&grid, row), col_of(&grid, col));
                        let actual = great_circle_km(from, to);
                        assert!(
                            slack >= actual,
                            "spacing {cells}, block {block:?}, cell {:?}: slack {slack} is under \
                             {actual}",
                            (row, col)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_single_cell_block_has_no_slack() {
        // Exactly zero rather than nearly: both offsets are zero, so the margin has nothing to inflate
        // and the widened radius a search builds from it is the radius itself.
        let grid = grid();
        for block in Block::tile(&grid, spacing(1)) {
            assert_eq!(slack_km(&grid, block), 0.0);
        }
    }

    #[test]
    fn a_one_column_block_is_the_meridian_arc_and_holds_only_by_the_margin() {
        // The first zero-margin configuration: with no longitude offset the two-hop path *is* the
        // meridian geodesic, so the bound is tight and the inequality survives on the inflation alone.
        // Tightness is asserted as well as the inequality — that is what says this case still exercises
        // the margin, rather than having drifted into a regime where slack to spare carries it.
        let grid = grid();
        for column in [0u32, 17, 35] {
            for (north, south) in [(0u32, 5u32), (6, 11), (12, 17), (0, 17)] {
                let block = block_of(&grid, north, south, column, column);
                let slack = slack_km(&grid, block);
                let furthest = furthest_km(&grid, block);

                assert!(slack >= furthest, "{block:?}: {slack} under {furthest}");
                assert!(
                    (slack - furthest) / furthest < 1e-12,
                    "{block:?}: the bound is no longer tight here, {slack} against {furthest}"
                );
            }
        }
    }

    #[test]
    fn a_one_row_block_on_the_equator_is_the_parallel_and_holds_only_by_the_margin() {
        // The second: on the equator the parallel is a great circle, so a longitude-only offset is again
        // exactly the geodesic. Row 4 of the equatorial fixture is latitude 0.
        let grid = equatorial_grid();
        assert_eq!(grid.centre_lat(row_of(&grid, 4)), 0.0);

        for (first, last) in [(0u32, 3u32), (10, 17), (0, 17)] {
            let block = block_of(&grid, 4, 4, first, last);
            let slack = slack_km(&grid, block);
            let furthest = furthest_km(&grid, block);

            assert!(slack >= furthest, "{block:?}: {slack} under {furthest}");
            assert!(
                (slack - furthest) / furthest < 1e-12,
                "{block:?}: the bound is no longer tight here, {slack} against {furthest}"
            );
        }
    }

    #[test]
    fn the_slack_shrinks_toward_the_pole_for_the_same_index_extent() {
        // What the probe's own cosine buys, and the reason the factor is not the block's worst latitude:
        // the same rectangle of indices bounds a far shorter distance beside the pole than at the
        // equator. Asserted as an ordering rather than a figure.
        let grid = equatorial_grid();
        let equator = slack_km(&grid, block_of(&grid, 4, 4, 0, 8));
        let polar = slack_km(&grid, block_of(&grid, 0, 0, 0, 8));
        assert!(polar < equator, "{polar} is not under {equator}");
    }

    proptest! {
        /// The same claim as the exhaustive test over a domain nobody chose: any rectangle of either
        /// fixture. Single-column and single-row blocks are inside it, so are blocks wider than half the
        /// grid, and so are row 0 and the row beside the southern edge — the configurations where the
        /// bound is tight or the geometry is worst are drawn rather than listed.
        #[test]
        fn the_slack_dominates_over_any_rectangle(
            equatorial in proptest::bool::ANY,
            north in 0u32..18,
            height in 1u32..18,
            first in 0u32..WIDTH,
            width in 1u32..WIDTH,
        ) {
            let grid = if equatorial { equatorial_grid() } else { grid() };
            let north = north.min(grid.height() - 1);
            let south = (north + height - 1).min(grid.height() - 1);
            let last = (first + width - 1).min(WIDTH - 1);

            let block = block_of(&grid, north, south, first, last);
            let slack = slack_km(&grid, block);
            prop_assert!(
                slack >= furthest_km(&grid, block),
                "{:?}: slack {} under {}",
                block,
                slack,
                furthest_km(&grid, block)
            );
        }
    }

    /// Closing in longitude, thirty degrees of latitude: `kernel.rs:539`'s shape, where a cap is clipped
    /// at the grid's edge rather than refused. It is here to exercise the clipping step of the bound's
    /// argument below.
    fn band_grid() -> Grid {
        Grid::new(
            360,
            30,
            LatLon {
                lat: 60.0,
                lon: -180.0,
            },
            1.0,
            -1.0,
        )
        .expect("a band grid that closes in longitude is valid")
    }

    /// The registry raster's sentinel, so a fixture reaches the table by the path a real raster takes.
    const NODATA: f32 = -3.402_823e38;

    fn radius(km: f64) -> RadiusKm {
        RadiusKm::new(km).expect("a fixture radius is a length")
    }

    /// The padded payload a real build emits over a grid, from a cell function: the fixture is then the
    /// path the search uses rather than a second construction of it.
    fn payload_over(grid: &Grid, cell: impl Fn(u32, u32) -> f32) -> Vec<f64> {
        let rows: Vec<Vec<f32>> = (0..grid.height())
            .map(|row| (0..grid.width()).map(|col| cell(row, col)).collect())
            .collect();
        let source = Synthetic::new(*grid, NODATA, rows).expect("the rows are the grid's shape");
        let mut payload = Vec::new();
        build(source, Decimation::none(*grid), &mut (), |row| {
            payload.extend_from_slice(row);
            Ok::<(), Infallible>(())
        })
        .expect("neither a synthetic source nor this sink can fail");
        payload
    }

    #[test]
    fn no_centre_in_a_block_beats_its_probes_widened_circle() {
        // Box 1 of issue #6, and the claim the whole search rests on. It is separate from the slack's own
        // test rather than folded into it so that a failure localises: that one is a claim about distance,
        // this one adds containment and the monotonicity #5's proptest pinned, and one test covering all
        // three would not say which had moved.
        //
        // The band grid is here for the step the two-hop proof does not cover. `Kernel` clips a cap at a
        // grid's northern or southern edge rather than refusing it, so on that grid both circles in the
        // inequality are intersections with the grid — and intersecting both sides of a containment with
        // the same set preserves it. Which is why the bound holds on every grid `Kernel` accepts, and not
        // only on a whole globe.
        for (grid, spacings) in [(grid(), vec![4u32, 18]), (band_grid(), vec![90u32])] {
            let payload = payload_over(&grid, distinct(&grid));
            let table = Table::new(grid, &payload).expect("the build emits the padded product");

            for radius_km in [1500.0, 8000.0] {
                // One kernel per row, which is the reuse the type exists for and what keeps this
                // exhaustive test to seconds rather than minutes.
                let row_kernels: Vec<Kernel> = grid
                    .rows()
                    .map(|row| {
                        Kernel::new(grid, row, radius(radius_km)).expect("a grid that closes")
                    })
                    .collect();

                for cells in &spacings {
                    for block in Block::tile(&grid, spacing(*cells)) {
                        let (probe_row, probe_col) = block.probe(&grid);
                        let widened = radius(radius_km + slack_km(&grid, block));
                        let bound_kernel =
                            Kernel::new(grid, probe_row, widened).expect("a grid that closes");
                        let bound = circle::population(&table, &bound_kernel, probe_col);

                        for (row, col) in cells_of(block) {
                            let here = circle::population(
                                &table,
                                &row_kernels[row as usize],
                                col_of(&grid, col),
                            );
                            assert!(
                                here <= bound,
                                "{}x{} grid, spacing {cells}, radius {radius_km} km, block \
                                 {block:?}, cell {:?}: {here} beats the bound {bound}",
                                grid.width(),
                                grid.height(),
                                (row, col)
                            );
                        }
                    }
                }
            }
        }
    }

    /// Distinct at every position and no larger than 10 800, so every partial sum in the table and in the
    /// fold is exact in f64 and a cell counted twice moves a total.
    fn distinct(grid: &Grid) -> impl Fn(u32, u32) -> f32 + use<> {
        let width = grid.width();
        move |row, col| (row * width + col + 1) as f32
    }

    /// The maximum over every cell of the grid, by `circle::population` and 3.1's rule. #5 pinned that
    /// function against a whole-grid distance scan, so this reference inherits a checked answer rather
    /// than being a third fold of the same sum.
    fn brute_force(table: &Table<'_>, grid: &Grid, radius_km: f64) -> Candidate {
        let mut best: Option<Candidate> = None;
        for row in grid.rows() {
            let kernel = Kernel::new(*grid, row, radius(radius_km)).expect("a grid that closes");
            for col in grid.cols() {
                let candidate = Candidate {
                    row,
                    col,
                    population: circle::population(table, &kernel, col),
                };
                best = Some(match best {
                    Some(held) => held.better(candidate),
                    None => candidate,
                });
            }
        }
        best.expect("the grid has cells")
    }

    fn search(table: &Table<'_>, radius_km: f64, cells: u32) -> MostPopulous {
        most_populous(table, radius(radius_km), Some(spacing(cells)), &mut ())
            .expect("a whole-globe grid and a radius far from the overflow")
    }

    #[test]
    fn the_search_finds_what_an_exhaustive_scan_finds() {
        // The centre and the population, both by `assert_eq!`: a search that found a near-miss centre
        // would report a plausible population, and only the pair pins it.
        let grid = grid();
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        for radius_km in [1500.0, 8000.0] {
            let expected = brute_force(&table, &grid, radius_km);
            for cells in [3u32, 5, 18] {
                let found = search(&table, radius_km, cells).centre;
                assert_eq!(
                    (found.row.get(), found.col.get(), found.population),
                    (expected.row.get(), expected.col.get(), expected.population),
                    "radius {radius_km} km, spacing {cells}"
                );
            }
        }
    }

    #[test]
    fn the_answer_does_not_depend_on_the_initial_spacing() {
        // Spacing 1 is a brute force by construction — every block is one cell, so every slack is zero and
        // every bound is the exact circle — which makes this both a spacing check and the strict prune's
        // own test: a `<=` prune drops ties, and a dropped tie shows up here as a different centre at a
        // different spacing.
        let grid = grid();
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        for radius_km in [1500.0, 8000.0] {
            let reference = search(&table, radius_km, 1).centre;
            for cells in [Some(2u32), Some(3), Some(5), Some(18), None] {
                let found = most_populous(&table, radius(radius_km), cells.map(spacing), &mut ())
                    .unwrap()
                    .centre;
                assert_eq!(
                    (found.row, found.col, found.population.to_bits()),
                    (reference.row, reference.col, reference.population.to_bits()),
                    "radius {radius_km} km, spacing {cells:?}"
                );
            }
        }
    }

    #[test]
    fn automatic_statistics_match_the_exact_derived_side() {
        let grid = grid();
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).unwrap();
        for km in [0.0, 200.0, 3300.0, 8000.0, f64::MAX] {
            let radius = radius(km);
            let automatic = most_populous(&table, radius, None, &mut ()).unwrap();
            let explicit = most_populous(
                &table,
                radius,
                Some(initial_spacing(&grid, radius)),
                &mut (),
            )
            .unwrap();
            assert_eq!(automatic.stats, explicit.stats);
            assert_eq!(
                (
                    automatic.centre.row,
                    automatic.centre.col,
                    automatic.centre.population.to_bits()
                ),
                (
                    explicit.centre.row,
                    explicit.centre.col,
                    explicit.centre.population.to_bits()
                )
            );
            assert_eq!(
                automatic.radius.km().to_bits(),
                explicit.radius.km().to_bits()
            );
            assert_eq!(automatic.tolerance_persons.to_bits(), 0.0f64.to_bits());
        }
    }

    #[test]
    fn an_all_zero_raster_answers_the_north_west_cell() {
        // Every circle holds nothing, so every candidate ties and the tie-break decides alone. This is the
        // case an incumbent seeded with a population of zero gets wrong: the first block's own probe would
        // prune the rest of the globe and the answer would be that probe rather than cell (0, 0).
        let grid = grid();
        let payload = payload_over(&grid, |_, _| 0.0);
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        for cells in [Some(1u32), Some(4), Some(18), None] {
            let found = most_populous(&table, radius(1500.0), cells.map(spacing), &mut ())
                .unwrap()
                .centre;
            assert_eq!(
                (found.row.get(), found.col.get(), found.population),
                (0, 0, 0.0),
                "spacing {cells:?}"
            );
        }
    }

    #[test]
    fn the_largest_radius_there_is_answers_the_whole_table() {
        // Absurd and legal, and the case that says widening cannot overflow. A block's slack is bounded by
        // the sphere at about 60 000 km, while the gap between `f64::MAX` and its neighbour is 2e292, so
        // the widened radius is `f64::MAX` again rather than an infinity — which is why the search has no
        // error arm for it. Every circle is the world, so every candidate ties and cell (0, 0) wins.
        let grid = grid();
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");
        let (rows, cols) = table.whole();

        let found = search(&table, f64::MAX, 4).centre;
        assert_eq!(
            (found.row.get(), found.col.get(), found.population),
            (0, 0, table.population(rows, cols))
        );
    }

    #[test]
    fn a_grid_whose_columns_do_not_close_has_no_search() {
        let window = Grid::new(
            60,
            60,
            LatLon {
                lat: 60.0,
                lon: -10.0,
            },
            0.5,
            -0.5,
        )
        .expect("a window grid is valid");
        let payload = payload_over(&window, distinct(&window));
        let table = Table::new(window, &payload).expect("the build emits the padded product");

        assert!(matches!(
            most_populous(&table, radius(500.0), Some(spacing(4)), &mut ()),
            Err(SearchError::Kernel(KernelError::ColumnsDoNotClose { .. }))
        ));
    }

    #[test]
    fn the_same_search_twice_gives_the_same_bits() {
        let grid = grid();
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let (first, second) = (search(&table, 4000.0, 5), search(&table, 4000.0, 5));
        assert_eq!(
            first.centre.population.to_bits(),
            second.centre.population.to_bits()
        );
        assert_eq!(first.centre.row, second.centre.row);
        assert_eq!(first.centre.col, second.centre.col);
        assert_eq!(first.stats, second.stats);
    }

    #[derive(Debug, Default)]
    struct Reported {
        calls: Vec<(u64, u64)>,
    }

    impl Progress for Reported {
        fn advance(&mut self, done: u64, total: u64) {
            self.calls.push((done, total));
        }
    }

    #[test]
    fn the_sink_sees_one_finished_pair_per_level() {
        // The per-level contract, asserted as the runs it produces: a level's calls start at one and end
        // at (n, n), and the number of runs is the number of levels the stats report. A level counter that
        // drifted from the reporting would fail here rather than being invisible.
        let grid = grid();
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let mut sink = Reported::default();
        let result = most_populous(&table, radius(2000.0), Some(spacing(5)), &mut sink)
            .expect("a whole-globe grid");

        let mut runs: Vec<Vec<(u64, u64)>> = Vec::new();
        for call in sink.calls {
            if call.0 == 1 {
                runs.push(Vec::new());
            }
            runs.last_mut().expect("a run opens at one").push(call);
        }

        assert_eq!(u32::try_from(runs.len()).unwrap(), result.stats.levels);
        for run in runs {
            let (done, total) = *run.last().expect("a run is not empty");
            assert_eq!(done, total);
            assert_eq!(u64::try_from(run.len()).unwrap(), total);
        }
    }

    #[test]
    fn one_kernel_serves_a_whole_row_of_blocks() {
        // The reuse claim, and the reason `kernels_built` is on the result at all: two kernels per probe
        // row per level is the ceiling the sort makes possible, and a build per block would exceed it by
        // the width of the grid.
        let grid = grid();
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let result = search(&table, 2000.0, 5);
        let ceiling = 2 * u64::from(result.stats.levels) * u64::from(grid.height());
        assert!(
            result.stats.kernels_built <= ceiling,
            "{} kernels for {} levels, past {ceiling}",
            result.stats.kernels_built,
            result.stats.levels
        );
        assert!(result.stats.kernels_built < result.stats.blocks_examined);
    }

    #[test]
    fn the_result_reports_a_tolerance_of_zero() {
        // Box 4 of issue #6: the tolerance is a documented choice and the result reports it. Zero is that
        // choice, and the search is exact over cell centres because of it.
        let grid = grid();
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let result = search(&table, 3000.0, 4);
        assert_eq!(result.tolerance_persons, 0.0);
        assert_eq!(result.radius.km(), 3000.0);
    }

    /// A raster of zeros with `value` planted on each of `patches`, given as inclusive `(rows, cols)`.
    fn planted(patches: Vec<((u32, u32), (u32, u32))>, value: f32) -> impl Fn(u32, u32) -> f32 {
        move |row, col| {
            let inside = patches.iter().any(|((north, south), (first, last))| {
                (*north..=*south).contains(&row) && (*first..=*last).contains(&col)
            });
            if inside { value } else { 0.0 }
        }
    }

    /// The span the winning circle covers in its own centre row, which is what the seam and pole cases
    /// assert alongside the answer.
    fn centre_span(grid: &Grid, found: Candidate, radius_km: f64) -> (Span, ColSpan) {
        let kernel = Kernel::new(*grid, found.row, radius(radius_km)).expect("a grid that closes");
        let span = kernel
            .rows()
            .find(|(row, _)| *row == found.row)
            .map(|(_, span)| span)
            .expect("the centre row is in the band");
        let cols = kernel
            .place(found.col)
            .find(|(row, _)| *row == found.row)
            .map(|(_, cols)| cols)
            .expect("the centre row is in the band");
        (span, cols)
    }

    #[test]
    fn a_planted_maximum_in_the_interior_is_found() {
        // Four cells of a hundred at 5 N to 5 S. The radius has to clear the cluster's diagonal for the
        // answer to be the whole cluster rather than part of it: those corners are 14.13 degrees apart,
        // which is 1571 km, so 1500 would hold three cells and 1800 holds four.
        let grid = grid();
        let payload = payload_over(&grid, planted(vec![((8, 9), (15, 16))], 100.0));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let expected = brute_force(&table, &grid, 1800.0);
        for cells in [Some(1u32), Some(4), Some(18), None] {
            let found = most_populous(&table, radius(1800.0), cells.map(spacing), &mut ())
                .unwrap()
                .centre;
            assert_eq!(
                (found.row.get(), found.col.get(), found.population.to_bits()),
                (
                    expected.row.get(),
                    expected.col.get(),
                    expected.population.to_bits()
                ),
                "spacing {cells:?}"
            );
        }
        assert_eq!(expected.population, 400.0);
    }

    #[test]
    fn a_planted_maximum_across_the_antimeridian_is_found() {
        // The cluster straddles the seam — column 35 and column 0 — so the winning circle has to run off
        // one end of a row and back on at the other. The wrapped span is asserted as well as the answer:
        // a search that found this centre while its kernel clipped at the seam would report a smaller
        // population, and a search whose span had stopped wrapping would pass on a case it no longer
        // covers.
        let grid = grid();
        let payload = payload_over(
            &grid,
            planted(vec![((9, 9), (35, 35)), ((9, 9), (0, 0))], 100.0),
        );
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let expected = brute_force(&table, &grid, 1500.0);
        for cells in [Some(1u32), Some(4), Some(18), None] {
            let found = most_populous(&table, radius(1500.0), cells.map(spacing), &mut ())
                .unwrap()
                .centre;
            assert_eq!(
                (found.row.get(), found.col.get(), found.population.to_bits()),
                (
                    expected.row.get(),
                    expected.col.get(),
                    expected.population.to_bits()
                ),
                "spacing {cells:?}"
            );
        }
        // Both planted cells, which only a wrapping traversal reaches from one centre.
        assert_eq!(expected.population, 200.0);

        let (_, cols) = centre_span(&grid, expected, 1500.0);
        match cols {
            ColSpan::Through { west, east } => {
                assert!(west.get() > east.get(), "{west:?} to {east:?}");
            }
            ColSpan::FullTurn => panic!("a 1500 km cap does not close a row at 5 S on this grid"),
        }
    }

    #[test]
    fn a_planted_maximum_over_the_pole_is_found() {
        // Row 0 is 85 N and a 2000 km cap is 17.99 degrees, so the far side of that parallel is 10 degrees
        // away and the whole row is inside the circle. That is the case a traversal assembling a closed
        // row from two pieces double-counts, so the `FullTurn` is asserted beside the answer.
        let grid = grid();
        let payload = payload_over(&grid, planted(vec![((0, 0), (0, 1))], 100.0));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let expected = brute_force(&table, &grid, 2000.0);
        for cells in [Some(1u32), Some(4), Some(18), None] {
            let found = most_populous(&table, radius(2000.0), cells.map(spacing), &mut ())
                .unwrap()
                .centre;
            assert_eq!(
                (found.row.get(), found.col.get(), found.population.to_bits()),
                (
                    expected.row.get(),
                    expected.col.get(),
                    expected.population.to_bits()
                ),
                "spacing {cells:?}"
            );
        }
        assert_eq!(expected.row.get(), 0);
        assert_eq!(expected.population, 200.0);
        assert_eq!(centre_span(&grid, expected, 2000.0).0, Span::FullTurn);
    }

    #[test]
    fn two_equal_maxima_resolve_to_the_north_western_one() {
        // Identical clusters, one in the northern hemisphere and one in the southern, so the maximum is
        // genuinely tied and the tie-break alone decides. The three spacings are the point: a prune that
        // dropped a tying block would answer the southern cluster at one spacing and the northern at
        // another, and each answer would look perfectly plausible on its own.
        let grid = grid();
        let payload = payload_over(
            &grid,
            planted(vec![((3, 4), (10, 11)), ((13, 14), (25, 26))], 100.0),
        );
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let expected = brute_force(&table, &grid, 1500.0);
        for cells in [Some(1u32), Some(4), Some(18), None] {
            let found = most_populous(&table, radius(1500.0), cells.map(spacing), &mut ())
                .unwrap()
                .centre;
            assert_eq!(
                (found.row.get(), found.col.get(), found.population.to_bits()),
                (
                    expected.row.get(),
                    expected.col.get(),
                    expected.population.to_bits()
                ),
                "spacing {cells:?}"
            );
        }
        // The northern cluster, which is what "north-west wins" means on this fixture.
        assert!(expected.row.get() <= 4, "row {}", expected.row.get());
        assert_eq!(expected.population, 400.0);
    }

    #[test]
    fn a_zero_radius_answers_the_single_most_populous_cell() {
        // Degenerate and legal: each circle is its own centre cell, so the answer is the largest cell of
        // the fixture. Checked against a direct scan of the cells rather than against another search.
        let grid = grid();
        let cell = distinct(&grid);
        let payload = payload_over(&grid, &cell);
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let mut heaviest = (0u32, 0u32, 0.0f64);
        for row in 0..HEIGHT {
            for col in 0..WIDTH {
                let value = f64::from(cell(row, col));
                if value > heaviest.2 {
                    heaviest = (row, col, value);
                }
            }
        }

        for cells in [Some(1u32), Some(4), Some(18), None] {
            let found = most_populous(&table, radius(0.0), cells.map(spacing), &mut ())
                .unwrap()
                .centre;
            assert_eq!(
                (found.row.get(), found.col.get(), found.population.to_bits()),
                (heaviest.0, heaviest.1, heaviest.2.to_bits()),
                "spacing {cells:?}"
            );
        }
    }

    #[test]
    fn the_better_candidate_is_the_same_whichever_order_they_arrive_in() {
        // Folded forwards and backwards over each list, because order-independence is the property and a
        // rule that took the first maximum it saw would pass one direction and fail the other.
        let grid = grid();
        let at = |row: u32, col: u32, population: f64| Candidate {
            row: row_of(&grid, row),
            col: col_of(&grid, col),
            population,
        };

        let cases = [
            // One clear maximum, which is the case that says the rule reads population first.
            (
                vec![at(3, 4, 10.0), at(0, 0, 50.0), at(17, 35, 20.0)],
                (0, 0),
            ),
            // Two equal maxima in different rows: the northern wins.
            (
                vec![at(9, 2, 50.0), at(2, 30, 50.0), at(5, 5, 10.0)],
                (2, 30),
            ),
            // Two equal maxima in one row: the western wins, by column index and not by longitude.
            (vec![at(4, 30, 50.0), at(4, 7, 50.0)], (4, 7)),
            // Everything tied, which is the all-zero raster in miniature.
            (vec![at(1, 1, 7.0), at(0, 35, 7.0), at(0, 3, 7.0)], (0, 3)),
        ];

        for (candidates, expected) in cases {
            for reversed in [false, true] {
                let mut order = candidates.clone();
                if reversed {
                    order.reverse();
                }
                let winner = order
                    .into_iter()
                    .reduce(Candidate::better)
                    .expect("the list is not empty");
                assert_eq!(
                    (winner.row.get(), winner.col.get()),
                    expected,
                    "reversed: {reversed}"
                );
            }
        }
    }

    #[test]
    fn a_block_given_its_columns_backwards_is_the_same_block() {
        let grid = grid();
        let rows = RowBand::new(row_of(&grid, 2), row_of(&grid, 5));
        assert_eq!(
            Block::new(rows, col_of(&grid, 30), col_of(&grid, 7)),
            Block::new(rows, col_of(&grid, 7), col_of(&grid, 30))
        );
    }
}
