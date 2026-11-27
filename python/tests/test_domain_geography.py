import math
from dataclasses import FrozenInstanceError

import pytest

from population_circles.domain.geography import GeographicCoordinate, SphericalCircle

EARTH_RADIUS_KM = 6371.0088


@pytest.mark.parametrize(
    ("lat", "lon"),
    [
        (-90.0, -180.0),
        (-90.0, 180.0),
        (90.0, -180.0),
        (90.0, 180.0),
        (0.0, 0.0),
    ],
)
def test_geographic_coordinate_accepts_finite_values_at_geographic_boundaries(
    lat: float, lon: float
) -> None:
    coordinate = GeographicCoordinate(lat=lat, lon=lon)

    assert coordinate.lat == lat
    assert coordinate.lon == lon


@pytest.mark.parametrize(
    ("lat", "lon"),
    [
        (float("-inf"), 0.0),
        (float("inf"), 0.0),
        (float("nan"), 0.0),
        (-90.1, 0.0),
        (90.1, 0.0),
        (0.0, float("-inf")),
        (0.0, float("inf")),
        (0.0, float("nan")),
        (0.0, -180.1),
        (0.0, 180.1),
    ],
)
def test_geographic_coordinate_rejects_non_finite_or_out_of_range_values(
    lat: float, lon: float
) -> None:
    with pytest.raises(ValueError, match="finite latitude and longitude in geographic range"):
        GeographicCoordinate(lat=lat, lon=lon)


def test_geographic_coordinate_is_immutable() -> None:
    coordinate = GeographicCoordinate(lat=0.0, lon=0.0)

    with pytest.raises(FrozenInstanceError):
        coordinate.lat = 1.0  # pyright: ignore[reportAttributeAccessIssue]


def test_spherical_circle_accepts_positive_radius_below_half_the_circumference() -> None:
    centre = GeographicCoordinate(lat=0.0, lon=0.0)
    circle = SphericalCircle(
        centre=centre,
        radius_km=math.pi * EARTH_RADIUS_KM - 1e-9,
        earth_radius_km=EARTH_RADIUS_KM,
    )

    assert circle.centre is centre
    assert circle.radius_km < math.pi * circle.earth_radius_km


@pytest.mark.parametrize(
    "earth_radius_km",
    [float("-inf"), -1.0, 0.0, float("inf"), float("nan")],
)
def test_spherical_circle_rejects_non_finite_or_non_positive_earth_radius(
    earth_radius_km: float,
) -> None:
    with pytest.raises(ValueError, match="earth radius must be finite and positive"):
        SphericalCircle(
            centre=GeographicCoordinate(lat=0.0, lon=0.0),
            radius_km=1.0,
            earth_radius_km=earth_radius_km,
        )


@pytest.mark.parametrize(
    ("radius_km", "earth_radius_km"),
    [
        (float("-inf"), EARTH_RADIUS_KM),
        (-1.0, EARTH_RADIUS_KM),
        (0.0, EARTH_RADIUS_KM),
        (math.pi * EARTH_RADIUS_KM, EARTH_RADIUS_KM),
        (math.pi * EARTH_RADIUS_KM + 1e-9, EARTH_RADIUS_KM),
        (float("inf"), EARTH_RADIUS_KM),
        (float("nan"), EARTH_RADIUS_KM),
    ],
)
def test_spherical_circle_rejects_non_finite_non_positive_or_excessive_radius(
    radius_km: float, earth_radius_km: float
) -> None:
    with pytest.raises(ValueError, match=r"circle radius .* must be finite and between zero"):
        SphericalCircle(
            centre=GeographicCoordinate(lat=0.0, lon=0.0),
            radius_km=radius_km,
            earth_radius_km=earth_radius_km,
        )


def test_spherical_circle_is_immutable() -> None:
    circle = SphericalCircle(
        centre=GeographicCoordinate(lat=0.0, lon=0.0),
        radius_km=1.0,
        earth_radius_km=EARTH_RADIUS_KM,
    )

    with pytest.raises(FrozenInstanceError):
        circle.radius_km = 2.0  # pyright: ignore[reportAttributeAccessIssue]
