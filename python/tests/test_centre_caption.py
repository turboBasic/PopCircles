from urllib.parse import parse_qs, urlparse

import pytest

from population_circles.centre_caption import markdown_centre, plain_centre
from population_circles.circle_document import Answer, Settlement, circle_of
from population_circles.render_map import title_of
from test_render_map import CENTRE_LAT, CENTRE_LON, document

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
    ("latitude", "longitude", "expected"),
    [
        (CENTRE_LAT, CENTRE_LON, "25.12 N, 79.71 E"),
        (-12.345, -0.5, "12.35 S, 0.50 W"),
    ],
)
def test_markdown_links_the_displayed_centre_to_its_original_coordinates(
    latitude: float,
    longitude: float,
    expected: str,
) -> None:
    answer = answer_with_settlement(latitude=latitude, longitude=longitude)
    rendered = markdown_centre(answer)
    assert rendered.startswith(f"[{expected}](https://www.openstreetmap.org/?")
    url = rendered.split("](", maxsplit=1)[1].split(")", maxsplit=1)[0]
    query = parse_qs(urlparse(url).query)
    assert query == {"mlat": [str(latitude)], "mlon": [str(longitude)]}
    assert rendered.endswith("(12 km from Fixture Place, Fixture Country)")


def test_short_form_omits_distance_and_markdown_escapes_place_punctuation() -> None:
    answer = answer_with_settlement(
        settlement=Settlement(
            name="A_[B] * Place!",
            country="Côte d'Ivoire",
            distance_km=10,
            short_form=True,
        ),
    )
    assert markdown_centre(answer).endswith(r"(A\_\[B\] \* Place\!, Côte d'Ivoire)")
    assert plain_centre(answer) == "25.1250, 79.7080, A_[B] * Place!, Côte d'Ivoire"


def test_figure_centre_uses_four_decimal_coordinates_and_the_documented_place_decision() -> None:
    assert plain_centre(
        answer_with_settlement(
            settlement=Settlement(
                name="Fixture Place",
                country="Fixture Country",
                distance_km=500,
                short_form=False,
            ),
        ),
    ) == ("25.1250, 79.7080, 500 km from Fixture Place, Fixture Country")
    assert plain_centre(circle_of(document())) == "25.1250, 79.7080"


@pytest.mark.parametrize(
    ("latitude", "longitude", "settlement", "expected"),
    [
        (
            CENTRE_LAT,
            CENTRE_LON,
            Settlement(
                name="Fixture Place", country="Fixture Country", distance_km=10, short_form=True
            ),
            "25.1250, 79.7080, Fixture Place, Fixture Country",
        ),
        (
            CENTRE_LAT,
            CENTRE_LON,
            Settlement(
                name="Fixture Place", country="Fixture Country", distance_km=500, short_form=False
            ),
            "25.1250, 79.7080, 500 km from Fixture Place, Fixture Country",
        ),
        (CENTRE_LAT, CENTRE_LON, None, "25.1250, 79.7080"),
        (
            -12.345,
            -0.5,
            Settlement(
                name="Fixture Place",
                country="Fixture Country",
                distance_km=12.345,
                short_form=False,
            ),
            "-12.3450, -0.5000, 12 km from Fixture Place, Fixture Country",
        ),
    ],
)
def test_figure_title_uses_the_answer_centre_and_settlement_decision(
    latitude: float,
    longitude: float,
    settlement: Settlement | None,
    expected: str,
) -> None:
    answer = answer_with_settlement(latitude=latitude, longitude=longitude, settlement=settlement)
    title = title_of(answer)
    assert f"centred {expected}" in " ".join(title.split())


def test_long_figure_place_wraps_inside_the_centre_title() -> None:
    answer = answer_with_settlement(
        settlement=Settlement(
            name="An exceptionally long settlement name that should wrap rather than clip",
            country="A country with a long descriptive name",
            distance_km=125,
            short_form=False,
        ),
    )
    title = title_of(answer)
    assert any(line.startswith("         ") for line in title.splitlines())
