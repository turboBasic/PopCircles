import pytest

from population_circles.documents.result_document import Answer, circle_of
from population_circles.documents.result_document import _Settlement as Settlement
from population_circles.presentation.maps.render_map import title_of
from test_render_map import CENTRE_LAT, CENTRE_LON, country_result_document, document

DEFAULT_SETTLEMENT = Settlement(
    name="Fixture Place",
    country="Fixture Country",
    distance_km=12.345,
    short_form=False,
)


def answer_with_settlement(
    *,
    latitude: float = CENTRE_LAT,
    longitude: float = CENTRE_LON,
    settlement: Settlement | None = DEFAULT_SETTLEMENT,
) -> Answer:
    payload = document()
    payload["result"]["centre"] = {"lat": latitude, "lon": longitude}
    if settlement is not None:
        payload["result"]["nearest_settlement"] = settlement.model_dump()
    return circle_of(payload)


@pytest.mark.parametrize(
    ("latitude", "longitude", "settlement", "expected"),
    [
        (
            CENTRE_LAT,
            CENTRE_LON,
            Settlement(
                name="Fixture Place", country="Fixture Country", distance_km=10, short_form=True
            ),
            (
                "1,000 km circle holding 16% of the population (1,254,363,868 people); "
                "Center: 25.1250, 79.7080"
            ),
        ),
        (
            CENTRE_LAT,
            CENTRE_LON,
            Settlement(
                name="Fixture Place", country="Fixture Country", distance_km=500, short_form=False
            ),
            (
                "1,000 km circle holding 16% of the population (1,254,363,868 people); "
                "Center: 25.1250, 79.7080"
            ),
        ),
        (
            CENTRE_LAT,
            CENTRE_LON,
            None,
            (
                "1,000 km circle holding 16% of the population (1,254,363,868 people); "
                "Center: 25.1250, 79.7080"
            ),
        ),
        (
            -12.345,
            -0.5,
            Settlement(
                name="Fixture Place",
                country="Fixture Country",
                distance_km=12.345,
                short_form=False,
            ),
            (
                "1,000 km circle holding 16% of the population (1,254,363,868 people); "
                "Center: -12.3450, -0.5000"
            ),
        ),
    ],
)
def test_figure_title_uses_the_answer_centre_without_settlement_details(
    latitude: float,
    longitude: float,
    settlement: Settlement | None,
    expected: str,
) -> None:
    answer = answer_with_settlement(latitude=latitude, longitude=longitude, settlement=settlement)
    assert title_of(answer) == expected


def test_long_figure_place_stays_in_the_one_line_title() -> None:
    answer = answer_with_settlement(
        settlement=Settlement(
            name="An exceptionally long settlement name that should wrap rather than clip",
            country="A country with a long descriptive name",
            distance_km=125,
            short_form=False,
        ),
    )
    title = title_of(answer)
    assert "\n" not in title
    assert "An exceptionally long settlement name that should wrap rather than clip" not in title


def test_country_figure_title_uses_nested_nearest_settlement() -> None:
    payload = country_result_document()
    payload["result"]["circle"]["nearest_settlement"] = DEFAULT_SETTLEMENT.model_dump()

    title = title_of(circle_of(payload))

    assert title == (
        "Germany: 200 km circle holding 50% of the population (40 people); Center: 51.0000, 10.0000"
    )
