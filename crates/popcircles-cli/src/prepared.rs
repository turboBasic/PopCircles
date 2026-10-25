use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
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
        found: usize,
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
    pub(crate) source_grid: Grid,
}

pub(crate) struct Loaded {
    pub(crate) manifest: PreparedManifest,
    pub(crate) tables: Vec<(FeatureId, Option<CountryTable>)>,
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

impl PreparedManifest {
    pub(crate) fn publish(
        directory: &Path,
        mut manifest: Self,
        features: &[(FeatureId, Option<&CountryTable>)],
        replace: bool,
    ) -> Result<(), PreparedError> {
        Self::publish_with(directory, &mut manifest, features, replace, |from, to| {
            fs::rename(from, to)
        })
    }

    fn publish_with(
        directory: &Path,
        manifest: &mut Self,
        features: &[(FeatureId, Option<&CountryTable>)],
        replace: bool,
        publish_manifest: impl FnOnce(&Path, &Path) -> io::Result<()>,
    ) -> Result<(), PreparedError> {
        fs::create_dir_all(directory).map_err(|source| PreparedError::DirectoryCreate {
            path: directory.to_path_buf(),
            source,
        })?;
        let manifest_path = directory.join(MANIFEST_NAME);
        if manifest_path.exists() && !replace {
            return Err(PreparedError::Existing {
                path: manifest_path,
            });
        }
        let previous = fs::read(&manifest_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Self>(&bytes).ok());
        let generation = generation_name();
        let mut created = Vec::new();
        let result = (|| {
            let mut entries = Vec::with_capacity(features.len());
            for (index, (feature_id, table)) in features.iter().enumerate() {
                let payload = table
                    .map(|table| {
                        write_country(
                            directory,
                            &generation,
                            index,
                            feature_id,
                            table,
                            manifest,
                            &mut created,
                        )
                    })
                    .transpose()?;
                entries.push(Feature {
                    id: feature_id.as_str().to_owned(),
                    table: payload,
                });
            }
            manifest.format_version = FORMAT_VERSION;
            manifest.byte_order = ByteOrder::Little;
            manifest.features = entries;
            let bytes = serde_json::to_vec_pretty(manifest).map_err(|source| {
                PreparedError::ManifestSerialize {
                    path: manifest_path.clone(),
                    source,
                }
            })?;
            let temporary = directory.join(format!("manifest-{generation}.json.tmp"));
            created.push(temporary.clone());
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
            publish_manifest(&temporary, &manifest_path).map_err(|source| {
                PreparedError::ManifestPublish {
                    path: manifest_path.clone(),
                    source,
                }
            })?;
            Ok(())
        })();
        if let Err(error) = result {
            for path in created {
                let _ = fs::remove_file(path);
            }
            return Err(error);
        }
        if let Some(previous) = previous {
            for feature in previous.features {
                if let Some(payload) = feature.table
                    && safe_payload_path(&payload.file)
                {
                    let _ = fs::remove_file(directory.join(payload.file));
                }
            }
        }
        Ok(())
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

        let mut tables = Vec::with_capacity(manifest.features.len());
        let mut feature_ids = BTreeSet::new();
        let mut payload_files = BTreeSet::new();
        for feature in &manifest.features {
            require(
                feature_ids.insert(feature.id.as_str()),
                "unique feature identity",
            )?;
            let feature_id =
                FeatureId::new(feature.id.clone()).map_err(|source| PreparedError::FeatureId {
                    id: feature.id.clone(),
                    source,
                })?;
            let table = feature
                .table
                .as_ref()
                .map(|payload| {
                    require(
                        payload_files.insert(payload.file.as_str()),
                        "unique payload path",
                    )?;
                    require(
                        valid_sha256(&payload.payload_sha256),
                        "payload SHA-256 spelling",
                    )?;
                    load_country(directory, &manifest, &feature_id, payload)
                })
                .transpose()?;
            tables.push((feature_id, table));
        }
        Ok(Loaded { manifest, tables })
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
    directory: &Path,
    manifest: &PreparedManifest,
    feature_id: &FeatureId,
    payload: &CountryPayload,
) -> Result<CountryTable, PreparedError> {
    let relative = Path::new(&payload.file);
    if relative
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(PreparedError::UnsafePayloadPath {
            path: payload.file.clone(),
        });
    }
    let path = directory.join(relative);
    let bytes = fs::read(&path).map_err(|source| PreparedError::PayloadRead {
        path: path.clone(),
        source,
    })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != payload.payload_length {
        return Err(PreparedError::PayloadLength {
            path: payload.file.clone(),
            expected: payload.payload_length,
            found: bytes.len(),
        });
    }
    if sha256(&bytes) != payload.payload_sha256 {
        return Err(PreparedError::PayloadDigest {
            path: payload.file.clone(),
        });
    }
    if !bytes.len().is_multiple_of(size_of::<f64>()) {
        return Err(PreparedError::PayloadAlignment {
            path: payload.file.clone(),
        });
    }
    let cells = bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|value| f64::from_le_bytes(*value))
        .collect();
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

pub(crate) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
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
#[allow(clippy::expect_used, clippy::unwrap_used)]
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
            },
            boundary_source: Source {
                name: "file:boundaries.geojson".to_owned(),
                sha256: None,
                attribution: None,
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
            source_grid: grid,
        }
    }

    #[test]
    fn reads_portable_prefixes_without_provenance_and_keeps_empty_features() {
        let (directory, grid) = fixture();
        let loaded = PreparedManifest::load(directory.path(), &request(grid))
            .expect("a provenance-free format-compatible bundle loads");
        assert_eq!(loaded.tables.len(), 2);
        assert_eq!(loaded.tables[0].0.as_str(), "A");
        assert_eq!(
            loaded.tables[0].1.as_ref().expect("A has a table").total(),
            10.0
        );
        assert_eq!(loaded.tables[1].0.as_str(), "EMPTY");
        assert!(loaded.tables[1].1.is_none());
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
        assert!(matches!(
            PreparedManifest::load(directory.path(), &request(grid)),
            Err(PreparedError::PayloadDigest { .. })
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
        let loaded = PreparedManifest::load(fixture_directory.path(), &request(grid))
            .expect("source fixture loads");
        let features: Vec<_> = loaded
            .tables
            .iter()
            .map(|(id, table)| (id.clone(), table.as_ref()))
            .collect();
        let directory = tempfile::tempdir().expect("output directory is available");
        let mut manifest = loaded.manifest.clone();
        PreparedManifest::publish(directory.path(), manifest.clone(), &features, false)
            .expect("first generation is published");
        let first = PreparedManifest::load(directory.path(), &request(grid))
            .expect("first generation loads");

        assert!(matches!(
            PreparedManifest::publish(directory.path(), manifest.clone(), &features, false),
            Err(PreparedError::Existing { .. })
        ));
        let failure = PreparedManifest::publish_with(
            directory.path(),
            &mut manifest,
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

        PreparedManifest::publish(directory.path(), manifest, &features, true)
            .expect("explicit replacement is published");
        let replaced = PreparedManifest::load(directory.path(), &request(grid))
            .expect("replacement generation loads");
        assert_eq!(
            replaced.tables[0]
                .1
                .as_ref()
                .expect("country has a table")
                .total(),
            10.0
        );
        assert_ne!(
            replaced.manifest.features[0].table,
            first.manifest.features[0].table
        );
    }
}
