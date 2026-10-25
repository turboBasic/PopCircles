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
    let features = boundaries::load(&boundary.path, &boundary.id_field).map_err(|error| {
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
        .map(|feature| (feature.id, feature.name))
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
        CountryListDocument::new(selection, countries),
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
            .population
            .ok_or_else(|| Failure::bad_input("local population source metadata is missing"))?;
        (spec.grid, spec.epsg, spec.nodata, None)
    };
    let (boundary_id_field, boundary_sha256) =
        if let Some(name) = combination.boundary_source.strip_prefix("dataset:") {
            let boundary = registry.boundary(name).ok_or_else(|| {
                Failure::bad_input(format!("`{name}` is not a registered boundary dataset"))
            })?;
            let field = boundary.source_id_field.clone().ok_or_else(|| {
                Failure::bad_input(format!("boundary dataset `{name}` has no source ID field"))
            })?;
            (field, boundary.sha256.clone())
        } else {
            let field = combination
                .boundary_source_id_field
                .clone()
                .ok_or_else(|| Failure::bad_input("local boundary source ID field is missing"))?;
            (field, None)
        };
    let request = Request {
        combination_id: &combination.id,
        resolution,
        population_source: &combination.population_source,
        population_sha256: population.3.as_deref(),
        boundary_source: &combination.boundary_source,
        boundary_sha256: boundary_sha256.as_deref(),
        population_epsg: population.1,
        population_nodata: population.2,
        boundary_source_id_field: &boundary_id_field,
        source_grid: population.0,
    };
    fetch_bundle(remote, directory, &request, replace, false)?;
    prepared_document(combination, resolution, directory)
}

fn fetch_bundle(
    remote: &str,
    directory: &Path,
    request: &Request<'_>,
    replace: bool,
    allow_http: bool,
) -> Result<(), Failure> {
    if directory.exists() {
        match load_verified_bundle(directory, request) {
            Ok(()) => return Ok(()),
            Err(error) if !replace => {
                return Err(Failure::bad_input(format!(
                    "prepared data at {} is invalid or does not match the configured identity ({error}); pass --replace to replace it",
                    directory.display()
                )));
            }
            Err(_) => {}
        }
    }
    let parent = directory
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| Failure::bad_input(error.to_string()))?;
    let staging = Builder::new()
        .prefix(".country-fetch-")
        .tempdir_in(parent)
        .map_err(|error| Failure::bad_input(error.to_string()))?;
    let remote = remote.trim_end_matches('/');
    // shortcut: manifests above 16 MiB are refused; raise the limit if future bundles exceed it.
    download_file(
        &format!("{remote}/manifest.json"),
        &staging.path().join("manifest.json"),
        16 * 1024 * 1024,
        allow_http,
    )?;
    let bytes = fs::read(staging.path().join("manifest.json"))
        .map_err(|error| Failure::bad_input(error.to_string()))?;
    let manifest: PreparedManifest = serde_json::from_slice(&bytes).map_err(|error| {
        Failure::bad_input(format!("remote prepared manifest is invalid: {error}"))
    })?;
    manifest
        .validate(request)
        .map_err(|error| Failure::bad_input(error.to_string()))?;
    for feature in &manifest.features {
        let Some(payload) = &feature.table else {
            continue;
        };
        if !safe_payload_name(&payload.file) {
            return Err(Failure::bad_input(format!(
                "prepared payload path is not a safe relative file name: {}",
                payload.file
            )));
        }
        let url = format!("{remote}/{}", payload.file);
        let path = staging.path().join(&payload.file);
        let (length, digest) = download_file(&url, &path, payload.payload_length, allow_http)?;
        if length != payload.payload_length || digest != payload.payload_sha256 {
            return Err(Failure::bad_input(format!(
                "prepared payload {} failed its length or SHA-256 check",
                payload.file
            )));
        }
    }
    load_verified_bundle(staging.path(), request).map_err(Failure::bad_input)?;
    PreparedManifest::install(directory, manifest, staging.path(), replace)
        .map_err(|error| Failure::bad_input(error.to_string()))?;
    Ok(())
}

fn load_verified_bundle(directory: &Path, request: &Request<'_>) -> Result<(), String> {
    let mut loaded =
        PreparedManifest::load(directory, request).map_err(|error| error.to_string())?;
    let ids: Vec<_> = loaded.feature_ids().map(str::to_owned).collect();
    for id in ids {
        loaded.load_table(&id).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn validate_remote(remote: &str) -> Result<(), Failure> {
    if !remote
        .strip_prefix("https://")
        .and_then(|url| url.split('/').next())
        .is_some_and(|authority| !authority.is_empty() && !authority.contains('@'))
    {
        return Err(Failure::bad_input(format!(
            "prepared bundle location must be a direct HTTPS URL without credentials: {remote}"
        )));
    }
    if remote.contains(['?', '#']) {
        return Err(Failure::bad_input(
            "prepared bundle URL must not include a query or fragment",
        ));
    }
    Ok(())
}

fn safe_payload_name(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.contains('/')
        && !value.contains('\\')
}

fn download_file(
    url: &str,
    destination: &Path,
    maximum_length: u64,
    allow_http: bool,
) -> Result<(u64, String), Failure> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| Failure::bad_input(error.to_string()))?;
    let protocols = if allow_http { "=http,https" } else { "=https" };
    // cspell:ignore redir redirs
    let output = Command::new("curl")
        .args([
            "--disable",
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--disallow-username-in-url",
            "--max-redirs",
            "5",
            "--max-time",
            "300",
            "--max-filesize",
            &maximum_length.to_string(),
            "--proto",
            protocols,
            "--proto-redir",
            protocols,
            "--output",
        ])
        .arg(destination)
        .arg(url)
        .output()
        .map_err(|error| {
            Failure::bad_input(format!(
                "could not start curl while fetching {url}: {error}; install the project tools with mise"
            ))
        })?;
    if !output.status.success() {
        let _ = fs::remove_file(destination);
        return Err(Failure::bad_input(format!(
            "could not fetch {url}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let mut file = OpenOptions::new()
        .read(true)
        .open(destination)
        .map_err(|error| Failure::bad_input(error.to_string()))?;
    let length = file
        .metadata()
        .map_err(|error| Failure::bad_input(error.to_string()))?
        .len();
    if length > maximum_length {
        let _ = fs::remove_file(destination);
        return Err(Failure::bad_input(format!(
            "download from {url} exceeds its declared size limit of {maximum_length} bytes"
        )));
    }
    let mut digest = Sha256::new();
    let mut buffer = vec![0; 64 * 1024].into_boxed_slice();
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| Failure::bad_input(format!("could not read {url}: {error}")))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok((length, format!("{:x}", digest.finalize())))
}

fn prepared_document(
    combination: &Combination,
    resolution: u32,
    directory: &Path,
) -> Result<String, Failure> {
    serialised(serde_json::to_string(&Envelope::new(
        PreparedCombinationDocument {
            combination_id: combination.id.clone(),
            resolution,
            directory: directory.to_path_buf(),
        },
    )))
}

struct SelectedPrepared {
    loaded: Loaded,
    features: Vec<crate::boundaries::BoundaryFeature>,
    revision: String,
    population_name: String,
    boundary_name: String,
    boundary_digest: String,
}

struct QueryPopulation {
    grid: popcircles::grid::Grid,
    epsg: u16,
    nodata: f32,
    sha256: Option<String>,
    attribution: Option<String>,
}

struct QueryBoundary {
    path: PathBuf,
    id_field: String,
    registered_sha256: Option<String>,
    revision: String,
    attribution: Option<String>,
}

impl SelectedPrepared {
    fn feature(&self, selector: &str) -> Result<&crate::boundaries::BoundaryFeature, Failure> {
        crate::boundaries::resolve(&self.features, selector).map_err(Failure::bad_input)
    }

    fn country_table(&mut self, id: &str) -> Result<Option<CountryTable>, Failure> {
        self.loaded
            .load_table(id)
            .map_err(|error| Failure::bad_input(error.to_string()))
    }

    fn boundary_identity(&self) -> (&str, Option<&str>, &str) {
        (
            &self.boundary_name,
            (!self.revision.is_empty()).then_some(self.revision.as_str()),
            &self.boundary_digest,
        )
    }

    fn population_source(&self) -> (&str, u32) {
        (&self.population_name, self.loaded.manifest.resolution)
    }

    fn selection(&self) -> CountrySelectionReport {
        let manifest = &self.loaded.manifest;
        CountrySelectionReport::new(
            manifest.combination_id.clone(),
            manifest.resolution,
            CountrySourceReport::new(
                &self.population_name,
                manifest.population_source.sha256.as_deref(),
                manifest.population_source.attribution.as_deref(),
            ),
            CountrySourceReport::new(
                &self.boundary_name,
                manifest.boundary_source.sha256.as_deref(),
                manifest.boundary_source.attribution.as_deref(),
            ),
        )
    }
}

fn query_population_source(
    combination: &Combination,
    registry: &Registry,
) -> Result<QueryPopulation, Failure> {
    if let Some(name) = combination.population_source.strip_prefix("dataset:") {
        let dataset = registry
            .raster(name)
            .map_err(|error| Failure::registry(&error))?;
        Ok(QueryPopulation {
            grid: dataset.grid.grid().map_err(|error| Failure::grid(&error))?,
            epsg: dataset.epsg,
            nodata: dataset.nodata,
            sha256: dataset.sha256,
            attribution: dataset.attribution,
        })
    } else {
        let spec = combination
            .population
            .ok_or_else(|| Failure::bad_input("local population source metadata is missing"))?;
        Ok(QueryPopulation {
            grid: spec.grid,
            epsg: spec.epsg,
            nodata: spec.nodata,
            sha256: None,
            attribution: combination.attribution.clone(),
        })
    }
}

fn query_boundary_source(
    combination: &Combination,
    registry: &Registry,
) -> Result<QueryBoundary, Failure> {
    if let Some(name) = combination.boundary_source.strip_prefix("dataset:") {
        let boundary = registry.boundary(name).ok_or_else(|| {
            Failure::bad_input(format!("`{name}` is not a registered boundary dataset"))
        })?;
        let id_field = boundary.source_id_field.as_deref().ok_or_else(|| {
            Failure::bad_input(format!("boundary dataset `{name}` has no source ID field"))
        })?;
        Ok(QueryBoundary {
            path: boundary.path.clone(),
            id_field: id_field.to_owned(),
            registered_sha256: boundary.sha256.clone(),
            revision: boundary.revision.clone().unwrap_or_default(),
            attribution: boundary
                .attribution
                .clone()
                .or_else(|| combination.attribution.clone()),
        })
    } else {
        let path = combination
            .boundary_source
            .strip_prefix("file:")
            .filter(|path| !path.is_empty())
            .ok_or_else(|| Failure::bad_input("invalid boundary_source"))?;
        let id_field = combination
            .boundary_source_id_field
            .clone()
            .ok_or_else(|| Failure::bad_input("local boundary source ID field is missing"))?;
        Ok(QueryBoundary {
            path: Path::new(path).to_path_buf(),
            id_field,
            registered_sha256: None,
            revision: String::new(),
            attribution: combination.attribution.clone(),
        })
    }
}

fn source_name(name: &str) -> String {
    if let Some(name) = name.strip_prefix("dataset:") {
        name.to_owned()
    } else if let Some(path) = name.strip_prefix("file:") {
        Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .map_or_else(|| "local-file".to_owned(), |name| format!("file:{name}"))
    } else {
        name.to_owned()
    }
}

fn load_selected_prepared(
    config_path: Option<&Path>,
    args: &CountryTableArgs,
) -> Result<SelectedPrepared, Failure> {
    let config =
        Configuration::load(config_path).map_err(|error| Failure::bad_input(error.to_string()))?;
    let (combination, resolution) = config
        .combination(args.combination.as_deref(), args.resolution)
        .map_err(|error| Failure::bad_input(error.to_string()))?;
    let registry =
        Registry::load(Path::new(REGISTRY_PATH)).map_err(|error| Failure::registry(&error))?;
    let population = query_population_source(combination, &registry)?;
    let boundary = query_boundary_source(combination, &registry)?;
    let actual_boundary_sha256 =
        crate::prepared::fingerprint_file(&boundary.path).map_err(|error| Failure {
            code: EXIT_MISSING_DATA,
            message: format!(
                "{} could not be read: {error}",
                boundary_source_guidance(&boundary)
            ),
        })?;
    if boundary
        .registered_sha256
        .as_deref()
        .is_some_and(|expected| expected != actual_boundary_sha256)
    {
        return Err(Failure::bad_input(format!(
            "selected boundary source at {} does not match its registered SHA-256",
            boundary.path.display()
        )));
    }
    let request = Request {
        combination_id: &combination.id,
        resolution,
        population_source: &combination.population_source,
        population_sha256: population.sha256.as_deref(),
        boundary_source: &combination.boundary_source,
        boundary_sha256: Some(&actual_boundary_sha256),
        boundary_source_id_field: &boundary.id_field,
        population_epsg: population.epsg,
        population_nodata: population.nodata,
        source_grid: population.grid,
    };
    let location = &combination.resolutions[&resolution];
    let directory = location.local.as_deref().ok_or_else(|| Failure {
        code: EXIT_MISSING_DATA,
        message: format!(
            "combination `{}` resolution {resolution} has no local prepared data; run `country fetch --combination {} --resolution {resolution}`",
            combination.id, combination.id
        ),
    })?;
    let loaded = PreparedManifest::load(directory, &request).map_err(|error| {
        let guidance = if location.remote.is_some() {
            format!(
                "; run `country prepare --combination {} --resolution {resolution}` or `country fetch --combination {} --resolution {resolution}`",
                combination.id, combination.id
            )
        } else {
            format!(
                "; run `country prepare --combination {} --resolution {resolution}`",
                combination.id
            )
        };
        let missing = match &error {
            crate::prepared::PreparedError::ManifestRead { source, .. }
            | crate::prepared::PreparedError::PayloadRead { source, .. } => {
                source.kind() == std::io::ErrorKind::NotFound
            }
            _ => false,
        };
        Failure {
            code: if missing {
                EXIT_MISSING_DATA
            } else {
                EXIT_BAD_INPUT
            },
            message: format!("prepared data at {} is missing, invalid or mismatched: {error}{guidance}", directory.display()),
        }
    })?;
    let boundary_digest = loaded
        .manifest
        .boundary_source
        .sha256
        .clone()
        .ok_or_else(|| Failure::bad_input("prepared boundary source has no fingerprint"))?;
    let features = load_query_boundary(&boundary)?;
    let population_name = source_name(&combination.population_source);
    let boundary_name = source_name(&combination.boundary_source);
    Ok(SelectedPrepared {
        loaded,
        features,
        revision: boundary.revision,
        population_name,
        boundary_name,
        boundary_digest,
    })
}

fn load_query_boundary(
    boundary: &QueryBoundary,
) -> Result<Vec<crate::boundaries::BoundaryFeature>, Failure> {
    crate::boundaries::load(&boundary.path, &boundary.id_field).map_err(|error| {
        if boundary.path.exists() {
            Failure::bad_input(error.to_string())
        } else {
            Failure {
                code: EXIT_MISSING_DATA,
                message: boundary_source_guidance(boundary),
            }
        }
    })
}

fn boundary_source_guidance(boundary: &QueryBoundary) -> String {
    if boundary.registered_sha256.is_some() {
        format!(
            "selected boundary source at {} could not be read; fetch the registered boundary data before querying",
            boundary.path.display()
        )
    } else {
        format!(
            "selected local boundary source at {} could not be read; provide or fix the configured file before querying",
            boundary.path.display()
        )
    }
}

pub(crate) fn smallest(
    config_path: Option<&Path>,
    selector: &str,
    share: Share,
    args: &CountryTableArgs,
    search: SearchArgs,
    ledger_path: Option<&Path>,
) -> Result<String, Failure> {
    let mut prepared = load_selected_prepared(config_path, args)?;
    let feature = prepared.feature(selector)?.clone();
    let table = prepared
        .country_table(feature.id.as_str())?
        .ok_or_else(|| {
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
    let selection = prepared.selection();
    let boundary_identity = prepared.boundary_identity();
    let population_source = prepared.population_source();
    serialised(serde_json::to_string(&Envelope::new(
        CountrySmallestDocument::new(
            selection,
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
    config_path: Option<&Path>,
    selectors: &[String],
    range: SweepArgs,
    args: &CountryTableArgs,
    search: SearchArgs,
    ledger_dir: &Path,
) -> Result<String, Failure> {
    let mut prepared = load_selected_prepared(config_path, args)?;
    let walk = shares(range.from, range.to, range.step)?;
    let mut features = selectors
        .iter()
        .map(|selector| prepared.feature(selector).cloned())
        .collect::<Result<Vec<_>, _>>()?;
    features.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    features.dedup_by(|left, right| left.id == right.id);

    let selection = prepared.selection();
    let (boundary_dataset, boundary_revision, boundary_digest) = prepared.boundary_identity();
    let boundary_dataset = boundary_dataset.to_owned();
    let boundary_revision = boundary_revision.map(str::to_owned);
    let boundary_digest = boundary_digest.to_owned();
    let mut countries = Vec::with_capacity(features.len());
    for feature in features {
        let started = std::time::Instant::now();
        let Some(table) = prepared.country_table(feature.id.as_str())? else {
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
            &boundary_dataset,
            boundary_revision.as_deref(),
            &boundary_digest,
            LedgerReport::new(&path, ledger.len()),
            records,
        ));
    }

    serialised(serde_json::to_string(&Envelope::new(
        CountrySweepDocument::new(
            selection,
            SweepShares::new(range.from, range.to, range.step),
            countries,
        ),
    )))
}
#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "local-server fixture failures should identify the operation that failed"
)]
mod tests {
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::{self, JoinHandle};

    use popcircles::grid::Grid;
    use tempfile::TempDir;

    use super::*;
    use crate::prepared::tests::{fixture, request};

    fn local_server(routes: HashMap<String, (Vec<u8>, usize)>) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("server binds locally");
        let address = listener.local_addr().expect("server has an address");
        let requests = routes.len();
        let server = thread::spawn(move || {
            for _ in 0..requests {
                let (mut stream, _) = listener.accept().expect("server accepts request");
                let mut request = [0; 4096];
                let count = stream.read(&mut request).expect("request is readable");
                let path = String::from_utf8_lossy(&request[..count])
                    .split_whitespace()
                    .nth(1)
                    .expect("request contains a path")
                    .to_owned();
                let (body, declared_length) = routes.get(&path).expect("route exists");
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {declared_length}\r\nConnection: close\r\n\r\n"
                )
                .expect("response headers are written");
                stream.write_all(body).expect("response body is written");
            }
        });
        (format!("http://{address}/prepared"), server)
    }

    fn bundle_routes(
        directory: &Path,
        changed_id: Option<&str>,
        corrupt_payload: bool,
    ) -> HashMap<String, (Vec<u8>, usize)> {
        let manifest_path = directory.join("manifest.json");
        let mut manifest: PreparedManifest =
            serde_json::from_slice(&fs::read(&manifest_path).expect("source manifest is readable"))
                .expect("source manifest parses");
        if let Some(id) = changed_id {
            manifest.combination_id = id.to_owned();
        }
        let manifest = serde_json::to_vec(&manifest).expect("remote manifest serializes");
        let parsed: PreparedManifest =
            serde_json::from_slice(&manifest).expect("remote manifest parses");
        let payload = parsed.features[0]
            .table
            .as_ref()
            .expect("fixture country has a table");
        let mut payload_bytes =
            fs::read(directory.join(&payload.file)).expect("payload is readable");
        if corrupt_payload {
            payload_bytes[0] ^= 0x01;
        }
        HashMap::from([
            (
                "/prepared/manifest.json".to_owned(),
                (manifest.clone(), manifest.len()),
            ),
            (
                format!("/prepared/{}", payload.file),
                (payload_bytes.clone(), payload_bytes.len()),
            ),
        ])
    }

    fn snapshot(directory: &Path) -> Vec<(String, Vec<u8>)> {
        let mut files: Vec<_> = fs::read_dir(directory)
            .expect("bundle directory is readable")
            .map(|entry| {
                let entry = entry.expect("bundle entry is readable");
                (
                    entry.file_name().to_string_lossy().into_owned(),
                    fs::read(entry.path()).expect("bundle file is readable"),
                )
            })
            .collect();
        files.sort_by(|left, right| left.0.cmp(&right.0));
        files
    }

    fn install_fixture(target: &Path) -> (TempDir, Grid) {
        let (source, grid) = fixture();
        let manifest: PreparedManifest = serde_json::from_slice(
            &fs::read(source.path().join("manifest.json")).expect("fixture manifest is readable"),
        )
        .expect("fixture manifest parses");
        PreparedManifest::install(target, manifest, source.path(), false)
            .expect("fixture installs transactionally");
        (source, grid)
    }

    #[test]
    fn country_fetch_installs_a_verified_download_and_reuses_it_without_network() {
        let (source, grid) = fixture();
        let parent = tempfile::tempdir().expect("target parent exists");
        let target = parent.path().join("bundle");
        let (remote, server) = local_server(bundle_routes(source.path(), None, false));
        fetch_bundle(&remote, &target, &request(grid), false, true)
            .expect("verified bundle installs");
        server.join().expect("local server finishes");
        assert!(load_verified_bundle(&target, &request(grid)).is_ok());

        fetch_bundle(
            "http://127.0.0.1:1/prepared",
            &target,
            &request(grid),
            false,
            true,
        )
        .expect("valid installed bundle is reused without a request");
    }

    #[test]
    fn country_fetch_rejects_non_https_and_credential_bearing_locations() {
        assert!(validate_remote("http://example.com/prepared").is_err());
        assert!(validate_remote("https://user@example.com/prepared").is_err());
        assert!(validate_remote("https://example.com/prepared?token=value").is_err());
        assert!(validate_remote("https://example.com/prepared").is_ok());
    }

    #[test]
    fn country_fetch_caps_manifest_size() {
        let body = b"manifest".to_vec();
        let (remote, server) = local_server(HashMap::from([(
            "/prepared/manifest.json".to_owned(),
            (body.clone(), body.len()),
        )]));
        let directory = tempfile::tempdir().expect("temporary download directory exists");
        let error = download_file(
            &format!("{remote}/manifest.json"),
            &directory.path().join("manifest.json"),
            u64::try_from(body.len() - 1).expect("fixture length fits u64"),
            true,
        )
        .expect_err("manifest exceeds its configured size limit");
        server.join().expect("local server finishes");
        assert!(!directory.path().join("manifest.json").exists());
        assert_ne!(error.message, "");
    }

    #[test]
    fn country_fetch_truncated_download_preserves_the_existing_bundle() {
        let (_, grid) = fixture();
        let parent = tempfile::tempdir().expect("target parent exists");
        let target = parent.path().join("bundle");
        let (source, _) = install_fixture(&target);
        let before = snapshot(&target);
        let mut mismatch = request(grid);
        mismatch.combination_id = "replacement";
        let body = fs::read(source.path().join("manifest.json")).expect("manifest is readable");
        let (remote, server) = local_server(HashMap::from([(
            "/prepared/manifest.json".to_owned(),
            (body.clone(), body.len() + 12),
        )]));
        assert!(fetch_bundle(&remote, &target, &mismatch, true, true).is_err());
        server.join().expect("local server finishes");
        assert_eq!(snapshot(&target), before);
        assert!(load_verified_bundle(&target, &request(grid)).is_ok());
    }

    #[test]
    fn country_fetch_requires_replacement_for_a_mismatched_target_and_refuses_wrong_identity() {
        let (_, grid) = fixture();
        let parent = tempfile::tempdir().expect("target parent exists");
        let target = parent.path().join("bundle");
        let (source, _) = install_fixture(&target);
        let before = snapshot(&target);
        let mut replacement = request(grid);
        replacement.combination_id = "replacement";
        let no_replace = fetch_bundle(
            "http://127.0.0.1:1/prepared",
            &target,
            &replacement,
            false,
            true,
        )
        .expect_err("mismatched target needs explicit replacement");
        assert!(no_replace.message.contains("pass --replace"));

        let mut routes = bundle_routes(source.path(), None, false);
        routes.retain(|path, _| path.ends_with("manifest.json"));
        let (remote, server) = local_server(routes);
        assert!(fetch_bundle(&remote, &target, &replacement, true, true).is_err());
        server.join().expect("local server finishes");
        assert_eq!(snapshot(&target), before);
        assert!(load_verified_bundle(&target, &request(grid)).is_ok());
    }

    #[test]
    fn country_fetch_checksum_failure_preserves_the_existing_bundle() {
        let (_, grid) = fixture();
        let parent = tempfile::tempdir().expect("target parent exists");
        let target = parent.path().join("bundle");
        let (source, _) = install_fixture(&target);
        let before = snapshot(&target);
        let mut replacement = request(grid);
        replacement.combination_id = "replacement";
        let (remote, server) =
            local_server(bundle_routes(source.path(), Some("replacement"), true));
        assert!(fetch_bundle(&remote, &target, &replacement, true, true).is_err());
        server.join().expect("local server finishes");
        assert_eq!(snapshot(&target), before);
        assert!(load_verified_bundle(&target, &request(grid)).is_ok());
    }

    #[test]
    fn country_fetch_interrupted_payload_preserves_the_existing_bundle() {
        let (_, grid) = fixture();
        let parent = tempfile::tempdir().expect("target parent exists");
        let target = parent.path().join("bundle");
        let (source, _) = install_fixture(&target);
        let before = snapshot(&target);
        let mut replacement = request(grid);
        replacement.combination_id = "replacement";
        let mut routes = bundle_routes(source.path(), Some("replacement"), false);
        let payload = routes
            .iter_mut()
            .find(|(path, _)| !path.ends_with("manifest.json"))
            .map(|(_, response)| response)
            .expect("payload route exists");
        payload.1 += 12;
        let (remote, server) = local_server(routes);
        assert!(fetch_bundle(&remote, &target, &replacement, true, true).is_err());
        server.join().expect("local server finishes");
        assert_eq!(snapshot(&target), before);
        assert!(load_verified_bundle(&target, &request(grid)).is_ok());
    }
}
