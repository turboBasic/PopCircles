import hashlib

import pytest

from population_circles.presentation.maps.presentation_config import (
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
    assert config.map_sources.terrain_overrides == {
        "AD": "copdem-glo-90-n42-e001",
        "CH": "swisstopo-dhm25-200m",
    }
    assert config.map_sources.lakes == "natural-earth-10m-lakes-v5.0.0"
    assert config.map_sources.rivers == "natural-earth-10m-rivers-lake-centerlines-v5.0.0"
    assert config.map_sources.roads == "natural-earth-10m-roads-v5.0.0"
    assert config.map_sources.places == "populated-places-1to10m"
    assert config.default_projection == "orthographic"
    us = config.country_maps["US"]
    chile = config.country_maps["CL"]
    assert not isinstance(us, str)
    assert not isinstance(chile, str)
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


def test_committed_map_title_override_is_keyed_by_output_stem() -> None:
    assert load().map_titles == {
        "world-most-populous-1000km-5arcmin": "Most populous 1000km circle"
    }


def test_bhutan_terrain_overlay_uses_all_intersecting_glo90_tiles() -> None:
    assert load().map_sources.terrain_overlays["BT"] == [
        "copdem-glo-90-n26-e088",
        "copdem-glo-90-n26-e089",
        "copdem-glo-90-n26-e090",
        "copdem-glo-90-n26-e091",
        "copdem-glo-90-n26-e092",
        "copdem-glo-90-n27-e088",
        "copdem-glo-90-n27-e089",
        "copdem-glo-90-n27-e090",
        "copdem-glo-90-n27-e091",
        "copdem-glo-90-n27-e092",
        "copdem-glo-90-n28-e089",
        "copdem-glo-90-n28-e090",
        "copdem-glo-90-n28-e091",
    ]


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
        (
            PRESENTATION_CONFIG.read_text(encoding="utf-8").replace(
                "world-most-populous-1000km-5arcmin", "Invalid_key"
            ),
            "map_titles keys must be lowercase map filename stems",
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
