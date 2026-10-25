use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use popcircles::report::{
    CountryListDocument, CountrySmallestDocument, CountrySweepCountry, CountrySweepDocument,
    CountrySweepRecord, Document, Envelope, LedgerReport, SmallestReport, SweepShares,
};
use popcircles::smallest::cache::CountryLedger;
use popcircles::smallest::{self, Share};
use popcircles::table::Decimation;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tempfile::Builder;

use crate::args::{CountryTableArgs, SearchArgs, SweepArgs};
use crate::boundaries;
use crate::commands::search::{settlement_context, settlement_places, shares};
use crate::commands::{make_room_for, serialised};
use crate::config::{Combination, Configuration};
use crate::country_breakdown::Prepared;
use crate::failure::{EXIT_MISSING_DATA, Failure};
use crate::observe::StderrProgress;
use crate::prepared::{PreparedManifest, Request};
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
    if !remote.starts_with("https://") {
        return Err(Failure::bad_input(format!(
            "refusing to fetch prepared data from a non-HTTPS URL: {remote}"
        )));
    }
    if remote.contains(['?', '#']) {
        return Err(Failure::bad_input(
            "prepared bundle URL must not include a query or fragment",
        ));
    }
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
    let agent = ureq::Agent::config_builder()
        .https_only(true)
        .timeout_global(Some(std::time::Duration::from_secs(300)))
        .build()
        .new_agent();
    fetch_bundle(remote, directory, &request, replace, &agent)?;
    prepared_document(combination, resolution, directory)
}

fn fetch_bundle(
    remote: &str,
    directory: &Path,
    request: &Request<'_>,
    replace: bool,
    agent: &ureq::Agent,
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
    download_file(
        agent,
        &format!("{remote}/manifest.json"),
        &staging.path().join("manifest.json"),
        16 * 1024 * 1024,
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
        let (length, digest) = download_file(agent, &url, &path, payload.payload_length)?;
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

fn safe_payload_name(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.contains('/')
        && !value.contains('\\')
}

fn download_file(
    agent: &ureq::Agent,
    url: &str,
    destination: &Path,
    maximum_length: u64,
) -> Result<(u64, String), Failure> {
    let response = agent
        .get(url)
        .call()
        .map_err(|error| Failure::bad_input(format!("could not fetch {url}: {error}")))?;
    let mut reader = response.into_body().into_reader();
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| Failure::bad_input(error.to_string()))?;
    let mut digest = Sha256::new();
    let mut length = 0_u64;
    let mut buffer = vec![0; 64 * 1024].into_boxed_slice();
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| Failure::bad_input(format!("could not read {url}: {error}")))?;
        if count == 0 {
            break;
        }
        length = length.saturating_add(count as u64);
        if length > maximum_length {
            return Err(Failure::bad_input(format!(
                "download from {url} exceeds its declared size limit of {maximum_length} bytes"
            )));
        }
        file.write_all(&buffer[..count])
            .map_err(|error| Failure::bad_input(error.to_string()))?;
        digest.update(&buffer[..count]);
    }
    file.sync_all()
        .map_err(|error| Failure::bad_input(error.to_string()))?;
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

    fn local_agent() -> ureq::Agent {
        ureq::Agent::config_builder()
            .https_only(false)
            .timeout_global(Some(std::time::Duration::from_secs(5)))
            .build()
            .new_agent()
    }

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
        fetch_bundle(&remote, &target, &request(grid), false, &local_agent())
            .expect("verified bundle installs");
        server.join().expect("local server finishes");
        assert!(load_verified_bundle(&target, &request(grid)).is_ok());

        fetch_bundle(
            "http://127.0.0.1:1/prepared",
            &target,
            &request(grid),
            false,
            &local_agent(),
        )
        .expect("valid installed bundle is reused without a request");
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
        assert!(fetch_bundle(&remote, &target, &mismatch, true, &local_agent()).is_err());
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
            &local_agent(),
        )
        .expect_err("mismatched target needs explicit replacement");
        assert!(no_replace.message.contains("pass --replace"));

        let mut routes = bundle_routes(source.path(), None, false);
        routes.retain(|path, _| path.ends_with("manifest.json"));
        let (remote, server) = local_server(routes);
        assert!(fetch_bundle(&remote, &target, &replacement, true, &local_agent()).is_err());
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
        assert!(fetch_bundle(&remote, &target, &replacement, true, &local_agent()).is_err());
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
        assert!(fetch_bundle(&remote, &target, &replacement, true, &local_agent()).is_err());
        server.join().expect("local server finishes");
        assert_eq!(snapshot(&target), before);
        assert!(load_verified_bundle(&target, &request(grid)).is_ok());
    }
}
