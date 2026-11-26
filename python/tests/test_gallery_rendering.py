import importlib.metadata
import os
from pathlib import Path
from unittest.mock import Mock

import pytest
from shapely.geometry import box

import population_circles.gallery.rendering as rendering_module
from population_circles.maps.frame import ORTHOGRAPHIC
from population_circles.result_document import circle_of
from test_gallery import (
    AVAILABLE_CPUS,
    CORPUS,
    COUNTRY_TERRITORY_SHARE,
    EXPECTED_WORKERS,
    SMALL_MAP_COUNT,
    WORLD_POPULATION,
    WORLD_RADIUS_KM,
    WORLD_SHARE,
    WORLD_TERRITORY_SHARE,
    document,
)
from test_render_map import country_result_document

RENDER_WORKER_COUNT = rendering_module.render_worker_count


def test_figure_caption_includes_country_and_world_answers() -> None:
    caption = rendering_module.figure_caption(circle_of(country_result_document()))

    assert caption == {
        "radius_km": 200.0,
        "share": 0.5,
        "population": 40.0,
        "centre_lat": 51.0,
        "centre_lon": 10.0,
        "settlement": None,
    }

    world = rendering_module.figure_caption(circle_of(document("most-populous")))
    assert world["radius_km"] == WORLD_RADIUS_KM
    assert world["share"] == WORLD_SHARE
    assert world["population"] == WORLD_POPULATION
    assert "territory_share" not in world

    world_with_territory = rendering_module.figure_caption(
        circle_of(document("most-populous", territory_share=WORLD_TERRITORY_SHARE))
    )
    assert world_with_territory.get("territory_share") == WORLD_TERRITORY_SHARE
    assert world_with_territory.get("territory_name") == "Earth\u2019s land area"

    country = country_result_document()
    country["result"]["circle"]["territory_share"] = COUNTRY_TERRITORY_SHARE
    assert (
        rendering_module.figure_caption(circle_of(country)).get("territory_share")
        == COUNTRY_TERRITORY_SHARE
    )
    assert rendering_module.figure_caption(circle_of(country)).get("territory_name") == "Germany"


def test_map_cache_key_changes_for_each_render_input_category() -> None:
    coastline = box(-20, -10, 20, 10)
    outline = box(-5, -5, 5, 5)
    base = rendering_module.map_cache_key("context", b"document", coastline, outline)

    assert base != rendering_module.map_cache_key(
        "changed context", b"document", coastline, outline
    )
    assert base != rendering_module.map_cache_key(
        "context", b"changed document", coastline, outline
    )
    assert base != rendering_module.map_cache_key(
        "context", b"document", box(-1, -1, 1, 1), outline
    )
    assert base != rendering_module.map_cache_key(
        "context", b"document", coastline, box(-1, -1, 1, 1)
    )
    assert base != rendering_module.map_cache_key(
        "context", b"document", coastline, outline, ORTHOGRAPHIC
    )


def test_render_cache_inputs_exclude_tool_task_configuration() -> None:
    package = CORPUS.parent / "python" / "src" / "population_circles"
    python_sources = [path for path in rendering_module.RENDER_INPUTS if path.suffix == ".py"]
    assert all(path.is_file() for path in python_sources)
    assert all(path.is_relative_to(package) for path in python_sources)
    assert CORPUS.parent / "mise.toml" not in rendering_module.RENDER_INPUTS
    assert CORPUS.parent / "pyproject.toml" not in rendering_module.RENDER_INPUTS
    assert CORPUS.parent / "uv.lock" not in rendering_module.RENDER_INPUTS
    assert CORPUS.parent / "data" / "presentation.toml" in rendering_module.RENDER_INPUTS
    assert CORPUS.parent / "data" / "registry.toml" in rendering_module.RENDER_INPUTS
    assert package / "gallery" / "rendering.py" in rendering_module.RENDER_INPUTS
    assert package / "data_config.py" in rendering_module.RENDER_INPUTS
    assert CORPUS.parent / "python" / "src" / "population_circles" / "maps/labels.py" in (
        rendering_module.RENDER_INPUTS
    )
    for unrelated in (
        "gallery/build.py",
        "gallery/country_sweep.py",
        "workflows/circle_map.py",
        "workflows/rust_cli.py",
        "gallery/page_data.py",
        "render_globe.py",
    ):
        assert CORPUS.parent / "python" / "src" / "population_circles" / unrelated not in (
            rendering_module.RENDER_INPUTS
        )
    assert CORPUS.parent / "python" / "src" / "population_circles" / "maps/terrain.py" in (
        rendering_module.RENDER_INPUTS
    )


def test_render_context_includes_rasterio_version(monkeypatch: pytest.MonkeyPatch) -> None:
    version = Mock(wraps=importlib.metadata.version)
    monkeypatch.setattr(importlib.metadata, "version", version)

    rendering_module.render_context_digest()

    assert "rasterio" in [call.args[0] for call in version.call_args_list]


def test_render_context_hashes_source_and_configuration_files(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source = tmp_path / "renderer.py"
    source.write_text("first", encoding="utf-8")
    monkeypatch.setattr(rendering_module, "RENDER_INPUTS", (source,))
    first = rendering_module.render_context_digest()

    source.write_text("second", encoding="utf-8")

    assert rendering_module.render_context_digest() != first


def test_map_source_identity_hashes_file_contents(tmp_path: Path) -> None:
    source = tmp_path / "terrain.tif"
    source.write_bytes(b"first terrain")
    first = rendering_module.source_identity((source,), {})

    source.write_bytes(b"updated terrain")

    updated = rendering_module.source_identity((source,), {})
    assert updated != first
    coastline = box(-20, -10, 20, 10)
    outline = box(-5, -5, 5, 5)
    assert rendering_module.map_cache_key("context", first, coastline, outline) != (
        rendering_module.map_cache_key("context", updated, coastline, outline)
    )


@pytest.mark.parametrize(
    ("source_kind", "before", "after"),
    [
        ("terrain-checksum", "terrain sha256 = 111", "terrain sha256 = 222"),
        ("context-checksum", "roads sha256 = 111", "roads sha256 = 222"),
        ("palette", "ELEVATION_STOPS = low", "ELEVATION_STOPS = high"),
        ("illumination", "HILLSHADE_ALPHA = 0.34", "HILLSHADE_ALPHA = 0.35"),
        ("renderer", "circle below roads", "circle above roads"),
    ],
)
def test_map_cache_key_changes_when_terrain_inputs_or_presentation_change(
    source_kind: str,
    before: str,
    after: str,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    source = tmp_path / f"{source_kind}.txt"
    source.write_text(before, encoding="utf-8")
    monkeypatch.setattr(rendering_module, "RENDER_INPUTS", (source,))
    coastline = box(-20, -10, 20, 10)
    outline = box(-5, -5, 5, 5)
    context = rendering_module.render_context_digest()
    cached_key = rendering_module.map_cache_key(context, b"document", coastline, outline)
    assert rendering_module.map_cache_key(context, b"document", coastline, outline) == cached_key

    source.write_text(after, encoding="utf-8")
    updated_context = rendering_module.render_context_digest()

    assert (
        rendering_module.map_cache_key(updated_context, b"document", coastline, outline)
        != cached_key
    )


def test_render_worker_count_is_capped_at_two(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(os, "process_cpu_count", lambda: AVAILABLE_CPUS)

    assert RENDER_WORKER_COUNT(20) == EXPECTED_WORKERS
    assert RENDER_WORKER_COUNT(SMALL_MAP_COUNT) == SMALL_MAP_COUNT
    assert RENDER_WORKER_COUNT(1) == 1
