import hashlib

import pytest

from population_circles.presentation_config import (
    PRESENTATION_CONFIG,
    load,
    parse,
)

EXPECTED_EXCLUDED_COUNTRY_COUNT = 48
MAP_SOURCES = (
    '\n[map_sources]\nterrain_default = "global"\nlakes = "lakes"\n'
    'rivers = "rivers"\nroads = "roads"\nplaces = "places"\n'
)


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
    assert config.map_sources.terrain_default == "etopo-2022-60s"
    assert config.map_sources.terrain_overrides == {"CH": "swisstopo-dhm25-200m"}
    assert config.map_sources.lakes == "natural-earth-10m-lakes-v5.0.0"
    assert config.map_sources.rivers == "natural-earth-10m-rivers-lake-centerlines-v5.0.0"
    assert config.map_sources.roads == "natural-earth-10m-roads-v5.0.0"
    assert config.map_sources.places == "populated-places-1to10m"
    assert config.default_projection == "orthographic"
    us = config.country_maps["US"]
    chile = config.country_maps["CL"]
    france = config.country_maps["FR"]
    assert not isinstance(us, str)
    assert not isinstance(chile, str)
    assert not isinstance(france, str)
    assert us.projection == "orthographic"
    assert us.projection_parameters == {"lon_0": -96.0, "lat_0": 44.0}
    assert us.extent is not None
    assert us.extent.units == "metres"
    assert (
        us.extent.x_min,
        us.extent.y_min,
        us.extent.x_max,
        us.extent.y_max,
    ) == (-5397723.3, -2111802.2, 2419813.8, 3835144.3)
    assert chile.projection == "plate-carree"
    assert chile.projection_parameters == {}
    assert chile.extent is not None
    assert chile.extent.units == "degrees"
    assert (
        chile.extent.x_min,
        chile.extent.y_min,
        chile.extent.x_max,
        chile.extent.y_max,
    ) == (-75.724713888, -56.528847748, -66.418757789, -17.498400217)
    assert france.projection == "plate-carree"
    assert france.extent is not None
    assert france.extent.units == "degrees"
    assert (
        france.extent.x_min,
        france.extent.y_min,
        france.extent.x_max,
        france.extent.y_max,
    ) == (-6.0, 41.0, 10.0, 52.0)


@pytest.mark.parametrize(
    ("text", "field"),
    [
        (
            "[map_text]\nrender_title = true\nrender_description = true\nunknown = 1\n"
            + MAP_SOURCES,
            "unknown",
        ),
        (
            (
                "[map_text]\nrender_title = true\nrender_description = true\n"
                '[gallery]\nexcluded_country_ids = ["BAD1"]\n' + MAP_SOURCES
            ),
            "excluded_country_ids",
        ),
        (
            (
                "[map_text]\nrender_title = 1\nrender_description = true\n"
                "[gallery]\nexcluded_country_ids = []\n" + MAP_SOURCES
            ),
            "render_title",
        ),
        (
            (
                "[map_text]\nrender_title = true\nrender_description = true\n"
                "[gallery]\nexcluded_country_ids = []\n"
                '[country_maps]\ndefault_projection = "mercator"\n' + MAP_SOURCES
            ),
            "default_projection",
        ),
        (
            (
                "[map_text]\nrender_title = true\nrender_description = true\n"
                "[gallery]\nexcluded_country_ids = []\n"
                '[country_maps]\ndefault_projection = "orthographic"\n'
                'US = "orthographic"\n' + MAP_SOURCES
            ),
            "country_maps.US",
        ),
        (
            PRESENTATION_CONFIG.read_text(encoding="utf-8").replace(
                "x_max = -66.418757789", "x_max = -80.0"
            ),
            "extent minimums must be less than maximums",
        ),
        (
            PRESENTATION_CONFIG.read_text(encoding="utf-8").replace("lat_0 = 44.0", "lat_1 = 44.0"),
            "projection_parameters",
        ),
    ],
)
def test_invalid_config_names_its_source_and_field(text: str, field: str) -> None:
    with pytest.raises(ValueError, match=rf"presentation\.toml:.*{field}"):
        parse(text)


def test_country_map_key_must_be_a_canonical_id() -> None:
    text = PRESENTATION_CONFIG.read_text(encoding="utf-8").replace(
        "[country_maps.US", "[country_maps.USA1"
    )

    with pytest.raises(ValueError, match=r"country_maps\.USA1.*canonical country ID"):
        parse(text)


@pytest.mark.parametrize(
    ("projection", "parameters"),
    [
        (
            "albers-equal-area",
            "lon_0 = -125.0\nlat_0 = 45.0\nlat_1 = 35.0\nlat_2 = 60.0",
        ),
        (
            "transverse-mercator",
            "lon_0 = -71.0\nlat_0 = -38.0\nk_0 = 1.0",
        ),
    ],
)
def test_projection_parameters_are_specific_to_the_projection(
    projection: str, parameters: str
) -> None:
    config = PRESENTATION_CONFIG.read_text(encoding="utf-8")
    config = config.replace(
        '[country_maps.CL]\nprojection = "plate-carree"',
        f'[country_maps.CL]\nprojection = "{projection}"\n'
        "[country_maps.CL.projection_parameters]\n"
        f"{parameters}",
    )
    config = config.replace(
        '[country_maps.CL.extent]\nunits = "degrees"',
        '[country_maps.CL.extent]\nunits = "metres"',
    )

    chile = parse(config).country_maps["CL"]

    assert not isinstance(chile, str)
    assert chile.projection == projection


def test_models_are_frozen() -> None:
    assert load().model_config.get("frozen") is True
