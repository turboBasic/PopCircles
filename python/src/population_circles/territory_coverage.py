import math
from collections.abc import Iterable

from pyproj import CRS, Transformer
from shapely.affinity import scale, translate
from shapely.geometry import MultiPolygon, Point, Polygon, box
from shapely.geometry.base import BaseGeometry
from shapely.ops import unary_union

from population_circles.circle_document import Circle, Coordinate
from population_circles.circle_geometry import (
    METRES_PER_KM,
    QUAD_SEGS,
    cap,
    geodetic_crs,
    geographic,
)

HALF_TURN_DEG = 180.0
WORLD = box(-HALF_TURN_DEG, -90.0, HALF_TURN_DEG, 90.0)


def _unwrap_ring(points: Iterable[tuple[float, float]]) -> list[tuple[float, float]]:
    unwrapped: list[tuple[float, float]] = []
    turns = 0
    previous: float | None = None
    for lon, lat in points:
        if previous is not None:
            if lon - previous > HALF_TURN_DEG:
                turns -= 360.0
            elif lon - previous < -HALF_TURN_DEG:
                turns += 360.0
        unwrapped.append((lon + turns, lat))
        previous = lon
    return unwrapped


def _relative_lon(lon: float, central_meridian: float) -> float:
    return (lon - central_meridian + HALF_TURN_DEG) % 360.0 - HALF_TURN_DEG


def _world_parts(geometry: BaseGeometry, central_meridian: float) -> tuple[Polygon, ...]:
    if isinstance(geometry, Polygon):
        source = (geometry,)
    elif isinstance(geometry, MultiPolygon):
        source = geometry.geoms
    else:
        message = "territory geometry must be a polygon or multipolygon"
        raise TypeError(message)
    parts: list[Polygon] = []
    for polygon in source:
        exterior = _unwrap_ring(
            [
                (_relative_lon(float(point[0]), central_meridian), float(point[1]))
                for point in polygon.exterior.coords
            ]
        )
        middle = sum(lon for lon, _ in exterior[:-1]) / (len(exterior) - 1)
        holes: list[list[tuple[float, float]]] = []
        for ring in polygon.interiors:
            hole = _unwrap_ring(
                [
                    (_relative_lon(float(point[0]), central_meridian), float(point[1]))
                    for point in ring.coords
                ]
            )
            hole_middle = sum(lon for lon, _ in hole[:-1]) / (len(hole) - 1)
            shift = 360.0 * round((middle - hole_middle) / 360.0)
            holes.append([(lon + shift, lat) for lon, lat in hole])
        unwrapped = Polygon(exterior, holes)
        for offset in (-360.0, 0.0, 360.0):
            clipped = translate(unwrapped, xoff=offset).intersection(WORLD)
            if isinstance(clipped, Polygon):
                parts.append(translate(clipped, xoff=central_meridian))
            elif isinstance(clipped, MultiPolygon):
                parts.extend(translate(part, xoff=central_meridian) for part in clipped.geoms)
    return tuple(parts)


def _project_points(
    points: Iterable[tuple[float, float]], transformer: Transformer
) -> list[tuple[float, float]]:
    return [transformer.transform(lon, lat) for lon, lat in points]


def _project_polygon(polygon: Polygon, transformer: Transformer) -> Polygon:
    exterior = _project_points(
        [(float(point[0]), float(point[1])) for point in polygon.exterior.coords], transformer
    )
    holes = [
        _project_points([(float(point[0]), float(point[1])) for point in ring.coords], transformer)
        for ring in polygon.interiors
    ]
    return Polygon(exterior, holes)


def _projected_circle(
    circle: Circle, transformer: Transformer, central_meridian: float
) -> BaseGeometry:
    earth_radius_km = circle.earth_radius_km
    hemisphere_km = math.pi * earth_radius_km / 2.0
    if circle.radius_km <= hemisphere_km:
        parts = _world_parts(cap_geometry(circle), central_meridian)
        return unary_union([_project_polygon(part, transformer) for part in parts])

    antipode = Coordinate(
        lat=-circle.centre.lat,
        lon=(circle.centre.lon + 360.0) % 360.0 - 180.0,
    )
    complement = Circle(
        centre=antipode,
        radius_km=math.pi * earth_radius_km - circle.radius_km,
        earth_radius_km=earth_radius_km,
    )
    complement_parts = _world_parts(cap_geometry(complement), central_meridian)
    excluded = unary_union([_project_polygon(part, transformer) for part in complement_parts])
    radial_m = (
        math.sqrt(2.0) * earth_radius_km * METRES_PER_KM / math.cos(math.pi / (4 * QUAD_SEGS))
    )
    world = scale(
        Point(0.0, 0.0).buffer(radial_m, quad_segs=QUAD_SEGS),
        xfact=2.0,
        yfact=1.0,
    )
    return world.difference(excluded)


def cap_geometry(circle: Circle) -> BaseGeometry:
    return geographic(cap(circle.centre, circle.radius_km, circle.earth_radius_km))


def equal_area_crs(earth_radius_km: float, central_meridian: float = 0.0) -> CRS:
    return CRS.from_proj4(
        f"+proj=moll +lon_0={central_meridian} +R={earth_radius_km * METRES_PER_KM} "
        "+units=m +no_defs",
    )


def territory_share(circle: Circle, territories: Iterable[BaseGeometry]) -> float:
    earth_radius_km = circle.earth_radius_km
    radius_km = circle.radius_km
    if not math.isfinite(earth_radius_km) or earth_radius_km <= 0.0:
        message = "earth radius must be finite and positive"
        raise ValueError(message)
    if not math.isfinite(radius_km) or radius_km <= 0.0 or radius_km >= math.pi * earth_radius_km:
        message = "circle radius must be finite and between zero and half the circumference"
        raise ValueError(message)

    hemisphere_km = math.pi * earth_radius_km / 2.0
    central_meridian = circle.centre.lon
    if radius_km > hemisphere_km:
        central_meridian += 180.0
    central_meridian = (central_meridian + 180.0) % 360.0 - 180.0
    crs = equal_area_crs(earth_radius_km, central_meridian)
    to_equal_area = Transformer.from_crs(geodetic_crs(earth_radius_km), crs, always_xy=True)
    territory_parts: list[BaseGeometry] = []
    for territory in territories:
        if territory.is_empty or not territory.is_valid:
            message = "territory geometry must be non-empty and valid"
            raise ValueError(message)
        parts = _world_parts(territory, central_meridian)
        projected = [_project_polygon(part, to_equal_area) for part in parts]
        territory_parts.extend(part for part in projected if not part.is_empty)
    land = unary_union(territory_parts)
    territory_area = land.area
    if not math.isfinite(territory_area) or territory_area <= 0.0:
        message = "territory geometry must have finite positive area"
        raise ValueError(message)

    projected_circle = _projected_circle(circle, to_equal_area, central_meridian)
    share = land.intersection(projected_circle).area / territory_area
    if not math.isfinite(share) or not 0.0 <= share <= 1.0:
        message = "territory coverage must be finite and within [0, 1]"
        raise ValueError(message)

    boundary_radius_km = min(radius_km, math.pi * earth_radius_km - radius_km)
    sagitta_km = boundary_radius_km * (1.0 - math.cos(math.pi / (4 * QUAD_SEGS)))
    angle = boundary_radius_km / earth_radius_km
    inner_angle = max(0.0, angle - sagitta_km / earth_radius_km)
    annulus_km2 = 2.0 * math.pi * earth_radius_km**2 * (math.cos(inner_angle) - math.cos(angle))
    error_bound = annulus_km2 / territory_area
    if not math.isfinite(error_bound):
        message = "territory coverage error bound must be finite"
        raise ValueError(message)
    share_error_bound = min(1.0, error_bound)
    if not 0.0 <= share_error_bound <= 1.0:
        message = "territory coverage error bound must be within [0, 1]"
        raise ValueError(message)
    return share
