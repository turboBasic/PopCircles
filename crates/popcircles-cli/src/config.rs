use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use popcircles::geodesy::LatLon;
use popcircles::grid::Grid;
use popcircles::raster::{PixelType, RasterSpec};
use serde::Deserialize;

use crate::registry::{REGISTRY_PATH, Registry};

pub(crate) const BUILTIN_CONFIG_PATH: &str = "data/config.toml";

#[derive(Debug, thiserror::Error)]
pub(crate) enum ConfigError {
    #[error("configuration at {path} could not be read: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("configuration at {path} is not valid TOML: {source}")]
    Syntax {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("configuration at {path}: {message}")]
    Invalid { path: PathBuf, message: String },
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Document {
    #[serde(default)]
    defaults: Defaults,
    #[serde(default)]
    combinations: BTreeMap<String, PartialCombination>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Defaults {
    population_dataset: Option<String>,
    country_combination: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PartialCombination {
    population_source: Option<String>,
    boundary_source: Option<String>,
    default_resolution: Option<u32>,
    #[serde(default)]
    resolutions: BTreeMap<u32, Location>,
    provenance: Option<String>,
    attribution: Option<String>,
    population_metadata: Option<PartialPopulationMetadata>,
    boundary_metadata: Option<PartialBoundaryMetadata>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PartialPopulationMetadata {
    width: Option<u32>,
    height: Option<u32>,
    origin_lat: Option<f64>,
    origin_lon: Option<f64>,
    lon_step: Option<f64>,
    lat_step: Option<f64>,
    epsg: Option<u16>,
    nodata: Option<f32>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PartialBoundaryMetadata {
    source_id_field: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct Location {
    pub(crate) local: Option<PathBuf>,
    pub(crate) remote: Option<String>,
}

#[derive(Debug)]
pub(crate) struct Combination {
    pub(crate) id: String,
    pub(crate) population_source: String,
    pub(crate) boundary_source: String,
    pub(crate) resolutions: BTreeMap<u32, Location>,
    pub(crate) default_resolution: u32,
    #[expect(
        dead_code,
        reason = "#222 carries supplied provenance into calculation results"
    )]
    pub(crate) provenance: Option<String>,
    pub(crate) attribution: Option<String>,
    pub(crate) population: Option<RasterSpec>,
    pub(crate) boundary_source_id_field: Option<String>,
}

#[derive(Debug)]
pub(crate) struct Configuration {
    pub(crate) default_population_dataset: String,
    pub(crate) default_country_combination: String,
    pub(crate) combinations: BTreeMap<String, Combination>,
}

impl Configuration {
    pub(crate) fn load(user_path: Option<&Path>) -> Result<Self, ConfigError> {
        Self::load_from_paths(
            Path::new(BUILTIN_CONFIG_PATH),
            Path::new(REGISTRY_PATH),
            user_path,
        )
    }

    fn load_from_paths(
        builtin_path: &Path,
        registry_path: &Path,
        user_path: Option<&Path>,
    ) -> Result<Self, ConfigError> {
        let registry = Registry::load(registry_path).map_err(|error| ConfigError::Invalid {
            path: registry_path.to_path_buf(),
            message: error.to_string(),
        })?;
        let mut base = read_document(builtin_path)?;
        validate_complete(&base, builtin_path, &registry)?;
        if let Some(path) = user_path {
            let overlay = read_document(path)?;
            merge(&mut base, overlay, path, &registry)?;
        }
        let source_path = user_path.unwrap_or(builtin_path);
        let default_population_dataset = base
            .defaults
            .population_dataset
            .ok_or_else(|| invalid(source_path, "missing defaults.population_dataset"))?;
        let default_country_combination = base
            .defaults
            .country_combination
            .ok_or_else(|| invalid(source_path, "missing defaults.country_combination"))?;
        let mut combinations = BTreeMap::new();
        for (id, row) in base.combinations {
            let default_resolution = row.default_resolution.ok_or_else(|| {
                invalid(
                    source_path,
                    format!("combination `{id}` has no default_resolution"),
                )
            })?;
            if !row.resolutions.contains_key(&default_resolution) {
                return Err(invalid(
                    source_path,
                    format!(
                        "combination `{id}` default_resolution {default_resolution} is unavailable"
                    ),
                ));
            }
            let population =
                local_population_spec(source_path, &id, row.population_metadata.as_ref())?;
            let boundary_source_id_field = row
                .boundary_metadata
                .as_ref()
                .and_then(|metadata| metadata.source_id_field.clone());
            combinations.insert(
                id.clone(),
                Combination {
                    population_source: row.population_source.ok_or_else(|| {
                        invalid(
                            source_path,
                            format!("combination `{id}` is missing population_source"),
                        )
                    })?,
                    boundary_source: row.boundary_source.ok_or_else(|| {
                        invalid(
                            source_path,
                            format!("combination `{id}` is missing boundary_source"),
                        )
                    })?,
                    resolutions: row.resolutions,
                    default_resolution,
                    provenance: row.provenance,
                    attribution: row.attribution,
                    population,
                    boundary_source_id_field,
                    id,
                },
            );
        }
        if !combinations.contains_key(&default_country_combination) {
            return Err(invalid(
                source_path,
                format!(
                    "default country combination `{default_country_combination}` is not configured"
                ),
            ));
        }
        Ok(Self {
            default_population_dataset,
            default_country_combination,
            combinations,
        })
    }

    pub(crate) fn population_dataset<'a>(&'a self, explicit: Option<&'a str>) -> &'a str {
        explicit.unwrap_or(&self.default_population_dataset)
    }

    pub(crate) fn combination(
        &self,
        id: Option<&str>,
        resolution: Option<u32>,
    ) -> Result<(&Combination, u32), ConfigError> {
        let id = id.unwrap_or(&self.default_country_combination);
        let combination = self.combinations.get(id).ok_or_else(|| {
            invalid(
                Path::new(BUILTIN_CONFIG_PATH),
                format!("unknown country combination `{id}`"),
            )
        })?;
        let resolution = resolution.unwrap_or(combination.default_resolution);
        if !combination.resolutions.contains_key(&resolution) {
            return Err(invalid(
                Path::new(BUILTIN_CONFIG_PATH),
                format!("combination `{id}` has no resolution {resolution}"),
            ));
        }
        Ok((combination, resolution))
    }
}

fn read_document(path: &Path) -> Result<Document, ConfigError> {
    let text = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&text).map_err(|source| ConfigError::Syntax {
        path: path.to_path_buf(),
        source,
    })
}

fn validate_complete(
    document: &Document,
    path: &Path,
    registry: &Registry,
) -> Result<(), ConfigError> {
    if document.defaults.population_dataset.is_none() {
        return Err(invalid(path, "missing defaults.population_dataset"));
    }
    if document.defaults.country_combination.is_none() {
        return Err(invalid(path, "missing defaults.country_combination"));
    }
    for (id, row) in &document.combinations {
        let population = row.population_source.as_deref().ok_or_else(|| {
            invalid(
                path,
                format!("combination `{id}` is missing population_source"),
            )
        })?;
        let boundary = row.boundary_source.as_deref().ok_or_else(|| {
            invalid(
                path,
                format!("combination `{id}` is missing boundary_source"),
            )
        })?;
        validate_source(path, id, "population_source", population, registry, true)?;
        validate_source(path, id, "boundary_source", boundary, registry, false)?;
        validate_source_metadata(path, id, row, population, boundary)?;
        let default = row.default_resolution.ok_or_else(|| {
            invalid(
                path,
                format!("combination `{id}` is missing default_resolution"),
            )
        })?;
        if row.resolutions.contains_key(&0) {
            return Err(invalid(
                path,
                format!("combination `{id}` has invalid resolution 0"),
            ));
        }
        if row.resolutions.is_empty() || !row.resolutions.contains_key(&default) {
            return Err(invalid(
                path,
                format!("combination `{id}` default_resolution {default} is unavailable"),
            ));
        }
        if row.resolutions.iter().any(|(_, location)| {
            (location.local.is_none() && location.remote.is_none())
                || location
                    .local
                    .as_ref()
                    .is_some_and(|path| path.as_os_str().is_empty())
                || location.remote.as_deref().is_some_and(str::is_empty)
        }) {
            return Err(invalid(
                path,
                format!("combination `{id}` has a resolution without a local or remote location"),
            ));
        }
    }
    let dataset = document
        .defaults
        .population_dataset
        .as_deref()
        .unwrap_or_default();
    validate_source(
        path,
        "defaults",
        "population_dataset",
        &format!("dataset:{dataset}"),
        registry,
        true,
    )?;
    let default_combination = document
        .defaults
        .country_combination
        .as_deref()
        .unwrap_or_default();
    if !document.combinations.contains_key(default_combination) {
        return Err(invalid(
            path,
            format!("default country combination `{default_combination}` is not configured"),
        ));
    }
    Ok(())
}

fn validate_source_metadata(
    path: &Path,
    id: &str,
    row: &PartialCombination,
    population: &str,
    boundary: &str,
) -> Result<(), ConfigError> {
    for (field, local, present) in [
        (
            "population_metadata",
            population.starts_with("file:"),
            row.population_metadata.is_some(),
        ),
        (
            "boundary_metadata",
            boundary.starts_with("file:"),
            row.boundary_metadata.is_some(),
        ),
    ] {
        if local != present {
            return Err(invalid(
                path,
                format!(
                    "combination `{id}` {field} is required for a local source and forbidden for a dataset source"
                ),
            ));
        }
    }
    if let Some(metadata) = &row.population_metadata {
        let _ = population_spec(path, id, metadata)?;
    }
    if row.boundary_metadata.is_some()
        && row
            .boundary_metadata
            .as_ref()
            .and_then(|metadata| metadata.source_id_field.as_deref())
            .is_none_or(str::is_empty)
    {
        return Err(invalid(
            path,
            format!("combination `{id}` boundary_metadata.source_id_field is missing or empty"),
        ));
    }
    Ok(())
}

fn local_population_spec(
    path: &Path,
    id: &str,
    metadata: Option<&PartialPopulationMetadata>,
) -> Result<Option<RasterSpec>, ConfigError> {
    metadata
        .map(|metadata| population_spec(path, id, metadata))
        .transpose()
}

fn population_spec(
    path: &Path,
    id: &str,
    metadata: &PartialPopulationMetadata,
) -> Result<RasterSpec, ConfigError> {
    macro_rules! required {
        ($field:ident) => {
            metadata.$field.ok_or_else(|| {
                invalid(
                    path,
                    format!(
                        "combination `{id}` population_metadata.{} is required",
                        stringify!($field)
                    ),
                )
            })?
        };
    }
    let width = required!(width);
    let height = required!(height);
    let origin_lat = required!(origin_lat);
    let origin_lon = required!(origin_lon);
    let lon_step = required!(lon_step);
    let lat_step = required!(lat_step);
    let epsg = required!(epsg);
    let nodata = required!(nodata);
    let grid = Grid::new(
        width,
        height,
        LatLon {
            lat: origin_lat,
            lon: origin_lon,
        },
        lon_step,
        lat_step,
    )
    .map_err(|error| {
        invalid(
            path,
            format!("combination `{id}` population_metadata grid: {error}"),
        )
    })?;
    if epsg == 0 {
        return Err(invalid(
            path,
            format!("combination `{id}` population_metadata.epsg must be positive"),
        ));
    }
    if !nodata.is_finite() {
        return Err(invalid(
            path,
            format!("combination `{id}` population_metadata.nodata must be finite"),
        ));
    }
    Ok(RasterSpec {
        grid,
        epsg,
        pixel: PixelType::Float32,
        nodata,
    })
}

fn validate_source(
    path: &Path,
    id: &str,
    field: &str,
    source: &str,
    registry: &Registry,
    population: bool,
) -> Result<(), ConfigError> {
    if let Some(name) = source.strip_prefix("dataset:") {
        let valid = if population {
            registry.raster(name).is_ok()
        } else {
            registry.boundary(name).is_some()
        };
        if !valid {
            return Err(invalid(
                path,
                format!("`{id}` {field} refers to undeclared or wrong-kind dataset `{name}`"),
            ));
        }
    } else if source.strip_prefix("file:").is_none_or(str::is_empty) {
        return Err(invalid(
            path,
            format!("`{id}` {field} must use dataset:<name> or file:<path>"),
        ));
    }
    Ok(())
}

fn merge(
    base: &mut Document,
    overlay: Document,
    overlay_path: &Path,
    registry: &Registry,
) -> Result<(), ConfigError> {
    if let Some(value) = overlay.defaults.population_dataset {
        base.defaults.population_dataset = Some(value);
    }
    if let Some(value) = overlay.defaults.country_combination {
        base.defaults.country_combination = Some(value);
    }
    for (id, mut user) in overlay.combinations {
        if let Some(builtin) = base.combinations.get_mut(&id) {
            if user.provenance.is_some() {
                return Err(invalid(
                    overlay_path,
                    format!("combination `{id}` cannot override provenance"),
                ));
            }
            if user.attribution.is_some() {
                return Err(invalid(
                    overlay_path,
                    format!("combination `{id}` cannot override attribution"),
                ));
            }
            if user.population_source.is_some() || user.boundary_source.is_some() {
                for (field, supplied, expected) in [
                    (
                        "population_source",
                        user.population_source.as_ref(),
                        builtin.population_source.as_ref(),
                    ),
                    (
                        "boundary_source",
                        user.boundary_source.as_ref(),
                        builtin.boundary_source.as_ref(),
                    ),
                ] {
                    if supplied != expected {
                        return Err(invalid(
                            overlay_path,
                            format!(
                                "combination `{id}` source identity does not match built-in {field}"
                            ),
                        ));
                    }
                }
            }
            if let Some(default) = user.default_resolution {
                builtin.default_resolution = Some(default);
            }
            if !user.resolutions.is_empty() {
                for (resolution, mut location) in user.resolutions {
                    if let Some(path) = &location.local {
                        location.local = Some(resolve(overlay_path, path));
                    }
                    builtin.resolutions.insert(resolution, location);
                }
            }
        } else {
            if user.population_source.is_none()
                || user.boundary_source.is_none()
                || user.default_resolution.is_none()
            {
                return Err(invalid(
                    overlay_path,
                    format!(
                        "new combination `{id}` requires population_source, boundary_source, and default_resolution"
                    ),
                ));
            }
            for (field, source, population) in [
                (
                    "population_source",
                    user.population_source.clone().unwrap_or_default(),
                    true,
                ),
                (
                    "boundary_source",
                    user.boundary_source.clone().unwrap_or_default(),
                    false,
                ),
            ] {
                validate_source(overlay_path, &id, field, &source, registry, population)?;
                if source.starts_with("file:") {
                    if field == "population_source" {
                        user.population_source =
                            Some(resolve_file_source(overlay_path, &id, field, &source)?);
                    } else {
                        user.boundary_source =
                            Some(resolve_file_source(overlay_path, &id, field, &source)?);
                    }
                }
            }
            for location in user.resolutions.values_mut() {
                if let Some(path) = &location.local {
                    location.local = Some(resolve(overlay_path, path));
                }
            }
            base.combinations.insert(id, user);
        }
    }
    validate_complete(base, overlay_path, registry)
}

fn resolve(config_path: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        config_path.parent().unwrap_or(Path::new(".")).join(path)
    }
}

fn resolve_file_source(
    config_path: &Path,
    id: &str,
    field: &str,
    source: &str,
) -> Result<String, ConfigError> {
    let file = source.strip_prefix("file:").unwrap_or_default();
    let resolved = resolve(config_path, Path::new(file));
    let Some(path) = resolved.to_str() else {
        return Err(invalid(
            config_path,
            format!("combination `{id}` {field} path cannot be represented as UTF-8"),
        ));
    };
    Ok(format!("file:{path}"))
}

fn invalid(path: &Path, message: impl Into<String>) -> ConfigError {
    ConfigError::Invalid {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn registry() -> Registry {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(REGISTRY_PATH);
        Registry::load(&path).expect("the committed registry is valid")
    }

    fn document(text: &str) -> Document {
        toml::from_str(text).expect("the fixture is TOML")
    }

    const VALID: &str = r#"
        [defaults]
        population_dataset = "population-count-2020-30arcsec"
        country_combination = "baseline"

        [combinations.baseline]
        population_source = "dataset:population-count-2020-30arcsec"
        boundary_source = "dataset:gisco-countries-2024-1m"
        default_resolution = 10

        [combinations.baseline.resolutions.10]
        local = "cache"
    "#;

    #[test]
    fn configured_world_dataset_uses_default_and_explicit_choice() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let config = Configuration::load_from_paths(
            &root.join(BUILTIN_CONFIG_PATH),
            &root.join(REGISTRY_PATH),
            None,
        )
        .expect("the committed configuration is valid");
        assert_eq!(
            config.default_population_dataset,
            "population-count-2020-10arcmin"
        );
        assert_eq!(
            config.population_dataset(None),
            "population-count-2020-10arcmin"
        );
        assert_eq!(
            config.population_dataset(Some("population-count-2020-30arcsec")),
            "population-count-2020-30arcsec"
        );
        assert_eq!(config.default_country_combination, "gpw-2020-gisco-2024");
        let (combination, resolution) = config
            .combination(None, None)
            .expect("built-in combination resolves");
        assert_eq!(combination.id, "gpw-2020-gisco-2024");
        assert_eq!(resolution, 2);
        assert_eq!(
            combination.resolutions.keys().copied().collect::<Vec<_>>(),
            [1, 2]
        );
        let (gadm, resolution) = config
            .combination(Some("gpw-2020-gadm-4.1"), None)
            .expect("GADM combination resolves");
        assert_eq!(gadm.boundary_source, "file:boundaries/gadm41-adm0.geojson");
        assert_eq!(gadm.boundary_source_id_field.as_deref(), Some("GID_0"));
        assert_eq!(gadm.resolutions.keys().copied().collect::<Vec<_>>(), [1, 2]);
        assert_eq!(resolution, 2);
    }

    #[test]
    fn builtin_pairings_reject_source_rebinding() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = root.join(BUILTIN_CONFIG_PATH);
        let registry = registry();
        let mut base = read_document(&path).expect("the committed configuration parses");
        let overlay = document(
            r#"
            [combinations."gpw-2020-gisco-2024"]
            boundary_source = "dataset:coastline-1to110m"
        "#,
        );
        assert!(merge(&mut base, overlay, Path::new("user.toml"), &registry).is_err());

        let mut base = read_document(&path).expect("the committed configuration parses");
        let overlay = document(
            r#"
            [combinations."gpw-2020-gadm-4.1"]
            boundary_source = "dataset:gisco-countries-2024-1m"
        "#,
        );
        assert!(merge(&mut base, overlay, Path::new("user.toml"), &registry).is_err());
    }

    #[test]
    fn invalid_defaults_duplicate_ids_and_undeclared_sources_are_rejected() {
        let registry = registry();
        for missing in ["population", "country combination"] {
            let mut config = document(VALID);
            if missing == "population" {
                config.defaults.population_dataset = None;
            } else {
                config.defaults.country_combination = None;
            }
            assert!(validate_complete(&config, Path::new("fixture.toml"), &registry).is_err());
        }

        let mut missing_default = document(VALID);
        missing_default
            .combinations
            .get_mut("baseline")
            .expect("row exists")
            .default_resolution = Some(5);
        assert!(validate_complete(&missing_default, Path::new("fixture.toml"), &registry).is_err());

        let mut zero_resolution = document(VALID);
        let row = zero_resolution
            .combinations
            .get_mut("baseline")
            .expect("row exists");
        row.default_resolution = Some(0);
        row.resolutions.insert(
            0,
            Location {
                local: Some(PathBuf::from("cache")),
                remote: None,
            },
        );
        assert!(validate_complete(&zero_resolution, Path::new("fixture.toml"), &registry).is_err());

        for location in [
            Location {
                local: Some(PathBuf::new()),
                remote: None,
            },
            Location {
                local: None,
                remote: Some(String::new()),
            },
        ] {
            let mut empty_location = document(VALID);
            empty_location
                .combinations
                .get_mut("baseline")
                .expect("row exists")
                .resolutions
                .insert(10, location);
            assert!(
                validate_complete(&empty_location, Path::new("fixture.toml"), &registry).is_err()
            );
        }

        let duplicate = r#"
            [combinations.same]
            population_source = "dataset:population-count-2020-30arcsec"
            [combinations.same]
            boundary_source = "dataset:gisco-countries-2024-1m"
        "#;
        assert!(toml::from_str::<Document>(duplicate).is_err());

        let mut unknown = document(VALID);
        unknown
            .combinations
            .get_mut("baseline")
            .expect("row exists")
            .population_source = Some("dataset:missing".to_owned());
        let error = validate_complete(&unknown, Path::new("fixture.toml"), &registry)
            .expect_err("unknown datasets are rejected");
        assert!(error.to_string().contains("baseline` population_source"));
    }

    #[test]
    fn user_documents_add_combinations_and_override_defaults_locations_and_resolution() {
        let registry = registry();
        let mut base = document(VALID);
        let overlay = document(
            r#"
            [defaults]
            population_dataset = "population-count-2020-30arcsec"
            country_combination = "extra"

            [combinations.baseline]
            population_source = "dataset:population-count-2020-30arcsec"
            boundary_source = "dataset:gisco-countries-2024-1m"
            default_resolution = 5

            [combinations.baseline.resolutions.5]
            local = "prepared/5"

            [combinations.extra]
            population_source = "file:population.tif"
            boundary_source = "dataset:gisco-countries-2024-1m"
            default_resolution = 1
            provenance = "local"

            [combinations.extra.population_metadata]
            width = 1
            height = 1
            origin_lat = 1.0
            origin_lon = 0.0
            lon_step = 1.0
            lat_step = -1.0
            epsg = 4326
            nodata = -9999.0

            [combinations.extra.resolutions.1]
            remote = "https://example.invalid/prepared"
        "#,
        );
        let directory = tempfile::tempdir().expect("temporary config directory");
        let overlay_path = directory.path().join("settings.toml");
        merge(&mut base, overlay, &overlay_path, &registry).expect("allowed overrides merge");
        let extra = &base.combinations["extra"];
        let expected_source = format!("file:{}", directory.path().join("population.tif").display());
        let expected_location = directory.path().join("prepared/5");
        assert_eq!(
            extra.population_source.as_deref(),
            Some(expected_source.as_str())
        );
        assert_eq!(base.defaults.country_combination.as_deref(), Some("extra"));
        assert_eq!(base.combinations["baseline"].default_resolution, Some(5));
        assert_eq!(
            base.combinations["baseline"].resolutions[&5]
                .local
                .as_deref(),
            Some(expected_location.as_path())
        );
    }

    #[test]
    fn existing_ids_cannot_rebind_either_source_independently() {
        let registry = registry();
        for overlay in [
            "[combinations.baseline]\nprovenance = \"user metadata\"",
            "[combinations.baseline]\npopulation_source = \"dataset:population-count-2020-30arcsec\"",
            "[combinations.baseline]\npopulation_source = \"dataset:population-count-2020-30arcsec\"\nboundary_source = \"dataset:coastline-1to110m\"",
            "[combinations.baseline]\npopulation_source = \"dataset:coastline-1to110m\"\nboundary_source = \"dataset:gisco-countries-2024-1m\"",
        ] {
            let mut base = document(VALID);
            let overlay = document(overlay);
            assert!(merge(&mut base, overlay, Path::new("user.toml"), &registry).is_err());
        }
    }

    const LOCAL_SOURCES: &str = r#"
        [defaults]
        population_dataset = "population-count-2020-30arcsec"
        country_combination = "local"

        [combinations.local]
        population_source = "file:population.tif"
        boundary_source = "file:boundaries.geojson"
        default_resolution = 1

        [combinations.local.population_metadata]
        width = 2
        height = 2
        origin_lat = 1.0
        origin_lon = 0.0
        lon_step = 1.0
        lat_step = -1.0
        epsg = 4326
        nodata = -9999.0

        [combinations.local.boundary_metadata]
        source_id_field = "CNTR_ID"

        [combinations.local.resolutions.1]
        local = "prepared"
    "#;

    #[test]
    fn local_sources_require_valid_population_metadata() {
        let registry = registry();
        let parsed = document(LOCAL_SOURCES);
        validate_complete(&parsed, Path::new("user.toml"), &registry)
            .expect("a complete local source pair is valid");

        for field in [
            "width",
            "height",
            "origin_lat",
            "origin_lon",
            "lon_step",
            "lat_step",
            "epsg",
            "nodata",
        ] {
            let missing = LOCAL_SOURCES
                .lines()
                .filter(|line| !line.trim_start().starts_with(&format!("{field} =")))
                .collect::<Vec<_>>()
                .join("\n");
            let error = toml::from_str::<Document>(&missing)
                .ok()
                .and_then(|parsed| {
                    validate_complete(&parsed, Path::new("user.toml"), &registry).err()
                })
                .expect("missing local population metadata is refused");
            assert!(error.to_string().contains(field), "{error}");
        }

        for (text, field) in [
            (
                LOCAL_SOURCES.replace("width = 2", "width = 0"),
                "population_metadata grid",
            ),
            (
                LOCAL_SOURCES.replace("epsg = 4326", "epsg = 0"),
                "population_metadata.epsg",
            ),
            (
                LOCAL_SOURCES.replace("nodata = -9999.0", "nodata = inf"),
                "population_metadata.nodata",
            ),
            (
                LOCAL_SOURCES.replace("source_id_field = \"CNTR_ID\"", "source_id_field = \"\""),
                "boundary_metadata.source_id_field",
            ),
        ] {
            let parsed = document(&text);
            let error = validate_complete(&parsed, Path::new("user.toml"), &registry)
                .expect_err("invalid local metadata is refused");
            assert!(error.to_string().contains(field), "{error}");
        }
    }

    #[test]
    fn local_boundary_requires_an_id_field_and_dataset_sources_reject_local_metadata() {
        let registry = registry();
        let missing_id = LOCAL_SOURCES.replace("source_id_field = \"CNTR_ID\"\n", "");
        let error = validate_complete(&document(&missing_id), Path::new("user.toml"), &registry)
            .expect_err("a local boundary must declare its identifier property");
        assert!(
            error
                .to_string()
                .contains("boundary_metadata.source_id_field"),
            "{error}"
        );

        let boundary_metadata_on_dataset =
            LOCAL_SOURCES.replace("file:boundaries.geojson", "dataset:gisco-countries-2024-1m");
        let error = validate_complete(
            &document(&boundary_metadata_on_dataset),
            Path::new("user.toml"),
            &registry,
        )
        .expect_err("metadata on a declared boundary dataset is refused");
        assert!(error.to_string().contains("boundary_metadata"));

        let population_metadata_on_dataset = LOCAL_SOURCES.replace(
            "file:population.tif",
            "dataset:population-count-2020-30arcsec",
        );
        let error = validate_complete(
            &document(&population_metadata_on_dataset),
            Path::new("user.toml"),
            &registry,
        )
        .expect_err("metadata on a declared population dataset is refused");
        assert!(error.to_string().contains("population_metadata"));
    }

    #[test]
    fn local_source_paths_must_not_be_empty() {
        let registry = registry();
        let malformed_source = LOCAL_SOURCES.replace("file:population.tif", "file:");
        let error = validate_complete(
            &document(&malformed_source),
            Path::new("user.toml"),
            &registry,
        )
        .expect_err("an empty local source path is refused");
        assert!(error.to_string().contains("population_source"));
    }

    #[test]
    fn user_population_default_can_change_to_another_registered_raster() {
        let registry = Registry::parse(
            Path::new("registry.toml"),
            r#"
                [datasets.pop-a]
                kind = "population-raster"
                path = "data/population/pop-a.tif"
                width = 1
                height = 1
                origin_lat = 1.0
                origin_lon = 0.0
                lon_step = 1.0
                lat_step = -1.0
                epsg = 4326
                nodata = -9999.0

                [datasets.pop-b]
                kind = "population-raster"
                path = "data/population/pop-b.tif"
                width = 1
                height = 1
                origin_lat = 1.0
                origin_lon = 0.0
                lon_step = 1.0
                lat_step = -1.0
                epsg = 4326
                nodata = -9999.0

                [datasets.boundaries]
                kind = "boundary-vector"
                path = "data/boundaries/boundaries.geojson"
                sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
            "#,
        )
        .expect("fixture registry is valid");
        let mut base = document(
            r#"
                [defaults]
                population_dataset = "pop-a"
                country_combination = "baseline"

                [combinations.baseline]
                population_source = "dataset:pop-a"
                boundary_source = "dataset:boundaries"
                default_resolution = 1

                [combinations.baseline.resolutions.1]
                local = "cache"
            "#,
        );
        merge(
            &mut base,
            document("[defaults]\npopulation_dataset = \"pop-b\""),
            Path::new("user.toml"),
            &registry,
        )
        .expect("registered population default override is valid");
        assert_eq!(base.defaults.population_dataset.as_deref(), Some("pop-b"));
    }

    #[cfg(unix)]
    #[test]
    fn a_file_source_rejects_a_resolved_path_that_cannot_be_serialized() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let config_path = PathBuf::from(OsString::from_vec(vec![
            b'/', b't', b'm', b'p', b'/', 0xff, b'/', b'u', b's', b'e', b'r', b'.', b't', b'o',
            b'm', b'l',
        ]));
        let error = resolve_file_source(
            &config_path,
            "local",
            "population_source",
            "file:population.tif",
        )
        .expect_err("lossy path conversion is refused");
        assert!(error.to_string().contains("local` population_source"));
    }
}
