use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use popcircles::report::{
    CountryListDocument, CountrySmallestDocument, CountrySweepCountry, CountrySweepDocument,
    CountrySweepRecord, Envelope, LedgerReport, SmallestReport, SweepShares,
};
use popcircles::smallest::cache::CountryLedger;
use popcircles::smallest::{self, Share};
use popcircles::table::Decimation;

use crate::args::{CountryTableArgs, SearchArgs, SweepArgs};
use crate::boundaries;
use crate::commands::search::{settlement_context, settlement_places, shares};
use crate::commands::{make_room_for, serialised};
use crate::country_breakdown::Prepared;
use crate::failure::{EXIT_MISSING_DATA, Failure};
use crate::observe::StderrProgress;
use crate::registry::{COUNTRY_BOUNDARY_DATASET, REGISTRY_PATH, Registry};

fn country_ledger_path(directory: &Path, id: &str) -> PathBuf {
    let mut file = String::new();
    for byte in id.bytes() {
        let _ = write!(file, "{byte:02x}");
    }
    directory.join(format!("{file}.json"))
}

pub(crate) fn list() -> Result<String, Failure> {
    let registry =
        Registry::load(Path::new(REGISTRY_PATH)).map_err(|error| Failure::registry(&error))?;
    let boundary = registry.boundary(COUNTRY_BOUNDARY_DATASET).ok_or_else(|| {
        Failure::bad_input(format!(
            "country boundary `{COUNTRY_BOUNDARY_DATASET}` is not registered"
        ))
    })?;
    let source_id_field = boundary.source_id_field.as_deref().ok_or_else(|| {
        Failure::bad_input(format!(
            "country boundary `{COUNTRY_BOUNDARY_DATASET}` has no source identifier field"
        ))
    })?;
    let features = boundaries::load(&boundary.path, source_id_field).map_err(|error| {
        if boundary.path.exists() {
            Failure::bad_input(error.to_string())
        } else {
            Failure {
                code: EXIT_MISSING_DATA,
                message: format!(
                    "{error}; run `mise run data:get` to fetch the registered country boundary"
                ),
            }
        }
    })?;
    let mut countries: Vec<_> = features
        .into_iter()
        .map(|feature| (feature.id, feature.name))
        .collect();
    countries.sort_by(|left, right| left.0.as_str().cmp(right.0.as_str()));

    serialised(serde_json::to_string(&Envelope::new(
        CountryListDocument::new(countries),
    )))
}

pub(crate) fn smallest(
    selector: &str,
    share: Share,
    args: &CountryTableArgs,
    search: SearchArgs,
    ledger_path: Option<&Path>,
) -> Result<String, Failure> {
    let source = args.grid.grid()?;
    let factor = args.decimation(1);
    let decimation = Decimation::new(source, factor).map_err(|error| Failure::table(&error))?;
    let prepared = Prepared::load(
        args.grid.dataset.as_deref(),
        source,
        *decimation.grid(),
        factor,
    )?;
    let feature = prepared.feature(selector)?;
    let table = prepared.country_table(feature)?.ok_or_else(|| {
        Failure::bad_input(format!(
            "country {} owns no population cells at this resolution",
            feature.id.as_str()
        ))
    })?;
    let path = ledger_path.map_or_else(
        || country_ledger_path(Path::new("out/countries"), feature.id.as_str()),
        Path::to_path_buf,
    );
    make_room_for(&path)?;
    let mut ledger = CountryLedger::open_or_empty(&path, table.identity())
        .map_err(|error| Failure::ledger(&error))?;
    let mut progress = StderrProgress::new();
    let places = settlement_places()?;
    let found =
        smallest::smallest_country(&table, share, search.spacing, &mut ledger, &mut progress)
            .map_err(|error| Failure::smallest(&error))?;
    progress.finish();

    let centre = table
        .effective_grid()
        .centre_of(found.centre.row, found.centre.col)
        .map_err(|error| Failure::grid_index(&error))?;
    let context = settlement_context(centre, &places)?;
    let circle = SmallestReport::new(&found, table.effective_grid())
        .map_err(|error| Failure::grid_index(&error))?
        .with_nearest_settlement(context.as_ref());
    let boundary_identity = prepared.boundary_identity();
    let population_source = prepared.population_source();
    serialised(serde_json::to_string(&Envelope::new(
        CountrySmallestDocument::new(
            &feature.id,
            &feature.name,
            boundary_identity,
            population_source,
            LedgerReport::new(&path, ledger.len()),
            circle,
        ),
    )))
}

pub(crate) fn sweep(
    selectors: &[String],
    range: SweepArgs,
    args: &CountryTableArgs,
    search: SearchArgs,
    ledger_dir: &Path,
) -> Result<String, Failure> {
    let source = args.grid.grid()?;
    let factor = args.decimation(10);
    let decimation = Decimation::new(source, factor).map_err(|error| Failure::table(&error))?;
    let prepared = Prepared::load(
        args.grid.dataset.as_deref(),
        source,
        *decimation.grid(),
        factor,
    )?;
    let walk = shares(range.from, range.to, range.step)?;
    let mut features = selectors
        .iter()
        .map(|selector| prepared.feature(selector))
        .collect::<Result<Vec<_>, _>>()?;
    features.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    features.dedup_by(|left, right| left.id == right.id);

    let (boundary_dataset, boundary_revision, boundary_digest) = prepared.boundary_identity();
    let mut countries = Vec::with_capacity(features.len());
    for feature in features {
        let started = std::time::Instant::now();
        let Some(table) = prepared.country_table(feature)? else {
            eprintln!(
                "{}: no owned population cells at this resolution",
                feature.id.as_str()
            );
            continue;
        };
        let path = country_ledger_path(ledger_dir, feature.id.as_str());
        make_room_for(&path)?;
        let mut ledger = CountryLedger::open_or_empty(&path, table.identity())
            .map_err(|error| crate::failure::Failure::ledger(&error))?;
        let mut progress = StderrProgress::new();
        let mut records = Vec::with_capacity(walk.len());
        for (index, share) in walk.iter().copied().enumerate() {
            let found = smallest::smallest_country(
                &table,
                share,
                search.spacing,
                &mut ledger,
                &mut progress,
            )
            .map_err(|error| Failure::smallest(&error))?;
            log::info!(
                "{}: {:.0}% reached at {} km ({}/{})",
                feature.id.as_str(),
                share.get() * 100.0,
                found.radius_km,
                index + 1,
                walk.len()
            );
            records.push(CountrySweepRecord::new(
                SmallestReport::new(&found, table.effective_grid())
                    .map_err(|error| Failure::grid_index(&error))?,
            ));
        }
        progress.finish();
        log::info!(
            "{}: completed in {:.3}s (crop {} × {})",
            feature.id.as_str(),
            started.elapsed().as_secs_f64(),
            table.grid().width(),
            table.grid().height()
        );
        countries.push(CountrySweepCountry::new(
            &feature.id,
            &feature.name,
            boundary_dataset,
            boundary_revision,
            boundary_digest,
            LedgerReport::new(&path, ledger.len()),
            records,
        ));
    }

    serialised(serde_json::to_string(&Envelope::new(
        CountrySweepDocument::new(
            SweepShares::new(range.from, range.to, range.step),
            countries,
        ),
    )))
}
