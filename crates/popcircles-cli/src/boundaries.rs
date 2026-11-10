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
    pub name: String,
    pub iso_alpha2: Option<String>,
    pub geometry: Geometry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IsoCodeSource {
    Gisco,
    Gadm,
}

impl IsoCodeSource {
    pub(crate) fn registered_dataset(name: &str) -> Option<Self> {
        match name {
            "gisco-countries-2024-1m" => Some(Self::Gisco),
            "gadm41-adm0" => Some(Self::Gadm),
            _ => None,
        }
    }

    const fn id_field(self) -> &'static str {
        match self {
            Self::Gisco => "CNTR_ID",
            Self::Gadm => "GID_0",
        }
    }
}

/// Resolves an exact native ID, English name, or ISO alpha-2 alias.
///
/// # Errors
/// Returns a message with available candidates when the selector is unknown or ambiguous.
pub fn resolve<'a>(
    features: &'a [BoundaryFeature],
    selector: &str,
) -> Result<&'a BoundaryFeature, String> {
    let matches: Vec<_> = features
        .iter()
        .filter(|feature| {
            feature.id.as_str() == selector
                || feature.name == selector
                || feature
                    .iso_alpha2
                    .as_deref()
                    .is_some_and(|code| code.eq_ignore_ascii_case(selector))
        })
        .collect();
    match matches.as_slice() {
        [feature] => Ok(feature),
        [] => Err(format!(
            "unknown country {selector:?}; available countries: {}",
            candidates(features.iter())
        )),
        _ => Err(format!(
            "ambiguous country selector {selector:?}; matching countries: {}",
            candidates(matches.iter().copied())
        )),
    }
}

fn candidates<'a>(features: impl Iterator<Item = &'a BoundaryFeature>) -> String {
    let mut features: Vec<_> = features.collect();
    features.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    features
        .iter()
        .map(|feature| format!("{} ({})", feature.id.as_str(), feature.name))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Reads a registered boundary file without mixing file access into geometry rasterization.
///
/// # Errors
/// Returns [`BoundaryError::Read`] for filesystem errors and a format or feature error for invalid data.
pub fn load(path: &Path, source_id_field: &str) -> Result<Vec<BoundaryFeature>, BoundaryError> {
    load_with_iso_source(path, source_id_field, None)
}

pub(crate) fn load_with_iso_source(
    path: &Path,
    source_id_field: &str,
    iso_code_source: Option<IsoCodeSource>,
) -> Result<Vec<BoundaryFeature>, BoundaryError> {
    let text = fs::read_to_string(path).map_err(|source| BoundaryError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    decode_with_iso_source(&text, source_id_field, iso_code_source)
}

/// Decodes `GeoJSON` polygon features while retaining every source identity and coordinate.
///
/// # Errors
/// Returns a format error for malformed JSON or a feature error naming the identifier (or source
/// index when no identifier exists) for invalid feature structure, identity, geometry or coordinates.
pub fn decode(text: &str, source_id_field: &str) -> Result<Vec<BoundaryFeature>, BoundaryError> {
    decode_with_iso_source(text, source_id_field, None)
}

pub(crate) fn decode_with_iso_source(
    text: &str,
    source_id_field: &str,
    iso_code_source: Option<IsoCodeSource>,
) -> Result<Vec<BoundaryFeature>, BoundaryError> {
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
            let name = properties
                .get("NAME_ENGL")
                .or_else(|| properties.get("CNTR_NAME"))
                .or_else(|| properties.get("COUNTRY"))
                .and_then(Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .unwrap_or(id.as_str())
                .to_owned();
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
            let iso_alpha2 = iso_code_source
                .filter(|source| source.id_field() == source_id_field)
                .and_then(|source| iso_alpha2(source, id.as_str(), properties));
            Ok(BoundaryFeature {
                id,
                name,
                iso_alpha2,
                geometry,
            })
        })
        .collect()
}

fn iso_alpha2(
    source: IsoCodeSource,
    source_id: &str,
    properties: &serde_json::Map<String, Value>,
) -> Option<String> {
    let code = properties
        .get("ISO_A2")
        .and_then(Value::as_str)
        .filter(|value| value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_alphabetic()))
        .map(str::to_ascii_uppercase)
        .or_else(|| match source {
            IsoCodeSource::Gisco => Some(
                match source_id {
                    "EL" => "GR",
                    "UK" => "GB",
                    _ => source_id,
                }
                .to_owned(),
            ),
            IsoCodeSource::Gadm => iso_alpha3_to_alpha2(source_id).map(str::to_owned),
        })?;
    normalize_iso_alpha2(&code)
}

pub(crate) fn normalize_iso_alpha2(code: &str) -> Option<String> {
    let code = code.to_ascii_uppercase();
    ISO_ALPHA3_TO_ALPHA2
        .iter()
        .any(|(_, alpha2)| *alpha2 == code)
        .then_some(code)
}

fn iso_alpha3_to_alpha2(code: &str) -> Option<&'static str> {
    ISO_ALPHA3_TO_ALPHA2
        .iter()
        .find_map(|(alpha3, alpha2)| (*alpha3 == code).then_some(*alpha2))
}

const ISO_ALPHA3_TO_ALPHA2: &[(&str, &str)] = &[
    ("ABW", "AW"),
    ("AFG", "AF"),
    ("AGO", "AO"),
    ("AIA", "AI"),
    ("ALA", "AX"),
    ("ALB", "AL"),
    ("AND", "AD"),
    ("ARE", "AE"),
    ("ARG", "AR"),
    ("ARM", "AM"),
    ("ASM", "AS"),
    ("ATA", "AQ"),
    ("ATF", "TF"),
    ("ATG", "AG"),
    ("AUS", "AU"),
    ("AUT", "AT"),
    ("AZE", "AZ"),
    ("BDI", "BI"),
    ("BEL", "BE"),
    ("BEN", "BJ"),
    ("BES", "BQ"),
    ("BFA", "BF"),
    ("BGD", "BD"),
    ("BGR", "BG"),
    ("BHR", "BH"),
    ("BHS", "BS"),
    ("BIH", "BA"),
    ("BLM", "BL"),
    ("BLR", "BY"),
    ("BLZ", "BZ"),
    ("BMU", "BM"),
    ("BOL", "BO"),
    ("BRA", "BR"),
    ("BRB", "BB"),
    ("BRN", "BN"),
    ("BTN", "BT"),
    ("BVT", "BV"),
    ("BWA", "BW"),
    ("CAF", "CF"),
    ("CAN", "CA"),
    ("CCK", "CC"),
    ("CHE", "CH"),
    ("CHL", "CL"),
    ("CHN", "CN"),
    ("CIV", "CI"),
    ("CMR", "CM"),
    ("COD", "CD"),
    ("COG", "CG"),
    ("COK", "CK"),
    ("COL", "CO"),
    ("COM", "KM"),
    ("CPV", "CV"),
    ("CRI", "CR"),
    ("CUB", "CU"),
    ("CUW", "CW"),
    ("CXR", "CX"),
    ("CYM", "KY"),
    ("CYP", "CY"),
    ("CZE", "CZ"),
    ("DEU", "DE"),
    ("DJI", "DJ"),
    ("DMA", "DM"),
    ("DNK", "DK"),
    ("DOM", "DO"),
    ("DZA", "DZ"),
    ("ECU", "EC"),
    ("EGY", "EG"),
    ("ERI", "ER"),
    ("ESH", "EH"),
    ("ESP", "ES"),
    ("EST", "EE"),
    ("ETH", "ET"),
    ("FIN", "FI"),
    ("FJI", "FJ"),
    ("FLK", "FK"),
    ("FRA", "FR"),
    ("FRO", "FO"),
    ("FSM", "FM"),
    ("GAB", "GA"),
    ("GBR", "GB"),
    ("GEO", "GE"),
    ("GGY", "GG"),
    ("GHA", "GH"),
    ("GIB", "GI"),
    ("GIN", "GN"),
    ("GLP", "GP"),
    ("GMB", "GM"),
    ("GNB", "GW"),
    ("GNQ", "GQ"),
    ("GRC", "GR"),
    ("GRD", "GD"),
    ("GRL", "GL"),
    ("GTM", "GT"),
    ("GUF", "GF"),
    ("GUM", "GU"),
    ("GUY", "GY"),
    ("HKG", "HK"),
    ("HMD", "HM"),
    ("HND", "HN"),
    ("HRV", "HR"),
    ("HTI", "HT"),
    ("HUN", "HU"),
    ("IDN", "ID"),
    ("IMN", "IM"),
    ("IND", "IN"),
    ("IOT", "IO"),
    ("IRL", "IE"),
    ("IRN", "IR"),
    ("IRQ", "IQ"),
    ("ISL", "IS"),
    ("ISR", "IL"),
    ("ITA", "IT"),
    ("JAM", "JM"),
    ("JEY", "JE"),
    ("JOR", "JO"),
    ("JPN", "JP"),
    ("KAZ", "KZ"),
    ("KEN", "KE"),
    ("KGZ", "KG"),
    ("KHM", "KH"),
    ("KIR", "KI"),
    ("KNA", "KN"),
    ("KOR", "KR"),
    ("KWT", "KW"),
    ("LAO", "LA"),
    ("LBN", "LB"),
    ("LBR", "LR"),
    ("LBY", "LY"),
    ("LCA", "LC"),
    ("LIE", "LI"),
    ("LKA", "LK"),
    ("LSO", "LS"),
    ("LTU", "LT"),
    ("LUX", "LU"),
    ("LVA", "LV"),
    ("MAC", "MO"),
    ("MAF", "MF"),
    ("MAR", "MA"),
    ("MCO", "MC"),
    ("MDA", "MD"),
    ("MDG", "MG"),
    ("MDV", "MV"),
    ("MEX", "MX"),
    ("MHL", "MH"),
    ("MKD", "MK"),
    ("MLI", "ML"),
    ("MLT", "MT"),
    ("MMR", "MM"),
    ("MNE", "ME"),
    ("MNG", "MN"),
    ("MNP", "MP"),
    ("MOZ", "MZ"),
    ("MRT", "MR"),
    ("MSR", "MS"),
    ("MTQ", "MQ"),
    ("MUS", "MU"),
    ("MWI", "MW"),
    ("MYS", "MY"),
    ("MYT", "YT"),
    ("NAM", "NA"),
    ("NCL", "NC"),
    ("NER", "NE"),
    ("NFK", "NF"),
    ("NGA", "NG"),
    ("NIC", "NI"),
    ("NIU", "NU"),
    ("NLD", "NL"),
    ("NOR", "NO"),
    ("NPL", "NP"),
    ("NRU", "NR"),
    ("NZL", "NZ"),
    ("OMN", "OM"),
    ("PAK", "PK"),
    ("PAN", "PA"),
    ("PCN", "PN"),
    ("PER", "PE"),
    ("PHL", "PH"),
    ("PLW", "PW"),
    ("PNG", "PG"),
    ("POL", "PL"),
    ("PRI", "PR"),
    ("PRK", "KP"),
    ("PRT", "PT"),
    ("PRY", "PY"),
    ("PSE", "PS"),
    ("PYF", "PF"),
    ("QAT", "QA"),
    ("REU", "RE"),
    ("ROU", "RO"),
    ("RUS", "RU"),
    ("RWA", "RW"),
    ("SAU", "SA"),
    ("SDN", "SD"),
    ("SEN", "SN"),
    ("SGP", "SG"),
    ("SGS", "GS"),
    ("SHN", "SH"),
    ("SJM", "SJ"),
    ("SLB", "SB"),
    ("SLE", "SL"),
    ("SLV", "SV"),
    ("SMR", "SM"),
    ("SOM", "SO"),
    ("SPM", "PM"),
    ("SRB", "RS"),
    ("SSD", "SS"),
    ("STP", "ST"),
    ("SUR", "SR"),
    ("SVK", "SK"),
    ("SVN", "SI"),
    ("SWE", "SE"),
    ("SWZ", "SZ"),
    ("SXM", "SX"),
    ("SYC", "SC"),
    ("SYR", "SY"),
    ("TCA", "TC"),
    ("TCD", "TD"),
    ("TGO", "TG"),
    ("THA", "TH"),
    ("TJK", "TJ"),
    ("TKL", "TK"),
    ("TKM", "TM"),
    ("TLS", "TL"),
    ("TON", "TO"),
    ("TTO", "TT"),
    ("TUN", "TN"),
    ("TUR", "TR"),
    ("TUV", "TV"),
    ("TWN", "TW"),
    ("TZA", "TZ"),
    ("UGA", "UG"),
    ("UKR", "UA"),
    ("UMI", "UM"),
    ("URY", "UY"),
    ("USA", "US"),
    ("UZB", "UZ"),
    ("VAT", "VA"),
    ("VCT", "VC"),
    ("VEN", "VE"),
    ("VGB", "VG"),
    ("VIR", "VI"),
    ("VNM", "VN"),
    ("VUT", "VU"),
    ("WLF", "WF"),
    ("WSM", "WS"),
    ("YEM", "YE"),
    ("ZAF", "ZA"),
    ("ZMB", "ZM"),
    ("ZWE", "ZW"),
];

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
#[allow(clippy::expect_used)] // Synthetic country fixtures and focused failures are clearer with assertion messages.
mod tests {
    use super::*;

    const VALID: &str = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"CNTR_ID":"AA","name":"ignored"},"geometry":{"type":"MultiPolygon","coordinates":[[[[0,0],[2,0],[2,2],[0,0]]]]}}]}"#;

    fn countries(entries: &[(&str, &str)]) -> Vec<BoundaryFeature> {
        let features: Vec<_> = entries
            .iter()
            .map(|(id, name)| {
                serde_json::json!({
                    "type": "Feature",
                    "properties": {"CNTR_ID": id, "NAME_ENGL": name},
                    "geometry": {"type": "Polygon", "coordinates": []},
                })
            })
            .collect();
        decode(
            &serde_json::json!({"type": "FeatureCollection", "features": features}).to_string(),
            "CNTR_ID",
        )
        .expect("synthetic features decode")
    }

    #[test]
    fn country_selector_rejects_a_native_id_and_name_collision() {
        let features = countries(&[("AA", "Alpha"), ("BB", "AA")]);

        let error = resolve(&features, "AA").expect_err("colliding aliases are ambiguous");
        assert!(error.contains("AA (Alpha), BB (AA)"), "{error}");
    }

    #[test]
    fn country_selector_resolves_a_unique_exact_name() {
        let features = countries(&[("AA", "Alpha"), ("BB", "Beta")]);

        assert_eq!(
            resolve(&features, "Beta").map(|feature| feature.id.as_str()),
            Ok("BB")
        );
    }

    #[test]
    fn country_selector_resolves_native_id_name_and_case_insensitive_iso_code() {
        let document = serde_json::json!({
            "type": "FeatureCollection",
            "features": [{"type": "Feature", "properties": {"GID_0": "FRA", "COUNTRY": "France"}, "geometry": {"type": "Polygon", "coordinates": []}}],
        });
        let features =
            decode_with_iso_source(&document.to_string(), "GID_0", Some(IsoCodeSource::Gadm))
                .expect("fixture decodes");

        for selector in ["FRA", "France", "FR", "fr"] {
            assert_eq!(
                resolve(&features, selector).map(|feature| feature.id.as_str()),
                Ok("FRA"),
                "selector {selector}"
            );
        }
    }

    #[test]
    fn country_selector_reports_a_code_and_native_id_collision() {
        let gadm = serde_json::json!({
            "type": "FeatureCollection",
            "features": [{"type": "Feature", "properties": {"GID_0": "FRA", "COUNTRY": "France"}, "geometry": {"type": "Polygon", "coordinates": []}}],
        });
        let gisco = serde_json::json!({
            "type": "FeatureCollection",
            "features": [{"type": "Feature", "properties": {"CNTR_ID": "FR", "NAME_ENGL": "Other"}, "geometry": {"type": "Polygon", "coordinates": []}}],
        });
        let mut features =
            decode_with_iso_source(&gadm.to_string(), "GID_0", Some(IsoCodeSource::Gadm))
                .expect("GADM fixture decodes");
        features.extend(
            decode_with_iso_source(&gisco.to_string(), "CNTR_ID", Some(IsoCodeSource::Gisco))
                .expect("GISCO fixture decodes"),
        );

        let error = resolve(&features, "FR").expect_err("different features match");
        assert!(error.contains("FR (Other), FRA (France)"), "{error}");
    }

    #[test]
    fn country_selector_lists_sorted_candidates_for_unknown_names() {
        let features = countries(&[("BB", "Beta"), ("AA", "Alpha")]);

        let error = resolve(&features, "Missing").expect_err("unknown name fails");
        assert!(error.contains("AA (Alpha), BB (Beta)"), "{error}");
    }

    #[test]
    fn country_selector_lists_duplicate_name_matches() {
        let features = countries(&[("BB", "Same"), ("AA", "Same"), ("CC", "Other")]);

        let error = resolve(&features, "Same").expect_err("duplicate name is ambiguous");
        assert!(error.contains("AA (Same), BB (Same)"), "{error}");
        assert!(!error.contains("CC"), "{error}");
    }

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
    fn decodes_gadm_country_name() {
        let document = serde_json::json!({
            "type": "FeatureCollection",
            "features": [{
                "type": "Feature",
                "properties": {"GID_0": "AND", "COUNTRY": "Andorra"},
                "geometry": {"type": "Polygon", "coordinates": []},
            }],
        });

        let features = decode(&document.to_string(), "GID_0").expect("GADM feature decodes");

        assert_eq!(features[0].name, "Andorra");
    }

    #[test]
    fn gisco_and_gadm_assign_the_same_alpha2_code() {
        let gisco = serde_json::json!({
            "type": "FeatureCollection",
            "features": [{"type": "Feature", "properties": {"CNTR_ID": "FR", "NAME_ENGL": "France"}, "geometry": {"type": "Polygon", "coordinates": []}}],
        });
        let gadm = serde_json::json!({
            "type": "FeatureCollection",
            "features": [{"type": "Feature", "properties": {"GID_0": "FRA", "COUNTRY": "France"}, "geometry": {"type": "Polygon", "coordinates": []}}],
        });

        let gisco =
            decode_with_iso_source(&gisco.to_string(), "CNTR_ID", Some(IsoCodeSource::Gisco))
                .expect("GISCO fixture decodes");
        let gadm = decode_with_iso_source(&gadm.to_string(), "GID_0", Some(IsoCodeSource::Gadm))
            .expect("GADM fixture decodes");

        assert_eq!(gisco[0].iso_alpha2.as_deref(), Some("FR"));
        assert_eq!(gadm[0].iso_alpha2.as_deref(), Some("FR"));
    }

    #[test]
    fn gisco_source_exception_maps_greece_to_its_iso_alpha2_code() {
        let features = decode_with_iso_source(
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"CNTR_ID":"EL","NAME_ENGL":"Greece"},"geometry":{"type":"Polygon","coordinates":[]}}]}"#,
            "CNTR_ID",
            Some(IsoCodeSource::Gisco),
        )
        .expect("GISCO fixture decodes");

        assert_eq!(features[0].iso_alpha2.as_deref(), Some("GR"));
        assert_eq!(features[0].id.as_str(), "EL");
    }

    #[test]
    fn unknown_gadm_code_keeps_native_identity_without_an_alias() {
        let features = decode_with_iso_source(
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"GID_0":"XKS","COUNTRY":"Kosovo"},"geometry":{"type":"Polygon","coordinates":[]}}]}"#,
            "GID_0",
            Some(IsoCodeSource::Gadm),
        )
        .expect("unmapped feature remains valid");

        assert_eq!(features[0].id.as_str(), "XKS");
        assert_eq!(features[0].name, "Kosovo");
        assert_eq!(features[0].iso_alpha2, None);
    }

    #[test]
    fn a_custom_source_with_a_registered_field_name_gets_no_alias() {
        let features = decode(
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"CNTR_ID":"FR","NAME_ENGL":"Custom region"},"geometry":{"type":"Polygon","coordinates":[]}}]}"#,
            "CNTR_ID",
        )
        .expect("custom feature remains valid");

        assert_eq!(features[0].id.as_str(), "FR");
        assert_eq!(features[0].name, "Custom region");
        assert_eq!(features[0].iso_alpha2, None);
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
