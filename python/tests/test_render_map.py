import json
import re
import socket
from pathlib import Path
from typing import Any, NoReturn

import pytest
from matplotlib import pyplot as plt
from matplotlib.figure import Figure

from population_circles.circle_document import Answer, circle_of
from population_circles.circle_geometry import HALF_TURN_DEG, POLE_LAT
from population_circles.dataset_registry import load, parse
from population_circles.map_frame import ORTHOGRAPHIC, PLATE_CARREE
from population_circles.render_map import (
    COASTLINE,
    UnregisteredDatasetError,
    basemap,
    citation,
    main,
    render,
    title_of,
)

REGISTRY = load()
DATASET = "population-count-2020-30arcsec"
PROSE = Path(__file__).resolve().parents[2] / "data" / "README.md"

CENTRE_LAT = 25.125
CENTRE_LON = 79.708
RADIUS_KM = 1000.0

PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
NO_NETWORK = "a figure is drawn from the committed basemap and reaches no network"
# Handed to `render` in place of a citation, so the footer assertion is about what the caller passed
# rather than about a value the test could also have read.
MARKER = "attribution handed to render by its caller"


def document() -> dict[str, Any]:
    return {
        "schema_version": 1,
        "document": "most-populous",
        "tool": "popcircles",
        "tool_version": "0.1.0",
        "earth_model": {"model": "sphere", "radius_km": 6371.0088},
        "provenance": {"digest": "0xf17aa802a6890f0c", "decimation": 10, "dataset": DATASET},
        "result": {
            "centre": {"lat": CENTRE_LAT, "lon": CENTRE_LON},
            "row": 508,
            "col": 3116,
            "radius_km": RADIUS_KM,
            "population": 1254363867.93,
            "total_population": 7757982599.323671,
            "share_of_total": 0.1616868627727305,
        },
    }


def test_an_unknown_projection_is_refused_before_a_figure_is_created() -> None:
    before = plt.get_fignums()
    with pytest.raises(ValueError, match="unsupported projection 'unknown'"):
        render(circle_of(document()), "unknown", MARKER, coastline=None)
    assert plt.get_fignums() == before


def test_an_unknown_terminal_projection_exits_two() -> None:
    with pytest.raises(SystemExit) as caught:
        main(["--input", "unused.json", "--output", "unused.png", "--projection", "unknown"])
    assert caught.value.code == 2  # noqa: PLR2004 — argparse's usage-error exit code


def normalised(text: str) -> str:
    # Blockquote markers, emphasis and autolink brackets go, then whitespace collapses. The registry
    # writes the citation as Markdown and a figure draws it as prose, so comparing them at all means
    # discounting the markup that only one of the two carries.
    return " ".join(re.sub(r"[>*<]", " ", text).split())


def test_the_citation_is_the_text_the_prose_entry_quotes() -> None:
    # data/registry.toml owns the wording and data/README.md restates it for a human, so a drift
    # between the two fails here rather than shipping a credit that matches nothing in the tree.
    assert normalised(citation(REGISTRY, DATASET)) in normalised(
        PROSE.read_text(encoding="utf-8"),
    )


def test_the_credited_dataset_owes_a_citation_at_all() -> None:
    # An empty attribution is a legal value in the registry — the coastline has one — so the footer
    # assertion below would pass vacuously on "". CC BY is the whole reason a figure carries text.
    assert citation(REGISTRY, DATASET).strip()
    assert REGISTRY.datasets[DATASET].licence != "public domain"


# Two rows, each owing a different credit, which the committed registry cannot supply: it carries
# one attributed dataset, so a selection keyed by anything at all passes against it.
TWO_ATTRIBUTED = """
[datasets.first-raster]
kind = "population-raster"
path = "data/population/first-raster.tif"
bytes = 1
sha256 = "0"
source_url = "https://example.invalid/first"
licence = "CC BY 4.0"
licence_url = "https://creativecommons.org/licenses/by/4.0/"
attribution = "the credit the first dataset requires"
width = 4
height = 2
origin_lat = 90.0
origin_lon = -180.0
lat_step = -90.0
lon_step = 90.0
epsg = 4326
nodata = -1.0

[datasets.second-raster]
kind = "population-raster"
path = "data/population/second-raster.tif"
bytes = 1
sha256 = "0"
source_url = "https://example.invalid/second"
licence = "CC BY 4.0"
licence_url = "https://creativecommons.org/licenses/by/4.0/"
attribution = "the credit the second dataset requires"
width = 4
height = 2
origin_lat = 90.0
origin_lon = -180.0
lat_step = -90.0
lon_step = 90.0
epsg = 4326
nodata = -1.0
"""


def test_the_credit_is_the_one_the_named_dataset_owes_and_not_a_neighbours() -> None:
    registry = parse(TWO_ATTRIBUTED)
    assert citation(registry, "first-raster") == "the credit the first dataset requires"
    assert citation(registry, "second-raster") == "the credit the second dataset requires"


def test_a_document_naming_a_dataset_the_registry_lacks_is_refused() -> None:
    with pytest.raises(UnregisteredDatasetError) as caught:
        citation(REGISTRY, "gpw-v4")
    assert caught.value.key == "gpw-v4"
    assert "gpw-v4" in str(caught.value)


def test_the_basemap_is_the_committed_one() -> None:
    # data/README.md's entry is what a reader is sent to; this is the half a test can hold — the
    # file is committed, and parses into coastlines spanning the world.
    assert COASTLINE.is_file()
    coastline = basemap(COASTLINE)
    west, south, east, north = coastline.bounds
    assert (west, east) == pytest.approx((-HALF_TURN_DEG, HALF_TURN_DEG), abs=1e-6)
    assert -POLE_LAT < south < north < POLE_LAT


def test_the_footer_artist_carries_the_citation() -> None:
    figure = render(circle_of(document()), PLATE_CARREE, MARKER, coastline=None)
    drawn_text = " ".join(normalised(artist.get_text()) for artist in figure.texts)
    # The string handed in, not the one the registry holds: `render` draws what it is given, which
    # is the whole of what this test can prove about it.
    assert MARKER in drawn_text


def test_the_title_states_the_radius_the_share_the_centre_and_the_table() -> None:
    answer = circle_of(document())
    figure = render(answer, ORTHOGRAPHIC, MARKER, coastline=None)
    drawn_text = " ".join(normalised(artist.get_text()) for artist in figure.texts)
    assert "1,000 km circle" in drawn_text
    assert "16.17% of the population" in drawn_text
    assert "1,254,363,868 people" in drawn_text
    assert f"{CENTRE_LAT:.4f}, {CENTRE_LON:.4f}" in drawn_text
    # The decimation as well as the digest: the fixture's digest is the one both committed tables
    # carry, so a caption dropping the decimation would read the same for either of them.
    assert "0xf17aa802a6890f0c at decimation 10" in drawn_text


def test_measured_and_nested_documents_keep_their_existing_title_text() -> None:
    assert title_of(circle_of(document())) == (
        "1,000 km circle holding 16.17% of the population\n"
        "1,254,363,868 people, centred 25.1250, 79.7080\n"
        "answered from table 0xf17aa802a6890f0c at decimation 10"
    )

    nested = document()
    nested["document"] = "smallest"
    nested["result"] = {
        "ledger": {"path": "out/radii.json", "radii": 24},
        "circle": {
            "radius_km": 2639,
            "centre": {"lat": -85.0, "lon": 105.0},
            "population": 52635.0,
            "share_achieved": 0.25031387319522913,
            "covers_whole_grid": False,
        },
    }
    assert title_of(circle_of(nested)) == (
        "2,639 km circle holding 25.03% of the population\n"
        "52,635 people, centred -85.0000, 105.0000\n"
        "answered from table 0xf17aa802a6890f0c at decimation 10"
    )


@pytest.mark.parametrize("projection", [PLATE_CARREE, ORTHOGRAPHIC])
def test_a_complete_figure_is_written_where_it_is_told(projection: str, tmp_path: Path) -> None:
    # The figure a test writes goes to tmp_path: no rendered map enters the repository, which is the
    # first project invariant and the reason there is no baseline image to compare against.
    source = tmp_path / "circle.json"
    source.write_text(json.dumps(document()), encoding="utf-8")
    output = tmp_path / "figures" / "map.png"

    argv = ["--input", str(source), "--output", str(output), "--projection", projection]
    assert main(argv) == 0
    assert output.read_bytes().startswith(PNG_MAGIC)


@pytest.mark.parametrize("projection", [PLATE_CARREE, ORTHOGRAPHIC])
def test_a_complete_figure_with_a_long_settlement_needs_no_network(
    projection: str,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # The claim the committed basemap exists to make, proved by denial rather than by reading the
    # imports: sockets are taken away for the duration, so a figure that still needs a download
    # fails here instead of passing on whichever machine happened to have one.
    def refuse(*_args: object, **_kwargs: object) -> NoReturn:
        raise AssertionError(NO_NETWORK)

    for name in ("socket", "create_connection", "socketpair"):
        monkeypatch.setattr(socket, name, refuse)

    source = tmp_path / "circle.json"
    payload = document()
    payload["result"]["nearest_settlement"] = {
        "name": "An exceptionally long settlement name that should wrap rather than clip",
        "country": "A country with a long descriptive name",
        "distance_km": 125,
        "short_form": False,
    }
    source.write_text(json.dumps(payload), encoding="utf-8")
    output = tmp_path / f"map-{projection}.png"
    assert not (tmp_path / "data/settlements/populated-places-1to10m.geojson").exists()
    monkeypatch.chdir(tmp_path)

    assert main(["--input", str(source), "--output", str(output), "--projection", projection]) == 0
    assert output.read_bytes().startswith(PNG_MAGIC)


def test_main_credits_the_dataset_its_document_names_and_not_a_constant(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # The one wire nothing else covers: `main` passing the document's own key to `citation`. Every
    # other case here hands `citation` a literal, so a constant put back in `main` would pass them
    # all. The fixture registry carries neither committed row, so a `main` reaching for one of those
    # cannot resolve it at all.
    monkeypatch.setattr("population_circles.render_map.load", lambda: parse(TWO_ATTRIBUTED))
    drawn: list[str] = []

    def capture(_answer: Answer, _projection: str, attribution: str, **_options: bool) -> Figure:
        drawn.append(attribution)
        return Figure()

    monkeypatch.setattr("population_circles.render_map.render", capture)

    named = document()
    named["provenance"]["dataset"] = "second-raster"
    source = tmp_path / "circle.json"
    source.write_text(json.dumps(named), encoding="utf-8")

    assert main(["--input", str(source), "--output", str(tmp_path / "map.png")]) == 0
    assert drawn == ["the credit the second dataset requires"]
