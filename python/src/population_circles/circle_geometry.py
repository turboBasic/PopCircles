import math
from dataclasses import dataclass

from pyproj import CRS, Transformer
from shapely.geometry import LineString, MultiLineString, MultiPolygon, Point, Polygon, box
from shapely.geometry.base import BaseGeometry
from shapely.ops import unary_union

from population_circles.circle_document import Coordinate

METRES_PER_KM = 1000.0

# Per quadrant, so the ring shapely returns has 720 sides. The chord then cuts R(1 - cos(pi/720))
# inside the arc, which at 3300 km is 31.41 m and falls with the radius — more than an order below
# the finest cell a grid this program answers on has, so the shortfall is below anything a figure
# can show. Raising it costs vertices in every drawn polygon and buys nothing visible.
QUAD_SEGS = 180

TURN_DEG = 360.0
HALF_TURN_DEG = 180.0
POLE_LAT = 90.0
POLES = (POLE_LAT, -POLE_LAT)

# Every longitude a figure may carry, and the shape the seam is cut against.
WORLD = box(-HALF_TURN_DEG, -POLE_LAT, HALF_TURN_DEG, POLE_LAT)


@dataclass(frozen=True)
class Cap:
    # A circle about the origin in the source CRS's metres, which is what a spherical cap *is* in an
    # azimuthal equidistant projection centred on it. Every hard case — the antimeridian, either
    # pole — is ordinary here and becomes PROJ's problem rather than a traversal's.
    polygon: Polygon
    source: CRS
    # Published so a caller drawing this cap builds its own projections on the same sphere. One on
    # another sphere makes PROJ shift the datum, which is a second earth model arriving by default
    # rather than by decision.
    geodetic: CRS
    centre: Coordinate
    radius_km: float
    earth_radius_km: float


def projected_crs(projection: str, centre: Coordinate, earth_radius_km: float) -> CRS:
    # The one place a PROJ definition is spelled, so the sphere every CRS a figure holds is built on
    # is the document's own earth model rather than a default ellipsoid in whichever one was written
    # second.
    return CRS.from_proj4(
        f"+proj={projection} +lat_0={centre.lat} +lon_0={centre.lon} "
        f"+R={earth_radius_km * METRES_PER_KM} +units=m +no_defs",
    )


def geodetic_crs(earth_radius_km: float) -> CRS:
    return CRS.from_proj4(f"+proj=longlat +R={earth_radius_km * METRES_PER_KM} +no_defs")


def hemisphere_km(earth_radius_km: float) -> float:
    # A quarter of the circumference: the radius at which a cap is exactly half the globe, which is
    # both the far edge of what an orthographic frame can show and the point past which a cap
    # contains both poles.
    return earth_radius_km * math.pi / 2.0


def cap(centre: Coordinate, radius_km: float, earth_radius_km: float) -> Cap:
    return Cap(
        polygon=Point(0.0, 0.0).buffer(radius_km * METRES_PER_KM, quad_segs=QUAD_SEGS),
        source=projected_crs("aeqd", centre, earth_radius_km),
        geodetic=geodetic_crs(earth_radius_km),
        centre=centre,
        radius_km=radius_km,
        earth_radius_km=earth_radius_km,
    )


def boundary(built: Cap) -> tuple[Coordinate, ...]:
    # The buffer's own vertices, one transform each. Every one is a real point on the cap's boundary
    # and `radius_km` from the centre, which is the claim a test can make here and cannot make about
    # `geographic`: cutting a projected cap synthesises vertices that lie on no boundary at all.
    to_geodetic = Transformer.from_crs(built.source, built.geodetic, always_xy=True)
    return tuple(
        Coordinate(lat=lat, lon=lon)
        for lon, lat in (to_geodetic.transform(x, y) for x, y in built.polygon.exterior.coords)
    )


def enclosed_poles(built: Cap) -> tuple[float, ...]:
    # Asked of the cap in its own projection, where containment is a point in a disc, rather than
    # derived from the ring's winding: the two-pole case winds like the no-pole one and is the case
    # that needs telling apart.
    to_source = Transformer.from_crs(built.geodetic, built.source, always_xy=True)
    return tuple(
        pole for pole in POLES if built.polygon.contains(Point(to_source.transform(0.0, pole)))
    )


def turns(ring: list[tuple[float, float]]) -> list[int]:
    # How many whole turns of longitude the walk has taken by each vertex, counted as an integer so
    # that a copy shifted by one turn lands bit-identically on the next copy's seam. Adding 360.0 to
    # an already-shifted coordinate does not, and the two copies then fail to merge along the edge
    # they share.
    counted: list[int] = []
    total = 0
    previous: float | None = None
    for lon, _ in ring:
        if previous is not None:
            if lon - previous > HALF_TURN_DEG:
                total -= 1
            elif lon - previous < -HALF_TURN_DEG:
                total += 1
        counted.append(total)
        previous = lon
    return counted


def polygons(geometry: BaseGeometry) -> tuple[Polygon, ...]:
    # A cut answers with a Polygon, a MultiPolygon or nothing at all depending on where it fell, and
    # whoever draws the result wants the same tuple of parts from each of the three.
    if isinstance(geometry, Polygon):
        return (geometry,) if not geometry.is_empty else ()
    if isinstance(geometry, MultiPolygon):
        return tuple(polygon for polygon in geometry.geoms if not polygon.is_empty)
    return ()


def linestrings(geometry: BaseGeometry) -> tuple[LineString, ...]:
    if isinstance(geometry, LineString):
        return (geometry,)
    if isinstance(geometry, MultiLineString):
        return tuple(geometry.geoms)
    return ()


def geographic(built: Cap) -> BaseGeometry:
    # The cap in longitude and latitude, cut at the seam and closed over a pole — which is the whole
    # reason a ring of coordinates is not the drawing path. Three cases, told apart by how many
    # poles the cap holds: none and the unwrapped walk closes on itself; one and the walk advances a
    # whole turn, so it is closed along that pole; both and the walk closes again but around the
    # *antipode*, bounding the one region the cap does not cover.
    to_geodetic = Transformer.from_crs(built.source, built.geodetic, always_xy=True)
    ring = [to_geodetic.transform(x, y) for x, y in built.polygon.exterior.coords]
    counted = turns(ring)
    poles = enclosed_poles(built)

    def copy(shift: int) -> Polygon:
        walk = [
            (lon + TURN_DEG * (turn + shift), lat)
            for (lon, lat), turn in zip(ring, counted, strict=True)
        ]
        if len(poles) == 1:
            walk = [*walk, (walk[-1][0], poles[0]), (walk[0][0], poles[0])]
        return Polygon(walk)

    # Every copy, not the one the centre sits in: a cap wider than the window reaches into both
    # neighbours, and clipping their union is what splits it at the seam without cutting anything a
    # single copy would have kept.
    tiled = unary_union([copy(-1), copy(0), copy(1)])
    drawn = tiled.intersection(WORLD)
    return WORLD.difference(drawn) if len(poles) == len(POLES) else drawn
