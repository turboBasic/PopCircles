import json
import re
from pathlib import Path

import pytest
from shapely.geometry import LineString, box

from population_circles.infrastructure.geodata.dataset_registry import Registry, parse
from population_circles.infrastructure.geodata.map_layers import (
    COASTLINE_DATASET,
    UnregisteredDatasetError,
    citation,
    load_map_layers,
    registered_coastline,
    registered_map_sources,
)
from population_circles.presentation.maps.presentation_config import (
    load as load_presentation_config,
)
from test_render_map import (
    DATASET,
    PROSE,
    REGISTRY,
    TWO_ATTRIBUTED,
    fixture_map_sources,
    normalised,
    use_coastline_fixture,
)


def test_the_citation_is_the_text_the_prose_entry_quotes() -> None:
    # data/registry.toml owns the wording and data/README.md restates it for a human, so a drift
    # between the two fails here rather than shipping a credit that matches nothing in the tree.
    assert normalised(citation(REGISTRY, DATASET)) in normalised(
        PROSE.read_text(encoding="utf-8"),
    )


def test_the_credited_dataset_owes_a_citation_at_all() -> None:
    # An empty attribution is a legal value in the registry — the coastline has one — so the footer
    # assertion below would pass vacuously on "". CC BY is the whole reason a figure carries text.
    assert citation(REGISTRY, DATASET).strip()
    assert REGISTRY.datasets[DATASET].licence != "public domain"


def test_the_credit_is_the_one_the_named_dataset_owes_and_not_a_neighbours() -> None:
    registry = parse(TWO_ATTRIBUTED)
    assert citation(registry, "first-raster") == "the credit the first dataset requires"
    assert citation(registry, "second-raster") == "the credit the second dataset requires"


def test_a_document_naming_a_dataset_the_registry_lacks_is_refused() -> None:
    with pytest.raises(UnregisteredDatasetError) as caught:
        citation(REGISTRY, "gpw-v4")
    assert caught.value.key == "gpw-v4"
    assert "gpw-v4" in str(caught.value)


def test_the_registered_coastline_uses_its_registry_path_and_credit(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    path = use_coastline_fixture(tmp_path, monkeypatch)
    coastline, credit = registered_coastline(REGISTRY)

    assert path.is_file()
    assert coastline.bounds == (-20, 0, 20, 0)
    assert credit == citation(REGISTRY, COASTLINE_DATASET)


def test_registered_map_sources_use_the_global_default_and_country_override() -> None:
    world = registered_map_sources(REGISTRY, None)
    france = registered_map_sources(REGISTRY, "FR")
    switzerland = registered_map_sources(REGISTRY, "CH")

    assert world.terrain.path == REGISTRY.datasets["etopo-2022-60s"].path
    assert france.terrain.path == world.terrain.path
    assert switzerland.terrain.path == REGISTRY.datasets["swisstopo-dhm25-200m"].path
    assert [name for name, _ in world.attributions] == ["Terrain"]
    assert switzerland.attributions == (("Terrain", "©swisstopo"),)


def test_every_configured_country_terrain_tile_is_registered() -> None:
    settings = load_presentation_config().map_sources
    for country_id, keys in settings.terrain_overlays.items():
        sources = registered_map_sources(REGISTRY, country_id)
        assert tuple(Path(source.path).stem for source in sources.terrain_overlays) == tuple(keys)
        for source in sources.terrain_overlays:
            tile = re.fullmatch(
                r"copdem-glo-90-([ns])(\d{2})-([ew])(\d{3})", Path(source.path).stem
            )
            if tile is None:
                continue
            y_hemisphere, latitude, x_hemisphere, longitude = tile.groups()
            half_cell = source.x_step / 2
            expected_x = int(longitude) * (1 if x_hemisphere == "e" else -1) - half_cell
            expected_y = int(latitude) * (1 if y_hemisphere == "n" else -1) + 1 - source.y_step / 2
            assert source.x_origin == pytest.approx(expected_x, abs=1e-9)
            assert source.y_origin == pytest.approx(expected_y, abs=1e-9)


def test_registered_map_source_missing_from_registry_is_refused() -> None:
    with pytest.raises(UnregisteredDatasetError, match="etopo-2022-60s"):
        registered_map_sources(Registry(datasets={}), None)


def test_map_context_loader_filters_roads_without_fetched_inputs(tmp_path: Path) -> None:
    sources = fixture_map_sources("FR")
    for dataset in (sources.lakes, sources.rivers, sources.roads, sources.places):
        path = dataset.file(tmp_path)
        path.parent.mkdir(parents=True, exist_ok=True)
        features: list[dict[str, object]] = []
        if dataset == sources.roads:
            for authority, min_zoom, start in (
                ("FRA", 4, [1, 2]),
                ("XNR", 4, [1, 2]),
                ("FRA", 5, [1, 2]),
                ("FRA", None, [1, 2]),
                ("FRA", 4, [100, 50]),
            ):
                features.append(
                    {
                        "type": "Feature",
                        "properties": {"sov_a3": authority, "min_zoom": min_zoom},
                        "geometry": {
                            "type": "LineString",
                            "coordinates": [start, [start[0] + 2, start[1] + 2]],
                        },
                    },
                )
        if dataset == sources.places:
            for name, population, feature_class in (
                ("Town", 120_000, "City"),
                ("Capital", 500_000, "Admin-0 capital"),
                ("Village", 50_000, "City"),
            ):
                features.append(
                    {
                        "type": "Feature",
                        "properties": {
                            "NAME": name,
                            "POP_MAX": population,
                            "FEATURECLA": feature_class,
                        },
                        "geometry": {"type": "Point", "coordinates": [2, 48]},
                    },
                )
        path.write_text(json.dumps({"type": "FeatureCollection", "features": features}))

    layers = load_map_layers(sources, tmp_path, box(0, 0, 5, 5))

    assert layers.roads == (LineString([(1, 2), (3, 4)]),)
    assert layers.lakes == ()
    assert layers.rivers == ()
    assert [(name, capital) for _, name, _, capital in layers.places[:2]] == [
        ("Capital", True),
        ("Town", False),
    ]
