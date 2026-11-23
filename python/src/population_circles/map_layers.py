import json
import math
from collections.abc import Iterator, Mapping, Sequence
from dataclasses import dataclass
from functools import cache, lru_cache
from pathlib import Path
from typing import Any, cast

from shapely.geometry import Point, shape
from shapely.geometry.base import BaseGeometry
from shapely.ops import unary_union

from population_circles.dataset_registry import (
    REPO_ROOT,
    BoundaryVector,
    MapContextVector,
    Registry,
    SettlementVector,
    TerrainRaster,
    coastline_dataset_key,
)
from population_circles.presentation_config import load as load_presentation_config

COASTLINE_DATASET = coastline_dataset_key()


class UnregisteredDatasetError(ValueError):
    def __init__(self, key: str) -> None:
        super().__init__(f"dataset {key!r} is not registered")
        self.key = key


@dataclass(frozen=True)
class MapSources:
    terrain: TerrainRaster
    lakes: MapContextVector
    rivers: MapContextVector
    roads: MapContextVector
    places: SettlementVector
    terrain_overlays: tuple[TerrainRaster, ...] = ()

    @property
    def attributions(self) -> tuple[tuple[str, str], ...]:
        sources = (
            ("Terrain", self.terrain),
            *(("Terrain", dataset) for dataset in self.terrain_overlays),
            ("Lakes", self.lakes),
            ("Rivers", self.rivers),
            ("Roads", self.roads),
        )
        attributions: list[tuple[str, str]] = []
        seen: set[str] = set()
        for name, dataset in sources:
            if dataset.attribution and dataset.attribution not in seen:
                attributions.append((name, dataset.attribution))
                seen.add(dataset.attribution)
        return tuple(attributions)


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

    overlays = (
        tuple(
            cast(TerrainRaster, source(key, TerrainRaster))
            for key in settings.terrain_overlays.get(country_id, [])
        )
        if country_id is not None
        else ()
    )
    return MapSources(
        terrain=cast(TerrainRaster, source(terrain_key, TerrainRaster)),
        lakes=cast(MapContextVector, source(settings.lakes, MapContextVector)),
        rivers=cast(MapContextVector, source(settings.rivers, MapContextVector)),
        roads=cast(MapContextVector, source(settings.roads, MapContextVector)),
        places=cast(SettlementVector, source(settings.places, SettlementVector)),
        terrain_overlays=overlays,
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
        capital = properties["FEATURECLA"] == "Admin-0 capital"
        if properties.get("POP_MAX", 0) < MIN_PLACE_POPULATION and not capital:
            continue
        geometry = shape(feature["geometry"])
        if isinstance(geometry, Point):
            places.append(
                (
                    geometry,
                    properties["NAME"],
                    properties["POP_MAX"],
                    capital,
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


MIN_PLACE_POPULATION = 100_000

MAX_ROAD_MIN_ZOOM = 4


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
