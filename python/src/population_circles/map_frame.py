import math
from dataclasses import dataclass

from pyproj import CRS, Transformer
from shapely import get_coordinates
from shapely.geometry import MultiLineString, Point, box
from shapely.geometry.base import BaseGeometry
from shapely.ops import transform

from population_circles.circle_geometry import (
    HALF_TURN_DEG,
    METRES_PER_KM,
    POLE_LAT,
    QUAD_SEGS,
    WORLD,
    Cap,
    cap,
    geographic,
    hemisphere_km,
    projected_crs,
)

PLATE_CARREE = "plate-carree"
ORTHOGRAPHIC = "orthographic"
PROJECTIONS = (PLATE_CARREE, ORTHOGRAPHIC)

# The orthographic clip stops this far short of the limb, as a fraction of the radius reaching it —
# 1 km on this sphere. Cutting a shape against a polygon synthesises vertices on lines straight in
# longitude and latitude, and one of those lands measurably outside the arc it stands in for: 17 m
# for the hemisphere itself. A point beyond the limb has no orthographic image, so PROJ answers
# infinity and the geometry carrying it is lost rather than clipped. What the shave costs is a gap
# of R(1 - cos) at the limb — 78 mm, on a figure 12 742 km across.
LIMB_SHAVE = 1e-4

GRATICULE_STEP_DEG = 30
# A meridian and a parallel are straight only in the frame they are stated in, so each is walked at
# this step rather than drawn end to end.
GRATICULE_TRACE_DEG = 2


@dataclass(frozen=True)
class Frame:
    # What the axes' own x and y are, and the transform reaching them from longitude and latitude.
    crs: CRS
    to_frame: Transformer
    # What the frame can show, in longitude and latitude. Clipping against it is what keeps the far
    # side of the globe off an orthographic figure.
    visible: BaseGeometry
    # The circle and the outline of the frame itself, both already in the frame's coordinates.
    circle: BaseGeometry
    horizon: BaseGeometry
    country_outline: BaseGeometry | None = None


def plate_carree(built: Cap) -> Frame:
    return Frame(
        crs=built.geodetic,
        to_frame=Transformer.from_crs(built.geodetic, built.geodetic, always_xy=True),
        visible=WORLD,
        circle=geographic(built),
        horizon=WORLD,
    )


def orthographic(built: Cap) -> Frame:
    # The circle reaches this frame through the projection it was built in rather than through
    # longitude and latitude: the geographic form carries a cut down the ±180 meridian, and that
    # meridian is not an edge of an orthographic figure, so drawing that form strokes a seam across
    # the middle of the fill.
    face = projected_crs("ortho", built.centre, built.earth_radius_km)
    to_face = Transformer.from_crs(built.source, face, always_xy=True)
    limb_km = hemisphere_km(built.earth_radius_km) * (1.0 - LIMB_SHAVE)
    disc = Point(0.0, 0.0).buffer(limb_km * METRES_PER_KM, quad_segs=QUAD_SEGS)
    return Frame(
        crs=face,
        to_frame=Transformer.from_crs(built.geodetic, face, always_xy=True),
        visible=geographic(cap(built.centre, limb_km, built.earth_radius_km)),
        circle=transform(to_face.transform, built.polygon.intersection(disc)),
        horizon=transform(to_face.transform, disc),
    )


def country_frame(built: Cap, country_outline: BaseGeometry) -> Frame:
    coordinates = get_coordinates(country_outline)
    longitudes = [math.radians(point[0]) for point in coordinates]
    centre_lon = math.degrees(
        math.atan2(sum(map(math.sin, longitudes)), sum(map(math.cos, longitudes))),
    )
    centre_lat = math.fsum(point[1] for point in coordinates) / len(coordinates)
    centre = type(built.centre)(lat=centre_lat, lon=centre_lon)
    face = projected_crs("ortho", centre, built.earth_radius_km)
    to_face = Transformer.from_crs(built.geodetic, face, always_xy=True)

    def unwrap(x: float, y: float, z: float | None = None) -> tuple[float, ...]:
        lon = centre_lon + (x - centre_lon + HALF_TURN_DEG) % (2 * HALF_TURN_DEG) - HALF_TURN_DEG
        return (lon, y) if z is None else (lon, y, z)

    outline = transform(to_face.transform, transform(unwrap, country_outline))
    circle_to_face = Transformer.from_crs(built.source, face, always_xy=True)
    circle = transform(circle_to_face.transform, built.polygon)
    west = min(outline.bounds[0], circle.bounds[0])
    south = min(outline.bounds[1], circle.bounds[1])
    east = max(outline.bounds[2], circle.bounds[2])
    north = max(outline.bounds[3], circle.bounds[3])
    return Frame(
        crs=face,
        to_frame=to_face,
        visible=WORLD,
        circle=circle,
        horizon=box(west, south, east, north),
        country_outline=outline,
    )


def frame(projection: str, built: Cap) -> Frame:
    if projection == ORTHOGRAPHIC:
        return orthographic(built)
    if projection == PLATE_CARREE:
        return plate_carree(built)
    message = f"unsupported projection {projection!r}; choose from {', '.join(PROJECTIONS)}"
    raise ValueError(message)


def project(view: Frame, geometry: BaseGeometry) -> BaseGeometry:
    return transform(view.to_frame.transform, geometry.intersection(view.visible))


def trace(start: int, stop: int) -> list[int]:
    return [*range(start, stop, GRATICULE_TRACE_DEG), stop]


def graticule() -> MultiLineString:
    # No parallel at either pole: one is a single point, and the meridians already meet on it.
    lat_limit, lon_limit = int(POLE_LAT), int(HALF_TURN_DEG)
    meridians = [
        [(float(lon), float(lat)) for lat in trace(-lat_limit, lat_limit)]
        for lon in range(-lon_limit, lon_limit + 1, GRATICULE_STEP_DEG)
    ]
    parallels = [
        [(float(lon), float(lat)) for lon in trace(-lon_limit, lon_limit)]
        for lat in range(-lat_limit + GRATICULE_STEP_DEG, lat_limit, GRATICULE_STEP_DEG)
    ]
    return MultiLineString(meridians + parallels)
