import math

import pytest
from pyproj import Geod
from shapely.geometry.polygon import Polygon

from population_circles.documents.result_document import Coordinate
from population_circles.geography.circle_geometry import (
    METRES_PER_KM,
    POLE_LAT,
    QUAD_SEGS,
    Cap,
    boundary,
    cap,
    enclosed_poles,
    geographic,
    polygons,
)

EARTH_RADIUS_KM = 6371.0088

# The five cases the drawing primitive was chosen on: a cap crossing the seam, one over each pole,
# one holding both, and one crossing nothing. The last is the only one that round-trips, which is
# why it is here.
SEAM = Coordinate(lat=10.0, lon=178.0)
NORTH = Coordinate(lat=78.0, lon=20.0)
SOUTH = Coordinate(lat=-80.0, lon=-170.0)
WHOLE = Coordinate(lat=25.125, lon=79.708)
CLEAN = Coordinate(lat=30.0, lon=100.0)

SEAM_RADIUS_KM = 3000.0
NORTH_RADIUS_KM = 3000.0
SOUTH_RADIUS_KM = 4000.0
WHOLE_RADIUS_KM = 16384.0
CLEAN_RADIUS_KM = 3300.0

CASES = (
    (SEAM, SEAM_RADIUS_KM),
    (NORTH, NORTH_RADIUS_KM),
    (SOUTH, SOUTH_RADIUS_KM),
    (WHOLE, WHOLE_RADIUS_KM),
    (CLEAN, CLEAN_RADIUS_KM),
)
CASE_IDS = ("antimeridian", "north-pole", "south-pole", "both-poles", "no-crossing")

# The two one-pole cases with the latitude the cap reaches away from its pole, which is the far edge
# of the band the closing bounds.
POLE_CASES = (
    (NORTH, NORTH_RADIUS_KM, POLE_LAT, 51.020),
    (SOUTH, SOUTH_RADIUS_KM, -POLE_LAT, -44.027),
)
POLE_CASE_IDS = ("north-pole", "south-pole")

# shapely starts a buffer's ring due east and walks it clockwise, so a quarter of QUAD_SEGS * 4
# sides is a quarter turn. Each index below is paired with the azimuth that vertex lies on.
CARDINALS = ((0, 90.0), (QUAD_SEGS, 180.0), (2 * QUAD_SEGS, -90.0), (3 * QUAD_SEGS, 0.0))

# A ring of QUAD_SEGS * 4 sides is that many vertices plus the repeated close.
RING_VERTICES = 4 * QUAD_SEGS + 1

# Measured, and the figures the two-object split exists for: cutting the ring down the ±180 meridian
# or closing it over a pole each leaves five more vertices than the ring carried, and none of the
# five lies on the cap's boundary. A cap holding both poles is the other shape entirely — the world,
# whose ring is four corners and the repeated close, with the region it misses as a hole.
CUT_VERTICES = RING_VERTICES + 5
WORLD_VERTICES = 5
SEAM_PARTS = 2
SEAM_LON = 180.0

# A vertex the cut synthesises sits on a line straight in longitude and latitude between two ring
# vertices, and the boundary between them is an arc, so it can land outside the cap: 7.2 m at 3000
# km over the north pole, measured, which is the worst of the five cases. The claim is that nothing
# is drawn outside the cap, and this is the width of "nothing" — two orders below the finest cell a
# grid this program answers on has.
CUT_SLACK_KM = 0.01


def great_circle_km(a: Coordinate, b: Coordinate) -> float:
    # Written here rather than imported: a vertex checked against the same code that placed it is
    # not checked. This is the haversine on the sphere `geodesy.rs` owns, which is the model the
    # document under test published.
    phi_a, phi_b = math.radians(a.lat), math.radians(b.lat)
    half_lat = math.sin((phi_b - phi_a) / 2.0) ** 2
    half_lon = math.sin(math.radians(b.lon - a.lon) / 2.0) ** 2
    chord = half_lat + math.cos(phi_a) * math.cos(phi_b) * half_lon
    return 2.0 * EARTH_RADIUS_KM * math.asin(math.sqrt(chord))


def built(centre: Coordinate, radius_km: float) -> Cap:
    return cap(centre, radius_km, EARTH_RADIUS_KM)


def rings(polygon: Polygon) -> list[tuple[float, float]]:
    # Holes included: the region a cap holding both poles does not cover is one, and its vertices
    # are the only real boundary the drawn shape carries.
    walk = [(x, y) for x, y in polygon.exterior.coords]
    for hole in polygon.interiors:
        walk.extend((x, y) for x, y in hole.coords)
    return walk


def drawn(shape: Cap) -> tuple[Polygon, ...]:
    return polygons(geographic(shape))


@pytest.mark.parametrize(("centre", "radius_km"), CASES, ids=CASE_IDS)
def test_every_boundary_vertex_is_the_radius_from_the_centre(
    centre: Coordinate,
    radius_km: float,
) -> None:
    ring = boundary(built(centre, radius_km))
    assert len(ring) == RING_VERTICES
    for vertex in ring:
        assert great_circle_km(centre, vertex) == pytest.approx(radius_km, abs=1e-6)


def test_the_cardinal_vertices_match_the_direct_geodesic_problem() -> None:
    # pyproj's direct geodesic problem at flattening zero: a different code path than the one that
    # drew the ring, answering about the same sphere.
    geod = Geod(a=EARTH_RADIUS_KM * METRES_PER_KM, f=0.0)
    ring = boundary(built(CLEAN, CLEAN_RADIUS_KM))
    for index, azimuth in CARDINALS:
        lon, lat, _ = geod.fwd(CLEAN.lon, CLEAN.lat, azimuth, CLEAN_RADIUS_KM * METRES_PER_KM)
        assert ring[index].lat == pytest.approx(lat, abs=1e-9)
        assert ring[index].lon == pytest.approx(lon, abs=1e-9)


def test_a_cap_crossing_the_antimeridian_is_drawn_in_two_parts() -> None:
    # The case a plotting library's geodetic transform fills the complement of. Two parts and a
    # longitude span reaching both limits is what a cut down the seam looks like.
    parts = drawn(built(SEAM, SEAM_RADIUS_KM))
    assert len(parts) == SEAM_PARTS
    west, east = (
        min(x for p in parts for x, _ in rings(p)),
        max(x for p in parts for x, _ in rings(p)),
    )
    assert (west, east) == (-SEAM_LON, SEAM_LON)
    assert sum(len(rings(p)) for p in parts) == CUT_VERTICES


@pytest.mark.parametrize(
    ("centre", "radius_km", "pole", "far_lat"),
    POLE_CASES,
    ids=POLE_CASE_IDS,
)
def test_a_cap_over_a_pole_is_closed_at_that_pole(
    centre: Coordinate,
    radius_km: float,
    pole: float,
    far_lat: float,
) -> None:
    # The vertices the closing synthesises are the point of the two-object split: every one of the
    # ring's 721 is the radius out, and the drawn polygon reaches the pole itself — 1334.341 km from
    # the northern centre, and therefore on no boundary at all.
    shape = built(centre, radius_km)
    assert enclosed_poles(shape) == (pole,)
    assert all(abs(vertex.lat) < POLE_LAT for vertex in boundary(shape))

    parts = drawn(shape)
    assert len(parts) == 1
    vertices = rings(parts[0])
    assert len(vertices) == CUT_VERTICES
    assert any(lat == pole for _, lat in vertices)
    assert parts[0].bounds == pytest.approx(
        (-SEAM_LON, min(pole, far_lat), SEAM_LON, max(pole, far_lat)),
        abs=1e-3,
    )


def test_a_cap_holding_both_poles_is_the_world_less_what_it_misses() -> None:
    # The case the winding cannot tell from no pole at all: the walk closes on itself either way,
    # and what it bounds here is the one region the cap does *not* cover. Drawn as a polygon with a
    # hole, so the hole's vertices are the cap's own boundary and the exterior is the world.
    shape = built(WHOLE, WHOLE_RADIUS_KM)
    assert enclosed_poles(shape) == (POLE_LAT, -POLE_LAT)

    parts = drawn(shape)
    assert len(parts) == 1
    assert len(parts[0].exterior.coords) == WORLD_VERTICES
    assert len(parts[0].interiors) == 1
    assert parts[0].bounds == (-SEAM_LON, -POLE_LAT, SEAM_LON, POLE_LAT)
    for lon, lat in parts[0].interiors[0].coords:
        assert great_circle_km(WHOLE, Coordinate(lat=lat, lon=lon)) == pytest.approx(
            WHOLE_RADIUS_KM,
            abs=1e-6,
        )


def test_a_cap_crossing_nothing_synthesises_no_vertex() -> None:
    # The only case where one assertion would have done for both objects, and the reason it cannot
    # be the only case tested.
    parts = drawn(built(CLEAN, CLEAN_RADIUS_KM))
    assert len(parts) == 1
    assert len(rings(parts[0])) == RING_VERTICES
    assert parts[0].bounds == pytest.approx((65.130, 0.322, 134.870, 59.678), abs=1e-3)
    for lon, lat in rings(parts[0]):
        assert great_circle_km(CLEAN, Coordinate(lat=lat, lon=lon)) == pytest.approx(
            CLEAN_RADIUS_KM,
            abs=1e-6,
        )


@pytest.mark.parametrize(("centre", "radius_km"), CASES, ids=CASE_IDS)
def test_no_drawn_vertex_lies_outside_the_cap(centre: Coordinate, radius_km: float) -> None:
    # The one distance claim that survives synthesised vertices. Asserting each is *at* the radius
    # would fail on precisely the cases this primitive exists to get right.
    for polygon in drawn(built(centre, radius_km)):
        for lon, lat in rings(polygon):
            assert great_circle_km(centre, Coordinate(lat=lat, lon=lon)) <= radius_km + CUT_SLACK_KM
