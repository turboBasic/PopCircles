// The four commands that search: one circle about a coordinate, the most populous of a fixed radius,
// the smallest reaching a share, and a sweep of shares over one ledger. They share a module because
// each is a thin call into the library's own search — the branch and bound, and the search over radius
// above it — and because the last two share a ledger.

use std::path::Path;

use popcircles::circle;
use popcircles::geodesy::{LatLon, RadiusKm};
use popcircles::grid::{Col, Grid, Row};
use popcircles::kernel::Kernel;
use popcircles::report::{
    CircleReport, Envelope, LedgerReport, MostPopulousReport, SmallestDocument, SmallestReport,
    SweepDocument, SweepShares,
};
use popcircles::search;
use popcircles::smallest::cache::Ledger;
use popcircles::smallest::{self, Share};
use popcircles::table::cache::Identity;

use crate::args::{CachedTableArgs, LedgerArgs, SearchArgs, SweepArgs};
use crate::commands::{CachedTable, make_room_for, serialised};
use crate::failure::Failure;
use crate::observe::StderrProgress;

/// The cell holding `at`, or bad input naming the extent it is not on.
///
/// A function of its own so both arms are testable without a table: what makes a coordinate bad input is
/// the grid, and opening a cache to find that out would put a fixture in the way of the check.
pub(crate) fn centre_cell(grid: &Grid, at: LatLon) -> Result<(Row, Col), Failure> {
    grid.cell_containing(at).ok_or_else(|| {
        Failure::bad_input(format!(
            "(lat {}, lon {}) is not on a {} x {} grid whose origin is (lat {}, lon {}); a coordinate \
             on the grid's outer southern or eastern boundary lies in no cell",
            at.lat,
            at.lon,
            grid.width(),
            grid.height(),
            grid.origin().lat,
            grid.origin().lon
        ))
    })
}

pub(crate) fn population_at(
    cached: &CachedTableArgs,
    lat: f64,
    lon: f64,
    radius: RadiusKm,
) -> Result<String, Failure> {
    let cached = CachedTable::open(cached)?;
    let view = cached.table()?;
    let requested = LatLon { lat, lon };
    let cell = centre_cell(&cached.grid, requested)?;

    let kernel = Kernel::new(cached.grid, cell.0, radius).map_err(Failure::kernel)?;
    let population = circle::population(&view, &kernel, cell.1);
    let (rows, cols) = view.whole();
    let total = view.population(rows, cols);

    let centre = cached.grid.centre_of(cell.0, cell.1);
    log::info!(
        "a {} km circle centred (lat {:.4}, lon {:.4}) holds {population} of {total}",
        radius.km(),
        centre.lat,
        centre.lon
    );

    serialised(serde_json::to_string(&Envelope::with_provenance(
        CircleReport::new(requested, cell, &cached.grid, radius, population, total),
        cached.provenance(),
    )))
}

pub(crate) fn most_populous(
    cached: &CachedTableArgs,
    radius: RadiusKm,
    search: SearchArgs,
) -> Result<String, Failure> {
    let cached = CachedTable::open(cached)?;
    let view = cached.table()?;
    let (rows, cols) = view.whole();
    let total = view.population(rows, cols);

    let mut progress = StderrProgress::new();
    let found = search::most_populous(&view, radius, search.spacing, &mut progress)
        .map_err(Failure::search)?;
    progress.finish();

    let centre = cached.grid.centre_of(found.centre.row, found.centre.col);
    log::info!(
        "the most populous {} km circle is centred (lat {:.4}, lon {:.4}) and holds {} of {total}",
        radius.km(),
        centre.lat,
        centre.lon,
        found.centre.population
    );

    serialised(serde_json::to_string(&Envelope::with_provenance(
        MostPopulousReport::new(&found, &cached.grid, total),
        cached.provenance(),
    )))
}

/// The ledger at `path` for the table `wanted` names, with room made for it.
///
/// The one place this crate opens one, so a sweep cannot open a ledger per share: what a ledger records is
/// the maximum at a radius, a property of the table alone, so a twenty-five percent share reuses every
/// radius a fifty percent share paid for.
pub(crate) fn open_ledger(path: &Path, wanted: &Identity) -> Result<Ledger, Failure> {
    make_room_for(path)?;
    Ledger::open_or_empty(path, wanted).map_err(|error| Failure::ledger(&error))
}

/// The shares a sweep walks, ascending, each converted by [`Share::from_percent`].
///
/// A function of its own so the count and every rejection are testable without a table. The walk is over
/// integers, and the two grounds below are relations between flags rather than properties of one, which is
/// why they are not a value parser's to refuse.
pub(crate) fn shares(from: u32, to: u32, step: u32) -> Result<Vec<Share>, Failure> {
    if step == 0 {
        return Err(Failure::bad_input(
            "a sweep's step must be at least one percent; a step of zero never reaches its end",
        ));
    }
    if from > to {
        return Err(Failure::bad_input(format!(
            "a sweep runs from the smaller share to the larger; {from}% is above {to}%"
        )));
    }

    let mut walk = Vec::new();
    let mut percent = from;
    loop {
        walk.push(
            Share::from_percent(percent).map_err(|error| Failure::bad_input(error.to_string()))?,
        );
        // Saturating, so a step near `u32::MAX` ends the walk rather than wrapping back under `to`.
        let next = percent.saturating_add(step);
        if next > to {
            return Ok(walk);
        }
        percent = next;
    }
}

pub(crate) fn sweep(
    cached: &CachedTableArgs,
    range: SweepArgs,
    search: SearchArgs,
    ledger: &LedgerArgs,
) -> Result<String, Failure> {
    let cached = CachedTable::open(cached)?;
    let view = cached.table()?;
    let walk = shares(range.from, range.to, range.step)?;
    let mut ledger = open_ledger(&ledger.ledger, &cached.identity)?;

    let mut progress = StderrProgress::new();
    let mut records = Vec::with_capacity(walk.len());
    for share in walk {
        let found = smallest::smallest(&view, share, search.spacing, &mut ledger, &mut progress)
            .map_err(|error| Failure::smallest(&error))?;
        // One per settled share rather than one at the end: a sweep's answer is the whole sequence, and a
        // reader watching a long one wants each share as it lands.
        log::info!(
            "{:.0}% of the table is reached at {} km",
            share.get() * 100.0,
            found.radius_km
        );
        records.push(SmallestReport::new(&found, &cached.grid));
    }
    progress.finish();

    serialised(serde_json::to_string(&Envelope::with_provenance(
        SweepDocument::new(
            LedgerReport::new(ledger.path(), ledger.len()),
            SweepShares::new(range.from, range.to, range.step),
            records,
        ),
        cached.provenance(),
    )))
}

pub(crate) fn smallest_for_share(
    cached: &CachedTableArgs,
    share: Share,
    search: SearchArgs,
    ledger: &LedgerArgs,
) -> Result<String, Failure> {
    let cached = CachedTable::open(cached)?;
    let view = cached.table()?;

    // Against the same identity the table was opened with, so a ledger of some other table is refused
    // rather than resumed from — which is what makes an opt-out unnecessary.
    let mut ledger = open_ledger(&ledger.ledger, &cached.identity)?;

    let mut progress = StderrProgress::new();
    let found = smallest::smallest(&view, share, search.spacing, &mut ledger, &mut progress)
        .map_err(|error| Failure::smallest(&error))?;
    progress.finish();

    log::info!(
        "{:.0}% of the table is reached at {} km",
        share.get() * 100.0,
        found.radius_km
    );

    serialised(serde_json::to_string(&Envelope::with_provenance(
        SmallestDocument::new(
            LedgerReport::new(ledger.path(), ledger.len()),
            SmallestReport::new(&found, &cached.grid),
        ),
        cached.provenance(),
    )))
}

// expect is warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows it in tests. No unwrap_used beside it: every assertion below reaches
// for expect_err or expect, and a lint nothing here trips would not be reviewable.
#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::failure::EXIT_BAD_INPUT;

    use super::*;

    /// A four-by-three whole-globe grid, which is what the parsing tests below declare and what
    /// `centre_cell` is checked against: the smallest shape whose columns close.
    fn coarse_grid() -> Grid {
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

    #[test]
    fn a_coordinate_off_the_grid_is_bad_input_naming_the_extent() {
        let grid = coarse_grid();
        let inside = centre_cell(
            &grid,
            LatLon {
                lat: 45.0,
                lon: -90.0,
            },
        )
        .expect("the coordinate is on the grid");
        assert_eq!((inside.0.get(), inside.1.get()), (0, 1));

        // The outer southern boundary lies in no cell, which is `Grid::cell_containing`'s rule rather than
        // this crate's, and the message has to say enough for a caller to see which extent it missed.
        let outside = centre_cell(
            &grid,
            LatLon {
                lat: -90.0,
                lon: 0.0,
            },
        )
        .expect_err("the south pole is on no row of this grid");
        assert_eq!(outside.code, EXIT_BAD_INPUT);
        assert!(outside.message.contains("4 x 3"), "{}", outside.message);
        assert!(outside.message.contains("lat 90"), "{}", outside.message);
    }

    #[test]
    fn a_sweep_walks_whole_percent_and_ends_on_its_last_share() {
        let walk = shares(10, 90, 10).expect("a sweep from a tenth to nine tenths is a range");
        assert_eq!(walk.len(), 9);
        assert_eq!(walk.first().map(|share| share.get()), Some(0.1));
        assert_eq!(walk.last().map(|share| share.get()), Some(0.9));
        // Ascending and exact, which is what the records inherit.
        let values: Vec<f64> = walk.iter().map(|share| share.get()).collect();
        assert_eq!(values, vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9]);

        // A share the step would carry past the end is not answered, which is what a stepped range means.
        let short =
            shares(10, 95, 10).expect("a range whose step overshoots its end is still a range");
        assert_eq!(short.len(), 9);
        assert_eq!(short.last().map(|share| share.get()), Some(0.9));

        // One share is a sweep of one, and everyone is a share.
        let whole = shares(100, 100, 10).expect("a hundred percent is a share");
        assert_eq!(whole.len(), 1);
        assert_eq!(whole.first().map(|share| share.get()), Some(1.0));
    }

    #[test]
    fn a_sweep_with_no_step_is_refused() {
        // Refused rather than looping: a step of zero would settle the first share for ever.
        let none = shares(10, 90, 0).expect_err("zero is not a step");
        assert_eq!(none.code, EXIT_BAD_INPUT);
        assert!(none.message.contains("step"), "{}", none.message);
    }

    #[test]
    fn a_sweep_from_no_share_is_refused() {
        // `Share::from_percent` refuses it, and the reason is the domain's: a circle holding nobody is
        // satisfied by every radius there is.
        let empty = shares(0, 90, 10).expect_err("zero percent is not a share");
        assert_eq!(empty.code, EXIT_BAD_INPUT);
        // Past a hundred too, wherever the walk reaches it, rather than being silently truncated.
        assert!(shares(90, 150, 10).is_err());
    }

    #[test]
    fn a_sweep_that_runs_backwards_is_refused_rather_than_empty() {
        // The failure this one exists to prevent is the quiet one: a descending range yielding nothing at
        // all, and a document of zero records reading as a table with nobody in it.
        let backwards = shares(60, 40, 10).expect_err("a sweep does not run backwards");
        assert_eq!(backwards.code, EXIT_BAD_INPUT);
        assert!(backwards.message.contains("60%"), "{}", backwards.message);
    }
}
