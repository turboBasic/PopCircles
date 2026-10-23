// The earth model, stated once for the whole program: a sphere of the IUGG mean radius, with
// distances as great-circle arcs on it. Nothing here is ellipsoidal, so a result may differ from a
// WGS 84 geodesic by a few tenths of a percent.
pub const EARTH_RADIUS_KM: f64 = 6371.0088;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

/// Longitude reduced to [-180, 180). Latitude has no counterpart here on purpose: a coordinate past
/// a pole is an error to reject, not a value to fold, because folding it silently moves a point to
/// the far side of the globe.
#[must_use]
pub fn wrap_lon(lon: f64) -> f64 {
    (lon + 180.0).rem_euclid(360.0) - 180.0
}

/// The central angle between two points, by the haversine formula (Sinnott, "Virtues of the
/// Haversine", *Sky & Telescope* 68(2), 1984).
///
/// Haversine loses relative precision as the pair approaches antipodal, where the sine of the half
/// angle flattens against 1. That regime is documented rather than handled: this search asks for
/// distances against a circle radius far below half the globe, so it never reads an answer from it.
#[must_use]
pub fn angular_distance_rad(from: LatLon, to: LatLon) -> f64 {
    let lat_from = from.lat.to_radians();
    let lat_to = to.lat.to_radians();
    let half_delta_lat = (lat_to - lat_from) / 2.0;
    let half_delta_lon = (to.lon - from.lon).to_radians() / 2.0;

    let h =
        half_delta_lat.sin().powi(2) + lat_from.cos() * lat_to.cos() * half_delta_lon.sin().powi(2);
    // Pole to pole puts h a few ulps above 1 — cos(90°) is 6.1e-17 rather than 0, so the second
    // term never quite vanishes. Unclamped that makes sqrt(1 - h) a NaN out of a well-defined
    // input, so the clamp is correctness rather than tidiness.
    let h = h.clamp(0.0, 1.0);

    2.0 * f64::atan2(h.sqrt(), (1.0 - h).sqrt())
}

/// Great-circle distance in kilometres on the [`EARTH_RADIUS_KM`] sphere.
#[must_use]
pub fn great_circle_km(from: LatLon, to: LatLon) -> f64 {
    arc_km(angular_distance_rad(from, to))
}

/// The length of an arc subtending `angle_rad`: [`central_angle_rad`]'s inverse.
///
/// Apart from [`great_circle_km`] because a length assembled from two arcs has no pair of points to
/// measure between — a bound over a rectangle of the grid is one hop along a parallel and one along a
/// meridian, summed as angles and converted once.
#[must_use]
pub fn arc_km(angle_rad: f64) -> f64 {
    EARTH_RADIUS_KM * angle_rad
}

/// The central angle an arc of `km` subtends: [`great_circle_km`]'s inverse.
///
/// It lives here for the reason [`zone_area_km2`] does. Dividing by the radius names the earth model as
/// much as multiplying by it does, and a circle's radius arrives in kilometres while the angle a
/// spherical cap is compared against is in radians.
#[must_use]
pub fn central_angle_rad(km: f64) -> f64 {
    km / EARTH_RADIUS_KM
}

#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum RadiusError {
    #[error("a circle radius must be finite; {km} km is not")]
    NotFinite { km: f64 },

    #[error("a circle radius must not be negative; {km} km is")]
    Negative { km: f64 },
}

/// A circle radius in kilometres, checked once where it is made.
///
/// It lives here because the check is against the earth model rather than against any one caller: a
/// radius is a length on this sphere, and every signature taking one would otherwise repeat the same
/// two tests. Zero is a radius — the circle is its own centre.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct RadiusKm(f64);

impl RadiusKm {
    /// # Errors
    /// [`RadiusError::NotFinite`] or [`RadiusError::Negative`] when the value is not a length.
    ///
    /// No upper bound: a radius past half the circumference names the whole sphere, which is a legal
    /// question with a legal answer. Growing one is [`RadiusKm::widened_by`]'s, which is total, so nothing
    /// downstream needs a ceiling here to protect it.
    pub fn new(km: f64) -> Result<Self, RadiusError> {
        if !km.is_finite() {
            return Err(RadiusError::NotFinite { km });
        }
        if km < 0.0 {
            return Err(RadiusError::Negative { km });
        }
        Ok(Self(km))
    }

    #[must_use]
    pub const fn km(self) -> f64 {
        self.0
    }

    /// This radius widened by `km`.
    ///
    /// Total rather than fallible, and the reason is a bound on the argument its caller can prove.
    /// `search`'s slack is bounded by the sphere — a [`crate::grid::Grid`] spans at most a full turn of
    /// longitude and half a turn of latitude, so the two-hop bound over one cannot exceed about 60 000 km
    /// — and adding a quantity that small to any finite radius stays finite, because the gap between
    /// `f64::MAX` and its neighbour is 2e292. A fallible constructor here would hand every caller an error
    /// arm no input can reach.
    ///
    /// Anything that is not a length lands on the widest radius there is rather than on the original. A
    /// caller widens in order to bound something, so an answer narrower than asked is the one failure mode
    /// that could lose a maximum; wider is merely slower.
    #[must_use]
    pub fn widened_by(self, km: f64) -> Self {
        let widened = self.0 + km;
        if widened.is_finite() {
            Self(widened.max(self.0))
        } else {
            Self(f64::MAX)
        }
    }
}

/// An integer number of kilometres, which is always a length.
///
/// Total where [`RadiusKm::new`] is fallible, and that is what it is for: the search over radius steps in
/// whole kilometres, so a fallible conversion there would hand every step an error arm no `u32` can reach.
impl From<u32> for RadiusKm {
    fn from(km: u32) -> Self {
        Self(f64::from(km))
    }
}

/// The band of latitude between two parallels: what a spherical zone stands on, and what a grid row
/// occupies.
///
/// Named fields rather than a tuple because [`zone_area_km2`] subtracts these two in one direction
/// only, and a silent swap there flips a sign rather than failing.
///
/// Library consumers obtain bands from [`crate::grid::Grid::lat_bounds`], so they cannot pass an
/// inverted band to [`zone_area_km2`].
///
/// ```compile_fail
/// use popcircles::geodesy::LatBand;
///
/// let _ = LatBand { north: -90.0, south: 90.0 };
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatBand {
    north: f64,
    south: f64,
}

impl LatBand {
    pub(crate) const fn new(north: f64, south: f64) -> Self {
        Self { north, south }
    }

    #[must_use]
    pub const fn north(self) -> f64 {
        self.north
    }

    #[must_use]
    pub const fn south(self) -> f64 {
        self.south
    }
}

/// The area a band cuts from the sphere across `delta_lon_deg` of longitude, by the spherical zone
/// formula `R² · Δλ · (sin φ_north − sin φ_south)`.
///
/// It lives beside the radius rather than beside its caller so that the sphere appears in one module:
/// a second `EARTH_RADIUS_KM` in an expression elsewhere is how the earth model starts to drift.
#[must_use]
pub fn zone_area_km2(band: LatBand, delta_lon_deg: f64) -> f64 {
    EARTH_RADIUS_KM
        * EARTH_RADIUS_KM
        * delta_lon_deg.to_radians()
        * (band.north.to_radians().sin() - band.south.to_radians().sin())
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests. float_cmp covers the symmetry assertion,
// where exact equality is the property: the formula is symmetric in its arguments, so any
// asymmetric rewrite of it should fail rather than pass within a tolerance.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn the_seam_folds_west() {
        // A half-open range has to choose an end, and -180 is the one that exists. All three of
        // these name the same meridian.
        for lon in [180.0, -180.0, 540.0, -540.0] {
            assert!((wrap_lon(lon) - -180.0).abs() < 1e-12, "wrap_lon({lon})");
        }
    }

    #[test]
    fn longitudes_already_in_range_are_untouched() {
        for lon in [-180.0, -179.5, -0.5, 0.0, 0.5, 179.5] {
            assert!((wrap_lon(lon) - lon).abs() < 1e-12, "wrap_lon({lon})");
        }
    }

    fn at(lat: f64, lon: f64) -> LatLon {
        LatLon { lat, lon }
    }

    fn assert_rel(actual: f64, expected: f64, what: &str) {
        let scale = if expected == 0.0 { 1.0 } else { expected.abs() };
        assert!(
            (actual - expected).abs() / scale < 1e-9,
            "{what}: got {actual}, expected {expected}"
        );
    }

    #[test]
    fn identical_points_are_zero_apart() {
        for p in [
            at(0.0, 0.0),
            at(51.5, -0.12),
            at(90.0, 0.0),
            at(-90.0, 73.0),
        ] {
            assert_eq!(angular_distance_rad(p, p), 0.0);
        }
    }

    #[test]
    fn one_degree_along_the_equator_is_one_degree_of_arc() {
        assert_rel(
            great_circle_km(at(0.0, 0.0), at(0.0, 1.0)),
            EARTH_RADIUS_KM * std::f64::consts::PI / 180.0,
            "one degree of equator",
        );
    }

    #[test]
    fn a_quarter_turn_is_a_quarter_of_the_circumference() {
        let quarter = EARTH_RADIUS_KM * std::f64::consts::FRAC_PI_2;
        assert_rel(
            great_circle_km(at(0.0, 0.0), at(0.0, 90.0)),
            quarter,
            "equator to 90E",
        );
        assert_rel(
            great_circle_km(at(0.0, 0.0), at(90.0, 0.0)),
            quarter,
            "equator to north pole",
        );
    }

    #[test]
    fn antipodes_are_half_the_circumference() {
        let half = EARTH_RADIUS_KM * std::f64::consts::PI;
        assert_rel(
            great_circle_km(at(0.0, 0.0), at(0.0, 180.0)),
            half,
            "east around",
        );
        assert_rel(
            great_circle_km(at(0.0, 0.0), at(0.0, -180.0)),
            half,
            "west around",
        );
        // The case the clamp exists for: unclamped this is a NaN, not a slightly wrong number.
        assert_rel(
            great_circle_km(at(90.0, 0.0), at(-90.0, 0.0)),
            half,
            "pole to pole",
        );
    }

    #[test]
    fn a_distance_converts_back_to_the_angle_it_came_from() {
        // The pairs that make this more than an algebraic identity are the last two: pole to pole is
        // where the clamp above holds the angle at pi, and the antipodal pair is where haversine's
        // precision is worst. A cap radius is turned into an angle exactly this way.
        for (from, to) in [
            (at(0.0, 0.0), at(0.0, 1.0)),
            (at(51.5, -0.12), at(48.858, 2.294)),
            (at(0.0, 0.0), at(0.0, 180.0)),
            (at(90.0, 0.0), at(-90.0, 0.0)),
        ] {
            assert_rel(
                central_angle_rad(great_circle_km(from, to)),
                angular_distance_rad(from, to),
                "round trip",
            );
        }
        // The degenerate cap: no radius, no angle, and no division that turns one into the other.
        assert_eq!(central_angle_rad(0.0), 0.0);
    }

    // Published worked examples, each stating its own earth model. The assertion is on the central
    // angle because that is radius-independent: every source uses a different sphere from ours
    // (6371.2, 6371, and one implied by the nautical mile), and comparing angles removes that
    // disagreement rather than hiding it inside a tolerance. Each tolerance is the precision the
    // source printed — five or six significant figures — not a number picked to pass.
    //
    // 1. White House to Eiffel Tower — "Haversine formula", Wikipedia. Sphere of 6371.2 km.
    // 2. Valparaiso to Shanghai — "Great-circle navigation", Wikipedia. Sphere of 6371 km. This
    //    one crosses the antimeridian and is 168.56 deg long, so it also covers the long-line case.
    // 3. LAX to JFK — Ed Williams, "Aviation Formulary" v1.47, https://edwilliams.org/avform147.htm
    //    Spherical, published directly in radians.
    #[test]
    fn published_central_angles_agree() {
        let cases = [
            (
                "White House to Eiffel Tower",
                at(38.898, -77.037),
                at(48.858, 2.294),
                55.411_f64.to_radians(),
                1e-5,
            ),
            (
                "Valparaiso to Shanghai",
                at(-33.0, -71.6),
                at(31.4, 121.8),
                168.56_f64.to_radians(),
                5e-5,
            ),
            (
                "LAX to JFK",
                at(33.0 + 57.0 / 60.0, -(118.0 + 24.0 / 60.0)),
                at(40.0 + 38.0 / 60.0, -(73.0 + 47.0 / 60.0)),
                0.623_585,
                2e-6,
            ),
        ];

        for (what, from, to, expected_rad, tolerance) in cases {
            let actual = angular_distance_rad(from, to);
            let relative = (actual - expected_rad).abs() / expected_rad;
            assert!(
                relative < tolerance,
                "{what}: {actual} rad against published {expected_rad} rad, off by {relative}"
            );
        }
    }

    // The counterpart check, and the one whose tolerance has to be argued: these are ellipsoidal
    // figures from the same two sources, and a sphere disagrees with WGS 84 by up to about 0.5%
    // (both sources say so). So 0.5% is what they get. Nothing tighter would be meaningful, and the
    // test's job is catching a unit slip, a lat/lon swap or a wrong radius — each of which misses by
    // far more than half a percent — rather than pinning accuracy we do not claim.
    #[test]
    fn published_ellipsoidal_distances_agree_to_half_a_percent() {
        let cases = [
            (
                "White House to Eiffel Tower",
                at(38.898, -77.037),
                at(48.858, 2.294),
                6177.45,
            ),
            (
                "Valparaiso to Shanghai",
                at(-33.0, -71.6),
                at(31.4, 121.8),
                18752.0,
            ),
        ];

        for (what, from, to, ellipsoidal_km) in cases {
            let actual = great_circle_km(from, to);
            let relative = (actual - ellipsoidal_km).abs() / ellipsoidal_km;
            assert!(
                relative < 0.005,
                "{what}: {actual} km against WGS 84's {ellipsoidal_km} km, off by {relative}"
            );
        }
    }

    #[test]
    fn a_zone_over_the_whole_sphere_is_four_pi_r_squared() {
        let whole = zone_area_km2(
            LatBand {
                north: 90.0,
                south: -90.0,
            },
            360.0,
        );
        assert_rel(
            whole,
            4.0 * std::f64::consts::PI * EARTH_RADIUS_KM * EARTH_RADIUS_KM,
            "the whole sphere",
        );
        // Half the longitude is half the area at any latitude, which is what makes a per-row area
        // independent of column.
        assert_rel(
            zone_area_km2(
                LatBand {
                    north: 90.0,
                    south: -90.0,
                },
                180.0,
            ),
            whole / 2.0,
            "half the turn",
        );
    }

    #[test]
    fn an_arc_is_the_angle_it_subtends_and_back_again() {
        // The pair is what a radius is converted through in both directions, so the round trip is the
        // property rather than either figure. Zero is separate because it is the one value where the
        // conversion has no division to lose anything in.
        for angle_rad in [1e-9, 0.5, 1.0, std::f64::consts::PI] {
            assert_rel(
                central_angle_rad(arc_km(angle_rad)),
                angle_rad,
                "round trip",
            );
        }
        assert_eq!(arc_km(0.0), 0.0);
        assert_eq!(arc_km(1.0), EARTH_RADIUS_KM);
    }

    #[test]
    fn a_radius_that_is_not_a_number_is_no_radius() {
        // Matched rather than compared: the variant carries the value it rejected, and a NaN is not
        // equal to itself, so `assert_eq!` on this one would fail on a correct rejection.
        assert!(matches!(
            RadiusKm::new(f64::NAN),
            Err(RadiusError::NotFinite { km }) if km.is_nan()
        ));
    }

    #[test]
    fn an_infinite_radius_is_no_radius() {
        for km in [f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                RadiusKm::new(km).unwrap_err(),
                RadiusError::NotFinite { km },
                "{km}"
            );
        }
    }

    #[test]
    fn a_negative_radius_is_no_radius() {
        // Rejected before the sign test can be mistaken for the finite one: -1 is finite, so this
        // fails on the second branch and not the first.
        assert_eq!(
            RadiusKm::new(-1.0).unwrap_err(),
            RadiusError::Negative { km: -1.0 }
        );
    }

    #[test]
    fn a_zero_radius_is_a_length() {
        assert_eq!(RadiusKm::new(0.0).unwrap().km(), 0.0);
    }

    #[test]
    fn a_radius_past_the_globe_is_a_length() {
        // Half the circumference is 20 015.09 km, so this names the whole sphere. Admitting it is what
        // lets a caller ask for the world without a special case.
        assert_eq!(RadiusKm::new(20_016.0).unwrap().km(), 20_016.0);
    }

    #[test]
    fn every_integer_kilometre_is_a_radius() {
        // The conversion the search over radius steps through, at both ends of the type and at the
        // ceiling in between: total, so no step of that loop carries an error arm.
        for km in [0u32, 1, 20_016, u32::MAX] {
            assert_eq!(
                RadiusKm::from(km).km(),
                RadiusKm::new(f64::from(km)).unwrap().km()
            );
        }
    }

    #[test]
    fn the_largest_finite_radius_is_a_length() {
        // Absurd and legal, and the reason it is pinned: a caller widening a radius by adding to it
        // overflows here, and the failure it is entitled to is this constructor's rather than a panic.
        let largest = RadiusKm::new(f64::MAX).unwrap();
        assert_eq!(largest.km(), f64::MAX);
        assert_eq!(
            RadiusKm::new(largest.km() + 1e300).unwrap_err(),
            RadiusError::NotFinite { km: f64::INFINITY }
        );
    }

    #[test]
    fn widening_a_radius_never_narrows_it_and_never_leaves_the_range() {
        // The ordinary case, then the three a bound must not be quietly narrowed by. `f64::MAX` widened by
        // any slack the sphere can produce is `f64::MAX` again — the gap to its neighbour is 2e292 — which
        // is what makes this total rather than fallible.
        assert_eq!(
            RadiusKm::new(3000.0).unwrap().widened_by(300.0).km(),
            3300.0
        );

        let largest = RadiusKm::new(f64::MAX).unwrap();
        assert_eq!(largest.widened_by(60_083.0).km(), f64::MAX);
        // Past what any grid can produce, so the answer saturates rather than returning something
        // narrower than asked: for a bound, wider is slower and narrower loses a maximum.
        assert_eq!(largest.widened_by(1e300).km(), f64::MAX);

        let three = RadiusKm::new(3000.0).unwrap();
        assert_eq!(three.widened_by(-500.0).km(), 3000.0);
        assert_eq!(three.widened_by(f64::NAN).km(), f64::MAX);
    }

    #[test]
    fn pole_to_pole_is_finite_from_every_meridian() {
        for lon in [-180.0, -73.0, 0.0, 73.0, 179.9] {
            let d = great_circle_km(at(90.0, 0.0), at(-90.0, lon));
            assert!(d.is_finite(), "meridian {lon} produced {d}");
        }
    }

    proptest! {
        #[test]
        fn distance_is_symmetric(
            lat_a in -90.0f64..=90.0,
            lon_a in -180.0f64..180.0,
            lat_b in -90.0f64..=90.0,
            lon_b in -180.0f64..180.0,
        ) {
            let a = at(lat_a, lon_a);
            let b = at(lat_b, lon_b);
            prop_assert_eq!(angular_distance_rad(a, b), angular_distance_rad(b, a));
        }

        #[test]
        fn distance_obeys_the_triangle_inequality(
            lat_a in -90.0f64..=90.0,
            lon_a in -180.0f64..180.0,
            lat_b in -90.0f64..=90.0,
            lon_b in -180.0f64..180.0,
            lat_c in -90.0f64..=90.0,
            lon_c in -180.0f64..180.0,
        ) {
            let a = at(lat_a, lon_a);
            let b = at(lat_b, lon_b);
            let c = at(lat_c, lon_c);
            let direct = angular_distance_rad(a, c);
            let via_b = angular_distance_rad(a, b) + angular_distance_rad(b, c);
            prop_assert!(direct <= via_b + 1e-9, "{} > {}", direct, via_b);
        }

        #[test]
        fn no_pair_of_points_is_further_than_half_the_globe(
            lat_a in -90.0f64..=90.0,
            lon_a in -180.0f64..180.0,
            lat_b in -90.0f64..=90.0,
            lon_b in -180.0f64..180.0,
        ) {
            let d = angular_distance_rad(at(lat_a, lon_a), at(lat_b, lon_b));
            prop_assert!(d.is_finite() && (0.0..=std::f64::consts::PI).contains(&d), "{}", d);
        }

        #[test]
        fn wrapping_always_lands_in_the_half_open_range(lon in -1.0e6f64..1.0e6) {
            let wrapped = wrap_lon(lon);
            prop_assert!((-180.0..180.0).contains(&wrapped), "{} wrapped to {}", lon, wrapped);
        }

        #[test]
        fn wrapping_is_invariant_under_whole_turns(lon in -180.0f64..180.0, k in -3i32..=3) {
            let shifted = wrap_lon(lon + 360.0 * f64::from(k));
            prop_assert!((shifted - wrap_lon(lon)).abs() < 1e-9, "{} vs {}", shifted, wrap_lon(lon));
        }
    }
}
