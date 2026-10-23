use std::path::Path;

use popcircles::report::{
    CountryListDocument, CountrySmallestDocument, Envelope, LedgerReport, SmallestReport,
};
use popcircles::smallest::cache::CountryLedger;
use popcircles::smallest::{self, Share};
use popcircles::table::Decimation;

use crate::args::{CountryTableArgs, SearchArgs};
use crate::boundaries;
use crate::commands::{make_room_for, serialised};
use crate::country_breakdown::Prepared;
use crate::failure::{EXIT_MISSING_DATA, Failure};
use crate::observe::StderrProgress;
use crate::registry::{COUNTRY_BOUNDARY_DATASET, REGISTRY_PATH, Registry};

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
    let decimation =
        Decimation::new(source, args.decimate).map_err(|error| Failure::table(&error))?;
    let prepared = Prepared::load(
        args.grid.dataset.as_deref(),
        source,
        *decimation.grid(),
        args.decimate,
    )?;
    let feature = prepared.feature(selector)?;
    let table = prepared.country_table(feature)?.ok_or_else(|| {
        Failure::bad_input(format!(
            "country {} owns no population cells at this resolution",
            feature.id.as_str()
        ))
    })?;
    let path = ledger_path.map_or_else(
        || Path::new("out/countries").join(format!("{}.json", feature.id.as_str())),
        Path::to_path_buf,
    );
    make_room_for(&path)?;
    let mut ledger = CountryLedger::open_or_empty(&path, table.identity())
        .map_err(|error| Failure::ledger(&error))?;
    let mut progress = StderrProgress::new();
    let found =
        smallest::smallest_country(&table, share, search.spacing, &mut ledger, &mut progress)
            .map_err(|error| Failure::smallest(&error))?;
    progress.finish();

    let circle = SmallestReport::new(&found, table.effective_grid())
        .map_err(|error| Failure::grid_index(&error))?;
    let (boundary_dataset, boundary_revision, boundary_digest) = prepared.boundary_identity();
    serialised(serde_json::to_string(&Envelope::new(
        CountrySmallestDocument::new(
            &feature.id,
            &feature.name,
            boundary_dataset,
            boundary_revision,
            boundary_digest,
            LedgerReport::new(&path, ledger.len()),
            circle,
        ),
    )))
}
