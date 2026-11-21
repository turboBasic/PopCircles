import math
from collections.abc import Callable, Iterable, Mapping
from dataclasses import dataclass
from typing import cast

from pyproj import CRS, Transformer
from shapely import (
    get_coordinates,
    set_precision,  # pyright: ignore[reportUnknownVariableType] — Shapely leaves this overload partially typed
    transform,
)
from shapely.geometry import MultiLineString, Point, box
from shapely.geometry.base import BaseGeometry
from shapely.ops import unary_union

from population_circles.circle_document import Coordinate
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

# Shapely's stubs omit the documented interleaved=False callback and include_z=None options.
_transform = cast(Callable[..., BaseGeometry], transform)
_set_precision = cast(Callable[..., BaseGeometry], set_precision)

PLATE_CARREE = "plate-carree"
ORTHOGRAPHIC = "orthographic"
PROJECTIONS = (PLATE_CARREE, ORTHOGRAPHIC)
COUNTRY_PROJECTIONS = (*PROJECTIONS, "albers-equal-area", "transverse-mercator")

# The orthographic clip stops this far short of the limb, as a fraction of the radius reaching it —
# 1 km on this sphere. Cutting a shape against a polygon synthesises vertices on lines straight in
# longitude and latitude, and one of those lands measurably outside the arc it stands in for: 17 m
# for the hemisphere itself. A point beyond the limb has no orthographic image, so PROJ answers
# infinity and the geometry carrying it is lost rather than clipped. What the shave costs is a gap
# of R(1 - cos) at the limb — 78 mm, on a figure 12 742 km across.
LIMB_SHAVE = 1e-4

GRATICULE_STEP_DEG = 30
ANTIMERIDIAN_LIMIT_DEG = 179.9
ANTIMERIDIAN_SNAP_METRES = 25.0
# Each side gets this share of the content width or height when no extent is configured.
COUNTRY_FRAME_MARGIN = 0.02
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


@dataclass(frozen=True)
class CountryFrameSettings:
    projection: str = ORTHOGRAPHIC
    projection_parameters: Mapping[str, float] | None = None
    extent: tuple[float, float, float, float] | None = None


def _country_extent(bounds: tuple[float, float, float, float]) -> tuple[float, float, float, float]:
    west, south, east, north = bounds
    x_margin = (east - west) * COUNTRY_FRAME_MARGIN
    y_margin = (north - south) * COUNTRY_FRAME_MARGIN
    return west - x_margin, south - y_margin, east + x_margin, north + y_margin


def _dissolve_projected_outline(outline: BaseGeometry, source: BaseGeometry) -> BaseGeometry:
    if source.bounds[0] < -ANTIMERIDIAN_LIMIT_DEG and source.bounds[2] > ANTIMERIDIAN_LIMIT_DEG:
        # GISCO's two antimeridian cuts are a few metres apart; close that display-only seam.
        outline = _set_precision(outline, grid_size=ANTIMERIDIAN_SNAP_METRES)
    return unary_union(outline)


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
        circle=_transform(
            built.polygon.intersection(disc),
            to_face.transform,
            include_z=None,
            interleaved=False,
        ),
        horizon=_transform(disc, to_face.transform, include_z=None, interleaved=False),
    )


def country_frame(
    built: Cap,
    country_outline: BaseGeometry,
    settings: CountryFrameSettings | None = None,
) -> Frame:
    settings = settings or CountryFrameSettings()
    projection = settings.projection
    extent = settings.extent
    parameters = settings.projection_parameters or {}
    if projection not in COUNTRY_PROJECTIONS:
        message = (
            f"unsupported projection {projection!r}; choose from {', '.join(COUNTRY_PROJECTIONS)}"
        )
        raise ValueError(message)
    if projection == PLATE_CARREE:
        world = plate_carree(built)
        circle = world.circle
        if not all(
            math.isfinite(value)
            for geometry in (country_outline, circle)
            for point in get_coordinates(geometry)
            for value in point
        ):
            message = "country circle or outline contains a non-finite coordinate"
            raise ValueError(message)
        west = min(country_outline.bounds[0], circle.bounds[0])
        south = min(country_outline.bounds[1], circle.bounds[1])
        east = max(country_outline.bounds[2], circle.bounds[2])
        north = max(country_outline.bounds[3], circle.bounds[3])
        return Frame(
            crs=world.crs,
            to_frame=world.to_frame,
            visible=world.visible,
            circle=circle,
            horizon=(
                box(*extent)
                if extent is not None
                else box(*_country_extent((west, south, east, north)))
            ),
            country_outline=country_outline,
        )

    coordinates = get_coordinates(country_outline)
    centre: Coordinate | None = None
    if centre is None:
        if "lat_0" in parameters and "lon_0" in parameters:
            centre = Coordinate(lat=parameters["lat_0"], lon=parameters["lon_0"])
        else:
            longitudes = [math.radians(point[0]) for point in coordinates]
            centre = type(built.centre)(
                lat=math.fsum(point[1] for point in coordinates) / len(coordinates),
                lon=math.degrees(
                    math.atan2(sum(map(math.sin, longitudes)), sum(map(math.cos, longitudes))),
                ),
            )
    centre_lon = centre.lon
    face = projected_crs(projection, centre, built.earth_radius_km, parameters)
    to_face = Transformer.from_crs(built.geodetic, face, always_xy=True)

    def unwrap_lon(lon: float) -> float:
        return centre_lon + (lon - centre_lon + HALF_TURN_DEG) % (2 * HALF_TURN_DEG) - HALF_TURN_DEG

    def unwrap(
        x: Iterable[float],
        y: Iterable[float],
        z: Iterable[float] | None = None,
    ) -> tuple[list[float], ...]:
        if z is None:
            coordinates = list(zip(x, y, strict=True))
            return (
                [unwrap_lon(lon) for lon, _ in coordinates],
                [lat for _, lat in coordinates],
            )
        coordinates = list(zip(x, y, z, strict=True))
        return (
            [unwrap_lon(lon) for lon, _, _ in coordinates],
            [lat for _, lat, _ in coordinates],
            [altitude for _, _, altitude in coordinates],
        )

    outline = _transform(
        _transform(country_outline, unwrap, include_z=None, interleaved=False),
        to_face.transform,
        include_z=None,
        interleaved=False,
    )
    outline = _dissolve_projected_outline(outline, country_outline)
    circle_to_face = Transformer.from_crs(built.source, face, always_xy=True)
    circle = _transform(
        built.polygon,
        circle_to_face.transform,
        include_z=None,
        interleaved=False,
    )
    if any(
        not math.isfinite(value)
        for geometry in (outline, circle)
        for point in get_coordinates(geometry)
        for value in point
    ):
        message = f"country circle or outline extends beyond the {projection} projection domain"
        raise ValueError(message)
    west = min(outline.bounds[0], circle.bounds[0])
    south = min(outline.bounds[1], circle.bounds[1])
    east = max(outline.bounds[2], circle.bounds[2])
    north = max(outline.bounds[3], circle.bounds[3])
    visible = (
        geographic(
            cap(
                centre,
                hemisphere_km(built.earth_radius_km) * (1.0 - LIMB_SHAVE),
                built.earth_radius_km,
            ),
        )
        if projection == ORTHOGRAPHIC
        else WORLD
    )
    return Frame(
        crs=face,
        to_frame=to_face,
        visible=visible,
        circle=circle,
        horizon=(
            box(*extent)
            if extent is not None
            else box(*_country_extent((west, south, east, north)))
        ),
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
    return _transform(
        geometry.intersection(view.visible),
        view.to_frame.transform,
        include_z=None,
        interleaved=False,
    )


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
