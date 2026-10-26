// Dataset defaults and registered raster paths are resolved in the CLI, outside the numeric library.
//
// No `deny_unknown_fields`: the licence, the checksum and the attribution are keys this reader has no use
// for, and `python/src/population_circles/dataset_registry.py` forbids an unknown one — so a typo is caught
// by the reader that claims every field rather than by the one that claims eight.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::args::GridValues;

/// Relative to the working directory, which is where `out/table` and `out/radii.json` already resolve
/// from.
pub(crate) const REGISTRY_PATH: &str = "data/registry.toml";
pub(crate) const COUNTRY_BOUNDARY_DATASET: &str = "gisco-countries-2024-1m";

#[derive(Debug, thiserror::Error)]
pub(crate) enum RegistryError {
    // Nothing fetches this file — it is committed — so the message says what to do instead.
    #[error("the dataset registry at {} could not be read; run from the repository root", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("the dataset registry at {} is not the TOML document it should be", path.display())]
    Syntax {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    // One ground for a name no table can be built from, whether nothing is registered under it or a
    // boundary vector is: either way what a caller does is pick a name off the list, and the list is the
    // rasters. A second variant would distinguish two cases with one answer.
    #[error("`{name}` is not a registered population raster; the registered ones are {known}")]
    Unknown { name: String, known: String },

    #[error("`{name}` is not a registered settlement vector; the registered ones are {known}")]
    UnknownSettlement { name: String, known: String },

    #[error("dataset `{name}` has path {path}, whose stem is not `{name}`")]
    KeyIsNotTheStem { name: String, path: String },

    #[error("population dataset `{name}` has an invalid direct source identity")]
    InvalidDerivedIdentity { name: String },

    #[error("boundary dataset `{name}` has incomplete or invalid vector metadata")]
    InvalidBoundaryMetadata { name: String },
}

/// What a build needs to read a raster: the file, and everything the file is held to.
///
/// `grid` is [`GridValues`] rather than six numbers of its own, because that struct already owns the
/// conversion into a [`Grid`](popcircles::grid::Grid) and a second one here would be a second place a
/// declared grid is assembled.
#[derive(Debug)]
pub(crate) struct Dataset {
    pub(crate) raster: PathBuf,
    pub(crate) sha256: Option<String>,
    pub(crate) attribution: Option<String>,
    pub(crate) grid: GridValues,
    pub(crate) epsg: u16,
    pub(crate) nodata: f32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Registry {
    datasets: BTreeMap<String, Row>,
}

/// A row by its kind, which says whether its fields describe a population raster or a boundary. An unregistered
/// kind is a refusal rather than a row to skip, for the reason the Python reader discriminates too: a kind
/// arrives in the commit that teaches both readers to use it.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum Row {
    PopulationRaster(PopulationRaster),
    BoundaryVector(BoundaryVector),
    SettlementVector(SettlementVector),
}

#[derive(Debug, Deserialize)]
struct PopulationRaster {
    path: PathBuf,
    sha256: Option<String>,
    attribution: Option<String>,
    width: u32,
    height: u32,
    origin_lat: f64,
    origin_lon: f64,
    lon_step: f64,
    lat_step: f64,
    epsg: u16,
    /// Narrowed from the f64 TOML parses to the f32 the raster reader compares bit for bit, which is the
    /// one conversion in this file and the reason `the_sentinel_narrows_to_the_f32_a_raster_is_held_to`
    /// exists.
    nodata: f32,
    derived_from_dataset: Option<String>,
    derived_from_sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BoundaryVector {
    pub(crate) path: PathBuf,
    pub(crate) sha256: Option<String>,
    pub(crate) format: Option<String>,
    pub(crate) crs: Option<String>,
    pub(crate) axis_order: Option<String>,
    pub(crate) feature_count: Option<u32>,
    pub(crate) revision: Option<String>,
    pub(crate) source_id_field: Option<String>,
    pub(crate) identity_policy: Option<String>,
    pub(crate) attribution: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SettlementVector {
    path: PathBuf,
}

impl Row {
    fn path(&self) -> &Path {
        match self {
            Self::PopulationRaster(row) => &row.path,
            Self::BoundaryVector(row) => &row.path,
            Self::SettlementVector(row) => &row.path,
        }
    }
}

impl Registry {
    /// # Errors
    /// [`RegistryError::Syntax`] when the text is not the document this reads, and
    /// [`RegistryError::KeyIsNotTheStem`] when a key and its file disagree.
    pub(crate) fn parse(path: &Path, text: &str) -> Result<Self, RegistryError> {
        let registry: Self = toml::from_str(text).map_err(|source| RegistryError::Syntax {
            path: path.to_path_buf(),
            source,
        })?;

        // The property that makes one string the key, the file, the release asset and the heading in
        // `data/README.md`. Checked here rather than trusted, exactly as the Python reader checks it.
        for (name, row) in &registry.datasets {
            let stem = row.path().file_stem().and_then(|stem| stem.to_str());
            if stem != Some(name.as_str()) {
                return Err(RegistryError::KeyIsNotTheStem {
                    name: name.clone(),
                    path: row.path().display().to_string(),
                });
            }
            if let Some(Row::PopulationRaster(dataset)) = registry.datasets.get(name) {
                let valid_identity = match (
                    dataset.derived_from_dataset.as_deref(),
                    dataset.derived_from_sha256.as_deref(),
                ) {
                    (None, None) => true,
                    (Some(source_name), Some(source_digest)) => {
                        let valid_digest = source_digest.len() == 64
                            && source_digest
                                .bytes()
                                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
                        let is_direct_source = matches!(
                            registry.datasets.get(source_name),
                            Some(Row::PopulationRaster(source))
                                if source.derived_from_dataset.is_none()
                                    && source.sha256.as_deref() == Some(source_digest)
                        );
                        valid_digest && is_direct_source
                    }
                    _ => false,
                };
                if !valid_identity {
                    return Err(RegistryError::InvalidDerivedIdentity { name: name.clone() });
                }
            }
            if let Some(boundary) = registry.boundary(name) {
                let fields = [
                    boundary.format.as_deref(),
                    boundary.crs.as_deref(),
                    boundary.axis_order.as_deref(),
                    boundary.source_id_field.as_deref(),
                    boundary.identity_policy.as_deref(),
                ];
                let any_metadata =
                    fields.iter().any(Option::is_some) || boundary.feature_count.is_some();
                let complete = fields.iter().all(Option::is_some)
                    && boundary.feature_count.is_some_and(|count| count > 0);
                let valid_digest = boundary.sha256.as_deref().is_some_and(|digest| {
                    digest.len() == 64
                        && digest.bytes().all(|byte| {
                            byte.is_ascii_digit()
                                || (b'a'..=b'f').contains(&byte)
                                || (b'A'..=b'F').contains(&byte)
                        })
                });
                let valid = boundary.format.as_deref() == Some("GeoJSON")
                    && boundary.crs.as_deref() == Some("EPSG:4326")
                    && boundary.axis_order.as_deref() == Some("longitude-latitude");
                if !valid_digest || (any_metadata && (!complete || !valid)) {
                    return Err(RegistryError::InvalidBoundaryMetadata { name: name.clone() });
                }
            }
        }
        Ok(registry)
    }

    /// # Errors
    /// [`RegistryError::Read`] when the file is not there, and [`Self::parse`]'s grounds.
    pub(crate) fn load(path: &Path) -> Result<Self, RegistryError> {
        let text = fs::read_to_string(path).map_err(|source| RegistryError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(path, &text)
    }

    /// # Errors
    /// [`RegistryError::Unknown`] naming the rasters, for a name no table can be built from.
    pub(crate) fn raster(&self, name: &str) -> Result<Dataset, RegistryError> {
        let row = match self.datasets.get(name) {
            Some(Row::PopulationRaster(row)) => row,
            Some(Row::BoundaryVector(_) | Row::SettlementVector(_)) | None => {
                return Err(RegistryError::Unknown {
                    name: name.to_owned(),
                    // Ordered, because a `BTreeMap` is: the message a typo gets is the same message twice
                    // running rather than whatever order a file happened to be in.
                    known: self
                        .datasets
                        .iter()
                        .filter(|(_, row)| matches!(row, Row::PopulationRaster(_)))
                        .map(|(key, _)| key.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                });
            }
        };

        Ok(Dataset {
            raster: row.path.clone(),
            sha256: row.sha256.clone(),
            attribution: row.attribution.clone(),
            grid: GridValues {
                width: row.width,
                height: row.height,
                origin_lat: row.origin_lat,
                origin_lon: row.origin_lon,
                lon_step: row.lon_step,
                lat_step: row.lat_step,
            },
            epsg: row.epsg,
            nodata: row.nodata,
        })
    }

    pub(crate) fn default_raster(&self) -> Result<(String, Dataset), RegistryError> {
        if let Ok(dataset) = self.raster("population-count-2020-30arcsec") {
            return Ok(("population-count-2020-30arcsec".to_owned(), dataset));
        }
        let Some((name, Row::PopulationRaster(_))) = self
            .datasets
            .iter()
            .find(|(_, row)| matches!(row, Row::PopulationRaster(_)))
        else {
            return Err(RegistryError::Unknown {
                name: "default population raster".to_owned(),
                known: String::new(),
            });
        };
        Ok((name.clone(), self.raster(name)?))
    }

    /// Resolves a settlement-vector path without requiring population-grid metadata.
    ///
    /// # Errors
    /// Returns [`RegistryError::UnknownSettlement`] when the key is absent or has another kind.
    pub(crate) fn settlements(&self, name: &str) -> Result<PathBuf, RegistryError> {
        match self.datasets.get(name) {
            Some(Row::SettlementVector(row)) => Ok(row.path.clone()),
            Some(Row::BoundaryVector(_) | Row::PopulationRaster(_)) | None => {
                Err(RegistryError::UnknownSettlement {
                    name: name.to_owned(),
                    known: self
                        .datasets
                        .iter()
                        .filter(|(_, row)| matches!(row, Row::SettlementVector(_)))
                        .map(|(key, _)| key.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                })
            }
        }
    }

    /// Resolves a boundary vector's path and optional country description.
    pub(crate) fn boundary(&self, name: &str) -> Option<&BoundaryVector> {
        match self.datasets.get(name) {
            Some(Row::BoundaryVector(row)) => Some(row),
            Some(Row::PopulationRaster(_) | Row::SettlementVector(_)) | None => None,
        }
    }
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows both in tests. float_cmp because the steps are asserted against
// 1.0 / 120.0 exactly, which is the claim — a tolerance is what would let a truncated literal pass.
#[cfg(test)]
#[allow(clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;

    /// The committed registry, reached from the manifest rather than from the working directory: a unit
    /// test runs with its package as the working directory, and the file is two levels above that.
    fn committed() -> Registry {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(REGISTRY_PATH);
        Registry::load(&path).expect("the committed registry is this reader's own document")
    }

    #[test]
    fn the_registry_declares_the_grid_the_population_raster_is_held_to() {
        let dataset = committed()
            .raster("population-count-2020-30arcsec")
            .expect("the registry's population raster");
        assert_eq!(
            dataset.raster,
            PathBuf::from("data/population/population-count-2020-30arcsec.tif")
        );
        assert_eq!((dataset.grid.width, dataset.grid.height), (43200, 21600));
        assert_eq!(dataset.epsg, 4326);
        // The steps to full precision, because a truncated 1/120 misses a 180 degree span by 7.1e-13 over
        // the height and the registry's own comment says so.
        assert_eq!(dataset.grid.lon_step, 1.0 / 120.0);
        assert_eq!(dataset.grid.lat_step, -1.0 / 120.0);
    }

    #[test]
    fn default_raster_keeps_the_registry_key_with_its_source() {
        let (name, dataset) = committed()
            .default_raster()
            .expect("the registry's default raster");
        assert_eq!(name, "population-count-2020-30arcsec");
        assert_eq!(
            dataset.raster,
            PathBuf::from("data/population/population-count-2020-30arcsec.tif")
        );
    }

    #[test]
    fn the_coarse_raster_names_its_direct_source_identity() {
        let registry = committed();
        let coarse = registry
            .raster("population-count-2020-10arcmin")
            .expect("the coarse population dataset is registered with its source identity");
        let native = registry
            .raster("population-count-2020-30arcsec")
            .expect("the native population dataset remains registered");
        assert_eq!((coarse.grid.width, coarse.grid.height), (2160, 1080));
        assert_ne!(coarse.sha256, native.sha256);
    }

    #[test]
    fn the_sentinel_narrows_to_the_f32_a_raster_is_held_to() {
        // The one number in the registry that decides whether a raster opens at all: it is compared bit
        // for bit against the file's own tag, so the f64 TOML parses has to narrow to this exact f32 —
        // two ulps above -f32::MAX, which is what every fixture in this workspace spells -3.402_823e38.
        let dataset = committed()
            .raster("population-count-2020-30arcsec")
            .expect("the registry's population raster");
        assert_eq!(dataset.nodata.to_bits(), 0xff7f_fffd);
    }

    #[test]
    fn an_unknown_name_is_refused_naming_the_rasters_and_only_those() {
        // Only the rows a table can be built from: a name offered here and then refused as the wrong kind
        // would cost a second run to learn what the first could have said.
        let error = committed()
            .raster("gpw-v4")
            .expect_err("no such dataset is registered")
            .to_string();
        assert!(
            error.contains("`gpw-v4` is not a registered population raster"),
            "{error}"
        );
        assert!(error.contains("population-count-2020-30arcsec"), "{error}");
        assert!(!error.contains("coastline-1to110m"), "{error}");

        // And a name the registry does carry, as something no table can be built from, is refused the same
        // way rather than by a variant of its own.
        let vector = committed()
            .raster("coastline-1to110m")
            .expect_err("a boundary vector is no raster")
            .to_string();
        assert!(
            vector.contains("`coastline-1to110m` is not a registered population raster"),
            "{vector}"
        );
    }

    #[test]
    fn a_key_that_is_not_its_files_stem_is_refused() {
        let text = r#"
[datasets.coastline]
kind = "settlement-vector"
path = "data/settlements/coastline-1to110m.geojson"
"#;
        let error = Registry::parse(Path::new("fixture.toml"), text)
            .expect_err("the key is not the stem")
            .to_string();
        assert!(error.contains("dataset `coastline`"), "{error}");
        assert!(error.contains("coastline-1to110m.geojson"), "{error}");
    }

    #[test]
    fn settlement_vectors_resolve_without_grid_fields() {
        let text = r#"
[datasets.example-places]
kind = "settlement-vector"
path = "data/settlements/example-places.geojson"
"#;
        let registry = Registry::parse(Path::new("fixture.toml"), text)
            .expect("the settlement row needs no population grid");

        assert_eq!(
            registry
                .settlements("example-places")
                .expect("the registered settlement path"),
            PathBuf::from("data/settlements/example-places.geojson")
        );
        assert!(matches!(
            registry.settlements("missing"),
            Err(RegistryError::UnknownSettlement { .. })
        ));
    }

    #[test]
    fn country_boundary_description_is_available_without_population_grid_fields() {
        let text = r#"
[datasets.countries-2024]
kind = "boundary-vector"
path = "data/boundaries/countries-2024.geojson"
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
format = "GeoJSON"
crs = "EPSG:4326"
axis_order = "longitude-latitude"
feature_count = 263
revision = "2024"
source_id_field = "CNTR_ID"
identity_policy = "ADR 0015"
"#;
        let registry = Registry::parse(Path::new("fixture.toml"), text)
            .expect("the country boundary description is complete");
        let boundary = registry
            .boundary("countries-2024")
            .expect("the registered boundary vector");

        assert_eq!(
            boundary.path,
            PathBuf::from("data/boundaries/countries-2024.geojson")
        );
        assert_eq!(boundary.format.as_deref(), Some("GeoJSON"));
        assert_eq!(boundary.crs.as_deref(), Some("EPSG:4326"));
        assert_eq!(boundary.feature_count, Some(263));
        assert!(registry.raster("countries-2024").is_err());
    }

    #[test]
    fn a_boundary_revision_can_be_recorded_without_country_identity_metadata() {
        let text = r#"
[datasets.coastline]
kind = "boundary-vector"
path = "data/boundaries/coastline.geojson"
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
revision = "Countries 2024 snapshot"
"#;
        let registry = Registry::parse(Path::new("fixture.toml"), text)
            .expect("a coastline can pin its source revision without country metadata");

        assert_eq!(
            registry
                .boundary("coastline")
                .and_then(|boundary| boundary.revision.as_deref()),
            Some("Countries 2024 snapshot")
        );
    }

    #[test]
    fn incomplete_or_invalid_country_boundary_metadata_is_refused() {
        let valid = r#"
[datasets.countries-2024]
kind = "boundary-vector"
path = "data/boundaries/countries-2024.geojson"
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
format = "GeoJSON"
crs = "EPSG:4326"
axis_order = "longitude-latitude"
feature_count = 263
revision = "2024"
source_id_field = "CNTR_ID"
identity_policy = "ADR 0015"
"#;
        for invalid in [
            valid.replace("identity_policy = \"ADR 0015\"\n", ""),
            valid.replace("feature_count = 263", "feature_count = 0"),
            valid.replace("crs = \"EPSG:4326\"", "crs = \"EPSG:3857\""),
        ] {
            assert!(matches!(
                Registry::parse(Path::new("fixture.toml"), &invalid),
                Err(RegistryError::InvalidBoundaryMetadata { .. })
            ));
        }
    }

    #[test]
    fn an_unknown_registry_kind_is_a_syntax_error() {
        let text = r#"
[datasets.example-places]
kind = "settlement"
path = "data/settlements/example-places.geojson"
"#;

        assert!(matches!(
            Registry::parse(Path::new("fixture.toml"), text),
            Err(RegistryError::Syntax { .. })
        ));
    }
}
