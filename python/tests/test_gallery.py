import copy
import json
import shutil
import subprocess
from pathlib import Path
from typing import Any
from unittest.mock import Mock

import numpy as np
import pytest
import rasterio
from matplotlib.figure import Figure
from PIL import Image
from rasterio.transform import from_origin
from shapely.geometry import box
from shapely.geometry.base import BaseGeometry
from typer.testing import CliRunner

import population_circles.gallery.build as gallery_module
import population_circles.gallery.country_sweep as country_sweep_module
import population_circles.gallery.rendering as rendering_module
import population_circles.maps.layers as map_layers_module
import population_circles.maps.presentation_config as presentation_config_module
import population_circles.maps.render_map as render_map_module
import population_circles.maps.terrain as terrain_module
from population_circles import cli
from population_circles.applications.population_engine import PopulationRunner, WorkflowError
from population_circles.data.data_config import country_boundary_dataset_key
from population_circles.data.dataset_registry import (
    MapContextVector,
    Registry,
    SettlementVector,
    TerrainRaster,
    load,
)
from population_circles.documents.result_document import CIRCLE_KINDS, Answer, circle_of
from population_circles.gallery.build import build
from population_circles.maps.layers import (
    COASTLINE_DATASET,
    UnregisteredDatasetError,
    registered_coastline,
)
from population_circles.maps.layers import (
    load_map_layers as real_load_map_layers,
)
from population_circles.maps.layers import (
    registered_map_sources as real_registered_map_sources,
)
from test_render_map import (
    fixture_map_layers,
    fixture_registered_map_sources,
    transparent_terrain,
)

CORPUS = Path(__file__).resolve().parents[2] / "results"
CLI_ERROR_EXIT = 2
REFRESH_OPTION_COUNT = 2
DATASET = "population-count-2020-30arcsec"
WORLD_POPULATION = 1_254_363_867.93
WORLD_SHARE = 0.1616868627727305
WORLD_RADIUS_KM = 1000.0
WORLD_TERRITORY_SHARE = 0.375
COUNTRY_TERRITORY_SHARE = 0.625
EXPECTED_COUNTRY_CLI_CALLS = 2
VISIBLE_FOOTER = (
    "© Andrii Melnyk, 2026, https://github.com/turboBasic/PopCircles · "
    "© EuroGeographics for the administrative boundaries"
)
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
COASTLINE_CREDIT = "© EuroGeographics for the administrative boundaries"
AVAILABLE_CPUS = 6
EXPECTED_WORKERS = min(2, AVAILABLE_CPUS - 1)
SMALL_MAP_COUNT = 2


def use_coastline_fixture(root: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    registry = load()
    monkeypatch.setattr(gallery_module, "load", lambda: registry)
    monkeypatch.setattr(map_layers_module, "REPO_ROOT", root)
    path = root / registry.datasets[COASTLINE_DATASET].path
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(
            {
                "type": "FeatureCollection",
                "features": [
                    {
                        "type": "Feature",
                        "properties": {},
                        "geometry": {"type": "LineString", "coordinates": [[-20, 0], [20, 0]]},
                    },
                ],
            },
        ),
        encoding="utf-8",
    )


@pytest.fixture(autouse=True)
def keep_gallery_tests_serial_by_default(monkeypatch: pytest.MonkeyPatch) -> None:
    def one_worker(count: int) -> int:
        return min(1, count)

    monkeypatch.setattr(gallery_module, "load_map_boundaries", fixture_boundaries)
    monkeypatch.setattr(
        map_layers_module,
        "registered_map_sources",
        fixture_registered_map_sources,
    )
    monkeypatch.setattr(rendering_module, "render_worker_count", one_worker)
    monkeypatch.setattr(rendering_module, "load_map_boundaries", fixture_boundaries)
    monkeypatch.setattr(map_layers_module, "load_map_layers", fixture_map_layers)
    monkeypatch.setattr(render_map_module, "prepare_terrain", transparent_terrain)


def document(
    kind: str, dataset: str = DATASET, territory_share: float | None = None
) -> dict[str, Any]:
    payload: dict[str, Any] = {
        "schema_version": 1,
        "document": kind,
        "tool": "popcircles",
        "tool_version": "0.1.0",
        "earth_model": {"model": "sphere", "radius_km": 6371.0088},
        "provenance": {"digest": "0xf17aa802a6890f0c", "decimation": 10, "dataset": dataset},
        "result": {
            "centre": {"lat": 25.125, "lon": 79.708},
            "radius_km": 1000.0,
            "population": 1254363867.93,
            "share_of_total": 0.1616868627727305,
            # A sweep's own payload, carried so the skipped fixture is a document this corpus could
            # actually hold rather than a renderable one relabelled.
            "circles": [],
        },
    }
    if territory_share is not None:
        payload["result"]["territory_share"] = territory_share
    return payload


def written(directory: Path, name: str, payload: dict[str, Any]) -> None:
    (directory / name).write_text(json.dumps(payload), encoding="utf-8")


def result_territory_share(corpus: Path, name: str) -> float:
    circle = json.loads((corpus / name).read_text(encoding="utf-8"))["result"]["circle"]
    share = circle["territory_share"]
    assert 0.0 <= share <= 1.0
    return share


def test_header_background_crops_to_plate_carree_plot_without_white_margins(
    tmp_path: Path,
) -> None:
    source = tmp_path / "world.png"
    target = tmp_path / "header-background.png"
    image = Image.new("RGB", (120, 80), "white")
    image.paste(render_map_module.OCEAN, (10, 10, 110, 35))
    image.paste((100, 190, 180), (10, 35, 110, 60))
    image.save(source)

    gallery_module.crop_header_background(source, target)

    with Image.open(target) as cropped:
        assert cropped.size == (100, 49)
        assert cropped.getpixel((0, 0)) == (223, 255, 253)
        assert cropped.getpixel((50, 48)) == (100, 190, 180)


def assert_built_site(output: Path) -> None:
    page = (output / "index.html").read_text(encoding="utf-8")
    assert 'src="./assets/' in page
    assert 'href="./assets/' in page
    assert "cdn.jsdelivr.net" not in page
    assert any((output / "assets").iterdir())
    assert (output / "gallery-data.json").is_file()
    with Image.open(output / "header-background.png") as header_background:
        assert (
            abs(
                header_background.width
                - gallery_module.HEADER_CROP_ASPECT_RATIO * header_background.height
            )
            <= gallery_module.HEADER_CROP_ASPECT_TOLERANCE
        )


def image_aspect_ratio(path: Path) -> float:
    with Image.open(path) as image:
        return image.width / image.height


def assert_country_metadata_mirrors_json(output: Path, result_name: str) -> None:
    document_payload = json.loads((output / result_name).read_text(encoding="utf-8"))
    with Image.open(output / result_name.replace(".json", ".png")) as image:
        metadata = image.info
    xmp = str(metadata["XML:com.adobe.xmp"])
    for name, value in (
        ("countryName", "Alpha"),
        ("countryCode", "AA"),
        ("nearestSettlement", "Fixture Place"),
        ("nearestSettlementCountry", "Fixture Country"),
    ):
        assert f'pc:{name}="{value}"' in xmp
    assert "Title" in metadata
    assert "Copyright" in metadata
    assert "Title" in document_payload["render_metadata"]
    assert "Copyright" in document_payload["render_metadata"]
    properties = document_payload["render_metadata"]["XML:com.adobe.xmp"]
    assert properties["countryCode"] == "AA"
    assert properties["nearestSettlement"] == "Fixture Place"
    assert properties["photoshop:Country"] == "Alpha"
    assert properties["dc:creator"] == ["Andrii Melnyk"]
    assert properties["Iptc4xmpCore:CreatorContactInfo"]["Iptc4xmpCore:CiUrlWork"] == (
        "https://github.com/turboBasic/PopCircles"
    )
    assert properties["Iptc4xmpExt:LocationShown"][0]["exif:GPSLatitude"]
    assert properties["Iptc4xmpExt:LocationShown"][0]["exif:GPSLatitude"] == "25,0N"
    assert properties["Iptc4xmpExt:LocationShown"][0]["exif:GPSLongitude"] == "79,0E"
    assert properties["Iptc4xmpExt:LocationShown"][0]["Iptc4xmpExt:City"] == "Fixture Place"
    assert properties["dc:rights"] == "© Andrii Melnyk, 2026"
    assert properties["photoshop:Credit"].startswith("© Andrii Melnyk, 2026,")
    assert metadata["Title"] == properties["dc:title"]
    assert metadata["Description"] == properties["dc:description"]
    assert metadata["Copyright"] == properties["fullAttribution"]


def fixture_outline(answer: Answer, _registry: Registry) -> BaseGeometry:
    longitude = answer.circle.centre.lon
    latitude = answer.circle.centre.lat
    return box(longitude - 5, latitude - 5, longitude + 5, latitude + 5)


def fixture_boundaries(answer: Answer, registry: Registry) -> dict[str, BaseGeometry] | None:
    if answer.country is None:
        return {"AA": box(-10, -10, 10, 10)}
    return {answer.country.id: fixture_outline(answer, registry)}


def registry_without_country_boundaries() -> Registry:
    registry = load()
    return registry.model_copy(
        update={
            "datasets": {
                key: dataset
                for key, dataset in registry.datasets.items()
                if key != country_boundary_dataset_key()
            }
        }
    )


def registry_with_synthetic_map_inputs(root: Path) -> Registry:
    registry = registry_without_country_boundaries()
    datasets = dict(registry.datasets)
    terrain_key = "etopo-2022-60s"
    terrain = datasets[terrain_key]
    assert isinstance(terrain, TerrainRaster)
    terrain_path = root / terrain.path
    terrain_path.parent.mkdir(parents=True, exist_ok=True)
    values = np.full((80, 80), 500, dtype=np.float32)
    with rasterio.open(
        terrain_path,
        "w",
        driver="GTiff",
        width=80,
        height=80,
        count=1,
        dtype="float32",
        crs="EPSG:4326",
        transform=from_origin(78, 27, 0.05, 0.05),
        nodata=-9999.0,
    ) as output:
        output.write(values, 1)
    datasets[terrain_key] = terrain.model_copy(
        update={
            "width": 80,
            "height": 80,
            "epsg": 4326,
            "x_origin": 78.0,
            "y_origin": 27.0,
            "x_step": 0.05,
            "y_step": -0.05,
            "nodata": -9999.0,
        }
    )
    for key in (
        "natural-earth-10m-lakes-v5.0.0",
        "natural-earth-10m-rivers-lake-centerlines-v5.0.0",
        "natural-earth-10m-roads-v5.0.0",
    ):
        dataset = datasets[key]
        assert isinstance(dataset, MapContextVector)
        path = root / dataset.path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"type": "FeatureCollection", "features": []}))
    places = datasets["populated-places-1to10m"]
    assert isinstance(places, SettlementVector)
    place_path = root / places.path
    place_path.parent.mkdir(parents=True, exist_ok=True)
    place_path.write_text(json.dumps({"type": "FeatureCollection", "features": []}))
    return registry.model_copy(update={"datasets": datasets})


def country_cli(
    countries: list[dict[str, str]],
    swept_countries: list[dict[str, Any]],
    countries_without_population_cells: list[dict[str, str]] | None = None,
) -> tuple[PopulationRunner, list[list[str]]]:
    selection = country_selection()
    listing = {
        "schema_version": 1,
        "document": "country-list",
        "tool": "popcircles",
        "tool_version": "0.3.0",
        "earth_model": {"model": "sphere", "radius_km": 6371.0088},
        "result": {"selection": selection, "countries": countries},
    }
    sweep = country_sweep(swept_countries, selection)
    sweep["result"]["countries_without_population_cells"] = countries_without_population_cells or []
    outputs = [json.dumps(listing).encode(), json.dumps(sweep).encode()]
    covered = copy.deepcopy(sweep)
    for country in covered["result"]["countries"]:
        for record in country["records"]:
            record["circle"]["territory_share"] = COUNTRY_TERRITORY_SHARE
    calls: list[list[str]] = []

    def invoke(arguments: list[str]) -> subprocess.CompletedProcess[bytes]:
        calls.append(arguments)
        output = (
            json.dumps(covered).encode()
            if arguments[1:2] == ["coverage"]
            else outputs[min(len(calls) - 1, 1)]
        )
        return subprocess.CompletedProcess(arguments, 0, stdout=output, stderr=b"")

    return invoke, calls


def country_selection() -> dict[str, Any]:
    return {
        "combination_id": "fixture",
        "resolution": 10,
        "population_source": {
            "name": DATASET,
            "sha256": None,
            "attribution": "Population fixture credit",
        },
        "boundary_source": {
            "name": "gisco-countries-2024-1m",
            "sha256": None,
            "attribution": "Boundary fixture credit",
        },
    }


def country_sweep(
    swept_countries: list[dict[str, Any]],
    selection: dict[str, Any] | None = None,
) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "document": "country-sweep",
        "tool": "popcircles",
        "tool_version": "0.3.0",
        "earth_model": {"model": "sphere", "radius_km": 6371.0088},
        "result": {
            "selection": selection or country_selection(),
            "shares": {"from_percent": 50, "to_percent": 50, "step_percent": 1},
            "countries": swept_countries,
            "countries_without_population_cells": [],
        },
    }


def swept_country(
    identifier: str, name: str, settlement: dict[str, Any] | None = None
) -> dict[str, Any]:
    country: dict[str, Any] = {
        "country": {
            "id": identifier,
            "name": name,
            "boundary_dataset": "gisco-countries-2024-1m",
            "boundary_revision": "Countries 2024 snapshot downloaded 2026-10-22",
            "boundary_digest": "a" * 64,
        },
        "ledger": {"path": f"out/countries/{identifier}.json", "radii": 1},
        "records": [
            {
                "share": 0.5,
                "circle": {
                    "radius_km": 100,
                    "centre": {"lat": 25.0, "lon": 79.0},
                    "row": 10,
                    "col": 20,
                    "population": 50,
                    "target": {"share": 0.5, "persons": 50, "total": 100},
                    "share_achieved": 0.5,
                    "short_below": {"radius_km": 99, "population": 45},
                    "covers_whole_grid": False,
                    "predicate_slack_persons": 0,
                    "tolerance_persons": 0,
                    "stats": {
                        "radii_evaluated": 1,
                        "radii_reused": 0,
                        "searched": {
                            "levels": 1,
                            "blocks_examined": 1,
                            "blocks_pruned": 0,
                            "circles_evaluated": 1,
                            "kernels_built": 1,
                        },
                    },
                },
            },
        ],
    }
    if settlement is not None:
        country["records"][0]["circle"]["nearest_settlement"] = settlement
    return country


def configured_country(identifier: str, name: str) -> dict[str, str]:
    return {"id": identifier, "name": name}


def test_a_renderable_document_is_drawn_and_an_unrenderable_one_is_named(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    use_coastline_fixture(tmp_path, monkeypatch)
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    written(corpus, "drawable.json", document("most-populous"))
    written(corpus, "not-drawable.json", document("sweep"))
    coastline_path = tmp_path / load().datasets[COASTLINE_DATASET].path
    basemap_paths: list[Path] = []
    annotations: list[str] = []
    current_basemap = map_layers_module.basemap
    current_annotate = render_map_module.annotate

    def capture_basemap(path: Path) -> BaseGeometry:
        basemap_paths.append(path)
        return current_basemap(path)

    def capture_annotate(
        figure: Figure, y: float, text: str, size: float, alignment: str = "baseline"
    ) -> None:
        annotations.append(text)
        current_annotate(figure, y, text, size, alignment)

    monkeypatch.setattr(map_layers_module, "basemap", capture_basemap)
    monkeypatch.setattr(render_map_module, "annotate", capture_annotate)

    drawn, skipped = build(corpus, tmp_path / "gallery", country_cli([], [])[0])

    assert [one.figure.name for one in drawn] == ["drawable.png"]
    assert drawn[0].figure.read_bytes().startswith(PNG_MAGIC)
    assert [(one.document.name, one.kind) for one in skipped] == [
        ("country-sweep-fixture-10-half.json", "country-sweep"),
        ("not-drawable.json", "sweep"),
    ]
    assert_built_site(tmp_path / "gallery")
    data = json.loads((tmp_path / "gallery" / "gallery-data.json").read_text(encoding="utf-8"))
    assert data["world"] == [
        {
            "map": "drawable.png",
            "document": "drawable.json",
            "country_id": None,
            "alt": "drawable",
            "aspect_ratio": 11 / 6,
            "caption": {
                "radius_km": WORLD_RADIUS_KM,
                "share": WORLD_SHARE,
                "population": WORLD_POPULATION,
                "centre_lat": 25.125,
                "centre_lon": 79.708,
                "territory_share": 0.0,
                "territory_name": "Earth\u2019s land area",
                "settlement": None,
            },
        },
    ]
    assert "not-drawable.json" not in json.dumps(data)
    with Image.open(drawn[0].figure) as image:
        metadata = image.info
    copied_document = json.loads(
        (tmp_path / "gallery" / "drawable.json").read_text(encoding="utf-8")
    )
    assert " ".join(annotations[0].splitlines()) == metadata["Title"]
    assert " ".join(annotations[0].splitlines()).endswith(
        "Circle covers 0% of Earth\u2019s land area"
    )
    assert annotations[1] == metadata["Description"]
    assert annotations[-1].startswith(VISIBLE_FOOTER)
    assert "Terrain data:" in annotations[-1]
    assert annotations[-1].endswith("See full attribution in image metadata")
    assert "Title" in metadata
    assert "Copyright" in metadata
    assert "© Andrii Melnyk, 2026" in metadata["Copyright"]
    assert "Population data:" in metadata["Copyright"]
    assert metadata["Description"] == (
        f"Answered from population dataset {DATASET} at decimation k=10"
    )
    assert 'pc:population="1254363867.93"' in metadata["XML:com.adobe.xmp"]
    expected_result = document("most-populous")["result"]
    expected_result["territory_share"] = 0.0
    assert copied_document["result"] == expected_result
    assert copied_document["render_metadata"]["Title"] == metadata["Title"]
    assert copied_document["render_metadata"]["Description"] == metadata["Description"]
    assert copied_document["render_metadata"]["Copyright"] == metadata["Copyright"]
    assert copied_document["render_metadata"]["XML:com.adobe.xmp"]["population"] == WORLD_POPULATION
    assert basemap_paths == [coastline_path]
    assert "Coastline data:" in metadata["Copyright"]
    assert metadata["Copyright"].count(COASTLINE_CREDIT) == 1


def test_a_warm_build_reuses_unchanged_map_bytes_without_rendering(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    use_coastline_fixture(tmp_path, monkeypatch)
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    written(corpus, "world.json", document("most-populous"))
    output = tmp_path / "build" / "gallery"
    countries = [configured_country("AA", "Alpha")]
    swept = [swept_country("AA", "Alpha")]
    monkeypatch.setattr(gallery_module, "load_map_boundaries", fixture_boundaries)
    render = Mock(wraps=render_map_module.render)
    monkeypatch.setattr(rendering_module, "render", render)
    invoke = country_cli(countries, swept)[0]
    first, _ = build(corpus, output, invoke)
    original_bytes = {one.figure.name: one.figure.read_bytes() for one in first}
    expected_maps = 2
    assert len(original_bytes) == expected_maps
    assert f"0 reused, {expected_maps} rendered" in capsys.readouterr().err

    country_path = corpus / "country-fixture-10-AA-half.json"
    country_payload = json.loads(country_path.read_text(encoding="utf-8"))
    country_payload["result"]["circle"]["stats"] = {"radii_evaluated": 5}
    country_path.write_text(json.dumps(country_payload, indent=2), encoding="utf-8")
    sweep_path = corpus / "country-sweep-fixture-10-half.json"
    sweep_payload = json.loads(sweep_path.read_text(encoding="utf-8"))
    sweep_payload["result"]["countries"][0]["records"][0]["circle"]["stats"] = {
        "radii_evaluated": 5
    }
    sweep_path.write_text(json.dumps(sweep_payload, indent=2), encoding="utf-8")
    saved_results = {path.name: path.read_bytes() for path in (country_path, sweep_path)}

    render.reset_mock()
    second, _ = build(corpus, output, country_cli(countries, swept)[0])

    render.assert_not_called()
    assert {one.figure.name: one.figure.read_bytes() for one in second} == original_bytes
    assert {path.name: path.read_bytes() for path in (country_path, sweep_path)} == saved_results
    diagnostics = capsys.readouterr().err
    assert "sweep work avoided for 1 countries" in diagnostics
    assert "2 reused, 0 rendered" in diagnostics

    cache_image = output.parent / ".gallery-cache" / "world" / "map.png"
    cache_image.write_bytes(b"corrupt cache")
    third, _ = build(corpus, output, country_cli(countries, swept)[0])

    render.assert_called_once()
    assert {one.figure.name: one.figure.read_bytes() for one in third} == original_bytes


def test_map_text_rendering_change_misses_the_gallery_map_cache(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    use_coastline_fixture(tmp_path, monkeypatch)
    config_root = tmp_path / "config-root"
    config_path = config_root / "data" / "presentation.toml"
    config_path.parent.mkdir(parents=True)
    settings = presentation_config_module.PRESENTATION_CONFIG.read_text(encoding="utf-8")
    config_path.write_text(
        settings.replace("render_title = true", "render_title = false").replace(
            "render_description = true", "render_description = false"
        ),
        encoding="utf-8",
    )
    monkeypatch.setattr(presentation_config_module, "PRESENTATION_CONFIG", config_path)
    monkeypatch.setattr(rendering_module, "RENDER_INPUTS", (config_path,))
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    written(corpus, "world.json", document("most-populous"))
    output = tmp_path / "build" / "gallery"
    render = Mock(wraps=render_map_module.render)
    monkeypatch.setattr(rendering_module, "render", render)
    invoke = country_cli([], [])[0]

    first, _ = build(corpus, output, invoke)
    first_bytes = first[0].figure.read_bytes()
    render.reset_mock()
    second, _ = build(corpus, output, country_cli([], [])[0])
    render.assert_not_called()
    assert second[0].figure.read_bytes() == first_bytes

    config_path.write_text(
        settings.replace("render_description = true", "render_description = false"),
        encoding="utf-8",
    )
    third, _ = build(corpus, output, country_cli([], [])[0])

    render.assert_called_once()
    assert third[0].figure.read_bytes() != first_bytes
    with Image.open(third[0].figure) as image:
        assert "Title" in image.info
        assert "Description" in image.info
        assert "Copyright" in image.info


def test_gallery_data_uses_only_the_current_maps(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    use_coastline_fixture(tmp_path, monkeypatch)
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    written(corpus, "old.json", document("most-populous"))
    written(corpus, "current.json", document("most-populous", territory_share=0.375))
    output = tmp_path / "gallery"
    invoke = country_cli([], [])[0]

    build(corpus, output, invoke)
    (output / "assets" / "stale.js").write_text("stale", encoding="utf-8")
    (corpus / "old.json").unlink()

    build(corpus, output, country_cli([], [])[0])

    assert_built_site(output)
    assert not (output / "assets" / "stale.js").exists()
    assert (output / "current.png").is_file()
    assert (output / "current.json").is_file()
    data = json.loads((output / "gallery-data.json").read_text(encoding="utf-8"))
    assert data["world"] == [
        {
            "map": "current.png",
            "document": "current.json",
            "country_id": None,
            "alt": "current",
            "aspect_ratio": 11 / 6,
            "caption": {
                "radius_km": WORLD_RADIUS_KM,
                "share": WORLD_SHARE,
                "population": WORLD_POPULATION,
                "centre_lat": 25.125,
                "centre_lon": 79.708,
                "settlement": None,
                "territory_share": 0.0,
                "territory_name": "Earth\u2019s land area",
            },
        },
    ]
    assert "old.png" not in json.dumps(data)


def test_serial_and_concurrent_gallery_maps_are_identical(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    use_coastline_fixture(tmp_path, monkeypatch)

    def skip_frontend(_output: Path) -> None:
        return

    monkeypatch.setattr(gallery_module, "build_frontend", skip_frontend)
    payload = document("most-populous")
    sources = [
        (tmp_path / "alpha.json", payload),
        (tmp_path / "beta.json", payload),
    ]
    registry = registry_with_synthetic_map_inputs(tmp_path)
    coastline, attribution = registered_coastline(registry)
    serial_output = tmp_path / "serial"
    concurrent_output = tmp_path / "concurrent"
    serial_output.mkdir()
    concurrent_output.mkdir()
    serial_context = rendering_module.MapRenderContext(
        registry, coastline, attribution, serial_output, None, "fixture", data_root=tmp_path
    )
    concurrent_context = rendering_module.MapRenderContext(
        registry, coastline, attribution, concurrent_output, None, "fixture", data_root=tmp_path
    )

    def no_boundaries(_answer: Answer, _registry: Registry) -> dict[str, BaseGeometry]:
        return {}

    monkeypatch.setattr(gallery_module, "load_map_boundaries", no_boundaries)
    monkeypatch.setattr(rendering_module, "load_map_boundaries", no_boundaries)
    monkeypatch.setattr(
        map_layers_module,
        "registered_map_sources",
        real_registered_map_sources,
    )
    monkeypatch.setattr(map_layers_module, "load_map_layers", real_load_map_layers)
    monkeypatch.setattr(render_map_module, "prepare_terrain", terrain_module.prepare_terrain)

    serial, _ = gallery_module.build_gallery_output(tmp_path, sources, {}, [], serial_context)

    def two_workers(count: int) -> int:
        return min(2, count)

    monkeypatch.setattr(rendering_module, "render_worker_count", two_workers)
    concurrent, _ = gallery_module.build_gallery_output(
        tmp_path, sources, {}, [], concurrent_context
    )

    assert [item.figure.name for item in serial] == [item.figure.name for item in concurrent]
    assert {item.figure.name: item.figure.read_bytes() for item in serial} == {
        item.figure.name: item.figure.read_bytes() for item in concurrent
    }
    assert (serial_output / "gallery-data.json").read_bytes() == (
        concurrent_output / "gallery-data.json"
    ).read_bytes()
    assert "maps used 2 workers and took" in capsys.readouterr().err


def test_a_document_that_fails_to_draw_publishes_no_gallery_at_all(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    use_coastline_fixture(tmp_path, monkeypatch)
    registry = registry_with_synthetic_map_inputs(tmp_path)
    coastline, attribution = registered_coastline(registry)
    output = tmp_path / "gallery"
    output.mkdir()
    context = rendering_module.MapRenderContext(
        registry,
        coastline,
        attribution,
        output,
        None,
        "fixture",
        data_root=tmp_path,
    )

    def two_workers(count: int) -> int:
        return min(2, count)

    def skip_frontend(_output: Path) -> None:
        return

    monkeypatch.setattr(rendering_module, "render_worker_count", two_workers)
    monkeypatch.setattr(gallery_module, "build_frontend", skip_frontend)
    # The box this pins is "fails the build rather than publishing a gallery with a hole in it", so
    # what it asserts is the absence of the page: a collected failure would have written one.
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    written(corpus, "drawable.json", document("most-populous"))
    written(corpus, "unregistered.json", document("most-populous", "no-such-dataset"))
    with pytest.raises(UnregisteredDatasetError) as caught:
        gallery_module.build_gallery_output(
            corpus,
            [
                (source, json.loads(source.read_text(encoding="utf-8")))
                for source in sorted(corpus.glob("*.json"))
            ],
            {},
            [],
            context,
        )

    assert "unregistered.json" in " ".join(caught.value.__notes__)
    assert not (output / "index.html").exists()
    assert {source.name for source in corpus.glob("*.json")} == {
        "drawable.json",
        "unregistered.json",
    }


def test_the_committed_corpus_is_accounted_for_by_gallery(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # Keep every committed document on the Gallery parsing and classification path; representative
    # fixtures exercise actual map rendering without redrawing the full corpus in CI.
    use_coastline_fixture(tmp_path, monkeypatch)
    monkeypatch.setattr(gallery_module, "load_map_boundaries", fixture_boundaries)
    render = Mock(return_value=Figure())
    monkeypatch.setattr(rendering_module, "render", render)
    output = tmp_path / "gallery"
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    for source in CORPUS.glob("*.json"):
        shutil.copyfile(source, corpus / source.name)
    drawn, skipped = build(corpus, output, country_cli([], [])[0])

    committed = sorted(corpus.glob("*.json"))
    assert render.call_count == len(drawn)
    assert len({one.document for one in drawn}) + len(skipped) == len(committed)
    assert drawn, "the committed corpus holds no document this gallery can draw"
    zero_population = {
        source.name
        for source in committed
        if (payload := json.loads(source.read_text(encoding="utf-8"))).get("document")
        == "country-smallest"
        and payload["result"]["circle"]["target"]["total"] == 0
    }
    zero_population_ids = {
        json.loads(source.read_text(encoding="utf-8"))["result"]["country"]["id"]
        for source in committed
        if source.name in zero_population
    }
    assert zero_population_ids <= country_sweep_module.excluded_country_ids()
    assert not zero_population.intersection(one.document.name for one in drawn)
    assert any(one.country_id == "DE" for one in drawn)
    assert any(one.country_id == "CW" for one in drawn)
    assert not any(one.country_id in country_sweep_module.excluded_country_ids() for one in drawn)
    for one in drawn:
        assert one.figure.read_bytes().startswith(PNG_MAGIC)
        assert (output / one.document.name).is_file()
    figure_names = {one.figure.name for one in drawn}
    assert "world-half-30arcsec.png" in figure_names
    assert "world-most-populous-1000km-5arcmin.png" in figure_names
    expected_orthographic_maps: set[str] = set()
    for source in committed:
        payload = json.loads(source.read_text(encoding="utf-8"))
        if payload.get("document") not in {*CIRCLE_KINDS, "country-smallest"}:
            continue
        answer = circle_of(payload)
        share = (
            payload["result"]["circle"]["target"]["share"]
            if payload["document"] == "smallest"
            else answer.caption.share
        )
        if answer.country is None and share == country_sweep_module.HALF_SHARE:
            expected_orthographic_maps.add(f"{source.stem}-orthographic.png")
    assert expected_orthographic_maps <= figure_names
    for one in skipped:
        assert one.kind not in CIRCLE_KINDS


def test_world_half_target_adds_orthographic_gallery_map() -> None:
    source = CORPUS / "world-half-30arcsec.json"
    payload = json.loads(source.read_text(encoding="utf-8"))
    answer = circle_of(payload)

    assert answer.caption.share != country_sweep_module.HALF_SHARE
    assert payload["result"]["circle"]["target"]["share"] == country_sweep_module.HALF_SHARE
    assert [job.figure_suffix for job in gallery_module.render_jobs_for(source, payload, 1, 1)] == [
        "",
        "-orthographic",
    ]


def test_one_batched_sweep_publishes_each_country_result_and_names_missing_countries(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    use_coastline_fixture(tmp_path, monkeypatch)
    countries = [
        configured_country("AA", "Alpha"),
        configured_country("BB", "Beta"),
        configured_country("ZZ", "No population <yet>"),
    ]
    zero_population = swept_country("BB", "Beta")
    zero_population["records"][0]["circle"]["radius_km"] = 0
    zero_population["records"][0]["circle"]["population"] = 0
    zero_population["records"][0]["circle"]["target"]["persons"] = 0
    zero_population["records"][0]["circle"]["target"]["total"] = 0
    invoke, calls = country_cli(
        countries,
        [
            swept_country(
                "AA",
                "Alpha",
                {
                    "name": "Fixture Place",
                    "country": "Fixture Country",
                    "distance_km": 0.5,
                    "short_form": True,
                },
            ),
            zero_population,
        ],
        countries_without_population_cells=[{"id": "ZZ", "name": "No population <yet>"}],
    )
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    written(corpus, "world.json", document("most-populous"))
    output = tmp_path / "gallery"
    zero_result_name = "country-fixture-10-BB-half.json"
    output.mkdir()
    (output / zero_result_name).write_text("stale", encoding="utf-8")
    (output / zero_result_name.replace(".json", ".png")).write_bytes(b"stale")
    annotations: list[str] = []
    current_annotate = render_map_module.annotate

    def capture_annotate(
        figure: Figure, y: float, text: str, size: float, alignment: str = "baseline"
    ) -> None:
        annotations.append(text)
        current_annotate(figure, y, text, size, alignment)

    monkeypatch.setattr(gallery_module, "load_map_boundaries", fixture_boundaries)
    monkeypatch.setattr(render_map_module, "annotate", capture_annotate)

    drawn, skipped = build(corpus, output, invoke)

    result_names = [
        "country-fixture-10-AA-half.json",
        "country-fixture-10-BB-half.json",
    ]
    assert len(calls) == EXPECTED_COUNTRY_CLI_CALLS
    assert calls[0] == ["country", "list"]
    assert calls[1].count("--country") == len(countries)
    assert [
        calls[1][index + 1] for index, value in enumerate(calls[1]) if value == "--country"
    ] == [
        "AA",
        "BB",
        "ZZ",
    ]
    assert [one.document.name for one in drawn] == [result_names[0], "world.json"]
    assert [one.figure.name for one in drawn] == [
        result_names[0].replace(".json", ".png"),
        "world.png",
    ]
    assert [(one.document.name, one.kind) for one in skipped] == [
        (result_names[1], "zero-population country"),
        ("country-sweep-fixture-10-half.json", "country-sweep"),
    ]
    assert (corpus / "country-sweep-fixture-10-half.json").is_file()
    for name in result_names:
        result = json.loads((corpus / name).read_text(encoding="utf-8"))
        assert result["document"] == "country-smallest"
        assert result["result"]["circle"]["target"]["share"] == country_sweep_module.HALF_SHARE
        assert (corpus / name).is_file()
    assert (output / result_names[0]).is_file()
    assert_country_metadata_mirrors_json(output, result_names[0])
    assert not (output / result_names[1]).exists()
    assert not (output / result_names[1].replace(".json", ".png")).exists()
    assert_built_site(output)
    assert json.loads((output / "gallery-data.json").read_text(encoding="utf-8")) == {
        "world": [
            {
                "map": "world.png",
                "document": "world.json",
                "country_id": None,
                "alt": "world",
                "aspect_ratio": 11 / 6,
                "caption": {
                    "radius_km": WORLD_RADIUS_KM,
                    "share": WORLD_SHARE,
                    "population": WORLD_POPULATION,
                    "centre_lat": 25.125,
                    "centre_lon": 79.708,
                    "territory_share": 0.0,
                    "territory_name": "Earth\u2019s land area",
                    "settlement": None,
                },
            },
        ],
        "countries": [
            {
                "name": "Alpha",
                "anchor": "country-result-AA",
                "figures": [
                    {
                        "map": result_names[0].replace(".json", ".png"),
                        "document": result_names[0],
                        "country_id": "AA",
                        "alt": result_names[0].removesuffix(".json"),
                        "aspect_ratio": image_aspect_ratio(
                            output / result_names[0].replace(".json", ".png")
                        ),
                        "caption": {
                            "radius_km": 100,
                            "share": 0.5,
                            "population": 50,
                            "centre_lat": 25.0,
                            "centre_lon": 79.0,
                            "settlement": {
                                "name": "Fixture Place",
                                "country": "Fixture Country",
                                "distance_km": 0.5,
                            },
                            "territory_share": result_territory_share(corpus, result_names[0]),
                            "territory_name": "Alpha",
                        },
                    },
                ],
                "missing": False,
            },
            {
                "name": "Beta",
                "anchor": "country-missing-Beta",
                "figures": [],
                "missing": True,
            },
            {
                "name": "No population <yet>",
                "anchor": "country-missing-No%20population%20%3C" + "yet%3E",
                "figures": [],
                "missing": True,
            },
        ],
    }
    assert any(
        annotation.startswith(VISIBLE_FOOTER) and "Terrain data:" in annotation
        for annotation in annotations
    )
    assert any("Alpha: 100 km circle" in annotation for annotation in annotations)
    with Image.open(drawn[0].figure) as image:
        assert "Title" in image.info
        assert "Description" in image.info
        assert "Copyright" in image.info


def test_a_failed_render_does_not_replace_saved_country_results(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    use_coastline_fixture(tmp_path, monkeypatch)
    countries = [configured_country("AA", "Alpha")]
    swept = [swept_country("AA", "Alpha")]
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    sweep_path = corpus / "country-sweep-fixture-10-half.json"
    country_path = corpus / "country-fixture-10-AA-half.json"
    written(corpus, sweep_path.name, country_sweep(swept))
    written(corpus, country_path.name, document("country-smallest"))
    before = {path.name: path.read_bytes() for path in (sweep_path, country_path)}
    written(corpus, "bad.json", document("most-populous", "no-such-dataset"))
    invoke, _calls = country_cli(countries, swept)

    with pytest.raises(UnregisteredDatasetError):
        build(corpus, tmp_path / "gallery", invoke)

    assert {path.name: path.read_bytes() for path in (sweep_path, country_path)} == before


def test_a_failed_sweep_does_not_replace_saved_country_results(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    use_coastline_fixture(tmp_path, monkeypatch)
    countries = [configured_country("AA", "Alpha")]
    swept = [swept_country("AA", "Alpha")]
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    sweep_path = corpus / "country-sweep-fixture-10-half.json"
    country_path = corpus / "country-fixture-10-AA-half.json"
    written(corpus, sweep_path.name, country_sweep(swept))
    written(corpus, country_path.name, document("country-smallest"))
    before = {path.name: path.read_bytes() for path in (sweep_path, country_path)}
    list_invoke, calls = country_cli(countries, swept)

    def fail_sweep(arguments: list[str]) -> subprocess.CompletedProcess[bytes]:
        result = list_invoke(arguments)
        if arguments[1] == "sweep":
            return subprocess.CompletedProcess(arguments, 1, stdout=b"", stderr=b"failed")
        return result

    with pytest.raises(WorkflowError):
        build(corpus, tmp_path / "gallery", fail_sweep, country_refresh="sweep")

    assert calls[0] == ["country", "list"]
    assert calls[1][0:2] == ["country", "sweep"]
    assert len(calls) == EXPECTED_COUNTRY_CLI_CALLS
    assert {path.name: path.read_bytes() for path in (sweep_path, country_path)} == before


def test_gallery_cli_accepts_forced_country_refresh(monkeypatch: pytest.MonkeyPatch) -> None:
    build = Mock(return_value=([], []))
    monkeypatch.setattr(gallery_module, "build", build)
    runner = CliRunner()

    sweep = runner.invoke(
        cli.app,
        [
            "gallery",
            "build",
            "--input",
            "results",
            "--output",
            "build/gallery",
            "--refresh-country-sweep",
        ],
    )
    assert sweep.exit_code == 0
    assert build.call_args.kwargs["country_refresh"] == "sweep"

    coverage = runner.invoke(
        cli.app,
        [
            "gallery",
            "build",
            "--input",
            "results",
            "--output",
            "build/gallery",
            "--refresh-country-coverage",
        ],
    )
    assert coverage.exit_code == 0
    assert build.call_args.kwargs["country_refresh"] == "coverage"

    invalid = runner.invoke(
        cli.app,
        [
            "gallery",
            "build",
            "--input",
            "results",
            "--output",
            "build/gallery",
            "--refresh-country-sweep",
            "--refresh-country-coverage",
        ],
    )
    assert invalid.exit_code == CLI_ERROR_EXIT
    assert build.call_count == REFRESH_OPTION_COUNT


def test_a_failed_country_command_leaves_no_gallery_index(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    use_coastline_fixture(tmp_path, monkeypatch)
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    output = tmp_path / "gallery"
    output.mkdir()
    (output / "index.html").write_text("stale", encoding="utf-8")

    def fail(arguments: list[str]) -> subprocess.CompletedProcess[bytes]:
        return subprocess.CompletedProcess(arguments, 1, stdout=b"", stderr=b"failed")

    with pytest.raises(WorkflowError):
        build(corpus, output, fail)

    assert not (output / "index.html").exists()


def test_a_country_without_a_unique_half_record_leaves_no_gallery_index(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    use_coastline_fixture(tmp_path, monkeypatch)
    incomplete = swept_country("AA", "Alpha")
    incomplete["records"] = []
    invoke, _calls = country_cli([configured_country("AA", "Alpha")], [incomplete])
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    output = tmp_path / "gallery"
    output.mkdir()
    (output / "index.html").write_text("stale", encoding="utf-8")

    with pytest.raises(ValueError, match="has no unique 50% result"):
        build(corpus, output, invoke)

    assert not (output / "index.html").exists()
