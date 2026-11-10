import copy
import json
import shutil
import subprocess
from pathlib import Path
from typing import Any

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


def fixture_outline(answer: Answer, _registry: Registry) -> BaseGeometry:
    longitude = answer.circle.centre.lon
    latitude = answer.circle.centre.lat
    return box(longitude - 5, latitude - 5, longitude + 5, latitude + 5)


def country_cli(
    countries: list[dict[str, str]], swept_countries: list[dict[str, Any]]
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
    page = (tmp_path / "gallery" / "index.html").read_text(encoding="utf-8")
    assert "Committed documents with no figure" not in page
    assert "not-drawable.json" not in page
    footer = annotations[-1]
    assert basemap_paths == [coastline_path]
    assert "Coastline credit:" in footer
    assert footer.count(COASTLINE_CREDIT) == 1


def test_a_document_that_fails_to_draw_publishes_no_gallery_at_all(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    use_coastline_fixture(tmp_path, monkeypatch)
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


def test_the_committed_corpus_draws_every_kind_this_reader_holds(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # The gallery CI publishes, built here so a broken render fails a pull request. Which documents
    # are drawn is read off the corpus rather than listed: a kind added to the reader, or a document
    # added to `results/`, moves this assertion without editing it.
    use_coastline_fixture(tmp_path, monkeypatch)
    monkeypatch.setattr(gallery_module, "load_outline", fixture_outline)
    output = tmp_path / "gallery"
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    for source in CORPUS.glob("*.json"):
        shutil.copyfile(source, corpus / source.name)
    drawn, skipped = build(corpus, output, country_cli([], [])[0])

    committed = sorted(corpus.glob("*.json"))
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
    invoke, calls = country_cli(countries, [swept_country("AA", "Alpha"), zero_population])
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

    monkeypatch.setattr(gallery_module, "load_outline", fixture_outline)
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
    assert not (output / result_names[1]).exists()
    assert not (output / result_names[1].replace(".json", ".png")).exists()
    page = (output / "index.html").read_text(encoding="utf-8")
    assert f'href="{result_names[0]}"' in page
    assert result_names[1] not in page
    assert "No population &lt;yet&gt;" in page
    assert "Population credit: Population fixture credit" in " ".join(annotations)
    assert "Boundary credit: Boundary fixture credit" in " ".join(annotations)


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
    path = corpus / "country-sweep-fixture-10-half.json"
    path.write_text(raw if raw is not None else json.dumps(cache))
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


def test_a_failed_sweep_does_not_replace_saved_country_results(tmp_path: Path) -> None:
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
