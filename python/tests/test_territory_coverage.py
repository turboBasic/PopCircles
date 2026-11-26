import math

import pytest
from shapely import make_valid
from shapely.geometry import Polygon, box

from population_circles.documents.result_document import Circle, Coordinate
from population_circles.geography.territory_share import (
    enrich_document,
    equal_area_crs,
    territory_share,
)

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
    assert share == pytest.approx(0.318, abs=0.01)
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


def test_large_cap_share_uses_spherical_area_not_aeqd_disk_area() -> None:
    circle = Circle(
        centre=Coordinate(lat=0.0, lon=0.0),
        radius_km=9_000.0,
        earth_radius_km=EARTH_RADIUS_KM,
    )

    share = territory_share(circle, [box(-179.0, -89.0, 0.0, 89.0), box(0.0, -89.0, 179.0, 89.0)])

    expected = (1.0 - math.cos(circle.radius_km / EARTH_RADIUS_KM)) / 2.0
    assert share == pytest.approx(expected, abs=0.01)


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


def test_invalid_boundary_topology_is_repaired_without_mutating_the_source() -> None:
    circle = Circle(
        centre=Coordinate(lat=0.25, lon=0.5),
        radius_km=100.0,
        earth_radius_km=EARTH_RADIUS_KM,
    )
    invalid = Polygon(((0, 0), (1, 1), (0, 1), (1, 0), (0, 0)))
    assert not invalid.is_valid

    actual = territory_share(circle, [invalid])
    repaired = territory_share(circle, [make_valid(invalid)])

    assert actual == pytest.approx(repaired)
    assert not invalid.is_valid


def test_ring_winding_across_the_south_pole_is_clipped_validly() -> None:
    antarctic_cap = Polygon(
        (
            (-180, -63),
            (-90, -63),
            (0, -63),
            (90, -63),
            (180, -63),
            (180, -90),
            (-180, -90),
            (-180, -63),
        )
    )
    circle = Circle(
        centre=Coordinate(lat=-75.0, lon=79.0),
        radius_km=100.0,
        earth_radius_km=EARTH_RADIUS_KM,
    )

    assert territory_share(circle, [antarctic_cap]) == pytest.approx(0.00113, abs=0.0001)


def test_a_result_with_no_polygonal_source_area_keeps_coverage_absent() -> None:
    document: dict[str, object] = {
        "schema_version": 2,
        "document": "most-populous",
        "earth_model": {"model": "sphere", "radius_km": EARTH_RADIUS_KM},
        "provenance": {"dataset": "fixture", "digest": "fixture", "decimation": 1},
        "result": {
            "centre": {"lat": 0.0, "lon": 0.0},
            "radius_km": 100.0,
            "population": 1.0,
            "share_of_total": 0.5,
        },
    }

    enrich_document(document, {"empty": Polygon()})

    result = document["result"]
    assert isinstance(result, dict)
    assert "territory_share" not in result


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


@pytest.mark.parametrize(("lat", "lon"), [(91.0, 0.0), (0.0, 181.0)])
def test_invalid_circle_centres_are_refused(lat: float, lon: float) -> None:
    circle = Circle(
        centre=Coordinate(lat=lat, lon=lon),
        radius_km=1.0,
        earth_radius_km=EARTH_RADIUS_KM,
    )

    with pytest.raises(ValueError, match="centre"):
        territory_share(circle, [box(-1.0, -1.0, 1.0, 1.0)])
