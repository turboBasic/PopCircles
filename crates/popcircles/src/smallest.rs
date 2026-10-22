// Step 4 of application.md "Approach": the smallest circle containing a target share of a population.
// The search over radius is a climb from below and then a bisection over integer kilometres, driving
// `search`'s fixed-radius maximum at each probe. What is here is the question — a share, the population it
// resolves to, and the bracket every probe lives in; the I/O a resumed run needs is `smallest/cache.rs`'s.

pub mod cache;

use std::num::NonZeroU32;

use crate::bracket::Bracket;
use crate::geodesy::RadiusKm;
use crate::grid::{Col, Grid, Row};
use crate::kernel::Kernel;
use crate::progress::Progress;
use crate::search::{self, Candidate, SearchError, SearchStats};
use crate::table::Table;

#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum ShareError {
    #[error("a population share must be finite; {share} is not")]
    NotFinite { share: f64 },

    #[error("a population share must be above zero; {share} is not")]
    NotPositive { share: f64 },

    #[error("a population share must not exceed one; {share} does")]
    AboveOne { share: f64 },
}

/// The share of a population a circle is asked to contain, checked once where it is made.
///
/// Zero is refused rather than answered. A circle holding nobody is satisfied by every radius there is,
/// zero included, so the answer would be a property of this type rather than of the raster. One is
/// accepted, and [`CEILING_KM`] is what makes it answerable.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Share(f64);

impl Share {
    /// # Errors
    /// [`ShareError`] when the value is not a proportion: not finite, at or below zero, or above one.
    pub fn new(share: f64) -> Result<Self, ShareError> {
        if !share.is_finite() {
            return Err(ShareError::NotFinite { share });
        }
        if share <= 0.0 {
            return Err(ShareError::NotPositive { share });
        }
        if share > 1.0 {
            return Err(ShareError::AboveOne { share });
        }
        Ok(Self(share))
    }

    /// The same share given in whole percent, which is the unit a command line takes.
    ///
    /// Here rather than in whatever parses a flag, because a percent is a unit this type has an opinion
    /// about: [`Self::new`] already refuses zero and anything above one, so the conversion is those same
    /// checks over an integer and no caller repeats them.
    ///
    /// Exact where it matters. [`f64::from`] is lossless for a `u32` and IEEE division is correctly
    /// rounded, so `from_percent(10)` is bit for bit the f64 a caller typing `0.1` would have got —
    /// where a walk accumulating a step of a tenth reaches `0.30000000000000004` by its third share and
    /// publishes it.
    ///
    /// # Errors
    /// [`ShareError::NotPositive`] at zero and [`ShareError::AboveOne`] past 100. Never
    /// [`ShareError::NotFinite`]: an integer divided by a hundred is a number.
    pub fn from_percent(percent: u32) -> Result<Self, ShareError> {
        Self::new(f64::from(percent) / 100.0)
    }

    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// A share resolved against the population it is a share of.
///
/// The total is the table's own whole extent, which is the figure a build publishes, so there is one
/// answer in this program to how many people there are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    pub share: Share,
    /// `share × total`, which at a share of one is `total` exactly: multiplying by 1.0 is the identity in
    /// IEEE arithmetic, and that exactness is what makes a whole-population target reachable rather than
    /// a rounding away from it.
    pub persons: f64,
    pub total: f64,
}

impl Target {
    #[must_use]
    pub fn of(share: Share, total: f64) -> Self {
        Self {
            share,
            persons: share.get() * total,
            total,
        }
    }
}

/// The radius at which a circle is the whole grid, in kilometres, and the top of every bracket.
///
/// Half the circumference is `EARTH_RADIUS_KM · π` = 20 015.11 km, and [`crate::kernel::Kernel`] clamps a
/// cap's angle at half a turn, so at or past that figure every row of the grid is spanned in full from any
/// centre whatsoever. 20 016 is the first integer kilometre past it.
///
/// A circle this wide is never *searched*: every centre holds the same population, so nothing prunes and a
/// search would refine the grid to single cells. Its population is [`crate::table::Table::whole`]'s single
/// query instead — the circle is the whole extent, so that query is its population.
pub const CEILING_KM: u32 = 20_016;

/// [`CEILING_KM`] as a radius.
#[must_use]
pub fn ceiling_radius() -> RadiusKm {
    RadiusKm::from(CEILING_KM)
}

/// Where the maximum found at a radius is kept, so an interrupted run resumes instead of paying for it
/// twice.
///
/// A trait rather than a type for [`crate::raster::RasterSource`]'s reason: what the search needs of a
/// ledger is these two operations, and a fixture, an in-memory map and the JSON document
/// [`cache`] publishes are then the same seam. The error is associated because a
/// ledger's failures are its own — this module has no vocabulary for a filesystem — which is the shape
/// [`crate::table::build`] takes with its sink.
///
/// **What may be stored under a radius is the maximum over the table's cell centres at that radius**, and
/// nothing else. It is a property of the table alone, so it is independent of the share that asked for it
/// and of the initial spacing the search ran with — `search`'s own tests pin the second — which is why
/// neither appears in a key here or in the identity a stored ledger is checked against.
pub trait RadiusLedger {
    /// What a ledger fails with. [`std::convert::Infallible`] for one that cannot.
    type Error;

    /// The maximum recorded at `km`, or `None` when this radius has not been evaluated.
    fn get(&self, km: u32) -> Option<Candidate>;

    /// Records the maximum at `km`, durably enough that a run interrupted after this returns finds it.
    ///
    /// # Errors
    /// Whatever the implementation cannot do; a run stops rather than continuing with a ledger that has
    /// silently stopped recording.
    fn put(&mut self, km: u32, found: Candidate) -> Result<(), Self::Error>;
}

/// The ledger for a caller that wants no resumption, so the search takes `()` instead of growing a second
/// signature without the parameter — [`crate::progress::Progress`]'s reason, and the same shape.
impl RadiusLedger for () {
    type Error = std::convert::Infallible;

    fn get(&self, _km: u32) -> Option<Candidate> {
        None
    }

    fn put(&mut self, _km: u32, _found: Candidate) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Why a search for the smallest circle stopped.
///
/// Two arms, and there is no third for a target nothing reaches: the ceiling is the whole grid and a
/// [`Share`] is at most one, so every target this can be asked for is met by some radius in the bracket.
/// The ledger's own failure keeps its own type, so nothing here needs a vocabulary for whichever ledger a
/// caller brought — [`crate::table::BuildError`]'s shape.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum SmallestError<E> {
    #[error("a probe of the bracket could not be searched")]
    Search(#[from] SearchError),

    #[error("the radius ledger could not record a probe")]
    Ledger(#[source] E),
}

/// What a search over radius did beside answering, and the only window onto whether the ledger is working.
///
/// The two radius counters sum to the settled count, so a warm ledger shows up as the first falling and the
/// second rising by the same amount rather than as a run that merely felt faster. `searched` is the sum
/// over the probes that ran, which is what makes a reused radius visibly free: it contributes to none of
/// those figures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SmallestStats {
    pub radii_evaluated: u64,
    pub radii_reused: u64,
    pub searched: SearchStats,
}

impl SmallestStats {
    /// Radii the run settled, however it settled them.
    #[must_use]
    pub const fn radii_settled(self) -> u64 {
        self.radii_evaluated + self.radii_reused
    }
}

/// The widest pair of probed radii whose populations the summation slack cannot separate from the target,
/// and how many of them fell inside it.
///
/// **A floor on the ambiguity rather than the interval.** The radii a search visits are sparse — the climb
/// doubles — so the ends are the widest pair *measured* and the true interval runs past both: at a share of
/// one no radius above the answer can catch anyone new, so it reaches [`CEILING_KM`].
///
/// Accumulated over the radii one search visited, which makes it a property of the table, the share and the
/// probe order together rather than of the table alone. A warm ledger reports what a cold one did, because a
/// resumed run visits the same radii.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ambiguity {
    pub lowest_km: u32,
    pub highest_km: u32,
    /// How many probed radii fell inside, so a reader can tell seven radii spread over 477 km from 477 of
    /// them.
    pub radii: u32,
}

/// The smallest circle found, and everything a caller needs to disagree with it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Smallest {
    /// The answer in whole kilometres, which is the unit the search steps in.
    pub radius_km: u32,
    pub radius: RadiusKm,
    /// The most populous centre at [`Self::radius_km`], by `search`'s tie-break.
    pub centre: Candidate,
    pub target: Target,
    /// `centre.population / target.total`, and zero for a table holding nobody — where every circle
    /// achieves the same nothing and a quotient would be a `NaN` standing for it.
    pub share_achieved: f64,
    /// The radius one kilometre short of the answer and the population it held, or `None` when the answer
    /// is 0 km and there is nothing below it.
    ///
    /// This is the **bracket the search proved**, and it is a stronger statement than the answer alone:
    /// the pair says this radius reaches the target and the one below it does not, both measured. That the
    /// answer is also the *minimum* rests on the computed predicate being monotone, which
    /// [`Self::predicate_slack_persons`] is the width of.
    pub short_below: Option<(u32, f64)>,
    /// Whether the answer's circle is the whole table, which every circle at [`CEILING_KM`] is and a
    /// narrower one on a small grid can be. A rendered circle that is the grid is worth knowing about.
    pub covers_whole_grid: bool,
    /// How far apart two radii's populations have to be for the comparison between them to be certain.
    ///
    /// See [`predicate_slack_persons`]. Reported rather than applied: inside it the computed predicate can
    /// invert, so the answer is the true minimum for a target further than this from a plateau in the
    /// radius, and a caller close to one wants the bracket rather than a comparison this search cannot
    /// make.
    pub predicate_slack_persons: f64,
    /// The probed radii [`Self::predicate_slack_persons`] cannot separate from the target, or `None` when
    /// every probe's margin was outside it.
    ///
    /// Presence is the whole statement: inside that slack the comparison the search runs can invert, so
    /// [`Self::radius_km`] is *a* radius from this span and not a demonstrated minimum. Absent rather than
    /// empty in the ordinary case, which is [`Self::short_below`]'s convention.
    pub ambiguity: Option<Ambiguity>,
    /// The population slack the fixed-radius search at the answer's radius pruned with — zero, #6's
    /// choice, carried up so a caller reads one result rather than this crate's constants.
    pub tolerance_persons: f64,
    pub stats: SmallestStats,
}

/// How far apart two circles' computed populations must be before the comparison between them is certain,
/// in persons.
///
/// **Not the table's 4 ulp**, which bounds one rectangle query. A circle is a sum of up to one query per
/// grid row, added in [`crate::circle::population`]'s fixed order, so the error composes: `rows · 4 ulp` of
/// the total for the terms, plus about `rows · ε · total` for the additions that combine them. At the
/// registry raster's 21 600 rows and a total near 8e9 that is about 0.12 persons — seven orders of
/// magnitude below the total, and five orders *above* the per-query figure, which is why quoting the
/// per-query figure for a circle would be wrong by a factor of 30 000.
///
/// Conservative in both terms: no cancellation is assumed, and `4 ulp(total)` is charged to every row
/// including the ones whose queries are over a fraction of it.
#[must_use]
pub fn predicate_slack_persons(grid: &Grid, total: f64) -> f64 {
    let rows = f64::from(grid.height());
    let ulp = total.next_up() - total;
    rows * 4.0 * ulp + rows * f64::EPSILON * total
}

/// Whether a margin is inside the slack, so the comparison that produced it could have gone either way.
///
/// A margin rather than a population and a target, because the two sides are one question: the reaching
/// margin is `population - target` and the short margin is its negation, and the absolute value is what
/// makes them the same test.
///
/// **Equal to the slack counts as inside.** The slack is a bound on the error, so a margin that is exactly
/// it is exactly the case the bound was derived to cover.
///
/// `pub` beside [`predicate_slack_persons`], which is what supplies the second argument: a caller reading
/// that figure off a published result has this comparison to make and no way to make it otherwise.
#[must_use]
pub fn within_slack(margin: f64, slack: f64) -> bool {
    margin.abs() <= slack
}

/// The row `index` names.
///
/// A panic rather than a `Result` for [`search`]'s reason: a [`Grid`] has at least one row and one column,
/// so index 0 is a cell of every grid there is, and a miss would be a wiring mistake in this module rather
/// than an input a caller could act on.
fn north_west(grid: &Grid) -> (Row, Col) {
    match (grid.row(0), grid.col(0)) {
        (Some(row), Some(col)) => (row, col),
        _ => panic!("a grid has at least one row and one column"),
    }
}

/// `⌈log₂ width⌉`: how many halvings close an interval of `width` candidates.
fn halvings(width: u32) -> u32 {
    match width {
        0 | 1 => 0,
        _ => (width - 1).bit_width(),
    }
}

/// An upper bound on the probes still to come once `km` is the next one the climb will take.
///
/// The climb doubles until it reaches the cap, and the bracket it hands the bisection is no wider than the
/// probe that closed it, so both terms are ceilings of a logarithm rather than counts.
fn climb_probes_remaining(km: u32) -> u32 {
    let cap = CEILING_KM - 1;
    let doublings = 1 + halvings(cap / km.max(1));
    doublings + halvings(km)
}

/// Whether a circle of `radius` centred on `centre`'s row spans every row of the grid in full.
///
/// Geometric rather than a comparison of populations: two different circles can hold the same people, and
/// what this field claims is coverage.
fn spans_whole_grid(grid: &Grid, centre: Row, radius: RadiusKm) -> Result<bool, SearchError> {
    let kernel = Kernel::new(*grid, centre, radius)?;
    let mut rows = 0u32;
    for (_, span) in kernel.rows() {
        if span != crate::kernel::Span::FullTurn {
            return Ok(false);
        }
        rows += 1;
    }
    Ok(rows == grid.height())
}

/// `span` widened to hold `km` when its margin is inside the slack, and unchanged when it is not.
///
/// The flag and the ends are this one scan: a span exists exactly when some probed radius was inside, so
/// there is no second comparison to agree with. Restricting it to the answer and the radius below would
/// give the same flag and a span three orders of magnitude narrower.
fn widen(span: Option<Ambiguity>, km: u32, margin: f64, slack: f64) -> Option<Ambiguity> {
    if !within_slack(margin, slack) {
        return span;
    }
    Some(match span {
        Some(span) => Ambiguity {
            lowest_km: span.lowest_km.min(km),
            highest_km: span.highest_km.max(km),
            radii: span.radii + 1,
        },
        None => Ambiguity {
            lowest_km: km,
            highest_km: km,
            radii: 1,
        },
    })
}

/// The one record that says an answer is weaker than it looks, emitted where the field is set so the two
/// cannot disagree.
///
/// `warn` rather than `info`: everything else this library reports is what it did, and this is the only
/// thing it says about what a caller should not conclude. One per answer, so a sweep says it per share.
///
/// The slack is rounded to three figures here and exact on the wire, because a person reading a line of
/// stderr wants the order of magnitude and a program comparing margins wants the field.
fn warn_unseparated(span: Option<Ambiguity>, answer_km: u32, slack: f64) {
    if let Some(span) = span {
        log::warn!(
            "{answer_km} km is not separated: {} probed radii from {} to {} km hold the target to within {slack:.3e} persons, so it is one of them rather than demonstrably the least",
            span.radii,
            span.lowest_km,
            span.highest_km
        );
    }
}

/// The maximum at `km`, from the ledger when it holds one and from a search when it does not, with the
/// tally of which it was.
///
/// The ledger is read before the search and never after it, which is the whole of what one buys, and
/// [`SmallestStats`] is what says so from outside: a reused radius adds nothing to any of the search
/// counters, because no search ran.
fn probe<L: RadiusLedger>(
    table: &Table<'_>,
    km: u32,
    spacing: Option<NonZeroU32>,
    ledger: &mut L,
    stats: &mut SmallestStats,
) -> Result<Candidate, SmallestError<L::Error>> {
    // Box 7's third granularity, opened before the ledger is asked so a radius it answers is bracketed like
    // any other: the pair's near-zero duration is what says a rerun did no work, where emitting nothing
    // would leave a reader unable to tell that from a radius never tried.
    let _bracket = Bracket::open(module_path!(), format!("radius {km} km"));

    if let Some(found) = ledger.get(km) {
        stats.radii_reused += 1;
        // Inside the early return, so a record is emitted once per call on both paths and their count is
        // `radii_evaluated + radii_reused` — the figure the document publishes. Saying which of the two
        // answered is the whole reason a reader is reading this line.
        log::info!("{km} km holds {} — from the ledger", found.population);
        return Ok(found);
    }
    // The inner search reports per level of its own refinement; two meters through one sink interleave
    // into noise, so this one is silent and the caller's sink hears about radii.
    let searched = search::most_populous(table, RadiusKm::from(km), spacing, &mut ())?;
    ledger
        .put(km, searched.centre)
        .map_err(SmallestError::Ledger)?;
    stats.radii_evaluated += 1;
    stats.searched.levels += searched.stats.levels;
    stats.searched.blocks_examined += searched.stats.blocks_examined;
    stats.searched.blocks_pruned += searched.stats.blocks_pruned;
    stats.searched.circles_evaluated += searched.stats.circles_evaluated;
    stats.searched.kernels_built += searched.stats.kernels_built;
    log::info!("{km} km holds {} — searched", searched.centre.population);
    Ok(searched.centre)
}

/// The smallest circle whose population reaches `share` of the table's own total, by whole kilometres of
/// ground radius.
///
/// The total is [`Table::whole`]'s query, so it is the figure a build publishes rather than a second
/// summation of the same cells, and the target is `share × total`.
///
/// **The probe order climbs from below**: 1, 2, 4, … kilometres until one reaches the target, then a
/// bisection of the bracket that closed. Not a bisection of `[0, CEILING_KM]`, and the reason is a
/// property of the layer below rather than a preference. `search`'s pruning is strict, so a plateau of
/// centres that all hold the same population is never pruned and refinement runs to single cells across
/// all of it; at radii covering most of the globe that plateau is most of the grid. Climbing keeps every
/// probe of an ordinary share inside the regime where the bound bites, and `CEILING_KM` — where every
/// centre ties and the plateau is the whole grid — is answered by one [`Table::whole`] query instead of
/// ever being searched.
///
/// `spacing` is forwarded untouched: `None` derives an initial side separately at every searched radius;
/// `Some(side)` uses that exact side for all probes, even beyond the automatic ceiling. Neither changes
/// the answer or ledger identity.
///
/// `progress` is advanced once per settled radius, `(settled, settled + a bound on what remains)`. The
/// total is a bound rather than a count because how many probes there are depends on where the answer is,
/// and it may *rise* while the climb is still discovering how far it has to go — that is the honest shape
/// of a doubling search. It reaches the settled count exactly, so the last call a sink sees is `(n, n)`.
///
/// # Errors
/// [`SmallestError::Search`] when a probe cannot be searched — a grid whose columns do not close has no
/// kernels — and [`SmallestError::Ledger`] when the ledger cannot record one. There is no error for an
/// unreachable target: a [`Share`] is at most one and the ceiling holds the whole total.
pub fn smallest<L: RadiusLedger, P: Progress>(
    table: &Table<'_>,
    share: Share,
    spacing: Option<NonZeroU32>,
    ledger: &mut L,
    progress: &mut P,
) -> Result<Smallest, SmallestError<L::Error>> {
    let grid = *table.grid();
    let (whole_rows, whole_cols) = table.whole();
    let total = table.population(whole_rows, whole_cols);
    let target = Target::of(share, total);

    log::info!(
        "searching for the smallest circle holding {} of {total}",
        target.persons
    );

    let mut stats = SmallestStats::default();
    let slack = predicate_slack_persons(&grid, total);
    // The climb. `CEILING_KM - 1` is the cap because the ceiling itself is not a radius this searches.
    let cap = CEILING_KM - 1;
    let mut km = 1u32;
    let mut low = 0u32;
    let mut short_below: Option<(u32, f64)> = None;
    let mut reached: Option<(u32, Candidate)> = None;
    // Over the radii that go through `probe`, and the ceiling is not one of them: at a share of one its
    // population is the target bit for bit, so sweeping in the answer instead would call every ceiling
    // result unseparated including the ones whose short radius is measurably below the target.
    let mut ambiguity: Option<Ambiguity> = None;

    loop {
        let settled = stats.radii_settled();
        progress.advance(settled, settled + u64::from(climb_probes_remaining(km)));
        let found = probe(table, km, spacing, ledger, &mut stats)?;
        ambiguity = widen(ambiguity, km, found.population - target.persons, slack);

        if found.population >= target.persons {
            reached = Some((km, found));
            break;
        }
        short_below = Some((km, found.population));
        low = km + 1;
        if km == cap {
            break;
        }
        km = km.saturating_mul(2).min(cap);
    }

    let Some((mut high, mut best)) = reached else {
        // Nothing under the ceiling reaches the target, so the answer is the circle that is the grid. Its
        // population is the extent's own query, and every cell of the grid is a maximiser of it — so
        // `Candidate::better`'s rule, ties to the smaller `(row, col)`, names the north-west one.
        let (row, col) = north_west(&grid);
        log::info!(
            "nothing under the ceiling reaches it: {CEILING_KM} km, {} radii settled",
            stats.radii_settled()
        );
        warn_unseparated(ambiguity, CEILING_KM, slack);
        return Ok(Smallest {
            radius_km: CEILING_KM,
            radius: ceiling_radius(),
            centre: Candidate {
                row,
                col,
                population: total,
            },
            target,
            share_achieved: share_of(total, total),
            short_below,
            covers_whole_grid: true,
            predicate_slack_persons: slack,
            ambiguity,
            tolerance_persons: 0.0,
            stats,
        });
    };

    // The bisection. Every radius under `low` has been ruled out by a probe that fell short, and `high`
    // reaches, so the invariant holds at entry and the loop keeps it.
    while low < high {
        let settled = stats.radii_settled();
        // The candidates left are `high - low + 1`, not the difference: a bracket holding two radii still
        // needs a probe to say which, and a bound that read zero there would let the meter reach its total
        // and then pass it.
        progress.advance(settled, settled + u64::from(halvings(high - low + 1)));
        let mid = low + (high - low) / 2;
        let found = probe(table, mid, spacing, ledger, &mut stats)?;
        ambiguity = widen(ambiguity, mid, found.population - target.persons, slack);

        if found.population >= target.persons {
            high = mid;
            best = found;
        } else {
            short_below = Some((mid, found.population));
            low = mid + 1;
        }
    }

    progress.advance(stats.radii_settled(), stats.radii_settled());
    log::info!(
        "reached at {high} km, {} radii settled",
        stats.radii_settled()
    );
    warn_unseparated(ambiguity, high, slack);
    Ok(Smallest {
        radius_km: high,
        radius: RadiusKm::from(high),
        centre: best,
        target,
        share_achieved: share_of(best.population, total),
        // Only the radius directly below the answer is a witness to its minimality; a short probe further
        // down is one the bisection has already superseded.
        short_below: short_below.filter(|(km, _)| Some(*km) == high.checked_sub(1)),
        covers_whole_grid: spans_whole_grid(&grid, best.row, RadiusKm::from(high))?,
        predicate_slack_persons: slack,
        ambiguity,
        tolerance_persons: 0.0,
        stats,
    })
}

/// The share a population is of a total, and zero when the total is nothing: every circle over an empty
/// table achieves the same nothing, and `0 / 0` would publish a `NaN` standing for it.
///
/// `pub(crate)` because [`crate::report`] publishes the same quotient for a circle nothing searched for,
/// and two answers to one question is how a renderer ends up printing `NaN%`.
pub(crate) fn share_of(population: f64, total: f64) -> f64 {
    if total > 0.0 { population / total } else { 0.0 }
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows both in tests. float_cmp is the point rather than a concession: the
// fixtures below are distinct small integers, so every partial sum is exact and a tolerance would let a
// fold that lost a row pass.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss
)]
mod tests {
    use std::collections::BTreeMap;
    use std::convert::Infallible;

    use proptest::prelude::*;

    use super::*;
    use crate::circle;
    use crate::geodesy::LatLon;
    use crate::grid::{Col, Grid, Row};
    use crate::kernel::{Kernel, Span};
    use crate::raster::Synthetic;
    use crate::table::{Decimation, Table, build};

    /// The boundary of [`within_slack`], as two literals at a time.
    ///
    /// It is tested here rather than through a search because no fixture small enough to commit can reach
    /// it: every synthetic table that is ambiguous at all is ambiguous by a margin of **exactly zero** — a
    /// handful of populated rows gives the fold nothing to reorder — so `margin == 0.0` would satisfy every
    /// fixture case in this module. The registry raster's own residue appears below as a literal for that
    /// reason, and ADR 0007's Context is where it was measured.
    #[test]
    fn the_slack_is_inside_itself_and_the_bit_above_it_is_not() {
        let slack = 0.011_960_601_365_319_32;

        // `<=`, pinned at the bit rather than at a rounding: the slack bounds the error, so a margin equal
        // to it is the case the bound exists to cover.
        assert!(within_slack(slack, slack));
        assert!(within_slack(slack.next_down(), slack));
        assert!(!within_slack(slack.next_up(), slack));

        // Both zeroes, because a margin of zero is what an exact tie between two summation orders gives and
        // `-0.0 <= x` needs no special case to hold.
        assert!(within_slack(0.0, slack));
        assert!(within_slack(-0.0, slack));

        // The registry raster at a share of one, both ways round: 1.34e-5 of residue is inside the slack
        // that table carries, and outside a slack eleven thousand times smaller. This pair is the whole
        // reason the comparison is a function — no fixture produces a margin between the two.
        assert!(within_slack(1.34e-5, slack));
        assert!(!within_slack(1.34e-5, 1e-9));

        // The 50% run, seven orders of magnitude out, which is the ordinary answer staying separated.
        assert!(!within_slack(121_814.0, slack));
        // Sign does not matter: the short margin is the reaching margin's negation.
        assert!(!within_slack(-121_814.0, slack));

        // Stated rather than relied on. `search` documents that a population which is not a number cannot
        // arise from a sanitised raster; if one ever did, every comparison against it is false, so this
        // reports separated rather than claiming an ambiguity it cannot describe.
        assert!(!within_slack(f64::NAN, slack));
    }

    /// A ledger in a map, which is what the tests here drive a search with: the seam without the
    /// filesystem, so what a resumed run does is pinned before `cache.rs` exists.
    #[derive(Debug, Default)]
    struct Recorded {
        entries: BTreeMap<u32, Candidate>,
    }

    impl RadiusLedger for Recorded {
        type Error = Infallible;

        fn get(&self, km: u32) -> Option<Candidate> {
            self.entries.get(&km).copied()
        }

        fn put(&mut self, km: u32, found: Candidate) -> Result<(), Self::Error> {
            self.entries.insert(km, found);
            Ok(())
        }
    }

    /// The interruption, in the one shape a test can hold still: a ledger that records `stop_after` probes
    /// and then fails. What survives in `entries` is what a killed process would have left behind.
    #[derive(Debug)]
    struct FailsAfter {
        entries: BTreeMap<u32, Candidate>,
        stop_after: usize,
    }

    impl FailsAfter {
        fn new(stop_after: usize) -> Self {
            Self {
                entries: BTreeMap::new(),
                stop_after,
            }
        }
    }

    /// A ledger that has stopped recording, which is what the search refuses to continue past.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
    #[error("the ledger stopped recording")]
    struct Stopped;

    impl RadiusLedger for FailsAfter {
        type Error = Stopped;

        fn get(&self, km: u32) -> Option<Candidate> {
            self.entries.get(&km).copied()
        }

        fn put(&mut self, km: u32, found: Candidate) -> Result<(), Self::Error> {
            if self.entries.len() >= self.stop_after {
                return Err(Stopped);
            }
            self.entries.insert(km, found);
            Ok(())
        }
    }

    /// The whole-globe fixture the search's own tests use, ten degrees a side.
    fn grid() -> Grid {
        Grid::new(
            36,
            18,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            10.0,
            -10.0,
        )
        .expect("a 36 x 18 whole-globe grid is valid")
    }

    /// `search.rs`'s band shape: closing in longitude, thirty degrees of latitude, so a cap is clipped at
    /// the grid's edge rather than refused. The ceiling has to close this grid too, because a table over a
    /// band is one `Kernel` accepts.
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

    /// Distinct at every position, so every partial sum is exact in f64 and a cell counted twice moves a
    /// total.
    fn distinct(grid: &Grid) -> impl Fn(u32, u32) -> f32 + use<> {
        let width = grid.width();
        move |row, col| (row * width + col + 1) as f32
    }

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

    fn col_of(grid: &Grid, index: u32) -> Col {
        grid.col(index).expect("a column of the fixture")
    }

    fn row_of(grid: &Grid, index: u32) -> Row {
        grid.row(index).expect("a row of the fixture")
    }

    fn candidate(grid: &Grid, row: u32, col: u32, population: f64) -> Candidate {
        Candidate {
            row: row_of(grid, row),
            col: col_of(grid, col),
            population,
        }
    }

    fn spacing(cells: u32) -> NonZeroU32 {
        NonZeroU32::new(cells).expect("a fixture spacing is not zero")
    }

    fn share(value: f64) -> Share {
        Share::new(value).expect("a fixture share is a proportion")
    }

    /// A raster of zeros with `value` planted on each of `patches`, given as inclusive `(rows, cols)`.
    fn planted(
        patches: Vec<((u32, u32), (u32, u32))>,
        value: f32,
    ) -> impl Fn(u32, u32) -> f32 + use<> {
        move |row, col| {
            let inside = patches.iter().any(|((north, south), (first, last))| {
                (*north..=*south).contains(&row) && (*first..=*last).contains(&col)
            });
            if inside { value } else { 0.0 }
        }
    }

    /// The maximum at one radius, by the same search a probe runs, for a test that wants to check a
    /// bracket's other end without going through `smallest` again.
    fn maximum_at(table: &Table<'_>, km: u32, cells: u32) -> Candidate {
        search::most_populous(table, RadiusKm::from(km), Some(spacing(cells)), &mut ())
            .expect("a whole-globe fixture has kernels")
            .centre
    }

    fn found(table: &Table<'_>, value: f64, cells: u32) -> Smallest {
        smallest(table, share(value), Some(spacing(cells)), &mut (), &mut ())
            .expect("a whole-globe fixture and a no-op ledger cannot fail")
    }

    #[test]
    fn a_share_is_a_proportion_and_nothing_else() {
        assert_eq!(Share::new(0.5).unwrap().get(), 0.5);
        // One is a share, and the reason this test states it rather than leaving it to the range: it is
        // the case the whole design of the ceiling exists to answer.
        assert_eq!(Share::new(1.0).unwrap().get(), 1.0);

        assert_eq!(
            Share::new(0.0).unwrap_err(),
            ShareError::NotPositive { share: 0.0 }
        );
        assert_eq!(
            Share::new(-0.25).unwrap_err(),
            ShareError::NotPositive { share: -0.25 }
        );
        assert!(matches!(
            Share::new(1.000_000_1),
            Err(ShareError::AboveOne { .. })
        ));
        assert!(matches!(
            Share::new(f64::NAN),
            Err(ShareError::NotFinite { .. })
        ));
        assert!(matches!(
            Share::new(f64::INFINITY),
            Err(ShareError::NotFinite { .. })
        ));
    }

    #[test]
    fn a_whole_percent_is_the_share_a_caller_would_have_typed() {
        // Bit for bit, which is the claim: a flag in percent costs a user nothing in precision, and it is
        // what keeps a sweep's published shares free of the residue an accumulated step leaves.
        for (percent, fraction) in [(1u32, 0.01), (10, 0.1), (30, 0.3), (50, 0.5), (100, 1.0)] {
            assert_eq!(
                Share::from_percent(percent).unwrap().get().to_bits(),
                Share::new(fraction).unwrap().get().to_bits(),
                "{percent}% is not the f64 {fraction} is"
            );
        }

        // Two of `Share::new`'s three grounds, reported as themselves rather than as one "bad percent".
        assert_eq!(
            Share::from_percent(0).unwrap_err(),
            ShareError::NotPositive { share: 0.0 }
        );
        assert_eq!(
            Share::from_percent(101).unwrap_err(),
            ShareError::AboveOne { share: 1.01 }
        );
    }

    #[test]
    fn a_whole_share_targets_the_total_exactly() {
        // The identity that keeps a whole-population target reachable: no rounding stands between the
        // share and the figure the ceiling answers with.
        let total = 7_757_982_599.32;
        let whole = Target::of(Share::new(1.0).unwrap(), total);
        assert_eq!(whole.persons, total);
        assert_eq!(whole.persons.to_bits(), total.to_bits());

        let half = Target::of(Share::new(0.5).unwrap(), total);
        assert_eq!(half.persons, total / 2.0);
        assert_eq!(half.total, total);
    }

    #[test]
    fn a_ledger_gives_back_the_maximum_it_was_given_and_nothing_for_a_radius_it_has_not_seen() {
        let grid = grid();
        let mut ledger = Recorded::default();
        let found = candidate(&grid, 3, 7, 4200.0);

        assert_eq!(ledger.get(1500), None);
        ledger.put(1500, found).unwrap();
        assert_eq!(ledger.get(1500), Some(found));
        // The neighbouring kilometres are not the same probe, which is the whole of what an integer key
        // means here.
        assert_eq!(ledger.get(1499), None);
        assert_eq!(ledger.get(1501), None);
    }

    #[test]
    fn the_no_op_ledger_records_nothing() {
        // There is no behaviour to assert beyond this, and it is the reason the implementation exists: a
        // caller wanting no resumption passes `()` and the search needs no second signature. The `put`
        // succeeding while the `get` stays empty is what makes it a sink rather than a store.
        let grid = grid();
        let mut none = ();
        none.put(1500, candidate(&grid, 0, 0, 1.0)).unwrap();
        assert_eq!(RadiusLedger::get(&none, 1500), None);
    }

    #[test]
    fn the_ceiling_spans_every_row_of_a_grid_in_full() {
        // The claim `CEILING_KM` rests on, from every centre row rather than from one: a cap clamped at
        // half a turn reaches every row of the grid and closes each of them, so the circle is the grid.
        for grid in [grid(), band_grid()] {
            for centre in grid.rows() {
                let kernel =
                    Kernel::new(grid, centre, ceiling_radius()).expect("a grid that closes");
                let spans: Vec<(_, Span)> = kernel.rows().collect();

                assert_eq!(
                    spans.len(),
                    grid.height() as usize,
                    "the ceiling misses a row of a {}-row grid",
                    grid.height()
                );
                assert!(
                    spans.iter().all(|(_, span)| *span == Span::FullTurn),
                    "the ceiling leaves a row short of a full turn"
                );
            }
        }
    }

    #[test]
    fn the_ceiling_circle_is_the_tables_whole_extent() {
        // What licenses answering the ceiling with one `Table::whole` query instead of a fold over the
        // rows: the two are the same population. Here they agree bit for bit, because the fixture's sums
        // are exact; at full magnitude they agree to the bound `full_resolution_table.rs` measures, which
        // is the whole reason the query is preferred to the fold.
        for grid in [grid(), band_grid()] {
            let payload = payload_over(&grid, distinct(&grid));
            let table = Table::new(grid, &payload).expect("the build emits the padded product");
            let (rows, cols) = table.whole();
            let whole = table.population(rows, cols);

            // Three centres, one of them the first column, because a circle wide enough to close every
            // row must not depend on where it is centred — and the seam is where a fold that assembled a
            // closed row from two pieces would double count.
            for centre in [0, 17, 35] {
                let kernel = Kernel::new(grid, grid.middle_row(), ceiling_radius())
                    .expect("a grid that closes");
                let folded = circle::population(&table, &kernel, col_of(&grid, centre));
                assert_eq!(
                    folded.to_bits(),
                    whole.to_bits(),
                    "centre {centre} folds to {folded}, not the extent's {whole}"
                );
            }
        }
    }

    #[test]
    fn a_warm_ledger_is_read_and_no_radius_is_searched_twice() {
        // The reuse claim in the only terms that can fail: the counts. A second run over the ledger the
        // first one filled searches nothing at all, settles the same radii, and lands on the same answer
        // bit for bit — so the ledger is being read before the search rather than beside it.
        let grid = grid();
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let mut ledger = Recorded::default();
        let cold = smallest(&table, share(0.25), Some(spacing(4)), &mut ledger, &mut ())
            .expect("a whole-globe fixture and a map cannot fail");
        assert_eq!(cold.stats.radii_reused, 0);
        assert!(cold.stats.radii_evaluated > 1);
        assert_eq!(cold.stats.radii_evaluated, ledger.entries.len() as u64);

        let warm = smallest(&table, share(0.25), Some(spacing(4)), &mut ledger, &mut ())
            .expect("a whole-globe fixture and a map cannot fail");
        assert_eq!(warm.stats.radii_evaluated, 0);
        assert_eq!(warm.stats.radii_reused, cold.stats.radii_settled());
        // Nothing searched means none of the search's own work happened, which is the half of the claim a
        // radius counter alone would not carry.
        assert_eq!(warm.stats.searched, SearchStats::default());
        assert_eq!(warm.radius_km, cold.radius_km);
        assert_eq!(
            warm.centre.population.to_bits(),
            cold.centre.population.to_bits()
        );
        assert_eq!(warm.short_below, cold.short_below);
    }

    fn assert_same_answer(actual: Smallest, expected: Smallest) {
        assert_eq!(actual.radius_km, expected.radius_km);
        assert_eq!(actual.radius.km().to_bits(), expected.radius.km().to_bits());
        assert_eq!(
            (
                actual.centre.row,
                actual.centre.col,
                actual.centre.population.to_bits()
            ),
            (
                expected.centre.row,
                expected.centre.col,
                expected.centre.population.to_bits()
            )
        );
        assert_eq!(
            actual
                .short_below
                .map(|(km, people)| (km, people.to_bits())),
            expected
                .short_below
                .map(|(km, people)| (km, people.to_bits()))
        );
        assert_eq!(actual.ambiguity, expected.ambiguity);
        assert_eq!(actual.covers_whole_grid, expected.covers_whole_grid);
        assert_eq!(
            actual.predicate_slack_persons.to_bits(),
            expected.predicate_slack_persons.to_bits()
        );
        assert_eq!(
            actual.share_achieved.to_bits(),
            expected.share_achieved.to_bits()
        );
        assert_eq!(
            [
                actual.target.share.get().to_bits(),
                actual.target.persons.to_bits(),
                actual.target.total.to_bits()
            ],
            [
                expected.target.share.get().to_bits(),
                expected.target.persons.to_bits(),
                expected.target.total.to_bits()
            ]
        );
    }

    #[test]
    fn automatic_probes_derive_each_radius_and_ledgers_reuse_across_policies() {
        let grid = grid();
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).unwrap();
        let mut ledger = Recorded::default();
        let automatic = smallest(&table, share(0.25), None, &mut ledger, &mut ()).unwrap();
        assert_eq!(automatic.stats.radii_reused, 0);
        assert_eq!(automatic.stats.radii_evaluated, ledger.entries.len() as u64);
        let mut sides = std::collections::BTreeSet::new();
        let mut sum = SearchStats::default();
        for &km in ledger.entries.keys() {
            let radius = RadiusKm::from(km);
            let side = search::initial_spacing(&grid, radius);
            sides.insert(side);
            let run = search::most_populous(&table, radius, None, &mut ()).unwrap();
            sum.levels += run.stats.levels;
            sum.blocks_examined += run.stats.blocks_examined;
            sum.blocks_pruned += run.stats.blocks_pruned;
            sum.circles_evaluated += run.stats.circles_evaluated;
            sum.kernels_built += run.stats.kernels_built;
        }
        assert!(
            sides.len() > 1,
            "the fixture must require per-radius derivation"
        );
        assert_eq!(automatic.stats.searched, sum);
        let explicit_warm =
            smallest(&table, share(0.25), Some(spacing(1)), &mut ledger, &mut ()).unwrap();
        assert_same_answer(explicit_warm, automatic);
        assert_eq!(explicit_warm.stats.radii_evaluated, 0);
        assert_eq!(
            explicit_warm.stats.radii_reused,
            automatic.stats.radii_settled()
        );
        assert_eq!(explicit_warm.stats.searched, SearchStats::default());

        let mut ledger = Recorded::default();
        let explicit =
            smallest(&table, share(0.25), Some(spacing(1)), &mut ledger, &mut ()).unwrap();
        assert_same_answer(automatic, explicit);
        let automatic_warm = smallest(&table, share(0.25), None, &mut ledger, &mut ()).unwrap();
        assert_same_answer(automatic_warm, explicit);
        assert_eq!(automatic_warm.stats.radii_evaluated, 0);
        assert_eq!(
            automatic_warm.stats.radii_reused,
            explicit.stats.radii_settled()
        );
        assert_eq!(automatic_warm.stats.searched, SearchStats::default());
    }

    #[test]
    fn automatic_smallest_preserves_zero_and_ambiguous_plateau_answers() {
        let grid = grid();
        for cell in [
            planted(vec![], 100.0),
            planted(vec![((8, 9), (15, 16))], 100.0),
        ] {
            let payload = payload_over(&grid, cell);
            let table = Table::new(grid, &payload).unwrap();
            for value in [0.25, 1.0] {
                let explicit =
                    smallest(&table, share(value), Some(spacing(1)), &mut (), &mut ()).unwrap();
                let automatic = smallest(&table, share(value), None, &mut (), &mut ()).unwrap();
                assert_same_answer(automatic, explicit);
            }
        }
    }

    #[test]
    fn a_warm_ledger_reports_the_span_the_cold_run_measured() {
        // What accumulating over the visit buys, and what reading the ledger back would have cost: the span
        // is a function of the table, the share and the probed radii, so a run that searched nothing at all
        // reports what the run that searched everything measured.
        let grid = grid();
        let payload = payload_over(&grid, planted(vec![((8, 9), (15, 16))], 100.0));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let mut ledger = Recorded::default();
        let cold = smallest(&table, share(1.0), Some(spacing(4)), &mut ledger, &mut ())
            .expect("a whole-globe fixture and a map cannot fail");
        let warm = smallest(&table, share(1.0), Some(spacing(4)), &mut ledger, &mut ())
            .expect("a whole-globe fixture and a map cannot fail");

        assert_eq!(warm.stats.radii_evaluated, 0);
        assert_eq!(
            cold.ambiguity,
            Some(Ambiguity {
                lowest_km: 1572,
                highest_km: 2048,
                radii: 7,
            })
        );
        assert_eq!(warm.ambiguity, cold.ambiguity);
    }

    #[test]
    fn an_interrupted_run_resumes_and_answers_what_an_uninterrupted_one_does() {
        // Issue #7's second box at the seam, before a file is involved. The run stops on the ledger's
        // third put; the entries it kept are what a killed process would have left behind; and a fresh run
        // reading them reaches the same answer, reusing exactly those three and searching exactly the rest.
        let grid = grid();
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let uninterrupted = found(&table, 0.25, 4);

        let mut interrupted = FailsAfter::new(3);
        assert_eq!(
            smallest(
                &table,
                share(0.25),
                Some(spacing(4)),
                &mut interrupted,
                &mut ()
            ),
            Err(SmallestError::Ledger(Stopped))
        );
        assert_eq!(interrupted.entries.len(), 3);

        // The next process reads the file the last one left and records over it, which is why the resumed
        // ledger is an ordinary one seeded with those entries rather than the failing one reused.
        let mut resumed_ledger = Recorded {
            entries: interrupted.entries,
        };
        let resumed = smallest(
            &table,
            share(0.25),
            Some(spacing(4)),
            &mut resumed_ledger,
            &mut (),
        )
        .expect("a whole-globe fixture and a map cannot fail");

        assert_eq!(resumed.stats.radii_reused, 3);
        assert_eq!(
            resumed.stats.radii_evaluated,
            uninterrupted.stats.radii_settled() - 3
        );
        assert_eq!(
            resumed.stats.radii_settled(),
            uninterrupted.stats.radii_settled()
        );
        // The answer, down to the bits: an interruption costs time and nothing else.
        assert_eq!(resumed.radius_km, uninterrupted.radius_km);
        assert_eq!(
            resumed.centre.population.to_bits(),
            uninterrupted.centre.population.to_bits()
        );
        assert_eq!(resumed.centre.row, uninterrupted.centre.row);
        assert_eq!(resumed.centre.col, uninterrupted.centre.col);
        assert_eq!(resumed.short_below, uninterrupted.short_below);
    }

    #[test]
    fn the_answer_reaches_the_target_and_the_kilometre_below_it_does_not() {
        // The bracket, on two fixtures, with every figure a literal: the answer, the population it holds
        // and the population the kilometre below it holds. Changing any of the three fails, which is what
        // makes this a claim about the search rather than a record of what it printed.
        //
        // The cluster is #6's: four cells of a hundred at 5 N to 5 S, whose diagonal is 14.13 degrees, or
        // 1571 km. So no centre reaches all four at 1571 — three of them is 300 people — and 1572 is the
        // first radius that does.
        let grid = grid();
        let cluster = payload_over(&grid, planted(vec![((8, 9), (15, 16))], 100.0));
        let table = Table::new(grid, &cluster).expect("the build emits the padded product");

        let all = found(&table, 1.0, 4);
        assert_eq!(all.radius_km, 1572);
        assert_eq!(all.centre.population, 400.0);
        assert_eq!(all.short_below, Some((1571, 300.0)));
        assert_eq!(all.share_achieved, 1.0);
        assert!(!all.covers_whole_grid);
        assert_eq!(all.tolerance_persons, 0.0);
        // The witness is a probe and not an inference: the same radius searched directly is short too.
        assert_eq!(maximum_at(&table, 1571, 4).population, 300.0);
        // Everyone is 400 people whatever radius holds them, so every reaching probe's margin is zero and
        // the answer is one of seven radii this arithmetic cannot tell apart: 1572, 1576, 1584, 1600, 1664,
        // 1792 and the climb's 2048. The ends are 477 km apart where `short_below` is 1 km away.
        assert_eq!(
            all.ambiguity,
            Some(Ambiguity {
                lowest_km: 1572,
                highest_km: 2048,
                radii: 7,
            })
        );

        // And half of a whole-globe fixture, where the answer is a long way from either end of the
        // bracket: 210 276 people over 648 distinct cells, half of them inside 5770 km of one centre.
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");
        let half = found(&table, 0.5, 4);
        assert_eq!(half.radius_km, 5770);
        assert_eq!(half.target.persons, 105_138.0);
        assert_eq!(half.centre.population, 105_623.0);
        assert_eq!(half.short_below, Some((5769, 104_706.0)));
        assert_eq!(maximum_at(&table, 5769, 4).population, 104_706.0);
        // And the ordinary answer beside the ambiguous one: 485 people over the target and 432 under it,
        // against a slack of 2.94e-9, so nothing is unseparated and the field is absent.
        assert_eq!(half.ambiguity, None);
    }

    #[test]
    fn no_radius_under_the_answer_reaches_the_target() {
        // The minimality claim by exhaustion rather than by the bracket: every one of the 1572 integer
        // radii below the answer is searched and every one of them falls short. This is the test the
        // climb-then-bisect order has to survive — a bracket closed one kilometre too high would pass the
        // test above and fail here.
        //
        // Measured on this tree at 0.98 s in a debug build, which is why the fixture is the cluster and
        // not the half-share above: the same scan at 5770 km would be nearly four times that.
        let grid = grid();
        let payload = payload_over(&grid, planted(vec![((8, 9), (15, 16))], 100.0));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let answer = found(&table, 1.0, 4);
        assert_eq!(answer.radius_km, 1572);
        // Minimality by exhaustion and what the result claims about it, side by side: every radius below is
        // short, and the answer still cannot be separated from the six above it that hold the same people.
        assert_eq!(
            answer.ambiguity,
            Some(Ambiguity {
                lowest_km: 1572,
                highest_km: 2048,
                radii: 7,
            })
        );
        for km in 0..answer.radius_km {
            let held = maximum_at(&table, km, 4).population;
            assert!(
                held < answer.target.persons,
                "{km} km already holds {held} of the {} wanted",
                answer.target.persons
            );
        }
    }

    #[test]
    fn a_share_one_cell_holds_is_answered_at_zero_kilometres() {
        // Zero is inside the bracket rather than a case beside it: the climb's first reaching radius is
        // 1 km, the bisection over [0, 1] probes zero, and a circle that is its own centre cell is the
        // answer. `short_below` is `None` because there is no radius below it to have proved short.
        let grid = grid();
        let payload = payload_over(&grid, planted(vec![((4, 4), (7, 7))], 50.0));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let single = found(&table, 1.0, 4);
        assert_eq!(single.radius_km, 0);
        assert_eq!(single.radius.km(), 0.0);
        assert_eq!(single.short_below, None);
        assert_eq!((single.centre.row.get(), single.centre.col.get()), (4, 7));
        assert_eq!(single.centre.population, 50.0);
        // The degenerate span, and the case that says an answer of 0 km is scanned like any other: both
        // probed radii hold the one cell, so both margins are zero.
        assert_eq!(
            single.ambiguity,
            Some(Ambiguity {
                lowest_km: 0,
                highest_km: 1,
                radii: 2,
            })
        );
    }

    #[test]
    fn a_target_only_the_whole_grid_reaches_is_answered_at_the_ceiling() {
        // The fixture's own geometry is what makes this case reachable, and it is worth stating: the grid
        // is symmetric about the equator and the antimeridian, so every cell centre has another cell
        // centre exactly antipodal to it — (85 N, 175 W) against (85 S, 5 E). Every cell holds people, so
        // a circle containing all of them spans half the circumference, and no integer kilometre under
        // 20 016 does. At 20 015 the maximum misses exactly one cell, and on this fixture that cell is the
        // one holding a single person.
        let grid = grid();
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");
        let (rows, cols) = table.whole();
        let total = table.population(rows, cols);

        let whole = found(&table, 1.0, 4);
        assert_eq!(whole.radius_km, CEILING_KM);
        assert!(whole.covers_whole_grid);
        // The population is the extent's own query, bit for bit, and not a fold that agrees to a
        // tolerance — which is the whole reason the ceiling is answered the way it is.
        assert_eq!(whole.centre.population.to_bits(), total.to_bits());
        assert_eq!(whole.target.persons.to_bits(), total.to_bits());
        assert_eq!(whole.share_achieved, 1.0);
        // The north-west cell, by the tie-break the test below pins.
        assert_eq!((whole.centre.row.get(), whole.centre.col.get()), (0, 0));
        assert_eq!(whole.short_below, Some((20_015, total - 1.0)));
        // The exclusion the scan rests on. The ceiling's own margin is exactly zero every time the ceiling
        // fires — a whole-population target *is* the extent's query — but the ceiling is answered rather
        // than probed, and the widest radius that was probed falls a whole person short. A scan sweeping in
        // the returned answer would report a span here, over a result that is decided.
        assert_eq!(whole.ambiguity, None);
    }

    #[test]
    fn the_tie_break_over_equal_candidates_names_the_north_west_cell() {
        // What licenses the centre above without re-deriving `search`'s rule here: with every candidate
        // holding the same population, folding the real rule over every cell of the grid gives cell
        // (0, 0). Folded in both directions, because the rule is order-independent and a shortcut resting
        // on a first-seen rule would be wrong the moment the traversal changed.
        let grid = grid();
        let every: Vec<Candidate> = grid
            .rows()
            .flat_map(|row| {
                grid.cols().map(move |col| Candidate {
                    row,
                    col,
                    population: 7.0,
                })
            })
            .collect();

        let forwards = every
            .iter()
            .copied()
            .reduce(Candidate::better)
            .expect("the grid has cells");
        let backwards = every
            .iter()
            .rev()
            .copied()
            .reduce(Candidate::better)
            .expect("the grid has cells");

        assert_eq!((forwards.row.get(), forwards.col.get()), (0, 0));
        assert_eq!((backwards.row.get(), backwards.col.get()), (0, 0));
    }

    #[test]
    fn the_same_search_twice_gives_the_same_bits() {
        let grid = grid();
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let first = found(&table, 0.25, 5);
        let second = found(&table, 0.25, 5);
        assert_eq!(first.radius_km, second.radius_km);
        assert_eq!(
            first.centre.population.to_bits(),
            second.centre.population.to_bits()
        );
        assert_eq!(first.centre.row, second.centre.row);
        assert_eq!(first.centre.col, second.centre.col);
        assert_eq!(first.short_below, second.short_below);
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
    fn the_sink_hears_one_call_per_settled_radius_and_a_total_that_lands_on_it() {
        // The meter's contract, and the honest shape of a doubling search is inside it: the total *rises*
        // while the climb is still discovering how far it has to go, because each doubling widens the
        // bracket the bisection will have to halve. What may not happen is a total that rises once the
        // bracket is closed, or one the done count passes.
        let grid = grid();
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let mut sink = Reported::default();
        let answer = smallest(&table, share(0.25), Some(spacing(4)), &mut (), &mut sink)
            .expect("a whole-globe fixture and a no-op ledger cannot fail");

        let settled = answer.stats.radii_settled();
        // One call before each probe, and one at the end saying the work is done.
        assert_eq!(sink.calls.len() as u64, settled + 1);
        for (index, (done, total)) in sink.calls.iter().enumerate() {
            assert_eq!(*done, index as u64, "the done count skipped a radius");
            assert!(
                total >= done,
                "the meter passed its own total: {done} of {total}"
            );
        }
        assert_eq!(
            *sink.calls.last().expect("a call was made"),
            (settled, settled)
        );

        // The climb hands over to the bisection, and that is where the bound collapses: without it the
        // total would be a constant ceiling and this assertion is what fails if someone makes it one.
        let totals: Vec<u64> = sink.calls.iter().map(|(_, total)| *total).collect();
        let first_fall = totals
            .windows(2)
            .position(|pair| pair[1] < pair[0])
            .expect("the bound has to collapse when the bracket closes");
        assert!(
            totals[first_fall + 1..]
                .windows(2)
                .all(|pair| pair[1] <= pair[0]),
            "the total rose again after the bracket closed: {totals:?}"
        );
    }

    /// A fixture whose cells are large enough that the partial sums round, which is where monotonicity is
    /// the arithmetic's rather than the search's. 2^40 is past the point where adding a cell of the same
    /// size is exact in the running total, so the fold at two nearby radii can invert by ulps — the reason
    /// the property below is asserted to `predicate_slack_persons` here and exactly on `distinct`.
    fn scaled(grid: &Grid) -> impl Fn(u32, u32) -> f32 + use<> {
        let width = grid.width();
        move |row, col| ((row * width + col + 1) as f32) * 1.1e12
    }

    // 64 cases rather than the default 256, and the reason is what a case costs here: the second property
    // runs a whole search over radius, which is two dozen searches over the globe, so the default would put
    // this module past ten seconds on its own. 64 draws still cover the range densely at this fixture's
    // scale — the answers for shares between 1% and 100% span some twenty distinct radii.
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// The property bisection rests on, over drawn radii rather than chosen ones: widening a circle
        /// cannot lose people, so the maximum over centres cannot fall as the radius grows.
        ///
        /// Two fixtures and two strictnesses, because they are different claims. On `distinct` every
        /// partial sum is representable, so any inversion at all is a defect and the assertion is exact.
        /// On `scaled` the sums round, so the honest claim is the derived slack — issue #7's comment is
        /// right that this can invert and wrong about the magnitude, which is `predicate_slack_persons`.
        #[test]
        fn the_maximum_never_falls_as_the_radius_grows(
            rounding in proptest::bool::ANY,
            from in 0u32..8000,
            step in 1u32..4000,
        ) {
            let grid = grid();
            let payload = if rounding {
                payload_over(&grid, scaled(&grid))
            } else {
                payload_over(&grid, distinct(&grid))
            };
            let table = Table::new(grid, &payload).expect("the build emits the padded product");
            let (rows, cols) = table.whole();
            let slack = if rounding {
                predicate_slack_persons(&grid, table.population(rows, cols))
            } else {
                0.0
            };

            let near = maximum_at(&table, from, 6).population;
            let far = maximum_at(&table, from + step, 6).population;
            prop_assert!(
                far >= near - slack,
                "{} km holds {near} and {} km holds {far}, a fall of {} past a slack of {slack}",
                from,
                from + step,
                near - far
            );
        }

        /// The bracket, over drawn shares: the answer reaches the target and the kilometre below it does
        /// not. This is the claim the result publishes — minimality itself rests on the property above,
        /// which is why the two are tested together and stated apart.
        #[test]
        fn the_answer_reaches_and_the_radius_below_it_falls_short(share_of in 0.01f64..1.0) {
            let grid = grid();
            let payload = payload_over(&grid, distinct(&grid));
            let table = Table::new(grid, &payload).expect("the build emits the padded product");

            let answer = found(&table, share_of, 6);
            prop_assert!(answer.centre.population >= answer.target.persons);

            if let Some((km, population)) = answer.short_below {
                prop_assert_eq!(km, answer.radius_km - 1);
                prop_assert!(population < answer.target.persons);
                // The witness is a probe, not a claim: the same radius searched again is short too.
                prop_assert!(maximum_at(&table, km, 6).population < answer.target.persons);
            } else {
                // Nothing below zero to have proved short, which is the only way the witness is absent.
                prop_assert_eq!(answer.radius_km, 0);
            }
        }
    }
}
