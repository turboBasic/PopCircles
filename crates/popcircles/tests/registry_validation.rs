// The end-to-end run against the real dataset: the registry raster streamed into a table, and the
// smallest circle holding half the world's population searched for over it. `tests/registry_raster.rs`
// pins the reader against that file; this pins everything above the reader against it, which is the one
// thing no synthetic fixture can do — a fixture cannot disagree with the published prior art.
//
// **Decimated to 5 arcmin rather than full resolution, and that is a cost decision with a measured
// reason.** At full resolution one radius costs 207 s of which 13 s is CPU — the run is page faults
// against a 7.5 GB mmap — and a search over radius probes two dozen of them. So the cheap grid
// brackets and the expensive grid certifies: this test is the bracket, and the full-resolution
// certification of the same answer is recorded in issue #10 rather than run here.
//
// Skipped with a message rather than failed when the raster is absent, which is box 2 of
// issue #10. A `#[test]` cannot skip at runtime, so the skip is an early return that says why — the
// alternative being a red suite on every clone that has not run `mise run data:get`.
//
// expect is what a test documents an invariant with; docs/ai/code.md allows it here.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::num::NonZeroU32;
use std::path::PathBuf;

use popcircles::geodesy::LatLon;
use popcircles::grid::Grid;
use popcircles::raster::geotiff::GeoTiffSource;
use popcircles::raster::{PixelType, RasterError, RasterSpec};
use popcircles::smallest::{Share, smallest};
use popcircles::table::{Decimation, Table, build};

/// Every figure here is the `population-count-2020-30arcsec` row of `data/registry.toml`. Literals
/// for `tests/registry_raster.rs`'s reason: #8 owns how a user supplies a spec.
const WIDTH: u32 = 43200;
const HEIGHT: u32 = 21600;
const NODATA: f32 = -3.402_823e38;
const WORLD_TOTAL: f64 = 7_757_982_599.32;

/// The decimated grid, and the shape `README.md`'s worked example uses.
const DECIMATE: u32 = 10;

/// The spacing the search tiles its first level into. Measured rather than chosen: issue #10's sweep found
/// the wall clock falls monotonically with spacing and flattens from about a sixteenth of the grid's
/// width, so this is on the plateau. It changes how long the search takes and not what it answers, which
/// is why a test may hold one at all — see `crates/popcircles-cli/src/main.rs`'s `SearchArgs`.
const SPACING: u32 = 256;

/// What the 5 arcmin table answers for half the world, measured on 2026-08-15.
///
/// A band rather than the figure, and a wide one: the assertion's job is that the whole path agrees with
/// the published prior art on this dataset — Danny Quah's ~3300 km, and the Valeriepieris circle before
/// it — not that a later change to the earth model or the fold reproduces this bit pattern. A regression
/// that matters lands outside 40 km; one that does not lands inside it. `README.md`'s Validation section records
/// the exact figure and where the remaining 60 km against 3300 comes from.
const EXPECTED_RADIUS_KM: f64 = 3360.0;
const RADIUS_BAND_KM: f64 = 40.0;

/// The centre, to a degree, which is Yunnan. Pinned because a radius alone would pass on a search that had
/// found the wrong maximum at the right size.
const EXPECTED_CENTRE: LatLon = LatLon {
    lat: 28.79,
    lon: 100.62,
};
const CENTRE_BAND_DEG: f64 = 1.0;

fn registry_raster() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "data",
        "population",
        "population-count-2020-30arcsec.tif",
    ]
    .iter()
    .collect()
}

#[test]
#[ignore = "reads the registry raster from data/; run `mise run test:validate`"]
fn the_registry_raster_answers_the_published_half_of_the_world() {
    let source_grid = Grid::new(
        WIDTH,
        HEIGHT,
        LatLon {
            lat: 90.0,
            lon: -180.0,
        },
        1.0 / 120.0,
        -1.0 / 120.0,
    )
    .expect("the registry row describes a valid grid");
    let spec = RasterSpec {
        grid: source_grid,
        epsg: 4326,
        pixel: PixelType::Float32,
        nodata: NODATA,
    };

    let path = registry_raster();
    let source = match GeoTiffSource::open(&path, &spec) {
        Ok(source) => source,
        // The one failure that is not a defect: a clone that has not fetched the raster. Everything else
        // — a truncated file, tags that disagree with the row above — is a real failure and is reported
        // as one.
        Err(RasterError::Absent { .. }) => {
            eprintln!(
                "skipped: {} is not there. Run `mise run data:get` first.",
                path.display()
            );
            return;
        }
        Err(error) => panic!("the registry raster did not open: {error}"),
    };

    let decimation = Decimation::new(source_grid, DECIMATE)
        .expect("ten divides both of the registry's dimensions");
    let mut cells = Vec::new();
    let built = build(source, decimation, &mut (), |row| {
        cells.extend_from_slice(row);
        Ok::<(), std::convert::Infallible>(())
    })
    .expect("every strip of the registry raster decodes");

    // The same check `tests/registry_raster.rs` makes, here because it costs nothing and because it is
    // what says the table below was built from the dataset the band was measured on. The tolerance is the
    // registry's own quoted precision.
    assert!(
        (built.total - WORLD_TOTAL).abs() < 0.01,
        "world total {} against the registry's {WORLD_TOTAL}",
        built.total
    );
    assert_eq!(built.tallies.total(), 933_120_000);
    assert_eq!(built.tallies.unexpected_negative, 0);

    let grid = *decimation.grid();
    let table = Table::new(grid, &cells).expect("the build emits the padded product");
    let spacing = NonZeroU32::new(SPACING).expect("the spacing is not zero");
    let share = Share::new(0.5).expect("a half is a share");
    // No ledger: a test that resumed from one would be pinning whatever a previous run left on the
    // machine, and `()` is the implementation for a caller that wants no resumption.
    let found = smallest(&table, share, Some(spacing), &mut (), &mut ())
        .expect("a whole-globe table and half its population");

    let radius = f64::from(found.radius_km);
    assert!(
        (radius - EXPECTED_RADIUS_KM).abs() <= RADIUS_BAND_KM,
        "half the world is reached at {radius} km, outside {EXPECTED_RADIUS_KM} ± {RADIUS_BAND_KM}"
    );

    // Minimality, from the search's own report rather than from a second run: the bracket it proved is
    // published beside the answer precisely so a caller need not re-derive it.
    let (short_km, short_population) = found
        .short_below
        .expect("an answer above 0 km has a radius below it");
    assert_eq!(short_km, found.radius_km - 1);
    assert!(
        short_population < found.target.persons,
        "the radius below the answer reached the target: {short_population} against {}",
        found.target.persons
    );

    // The circle holds at least what was asked for. Its own assertion because it is what the answer
    // means, and the band above would pass a search that returned a radius near the right one without
    // reaching the target at all.
    assert!(
        found.centre.population >= found.target.persons,
        "the answer holds {} against a target of {}",
        found.centre.population,
        found.target.persons
    );

    // Unambiguous on this dataset, and worth pinning rather than merely observing: at half the world the
    // margin is six orders of magnitude outside the summation slack, so a later change that made this
    // answer ambiguous would be a change in the arithmetic and not in the question.
    assert!(
        found.ambiguity.is_none(),
        "half the world is not a plateau on this dataset, yet {:?} was reported",
        found.ambiguity
    );

    let centre = grid.centre_of(found.centre.row, found.centre.col);
    assert!(
        (centre.lat - EXPECTED_CENTRE.lat).abs() <= CENTRE_BAND_DEG
            && (centre.lon - EXPECTED_CENTRE.lon).abs() <= CENTRE_BAND_DEG,
        "the centre {centre:?} is outside a degree of {EXPECTED_CENTRE:?}"
    );
}
