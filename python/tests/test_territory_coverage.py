import pytest
from shapely.geometry import Polygon, box

from population_circles.circle_document import Circle, Coordinate
from population_circles.territory_coverage import equal_area_crs, territory_share

EARTH_RADIUS_KM = 6371.0088


def test_equal_area_projection_uses_the_result_sphere() -> None:
    projection = equal_area_crs(EARTH_RADIUS_KM)

    ellipsoid = projection.ellipsoid
    assert ellipsoid is not None
    assert ellipsoid.semi_major_metre == pytest.approx(EARTH_RADIUS_KM * 1000.0)


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


def test_share_uses_the_result_sphere_for_circle_radius() -> None:
    territory = [box(-10.0, -5.0, 10.0, 5.0)]
    circle = Circle(
        centre=Coordinate(lat=0.0, lon=0.0),
        radius_km=1000.0,
        earth_radius_km=EARTH_RADIUS_KM,
    )
    larger_sphere = circle.model_copy(update={"earth_radius_km": EARTH_RADIUS_KM * 2.0})

    assert territory_share(larger_sphere, territory) < territory_share(circle, territory)


def test_circle_larger_than_a_hemisphere_is_world_minus_its_complement() -> None:
    circle = Circle(
        centre=Coordinate(lat=0.0, lon=0.0),
        radius_km=15_000.0,
        earth_radius_km=EARTH_RADIUS_KM,
    )

    assert territory_share(circle, [box(-1.0, 79.0, 1.0, 81.0)]) == pytest.approx(1.0)
    assert territory_share(circle, [box(179.0, -1.0, 180.0, 1.0)]) == pytest.approx(0.0)


def test_world_union_does_not_double_count_overlapping_features() -> None:
    circle = Circle(
        centre=Coordinate(lat=0.0, lon=0.0),
        radius_km=100.0,
        earth_radius_km=EARTH_RADIUS_KM,
    )
    feature = box(-2.0, -1.0, 2.0, 1.0)

    assert territory_share(circle, [feature, feature]) == pytest.approx(
        territory_share(circle, [feature])
    )


def test_holes_and_antimeridian_features_keep_their_land_area() -> None:
    circle = Circle(
        centre=Coordinate(lat=0.0, lon=0.0),
        radius_km=10.0,
        earth_radius_km=EARTH_RADIUS_KM,
    )
    territory_with_hole = Polygon(
        box(-2.0, -2.0, 2.0, 2.0).exterior.coords,
        [box(-0.5, -0.5, 0.5, 0.5).exterior.coords],
    )
    seam_circle = circle.model_copy(
        update={"centre": Coordinate(lat=0.0, lon=180.0), "radius_km": 100.0}
    )
    seam_feature = Polygon(
        ((179.0, -2.0), (-179.0, -2.0), (-179.0, 2.0), (179.0, 2.0), (179.0, -2.0))
    )

    assert territory_share(circle, [territory_with_hole]) == pytest.approx(0.0)
    assert 0.0 < territory_share(seam_circle, [seam_feature]) <= 1.0


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
