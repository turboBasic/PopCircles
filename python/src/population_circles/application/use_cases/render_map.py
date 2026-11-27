from collections.abc import Callable, Mapping
from dataclasses import dataclass
from pathlib import Path

from shapely.geometry.base import BaseGeometry

from population_circles.application.use_cases.territory_coverage import enrich_document
from population_circles.documents.result_document import Answer, circle_of


@dataclass(frozen=True)
class LoadedMapLayers:
    drawing: object
    attributions: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class MapRenderPreferences:
    render_text: tuple[bool, bool]
    title_label: str | None


@dataclass(frozen=True)
class MapBoundaries:
    geometries: Mapping[str, BaseGeometry]
    names: Mapping[str, str]


@dataclass(frozen=True)
class PreparedMapRender:
    document: dict[str, object]
    answer: Answer
    output_path: Path
    projection: str
    coastline: BaseGeometry | None
    country_outline: BaseGeometry | None
    other_countries: tuple[BaseGeometry, ...]
    country_labels: tuple[tuple[BaseGeometry, str], ...]
    render_text: tuple[bool, bool]
    map_layers: object
    title_label: str | None
    population_attribution: str
    coastline_attribution: str | None
    country_boundary_attribution: str | None
    source_attributions: tuple[tuple[str, str], ...]


DocumentLoader = Callable[[Path], dict[str, object]]
BoundaryLoader = Callable[[Answer], MapBoundaries]
CoastlineLoader = Callable[[], tuple[BaseGeometry, str]]
CitationLoader = Callable[[str], str]
MapLayersLoader = Callable[[Answer, BaseGeometry | None], LoadedMapLayers]
PreferencesLoader = Callable[[Path], MapRenderPreferences]
PreparedRenderer = Callable[[PreparedMapRender], None]


@dataclass(frozen=True)
class RenderMapFileRequest:
    input_path: Path
    output_path: Path
    projection: str
    no_coastlines: bool = False
    coverage_enriched: bool = False


@dataclass(frozen=True)
class RenderMapCollaborators:
    load_document: DocumentLoader
    load_boundaries: BoundaryLoader
    load_coastline: CoastlineLoader
    citation: CitationLoader
    load_layers: MapLayersLoader
    load_preferences: PreferencesLoader
    render_prepared: PreparedRenderer


def other_countries(boundaries: MapBoundaries, answer: Answer) -> tuple[BaseGeometry, ...]:
    return tuple(
        geometry
        for identifier, geometry in boundaries.geometries.items()
        if answer.country is None or identifier != answer.country.id
    )


def other_country_labels(
    boundaries: MapBoundaries, answer: Answer
) -> tuple[tuple[BaseGeometry, str], ...]:
    if answer.country is None:
        return ()
    return tuple(
        (geometry, boundaries.names[identifier])
        for identifier, geometry in boundaries.geometries.items()
        if identifier != answer.country.id and identifier in boundaries.names
    )


def render_file(
    request: RenderMapFileRequest,
    collaborators: RenderMapCollaborators,
) -> None:
    document = collaborators.load_document(request.input_path)
    answer = circle_of(document)
    population_attribution = (
        answer.selection.population_source.attribution if answer.selection is not None else None
    ) or collaborators.citation(answer.provenance.dataset)
    boundary_attribution = (
        answer.selection.boundary_source.attribution if answer.selection is not None else None
    )
    coastline, coastline_attribution = (
        (None, None) if request.no_coastlines else collaborators.load_coastline()
    )
    preferences = collaborators.load_preferences(request.output_path)
    boundaries = collaborators.load_boundaries(answer)
    if not request.coverage_enriched:
        document = enrich_document(document, boundaries.geometries)
    answer = circle_of(document)
    country_outline = (
        boundaries.geometries.get(answer.country.id) if answer.country is not None else None
    )
    layers = collaborators.load_layers(answer, country_outline)
    country_boundary_attribution = None
    if answer.country is not None:
        country_boundary_attribution = boundary_attribution or collaborators.citation(
            answer.country.boundary_dataset
        )
    collaborators.render_prepared(
        PreparedMapRender(
            document=document,
            answer=answer,
            output_path=request.output_path,
            projection=request.projection,
            coastline=coastline,
            country_outline=country_outline,
            other_countries=other_countries(boundaries, answer),
            country_labels=other_country_labels(boundaries, answer),
            render_text=preferences.render_text,
            map_layers=layers.drawing,
            title_label=preferences.title_label,
            population_attribution=population_attribution,
            coastline_attribution=coastline_attribution,
            country_boundary_attribution=country_boundary_attribution,
            source_attributions=layers.attributions,
        )
    )
