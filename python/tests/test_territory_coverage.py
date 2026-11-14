import pytest
from shapely.geometry import box

from population_circles.circle_document import Circle, Coordinate
from population_circles.territory_coverage import equal_area_crs, territory_share

EARTH_RADIUS_KM = 6371.0088


def test_equal_area_projection_uses_the_result_sphere() -> None:
    projection = equal_area_crs(EARTH_RADIUS_KM)

    assert projection.ellipsoid.semi_major_metre == pytest.approx(EARTH_RADIUS_KM * 1000.0)


def test_share_uses_circle_intersection_over_territory_area() -> None:
    circle = Circle(
        centre=Coordinate(lat=0.0, lon=0.0),
        radius_km=100.0,
        earth_radius_km=EARTH_RADIUS_KM,
    )

    assert territory_share(circle, [box(-0.1, -0.1, 0.1, 0.1)]) == pytest.approx(1.0)
    share = territory_share(circle, [box(-2.0, -1.0, 2.0, 1.0)])
    assert 0.0 < share < 1.0
    assert territory_share(circle, [box(10.0, 10.0, 11.0, 11.0)]) == pytest.approx(0.0)


@pytest.mark.parametrize(
    ("radius_km", "earth_radius_km"),
    [(0.0, EARTH_RADIUS_KM), (float("inf"), EARTH_RADIUS_KM), (1.0, 0.0)],
)
def test_invalid_circle_measurements_are_refused(
    radius_km: float,
    earth_radius_km: float,
) -> None:
    circle = Circle(
        centre=Coordinate(lat=0.0, lon=0.0),
        radius_km=radius_km,
        earth_radius_km=earth_radius_km,
    )

    with pytest.raises(ValueError, match="radius"):
        territory_share(circle, [box(-1.0, -1.0, 1.0, 1.0)])
