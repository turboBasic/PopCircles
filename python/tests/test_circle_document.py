from typing import Any

import pytest
from pydantic import ValidationError

from population_circles.circle_document import (
    CIRCLE_KINDS,
    SCHEMA_VERSION,
    Circle,
    MissingDatasetError,
    MissingTableError,
    SweepRecords,
    UnsupportedDocumentError,
    circle_of,
)

# Every fixture below is a dictionary, never a file: the renderer's input is one JSON path and its
# tests open nothing at all, which is what keeps them runnable on a checkout with no raster. The
# figures are `report.rs`'s own snapshots, named so a change to one moves every assertion over it.
EARTH_RADIUS_KM = 6371.0088
EARTH_MODEL = {"model": "sphere", "radius_km": EARTH_RADIUS_KM}

DATASET = "population-count-2020-30arcsec"

MEASURED_LAT = 45.0
MEASURED_LON = 15.0
MEASURED_RADIUS_KM = 1200.0
MEASURED_POPULATION = 820.0
MEASURED_SHARE = 0.0038996366679982498

SEARCHED_LAT = -85.0
SEARCHED_RADIUS_KM = 2639
SEARCHED_POPULATION = 52635.0
SEARCHED_SHARE = 0.25031387319522913
SETTLEMENT_DISTANCE_KM = 12.345


def envelope(kind: str, result: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "document": kind,
        "tool": "popcircles",
        "tool_version": "0.1.0",
        "earth_model": EARTH_MODEL,
        "provenance": {
            "digest": "0xf17aa802a6890f0c",
            "decimation": 10,
            "dataset": DATASET,
        },
        "result": result,
    }


def measured() -> dict[str, Any]:
    return {
        "requested": {"lat": 48.0, "lon": 11.0},
        "centre": {"lat": MEASURED_LAT, "lon": MEASURED_LON},
        "row": 4,
        "col": 19,
        "radius_km": MEASURED_RADIUS_KM,
        "population": MEASURED_POPULATION,
        "total_population": 210276.0,
        "share_of_total": MEASURED_SHARE,
    }


def searched() -> dict[str, Any]:
    return {
        "ledger": {"path": "out/radii.json", "radii": 24},
        "circle": {
            "radius_km": SEARCHED_RADIUS_KM,
            "centre": {"lat": SEARCHED_LAT, "lon": 105.0},
            "population": SEARCHED_POPULATION,
            "share_achieved": SEARCHED_SHARE,
            "covers_whole_grid": False,
        },
    }


def breakdown() -> dict[str, Any]:
    return {
        "entries": [
            {
                "id": "A",
                "name": "Alpha",
                "population_inside": 30.0,
                "country_total_population": 100.0,
                "fraction_of_country_population": 0.3,
                "fraction_of_circle_population": 0.6,
            },
            {
                "id": "B",
                "name": "Beta",
                "population_inside": 10.0,
                "country_total_population": 20.0,
                "fraction_of_country_population": 0.5,
                "fraction_of_circle_population": 0.2,
            },
        ],
        "reconciliation": {"unassigned_population": 10.0, "bound_persons": 0.01},
        "provenance": {
            "boundary_dataset": "gisco-countries-2024-1m",
            "boundary_revision": "Countries 2024 snapshot",
            "boundary_digest": "a" * 64,
            "attribution": "© EuroGeographics",
            "effective_grid": {
                "width": 36,
                "height": 18,
                "origin": {"lat": 90.0, "lon": -180.0},
                "lon_step": 10.0,
                "lat_step": -10.0,
                "middle_row_cell_area_km2": 1.0,
                "spans_full_turn": True,
            },
            "decimation": 1,
            "accounting_route": "sequential cropped prefixes",
            "membership_rule_version": 1,
            "ownership_rule_version": 1,
            "aggregation_order_version": 1,
        },
    }


def locations(error: ValidationError) -> list[tuple[int | str, ...]]:
    return [detail["loc"] for detail in error.errors()]


def test_a_named_circle_reads_as_an_answer_with_separate_geometry_and_caption() -> None:
    answer = circle_of(envelope("circle", measured()))
    circle = answer.circle
    assert circle.centre.lat == MEASURED_LAT
    assert circle.centre.lon == MEASURED_LON
    assert circle.radius_km == MEASURED_RADIUS_KM
    assert circle.earth_radius_km == EARTH_RADIUS_KM
    assert answer.caption.population == MEASURED_POPULATION
    assert answer.caption.share == MEASURED_SHARE
    assert answer.provenance.dataset == DATASET
    assert answer.provenance.table == "0xf17aa802a6890f0c at decimation 10"
    assert list(Circle.model_fields) == ["centre", "radius_km", "earth_radius_km"]


def test_the_most_populous_circle_reads_as_a_circle() -> None:
    answer = circle_of(envelope("most-populous", measured()))
    circle = answer.circle
    assert circle.radius_km == MEASURED_RADIUS_KM
    assert answer.caption.share == MEASURED_SHARE


def test_a_smallest_document_reads_the_circle_under_its_ledger() -> None:
    # The share arrives under a different key here — `share_achieved`, not `share_of_total` — which
    # is why this kind is a payload shape of its own rather than a superset of the other two.
    answer = circle_of(envelope("smallest", searched()))
    circle = answer.circle
    assert circle.radius_km == SEARCHED_RADIUS_KM
    assert answer.caption.population == SEARCHED_POPULATION
    assert answer.caption.share == SEARCHED_SHARE
    assert circle.centre.lat == SEARCHED_LAT


def test_settlement_context_is_optional_and_validated_on_both_payload_shapes() -> None:
    context = {
        "name": "Fixture Place",
        "country": "Fixture Country",
        "distance_km": SETTLEMENT_DISTANCE_KM,
        "short_form": False,
        "a_later_field": "ignored",
    }
    measured_document = envelope("circle", measured())
    measured_document["result"]["nearest_settlement"] = context
    measured_answer = circle_of(measured_document)
    assert measured_answer.caption.settlement is not None
    assert measured_answer.caption.settlement.name == "Fixture Place"
    assert measured_answer.caption.settlement.distance_km == SETTLEMENT_DISTANCE_KM
    assert measured_answer.caption.settlement.short_form is False

    smallest_document = envelope("smallest", searched())
    smallest_document["result"]["circle"]["nearest_settlement"] = context
    smallest_answer = circle_of(smallest_document)
    assert smallest_answer.caption.settlement is not None
    assert smallest_answer.caption.settlement.country == "Fixture Country"

    assert circle_of(envelope("circle", measured())).caption.settlement is None
    assert circle_of(envelope("smallest", searched())).caption.settlement is None

    malformed = envelope("circle", measured())
    malformed["result"]["nearest_settlement"] = {**context, "distance_km": -1}
    with pytest.raises(ValidationError):
        circle_of(malformed)


def test_country_breakdown_is_checked_and_stays_out_of_circle_geometry() -> None:
    measured_document = envelope("circle", measured())
    measured_document["result"]["country_breakdown"] = breakdown()
    measured_answer = circle_of(measured_document)
    assert measured_answer.caption.country_breakdown is not None
    entries = measured_answer.caption.country_breakdown.entries
    assert [(entry.id, entry.population_inside) for entry in entries] == [("A", 30), ("B", 10)]
    assert entries[0].fraction_of_country_population == 0.3  # noqa: PLR2004 — 30 / 100 oracle
    assert entries[0].fraction_of_circle_population == 0.6  # noqa: PLR2004 — 30 / 50 oracle
    assert measured_answer.caption.country_breakdown.provenance.attribution == "© EuroGeographics"

    smallest_document = envelope("smallest", searched())
    smallest_document["result"]["circle"]["country_breakdown"] = breakdown()
    smallest_answer = circle_of(smallest_document)
    assert smallest_answer.caption.country_breakdown is not None
    assert smallest_answer.caption.country_breakdown.reconciliation.unassigned_population == 10  # noqa: PLR2004 — oracle remainder
    assert list(Circle.model_fields) == ["centre", "radius_km", "earth_radius_km"]

    malformed = envelope("circle", measured())
    malformed["result"]["country_breakdown"] = breakdown()
    malformed["result"]["country_breakdown"]["entries"][0]["population_inside"] = float("nan")
    with pytest.raises(ValidationError):
        circle_of(malformed)

    missing_context = envelope("circle", measured())
    missing_context["result"]["country_breakdown"] = breakdown()
    del missing_context["result"]["country_breakdown"]["provenance"]
    with pytest.raises(ValidationError):
        circle_of(missing_context)


def test_present_empty_country_breakdowns_are_valid_and_sweep_stays_not_drawable() -> None:
    empty = breakdown()
    empty["entries"] = []
    document = envelope("circle", measured())
    document["result"]["country_breakdown"] = empty
    answer = circle_of(document)
    assert answer.caption.country_breakdown is not None
    assert answer.caption.country_breakdown.entries == []

    sweep = SweepRecords.model_validate(
        {"records": [{**searched()["circle"], "country_breakdown": breakdown()}]}
    )
    assert sweep.records[0].country_breakdown is not None
    assert "sweep" not in CIRCLE_KINDS


def test_country_breakdown_rejects_duplicate_ids_and_ratios_above_one() -> None:
    duplicate = envelope("circle", measured())
    duplicate["result"]["country_breakdown"] = breakdown()
    duplicate["result"]["country_breakdown"]["entries"][1]["id"] = "A"
    with pytest.raises(ValidationError, match="identifiers must be unique"):
        circle_of(duplicate)

    invalid_ratio = envelope("circle", measured())
    invalid_ratio["result"]["country_breakdown"] = breakdown()
    invalid_ratio["result"]["country_breakdown"]["entries"][0]["fraction_of_country_population"] = (
        1.5
    )
    with pytest.raises(ValidationError):
        circle_of(invalid_ratio)


def test_a_later_schema_version_is_refused_on_the_version_field() -> None:
    document = envelope("circle", measured())
    document["schema_version"] = SCHEMA_VERSION + 1
    with pytest.raises(ValidationError) as caught:
        circle_of(document)
    assert locations(caught.value) == [("schema_version",)]


def test_an_unrecognised_kind_is_refused_on_the_document_field() -> None:
    with pytest.raises(ValidationError) as caught:
        circle_of(envelope("valeriepieris", measured()))
    assert locations(caught.value) == [("document",)]


def test_another_earth_model_is_refused_on_the_model_field() -> None:
    document = envelope("circle", measured())
    document["earth_model"] = {"model": "wgs84", "radius_km": 6378.137}
    with pytest.raises(ValidationError) as caught:
        circle_of(document)
    assert locations(caught.value) == [("earth_model", "model")]


def test_a_recognised_kind_carrying_no_circle_names_the_kind() -> None:
    # `distance` is a kind this reader knows and cannot draw, which is a different refusal from the
    # unrecognised one above and the reason it is not folded into that Literal.
    with pytest.raises(UnsupportedDocumentError) as caught:
        circle_of(envelope("distance", {"great_circle_km": 966.3013398709427}))
    assert caught.value.kind == "distance"
    assert "distance" in str(caught.value)

    with pytest.raises(UnsupportedDocumentError) as country:
        circle_of(envelope("country-smallest", {}))
    assert country.value.kind == "country-smallest"


def test_the_three_circle_bearing_kinds_are_the_ones_documented() -> None:
    assert sorted(CIRCLE_KINDS) == ["circle", "most-populous", "smallest"]


def test_a_field_the_reader_does_not_know_is_ignored_rather_than_refused() -> None:
    # `report.rs` "Growth" rules the format additive and tells consumers to ignore what they do not
    # know. A reader that refused instead would break on the next field the format publishes.
    document = envelope("circle", measured())
    document["result"]["a_field_from_a_later_release"] = 1
    document["a_document_level_field_from_a_later_release"] = 2
    assert circle_of(document).circle.radius_km == MEASURED_RADIUS_KM


def test_a_document_naming_no_dataset_is_refused_rather_than_drawn() -> None:
    # Both shapes of "cannot say": provenance absent altogether, which is a command that read no
    # cached table, and provenance from a table built before a name reached its header. Neither is
    # drawable, because the figure would credit whatever the renderer was written against.
    without = envelope("circle", measured())
    del without["provenance"]
    with pytest.raises(MissingDatasetError):
        circle_of(without)

    unnamed = envelope("circle", measured())
    del unnamed["provenance"]["dataset"]
    with pytest.raises(MissingDatasetError) as caught:
        circle_of(unnamed)
    assert "names no dataset" in str(caught.value)


def test_a_document_naming_half_a_table_is_refused_rather_than_captioned() -> None:
    # Either half missing is a caption that cannot name what answered it, and the digest alone is
    # not the half that would do: it is the raster's, identical for every table built from one.
    for field in ("digest", "decimation"):
        partial = envelope("circle", measured())
        del partial["provenance"][field]
        with pytest.raises(MissingTableError):
            circle_of(partial)


def test_a_kind_that_cannot_be_drawn_is_refused_before_its_dataset_is_looked_for() -> None:
    # The order matters: a `distance` document has no provenance at all, and reporting that as a
    # missing dataset would send its reader to rebuild a table the command never read.
    document = envelope("distance", {"great_circle_km": 966.3013398709427})
    del document["provenance"]
    with pytest.raises(UnsupportedDocumentError):
        circle_of(document)
