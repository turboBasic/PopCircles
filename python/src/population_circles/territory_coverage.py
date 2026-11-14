import math
from collections.abc import Iterable, Mapping
from dataclasses import dataclass
from typing import cast

from pyproj import CRS, Transformer
from shapely import make_valid, segmentize
from shapely.affinity import scale, translate
from shapely.geometry import MultiPolygon, Point, Polygon, box
from shapely.geometry.base import BaseGeometry
from shapely.ops import unary_union

from population_circles.circle_document import (
    Answer,
    Caption,
    Circle,
    Coordinate,
    CountryIdentity,
    Envelope,
    Provenance,
    circle_of,
)
from population_circles.circle_geometry import (
    METRES_PER_KM,
    POLE_LAT,
    QUAD_SEGS,
    cap,
    geodetic_crs,
    geographic,
)
from population_circles.country_outline import load_country_boundaries, load_map_boundaries
from population_circles.dataset_registry import Registry, load

HALF_TURN_DEG = 180.0
MAX_SOURCE_SEGMENT_DEG = 0.01
GEOGRAPHIC_CUT_SLACK_KM = 0.01
SHARE_NUMERICAL_TOLERANCE = 1e-12
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
    winding = unwrapped[-1][0] - unwrapped[0][0]
    pole_indices = [index for index, (_, lat) in enumerate(unwrapped) if abs(lat) == POLE_LAT]
    if pole_indices and abs(winding) >= HALF_TURN_DEG * 2.0:
        correction = -HALF_TURN_DEG * 2.0 * round(winding / (HALF_TURN_DEG * 2.0))
        for index in range(pole_indices[-1], len(unwrapped)):
            lon, lat = unwrapped[index]
            unwrapped[index] = (lon + correction, lat)
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
                parts.append(clipped)
            elif isinstance(clipped, MultiPolygon):
                parts.extend(clipped.geoms)
    return tuple(parts)


def _project_points(
    points: Iterable[tuple[float, float]], transformer: Transformer
) -> list[tuple[float, float]]:
    return [transformer.transform(lon, lat) for lon, lat in points]


def _project_polygon(
    polygon: Polygon,
    transformer: Transformer,
    max_segment_degrees: float = MAX_SOURCE_SEGMENT_DEG,
) -> Polygon:
    densified = segmentize(polygon, max_segment_length=max_segment_degrees)
    exterior = _project_points(
        [(float(point[0]), float(point[1])) for point in densified.exterior.coords], transformer
    )
    holes = [
        _project_points([(float(point[0]), float(point[1])) for point in ring.coords], transformer)
        for ring in densified.interiors
    ]
    return Polygon(exterior, holes)


def _polygon_parts(geometry: BaseGeometry) -> Iterable[Polygon]:
    if isinstance(geometry, Polygon):
        yield geometry
    elif isinstance(geometry, MultiPolygon):
        yield from geometry.geoms
    else:
        for part in getattr(geometry, "geoms", ()):
            yield from _polygon_parts(part)


def _projected_circle(
    circle: Circle,
    transformer: Transformer,
    central_meridian: float,
    max_segment_degrees: float = MAX_SOURCE_SEGMENT_DEG,
) -> BaseGeometry:
    earth_radius_km = circle.earth_radius_km
    hemisphere_km = math.pi * earth_radius_km / 2.0
    if circle.radius_km <= hemisphere_km:
        parts = _world_parts(cap_geometry(circle), central_meridian)
        return unary_union(
            [_project_polygon(part, transformer, max_segment_degrees) for part in parts]
        )

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
    excluded = unary_union(
        [_project_polygon(part, transformer, max_segment_degrees) for part in complement_parts]
    )
    radial_m = (
        math.sqrt(2.0) * earth_radius_km * METRES_PER_KM / math.cos(math.pi / (4 * QUAD_SEGS))
    )
    world = scale(
        Point(0.0, 0.0).buffer(radial_m, quad_segs=QUAD_SEGS),
        xfact=2.0,
        yfact=1.0,
    )
    return world.difference(excluded)


def _project_territory(
    territory: BaseGeometry, transformer: Transformer, central_meridian: float
) -> list[BaseGeometry]:
    if territory.is_empty:
        message = "territory geometry must be non-empty"
        raise ValueError(message)
    source = territory if territory.is_valid else make_valid(territory)
    polygons = list(_polygon_parts(source))
    if not polygons:
        message = "territory geometry must contain polygonal area"
        raise ValueError(message)
    projected = [
        _project_polygon(part, transformer)
        for polygon in polygons
        for part in _world_parts(polygon, central_meridian)
    ]
    return [part for part in projected if not part.is_empty]


def cap_geometry(circle: Circle) -> BaseGeometry:
    return geographic(cap(circle.centre, circle.radius_km, circle.earth_radius_km))


def _validate_centre(circle: Circle) -> None:
    centre = circle.centre
    if (
        not math.isfinite(centre.lat)
        or not math.isfinite(centre.lon)
        or not -POLE_LAT <= centre.lat <= POLE_LAT
        or not -HALF_TURN_DEG <= centre.lon <= HALF_TURN_DEG
    ):
        message = "circle centre must have finite geographic coordinates"
        raise ValueError(message)


def equal_area_crs(earth_radius_km: float, central_meridian: float = 0.0) -> CRS:
    return CRS.from_proj4(
        f"+proj=moll +lon_0={central_meridian} +R={earth_radius_km * METRES_PER_KM} "
        "+units=m +no_defs",
    )


def territory_share(circle: Circle, territories: Iterable[BaseGeometry]) -> float:
    earth_radius_km = circle.earth_radius_km
    radius_km = circle.radius_km
    _validate_centre(circle)
    if not math.isfinite(earth_radius_km) or earth_radius_km <= 0.0:
        message = "earth radius must be finite and positive"
        raise ValueError(message)
    if not math.isfinite(radius_km) or radius_km <= 0.0 or radius_km >= math.pi * earth_radius_km:
        message = (
            f"circle radius {radius_km} km must be finite and between zero and "
            f"half the circumference on a {earth_radius_km} km sphere"
        )
        raise ValueError(message)

    hemisphere_km = math.pi * earth_radius_km / 2.0
    central_meridian = circle.centre.lon
    if radius_km > hemisphere_km:
        central_meridian += 180.0
    central_meridian = (central_meridian + 180.0) % 360.0 - 180.0
    crs = equal_area_crs(earth_radius_km)
    to_equal_area = Transformer.from_crs(geodetic_crs(earth_radius_km), crs, always_xy=True)
    territory_parts = [
        part
        for territory in territories
        for part in _project_territory(territory, to_equal_area, central_meridian)
    ]
    land = unary_union(territory_parts)
    territory_area = land.area
    if not math.isfinite(territory_area) or territory_area <= 0.0:
        message = "territory geometry must have finite positive area"
        raise ValueError(message)

    projected_circle = _projected_circle(circle, to_equal_area, central_meridian)
    refined_circle = _projected_circle(
        circle, to_equal_area, central_meridian, MAX_SOURCE_SEGMENT_DEG / 2.0
    )
    share = land.intersection(projected_circle).area / territory_area
    if math.isclose(share, 0.0, abs_tol=SHARE_NUMERICAL_TOLERANCE):
        share = 0.0
    elif math.isclose(share, 1.0, abs_tol=SHARE_NUMERICAL_TOLERANCE):
        share = 1.0
    if not math.isfinite(share) or not 0.0 <= share <= 1.0:
        message = "territory coverage must be finite and within [0, 1]"
        raise ValueError(message)

    boundary_radius_km = min(radius_km, math.pi * earth_radius_km - radius_km)
    sagitta_km = (
        boundary_radius_km * (1.0 - math.cos(math.pi / (4 * QUAD_SEGS))) + GEOGRAPHIC_CUT_SLACK_KM
    )
    angle = boundary_radius_km / earth_radius_km
    inner_angle = max(0.0, angle - sagitta_km / earth_radius_km)
    annulus_km2 = 2.0 * math.pi * earth_radius_km**2 * (math.cos(inner_angle) - math.cos(angle))
    projection_convergence_km2 = (
        projected_circle.symmetric_difference(refined_circle).area / METRES_PER_KM**2
    )
    error_bound = (annulus_km2 + projection_convergence_km2) / territory_area
    if not math.isfinite(error_bound):
        message = "territory coverage error bound must be finite"
        raise ValueError(message)
    share_error_bound = min(1.0, error_bound)
    if not 0.0 <= share_error_bound <= 1.0:
        message = "territory coverage error bound must be within [0, 1]"
        raise ValueError(message)
    return share


@dataclass(frozen=True)
class _Enrichment:
    document: Mapping[str, object]
    earth_radius: float
    registry: Registry
    boundaries: Mapping[str, BaseGeometry] | None


def _enrich_circle(
    circle_data: dict[str, object],
    context: _Enrichment,
    country_data: Mapping[str, object] | None = None,
) -> None:
    circle = Circle.model_validate({**circle_data, "earth_radius_km": context.earth_radius})
    if circle.radius_km == 0.0:
        circle_data.pop("territory_share", None)
        return
    if country_data is not None:
        country = CountryIdentity.model_validate(country_data)
        answer = Answer(
            circle=circle,
            caption=Caption(population=0.0, share=0.0),
            provenance=Provenance(dataset="country-result", table=None),
            country=country,
        )
        selected_boundaries = (
            load_country_boundaries(answer, context.registry)
            if context.boundaries is None
            else context.boundaries
        )
        if selected_boundaries is None:
            message = f"country boundary for {country.id!r} is not registered"
            raise ValueError(message)
        geometries = [selected_boundaries[country.id]] if country.id in selected_boundaries else []
    else:
        answer = circle_of(context.document)
        selected_boundaries = (
            load_map_boundaries(answer, context.registry)
            if context.boundaries is None
            else context.boundaries
        )
        geometries = (
            [selected_boundaries[answer.country.id]]
            if answer.country is not None and answer.country.id in selected_boundaries
            else []
            if answer.country is not None
            else list(selected_boundaries.values())
        )
    try:
        circle_data["territory_share"] = territory_share(circle, geometries)
    except ValueError as error:
        if "finite positive area" in str(error):
            circle_data.pop("territory_share", None)
            return
        raise


def enrich_document(
    document: dict[str, object], boundaries: Mapping[str, BaseGeometry] | None = None
) -> dict[str, object]:
    """Add geometric coverage to the circle payloads in a published result document."""
    kind = Envelope.model_validate(document).document
    registry = load()
    earth_radius = Envelope.model_validate(document).earth_model.radius_km
    result = cast(dict[str, object], document["result"])
    context = _Enrichment(document, earth_radius, registry, boundaries)

    def enrich(
        circle_data: dict[str, object], country_data: Mapping[str, object] | None = None
    ) -> None:
        _enrich_circle(circle_data, context, country_data)

    if kind == "country-sweep":
        for country_entry in cast(list[dict[str, object]], result["countries"]):
            identity = cast(Mapping[str, object], country_entry["country"])
            for record in cast(list[dict[str, object]], country_entry["records"]):
                enrich(cast(dict[str, object], record["circle"]), identity)
    elif kind in {"smallest", "country-smallest"}:
        enrich(cast(dict[str, object], result["circle"]))
    else:
        enrich(result)
    return document
