use std::path::Path;

use popcircles::report::{CountryListDocument, Envelope};

use crate::boundaries;
use crate::commands::serialised;
use crate::failure::{EXIT_MISSING_DATA, Failure};
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
