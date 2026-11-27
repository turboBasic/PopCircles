import json
from pathlib import Path

import typer
from shapely.geometry.base import BaseGeometry

from population_circles.application.ports import PopulationRunner
from population_circles.application.use_cases.gallery import (
    CountrySweepCollaborators,
    GalleryFigure,
    RenderJob,
)
from population_circles.application.use_cases.gallery_build import GalleryBuildCollaborators
from population_circles.application.use_cases.render_map import (
    LoadedMapLayers,
    MapBoundaries,
    MapRenderPreferences,
    RenderMapCollaborators,
)
from population_circles.application.use_cases.territory_coverage import enrich_document
from population_circles.documents.result_document import Answer, circle_of
from population_circles.infrastructure import popcircles_cli
from population_circles.infrastructure.geodata import map_layers as map_data
from population_circles.infrastructure.geodata.country_boundaries import (
    CountryBoundaries,
    load_country_boundaries,
    load_map_boundaries,
)
from population_circles.infrastructure.geodata.dataset_registry import load
from population_circles.presentation.gallery import output as gallery_output
from population_circles.presentation.gallery import page_data as gallery_page_data
from population_circles.presentation.gallery import rendering as gallery_rendering
from population_circles.presentation.maps import render_map, source_selection
from population_circles.presentation.maps.presentation_config import (
    load as load_presentation_config,
)


def render_map_collaborators() -> RenderMapCollaborators:
    registry = load()

    def load_render_boundaries(answer: Answer) -> MapBoundaries:
        boundaries = load_map_boundaries(answer, registry)
        names = boundaries.names if isinstance(boundaries, CountryBoundaries) else {}
        return MapBoundaries(boundaries, names)

    def load_document(path: Path) -> dict[str, object]:
        return json.loads(path.read_text(encoding="utf-8"))

    def load_layers(answer: Answer, country_outline: BaseGeometry | None) -> LoadedMapLayers:
        sources = source_selection.registered_map_sources(
            registry, answer.country.id if answer.country is not None else None
        )
        return LoadedMapLayers(
            map_data.load_map_layers(sources, country_outline=country_outline),
            sources.attributions,
        )

    def load_preferences(path: Path) -> MapRenderPreferences:
        presentation = load_presentation_config()
        return MapRenderPreferences(
            (presentation.map_text.render_title, presentation.map_text.render_description),
            presentation.map_titles.get(path.stem),
        )

    return RenderMapCollaborators(
        load_document=load_document,
        load_boundaries=load_render_boundaries,
        load_coastline=lambda: map_data.registered_coastline(registry),
        citation=lambda key: map_data.citation(registry, key),
        load_layers=load_layers,
        load_preferences=load_preferences,
        render_prepared=render_map.render_prepared,
    )


def gallery_build_collaborators(
    run_population_command: PopulationRunner = popcircles_cli.invoke_popcircles_cli,
) -> GalleryBuildCollaborators[gallery_rendering.MapRenderContext]:
    registry = load()
    presentation = load_presentation_config()

    def enrich_saved_document(document: dict[str, object]) -> dict[str, object]:
        return enrich_document(
            document,
            load_country_boundaries=lambda answer: load_country_boundaries(answer, registry),
            load_map_boundaries=lambda answer: load_map_boundaries(answer, registry),
        )

    def enrich_source(_source: Path, payload: dict[str, object]) -> dict[str, object]:
        answer = circle_of(payload)
        return enrich_document(payload, load_map_boundaries(answer, registry))

    def prepare_render_context(
        output: Path, requested_cache: Path | None
    ) -> gallery_rendering.MapRenderContext:
        coastline, coastline_attribution = map_data.registered_coastline(registry)
        cache_directory = requested_cache or output.parent / ".gallery-cache"
        try:
            cache_directory.mkdir(parents=True, exist_ok=True)
        except OSError:
            cache_directory = None
        return gallery_rendering.MapRenderContext(
            registry,
            coastline,
            coastline_attribution,
            output,
            cache_directory,
            gallery_rendering.render_context_digest(),
            (presentation.map_text.render_title, presentation.map_text.render_description),
            map_titles=presentation.map_titles,
        )

    def render_gallery_documents(
        jobs: list[RenderJob],
        context: gallery_rendering.MapRenderContext,
    ) -> tuple[list[tuple[GalleryFigure, bool]], int, float]:
        results, workers, seconds = gallery_rendering.render_documents(jobs, context)
        return [(drawn, reused) for drawn, reused in results], workers, seconds

    return GalleryBuildCollaborators(
        run_population_command=run_population_command,
        prepare_render_context=prepare_render_context,
        enrich_source=enrich_source,
        country_sweep=CountrySweepCollaborators(
            set(presentation.gallery.excluded_country_ids),
            enrich_saved_document,
            lambda message: typer.echo(message, err=True),
        ),
        build_frontend=gallery_output.build_frontend,
        write_header=gallery_output.write_header_background,
        render_documents=render_gallery_documents,
        page_data=gallery_page_data.page_data,
        report=lambda message: typer.echo(message, err=True),
    )
