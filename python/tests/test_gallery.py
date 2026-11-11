import copy
import json
import shutil
import subprocess
from pathlib import Path
from typing import Any
from unittest.mock import Mock

import pytest
from matplotlib.figure import Figure
from shapely.geometry import box
from shapely.geometry.base import BaseGeometry

import population_circles.gallery as gallery_module
import population_circles.render_map as render_map_module
from population_circles.circle_document import CIRCLE_KINDS, Answer
from population_circles.circle_map import Invoke, WorkflowError
from population_circles.dataset_registry import Registry, load
from population_circles.gallery import build, country_documents
from population_circles.render_map import COASTLINE_DATASET, UnregisteredDatasetError

CORPUS = Path(__file__).resolve().parents[2] / "results"
DATASET = "population-count-2020-30arcsec"
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
COASTLINE_CREDIT = "© EuroGeographics for the administrative boundaries"
EXPECTED_COUNTRY_CLI_CALLS = 2


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


def assert_explorer_shell(page: str) -> None:
    assert '<aside class="country-nav-wrap" aria-label="Country navigation">' in page
    assert '<main class="content"' in page
    assert "countryBrowser.open = !mobileNavigation.matches;" in page
    assert ".country-browser > summary { display: flex;" in page
    assert "Browse countries" in page
    assert 'for="country-search">Search countries</label>' in page
    assert 'id="theme-toggle" aria-pressed="false"' in page
    assert 'id="copy-country-url"' in page
    assert 'id="open-full-size"' in page
    assert "No 50% result is available for this country." in page


def assert_map_card(page: str, image: str) -> None:
    assert '<figure class="map-card" data-country-id="AA">' in page
    assert f'<img data-src="{image}"' in page
    assert "height: auto;" in page
    assert "filter:" not in page


def fixture_outline(answer: Answer, _registry: Registry) -> BaseGeometry:
    longitude = answer.circle.centre.lon
    latitude = answer.circle.centre.lat
    return box(longitude - 5, latitude - 5, longitude + 5, latitude + 5)


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


def swept_country(identifier: str, name: str) -> dict[str, Any]:
    return {
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

... (+571 lines) [see remaining: rtk proxy git -c core.fsmonitor=false show HEAD:python/tests/test_gallery.py | tail -n +225]
