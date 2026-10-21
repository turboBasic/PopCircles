use std::cmp::Ordering;

use crate::geodesy::{LatLon, great_circle_km, wrap_lon};

const SHORT_LABEL_KM: f64 = 10.0;
const MAX_LABEL_KM: f64 = 500.0;

/// A populated place with a validated name, country and on-sphere coordinate.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    name: String,
    country: String,
    location: LatLon,
}

impl Place {
    /// Creates a place, wrapping its finite longitude into the geodesy module's half-open range.
    ///
    /// # Errors
    /// Returns [`SettlementError`] when either name is empty or the coordinate is invalid.
    pub fn new(
        name: impl Into<String>,
        country: impl Into<String>,
        location: LatLon,
    ) -> Result<Self, SettlementError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(SettlementError::EmptyName);
        }
        let country = country.into();
        if country.trim().is_empty() {
            return Err(SettlementError::EmptyCountry);
        }

        Ok(Self {
            name,
            country,
            location: validate_location(location)?,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn country(&self) -> &str {
        &self.country
    }

    #[must_use]
    pub const fn location(&self) -> LatLon {
        self.location
    }
}

/// The selected place and the unrounded great-circle distance to it.
#[derive(Debug, Clone, PartialEq)]
pub struct Context {
    name: String,
    country: String,
    distance_km: f64,
    short_form: bool,
}

impl Context {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn country(&self) -> &str {
        &self.country
    }

    #[must_use]
    pub const fn distance_km(&self) -> f64 {
        self.distance_km
    }

    #[must_use]
    pub const fn short_form(&self) -> bool {
        self.short_form
    }
}

#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum SettlementError {
    #[error("a settlement name must not be empty")]
    EmptyName,

    #[error("a settlement country must not be empty")]
    EmptyCountry,

    #[error("a settlement latitude must be finite and within [-90, 90]; {lat} is not")]
    InvalidLatitude { lat: f64 },

    #[error("a settlement longitude must be finite; {lon} is not")]
    InvalidLongitude { lon: f64 },

    #[error("a settlement lookup needs at least one place")]
    EmptyPlaces,
}

/// Selects the nearest place and applies the caption distance limits.
///
/// Distances through 10 km use the short form. Places farther than 500 km produce no context.
///
/// # Errors
/// Returns [`SettlementError`] for an invalid centre or an empty place list.
pub fn nearest_context(
    centre: LatLon,
    places: &[Place],
) -> Result<Option<Context>, SettlementError> {
    let centre = validate_location(centre)?;
    let Some((nearest, distance_km)) = places.iter().fold(None, |best, candidate| {
        let distance = great_circle_km(centre, candidate.location);
        let replace = match best {
            None => true,
            Some((selected, selected_distance)) => {
                distance
                    .total_cmp(&selected_distance)
                    .then_with(|| tie_order(candidate, selected))
                    == Ordering::Less
            }
        };
        if replace {
            Some((candidate, distance))
        } else {
            best
        }
    }) else {
        return Err(SettlementError::EmptyPlaces);
    };

    if distance_km > MAX_LABEL_KM {
        return Ok(None);
    }

    Ok(Some(Context {
        name: nearest.name.clone(),
        country: nearest.country.clone(),
        distance_km,
        short_form: distance_km <= SHORT_LABEL_KM,
    }))
}

fn tie_order(left: &Place, right: &Place) -> Ordering {
    left.name
        .cmp(&right.name)
        .then_with(|| left.country.cmp(&right.country))
        .then_with(|| left.location.lat.total_cmp(&right.location.lat))
        .then_with(|| left.location.lon.total_cmp(&right.location.lon))
}

fn validate_location(location: LatLon) -> Result<LatLon, SettlementError> {
    if !location.lat.is_finite() || !(-90.0..=90.0).contains(&location.lat) {
        return Err(SettlementError::InvalidLatitude { lat: location.lat });
    }
    if !location.lon.is_finite() {
        return Err(SettlementError::InvalidLongitude { lon: location.lon });
    }

    Ok(LatLon {
        lat: location.lat,
        lon: wrap_lon(location.lon),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::geodesy::EARTH_RADIUS_KM;

    fn place(name: &str, country: &str, lat: f64, lon: f64) -> Place {
        Place::new(name, country, LatLon { lat, lon }).unwrap()
    }

    fn at_km(km: f64) -> LatLon {
        LatLon {
            lat: (km / EARTH_RADIUS_KM).to_degrees(),
            lon: 0.0,
        }
    }

    #[test]
    fn great_circle_distance_selects_the_nearer_longitude_at_high_latitude() {
        let places = [place("A", "A", 77.0, 0.0), place("B", "B", 80.0, 10.0)];

        let selected = nearest_context(
            LatLon {
                lat: 80.0,
                lon: 0.0,
            },
            &places,
        )
        .unwrap()
        .unwrap();

        assert_eq!(selected.name(), "B");
    }

    #[test]
    fn ten_kilometres_is_inclusive_and_the_next_distance_uses_the_long_form() {
        let at_boundary = [place("Edge", "Country", at_km(10.0).lat, 0.0)];
        let just_beyond = [place("Beyond", "Country", at_km(10.1).lat, 0.0)];

        let edge = nearest_context(LatLon { lat: 0.0, lon: 0.0 }, &at_boundary)
            .unwrap()
            .unwrap();
        let beyond = nearest_context(LatLon { lat: 0.0, lon: 0.0 }, &just_beyond)
            .unwrap()
            .unwrap();

        assert!(edge.distance_km() <= SHORT_LABEL_KM);
        assert!(edge.short_form());
        assert!(beyond.distance_km() > SHORT_LABEL_KM);
        assert!(!beyond.short_form());
    }

    #[test]
    fn five_hundred_kilometres_is_inclusive_and_the_next_distance_has_no_context() {
        let at_boundary = [place("Edge", "Country", at_km(500.0).lat, 0.0)];
        let just_beyond = [place("Beyond", "Country", at_km(500.1).lat, 0.0)];

        let edge = nearest_context(LatLon { lat: 0.0, lon: 0.0 }, &at_boundary).unwrap();
        let beyond = nearest_context(LatLon { lat: 0.0, lon: 0.0 }, &just_beyond).unwrap();

        assert!(edge.is_some());
        assert!(beyond.is_none());
    }

    #[test]
    fn distance_comparison_uses_unrounded_values() {
        let nearer = place("Nearer", "Country", at_km(10.1).lat, 0.0);
        let farther = place("Farther", "Country", at_km(10.2).lat, 0.0);
        let centre = LatLon { lat: 0.0, lon: 0.0 };

        for places in [[farther.clone(), nearer.clone()], [nearer, farther]] {
            let selected = nearest_context(centre, &places).unwrap().unwrap();
            assert_eq!(selected.name(), "Nearer");
            assert_eq!(selected.distance_km().round(), 10.0);
        }
    }

    #[test]
    fn exact_ties_use_the_documented_key_independent_of_input_order() {
        let west = place("West", "Country", 0.0, -1.0);
        let east = place("East", "Country", 0.0, 1.0);
        let centre = LatLon { lat: 0.0, lon: 0.0 };
        let first = nearest_context(centre, &[west.clone(), east.clone()])
            .unwrap()
            .unwrap();
        let reversed = nearest_context(centre, &[east, west]).unwrap().unwrap();

        assert_eq!(first.name(), "East");
        assert_eq!(first.name(), reversed.name());
        assert_eq!(
            first.distance_km().to_bits(),
            reversed.distance_km().to_bits()
        );
    }

    #[test]
    fn longitude_wraps_across_the_antimeridian() {
        let across = place("Across", "Country", 0.0, -179.9);
        let nearby = place("Nearby", "Country", 0.0, 179.0);
        let selected = nearest_context(
            LatLon {
                lat: 0.0,
                lon: 179.9,
            },
            &[nearby, across],
        )
        .unwrap()
        .unwrap();

        assert_eq!(selected.name(), "Across");
    }

    #[test]
    fn empty_or_invalid_inputs_are_errors() {
        assert_eq!(
            nearest_context(LatLon { lat: 0.0, lon: 0.0 }, &[]),
            Err(SettlementError::EmptyPlaces),
        );
        assert!(matches!(
            Place::new("", "Country", LatLon { lat: 0.0, lon: 0.0 }),
            Err(SettlementError::EmptyName),
        ));
        assert!(matches!(
            Place::new("Name", " ", LatLon { lat: 0.0, lon: 0.0 }),
            Err(SettlementError::EmptyCountry),
        ));
        assert!(matches!(
            Place::new(
                "Name",
                "Country",
                LatLon {
                    lat: 91.0,
                    lon: 0.0
                }
            ),
            Err(SettlementError::InvalidLatitude { .. }),
        ));
        assert!(matches!(
            Place::new(
                "Name",
                "Country",
                LatLon {
                    lat: f64::NAN,
                    lon: 0.0
                }
            ),
            Err(SettlementError::InvalidLatitude { .. }),
        ));
        assert!(matches!(
            Place::new(
                "Name",
                "Country",
                LatLon {
                    lat: 0.0,
                    lon: f64::INFINITY
                }
            ),
            Err(SettlementError::InvalidLongitude { .. }),
        ));
        assert!(matches!(
            nearest_context(
                LatLon {
                    lat: 0.0,
                    lon: f64::NAN
                },
                &[place("Name", "Country", 0.0, 0.0)]
            ),
            Err(SettlementError::InvalidLongitude { .. }),
        ));
    }
}
