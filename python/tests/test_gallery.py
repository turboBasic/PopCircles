import copy
import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any
from unittest.mock import Mock

import pytest
from matplotlib.figure import Figure
from PIL import Image
from shapely.geometry import box
from shapely.geometry.base import BaseGeometry

import population_circles.dataset_registry as dataset_registry_module
import population_circles.gallery as gallery_module
import population_circles.render_map as render_map_module
from population_circles.circle_document import CIRCLE_KINDS, Answer, circle_of
from population_circles.circle_map import Invoke, WorkflowError
from population_circles.dataset_registry import Registry, country_boundary_dataset_key, load
from population_circles.gallery import build, country_documents
from population_circles.render_map import (
    COASTLINE_DATASET,
    UnregisteredDatasetError,
    registered_coastline,
)
from test_render_map import country_circle_document

CORPUS = Path(__file__).resolve().parents[2] / "results"
DATASET = "population-count-2020-30arcsec"
WORLD_POPULATION = 1_254_363_867.93
VISIBLE_FOOTER = (
    "© Andrii Melnyk, 2026, https://github.com/turboBasic/PopCircles · "
    "© EuroGeographics for the administrative boundaries · "
    "See full attribution in image metadata"
)
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
COASTLINE_CREDIT = "© EuroGeographics for the administrative boundaries"
EXPECTED_COUNTRY_CLI_CALLS = 2
RENDER_WORKER_COUNT = gallery_module.render_worker_count
AVAILABLE_CPUS = 6
EXPECTED_WORKERS = AVAILABLE_CPUS - 1
SMALL_MAP_COUNT = 2


def use_coastline_fixture(root: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    registry = load()
    monkeypatch.setattr(gallery_module, "load", lambda: registry)
    monkeypatch.setattr(render_map_module, "REPO_ROOT", root)
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

    monkeypatch.setattr(gallery_module, "render_worker_count", one_worker)
    monkeypatch.setattr(gallery_module, "load_map_boundaries", fixture_boundaries)


def document(kind: str, dataset: str = DATASET) -> dict[str, Any]:
    return {
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


def written(directory: Path, name: str, payload: dict[str, Any]) -> None:
    (directory / name).write_text(json.dumps(payload), encoding="utf-8")


def assert_built_site(output: Path) -> None:
    page = (output / "index.html").read_text(encoding="utf-8")
    assert 'src="./assets/' in page
    assert 'href="./assets/' in page
    assert "cdn.jsdelivr.net" not in page
    assets = list((output / "assets").glob("*.js"))
    assert len(assets) == 1
    bundle = assets[0].read_text(encoding="utf-8")
    assert "gallery-data.json" in bundle
    assert "nearest city:" in bundle
    assert "Math.round" in bundle
    assert ".toFixed(1)" in bundle
    assert "maximumFractionDigits:0" in bundle
    assert "https://www.openstreetmap.org/" in bundle
    assert "_blank" in bundle
    assert "noopener" in bundle
    assert "View result data" in bundle
    assert "cdn.jsdelivr.net" not in bundle
    assert "© EuroGeographics for the administrative boundaries" in bundle
    assert (output / "gallery-data.json").is_file()


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
    assert "Title" not in metadata
    assert "Copyright" not in metadata
    assert "Title" not in document_payload["render_metadata"]
    assert "Copyright" not in document_payload["render_metadata"]
    properties = document_payload["render_metadata"]["XML:com.adobe.xmp"]
    assert properties["countryCode"] == "AA"
    assert properties["nearestSettlement"] == "Fixture Place"
    assert properties["photoshop:Country"] == "Alpha"
    assert properties["dc:creator"] == ["Andrii Melnyk"]
    assert properties["Iptc4xmpCore:CreatorContactInfo"]["Iptc4xmpCore:CiUrlWork"] == (
        "https://github.com/turboBasic/PopCircles"
    )
    assert properties["Iptc4xmpExt:LocationShown"][0]["exif:GPSLatitude"]
    assert properties["Iptc4xmpExt:LocationShown"][0]["exif:GPSLatitude"] == "51,0N"
    assert properties["Iptc4xmpExt:LocationShown"][0]["exif:GPSLongitude"] == "10,0E"
    assert properties["Iptc4xmpExt:LocationShown"][0]["Iptc4xmpExt:City"] == "Fixture Place"
    assert properties["dc:rights"] == "© Andrii Melnyk, 2026"
    assert properties["photoshop:Credit"].startswith("© Andrii Melnyk, 2026,")


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


def country_cli(
    countries: list[dict[str, str]],
    swept_countries: list[dict[str, Any]],
    countries_without_population_cells: list[dict[str, str]] | None = None,
) -> tuple[Invoke, list[list[str]]]:
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
    calls: list[list[str]] = []

    def invoke(arguments: list[str]) -> subprocess.CompletedProcess[bytes]:
        calls.append(arguments)
        output = outputs[min(len(calls) - 1, 1)]
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
    current_basemap = render_map_module.basemap
    current_annotate = render_map_module.annotate

    def capture_basemap(path: Path) -> BaseGeometry:
        basemap_paths.append(path)
        return current_basemap(path)

    def capture_annotate(
        figure: Figure, y: float, text: str, size: float, alignment: str = "baseline"
    ) -> None:
        annotations.append(text)
        current_annotate(figure, y, text, size, alignment)

    monkeypatch.setattr(render_map_module, "basemap", capture_basemap)
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
        },
    ]
    assert "not-drawable.json" not in json.dumps(data)
    with Image.open(drawn[0].figure) as image:
        metadata = image.info
    copied_document = json.loads(
        (tmp_path / "gallery" / "drawable.json").read_text(encoding="utf-8")
    )
    assert annotations == []
    assert "Title" in metadata
    assert "Copyright" in metadata
    assert "© Andrii Melnyk, 2026" in metadata["Copyright"]
    assert "Population data:" in metadata["Copyright"]
    assert metadata["Description"] == (
        f"Answered from population dataset {DATASET} at decimation k=10"
    )
    assert 'pc:population="1254363867.93"' in metadata["XML:com.adobe.xmp"]
    assert copied_document["result"] == document("most-populous")["result"]
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
    monkeypatch.setattr(gallery_module, "render", render)
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


def test_text_placement_change_misses_the_gallery_map_cache(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    use_coastline_fixture(tmp_path, monkeypatch)
    config_root = tmp_path / "config-root"
    config_path = config_root / "data" / "config.toml"
    config_path.parent.mkdir(parents=True)
    config_path.write_text(
        '[map_text]\ntitle = "metadata"\ncredits = "metadata"\n', encoding="utf-8"
    )
    monkeypatch.setattr(dataset_registry_module, "REPO_ROOT", config_root)
    monkeypatch.setattr(gallery_module, "RENDER_INPUTS", (config_path,))
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    written(corpus, "world.json", document("most-populous"))
    output = tmp_path / "build" / "gallery"
    render = Mock(wraps=render_map_module.render)
    monkeypatch.setattr(gallery_module, "render", render)
    invoke = country_cli([], [])[0]

    first, _ = build(corpus, output, invoke)
    first_bytes = first[0].figure.read_bytes()
    render.reset_mock()
    second, _ = build(corpus, output, country_cli([], [])[0])
    render.assert_not_called()
    assert second[0].figure.read_bytes() == first_bytes

    config_path.write_text('[map_text]\ntitle = "raster"\ncredits = "metadata"\n', encoding="utf-8")
    third, _ = build(corpus, output, country_cli([], [])[0])

    render.assert_called_once()
    assert third[0].figure.read_bytes() != first_bytes
    with Image.open(third[0].figure) as image:
        assert "Title" not in image.info
        assert "Copyright" in image.info


def test_gallery_data_uses_only_the_current_maps(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    use_coastline_fixture(tmp_path, monkeypatch)
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    written(corpus, "old.json", document("most-populous"))
    written(corpus, "current.json", document("most-populous"))
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
        },
    ]
    assert "old.png" not in json.dumps(data)


def test_gallery_data_uses_existing_country_anchors_and_map_names() -> None:
    data = gallery_module.page_data(
        [
            gallery_module.Drawn(Path("germany.json"), Path("germany.png"), "Germany", "DE"),
            gallery_module.Drawn(Path("france.json"), Path("france.png"), "France", "FR"),
        ],
        ["Missing country"],
    )

    assert [country["anchor"] for country in data["countries"]] == [
        "country-result-FR",
        "country-result-DE",
        "country-missing-Missing%20country",
    ]
    assert [figure["map"] for country in data["countries"] for figure in country["figures"]] == [
        "france.png",
        "germany.png",
    ]


def test_country_caption_omits_settlement_when_the_answer_has_none() -> None:
    caption = gallery_module.country_caption(circle_of(country_circle_document()))

    assert caption == {
        "radius_km": 200.0,
        "share": 0.5,
        "population": 40.0,
        "centre_lat": 51.0,
        "centre_lon": 10.0,
        "settlement": None,
    }


def test_map_cache_key_changes_for_each_render_input_category() -> None:
    coastline = box(-20, -10, 20, 10)
    outline = box(-5, -5, 5, 5)
    base = gallery_module.map_cache_key("context", b"document", coastline, outline)

    assert base != gallery_module.map_cache_key("changed context", b"document", coastline, outline)
    assert base != gallery_module.map_cache_key("context", b"changed document", coastline, outline)
    assert base != gallery_module.map_cache_key("context", b"document", box(-1, -1, 1, 1), outline)
    assert base != gallery_module.map_cache_key(
        "context", b"document", coastline, box(-1, -1, 1, 1)
    )


def test_render_cache_inputs_exclude_tool_task_configuration() -> None:
    assert CORPUS.parent / "mise.toml" not in gallery_module.RENDER_INPUTS
    assert CORPUS.parent / "pyproject.toml" not in gallery_module.RENDER_INPUTS
    assert CORPUS.parent / "uv.lock" not in gallery_module.RENDER_INPUTS


def test_render_context_hashes_source_and_configuration_files(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source = tmp_path / "renderer.py"
    source.write_text("first", encoding="utf-8")
    monkeypatch.setattr(gallery_module, "RENDER_INPUTS", (source,))
    first = gallery_module.render_context_digest()

    source.write_text("second", encoding="utf-8")

    assert gallery_module.render_context_digest() != first


def test_render_worker_count_leaves_one_cpu_free(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(os, "process_cpu_count", lambda: AVAILABLE_CPUS)

    assert RENDER_WORKER_COUNT(20) == EXPECTED_WORKERS
    assert RENDER_WORKER_COUNT(SMALL_MAP_COUNT) == SMALL_MAP_COUNT


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
    registry = registry_without_country_boundaries()
    coastline, attribution = registered_coastline(registry)
    serial_output = tmp_path / "serial"
    concurrent_output = tmp_path / "concurrent"
    serial_output.mkdir()
    concurrent_output.mkdir()
    serial_context = gallery_module.MapRenderContext(
        registry, coastline, attribution, serial_output, None, "fixture"
    )
    concurrent_context = gallery_module.MapRenderContext(
        registry, coastline, attribution, concurrent_output, None, "fixture"
    )

    def no_boundaries(_answer: Answer, _registry: Registry) -> dict[str, BaseGeometry]:
        return {}

    monkeypatch.setattr(gallery_module, "load_map_boundaries", no_boundaries)

    serial, _ = gallery_module.build_gallery_output(tmp_path, sources, {}, [], serial_context)

    def two_workers(count: int) -> int:
        return min(2, count)

    monkeypatch.setattr(gallery_module, "render_worker_count", two_workers)
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
    registry = registry_without_country_boundaries()
    monkeypatch.setattr(gallery_module, "load", lambda: registry)

    def two_workers(count: int) -> int:
        return min(2, count)

    monkeypatch.setattr(gallery_module, "render_worker_count", two_workers)
    # The box this pins is "fails the build rather than publishing a gallery with a hole in it", so
    # what it asserts is the absence of the page: a collected failure would have written one.
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    written(corpus, "drawable.json", document("most-populous"))
    written(corpus, "unregistered.json", document("most-populous", "no-such-dataset"))
    output = tmp_path / "gallery"

    with pytest.raises(UnregisteredDatasetError) as caught:
        build(corpus, output, country_cli([], [])[0])

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
    monkeypatch.setattr(gallery_module, "render", render)
    output = tmp_path / "gallery"
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    for source in CORPUS.glob("*.json"):
        shutil.copyfile(source, corpus / source.name)
    drawn, skipped = build(corpus, output, country_cli([], [])[0])

    committed = sorted(corpus.glob("*.json"))
    assert render.call_count == len(drawn)
    assert len(drawn) + len(skipped) == len(committed)
    assert drawn, "the committed corpus holds no document this gallery can draw"
    zero_population = {
        source.name
        for source in committed
        if (payload := json.loads(source.read_text(encoding="utf-8"))).get("document")
        == "country-smallest"
        and payload["result"]["circle"]["target"]["total"] == 0
    }
    assert {
        one.document.name for one in skipped if one.kind == "zero-population country"
    } == zero_population
    assert not zero_population.intersection(one.document.name for one in drawn)
    for one in drawn:
        assert one.figure.read_bytes().startswith(PNG_MAGIC)
        assert (output / one.document.name).is_file()
    figure_names = {one.figure.name for one in drawn}
    assert "world-half-30arcsec.png" in figure_names
    assert "world-most-populous-1000km-5arcmin.png" in figure_names
    for one in skipped:
        assert one.kind not in CIRCLE_KINDS


def test_one_batched_sweep_publishes_each_country_result_and_names_missing_countries(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    use_coastline_fixture(tmp_path, monkeypatch)
    countries = [
        configured_country("AA", "Alpha"),
        configured_country("BB", "Beta"),
        configured_country("CC", "No population <yet>"),
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
        countries_without_population_cells=[{"id": "CC", "name": "No population <yet>"}],
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
    monkeypatch.setattr(gallery_module, "map_text_placement", lambda: ("raster", "raster"))

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
        "CC",
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
        assert result["result"]["circle"]["target"]["share"] == gallery_module.HALF_SHARE
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
    assert VISIBLE_FOOTER in annotations
    assert any("Alpha (AA): 100 km circle" in annotation for annotation in annotations)
    with Image.open(drawn[0].figure) as image:
        assert "Title" not in image.info
        assert "Copyright" not in image.info


def test_a_matching_country_sweep_is_reused_without_running_the_sweep(tmp_path: Path) -> None:
    countries = [configured_country("AA", "Alpha"), configured_country("BB", "Beta")]
    swept = [swept_country("AA", "Alpha"), swept_country("BB", "Beta")]
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    written(corpus, "country-sweep-fixture-10-half.json", country_sweep(swept))
    invoke, calls = country_cli(countries, swept)

    missing, generated = country_documents(invoke, corpus)

    assert calls == [["country", "list"]]
    assert missing == []
    assert set(generated) == {
        "country-fixture-10-AA-half.json",
        "country-fixture-10-BB-half.json",
        "country-sweep-fixture-10-half.json",
    }
    assert "stats" not in generated["country-fixture-10-AA-half.json"]["result"]["circle"]
    assert (
        "stats"
        not in generated["country-sweep-fixture-10-half.json"]["result"]["countries"][0]["records"][
            0
        ]["circle"]
    )


def test_a_saved_sweep_without_unavailable_country_metadata_is_refreshed(tmp_path: Path) -> None:
    countries = [configured_country("AA", "Alpha")]
    swept = [swept_country("AA", "Alpha")]
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    stale = country_sweep(swept)
    del stale["result"]["countries_without_population_cells"]
    written(corpus, "country-sweep-fixture-10-half.json", stale)
    invoke, calls = country_cli(countries, swept)

    country_documents(invoke, corpus)

    assert calls[0] == ["country", "list"]
    assert calls[1][0:2] == ["country", "sweep"]


def test_sweep_with_unavailable_country_reuses_independent_of_result_order(
    tmp_path: Path,
) -> None:
    countries = [
        configured_country("AA", "Alpha"),
        configured_country("BB", "Beta"),
        configured_country("CC", "Gamma"),
    ]
    swept = [swept_country("CC", "Gamma"), swept_country("AA", "Alpha")]
    invoke, calls = country_cli(
        countries,
        swept,
        countries_without_population_cells=[{"id": "BB", "name": "Beta"}],
    )
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    cached = country_sweep(swept)
    cached["result"]["countries_without_population_cells"] = [{"id": "BB", "name": "Beta"}]
    written(corpus, "country-sweep-fixture-10-half.json", cached)

    missing, generated = country_documents(invoke, corpus)

    assert calls == [["country", "list"]]
    assert missing == ["Beta"]
    assert set(generated) == {
        "country-fixture-10-AA-half.json",
        "country-fixture-10-CC-half.json",
        "country-sweep-fixture-10-half.json",
    }


def test_country_answer_documents_are_stable_when_only_sweep_stats_change(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    countries = [configured_country("AA", "Alpha")]
    first_swept = swept_country("AA", "Alpha")
    first_swept["records"][0]["circle"]["stats"]["radii_evaluated"] = 5
    first_swept["records"][0]["circle"]["stats"]["radii_reused"] = 3
    first_invoke, _ = country_cli(countries, [first_swept])
    first = country_documents(first_invoke, tmp_path / "first", refresh_country_sweep=True)[1]
    assert "country sweep evaluated 5 radii and reused 3" in capsys.readouterr().err

    second_swept = swept_country("AA", "Alpha")
    second_swept["records"][0]["circle"]["stats"]["radii_evaluated"] = 99
    second_swept["records"][0]["circle"]["stats"]["radii_reused"] = 42
    second_invoke, _ = country_cli(countries, [second_swept])
    second = country_documents(second_invoke, tmp_path / "second", refresh_country_sweep=True)[1]

    assert {name: json.dumps(payload, indent=2) for name, payload in first.items()} == {
        name: json.dumps(payload, indent=2) for name, payload in second.items()
    }


def test_a_stats_free_sweep_is_reusable_and_reports_avoided_work(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    countries = [configured_country("AA", "Alpha")]
    swept = [swept_country("AA", "Alpha")]
    invoke, calls = country_cli(countries, swept)
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    generated = country_documents(invoke, corpus, refresh_country_sweep=True)[1]
    sweep_path = corpus / "country-sweep-fixture-10-half.json"
    sweep_path.write_text(json.dumps(generated[sweep_path.name]), encoding="utf-8")
    calls.clear()

    country_documents(invoke, corpus)

    assert calls == [["country", "list"]]
    diagnostics = capsys.readouterr().err
    assert "sweep work avoided for 1 countries" in diagnostics
    assert "radii evaluated" not in diagnostics


@pytest.mark.parametrize(
    ("path", "value", "raw"),
    [
        (("result", "selection", "combination_id"), "other", None),
        (("result", "selection", "resolution"), 5, None),
        (("result", "selection", "population_source", "name"), "other", None),
        (("result", "selection", "population_source", "sha256"), "a" * 64, None),
        (("result", "selection", "boundary_source", "name"), "other", None),
        (("result", "selection", "boundary_source", "sha256"), "b" * 64, None),
        (("result", "countries", 0, "country", "id"), "XX", None),
        (("result", "countries", 0, "country", "name"), "Other", None),
        (("result", "countries"), [swept_country("AA", "Alpha")], None),
        (
            ("result", "countries"),
            [swept_country("AA", "Alpha"), swept_country("AA", "Alpha")],
            None,
        ),
        (("result", "countries", 0, "records"), [], None),
        (
            ("result", "countries", 0, "records"),
            [swept_country("AA", "Alpha")["records"][0]] * 2,
            None,
        ),
        (None, None, "{"),
    ],
)
def test_an_invalid_country_sweep_cache_runs_one_fresh_sweep(
    tmp_path: Path, path: tuple[str | int, ...] | None, value: object, raw: str | None
) -> None:
    countries = [configured_country("AA", "Alpha"), configured_country("BB", "Beta")]
    swept = [swept_country("AA", "Alpha"), swept_country("BB", "Beta")]
    cache: Any = copy.deepcopy(country_sweep(swept))
    if path is not None:
        target = cache
        for key in path[:-1]:
            target = target[key]
        target[path[-1]] = value

    corpus = tmp_path / "corpus"
    corpus.mkdir()
    cache_path = corpus / "country-sweep-fixture-10-half.json"
    cache_path.write_text(raw if raw is not None else json.dumps(cache))
    invoke, calls = country_cli(countries, swept)

    country_documents(invoke, corpus)

    assert calls[0] == ["country", "list"]
    assert calls[1][0:2] == ["country", "sweep"]
    assert len(calls) == EXPECTED_COUNTRY_CLI_CALLS


def test_refresh_bypasses_a_matching_country_sweep_cache(tmp_path: Path) -> None:
    countries = [configured_country("AA", "Alpha")]
    swept = [swept_country("AA", "Alpha")]
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    written(corpus, "country-sweep-fixture-10-half.json", country_sweep(swept))
    invoke, calls = country_cli(countries, swept)

    country_documents(invoke, corpus, refresh_country_sweep=True)

    assert len(calls) == EXPECTED_COUNTRY_CLI_CALLS
    assert calls[1][0:2] == ["country", "sweep"]


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
        build(corpus, tmp_path / "gallery", fail_sweep, refresh_country_sweep=True)

    assert calls[0] == ["country", "list"]
    assert calls[1][0:2] == ["country", "sweep"]
    assert len(calls) == EXPECTED_COUNTRY_CLI_CALLS
    assert {path.name: path.read_bytes() for path in (sweep_path, country_path)} == before


def test_gallery_cli_accepts_forced_country_sweep_refresh() -> None:
    args = gallery_module.parse(
        ["--input", "results", "--output", "build/gallery", "--refresh-country-sweep"]
    )

    assert args.refresh_country_sweep


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
