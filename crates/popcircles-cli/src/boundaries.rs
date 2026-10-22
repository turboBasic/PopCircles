//! `GeoJSON` boundary decoding in the CLI layer, outside the domain rasterizer.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use popcircles::domain::{FeatureId, Geometry, Polygon};
use popcircles::geodesy::LatLon;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum BoundaryError {
    #[error("boundary input at {} could not be read", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("boundary input is not a GeoJSON FeatureCollection")]
    Format(#[source] serde_json::Error),

    #[error("boundary feature {feature} is malformed: {reason}")]
    Feature { feature: String, reason: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoundaryFeature {
    pub id: FeatureId,
    pub geometry: Geometry,
}

/// Reads a registered boundary file without mixing file access into geometry rasterization.
///
/// # Errors
/// Returns [`BoundaryError::Read`] for filesystem errors and a format or feature error for invalid data.
pub fn load(path: &Path, source_id_field: &str) -> Result<Vec<BoundaryFeature>, BoundaryError> {
    let text = fs::read_to_string(path).map_err(|source| BoundaryError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    decode(&text, source_id_field)
}

/// Decodes `GeoJSON` polygon features while retaining every source identity and coordinate.
///
/// # Errors
/// Returns a format error for malformed JSON or a feature error naming the identifier (or source
/// index when no identifier exists) for invalid feature structure, identity, geometry or coordinates.
pub fn decode(text: &str, source_id_field: &str) -> Result<Vec<BoundaryFeature>, BoundaryError> {
    let document: Value = serde_json::from_str(text).map_err(BoundaryError::Format)?;
    let root = document
        .as_object()
        .filter(|object| object.get("type").and_then(Value::as_str) == Some("FeatureCollection"))
        .ok_or_else(|| feature_error("0", "root type must be FeatureCollection"))?;
    let features = root
        .get("features")
        .and_then(Value::as_array)
        .ok_or_else(|| feature_error("0", "features must be an array"))?;

    let mut ids = BTreeSet::new();
    features
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let fallback = format!("index {index}");
            let object = value
                .as_object()
                .filter(|object| object.get("type").and_then(Value::as_str) == Some("Feature"))
                .ok_or_else(|| feature_error(&fallback, "type must be Feature"))?;
            let properties = object
                .get("properties")
                .and_then(Value::as_object)
                .ok_or_else(|| feature_error(&fallback, "properties must be an object"))?;
            let raw_id = properties
                .get(source_id_field)
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    feature_error(
                        &fallback,
                        &format!("properties must contain string {source_id_field}"),
                    )
                })?;
            let id = FeatureId::new(raw_id).map_err(|error| {
                feature_error(raw_id, &format!("invalid source identifier: {error}"))
            })?;
            if !ids.insert(id.as_str().to_owned()) {
                return Err(feature_error(id.as_str(), "duplicate source identifier"));
            }
            let geometry_value = object
                .get("geometry")
                .and_then(Value::as_object)
                .ok_or_else(|| feature_error(id.as_str(), "geometry must be an object"))?;
            let kind = geometry_value
                .get("type")
                .and_then(Value::as_str)
                .ok_or_else(|| feature_error(id.as_str(), "geometry type must be a string"))?;
            let coordinates = geometry_value
                .get("coordinates")
                .ok_or_else(|| feature_error(id.as_str(), "geometry must contain coordinates"))?;
            let geometry = match kind {
                "Polygon" => Geometry::Polygon(parse_polygon(coordinates, id.as_str())?),
                "MultiPolygon" => {
                    let components = coordinates.as_array().ok_or_else(|| {
                        feature_error(id.as_str(), "MultiPolygon coordinates must be an array")
                    })?;
                    let polygons = components
                        .iter()
                        .map(|component| parse_polygon(component, id.as_str()))
                        .collect::<Result<Vec<_>, _>>()?;
                    Geometry::MultiPolygon(polygons)
                }
                other => {
                    return Err(feature_error(
                        id.as_str(),
                        &format!("unsupported geometry type {other}"),
                    ));
                }
            };
            Ok(BoundaryFeature { id, geometry })
        })
        .collect()
}

fn parse_polygon(value: &Value, feature: &str) -> Result<Polygon, BoundaryError> {
    let rings = value
        .as_array()
        .ok_or_else(|| feature_error(feature, "polygon coordinates must be an array of rings"))?;
    let rings = rings
        .iter()
        .enumerate()
        .map(|(ring_index, ring)| {
            let points = ring.as_array().ok_or_else(|| {
                feature_error(feature, &format!("ring {ring_index} must be an array"))
            })?;
            points
                .iter()
                .enumerate()
                .map(|(vertex_index, point)| {
                    let ordinates = point.as_array().ok_or_else(|| {
                        feature_error(
                            feature,
                            &format!("ring {ring_index} vertex {vertex_index} must be an array"),
                        )
                    })?;
                    let [longitude, latitude] = ordinates.as_slice() else {
                        return Err(feature_error(
                            feature,
                            &format!(
                                "ring {ring_index} vertex {vertex_index} must contain longitude and latitude"
                            ),
                        ));
                    };
                    let longitude = longitude.as_f64().ok_or_else(|| {
                        feature_error(
                            feature,
                            &format!("ring {ring_index} vertex {vertex_index} longitude is not numeric"),
                        )
                    })?;
                    let latitude = latitude.as_f64().ok_or_else(|| {
                        feature_error(
                            feature,
                            &format!("ring {ring_index} vertex {vertex_index} latitude is not numeric"),
                        )
                    })?;
                    if !longitude.is_finite() || !latitude.is_finite() {
                        return Err(feature_error(
                            feature,
                            &format!("ring {ring_index} vertex {vertex_index} is not finite"),
                        ));
                    }
                    Ok(LatLon {
                        lat: latitude,
                        lon: longitude,
                    })
                })
                .collect()
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Polygon { rings })
}

fn feature_error(feature: &str, reason: &str) -> BoundaryError {
    BoundaryError::Feature {
        feature: feature.to_owned(),
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod audit;

#[cfg(test)]
mod cache;

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"CNTR_ID":"AA","name":"ignored"},"geometry":{"type":"MultiPolygon","coordinates":[[[[0,0],[2,0],[2,2],[0,0]]]]}}]}"#;

    #[test]
    #[allow(clippy::expect_used)] // A focused parser test reports invalid fixtures at the assertion.
    fn decodes_polygon_geometry_and_keeps_source_id_and_coordinates() {
        let features = decode(VALID, "CNTR_ID").expect("valid collection decodes");
        assert_eq!(features.len(), 1);
        assert_eq!(features[0].id.as_str(), "AA");
        let Geometry::MultiPolygon(polygons) = &features[0].geometry else {
            panic!("source MultiPolygon is retained");
        };
        assert_eq!(polygons[0].rings[0][1], LatLon { lat: 0.0, lon: 2.0 });

        let rounded = VALID.replace("[0,0]", "[180.0000000000001,-90]");
        let features = decode(&rounded, "CNTR_ID").expect("finite source rounding is retained");
        let Geometry::MultiPolygon(polygons) = &features[0].geometry else {
            panic!("source MultiPolygon is retained");
        };
        assert_eq!(polygons[0].rings[0][0].lon, 180.000_000_000_000_1);
    }

    #[test]
    #[allow(clippy::expect_used)] // A focused parser test reports invalid fixtures at the assertion.
    fn malformed_identity_and_geometry_errors_name_the_feature_when_available() {
        for text in [
            VALID.replace("\"CNTR_ID\":\"AA\",", ""),
            VALID.replace("\"AA\"", "\" \""),
            VALID.replace("MultiPolygon", "Point"),
            VALID.replace("[2,0]", "[\"east\",0]"),
            VALID.replace("[2,0]", "[2]"),
        ] {
            let error = decode(&text, "CNTR_ID").expect_err("malformed feature is rejected");
            assert!(error.to_string().contains("feature"), "{error}");
        }
        let duplicate = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"CNTR_ID":"AA"},"geometry":{"type":"Polygon","coordinates":[]}},{"type":"Feature","properties":{"CNTR_ID":"AA"},"geometry":{"type":"Polygon","coordinates":[]}}]}"#;
        let error = decode(duplicate, "CNTR_ID").expect_err("duplicate identity is rejected");
        assert!(error.to_string().contains("AA"), "{error}");
    }

    #[test]
    #[ignore = "reads the registered GISCO source, available only after mise run data:get"]
    #[allow(clippy::expect_used)] // The audit is an explicit offline check against the registered source.
    fn gisco_registry_source_audit_preserves_all_feature_identities() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry_path = root.join(crate::registry::REGISTRY_PATH);
        let registry = crate::registry::Registry::load(&registry_path)
            .expect("the committed dataset registry parses");
        let boundary = registry
            .boundary("gisco-countries-2024-1m")
            .expect("GISCO row is a registered boundary vector");
        let path = root.join(&boundary.path);
        let features = load(
            &path,
            boundary
                .source_id_field
                .as_deref()
                .expect("source ID field"),
        )
        .expect("registered GISCO GeoJSON decodes");
        let ids: BTreeSet<_> = features.iter().map(|feature| feature.id.as_str()).collect();
        let invalid = ["CR", "DE", "ET", "GN", "UA", "IN", "LR", "MN", "NI"];
        assert_eq!(features.len(), 263);
        assert_eq!(ids.len(), 263);
        for id in invalid {
            assert!(
                ids.contains(id),
                "invalid source feature {id} remains represented"
            );
        }
        eprintln!(
            "GISCO source audit: {} features, {} unique IDs; invalid-ring identities retained: {}",
            features.len(),
            ids.len(),
            invalid.join(", ")
        );
    }
}
