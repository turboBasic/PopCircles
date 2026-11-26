import math

import pytest
from shapely import get_coordinates
from shapely.geometry import MultiPolygon, Polygon, box

from population_circles.documents.result_document import Coordinate
from population_circles.geography.circle_geometry import (
    HALF_TURN_DEG,
    METRES_PER_KM,
    WORLD,
    cap,
    geographic,
    linestrings,
    polygons,
)
from population_circles.maps.frame import (
    GRATICULE_STEP_DEG,
    LIMB_SHAVE,
    ORTHOGRAPHIC,
    PLATE_CARREE,
    CountryFrameSettings,
    Frame,
    country_frame,
    frame,
    graticule,
    project,
)

EARTH_RADIUS_KM = 6371.0088
CENTRE = Coordinate(lat=25.125, lon=79.708)
ANTIPODE = Coordinate(lat=-CENTRE.lat, lon=CENTRE.lon - 180.0)
SEAM = Coordinate(lat=10.0, lon=178.0)
RADIUS_KM = 1000.0
SEAM_RADIUS_KM = 3000.0
SEAM_PARTS = 2

# Both frames drawn for one cap: the globe's own radius in metres, and what the projected cap of
# that radius spans — R sin(1000 / R) rather than 1 000 000, because an orthographic figure is a
# view of a sphere and not a plan of it.
GLOBE_METRES = EARTH_RADIUS_KM * METRES_PER_KM
CAP_METRES = 995898.928

# What the shave costs at the limb, measured: the horizon falls this far short of the globe's
# radius. It is the figure `LIMB_SHAVE`'s own comment quotes, checked here rather than asserted
# there.
LIMB_GAP_METRES = 0.0786

MERIDIANS = 13
PARALLELS = 5


def view(projection: str, centre: Coordinate, radius_km: float) -> Frame:
    return frame(projection, cap(centre, radius_km, EARTH_RADIUS_KM))


@pytest.mark.parametrize("projection", ["unknown", "", "Orthographic"])
def test_an_unknown_projection_is_refused(projection: str) -> None:
    with pytest.raises(ValueError, match=r"unsupported projection.*plate-carree, orthographic"):
        view(projection, CENTRE, RADIUS_KM)


def test_the_plate_carree_frame_is_longitude_and_latitude_themselves() -> None:
    # Nothing to project: the axes' x and y *are* degrees, so the frame is the world rectangle and a
    # coordinate reaches it unchanged. Which is why the equal aspect the renderer sets is the whole
    # of what makes the figure equirectangular.
    built = view(PLATE_CARREE, CENTRE, RADIUS_KM)
    assert built.horizon == WORLD
    assert built.to_frame.transform(CENTRE.lon, CENTRE.lat) == (CENTRE.lon, CENTRE.lat)
    assert built.circle.equals(geographic(cap(CENTRE, RADIUS_KM, EARTH_RADIUS_KM)))


def test_the_orthographic_frame_is_the_globe_seen_from_the_centre() -> None:
    built = view(ORTHOGRAPHIC, CENTRE, RADIUS_KM)
    assert built.to_frame.transform(CENTRE.lon, CENTRE.lat) == pytest.approx((0.0, 0.0), abs=1e-6)
    west, south, east, north = built.horizon.bounds
    assert (-west, -south, east, north) == pytest.approx((GLOBE_METRES,) * 4, abs=1.0)
    assert GLOBE_METRES - east == pytest.approx(LIMB_GAP_METRES, abs=1e-4)
    assert built.circle.bounds == pytest.approx((-CAP_METRES, -CAP_METRES, CAP_METRES, CAP_METRES))


def test_geographic_footprint_covers_a_global_orthographic_view() -> None:
    built = view(ORTHOGRAPHIC, CENTRE, RADIUS_KM)

    footprint = built.geographic_footprint

    assert footprint.equals(built.visible)


def test_geographic_footprint_stays_inside_a_zoomed_orthographic_view() -> None:
    outline = box(1.4, 42.4, 1.8, 42.7)
    built = cap(Coordinate(lat=42.55, lon=1.6), 1.0, EARTH_RADIUS_KM)
    country = country_frame(built, outline)

    footprint = country.geographic_footprint

    assert footprint.within(country.visible)
    assert footprint.bounds[2] - footprint.bounds[0] < 1
    assert footprint.bounds[3] - footprint.bounds[1] < 1


def test_an_orthographic_cap_at_the_seam_carries_no_cut() -> None:
    # The reason the circle reaches an orthographic frame through the projection it was built in:
    # the ±180 meridian is an edge of one frame and an ordinary line inside the other, so the two
    # parts the geographic form is cut into would be stroked as a seam across the middle of the
    # fill.
    built = cap(SEAM, SEAM_RADIUS_KM, EARTH_RADIUS_KM)
    assert len(polygons(geographic(built))) == SEAM_PARTS
    assert len(polygons(frame(ORTHOGRAPHIC, built).circle)) == 1


def test_the_far_side_of_the_globe_is_not_drawn() -> None:
    # A cap around the antipode is exactly what an orthographic frame cannot show, and PROJ answers
    # infinity for every point of it rather than refusing — so the clip is what has to drop it.
    built = view(ORTHOGRAPHIC, CENTRE, RADIUS_KM)
    hidden = geographic(cap(ANTIPODE, RADIUS_KM, EARTH_RADIUS_KM))
    assert project(built, hidden).is_empty


def test_every_projected_coordinate_is_finite_and_inside_the_globe() -> None:
    # The claim the shave exists to make true. Against the globe's radius rather than the horizon's,
    # because a vertex the clip synthesised sits a little past the shaved limb — 2.7 mm here — and
    # what matters is that no coordinate escapes the sphere or comes back as infinity.
    built = view(ORTHOGRAPHIC, CENTRE, RADIUS_KM)
    for line in linestrings(project(built, graticule())):
        for x, y in line.coords:
            assert math.isfinite(x)
            assert math.isfinite(y)
            assert math.hypot(x, y) <= GLOBE_METRES


def test_the_graticule_is_meridians_and_parallels_at_a_fixed_step() -> None:
    lines = list(graticule().geoms)
    assert len(lines) == MERIDIANS + PARALLELS
    for line in lines[:MERIDIANS]:
        lons = {lon for lon, _ in line.coords}
        assert len(lons) == 1
        assert lons.pop() % GRATICULE_STEP_DEG == 0.0
    for line in lines[MERIDIANS:]:
        lats = {lat for _, lat in line.coords}
        assert len(lats) == 1
        # No parallel at either pole, where one is a single point.
        assert abs(lats.pop()) < GRATICULE_STEP_DEG * PARALLELS / 2.0


def test_the_shave_is_a_fraction_of_the_radius_it_shortens() -> None:
    # Stated as a fraction so it is the same 1 km whatever earth model a document carries, which is
    # the property that keeps the renderer free of a sphere of its own.
    assert LIMB_SHAVE * EARTH_RADIUS_KM * math.pi / 2.0 == pytest.approx(1.0, abs=1e-3)


@pytest.mark.parametrize(
    ("centre", "outline"),
    [
        (
            Coordinate(lat=50.0, lon=10.0),
            MultiPolygon(
                [
                    box(8, 49, 10, 51),
                    box(11, 49, 12, 50),
                ],
            ),
        ),
        (
            Coordinate(lat=10.0, lon=179.0),
            Polygon([(170, 8), (-170, 8), (-170, 12), (170, 12), (170, 8)]),
        ),
        (
            Coordinate(lat=82.0, lon=20.0),
            box(10, 78, 30, 86),
        ),
    ],
    ids=["multipart", "antimeridian", "high-latitude"],
)
def test_country_frame_contains_its_projected_outline_and_circle(
    centre: Coordinate,
    outline: Polygon | MultiPolygon,
) -> None:
    built = cap(centre, 500.0, EARTH_RADIUS_KM)
    view = country_frame(built, outline)
    for geometry in (view.country_outline, view.circle):
        assert geometry is not None
        assert all(
            math.isfinite(value) for coordinate in get_coordinates(geometry) for value in coordinate
        )
        assert view.horizon.covers(geometry)


def test_country_frame_uses_an_explicit_orthographic_centre() -> None:
    centre = Coordinate(lat=44.0, lon=-96.0)
    built = cap(Coordinate(lat=40.0, lon=-100.0), 500.0, EARTH_RADIUS_KM)
    outline = box(-125, 25, -66, 49)

    view = country_frame(
        built,
        outline,
        CountryFrameSettings(
            projection=ORTHOGRAPHIC,
            projection_parameters={"lat_0": centre.lat, "lon_0": centre.lon},
        ),
    )

    assert view.to_frame.transform(centre.lon, centre.lat) == pytest.approx((0.0, 0.0), abs=1e-6)
    assert view.country_outline is not None
    assert view.horizon.covers(view.country_outline)
    assert view.horizon.covers(view.circle)


def test_country_frame_dissolves_outline_parts_meeting_at_the_antimeridian() -> None:
    built = cap(Coordinate(lat=65.0, lon=179.0), 100.0, EARTH_RADIUS_KM)
    outline = MultiPolygon(
        [
            Polygon([(170, 60), (179.999988, 60), (179.999988, 70), (170, 70), (170, 60)]),
            Polygon(
                [(-179.999988, 60), (-170, 60), (-170, 70), (-179.999988, 70), (-179.999988, 60)]
            ),
        ],
    )

    view = country_frame(
        built,
        outline,
        CountryFrameSettings(
            projection="albers-equal-area",
            projection_parameters={"lon_0": 180.0, "lat_0": 65.0, "lat_1": 50.0, "lat_2": 70.0},
        ),
    )

    assert view.country_outline is not None
    assert view.country_outline.geom_type == "Polygon"
    footprint = polygons(view.geographic_footprint)
    assert any(part.bounds[0] > HALF_TURN_DEG - GRATICULE_STEP_DEG for part in footprint)
    assert any(part.bounds[2] < -HALF_TURN_DEG + GRATICULE_STEP_DEG for part in footprint)


def test_country_frame_fits_a_plate_carree_outline_and_circle() -> None:
    built = cap(Coordinate(lat=-40.0, lon=-72.0), 200.0, EARTH_RADIUS_KM)
    outline = box(-75, -45, -70, -35)

    view = country_frame(built, outline, CountryFrameSettings(projection=PLATE_CARREE))

    assert view.country_outline is outline
    assert view.to_frame.transform(-72.0, -40.0) == (-72.0, -40.0)
    assert view.horizon.covers(view.country_outline)
    assert view.horizon.covers(view.circle)
    west, south, east, north = outline.union(view.circle).bounds
    assert view.horizon.bounds == pytest.approx(
        (
            west - (east - west) * 0.02,
            south - (north - south) * 0.02,
            east + (east - west) * 0.02,
            north + (north - south) * 0.02,
        ),
    )


@pytest.mark.parametrize(
    ("projection", "centre", "parameters"),
    [
        (
            "albers-equal-area",
            Coordinate(lat=45.0, lon=-125.0),
            {"lon_0": -125.0, "lat_0": 45.0, "lat_1": 35.0, "lat_2": 60.0},
        ),
        (
            "transverse-mercator",
            Coordinate(lat=-38.0, lon=-71.0),
            {"lon_0": -71.0, "lat_0": -38.0, "k_0": 1.0},
        ),
    ],
)
def test_country_frame_applies_projection_parameters_and_exact_extent(
    projection: str, centre: Coordinate, parameters: dict[str, float]
) -> None:
    built = cap(centre, 100.0, EARTH_RADIUS_KM)
    outline = box(centre.lon - 1, centre.lat - 1, centre.lon + 1, centre.lat + 1)
    extent = (-1_000_000.0, -1_000_000.0, 1_000_000.0, 1_000_000.0)

    view = country_frame(
        built,
        outline,
        CountryFrameSettings(
            projection=projection,
            projection_parameters=parameters,
            extent=extent,
        ),
    )

    assert view.to_frame.transform(centre.lon, centre.lat) == pytest.approx((0.0, 0.0), abs=1e-6)
    assert view.horizon.bounds == extent


def test_country_frame_refuses_a_large_circle_beyond_the_horizon() -> None:
    built = cap(Coordinate(lat=51.0, lon=10.0), 15_000.0, EARTH_RADIUS_KM)

    with pytest.raises(ValueError, match="extends beyond the orthographic projection domain"):
        country_frame(built, box(9, 50, 11, 52))
