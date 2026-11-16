import hashlib

import pytest

from population_circles.presentation_config import (
    PRESENTATION_CONFIG,
    load,
    parse,
)

EXPECTED_EXCLUDED_COUNTRY_COUNT = 48


def test_committed_presentation_config_preserves_existing_settings() -> None:
    config = load()
    exclusions = config.gallery.excluded_country_ids

    # Pin the complete migrated set without copying its IDs into a second configuration.
    assert len(exclusions) == EXPECTED_EXCLUDED_COUNTRY_COUNT
    assert hashlib.sha256("\n".join(sorted(exclusions)).encode()).hexdigest() == (
        "d11c07ce3d8d606d4c9d7ea07aab8cb472f249c09c2489f8244c852248b8de56"
    )
    assert config.map_text.render_title is True
    assert config.map_text.render_description is True
    assert config.default_projection == "orthographic"
    us = config.country_maps["US"]
    chile = config.country_maps["CL"]
    assert not isinstance(us, str)
    assert not isinstance(chile, str)
    assert us.projection == "orthographic"
    assert (us.centre_lat, us.centre_lon) == (44.0, -96.0)
    assert us.component_bounds is None
    assert chile.projection == "plate-carree"
    assert (chile.centre_lat, chile.centre_lon) == (None, None)
    assert chile.component_bounds is not None
    assert (
        chile.component_bounds.west,
        chile.component_bounds.south,
        chile.component_bounds.east,
        chile.component_bounds.north,
    ) == (-76.0, -56.53, -66.42, -17.5)


@pytest.mark.parametrize(
    ("text", "field"),
    [
        ("[map_text]\nrender_title = true\nrender_description = true\nunknown = 1\n", "unknown"),
        (
            (
                "[map_text]\nrender_title = true\nrender_description = true\n"
                '[gallery]\nexcluded_country_ids = ["BAD1"]\n'
            ),
            "excluded_country_ids",
        ),
        (
            (
                "[map_text]\nrender_title = 1\nrender_description = true\n"
                "[gallery]\nexcluded_country_ids = []\n"
            ),
            "render_title",
        ),
        (
            (
                "[map_text]\nrender_title = true\nrender_description = true\n"
                "[gallery]\nexcluded_country_ids = []\n"
                '[country_maps]\ndefault_projection = "mercator"\n'
            ),
            "default_projection",
        ),
        (
            (
                "[map_text]\nrender_title = true\nrender_description = true\n"
                "[gallery]\nexcluded_country_ids = []\n"
                '[country_maps]\ndefault_projection = "orthographic"\n'
                'US = "orthographic"\n'
            ),
            "country_maps.US",
        ),
        (
            PRESENTATION_CONFIG.read_text(encoding="utf-8").replace(
                "east = -66.42", "east = -77.0"
            ),
            "west must be less than east",
        ),
    ],
)
def test_invalid_config_names_its_source_and_field(text: str, field: str) -> None:
    with pytest.raises(ValueError, match=rf"presentation\.toml:.*{field}"):
        parse(text)


def test_country_map_key_must_be_a_canonical_id() -> None:
    text = PRESENTATION_CONFIG.read_text(encoding="utf-8").replace(
        "[country_maps.US]", "[country_maps.USA1]"
    )

    with pytest.raises(ValueError, match=r"country_maps\.USA1.*canonical country ID"):
        parse(text)


def test_models_are_frozen() -> None:
    assert load().model_config.get("frozen") is True
