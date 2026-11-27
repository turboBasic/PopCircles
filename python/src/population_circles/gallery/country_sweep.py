import json
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal
from urllib.parse import quote

from population_circles.application.errors import PopulationApplicationError
from population_circles.application.ports import PopulationRunner
from population_circles.application.use_cases.territory_coverage import enrich_document
from population_circles.documents.result_document import CIRCLE_KINDS, Envelope, circle_of
from population_circles.infrastructure.geodata.country_boundaries import (
    load_country_boundaries,
    load_map_boundaries,
)
from population_circles.infrastructure.geodata.dataset_registry import load
from population_circles.presentation.maps.presentation_config import (
    load as load_presentation_config,
)

HALF_SHARE = 0.5
HALF_SHARE_PERCENT = "50"
CountryRefresh = Literal["sweep", "coverage"]


@dataclass(frozen=True)
class CountrySweepRequest:
    run_population_command: PopulationRunner
    corpus: Path
    selection: dict[str, Any]
    countries: list[dict[str, Any]]
    refresh: CountryRefresh | None
    report: Callable[[str], None]


def excluded_country_ids() -> set[str]:
    return set(load_presentation_config().gallery.excluded_country_ids)


def country_result_document(
    sweep: dict[str, Any], selection: dict[str, Any], swept_country: dict[str, Any]
) -> dict[str, Any]:
    country = swept_country["country"]
    record = next(record for record in swept_country["records"] if record["share"] == HALF_SHARE)
    return {
        "schema_version": sweep["schema_version"],
        "document": "country-smallest",
        "tool": sweep["tool"],
        "tool_version": sweep["tool_version"],
        "earth_model": sweep["earth_model"],
        "result": {
            "selection": selection,
            "country": country,
            "population_dataset": selection["population_source"]["name"],
            "decimation": selection["resolution"],
            "ledger": swept_country["ledger"],
            "circle": {key: value for key, value in record["circle"].items() if key != "stats"},
        },
    }


def stable_country_sweep(sweep: dict[str, Any]) -> dict[str, Any]:
    return {
        **sweep,
        "result": {
            **sweep["result"],
            "countries": [
                {
                    **country,
                    "records": [
                        {
                            **record,
                            "circle": {
                                key: value
                                for key, value in record["circle"].items()
                                if key != "stats"
                            },
                        }
                        for record in country["records"]
                    ],
                }
                for country in sweep["result"]["countries"]
            ],
        },
    }


def missing_country_names(sweep: dict[str, Any], countries: list[dict[str, Any]]) -> list[str]:
    expected = {country["id"]: country["name"] for country in countries}
    accounted_for: set[str] = set()
    for country in sweep["result"]["countries"]:
        identity = country["country"]
        country_id = identity["id"]
        if country_id not in expected or expected[country_id] != identity["name"]:
            message = f"country sweep: unexpected feature ID {country_id!r}"
            raise ValueError(message)
        if country_id in accounted_for:
            message = f"country sweep: duplicate feature ID {country_id!r}"
            raise ValueError(message)
        accounted_for.add(country_id)
    missing: list[str] = []
    for country in sweep["result"]["countries_without_population_cells"]:
        country_id = country["id"]
        if country_id not in expected or expected[country_id] != country["name"]:
            message = f"country sweep: unexpected unavailable feature ID {country_id!r}"
            raise ValueError(message)
        if country_id in accounted_for:
            message = f"country sweep: duplicate feature ID {country_id!r}"
            raise ValueError(message)
        accounted_for.add(country_id)
        missing.append(country_id)
    if accounted_for != expected.keys():
        message = "country sweep: result does not account for every configured country"
        raise ValueError(message)
    excluded = excluded_country_ids()
    return [expected[country_id] for country_id in missing if country_id not in excluded]


def cached_sweep(
    path: Path, selection: dict[str, Any], countries: list[dict[str, Any]]
) -> dict[str, Any] | None:
    try:
        sweep: Any = json.loads(path.read_text(encoding="utf-8"))
        if Envelope.model_validate(sweep).document != "country-sweep":
            return None
        if sweep["result"]["selection"] != selection:
            return None
        swept_countries = sweep["result"]["countries"]
        missing_country_names(sweep, countries)
        for country in swept_countries:
            if sum(record["share"] == HALF_SHARE for record in country["records"]) != 1:
                return None
            circle_of(country_result_document(sweep, selection, country))
    except OSError, ValueError, TypeError, KeyError, IndexError:
        return None
    return sweep


def report_sweep_work(sweep: dict[str, Any], report: Callable[[str], None]) -> None:
    evaluated = reused = 0
    for country in sweep["result"]["countries"]:
        for record in country["records"]:
            stats = record["circle"].get("stats", {})
            evaluated += stats.get("radii_evaluated", 0)
            reused += stats.get("radii_reused", 0)
    report(f"Gallery: country sweep evaluated {evaluated} radii and reused {reused}")


def enrich_saved_document(document: dict[str, Any]) -> dict[str, Any]:
    registry = load()
    return enrich_document(
        document,
        load_country_boundaries=lambda answer: load_country_boundaries(answer, registry),
        load_map_boundaries=lambda answer: load_map_boundaries(answer, registry),
    )


def coverage_sweep(path: Path, selection: dict[str, Any]) -> dict[str, Any]:
    sweep = json.loads(path.read_text(encoding="utf-8"))
    if sweep.get("document") != "country-sweep":
        message = "country coverage: saved document is not a country-sweep"
        raise ValueError(message)
    if sweep["result"]["selection"] != selection:
        message = "country coverage: saved sweep selection does not match configuration"
        raise ValueError(message)
    return enrich_saved_document(sweep)


def refresh_coverage(corpus: Path) -> None:
    supported = CIRCLE_KINDS | {"country-smallest", "country-sweep"}
    for path in sorted(corpus.glob("*.json")):
        document: Any = json.loads(path.read_text(encoding="utf-8"))
        if Envelope.model_validate(document).document not in supported:
            continue
        enriched = enrich_saved_document(document)
        path.write_text(json.dumps(enriched, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def country_sweep(
    request: CountrySweepRequest,
) -> dict[str, Any]:
    run_population_command = request.run_population_command
    corpus = request.corpus
    selection = request.selection
    countries = request.countries
    refresh = request.refresh
    report = request.report
    filename = (
        f"country-sweep-{quote(selection['combination_id'], safe='')}-"
        f"{selection['resolution']}-half.json"
    )
    saved = None if refresh == "sweep" else cached_sweep(corpus / filename, selection, countries)
    country_ids = [country["id"] for country in countries]
    if refresh == "coverage":
        if saved is None:
            message = "country coverage refresh requires a valid saved country sweep"
            raise PopulationApplicationError(message)
        sweep = coverage_sweep(corpus / filename, selection)
        report(f"Gallery: refreshed territory coverage for {len(countries)} saved country results")
        return sweep
    if saved is not None and refresh != "sweep":
        report(
            "Gallery: reused saved 50% country sweep; "
            f"sweep work avoided for {len(country_ids)} countries"
        )
        return saved
    arguments = [
        "country",
        "sweep",
        "--combination",
        selection["combination_id"],
        "--resolution",
        str(selection["resolution"]),
        "--from",
        HALF_SHARE_PERCENT,
        "--to",
        HALF_SHARE_PERCENT,
        "--step",
        "1",
    ]
    for country_id in country_ids:
        arguments.extend(["--country", country_id])
    report(f"Gallery: calculating 50% circles for {len(country_ids)} countries")
    swept = run_population_command("country sweep", arguments)
    result = json.loads(swept.stdout)
    if result.get("document") != "country-sweep":
        message = "country sweep: Rust CLI returned a document other than country-sweep"
        raise ValueError(message)
    report_sweep_work(result, report)
    return result
