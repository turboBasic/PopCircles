//! Narrow `GeoJSON` boundary for Natural Earth populated places.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use popcircles::geodesy::LatLon;
use popcircles::settlement::Place;
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub(crate) enum SettlementsError {
    #[error("settlement input at {} could not be read; run `mise run data:get`", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("settlement input is not a GeoJSON FeatureCollection")]
    Format(#[source] serde_json::Error),

    #[error("settlement feature {index} is malformed: {reason}")]
    Feature { index: usize, reason: String },

    #[error("settlement feature collection contains no places")]
    Empty,
}

#[derive(Deserialize)]
struct FeatureCollection<'a> {
    #[serde(rename = "type", borrow)]
    kind: &'a str,
    features: Vec<Feature<'a>>,
}

#[derive(Deserialize)]
struct Feature<'a> {
    #[serde(rename = "type", borrow)]
    kind: &'a str,
    geometry: Geometry<'a>,
    properties: Properties<'a>,
}

#[derive(Deserialize)]
struct Geometry<'a> {
    #[serde(rename = "type", borrow)]
    kind: &'a str,
    coordinates: Vec<f64>,
}

#[derive(Deserialize)]
struct Properties<'a> {
    #[serde(rename = "NAME", borrow)]
    name: Option<&'a str>,
    #[serde(rename = "ADM0NAME", borrow)]
    country: Option<&'a str>,
}

/// Reads a `GeoJSON` file and decodes only populated-place identity and Point coordinates.
///
/// # Errors
/// Returns [`SettlementsError::Read`] for filesystem errors and a format/feature error for invalid data.
pub(crate) fn load(path: &Path) -> Result<Vec<Place>, SettlementsError> {
    let text = fs::read_to_string(path).map_err(|source| SettlementsError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    decode(&text)
}

/// Decodes the small `GeoJSON` surface this reader needs; unrelated Natural Earth properties are ignored.
///
/// # Errors
/// Returns a format error for malformed JSON, wrong `GeoJSON` types, missing text or invalid coordinates.
pub(crate) fn decode(text: &str) -> Result<Vec<Place>, SettlementsError> {
    let collection: FeatureCollection<'_> =
        serde_json::from_str(text).map_err(SettlementsError::Format)?;
    if collection.kind != "FeatureCollection" {
        return Err(feature_error(0, "root type must be FeatureCollection"));
    }
    if collection.features.is_empty() {
        return Err(SettlementsError::Empty);
    }

    collection
        .features
        .into_iter()
        .enumerate()
        .map(|(index, feature)| {
            if feature.kind != "Feature" {
                return Err(feature_error(index, "feature type must be Feature"));
            }
            if feature.geometry.kind != "Point" {
                return Err(feature_error(index, "geometry type must be Point"));
            }
            let [lon, lat] = feature.geometry.coordinates.as_slice() else {
                return Err(feature_error(
                    index,
                    "Point coordinates must contain longitude and latitude",
                ));
            };
            let name = feature
                .properties
                .name
                .ok_or_else(|| feature_error(index, "properties must contain NAME"))?;
            let country = feature
                .properties
                .country
                .ok_or_else(|| feature_error(index, "properties must contain ADM0NAME"))?;

            Place::new(
                name,
                country,
                LatLon {
                    lat: *lat,
                    lon: *lon,
                },
            )
            .map_err(|error| feature_error(index, &error.to_string()))
        })
        .collect()
}

fn feature_error(index: usize, reason: &str) -> SettlementsError {
    SettlementsError::Feature {
        index,
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const VALID: &str = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"NAME":"Example","ADM0NAME":"Example","irrelevant":{"field":true}},"geometry":{"type":"Point","coordinates":[190,45]}}]}"#;

    #[test]
    fn decodes_only_the_required_properties_and_wraps_longitude() {
        let places = decode(VALID).unwrap();

        assert_eq!(places.len(), 1);
        assert_eq!(places[0].name(), "Example");
        assert_eq!(places[0].country(), "Example");
        assert_eq!(
            places[0].location(),
            LatLon {
                lat: 45.0,
                lon: -170.0
            }
        );
    }

    #[test]
    fn refuses_wrong_geometry_missing_properties_and_invalid_coordinates() {
        for text in [
            VALID.replace("\"Point\"", "\"Polygon\""),
            VALID.replace("\"NAME\":\"Example\",", ""),
            VALID.replace("\"ADM0NAME\":\"Example\",", ""),
            VALID.replace("[190,45]", "[190,91]"),
        ] {
            assert!(matches!(
                decode(&text),
                Err(SettlementsError::Feature { .. })
            ));
        }
    }

    #[test]
    fn refuses_a_non_collection_and_an_empty_collection() {
        assert!(matches!(
            decode(r#"{"type":"Feature","features":[]}"#),
            Err(SettlementsError::Feature { .. })
        ));
        assert!(matches!(
            decode(r#"{"type":"FeatureCollection","features":[]}"#),
            Err(SettlementsError::Empty)
        ));
    }
}
