use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use popcircles::domain::FeatureId;
use popcircles::geodesy::LatLon;
use popcircles::grid::Grid;
use popcircles::table::{
    COUNTRY_AGGREGATION_ORDER_VERSION, CountryTable, PreparedCountryTable,
    PreparedCountryTableError, PreparedCountryTableIdentity,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const FORMAT_VERSION: u32 = 1;
const MANIFEST_NAME: &str = "manifest.json";
static GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, thiserror::Error)]
pub(crate) enum PreparedError {
    #[error("prepared manifest at {} could not be read", path.display())]
    ManifestRead {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("prepared manifest at {} is not valid JSON", path.display())]
    ManifestSyntax {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("prepared manifest for {} could not be serialized", path.display())]
    ManifestSerialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("prepared data format version {found} is unsupported; this reader supports {expected}")]
    FormatVersion { expected: u32, found: u32 },
    #[error("prepared data does not match {ground}")]
    Mismatch { ground: &'static str },
    #[error("prepared payload path is not a safe relative file name: {path}")]
    UnsafePayloadPath { path: String },
    #[error("prepared payload at {} could not be read", path.display())]
    PayloadRead {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("prepared payload {path} has {found} bytes; manifest declares {expected}")]
    PayloadLength {
        path: String,
        expected: u64,
        found: u64,
    },
    #[error("prepared payload {path} SHA-256 differs from its manifest")]
    PayloadDigest { path: String },
    #[error("prepared payload {path} has a length that is not a multiple of eight")]
    PayloadAlignment { path: String },
    #[error("prepared feature identity {id} is invalid")]
    FeatureId {
        id: String,
        #[source]
        source: popcircles::domain::DomainError,
    },
    #[error("prepared feature {id} is invalid: {source}")]
    CountryTable {
        id: String,
        #[source]
        source: PreparedCountryTableError,
    },
    #[error("prepared artifact already exists at {}", path.display())]
    Existing { path: PathBuf },
    #[error(
        "prepared publisher lock exists at {}; if no preparation is active, remove it to clear an interrupted run",
        path.display()
    )]
    PublisherHeld { path: PathBuf },
    #[error("publisher lock at {} could not be created", path.display())]
    LockWrite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("prepared directory at {} could not be created", path.display())]
    DirectoryCreate {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("prepared payload at {} could not be written", path.display())]
    PayloadWrite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("prepared manifest temporary at {} could not be written", path.display())]
    ManifestWrite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("prepared manifest at {} could not be atomically published", path.display())]
    ManifestPublish {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("prepared feature `{id}` is not in the manifest")]
    UnknownFeature { id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreparedManifest {
    pub(crate) format_version: u32,
    pub(crate) combination_id: String,
    pub(crate) resolution: u32,
    pub(crate) population_source: Source,
    pub(crate) boundary_source: Source,
    pub(crate) source_grid: GridDocument,
    pub(crate) effective_grid: GridDocument,
    pub(crate) decimation: u32,
    pub(crate) ownership_rule_version: u32,
    pub(crate) aggregation_order_version: u32,
    pub(crate) byte_order: ByteOrder,
    pub(crate) features: Vec<Feature>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Source {
    pub(crate) name: String,
    pub(crate) sha256: Option<String>,
    pub(crate) attribution: Option<String>,
    pub(crate) epsg: Option<u16>,
    pub(crate) nodata: Option<f32>,
    #[serde(rename = "source_id_field")]
    pub(crate) id_field: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct GridDocument {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) origin_lat: f64,
    pub(crate) origin_lon: f64,
    pub(crate) lon_step: f64,
    pub(crate) lat_step: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Feature {
    pub(crate) id: String,
    pub(crate) table: Option<CountryPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct CountryPayload {
    pub(crate) file: String,
    pub(crate) grid: GridDocument,
    pub(crate) north_row: u32,
    pub(crate) west_col: u32,
    pub(crate) total: f64,
    pub(crate) unassigned: f64,
    pub(crate) source_digest: u64,
    pub(crate) ownership_digest: u64,
    pub(crate) payload_length: u64,
    pub(crate) payload_sha256: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ByteOrder {
    Little,
}

#[derive(Clone, Copy)]
pub(crate) struct Request<'a> {
    pub(crate) combination_id: &'a str,
    pub(crate) resolution: u32,
    pub(crate) population_source: &'a str,
    pub(crate) population_sha256: Option<&'a str>,
    pub(crate) boundary_source: &'a str,
    pub(crate) boundary_sha256: Option<&'a str>,
    pub(crate) population_epsg: u16,
    pub(crate) population_nodata: f32,
    pub(crate) boundary_source_id_field: &'a str,
    pub(crate) source_grid: Grid,
}

pub(crate) struct Loaded {
    pub(crate) manifest: PreparedManifest,
    payloads: Vec<Option<File>>,
}

impl Loaded {
    pub(crate) fn feature_ids(&self) -> impl Iterator<Item = &str> {
        self.manifest
            .features
            .iter()
            .map(|feature| feature.id.as_str())
    }

    pub(crate) fn load_table(&mut self, id: &str) -> Result<Option<CountryTable>, PreparedError> {
        let (index, feature) = self
            .manifest
            .features
            .iter()
            .enumerate()
            .find(|(_, feature)| feature.id == id)
            .ok_or_else(|| PreparedError::UnknownFeature { id: id.to_owned() })?;
        let Some(payload) = &feature.table else {
            return Ok(None);
        };
        let feature_id =
            FeatureId::new(feature.id.clone()).map_err(|source| PreparedError::FeatureId {
                id: feature.id.clone(),
                source,
            })?;
        let file = self.payloads[index]
            .as_mut()
            .ok_or(PreparedError::Mismatch {
                ground: "open country payload",
            })?;
        load_country(file, &self.manifest, &feature_id, payload).map(Some)
    }
}

impl GridDocument {
    pub(crate) fn from_grid(grid: Grid) -> Self {
        Self {
            width: grid.width(),
            height: grid.height(),
            origin_lat: grid.origin().lat,
            origin_lon: grid.origin().lon,
            lon_step: grid.lon_step(),
            lat_step: grid.lat_step(),
        }
    }

    fn grid(self) -> Result<Grid, ()> {
        Grid::new(
            self.width,
            self.height,
            LatLon {
                lat: self.origin_lat,
                lon: self.origin_lon,
            },
            self.lon_step,
            self.lat_step,
        )
        .map_err(|_| ())
    }
}

pub(crate) struct PreparedWriter {
    directory: PathBuf,
    manifest_path: PathBuf,
    lock_path: PathBuf,
    _lock: File,
    generation: String,
    manifest: PreparedManifest,
    previous: Option<PreparedManifest>,
    features: Vec<Feature>,
    created: Vec<PathBuf>,
    published: bool,
}

impl PreparedWriter {
    pub(crate) fn new(
        directory: &Path,
        manifest: PreparedManifest,
        replace: bool,
    ) -> Result<Self, PreparedError> {
        fs::create_dir_all(directory).map_err(|source| PreparedError::DirectoryCreate {
            path: directory.to_path_buf(),
            source,
        })?;
        let manifest_path = directory.join(MANIFEST_NAME);
        // shortcut: a crashed publisher leaves this lock behind; remove it to resume preparation.
        let lock_path = directory.join(".prepare.lock");
        let lock = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(lock) => lock,
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                return Err(PreparedError::PublisherHeld { path: lock_path });
            }
            Err(source) => {
                return Err(PreparedError::LockWrite {
                    path: lock_path,
                    source,
                });
            }
        };
        let mut writer = Self {
            directory: directory.to_path_buf(),
            manifest_path: manifest_path.clone(),
            lock_path,
            _lock: lock,
            generation: generation_name(),
            manifest,
            previous: None,
            features: Vec::new(),
            created: Vec::new(),
            published: false,
        };
        if manifest_path.exists() && !replace {
            return Err(PreparedError::Existing {
                path: manifest_path,
            });
        }
        writer.previous = fs::read(&manifest_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        Ok(writer)
    }

    pub(crate) fn write_feature(
        &mut self,
        feature_id: &FeatureId,
        table: Option<&CountryTable>,
    ) -> Result<(), PreparedError> {
        require(
            !self
                .features
                .iter()
                .any(|feature| feature.id == feature_id.as_str()),
            "unique feature identity",
        )?;
        let payload = table
            .map(|table| {
                write_country(
                    &self.directory,
                    &self.generation,
                    self.features.len(),
                    feature_id,
                    table,
                    &self.manifest,
                    &mut self.created,
                )
            })
            .transpose()?;
        self.features.push(Feature {
            id: feature_id.as_str().to_owned(),
            table: payload,
        });
        Ok(())
    }

    pub(crate) fn publish(self) -> Result<(), PreparedError> {
        self.publish_with(|from, to| fs::rename(from, to))
    }

    fn publish_with(
        mut self,
        publish_manifest: impl FnOnce(&Path, &Path) -> io::Result<()>,
    ) -> Result<(), PreparedError> {
        self.manifest.format_version = FORMAT_VERSION;
        self.manifest.byte_order = ByteOrder::Little;
        self.manifest.features = std::mem::take(&mut self.features);
        let bytes = serde_json::to_vec_pretty(&self.manifest).map_err(|source| {
            PreparedError::ManifestSerialize {
                path: self.manifest_path.clone(),
                source,
            }
        })?;
        let temporary = self
            .directory
            .join(format!("manifest-{}.json.tmp", self.generation));
        self.created.push(temporary.clone());
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|source| PreparedError::ManifestWrite {
                path: temporary.clone(),
                source,
            })?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|source| PreparedError::ManifestWrite {
                path: temporary.clone(),
                source,
            })?;
        publish_manifest(&temporary, &self.manifest_path).map_err(|source| {
            PreparedError::ManifestPublish {
                path: self.manifest_path.clone(),
                source,
            }
        })?;
        self.published = true;
        if let Some(previous) = self.previous.take() {
            for feature in previous.features {
                if let Some(payload) = feature.table
                    && safe_payload_path(&payload.file)
                {
                    let _ = fs::remove_file(self.directory.join(payload.file));
                }
            }
        }
        Ok(())
    }
}

impl Drop for PreparedWriter {
    fn drop(&mut self) {
        if !self.published {
            for path in &self.created {
                let _ = fs::remove_file(path);
            }
        }
        let _ = fs::remove_file(&self.lock_path);
    }
}

impl PreparedManifest {
    pub(crate) fn publish(
        directory: &Path,
        manifest: Self,
        features: &[(FeatureId, Option<&CountryTable>)],
        replace: bool,
    ) -> Result<(), PreparedError> {
        let mut writer = PreparedWriter::new(directory, manifest, replace)?;
        for (feature_id, table) in features {
            writer.write_feature(feature_id, *table)?;
        }
        writer.publish()
    }

    fn publish_with(
        directory: &Path,
        manifest: Self,
        features: &[(FeatureId, Option<&CountryTable>)],
        replace: bool,
        publish_manifest: impl FnOnce(&Path, &Path) -> io::Result<()>,
    ) -> Result<(), PreparedError> {
        let mut writer = PreparedWriter::new(directory, manifest, replace)?;
        for (feature_id, table) in features {
            writer.write_feature(feature_id, *table)?;
        }
        writer.publish_with(publish_manifest)
    }

    pub(crate) fn load(directory: &Path, request: &Request<'_>) -> Result<Loaded, PreparedError> {
        let manifest_path = directory.join(MANIFEST_NAME);
        let bytes = fs::read(&manifest_path).map_err(|source| PreparedError::ManifestRead {
            path: manifest_path.clone(),
            source,
        })?;
        let manifest: Self =
            serde_json::from_slice(&bytes).map_err(|source| PreparedError::ManifestSyntax {
                path: manifest_path,
                source,
            })?;
        validate_manifest(&manifest, request)?;
        let mut feature_ids = BTreeSet::new();
        let mut payload_files = BTreeSet::new();
        let mut payloads = Vec::with_capacity(manifest.features.len());
        for feature in &manifest.features {
            require(
                feature_ids.insert(feature.id.as_str()),
                "unique feature identity",
            )?;
            FeatureId::new(feature.id.clone()).map_err(|source| PreparedError::FeatureId {
                id: feature.id.clone(),
                source,
            })?;
            if let Some(payload) = &feature.table {
                require(
                    payload_files.insert(payload.file.as_str()),
                    "unique payload path",
                )?;
                require(safe_payload_path(&payload.file), "safe payload path")?;
                require(
                    valid_sha256(&payload.payload_sha256),
                    "payload SHA-256 spelling",
                )?;
                let cell_bytes = u64::try_from(size_of::<f64>()).unwrap_or(u64::MAX);
                require(
                    payload.payload_length.is_multiple_of(cell_bytes),
                    "payload length alignment",
                )?;
                require(payload.grid.grid().is_ok(), "country crop grid")?;
                payloads.push(Some(open_payload(directory, payload)?));
            } else {
                payloads.push(None);
            }
        }
        Ok(Loaded { manifest, payloads })
    }
}

fn validate_manifest(
    manifest: &PreparedManifest,
    request: &Request<'_>,
) -> Result<(), PreparedError> {
    if manifest.format_version != FORMAT_VERSION {
        return Err(PreparedError::FormatVersion {
            expected: FORMAT_VERSION,
            found: manifest.format_version,
        });
    }
    for (source, ground) in [
        (
            &manifest.population_source,
            "population source SHA-256 spelling",
        ),
        (
            &manifest.boundary_source,
            "boundary source SHA-256 spelling",
        ),
    ] {
        require(source.sha256.as_deref().is_none_or(valid_sha256), ground)?;
    }
    require(
        manifest.combination_id == request.combination_id,
        "combination ID",
    )?;
    require(manifest.resolution == request.resolution, "resolution")?;
    require(
        manifest.population_source.name == request.population_source,
        "population source name",
    )?;
    require(
        manifest.boundary_source.name == request.boundary_source,
        "boundary source name",
    )?;
    require(
        manifest.population_source.epsg == Some(request.population_epsg)
            && manifest
                .population_source
                .nodata
                .is_some_and(|nodata| nodata.to_bits() == request.population_nodata.to_bits())
            && manifest.population_source.id_field.is_none(),
        "population source interpretation",
    )?;
    require(
        manifest.boundary_source.id_field.as_deref() == Some(request.boundary_source_id_field)
            && manifest.boundary_source.epsg.is_none()
            && manifest.boundary_source.nodata.is_none(),
        "boundary source interpretation",
    )?;
    for (expected, found, ground) in [
        (
            request.population_sha256,
            manifest.population_source.sha256.as_deref(),
            "population source fingerprint",
        ),
        (
            request.boundary_sha256,
            manifest.boundary_source.sha256.as_deref(),
            "boundary source fingerprint",
        ),
    ] {
        require(
            expected.is_none_or(|expected| Some(expected) == found),
            ground,
        )?;
    }
    let source_grid = manifest
        .source_grid
        .grid()
        .map_err(|()| PreparedError::Mismatch {
            ground: "source grid",
        })?;
    require(source_grid == request.source_grid, "source grid")?;
    require(manifest.decimation == manifest.resolution, "decimation")?;
    let decimation =
        popcircles::table::Decimation::new(source_grid, manifest.decimation).map_err(|_| {
            PreparedError::Mismatch {
                ground: "effective grid decimation",
            }
        })?;
    let effective_grid = manifest
        .effective_grid
        .grid()
        .map_err(|()| PreparedError::Mismatch {
            ground: "effective grid",
        })?;
    require(effective_grid == *decimation.grid(), "effective grid")?;
    require(
        manifest.ownership_rule_version == popcircles::ownership::OWNERSHIP_RULE_VERSION,
        "ownership rule version",
    )?;
    require(
        manifest.aggregation_order_version == COUNTRY_AGGREGATION_ORDER_VERSION,
        "aggregation order version",
    )?;
    require(manifest.byte_order == ByteOrder::Little, "byte order")?;
    Ok(())
}

fn generation_name() -> String {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    format!(
        "{}-{}-{}",
        std::process::id(),
        time,
        GENERATION.fetch_add(1, Ordering::Relaxed)
    )
}

fn write_country(
    directory: &Path,
    generation: &str,
    index: usize,
    feature_id: &FeatureId,
    table: &CountryTable,
    manifest: &PreparedManifest,
    created: &mut Vec<PathBuf>,
) -> Result<CountryPayload, PreparedError> {
    require(
        table.identity().feature_id() == feature_id,
        "country feature identity",
    )?;
    require(
        GridDocument::from_grid(*table.source_grid()) == manifest.source_grid
            && GridDocument::from_grid(*table.effective_grid()) == manifest.effective_grid
            && table.identity().decimation() == manifest.decimation
            && table.identity().ownership_rule_version() == manifest.ownership_rule_version
            && table.identity().aggregation_order_version() == manifest.aggregation_order_version,
        "country table identity",
    )?;
    let file_name = format!("generation-{generation}-feature-{index:04}.prefix.f64le");
    let path = directory.join(&file_name);
    created.push(path.clone());
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|source| PreparedError::PayloadWrite {
            path: path.clone(),
            source,
        })?;
    let mut writer = BufWriter::new(file);
    let mut digest = Sha256::new();
    for cell in table.prefix_cells() {
        let bytes = cell.to_le_bytes();
        writer
            .write_all(&bytes)
            .map_err(|source| PreparedError::PayloadWrite {
                path: path.clone(),
                source,
            })?;
        digest.update(bytes);
    }
    let file = writer
        .into_inner()
        .map_err(|error| PreparedError::PayloadWrite {
            path: path.clone(),
            source: error.into_error(),
        })?;
    file.sync_all()
        .map_err(|source| PreparedError::PayloadWrite {
            path: path.clone(),
            source,
        })?;
    let payload_sha256 = format!("{:x}", digest.finalize());
    let payload_length = u64::try_from(table.prefix_cells().len())
        .ok()
        .and_then(|count| count.checked_mul(size_of::<f64>() as u64))
        .ok_or(PreparedError::Mismatch {
            ground: "payload length",
        })?;
    verify_written_payload(&path, payload_length, &payload_sha256)?;
    Ok(CountryPayload {
        file: file_name,
        grid: GridDocument::from_grid(*table.grid()),
        north_row: table.north_row(),
        west_col: table.west_col(),
        total: table.total(),
        unassigned: table.unassigned(),
        source_digest: table.identity().source_digest(),
        ownership_digest: table.identity().ownership_digest(),
        payload_length,
        payload_sha256,
    })
}

fn verify_written_payload(
    path: &Path,
    expected_length: u64,
    expected_digest: &str,
) -> Result<(), PreparedError> {
    let mut file = File::open(path).map_err(|source| PreparedError::PayloadRead {
        path: path.to_path_buf(),
        source,
    })?;
    let mut buffer = vec![0; 64 * 1024].into_boxed_slice();
    let mut digest = Sha256::new();
    let mut length = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| PreparedError::PayloadRead {
                path: path.to_path_buf(),
                source,
            })?;
        if read == 0 {
            break;
        }
        length = length.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        digest.update(&buffer[..read]);
    }
    if length != expected_length || format!("{:x}", digest.finalize()) != expected_digest {
        return Err(PreparedError::Mismatch {
            ground: "written payload integrity",
        });
    }
    Ok(())
}

fn safe_payload_path(value: &str) -> bool {
    Path::new(value)
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
}

fn load_country(
    file: &mut File,
    manifest: &PreparedManifest,
    feature_id: &FeatureId,
    payload: &CountryPayload,
) -> Result<CountryTable, PreparedError> {
    let cells = load_payload_cells(file, payload)?;
    let source_grid = manifest
        .source_grid
        .grid()
        .map_err(|()| PreparedError::Mismatch {
            ground: "source grid",
        })?;
    let effective_grid = manifest
        .effective_grid
        .grid()
        .map_err(|()| PreparedError::Mismatch {
            ground: "effective grid",
        })?;
    let grid = payload.grid.grid().map_err(|()| PreparedError::Mismatch {
        ground: "country crop grid",
    })?;
    CountryTable::from_prepared(PreparedCountryTable {
        source_grid,
        effective_grid,
        grid,
        north_row: payload.north_row,
        west_col: payload.west_col,
        total: payload.total,
        unassigned: payload.unassigned,
        identity: PreparedCountryTableIdentity {
            feature_id: feature_id.clone(),
            source_digest: payload.source_digest,
            boundary_source_digest: manifest.boundary_source.sha256.clone().unwrap_or_default(),
            ownership_digest: payload.ownership_digest,
            source_grid,
            effective_grid,
            decimation: manifest.decimation,
            ownership_rule_version: manifest.ownership_rule_version,
            aggregation_order_version: manifest.aggregation_order_version,
        }
        .into(),
        cells,
    })
    .map_err(|source| PreparedError::CountryTable {
        id: feature_id.as_str().to_owned(),
        source,
    })
}

fn load_payload_cells(
    file: &mut File,
    payload: &CountryPayload,
) -> Result<Vec<f64>, PreparedError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|source| PreparedError::PayloadRead {
            path: PathBuf::from(&payload.file),
            source,
        })?;
    let mut buffer = vec![0; 64 * 1024].into_boxed_slice();
    let mut digest = Sha256::new();
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| PreparedError::PayloadRead {
                path: PathBuf::from(&payload.file),
                source,
            })?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    if format!("{:x}", digest.finalize()) != payload.payload_sha256 {
        return Err(PreparedError::PayloadDigest {
            path: payload.file.clone(),
        });
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|source| PreparedError::PayloadRead {
            path: PathBuf::from(&payload.file),
            source,
        })?;
    let count =
        usize::try_from(payload.payload_length / 8).map_err(|_| PreparedError::Mismatch {
            ground: "payload addressable length",
        })?;
    let mut cells = Vec::with_capacity(count);
    for _ in 0..count {
        let mut value = [0; 8];
        file.read_exact(&mut value)
            .map_err(|source| PreparedError::PayloadRead {
                path: PathBuf::from(&payload.file),
                source,
            })?;
        cells.push(f64::from_le_bytes(value));
    }
    Ok(cells)
}

fn open_payload(directory: &Path, payload: &CountryPayload) -> Result<File, PreparedError> {
    let path = directory.join(&payload.file);
    let file = File::open(&path).map_err(|source| PreparedError::PayloadRead {
        path: path.clone(),
        source,
    })?;
    let found = file
        .metadata()
        .map_err(|source| PreparedError::PayloadRead {
            path: path.clone(),
            source,
        })?
        .len();
    if found != payload.payload_length {
        return Err(PreparedError::PayloadLength {
            path: payload.file.clone(),
            expected: payload.payload_length,
            found,
        });
    }
    Ok(file)
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn fingerprint_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut buffer = vec![0; 64 * 1024].into_boxed_slice();
    let mut digest = Sha256::new();
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f' | b'A'..=b'F'))
}

fn require(matches: bool, ground: &'static str) -> Result<(), PreparedError> {
    if matches {
        Ok(())
    } else {
        Err(PreparedError::Mismatch { ground })
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "fixture failures should identify the operation that failed"
)]
mod tests {
    use super::*;
    use popcircles::domain::{Geometry, Polygon};
    use popcircles::ownership::CellOwnership;
    use popcircles::raster::Synthetic;
    use popcircles::table::{Decimation, build_country_decimated};
    use tempfile::TempDir;

    fn grid() -> Grid {
        Grid::new(2, 2, LatLon { lat: 2.0, lon: 0.0 }, 1.0, -1.0).expect("fixture grid is valid")
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

    fn fixture() -> (TempDir, Grid) {
        let directory = tempfile::tempdir().expect("fixture directory is available");
        let grid = grid();
        let country = FeatureId::new("A").expect("feature ID is valid");
        let empty = FeatureId::new("EMPTY").expect("feature ID is valid");
        let (ownership, _) = CellOwnership::rasterize_features(
            &[
                (country.clone(), rectangle(0.0, 0.0, 2.0, 2.0)),
                (empty.clone(), rectangle(10.0, 10.0, 11.0, 11.0)),
            ],
            grid,
        )
        .expect("fixture ownership is valid");
        let raster = Synthetic::new(grid, -9999.0, vec![vec![1.0, 2.0], vec![3.0, 4.0]])
            .expect("fixture raster is valid");
        let table =
            build_country_decimated(raster, &ownership, &country, Decimation::none(grid), "")
                .expect("country table is built");
        let payload_file = "feature-0000.prefix.f64le";
        let mut payload_bytes = Vec::new();
        for cell in table.prefix_cells() {
            payload_bytes.extend_from_slice(&cell.to_le_bytes());
        }
        fs::write(directory.path().join(payload_file), &payload_bytes)
            .expect("payload fixture is written");
        let manifest = PreparedManifest {
            format_version: FORMAT_VERSION,
            combination_id: "fixture".to_owned(),
            resolution: 1,
            population_source: Source {
                name: "file:population.tif".to_owned(),
                sha256: None,
                attribution: None,
                epsg: Some(4326),
                nodata: Some(-9999.0),
                id_field: None,
            },
            boundary_source: Source {
                name: "file:boundaries.geojson".to_owned(),
                sha256: None,
                attribution: None,
                epsg: None,
                nodata: None,
                id_field: Some("CNTR_ID".to_owned()),
            },
            source_grid: GridDocument::from_grid(grid),
            effective_grid: GridDocument::from_grid(grid),
            decimation: 1,
            ownership_rule_version: popcircles::ownership::OWNERSHIP_RULE_VERSION,
            aggregation_order_version: COUNTRY_AGGREGATION_ORDER_VERSION,
            byte_order: ByteOrder::Little,
            features: vec![
                Feature {
                    id: "A".to_owned(),
                    table: Some(CountryPayload {
                        file: payload_file.to_owned(),
                        grid: GridDocument::from_grid(*table.grid()),
                        north_row: table.north_row(),
                        west_col: table.west_col(),
                        total: table.total(),
                        unassigned: table.unassigned(),
                        source_digest: table.identity().source_digest(),
                        ownership_digest: table.identity().ownership_digest(),
                        payload_length: u64::try_from(payload_bytes.len())
                            .expect("fixture payload fits u64"),
                        payload_sha256: sha256(&payload_bytes),
                    }),
                },
                Feature {
                    id: "EMPTY".to_owned(),
                    table: None,
                },
            ],
        };
        fs::write(
            directory.path().join(MANIFEST_NAME),
            serde_json::to_vec(&manifest).expect("fixture manifest serializes"),
        )
        .expect("manifest fixture is written");
        (directory, grid)
    }

    fn request(grid: Grid) -> Request<'static> {
        Request {
            combination_id: "fixture",
            resolution: 1,
            population_source: "file:population.tif",
            population_sha256: None,
            boundary_source: "file:boundaries.geojson",
            boundary_sha256: None,
            population_epsg: 4326,
            population_nodata: -9999.0,
            boundary_source_id_field: "CNTR_ID",
            source_grid: grid,
        }
    }

    #[test]
    fn reads_portable_prefixes_without_provenance_and_keeps_empty_features() {
        let (directory, grid) = fixture();
        let mut loaded = PreparedManifest::load(directory.path(), &request(grid))
            .expect("a provenance-free format-compatible bundle loads");
        assert_eq!(loaded.feature_ids().count(), 2);
        assert_eq!(
            loaded
                .load_table("A")
                .expect("A loads on demand")
                .expect("A has a table")
                .total(),
            10.0
        );
        assert!(
            loaded
                .load_table("EMPTY")
                .expect("empty feature resolves")
                .is_none()
        );
    }

    #[test]
    fn refuses_changed_resolution_and_corrupt_payload_bytes() {
        let (directory, grid) = fixture();
        let mut changed = request(grid);
        changed.resolution = 10;
        assert!(matches!(
            PreparedManifest::load(directory.path(), &changed),
            Err(PreparedError::Mismatch {
                ground: "resolution"
            })
        ));

        let payload = directory.path().join("feature-0000.prefix.f64le");
        let mut bytes = fs::read(&payload).expect("fixture payload is readable");
        bytes[0] ^= 1;
        fs::write(payload, bytes).expect("corrupt fixture payload is written");
        let mut loaded = PreparedManifest::load(directory.path(), &request(grid))
            .expect("manifest checks do not load all country tables");
        assert!(matches!(
            loaded.load_table("A"),
            Err(PreparedError::PayloadDigest { .. })
        ));
    }

    #[test]
    fn refuses_changed_source_interpretation() {
        let (directory, grid) = fixture();
        let mut changed = request(grid);
        changed.population_epsg = 3857;
        assert!(matches!(
            PreparedManifest::load(directory.path(), &changed),
            Err(PreparedError::Mismatch {
                ground: "population source interpretation"
            })
        ));

        let mut changed = request(grid);
        changed.population_nodata = -1.0;
        assert!(matches!(
            PreparedManifest::load(directory.path(), &changed),
            Err(PreparedError::Mismatch {
                ground: "population source interpretation"
            })
        ));

        let mut changed = request(grid);
        changed.boundary_source_id_field = "OTHER_ID";
        assert!(matches!(
            PreparedManifest::load(directory.path(), &changed),
            Err(PreparedError::Mismatch {
                ground: "boundary source interpretation"
            })
        ));
    }

    #[test]
    fn refuses_a_payload_length_that_differs_from_the_manifest() {
        let (directory, grid) = fixture();
        let manifest_path = directory.path().join(MANIFEST_NAME);
        let mut manifest: PreparedManifest = serde_json::from_slice(
            &fs::read(&manifest_path).expect("fixture manifest is readable"),
        )
        .expect("fixture manifest parses");
        manifest.features[0]
            .table
            .as_mut()
            .expect("fixture feature has a table")
            .payload_length += 8;
        fs::write(
            manifest_path,
            serde_json::to_vec(&manifest).expect("changed manifest serializes"),
        )
        .expect("changed manifest is written");
        assert!(matches!(
            PreparedManifest::load(directory.path(), &request(grid)),
            Err(PreparedError::PayloadLength { .. })
        ));
    }

    #[test]
    fn refuses_unknown_format_versions_and_known_source_fingerprint_mismatches() {
        let (directory, grid) = fixture();
        let manifest_path = directory.path().join(MANIFEST_NAME);
        let mut manifest: PreparedManifest = serde_json::from_slice(
            &fs::read(&manifest_path).expect("fixture manifest is readable"),
        )
        .expect("fixture manifest parses");
        manifest.format_version += 1;
        fs::write(
            &manifest_path,
            serde_json::to_vec(&manifest).expect("changed manifest serializes"),
        )
        .expect("changed manifest is written");
        assert!(matches!(
            PreparedManifest::load(directory.path(), &request(grid)),
            Err(PreparedError::FormatVersion { .. })
        ));

        manifest.format_version = FORMAT_VERSION;
        manifest.population_source.sha256 = Some("a".repeat(64));
        fs::write(
            &manifest_path,
            serde_json::to_vec(&manifest).expect("identified manifest serializes"),
        )
        .expect("identified manifest is written");
        let mut known = request(grid);
        let expected_sha256 = "b".repeat(64);
        known.population_sha256 = Some(&expected_sha256);
        assert!(matches!(
            PreparedManifest::load(directory.path(), &known),
            Err(PreparedError::Mismatch {
                ground: "population source fingerprint"
            })
        ));
    }

    #[test]
    fn publishes_new_generations_and_preserves_the_current_manifest_on_refusal_or_failure() {
        let (fixture_directory, grid) = fixture();
        let mut loaded = PreparedManifest::load(fixture_directory.path(), &request(grid))
            .expect("source fixture loads");
        let table_a = loaded
            .load_table("A")
            .expect("country table loads on demand")
            .expect("country has a table");
        let features = vec![
            (
                FeatureId::new("A").expect("feature ID is valid"),
                Some(&table_a),
            ),
            (FeatureId::new("EMPTY").expect("feature ID is valid"), None),
        ];
        let directory = tempfile::tempdir().expect("output directory is available");
        let manifest = loaded.manifest.clone();
        PreparedManifest::publish(directory.path(), manifest.clone(), &features, false)
            .expect("first generation is published");
        let mut first = PreparedManifest::load(directory.path(), &request(grid))
            .expect("first generation loads");

        assert!(matches!(
            PreparedManifest::publish(directory.path(), manifest.clone(), &features, false),
            Err(PreparedError::Existing { .. })
        ));
        let failure = PreparedManifest::publish_with(
            directory.path(),
            manifest.clone(),
            &features,
            true,
            |_, _| Err(io::Error::other("injected publication failure")),
        );
        assert!(matches!(
            failure,
            Err(PreparedError::ManifestPublish { .. })
        ));
        let still_current = PreparedManifest::load(directory.path(), &request(grid))
            .expect("failed replacement leaves the old manifest and payloads usable");
        assert_eq!(still_current.manifest, first.manifest);

        PreparedManifest::publish(directory.path(), manifest.clone(), &features, true)
            .expect("explicit replacement is published");
        let mut replaced = PreparedManifest::load(directory.path(), &request(grid))
            .expect("replacement generation loads");
        assert_eq!(
            replaced
                .load_table("A")
                .expect("replacement country loads")
                .expect("country has a table")
                .total(),
            10.0
        );
        assert_ne!(
            replaced.manifest.features[0].table,
            first.manifest.features[0].table
        );
        assert_eq!(
            first
                .load_table("A")
                .expect("the old generation's open payload remains readable")
                .expect("country has a table")
                .total(),
            10.0
        );

        let concurrent_directory =
            tempfile::tempdir().expect("concurrent output directory is available");
        let mut publisher =
            PreparedWriter::new(concurrent_directory.path(), manifest.clone(), false)
                .expect("first publisher acquires the lock");
        assert!(matches!(
            PreparedWriter::new(concurrent_directory.path(), manifest, false),
            Err(PreparedError::PublisherHeld { .. })
        ));
        for (feature_id, table) in &features {
            publisher
                .write_feature(feature_id, *table)
                .expect("first publisher writes its feature");
        }
        publisher.publish().expect("first publisher completes");
        PreparedManifest::load(concurrent_directory.path(), &request(grid))
            .expect("serialized publisher output loads");
    }
}
