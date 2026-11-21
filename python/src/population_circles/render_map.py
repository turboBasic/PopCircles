import argparse
import json
import math
import textwrap
import xml.etree.ElementTree as ET
from collections.abc import Callable, Iterator, Mapping, Sequence
from dataclasses import dataclass
from functools import cache, lru_cache
from pathlib import Path
from typing import Any, Literal, cast

import matplotlib as mpl
from matplotlib import pyplot as plt
from matplotlib.axes import Axes
from matplotlib.backends.backend_agg import FigureCanvasAgg
from matplotlib.collections import LineCollection
from matplotlib.figure import Figure
from matplotlib.font_manager import FontProperties
from matplotlib.image import AxesImage
from matplotlib.patches import PathPatch
from matplotlib.path import Path as DrawPath
from matplotlib.text import Text
from matplotlib.transforms import Bbox
from PIL import ExifTags, Image
from PIL.TiffImagePlugin import IFDRational
from shapely import get_coordinates
from shapely.geometry import MultiLineString, Point, shape
from shapely.geometry.base import BaseGeometry
from shapely.geometry.polygon import Polygon
from shapely.ops import unary_union

from population_circles.circle_document import Answer, circle_of
from population_circles.circle_geometry import cap, linestrings, polygons
from population_circles.country_outline import load_map_boundaries
from population_circles.dataset_registry import (
    REPO_ROOT,
    BoundaryVector,
    MapContextVector,
    Registry,
    SettlementVector,
    TerrainRaster,
    coastline_dataset_key,
    load,
)
from population_circles.map_frame import (
    PLATE_CARREE,
    PROJECTIONS,
    CountryFrameSettings,
    Frame,
    country_frame,
    frame,
    graticule,
    project,
)
from population_circles.presentation_config import (
    CountryMap,
    map_text_rendering,
)
from population_circles.presentation_config import (
    load as load_presentation_config,
)
from population_circles.terrain import prepare_terrain
from population_circles.territory_coverage import enrich_document

# Every figure this writes goes to a file, so an interactive backend is never wanted — and asking
# for one is what fails on a machine with no display, rather than falling back.
mpl.use("Agg")

COASTLINE_DATASET = coastline_dataset_key()


class UnregisteredDatasetError(ValueError):
    def __init__(self, key: str) -> None:
        super().__init__(f"dataset {key!r} is not registered")
        self.key = key


class MissingCountryOutlineError(ValueError):
    def __init__(self) -> None:
        super().__init__("country result rendering requires its registered boundary outline")


@dataclass(frozen=True)
class MapSources:
    terrain: TerrainRaster
    default_terrain: TerrainRaster
    lakes: MapContextVector
    rivers: MapContextVector
    roads: MapContextVector
    places: SettlementVector

    @property
    def attributions(self) -> tuple[tuple[str, str], ...]:
        terrain_sources = (
            (("Terrain", self.terrain),)
            if self.terrain.path == self.default_terrain.path
            else (("Terrain", self.default_terrain), ("Country terrain", self.terrain))
        )
        return tuple(
            (name, dataset.attribution)
            for name, dataset in (
                *terrain_sources,
                ("Lakes", self.lakes),
                ("Rivers", self.rivers),
                ("Roads", self.roads),
            )
            if dataset.attribution
        )


@dataclass(frozen=True)
class MapLayers:
    sources: MapSources
    lakes: tuple[BaseGeometry, ...]
    rivers: tuple[BaseGeometry, ...]
    roads: tuple[BaseGeometry, ...]
    places: tuple[tuple[Point, str, int, bool], ...]
    root: Path


def registered_map_sources(registry: Registry, country_id: str | None) -> MapSources:
    settings = load_presentation_config().map_sources
    terrain_key = (
        settings.terrain_overrides.get(country_id, settings.terrain_default)
        if country_id is not None
        else settings.terrain_default
    )

    def source(
        key: str, expected: type[TerrainRaster | MapContextVector | SettlementVector]
    ) -> object:
        dataset = registry.datasets.get(key)
        if not isinstance(dataset, expected):
            raise UnregisteredDatasetError(key)
        return dataset

    return MapSources(
        terrain=cast(TerrainRaster, source(terrain_key, TerrainRaster)),
        default_terrain=cast(TerrainRaster, source(settings.terrain_default, TerrainRaster)),
        lakes=cast(MapContextVector, source(settings.lakes, MapContextVector)),
        rivers=cast(MapContextVector, source(settings.rivers, MapContextVector)),
        roads=cast(MapContextVector, source(settings.roads, MapContextVector)),
        places=cast(SettlementVector, source(settings.places, SettlementVector)),
    )


@cache
def _geojson_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError as error:
        message = f"map context source is missing at {path}; run `mise run data:get`"
        raise ValueError(message) from error


def _geojson_features(path: Path) -> Iterator[Mapping[str, Any]]:
    text = _geojson_text(path)
    start = text.index("[", text.index('"features"')) + 1
    decoder = json.JSONDecoder()
    while True:
        while text[start].isspace() or text[start] == ",":
            start += 1
        if text[start] == "]":
            return
        feature, start = decoder.raw_decode(text, start)
        yield cast(Mapping[str, Any], feature)


def _coordinates_bounds(coordinates: object) -> tuple[float, float, float, float] | None:
    bounds = [math.inf, math.inf, -math.inf, -math.inf]
    stack = [coordinates]
    while stack:
        value = stack.pop()
        if not isinstance(value, list):
            continue
        coordinates = cast(list[Any], value)
        if coordinates and isinstance(coordinates[0], (int, float)):
            longitude, latitude, *_ = coordinates
            bounds[0] = min(bounds[0], longitude)
            bounds[1] = min(bounds[1], latitude)
            bounds[2] = max(bounds[2], longitude)
            bounds[3] = max(bounds[3], latitude)
        else:
            stack.extend(coordinates)
    if math.isinf(bounds[0]):
        return None
    return bounds[0], bounds[1], bounds[2], bounds[3]


@lru_cache(maxsize=1)
def _context_geometries(
    path: Path,
    *,
    major_roads: bool = False,
    bounds: tuple[float, float, float, float] | None = None,
) -> tuple[BaseGeometry, ...]:
    geometries: list[BaseGeometry] = []
    for feature in _geojson_features(path):
        properties = feature["properties"]
        if major_roads and (
            properties.get("sov_a3") == "XNR"
            or not isinstance(properties.get("min_zoom"), (int, float))
            or properties["min_zoom"] > MAX_ROAD_MIN_ZOOM
        ):
            continue
        raw_geometry = feature["geometry"]
        if bounds is not None:
            feature_bounds = _coordinates_bounds(raw_geometry["coordinates"])
            if feature_bounds is None or (
                feature_bounds[2] < bounds[0]
                or feature_bounds[0] > bounds[2]
                or feature_bounds[3] < bounds[1]
                or feature_bounds[1] > bounds[3]
            ):
                continue
        geometry = shape(raw_geometry)
        geometries.append(geometry)
    return tuple(geometries)


def load_map_layers(
    sources: MapSources,
    root: Path = REPO_ROOT,
    country_outline: BaseGeometry | None = None,
) -> MapLayers:
    bounds = tuple(country_outline.bounds) if country_outline is not None else None
    return MapLayers(
        sources=sources,
        lakes=_context_geometries(sources.lakes.file(root), bounds=bounds),
        rivers=_context_geometries(sources.rivers.file(root), bounds=bounds),
        roads=_context_geometries(sources.roads.file(root), major_roads=True, bounds=bounds),
        places=_selected_places(sources.places.file(root)),
        root=root,
    )


def place_city_labels(
    axes: Axes,
    labels: list[tuple[Text, float, float, bool]],
) -> int:
    canvas = cast(FigureCanvasAgg, axes.figure.canvas)
    canvas.draw()
    renderer = canvas.get_renderer()
    label_ids = {id(label) for label, _, _, _ in labels}
    protected = [
        label.get_window_extent(renderer)
        for label in axes.texts
        if id(label) not in label_ids and label.get_zorder() <= PLACE_LAYER
    ]
    marker_padding = 2 * renderer.dpi / 72
    marker_boxes: list[Bbox] = []
    retained_anchors: list[tuple[float, float]] = []
    retained_indices: set[int] = set()
    label_spacing = 12 * renderer.dpi / 72
    for _, x, y, _ in labels:
        marker_x, marker_y = axes.transData.transform((x, y))
        marker_boxes.append(
            Bbox.from_bounds(
                marker_x - marker_padding,
                marker_y - marker_padding,
                marker_padding * 2,
                marker_padding * 2,
            ),
        )
    for label_index, (_, x, y, capital) in enumerate(labels):
        coordinates = axes.transData.transform((x, y))
        anchor = float(coordinates[0]), float(coordinates[1])
        if not capital and any(
            math.hypot(anchor[0] - placed_x, anchor[1] - placed_y) < label_spacing
            for placed_x, placed_y in retained_anchors
        ):
            continue
        retained_anchors.append(anchor)
        retained_indices.add(label_index)
    axes_box = axes.bbox
    directions = ((1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1))
    candidates: list[
        tuple[
            int,
            int,
            Literal["left", "center", "right"],
            Literal["bottom", "center", "top"],
        ]
    ] = [
        (
            round(distance * dx),
            round(distance * dy),
            "left" if dx > 0 else "right" if dx < 0 else "center",
            "bottom" if dy > 0 else "top" if dy < 0 else "center",
        )
        for distance in (4, 6, 8)
        for dx, dy in directions
    ]
    candidates.sort(key=lambda candidate: candidate[0] ** 2 + candidate[1] ** 2)
    placed: list[Bbox] = []
    for label_index, (label, x, y, _) in enumerate(labels):
        if label_index not in retained_indices:
            continue
        for dx, dy, horizontal, vertical in candidates:
            label.set_position((dx, dy))
            label.set_horizontalalignment(horizontal)
            label.set_verticalalignment(vertical)
            label.set_visible(True)
            bounds = label.get_window_extent(renderer).padded(renderer.dpi / 72)
            collides = any(bounds.overlaps(other) for other in (*protected, *placed)) or any(
                bounds.overlaps(marker)
                for index, marker in enumerate(marker_boxes)
                if index != label_index
            )
            if not (
                axes_box.contains(*bounds.min) and axes_box.contains(*bounds.max) and not collides
            ):
                label.set_visible(False)
                continue
            placed.append(bounds)
            if dx**2 + dy**2 > 6**2:
                axes.annotate(  # pyright: ignore[reportUnknownMemberType] — Matplotlib kwargs: Unknown
                    "",
                    xy=(x, y),
                    xytext=(dx, dy),
                    textcoords="offset points",
                    arrowprops={
                        "arrowstyle": "-",
                        "color": "#68645f",
                        "linewidth": 0.45,
                        "shrinkA": 1.5,
                        "shrinkB": 2.5,
                    },
                    zorder=PLACE_LAYER - 0.1,
                )
            break
        else:
            label.set_visible(False)
    return sum(label.get_visible() for label, _, _, _ in labels)


@cache
def _selected_places(path: Path) -> tuple[tuple[Point, str, int, bool], ...]:
    try:
        features = json.loads(path.read_text(encoding="utf-8"))["features"]
    except FileNotFoundError as error:
        message = f"map place source is missing at {path}; run `mise run data:get`"
        raise ValueError(message) from error
    places: list[tuple[Point, str, int, bool]] = []
    for feature in features:
        properties = feature["properties"]
        if properties.get("POP_MAX", 0) < MIN_PLACE_POPULATION:
            continue
        geometry = shape(feature["geometry"])
        if isinstance(geometry, Point):
            places.append(
                (
                    geometry,
                    properties["NAME"],
                    properties["POP_MAX"],
                    properties["FEATURECLA"] == "Admin-0 capital",
                ),
            )
    places.sort(key=lambda place: (not place[3], -place[2]))
    return tuple(places)


def citation(registry: Registry, key: str) -> str:
    # Keyed by the document rather than by this file: `data/registry.toml` owns the wording a
    # licence requires, and which row is owed it is the answer's own property. The registry arrives
    # as an argument so a caller can select from one it parsed itself, and `render` still takes only
    # the text — nothing below here resolves a dataset.
    dataset = registry.datasets.get(key)
    if dataset is None:
        raise UnregisteredDatasetError(key)
    return dataset.attribution


# Bottom to top: coastlines, country fills, graticule, circle, its centre, and the frame's own
# outline last so no fill reaching the limb draws over it.
COASTLINE_LAYER = 2
COUNTRY_LAYER = 2.5
GRATICULE_LAYER = 2.75
WATER_LAYER = 2.6
ROAD_LAYER = 2.65
PLACE_LAYER = 2.85
MIN_PLACE_POPULATION = 100_000
CIRCLE_LAYER = 3
OCEAN = "#DFFFFD"
COUNTRY = "#65CBC1"
LAKE = "#9BD8E5"
RIVER = "#65B7CE"
ROAD = "#B56F52"
OUTLINE_WIDTH = 0.6
CENTRE_LAYER = 4
HORIZON_LAYER = 5
TERRAIN_LAYER = 1
COUNTRY_FILL_ALPHA = 0.14
MAX_ROAD_MIN_ZOOM = 4
MINUTES_PER_DEGREE = 60


@cache
def basemap(path: Path) -> BaseGeometry:
    features: Sequence[Mapping[str, dict[str, object]]] = json.loads(
        path.read_text(encoding="utf-8"),
    )["features"]
    return unary_union([shape(feature["geometry"]) for feature in features])


def registered_coastline(registry: Registry) -> tuple[BaseGeometry, str]:
    dataset = registry.datasets.get(coastline_dataset_key())
    if not isinstance(dataset, BoundaryVector):
        raise UnregisteredDatasetError(coastline_dataset_key())
    return basemap(dataset.file(REPO_ROOT)), dataset.attribution


def compound(polygon: Polygon) -> DrawPath:
    # Exterior first and every hole after it, in one path: a cap holding both poles covers the world
    # bar one region, and that region is a hole rather than a second part.
    return DrawPath.make_compound_path(
        DrawPath(list(polygon.exterior.coords)),
        *(DrawPath(list(ring.coords)) for ring in polygon.interiors),
    )


def fill_circle(axes: Axes, geometry: BaseGeometry) -> None:
    for polygon in polygons(geometry):
        axes.add_patch(
            PathPatch(
                compound(polygon),
                facecolor="crimson",
                edgecolor="darkred",
                alpha=0.45,
                linewidth=0.8,
                zorder=CIRCLE_LAYER,
            ),
        )


def outline(
    axes: Axes,
    geometry: BaseGeometry,
    layer: float = HORIZON_LAYER,
    *,
    facecolor: str = "none",
    linewidth: float = OUTLINE_WIDTH,
) -> None:
    for polygon in polygons(geometry):
        axes.add_patch(
            PathPatch(
                compound(polygon),
                facecolor=facecolor,
                edgecolor="0.4",
                linewidth=linewidth,
                zorder=layer,
            ),
        )


def stroke(axes: Axes, geometry: BaseGeometry, colour: str, width: float, layer: float) -> None:
    axes.add_collection(
        LineCollection(
            [list(line.coords) for line in linestrings(geometry)],
            colors=colour,
            linewidths=width,
            zorder=layer,
        ),
    )


def title_of(answer: Answer) -> str:
    circle = answer.circle
    country = f"{answer.country.name}: " if answer.country else ""
    centre = f"{circle.centre.lat:.4f}, {circle.centre.lon:.4f}"
    population_percent = math.floor(answer.caption.share * 100 + 0.5)
    title = (
        f"{country}{circle.radius_km:,.0f} km circle holding "
        f"{population_percent}% of the population "
        f"({answer.caption.population:,.0f} people); Center: {centre}"
    )
    if answer.caption.territory_share is not None:
        territory = answer.country.name if answer.country else "Earth\u2019s land area"
        percentage = answer.caption.territory_share * 100
        decimals = max(0, 1 - math.floor(math.log10(percentage))) if percentage else 0
        percent = f"{percentage:.{decimals}f}"
        title += f"; Circle covers {percent}% of {territory}"
    return title


def footer_text() -> str:
    return (
        "© Andrii Melnyk, 2026, https://github.com/turboBasic/PopCircles · "
        "© EuroGeographics for the administrative boundaries · "
        "See full attribution in image metadata"
    )


def visible_credits(map_attributions: Sequence[tuple[str, str]]) -> str:
    base = footer_text()
    additions = [f"{name} data: {attribution}" for name, attribution in map_attributions]
    return " · ".join((base, *additions))


def copyright_of(
    answer: Answer,
    attribution: str,
    coastline_attribution: str | None,
    boundary_attribution: str | None,
    map_attributions: Sequence[tuple[str, str]] = (),
) -> str:
    parts = ["© Andrii Melnyk, 2026, https://github.com/turboBasic/PopCircles"]
    if answer.country is not None:
        parts.append(f"Population data: {attribution}")
        parts.append(f"Administrative boundaries: {boundary_attribution}")
    elif answer.caption.country_breakdown is not None:
        parts.append(
            f"Administrative boundaries: {answer.caption.country_breakdown.provenance.attribution}"
        )
        parts.append(f"Population data: {attribution}")
    else:
        parts.append(f"Population data: {attribution}")
    if coastline_attribution:
        parts.append(f"Coastline data: {coastline_attribution}")
    parts.extend(f"{name} data: {credit}" for name, credit in map_attributions)
    return "\n".join(textwrap.fill(part, width=118) for part in parts)


def xmp_properties(
    answer: Answer,
    title: str,
    description: str,
    copyright_text: str,
    map_attributions: Sequence[tuple[str, str]] = (),
) -> dict[str, object]:
    circle = answer.circle
    properties: dict[str, object] = {
        "population": answer.caption.population,
        "populationShare": answer.caption.share,
        "radiusKm": circle.radius_km,
        "centerLatitude": circle.centre.lat,
        "centerLongitude": circle.centre.lon,
        "populationDataset": answer.provenance.dataset,
        "decimation": answer.provenance.decimation,
    }
    if answer.caption.territory_share is not None:
        properties["territoryShare"] = answer.caption.territory_share
    if answer.country is not None:
        properties["photoshop:Country"] = answer.country.name
        properties["Iptc4xmpCore:CountryCode"] = answer.country.id
        properties["countryName"] = answer.country.name
        properties["countryCode"] = answer.country.id
    settlement = answer.caption.settlement
    place: dict[str, object] = {
        "exif:GPSLatitude": gps_coordinate(circle.centre.lat, "N", "S"),
        "exif:GPSLongitude": gps_coordinate(circle.centre.lon, "E", "W"),
    }
    if answer.country is not None:
        place["Iptc4xmpExt:CountryName"] = answer.country.name
        place["Iptc4xmpExt:CountryCode"] = answer.country.id
    if settlement is not None:
        place["Iptc4xmpExt:City"] = settlement.name
        if answer.country is None:
            place["Iptc4xmpExt:CountryName"] = settlement.country
        properties["nearestSettlement"] = settlement.name
        properties["nearestSettlementCountry"] = settlement.country
        properties["distanceToSettlementKm"] = settlement.distance_km
    properties["Iptc4xmpExt:LocationShown"] = [place]
    properties["dc:creator"] = ["Andrii Melnyk"]
    properties["Iptc4xmpCore:CreatorContactInfo"] = {
        "Iptc4xmpCore:CiUrlWork": "https://github.com/turboBasic/PopCircles"
    }
    properties["dc:rights"] = "© Andrii Melnyk, 2026"
    properties["dc:title"] = title
    properties["dc:description"] = description
    properties["photoshop:Credit"] = visible_credits(map_attributions)
    properties["fullAttribution"] = copyright_text
    return {name: value for name, value in properties.items() if value is not None}


def gps_coordinate(value: float, positive: str, negative: str) -> str:
    degrees = int(abs(value))
    minutes = round((abs(value) - degrees) * MINUTES_PER_DEGREE, 5)
    if minutes == MINUTES_PER_DEGREE:
        degrees += 1
        minutes = 0
    hemisphere = positive if value >= 0 else negative
    return f"{degrees},{minutes:g}{hemisphere}"


def _xmp_creator(description: ET.Element, value: object, namespaces: Mapping[str, str]) -> None:
    sequence = ET.SubElement(description, f"{{{namespaces['dc']}}}creator")
    items = ET.SubElement(sequence, f"{{{namespaces['rdf']}}}Seq")
    for creator in cast(list[str], value):
        ET.SubElement(items, f"{{{namespaces['rdf']}}}li").text = creator


def _xmp_alt_text(
    description: ET.Element, name: str, value: object, namespaces: Mapping[str, str]
) -> None:
    local_name = name.split(":", maxsplit=1)[1]
    prop = ET.SubElement(description, f"{{{namespaces['dc']}}}{local_name}")
    items = ET.SubElement(prop, f"{{{namespaces['rdf']}}}Alt")
    item = ET.SubElement(items, f"{{{namespaces['rdf']}}}li")
    item.set("{http://www.w3.org/XML/1998/namespace}lang", "x-default")
    item.text = cast(str, value)


def _xmp_creator_contact(
    description: ET.Element, value: object, namespaces: Mapping[str, str]
) -> None:
    prop = ET.SubElement(description, f"{{{namespaces['Iptc4xmpCore']}}}CreatorContactInfo")
    prop.set(f"{{{namespaces['rdf']}}}parseType", "Resource")
    for field, text in cast(dict[str, str], value).items():
        _, local_name = field.split(":", maxsplit=1)
        ET.SubElement(prop, f"{{{namespaces['Iptc4xmpCore']}}}{local_name}").text = text


def _xmp_location_shown(
    description: ET.Element, value: object, namespaces: Mapping[str, str]
) -> None:
    prop = ET.SubElement(description, f"{{{namespaces['Iptc4xmpExt']}}}LocationShown")
    bag = ET.SubElement(prop, f"{{{namespaces['rdf']}}}Bag")
    for location in cast(list[dict[str, object]], value):
        item = ET.SubElement(bag, f"{{{namespaces['rdf']}}}li")
        item.set(f"{{{namespaces['rdf']}}}parseType", "Resource")
        for field, text in location.items():
            prefix, local_name = field.split(":", maxsplit=1)
            ET.SubElement(item, f"{{{namespaces[prefix]}}}{local_name}").text = str(text)


def _xmp_scalar(
    description: ET.Element, name: str, value: object, namespaces: Mapping[str, str]
) -> None:
    prefix, separator, local_name = name.partition(":")
    if not separator:
        prefix, local_name = "pc", name
    description.set(f"{{{namespaces[prefix]}}}{local_name}", str(value))


def xmp_packet(properties: Mapping[str, object]) -> str:
    namespaces = {
        "x": "adobe:ns:meta/",
        "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
        "dc": "http://purl.org/dc/elements/1.1/",
        "photoshop": "http://ns.adobe.com/photoshop/1.0/",
        "Iptc4xmpCore": "http://iptc.org/std/Iptc4xmpCore/1.0/xmlns/",
        "Iptc4xmpExt": "http://iptc.org/std/Iptc4xmpExt/2008-02-29/",
        "exif": "http://ns.adobe.com/exif/1.0/",
        "pc": "https://github.com/turboBasic/PopCircles/ns/1.0/",
    }
    for prefix, uri in namespaces.items():
        ET.register_namespace(prefix, uri)
    xmp = ET.Element(f"{{{namespaces['x']}}}xmpmeta")
    rdf = ET.SubElement(xmp, f"{{{namespaces['rdf']}}}RDF")
    description = ET.SubElement(rdf, f"{{{namespaces['rdf']}}}Description")
    description.set(f"{{{namespaces['rdf']}}}about", "")
    for name, value in properties.items():
        if name == "dc:creator":
            _xmp_creator(description, value, namespaces)
        elif name in {"dc:rights", "dc:title", "dc:description"}:
            _xmp_alt_text(description, name, value, namespaces)
        elif name == "Iptc4xmpCore:CreatorContactInfo":
            _xmp_creator_contact(description, value, namespaces)
        elif name == "Iptc4xmpExt:LocationShown":
            _xmp_location_shown(description, value, namespaces)
        else:
            _xmp_scalar(description, name, value, namespaces)
    return ET.tostring(xmp, encoding="unicode", xml_declaration=True)


def description_of(answer: Answer) -> str:
    return (
        f"Answered from population dataset {answer.provenance.dataset} "
        f"at decimation k={answer.provenance.decimation}"
    )


def metadata_json_fields(
    answer: Answer,
    properties: Mapping[str, object],
) -> dict[str, object]:
    title = cast(str, properties["dc:title"])
    description = cast(str, properties["dc:description"])
    copyright_text = cast(str, properties["fullAttribution"])
    return {
        "Title": title,
        "Description": description,
        "Copyright": copyright_text,
        "XML:com.adobe.xmp": dict(properties),
        "EXIF:GPSInfo": exif_gps_values(answer.circle.centre.lat, answer.circle.centre.lon),
    }


def exif_gps_values(latitude: float, longitude: float) -> dict[str, object]:
    def dms(value: float) -> list[list[int]]:
        total = round(abs(value) * 3_600_000)
        degrees, remainder = divmod(total, 3_600_000)
        minutes, seconds = divmod(remainder, 60_000)
        return [[degrees, 1], [minutes, 1], [seconds, 1000]]

    return {
        "GPSVersionID": [2, 3, 0, 0],
        "GPSLatitudeRef": "N" if latitude >= 0 else "S",
        "GPSLatitude": dms(latitude),
        "GPSLongitudeRef": "E" if longitude >= 0 else "W",
        "GPSLongitude": dms(longitude),
        "GPSMapDatum": "WGS-84",
    }


def exif_gps_packet(latitude: float, longitude: float) -> bytes:
    values = exif_gps_values(latitude, longitude)
    gps = {
        ExifTags.GPS.GPSVersionID: bytes(cast(list[int], values["GPSVersionID"])),
        ExifTags.GPS.GPSLatitudeRef: cast(str, values["GPSLatitudeRef"]),
        ExifTags.GPS.GPSLatitude: tuple(
            IFDRational(numerator, denominator)
            for numerator, denominator in cast(list[list[int]], values["GPSLatitude"])
        ),
        ExifTags.GPS.GPSLongitudeRef: cast(str, values["GPSLongitudeRef"]),
        ExifTags.GPS.GPSLongitude: tuple(
            IFDRational(numerator, denominator)
            for numerator, denominator in cast(list[list[int]], values["GPSLongitude"])
        ),
        ExifTags.GPS.GPSMapDatum: "WGS-84",
    }
    exif = Image.Exif()
    exif[ExifTags.IFD.GPSInfo] = gps
    return exif.tobytes()


def png_metadata(
    title: str,
    copyright_text: str,
    description: str,
    properties: Mapping[str, object],
) -> dict[str, str]:
    return {
        "Title": title,
        "Description": description,
        "Copyright": copyright_text,
        "XML:com.adobe.xmp": xmp_packet(
            {name: value for name, value in properties.items() if value is not None}
        ),
    }


def other_countries(
    boundaries: Mapping[str, BaseGeometry] | None, answer: Answer
) -> list[BaseGeometry]:
    if boundaries is None:
        return []
    return [
        geometry
        for identifier, geometry in boundaries.items()
        if answer.country is None or identifier != answer.country.id
    ]


def other_country_labels(
    boundaries: Mapping[str, BaseGeometry] | None, answer: Answer
) -> list[tuple[BaseGeometry, str]]:
    if boundaries is None or answer.country is None:
        return []
    names = cast(Mapping[str, str], getattr(boundaries, "names", {}))
    return [
        (geometry, names[identifier])
        for identifier, geometry in boundaries.items()
        if identifier != answer.country.id and identifier in names
    ]


def annotate(
    figure: Figure,
    y: float,
    text: str,
    size: float,
    alignment: str = "baseline",
) -> None:
    # matplotlib types Figure.text's keyword arguments as Unknown, so the ignore lives here once
    # rather than at each of the two callers.
    figure.text(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
        0.5,
        y,
        text,
        horizontalalignment="center",
        verticalalignment=alignment,
        fontsize=size,
        parse_math=False,
    )


def wrap_to_width(figure: Figure, text: str, size: float) -> str:
    canvas = cast(FigureCanvasAgg, figure.canvas)
    renderer = canvas.get_renderer()
    font = FontProperties(size=size)
    max_width = figure.bbox.width * 0.94
    lines: list[str] = []
    line = ""
    for word in text.split():
        candidate = f"{line} {word}".strip()
        width, _, _ = renderer.get_text_width_height_descent(candidate, font, ismath=False)
        if line and width > max_width:
            lines.append(line)
            line = word
        else:
            line = candidate
    if line:
        lines.append(line)
    return "\n".join(lines)


def _projected_context(
    view: Frame,
    geometries: Sequence[BaseGeometry],
    country_clip: BaseGeometry | None,
) -> list[BaseGeometry]:
    projected: list[BaseGeometry] = []
    for geometry in geometries:
        clipped = geometry
        if country_clip is not None:
            if not clipped.intersects(country_clip):
                continue
            clipped = clipped.intersection(country_clip)
        item = project(view, clipped)
        if all(math.isfinite(value) for point in get_coordinates(item) for value in point):
            projected.append(item)
    return projected


def draw_context(
    axes: Axes,
    view: Frame,
    layers: MapLayers,
    country_clip: BaseGeometry | None,
) -> None:
    lake_paths = [
        compound(polygon)
        for geometry in _projected_context(view, layers.lakes, country_clip)
        for polygon in polygons(geometry)
    ]
    if lake_paths:
        axes.add_patch(
            PathPatch(
                DrawPath.make_compound_path(*lake_paths),
                facecolor=LAKE,
                edgecolor=RIVER,
                linewidth=0.25,
                zorder=WATER_LAYER,
            ),
        )
    for geometries, colour, width, layer in (
        (layers.rivers, RIVER, 0.45, WATER_LAYER),
        (layers.roads, ROAD, 0.65, ROAD_LAYER),
    ):
        lines = [
            line
            for geometry in _projected_context(view, geometries, country_clip)
            for line in linestrings(geometry)
            if not line.is_empty
        ]
        if lines:
            stroke(
                axes,
                MultiLineString([list(line.coords) for line in lines]),
                colour,
                width,
                layer,
            )
    if country_clip is not None:
        places = [place for place in layers.places if country_clip.covers(place[0])][:10]
        labels: list[tuple[Text, float, float, bool]] = []
        for geometry, name, _, capital in places:
            point = cast(Point, project(view, geometry))
            if not all(math.isfinite(value) for value in point.coords[0]):
                continue
            axes.scatter(  # pyright: ignore[reportUnknownMemberType] — Matplotlib kwargs: Unknown
                [point.x],
                [point.y],
                s=15 if capital else 8,
                color="#59435f" if capital else "#545454",
                zorder=PLACE_LAYER,
            )
            label = axes.annotate(  # pyright: ignore[reportUnknownMemberType] — Matplotlib kwargs: Unknown
                name,
                (point.x, point.y),
                xytext=(0, 0),
                textcoords="offset points",
                fontsize=7 if not capital else 8,
                color="#454545",
                weight="bold" if capital else "normal",
                zorder=PLACE_LAYER,
                visible=False,
            )
            labels.append((label, point.x, point.y, capital))
        place_city_labels(axes, labels)


def draw_country_labels(
    axes: Axes,
    view: Frame,
    labels: Sequence[tuple[BaseGeometry, str]],
) -> None:
    for geometry, name in labels:
        projected = project(view, geometry)
        if not all(math.isfinite(value) for point in get_coordinates(projected) for value in point):
            continue
        visible = projected.intersection(view.horizon)
        if visible.is_empty:
            continue
        anchor = visible.representative_point()
        axes.text(
            anchor.x,
            anchor.y,
            name,
            horizontalalignment="center",
            verticalalignment="center",
            fontsize=8,
            color="#454545",
            zorder=PLACE_LAYER,
        )


def draw(  # noqa: PLR0913, PLR0917 — map layers and country clipping are distinct display inputs
    axes: Axes,
    view: Frame,
    coastline: BaseGeometry | None,
    other_countries: Sequence[BaseGeometry] = (),
    map_layers: MapLayers | None = None,
    country_clip: BaseGeometry | None = None,
    country_labels: Sequence[tuple[BaseGeometry, str]] = (),
) -> None:
    stroke(axes, project(view, graticule()), "0.75", 0.3, GRATICULE_LAYER)
    if coastline is not None:
        stroke(axes, project(view, coastline), "dimgrey", 0.5, COASTLINE_LAYER)
    country_paths: list[DrawPath] = []
    for geometry in other_countries:
        projected = project(view, geometry)
        if all(math.isfinite(value) for point in get_coordinates(projected) for value in point):
            country_paths.extend(compound(polygon) for polygon in polygons(projected))
    if country_paths:
        axes.add_patch(
            PathPatch(
                DrawPath.make_compound_path(*country_paths),
                facecolor=COUNTRY if view.country_outline is None else "none",
                alpha=COUNTRY_FILL_ALPHA if view.country_outline is None else 1.0,
                edgecolor="0.4",
                linewidth=OUTLINE_WIDTH,
                zorder=COUNTRY_LAYER,
            ),
        )
    if country_labels:
        draw_country_labels(axes, view, country_labels)
    if view.country_outline is not None:
        for polygon in polygons(view.country_outline):
            axes.add_patch(
                PathPatch(
                    compound(polygon),
                    facecolor=COUNTRY,
                    alpha=COUNTRY_FILL_ALPHA,
                    edgecolor="0.4",
                    linewidth=1,
                    zorder=COUNTRY_LAYER,
                ),
            )
    if map_layers is not None:
        draw_context(axes, view, map_layers, country_clip)
    fill_circle(axes, view.circle)
    outline(axes, view.horizon)


def fill_ocean(axes: Axes, view: Frame) -> None:
    west, south, east, north = view.horizon.bounds
    draw_image = cast(Callable[..., AxesImage], axes.imshow)  # pyright: ignore[reportUnknownMemberType] — Matplotlib kwargs: Unknown
    ocean = draw_image(
        [[(223 / 255, 1.0, 253 / 255, 1.0)]],
        extent=(west, east, south, north),
        zorder=0,
    )
    horizon = DrawPath.make_compound_path(
        *(compound(polygon) for polygon in polygons(view.horizon))
    )
    ocean.set_clip_path(PathPatch(horizon, transform=axes.transData))


def draw_map_layers(  # noqa: PLR0913, PLR0917 — each argument describes an independent map layer
    axes: Axes,
    view: Frame,
    coastline: BaseGeometry | None,
    other_countries: Sequence[BaseGeometry],
    map_layers: MapLayers | None,
    country_clip: BaseGeometry | None,
    country_labels: Sequence[tuple[BaseGeometry, str]],
) -> None:
    fill_ocean(axes, view)
    if map_layers is not None:
        draw_image = cast(Callable[..., AxesImage], axes.imshow)  # pyright: ignore[reportUnknownMemberType] — Matplotlib kwargs: Unknown
        west, south, east, north = view.horizon.bounds

        def draw_terrain(dataset: TerrainRaster, *, grayscale: bool = False) -> AxesImage:
            image, _ = prepare_terrain(
                dataset,
                view,
                root=map_layers.root,
                grayscale=grayscale,
            )
            return draw_image(
                image,
                extent=(west, east, south, north),
                origin="upper",
                interpolation="bilinear",
                zorder=TERRAIN_LAYER,
            )

        if country_clip is None:
            draw_terrain(map_layers.sources.terrain)
        else:
            draw_terrain(map_layers.sources.default_terrain, grayscale=True)
            colored = draw_terrain(map_layers.sources.terrain)
            if view.country_outline is not None:
                paths = [compound(polygon) for polygon in polygons(view.country_outline)]
                if paths:
                    clip = PathPatch(
                        DrawPath.make_compound_path(*paths),
                        transform=axes.transData,
                    )
                    colored.set_clip_path(clip)
    draw(axes, view, coastline, other_countries, map_layers, country_clip, country_labels)


def render(  # noqa: PLR0913 — distinct geometry and display inputs stay explicit
    answer: Answer,
    projection: str,
    *,
    # A caller drawing several figures unions the coastline once; `None` omits it.
    coastline: BaseGeometry | None,
    country_outline: BaseGeometry | None = None,
    other_countries: Sequence[BaseGeometry] = (),
    country_labels: Sequence[tuple[BaseGeometry, str]] = (),
    render_text: tuple[bool, bool] = (True, True),
    map_layers: MapLayers | None = None,
) -> Figure:
    if answer.country is not None and country_outline is None:
        raise MissingCountryOutlineError
    circle = answer.circle
    built = cap(circle.centre, circle.radius_km, circle.earth_radius_km)
    if answer.country is not None:
        country_outline, frame_settings = country_map_display(answer.country.id, country_outline)
    else:
        frame_settings = None
    view = (
        country_frame(built, country_outline, frame_settings)
        if country_outline is not None
        else frame(projection, built)
    )
    map_attributions = map_layers.sources.attributions if map_layers is not None else ()
    centre = view.to_frame.transform(circle.centre.lon, circle.centre.lat)

    figure = plt.figure(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
        figsize=(11.0, 6.0),
    )
    try:
        title = title_of(answer)
        footer = visible_credits(map_attributions)
        canvas = cast(FigureCanvasAgg, figure.canvas)
        header_bottom = 0.985
        if render_text[0]:
            annotate(figure, 0.985, wrap_to_width(figure, title, 12.0), 12.0, "top")
            canvas.draw()
            header_bottom = figure.texts[-1].get_window_extent(canvas.get_renderer()).y0
            header_bottom /= figure.bbox.height
        if render_text[1]:
            description_top = header_bottom - 0.015 if render_text[0] else 0.985
            annotate(
                figure,
                description_top,
                wrap_to_width(figure, description_of(answer), 7.0),
                7.0,
                "top",
            )
            canvas.draw()
            header_bottom = figure.texts[-1].get_window_extent(canvas.get_renderer()).y0
            header_bottom /= figure.bbox.height
        top = min(0.87, header_bottom - 0.05) if any(render_text) else 0.98
        bottom = 0.16
        figure.subplots_adjust(left=0.03, right=0.97, top=top, bottom=bottom)
        axes = figure.add_subplot(1, 1, 1)
        # Equal: degrees of longitude and latitude have the same length on the
        # frame stated in degrees, and the other frame is metres in both directions.
        axes.set_aspect("equal")
        # The horizon replaces the spines: one frame's outline is a rectangle and
        # the other's is a disc, and only one of those is an axes frame.
        axes.set_axis_off()
        west, south, east, north = view.horizon.bounds
        axes.set_xlim(west, east)
        axes.set_ylim(south, north)
        draw_map_layers(
            axes,
            view,
            coastline,
            other_countries,
            map_layers,
            country_outline,
            country_labels,
        )
        axes.plot(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
            [centre[0]],
            [centre[1]],
            marker="+",
            color="black",
            markersize=9,
            zorder=CENTRE_LAYER,
        )
        annotate(figure, 0.04, wrap_to_width(figure, footer, 6.5), 6.5, "bottom")
    except BaseException:
        plt.close(figure)
        raise
    return figure


def country_map_display(
    country_id: str, outline: BaseGeometry | None
) -> tuple[BaseGeometry, CountryFrameSettings]:
    if outline is None:
        raise MissingCountryOutlineError
    config = load_presentation_config()
    settings = config.country_maps.get(country_id)
    if not isinstance(settings, CountryMap):
        return outline, CountryFrameSettings(projection=config.default_projection)
    extent = None
    if settings.extent is not None:
        extent = (
            settings.extent.x_min,
            settings.extent.y_min,
            settings.extent.x_max,
            settings.extent.y_max,
        )
    return outline, CountryFrameSettings(
        projection=settings.projection,
        projection_parameters=settings.projection_parameters,
        extent=extent,
    )


def parse(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Render a circle from a popcircles document.")
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--projection", choices=PROJECTIONS, default=PLATE_CARREE)
    parser.add_argument("--no-coastlines", action="store_true")
    return parser.parse_args(argv)


def render_file(
    input_path: Path,
    output_path: Path,
    projection: str = PLATE_CARREE,
    *,
    no_coastlines: bool = False,
    coverage_enriched: bool = False,
) -> None:
    document = json.loads(input_path.read_text(encoding="utf-8"))
    answer = circle_of(document)
    registry = load()
    population_attribution = (
        answer.selection.population_source.attribution if answer.selection is not None else None
    ) or citation(registry, answer.provenance.dataset)
    boundary_attribution = (
        answer.selection.boundary_source.attribution if answer.selection is not None else None
    )
    coastline, coastline_attribution = (
        (None, None) if no_coastlines else registered_coastline(registry)
    )
    render_text = map_text_rendering()
    boundaries = load_map_boundaries(answer, registry)
    if not coverage_enriched:
        document = enrich_document(document, boundaries)
    answer = circle_of(document)
    selected = boundaries.get(answer.country.id) if answer.country is not None else None
    map_sources = registered_map_sources(
        registry,
        answer.country.id if answer.country is not None else None,
    )
    country_boundary_attribution = None
    if answer.country is not None:
        boundary_credit = boundary_attribution or citation(
            registry, answer.country.boundary_dataset
        )
        country_boundary_attribution = boundary_credit
    figure = render(
        answer,
        projection,
        coastline=coastline,
        country_outline=selected,
        other_countries=other_countries(boundaries, answer),
        render_text=render_text,
        map_layers=load_map_layers(map_sources, country_outline=selected),
        country_labels=other_country_labels(boundaries, answer),
    )
    title = title_of(answer)
    copyright_text = copyright_of(
        answer,
        population_attribution,
        coastline_attribution,
        country_boundary_attribution,
        map_sources.attributions,
    )
    description = description_of(answer)
    properties = xmp_properties(
        answer,
        title,
        description,
        copyright_text,
        map_sources.attributions,
    )
    metadata_values = metadata_json_fields(answer, properties)
    sidecar_path = output_path.with_suffix(".json")
    try:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        # No bbox_inches="tight": the title and citation use figure coordinates; cropping
        # to the drawn content moves them off the positions they were given.
        figure.savefig(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
            output_path,
            dpi=200,
            metadata=png_metadata(title, copyright_text, description, properties),
            pil_kwargs={
                "exif": exif_gps_packet(answer.circle.centre.lat, answer.circle.centre.lon)
            },
        )
        document["render_metadata"] = metadata_values
        sidecar_path.write_text(
            json.dumps(document, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
        )
    finally:
        plt.close(figure)


def main(argv: list[str] | None = None) -> int:
    args = parse(argv)
    render_file(args.input, args.output, args.projection, no_coastlines=args.no_coastlines)
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
