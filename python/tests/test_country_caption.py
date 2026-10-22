from population_circles.circle_document import Answer, circle_of
from population_circles.country_caption import lines_for
from test_circle_document import envelope, measured


def breakdown(entries: list[dict[str, object]]) -> dict[str, object]:
    return {
        "entries": entries,
        "reconciliation": {"unassigned_population": 10, "bound_persons": 0.01},
        "provenance": {
            "boundary_dataset": "gisco-countries-2024-1m",
            "boundary_revision": "Countries 2024 snapshot",
            "boundary_digest": "a" * 64,
            "attribution": "© EuroGeographics",
            "effective_grid": {
                "width": 36,
                "height": 18,
                "origin": {"lat": 90, "lon": -180},
                "lon_step": 10,
                "lat_step": -10,
                "middle_row_cell_area_km2": 1,
                "spans_full_turn": True,
            },
            "decimation": 1,
            "accounting_route": "sequential cropped prefixes",
            "membership_rule_version": 1,
            "ownership_rule_version": 1,
            "aggregation_order_version": 1,
        },
    }


def answer_with(entries: list[dict[str, object]]) -> Answer:
    document = envelope("circle", measured())
    document["result"]["country_breakdown"] = breakdown(entries)
    return circle_of(document)


def test_country_lines_keep_both_denominators_null_ratios_and_tiny_entries() -> None:
    answer = answer_with(
        [
            {
                "id": "A",
                "name": "Alpha",
                "population_inside": 30,
                "country_total_population": 100,
                "fraction_of_country_population": 0.3,
                "fraction_of_circle_population": 0.6,
            },
            {
                "id": "B",
                "name": "Beta",
                "population_inside": 10,
                "country_total_population": 20,
                "fraction_of_country_population": 0.5,
                "fraction_of_circle_population": 0.2,
            },
            {
                "id": "T",
                "name": "Tiny",
                "population_inside": 0.000001,
                "country_total_population": 0,
                "fraction_of_country_population": None,
                "fraction_of_circle_population": 0.00000001,
            },
        ]
    )

    lines = lines_for(answer)

    assert lines[1] == (
        "Alpha (A): 30 people; 30.0% of country population; 60.0% of circle population"
    )
    assert lines[2] == (
        "Beta (B): 10 people; 50.0% of country population; 20.0% of circle population"
    )
    assert (
        "Tiny (T): 1e-06 people; unavailable of country population; 0.0% of circle population"
        in lines
    )
    assert any("including overlaps, remain here" in line for line in lines)
    assert "Boundary credit: © EuroGeographics" in lines


def test_an_empty_breakdown_says_no_country_domain_intersects() -> None:
    lines = lines_for(answer_with([]))
    assert lines[1] == "No country domains intersect this circle on the accounting grid."


def test_large_country_populations_keep_grouped_fixed_point_formatting() -> None:
    lines = lines_for(
        answer_with(
            [
                {
                    "id": "A",
                    "name": "Alpha",
                    "population_inside": 1_234_567.89,
                    "country_total_population": 2_000_000,
                    "fraction_of_country_population": 0.6,
                    "fraction_of_circle_population": 0.3,
                }
            ]
        )
    )

    assert lines[1].startswith("Alpha (A): 1,234,567.89 people;")


def test_tiny_positive_country_populations_do_not_round_to_zero() -> None:
    lines = lines_for(
        answer_with(
            [
                {
                    "id": "A",
                    "name": "Alpha",
                    "population_inside": 0.0000001,
                    "country_total_population": 1,
                    "fraction_of_country_population": 0.0000001,
                    "fraction_of_circle_population": 0.0000001,
                }
            ]
        )
    )

    assert lines[1].startswith("Alpha (A): 1e-07 people;")
