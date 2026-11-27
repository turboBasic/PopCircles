from population_circles.infrastructure.geodata import map_layers
from population_circles.infrastructure.geodata.dataset_registry import Registry
from population_circles.infrastructure.geodata.map_layers import (
    MapSources,
    MapSourceSelection,
)
from population_circles.presentation.maps.presentation_config import (
    load as load_presentation_config,
)


def registered_map_sources(registry: Registry, country_id: str | None) -> MapSources:
    settings = load_presentation_config().map_sources
    terrain_key = (
        settings.terrain_overrides.get(country_id, settings.terrain_default)
        if country_id is not None
        else settings.terrain_default
    )
    overlays = (
        tuple(settings.terrain_overlays.get(country_id, [])) if country_id is not None else ()
    )
    return map_layers.registered_map_sources(
        registry,
        MapSourceSelection(
            terrain=terrain_key,
            lakes=settings.lakes,
            rivers=settings.rivers,
            roads=settings.roads,
            places=settings.places,
            terrain_overlays=overlays,
        ),
    )
