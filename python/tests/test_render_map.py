import hashlib
import json
import re
import socket
from pathlib import Path
from typing import Any, NoReturn

import pytest
from matplotlib import pyplot as plt
from matplotlib.backends.backend_agg import FigureCanvasAgg
from matplotlib.figure import Figure

from population_circles.circle_document import Answer, circle_of
from population_circles.circle_geometry import HALF_TURN_DEG, POLE_LAT, cap
from population_circles.country_outline import BoundaryIdentityError, load_outline
from population_circles.dataset_registry import Registry, load, parse
from population_circles.map_frame import ORTHOGRAPHIC, PLATE_CARREE, country_frame
from population_circles.render_map import (
    COASTLINE,
    MissingCountryOutlineError,
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
MAX_LOCAL_EXTENT_METRES = 5_000_000
NO_NETWORK = "a figure is drawn from the committed basemap and reaches no network"
# Handed to `render` in place of a citation, so the footer assertion is about what the caller passed
# rather than about a value the test could also have read.
MARKER = "attribution handed to render by its caller"
BOUNDARY_CREDIT = (
    "© EuroGeographics for the administrative boundaries; acknowledge EuroGeographics and UN-FAO."
)


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


def country_document(count: int = 1) -> dict[str, Any]:
    result = document()
    result["result"]["country_breakdown"] = {
        "entries": [
            {
                "id": f"C{index:02}",
                "name": "$ literal \\ alpha " + "a very long country name " * 3 + str(index),
                "population_inside": 10,
                "country_total_population": 100,
                "fraction_of_country_population": 0.1,
                "fraction_of_circle_population": 0.01,
            }
            for index in range(count)
        ],
        "reconciliation": {"unassigned_population": 20, "bound_persons": 0.01},
        "provenance": {
            "boundary_dataset": "gisco-countries-2024-1m",
            "boundary_revision": "Countries 2024 snapshot",
            "boundary_digest": "a" * 64,
            "attribution": "© EuroGeographics for the administrative boundaries; "
            "acknowledge EuroGeographics and UN-FAO.",
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
    return result


def country_circle_document(digest: str = "a" * 64) -> dict[str, Any]:
    result = document()
    result["document"] = "country-smallest"
    result["result"] = {
        "country": {
            "id": "DE",
            "name": "Germany",
            "boundary_dataset": "gisco-countries-2024-1m",
            "boundary_revision": "Countries 2024 snapshot",
            "boundary_digest": digest,
        },
        "population_dataset": DATASET,
        "decimation": 10,
        "ledger": {"path": "out/country.json", "radii": 4},
        "circle": {
            "centre": {"lat": 51.0, "lon": 10.0},
            "radius_km": 200.0,
            "population": 40.0,
            "share_achieved": 0.5,
        },
    }
    result.pop("provenance")
    return result


def country_boundary_registry(
    revision: str = "Countries 2024 snapshot",
    digest: str = "a" * 64,
    size: int = 1,
) -> Registry:
    return parse(
        f"""[datasets.gisco-countries-2024-1m]
kind = "boundary-vector"
path = "data/boundaries/gisco-countries-2024-1m.geojson"
bytes = {size}
sha256 = "{digest}"
source_url = "https://example.invalid/source"
licence = "fixture"
licence_url = "https://example.invalid/licence"
attribution = "{BOUNDARY_CREDIT}"
format = "GeoJSON"
crs = "EPSG:4326"
axis_order = "longitude-latitude"
feature_count = 2
revision = "{revision}"
source_id_field = "CNTR_ID"
identity_policy = "ADR 0015"
""",
    )


def boundary_feature(identifier: str, name: str, west: float) -> dict[str, Any]:
    return {
        "type": "Feature",
        "properties": {"CNTR_ID": identifier, "NAME_ENGL": name},
        "geometry": {
            "type": "Polygon",
            "coordinates": [[[west, 0], [west + 1, 0], [west + 1, 1], [west, 1], [west, 0]]],
        },
    }


def country_fixture(
    tmp_path: Path,
    features: list[dict[str, Any]],
    centre: tuple[float, float] = (51.0, 10.0),
) -> tuple[Answer, Registry, Path]:
    content = json.dumps({"type": "FeatureCollection", "features": features})
    registry_digest = hashlib.sha256(content.encode()).hexdigest()
    registry = country_boundary_registry(digest=registry_digest, size=len(content.encode()))
    path = registry.datasets["gisco-countries-2024-1m"].file(tmp_path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)
    document = country_circle_document(registry_digest)
    document["result"]["circle"]["centre"] = {"lat": centre[0], "lon": centre[1]}
    return circle_of(document), registry, path


def polygon_feature(identifier: str, coordinates: list[list[list[float]]]) -> dict[str, Any]:
    return {
        "type": "Feature",
        "properties": {"CNTR_ID": identifier, "NAME_ENGL": identifier},
        "geometry": {"type": "Polygon", "coordinates": coordinates},
    }


def test_an_unknown_projection_is_refused_before_a_figure_is_created() -> None:
    before = plt.get_fignums()
    with pytest.raises(ValueError, match="unsupported projection 'unknown'"):
        render(circle_of(document()), "unknown", MARKER, coastline=None)
    assert plt.get_fignums() == before


def test_country_outline_uses_the_registered_id_when_names_are_duplicated(
    tmp_path: Path,
) -> None:
    features = [
        boundary_feature("DE", "Same name", 0),
        boundary_feature("FR", "Same name", 10),
    ]
    answer, registry, _ = country_fixture(tmp_path, features)
    assert load_outline(answer, registry, tmp_path).bounds == (0, 0, 1, 1)

    for bad_features, message in (
        ([boundary_feature("FR", "Same name", 10)], "no feature with CNTR_ID='DE'"),
        (
            [boundary_feature("DE", "Germany", 0), boundary_feature("DE", "Germany", 10)],
            "duplicate CNTR_ID 'DE'",
        ),
    ):
        answer, registry, _ = country_fixture(tmp_path, bad_features)
        with pytest.raises(ValueError, match=message):
            load_outline(answer, registry, tmp_path)


def test_country_outline_rejects_bytes_that_do_not_match_the_registered_digest(
    tmp_path: Path,
) -> None:
    features = [boundary_feature("DE", "Germany", 0)]
    answer, registry, path = country_fixture(tmp_path, features)
    original = path.read_text()
    path.write_text(original.replace("[0, 0]", "[2, 0]", 1))

    with pytest.raises(BoundaryIdentityError, match="does not match the registry"):
        load_outline(answer, registry, tmp_path)


def test_a_country_result_is_not_rendered_without_its_outline() -> None:
    before = plt.get_fignums()
    with pytest.raises(
        MissingCountryOutlineError, match="requires its registered boundary outline"
    ):
        render(circle_of(country_circle_document()), PLATE_CARREE, MARKER, coastline=None)
    assert plt.get_fignums() == before


@pytest.mark.parametrize(
    ("name", "centre", "coordinates"),
    [
        (
            "antimeridian",
            (10.0, 179.0),
            [[[170, 8], [-170, 8], [-170, 12], [170, 12], [170, 8]]],
        ),
        (
            "polar",
            (82.0, 20.0),
            [[[10, 78], [30, 78], [30, 86], [10, 86], [10, 78]]],
        ),
    ],
    ids=["antimeridian", "polar"],
)
def test_country_figure_draws_the_outline_and_circle_without_clipping(
    name: str,
    centre: tuple[float, float],
    coordinates: list[list[list[float]]],
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def refuse(*_args: object, **_kwargs: object) -> NoReturn:
        raise AssertionError(NO_NETWORK)

    for socket_name in ("socket", "create_connection", "socketpair"):
        monkeypatch.setattr(socket, socket_name, refuse)

    answer, registry, _ = country_fixture(
        tmp_path,
        [polygon_feature("DE", coordinates)],
        centre,
    )
    assert answer.country is not None
    country = load_outline(answer, registry, tmp_path)
    circle = answer.circle
    view = country_frame(
        cap(circle.centre, circle.radius_km, circle.earth_radius_km),
        country,
    )
    figure = render(
        answer,
        ORTHOGRAPHIC,
        MARKER,
        coastline=None,
        outline_and_credit=(country, citation(registry, answer.country.boundary_dataset)),
    )
    try:
        output = tmp_path / f"country-{name}.png"
        figure.savefig(output, dpi=120)  # pyright: ignore[reportUnknownMemberType] — matplotlib kwargs
        axes = figure.axes[0]
        west, south, east, north = view.horizon.bounds
        assert output.read_bytes().startswith(PNG_MAGIC)
        assert axes.get_xlim() == pytest.approx((west, east))
        assert axes.get_ylim() == pytest.approx((south, north))
        assert view.country_outline is not None
        assert east - west < MAX_LOCAL_EXTENT_METRES
        assert north - south < MAX_LOCAL_EXTENT_METRES
        assert view.horizon.covers(view.country_outline)
        assert view.horizon.covers(view.circle)
    finally:
        plt.close(figure)


def test_country_figure_shows_both_registry_credits_and_grows_its_footer(
    tmp_path: Path,
) -> None:
    answer, registry, _ = country_fixture(
        tmp_path,
        [boundary_feature("DE", "Germany", 9)],
    )
    assert answer.country is not None
    answer = answer.model_copy(
        update={
            "country": answer.country.model_copy(
                update={"name": "A country with an exceptionally long published name"},
            ),
        },
    )
    assert answer.country is not None
    boundary = load_outline(answer, registry, tmp_path)
    population_credit = citation(REGISTRY, DATASET)
    boundary_credit = citation(REGISTRY, answer.country.boundary_dataset)
    figure = render(
        answer,
        ORTHOGRAPHIC,
        population_credit,
        coastline=None,
        outline_and_credit=(boundary, boundary_credit),
    )
    try:
        canvas = FigureCanvasAgg(figure)
        canvas.draw()
        footer = figure.texts[-1].get_window_extent(canvas.get_renderer())
        axes = figure.axes[0].get_window_extent()
        rendered_text = "\n".join(artist.get_text() for artist in figure.texts)
        assert "A country with an exceptionally long published name (DE)" in rendered_text
        assert normalised(boundary_credit) in normalised(rendered_text)
        assert normalised(population_credit) in normalised(rendered_text)
        assert citation(REGISTRY, "coastline-1to110m") == ""
        assert "Basemap credit:" not in rendered_text
        assert footer.x0 >= 0
        assert footer.x1 <= figure.bbox.x1
        assert footer.y0 >= 0
        assert footer.y1 <= axes.y0
    finally:
        plt.close(figure)


def test_country_outline_rejects_a_registry_source_mismatch(tmp_path: Path) -> None:
    answer = circle_of(country_circle_document())
    for registry in (
        country_boundary_registry(revision="a different snapshot"),
        country_boundary_registry(digest="b" * 64),
    ):
        with pytest.raises(BoundaryIdentityError, match="does not match the registry"):
            load_outline(answer, registry, tmp_path)


def test_country_outline_names_the_fetch_task_when_its_file_is_missing(tmp_path: Path) -> None:
    answer = circle_of(country_circle_document())
    with pytest.raises(FileNotFoundError, match="mise run data:get"):
        load_outline(answer, country_boundary_registry(), tmp_path)


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


@pytest.mark.parametrize("projection", [PLATE_CARREE, ORTHOGRAPHIC])
def test_long_country_captions_grow_the_figure_and_keep_every_line_inside(
    projection: str,
    tmp_path: Path,
) -> None:
    answer = circle_of(country_document(18))
    figure = render(answer, projection, MARKER, coastline=None)
    try:
        output = tmp_path / f"countries-{projection}.png"
        figure.savefig(output, dpi=120)  # pyright: ignore[reportUnknownMemberType] — matplotlib kwargs
        canvas = FigureCanvasAgg(figure)
        canvas.draw()
        footer = figure.texts[-1].get_window_extent(canvas.get_renderer())
        axes = figure.axes[0].get_window_extent()
        rendered_text = "\n".join(artist.get_text() for artist in figure.texts)
        assert figure.get_size_inches()[1] > 6.0  # noqa: PLR2004 — initial figure height
        assert footer.x0 >= 0
        assert footer.x1 <= figure.bbox.x1
        assert footer.y0 >= 0
        assert footer.y1 <= axes.y0
        assert "$ literal \\ alpha" in rendered_text
        assert "C17" in rendered_text
        assert "Boundary credit: © EuroGeographics" in rendered_text
        assert output.read_bytes().startswith(PNG_MAGIC)
    finally:
        plt.close(figure)


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
    payload = country_document()
    payload["result"]["nearest_settlement"] = {
        "name": "An exceptionally long settlement name that should wrap rather than clip",
        "country": "A country with a long descriptive name",
        "distance_km": 125,
        "short_form": False,
    }
    source.write_text(json.dumps(payload), encoding="utf-8")
    output = tmp_path / f"map-{projection}.png"
    assert not (tmp_path / "data/boundaries/gisco-countries-2024-1m.geojson").exists()
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
