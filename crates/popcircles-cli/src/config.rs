use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

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
    pub(crate) provenance: Option<String>,
    pub(crate) attribution: Option<String>,
}

#[derive(Debug)]
pub(crate) struct Configuration {
    pub(crate) default_population_dataset: String,
    pub(crate) default_country_combination: String,
    pub(crate) combinations: BTreeMap<String, Combination>,
}

impl Configuration {
    pub(crate) fn load(user_path: Option<&Path>) -> Result<Self, ConfigError> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let builtin_path = root.join(BUILTIN_CONFIG_PATH);
        let registry_path = root.join(REGISTRY_PATH);
        let registry = Registry::load(&registry_path).map_err(|error| ConfigError::Invalid {
            path: registry_path.clone(),
            message: error.to_string(),
        })?;
        let mut base = read_document(&builtin_path)?;
        validate_complete(&base, &builtin_path, &registry)?;
        if let Some(path) = user_path {
            let overlay = read_document(path)?;
            merge(&mut base, overlay, path, &registry)?;
        }
        let source_path = user_path.unwrap_or(&builtin_path);
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
        let default = row.default_resolution.ok_or_else(|| {
            invalid(
                path,
                format!("combination `{id}` is missing default_resolution"),
            )
        })?;
        if row.resolutions.is_empty() || !row.resolutions.contains_key(&default) {
            return Err(invalid(
                path,
                format!("combination `{id}` default_resolution {default} is unavailable"),
            ));
        }
        if row
            .resolutions
            .iter()
            .any(|(_, location)| location.local.is_none() && location.remote.is_none())
        {
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
                if let Some(file) = source.strip_prefix("file:") {
                    let resolved = resolve(overlay_path, Path::new(file));
                    if field == "population_source" {
                        user.population_source = Some(format!("file:{}", resolved.display()));
                    } else {
                        user.boundary_source = Some(format!("file:{}", resolved.display()));
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
    fn committed_configuration_loads_and_keeps_the_explicit_population_choice() {
        let config = Configuration::load(None).expect("the committed configuration is valid");
        assert_eq!(
            config.default_population_dataset,
            "population-count-2020-30arcsec"
        );
        assert_eq!(
            config.population_dataset(None),
            "population-count-2020-30arcsec"
        );
        assert_eq!(config.population_dataset(Some("other")), "other");
        assert_eq!(config.default_country_combination, "gpw-2020-gisco-2024");
        let (combination, resolution) = config
            .combination(None, None)
            .expect("built-in combination resolves");
        assert_eq!(combination.id, "gpw-2020-gisco-2024");
        assert_eq!(resolution, 10);
    }

    #[test]
    fn invalid_defaults_duplicate_ids_and_undeclared_sources_are_rejected() {
        let registry = registry();
        let mut missing_default = document(VALID);
        missing_default
            .combinations
            .get_mut("baseline")
            .expect("row exists")
            .default_resolution = Some(5);
        assert!(validate_complete(&missing_default, Path::new("fixture.toml"), &registry).is_err());

        let duplicate = r#"
            [combinations.same]
            value = 1
            [combinations.same]
            value = 2
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
            "[combinations.baseline]\npopulation_source = \"dataset:population-count-2020-30arcsec\"",
            "[combinations.baseline]\npopulation_source = \"dataset:population-count-2020-30arcsec\"\nboundary_source = \"dataset:coastline-1to110m\"",
            "[combinations.baseline]\npopulation_source = \"dataset:coastline-1to110m\"\nboundary_source = \"dataset:gisco-countries-2024-1m\"",
        ] {
            let mut base = document(VALID);
            let overlay = document(overlay);
            assert!(merge(&mut base, overlay, Path::new("user.toml"), &registry).is_err());
        }
    }
}
