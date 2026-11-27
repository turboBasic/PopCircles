import copy
import json
from pathlib import Path
from typing import Any

import pytest
from shapely.geometry import box
from shapely.geometry.base import BaseGeometry

import population_circles.gallery.country_sweep as country_sweep_module
from population_circles.application.errors import PopulationApplicationError
from population_circles.data.dataset_registry import Registry
from population_circles.documents.result_document import Answer
from population_circles.gallery.build import country_documents
from test_gallery import (
    EXPECTED_COUNTRY_CLI_CALLS,
    configured_country,
    country_cli,
    country_sweep,
    swept_country,
    written,
)


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


def test_country_coverage_refresh_uses_saved_answers_without_running_country_sweep(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    countries = [configured_country("AA", "Alpha")]
    swept = [swept_country("AA", "Alpha")]
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    sweep_path = corpus / "country-sweep-fixture-10-half.json"
    written(corpus, sweep_path.name, country_sweep(swept))
    invoke, calls = country_cli(countries, swept)

    def country_boundaries(answer: Answer, _registry: Registry) -> dict[str, BaseGeometry]:
        assert answer.country is not None
        centre = answer.circle.centre
        return {
            answer.country.id: box(
                centre.lon - 5,
                centre.lat - 5,
                centre.lon + 5,
                centre.lat + 5,
            )
        }

    monkeypatch.setattr(country_sweep_module, "load_country_boundaries", country_boundaries)

    missing, generated = country_documents(invoke, corpus, country_refresh="coverage")

    assert calls == [["country", "list"]]
    assert missing == []
    circle = generated["country-fixture-10-AA-half.json"]["result"]["circle"]
    assert 0.0 < circle["territory_share"] < 1.0
    assert (
        circle["centre"],
        circle["radius_km"],
        circle["population"],
        circle["share_achieved"],
    ) == (
        swept[0]["records"][0]["circle"]["centre"],
        swept[0]["records"][0]["circle"]["radius_km"],
        swept[0]["records"][0]["circle"]["population"],
        swept[0]["records"][0]["circle"]["share_achieved"],
    )


def test_country_coverage_refresh_requires_a_valid_saved_sweep(tmp_path: Path) -> None:
    countries = [configured_country("AA", "Alpha")]
    invoke, calls = country_cli(countries, [swept_country("AA", "Alpha")])

    with pytest.raises(PopulationApplicationError, match="requires a valid saved country sweep"):
        country_documents(invoke, tmp_path / "missing", country_refresh="coverage")

    assert calls == [["country", "list"]]


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
    first = country_documents(first_invoke, tmp_path / "first", country_refresh="sweep")[1]
    assert "country sweep evaluated 5 radii and reused 3" in capsys.readouterr().err

    second_swept = swept_country("AA", "Alpha")
    second_swept["records"][0]["circle"]["stats"]["radii_evaluated"] = 99
    second_swept["records"][0]["circle"]["stats"]["radii_reused"] = 42
    second_invoke, _ = country_cli(countries, [second_swept])
    second = country_documents(second_invoke, tmp_path / "second", country_refresh="sweep")[1]

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
    generated = country_documents(invoke, corpus, country_refresh="sweep")[1]
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

    country_documents(invoke, corpus, country_refresh="sweep")

    assert len(calls) == EXPECTED_COUNTRY_CLI_CALLS
    assert calls[1][0:2] == ["country", "sweep"]


def test_excluded_ids_are_omitted_from_missing_country_names(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(country_sweep_module, "excluded_country_ids", lambda: {"HK"})
    countries = [configured_country("DE", "Germany"), configured_country("HK", "Hong Kong")]
    sweep = country_sweep([])
    sweep["result"]["countries_without_population_cells"] = countries

    assert country_sweep_module.missing_country_names(sweep, countries) == ["Germany"]
