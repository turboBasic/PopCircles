use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use popcircles::report::{
    CountryListDocument, CountrySelectionReport, CountrySmallestDocument, CountrySourceReport,
    CountrySweepCountry, CountrySweepDocument, CountrySweepRecord, Document, Envelope,
    LedgerReport, SmallestReport, SweepShares,
};
use popcircles::smallest::cache::CountryLedger;
use popcircles::smallest::{self, Share};
use popcircles::table::CountryTable;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tempfile::Builder;

use crate::args::{CountryTableArgs, SearchArgs, SweepArgs};
use crate::boundaries;
use crate::commands::search::{settlement_context, settlement_places, shares};
use crate::commands::{make_room_for, serialised};
use crate::config::{Combination, Configuration};
use crate::country_breakdown::Prepared;
use crate::failure::{EXIT_BAD_INPUT, EXIT_MISSING_DATA, Failure};
use crate::observe::StderrProgress;
use crate::prepared::{Loaded, PreparedManifest, Request};
use crate::registry::{REGISTRY_PATH, Registry};

fn country_ledger_path(directory: &Path, id: &str) -> PathBuf {
    let mut file = String::new();
    for byte in id.bytes() {
        let _ = write!(file, "{byte:02x}");
    }
    directory.join(format!("{file}.json"))
}

pub(crate) fn list(
    config_path: Option<&Path>,
    combination_id: Option<&str>,
    resolution: Option<u32>,
) -> Result<String, Failure> {
    let config =
        Configuration::load(config_path).map_err(|error| Failure::bad_input(error.to_string()))?;
    let (combination, resolution) = config
        .combination(combination_id, resolution)
        .map_err(|error| Failure::bad_input(error.to_string()))?;
    let registry =
        Registry::load(Path::new(REGISTRY_PATH)).map_err(|error| Failure::registry(&error))?;
    let population = query_population_source(combination, &registry)?;
    let boundary = query_boundary_source(combination, &registry)?;
    let boundary_sha256 = crate::prepared::fingerprint_file(&boundary.path).map_err(|error| {
        Failure {
            code: EXIT_MISSING_DATA,
            message: format!(
                "selected boundary source at {} could not be read: {error}; fetch the registered boundary data or provide the configured local file",
                boundary.path.display()
            ),
        }
    })?;
    if boundary
        .registered_sha256
        .as_deref()
        .is_some_and(|expected| expected != boundary_sha256)
    {
        return Err(Failure::bad_input(format!(
            "selected boundary source at {} does not match its registered SHA-256",
            boundary.path.display()
        )));
    }
    let features = boundaries::load_with_iso_source(
        &boundary.path,
        &boundary.id_field,
        boundary.iso_code_source,
    )
    .map_err(|error| {
        if boundary.path.exists() {
            Failure::bad_input(error.to_string())
        } else {
            Failure {
                code: EXIT_MISSING_DATA,
                message: format!(
                    "{error}; fetch the registered boundary data or provide the configured local file"
                ),
            }
        }
    })?;
    let mut countries: Vec<_> = features
        .into_iter()
        .map(|feature| (feature.id, feature.name, feature.iso_alpha2))
        .collect();
    countries.sort_by(|left, right| left.0.as_str().cmp(right.0.as_str()));

    let selection = CountrySelectionReport::new(
        combination.id.clone(),
        resolution,
        CountrySourceReport::new(
            &source_name(&combination.population_source),
            population.sha256.as_deref(),
            combination
                .attribution
                .as_deref()
                .or(population.attribution.as_deref()),
        ),
        CountrySourceReport::new(
            &source_name(&combination.boundary_source),
            Some(&boundary_sha256),
            boundary.attribution.as_deref(),
        ),
    );
    serialised(serde_json::to_string(&Envelope::new(
        CountryListDocument::new_with_iso_codes(selection, countries),
    )))
}

#[derive(Debug, Serialize)]
struct CombinationListDocument {
    combinations: Vec<CombinationListEntry>,
}

impl Document for CombinationListDocument {
    const KIND: &'static str = "country-combinations";
}

#[derive(Debug, Serialize)]
struct CombinationListEntry {
    id: String,
    population_source: String,
    boundary_source: String,
    available_resolutions: Vec<u32>,
    default_resolution: u32,
    is_default: bool,
}

pub(crate) fn list_combinations(config_path: Option<&Path>) -> Result<String, Failure> {
    let config =
        Configuration::load(config_path).map_err(|error| Failure::bad_input(error.to_string()))?;
    let combinations = config
        .combinations
        .values()
        .map(|combination| CombinationListEntry {
            id: combination.id.clone(),
            population_source: combination.population_source.clone(),
            boundary_source: combination.boundary_source.clone(),
            available_resolutions: combination.resolutions.keys().copied().collect(),
            default_resolution: combination.default_resolution,
            is_default: combination.id == config.default_country_combination,
        })
        .collect();
    serialised(serde_json::to_string(&Envelope::new(
        CombinationListDocument { combinations },
    )))
}

#[derive(Debug, Serialize)]
struct PreparedCombinationDocument {
    combination_id: String,
    resolution: u32,
    directory: PathBuf,
}

impl Document for PreparedCombinationDocument {
    const KIND: &'static str = "country-prepared";
}

pub(crate) fn prepare(
    config_path: Option<&Path>,
    combination_id: Option<&str>,
    resolution: Option<u32>,
    replace: bool,
) -> Result<String, Failure> {
    let config =
        Configuration::load(config_path).map_err(|error| Failure::bad_input(error.to_string()))?;
    let (combination, resolution) = config
        .combination(combination_id, resolution)
        .map_err(|error| Failure::bad_input(error.to_string()))?;
    Prepared::prepare_combination(combination, resolution, replace)?;
    let directory = combination
        .resolutions
        .get(&resolution)
        .and_then(|location| location.local.as_ref())
        .ok_or_else(|| {
            Failure::bad_input(format!(
                "combination `{}` resolution {resolution} has no local output directory",
                combination.id
            ))
        })?;
    prepared_document(combination, resolution, directory)
}

pub(crate) fn fetch(
    config_path: Option<&Path>,
    combination_id: Option<&str>,
    resolution: Option<u32>,
    replace: bool,
) -> Result<String, Failure> {
    let config =
        Configuration::load(config_path).map_err(|error| Failure::bad_input(error.to_string()))?;
    let (combination, resolution) = config
        .combination(combination_id, resolution)
        .map_err(|error| Failure::bad_input(error.to_string()))?;
    let location = &combination.resolutions[&resolution];
    let remote = location.remote.as_deref().ok_or_else(|| {
        Failure::bad_input(format!(
            "combination `{}` resolution {resolution} has no remote URL",
            combination.id
        ))
    })?;
    let directory = location.local.as_deref().ok_or_else(|| {
        Failure::bad_input(format!(
            "combination `{}` resolution {resolution} has no local install directory",
            combination.id
        ))
    })?;
    validate_remote(remote)?;
    let registry =
        Registry::load(Path::new(REGISTRY_PATH)).map_err(|error| Failure::registry(&error))?;
    let population = if let Some(name) = combination.population_source.strip_prefix("dataset:") {
        let dataset = registry
            .raster(name)
            .map_err(|error| Failure::registry(&error))?;
        (
            dataset.grid.grid().map_err(|error| Failure::grid(&error))?,
            dataset.epsg,
            dataset.nodata,
            dataset.sha256,
        )
    } else {
        let spec = combination
... (+906 lines) [see remaining: rtk proxy git -c core.fsmonitor=false show HEAD:crates/popcircles-cli/src/commands/country.rs | tail -n +230]
