import hashlib
import json
import re
import socket
from collections.abc import Iterable, Mapping, Sequence
from pathlib import Path
from typing import Any, NoReturn, cast

import numpy as np
import pytest
from matplotlib import pyplot as plt
from matplotlib.backends.backend_agg import FigureCanvasAgg
from matplotlib.colors import to_rgb
from matplotlib.figure import Figure
from matplotlib.text import Text
from PIL import Image
from shapely.geometry import LineString, MultiPolygon, Point, box
from shapely.geometry.base import BaseGeometry

import population_circles.presentation_config as presentation_config_module
import population_circles.render_map as render_map_module
import population_circles.territory_coverage as territory_coverage_module
from population_circles.circle_document import Answer, Circle, circle_of
from population_circles.circle_geometry import Cap, cap
from population_circles.country_outline import BoundaryIdentityError, load_outline
from population_circles.dataset_registry import (
    REPO_ROOT,
    MapContextVector,
    Registry,
    SettlementVector,
    TerrainRaster,
    load,
    parse,
)
from population_circles.map_frame import (
    ORTHOGRAPHIC,
    PLATE_CARREE,
    CountryFrameSettings,
    Frame,
)
from population_circles.map_frame import (
    country_frame as build_country_frame,
)
from population_circles.map_frame import (
    frame as world_frame,
)
from population_circles.presentation_config import CountryMap
from population_circles.presentation_config import load as load_presentation_config
from population_circles.render_map import (
    COASTLINE_DATASET,
    COUNTRY,
    COUNTRY_LAYER,
    OUTLINE_WIDTH,
    MapLayers,
    MapSources,
    MissingCountryOutlineError,
    UnregisteredDatasetError,
    citation,
    load_map_layers,
    main,
    place_city_labels,
    registered_coastline,
    registered_map_sources,
    render,
    render_file,
    title_of,
)

REGISTRY = load()
DATASET = "population-count-2020-30arcsec"
PROSE = Path(__file__).resolve().parents[2] / "data" / "README.md"

CENTRE_LAT = 25.125
CENTRE_LON = 79.708
RADIUS_KM = 1000.0
WORLD_POPULATION = 1_254_363_867.93
TERRITORY_SHARE = 0.375
EXPECTED_OFFSCREEN_CHILE_COMPONENTS = 10
CountryPresentationCase = tuple[
    str,
    tuple[float, float],
    BaseGeometry,
    str,
    tuple[float, float] | None,
]

PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
MAX_LOCAL_EXTENT_METRES = 5_000_000
NO_NETWORK = "a figure is drawn from the committed basemap and reaches no network"
COASTLINE_CREDIT = "© EuroGeographics for the administrative boundaries"
FOOTER_SIZE = 6.5
FOOTER_BOTTOM = 0.04
VISIBLE_FOOTER = (
    "© Andrii Melnyk, 2026, https://github.com/turboBasic/PopCircles · "
    "© EuroGeographics for the administrative boundaries"
)
# Handed to `render` in place of a citation, so the footer assertion is about what the caller passed
# rather than about a value the test could also have read.
MARKER = "attribution handed to render by its caller"
BOUNDARY_CREDIT = (
    "© EuroGeographics for the administrative boundaries; acknowledge EuroGeographics and UN-FAO."
)


def empty_boundaries(
    _answer: Answer, _registry: Registry, _root: Path = Path()
) -> dict[str, BaseGeometry]:
    return {}


def fixture_map_sources(country_id: str | None) -> MapSources:
    terrain_key = "swisstopo-dhm25-200m" if country_id == "CH" else "etopo-2022-60s"
    terrain = REGISTRY.datasets[terrain_key]
    lakes = REGISTRY.datasets["natural-earth-10m-lakes-v5.0.0"]
    rivers = REGISTRY.datasets["natural-earth-10m-rivers-lake-centerlines-v5.0.0"]
    roads = REGISTRY.datasets["natural-earth-10m-roads-v5.0.0"]
    places = REGISTRY.datasets["populated-places-1to10m"]
    assert isinstance(terrain, TerrainRaster)
    assert isinstance(lakes, MapContextVector)
    assert isinstance(rivers, MapContextVector)
    assert isinstance(roads, MapContextVector)
    assert isinstance(places, SettlementVector)
    return MapSources(terrain, lakes, rivers, roads, places)


def fixture_registered_map_sources(_registry: Registry, country_id: str | None) -> MapSources:
    return fixture_map_sources(country_id)


def fixture_map_layers(
    sources: MapSources,
    root: Path = REPO_ROOT,
    country_outline: BaseGeometry | None = None,
) -> MapLayers:
    _ = country_outline
    return MapLayers(
        sources,
        lakes=(box(79, 24, 80, 25),),
        rivers=(LineString([(79, 24), (80, 25)]),),
        roads=(LineString([(79, 25), (80, 24)]),),
        places=(),
        root=root,
    )


def transparent_terrain(
    _dataset: TerrainRaster | Sequence[TerrainRaster],
    view: Frame,
    *,
    root: Path = REPO_ROOT,
    include_below_sea_level: bool = False,
) -> tuple[np.ndarray, tuple[float, float, float, float]]:
    assert root == REPO_ROOT
    _ = include_below_sea_level
    return np.zeros((2, 2, 4), dtype=np.uint8), view.horizon.bounds


def png_info(path: Path) -> Mapping[str | tuple[int, int], Any]:
    with Image.open(path) as image:
        return image.info


@pytest.fixture(autouse=True)
def use_empty_boundary_fixtures(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(render_map_module, "load_map_boundaries", empty_boundaries)
    monkeypatch.setattr(
        render_map_module,
        "registered_map_sources",
        fixture_registered_map_sources,
    )
    monkeypatch.setattr(render_map_module, "load_map_layers", fixture_map_layers)
    monkeypatch.setattr(render_map_module, "prepare_terrain", transparent_terrain)


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


def territory_document() -> dict[str, Any]:
    payload = document()
    payload["result"]["territory_share"] = TERRITORY_SHARE
    return payload


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
display_name_field = "CNTR_NAME"
identity_policy = "ADR 0015"
""",
    )


def boundary_feature(identifier: str, name: str, west: float) -> dict[str, Any]:
    return {
        "type": "Feature",
        "properties": {
            "CNTR_ID": identifier,
            "CNTR_NAME": name,
            "NAME_ENGL": name,
        },
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


def write_coastline_fixture(root: Path) -> Path:
    path = root / REGISTRY.datasets[COASTLINE_DATASET].path
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(
            {
                "type": "FeatureCollection",
                "features": [
                    {
                        "type": "Feature",
                        "properties": {},
                        "geometry": {"type": "LineString", "coordinates": [[-20, 0], [20, 0]]},
                    },
                ],
            },
        ),
        encoding="utf-8",
    )
    return path


def use_coastline_fixture(root: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    monkeypatch.setattr(render_map_module, "REPO_ROOT", root)
    monkeypatch.setattr(render_map_module, "load", lambda: REGISTRY)
    return write_coastline_fixture(root)


def polygon_feature(identifier: str, coordinates: list[list[list[float]]]) -> dict[str, Any]:
    return {
        "type": "Feature",
        "properties": {
            "CNTR_ID": identifier,
            "CNTR_NAME": identifier,
            "NAME_ENGL": identifier,
        },
        "geometry": {"type": "Polygon", "coordinates": coordinates},
    }


def test_an_unknown_projection_is_refused_before_a_figure_is_created() -> None:
    before = plt.get_fignums()
    with pytest.raises(ValueError, match="unsupported projection 'unknown'"):
        render(circle_of(document()), "unknown", coastline=None)
    assert plt.get_fignums() == before


def test_country_outline_uses_the_registered_id_when_names_are_duplicated(
    tmp_path: Path,
) -> None:
    features = [
        boundary_feature("DE", "Same name", 0),
        boundary_feature("FR", "Same name", 10),
    ]
    answer, registry, _ = country_fixture(tmp_path, features)
    outline = load_outline(answer, registry, tmp_path)
    assert outline is not None
    assert outline.bounds == (0, 0, 1, 1)

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
        render(circle_of(country_circle_document()), PLATE_CARREE, coastline=None)
    assert plt.get_fignums() == before


@pytest.mark.parametrize(
    "case",
    [
        ("US", (44.0, -96.0), box(-110, 30, -80, 50), ORTHOGRAPHIC, (44.0, -96.0)),
        ("CL", (-40.0, -72.0), box(-75, -45, -70, -35), PLATE_CARREE, None),
        ("DE", (51.0, 10.0), box(8, 49, 12, 53), ORTHOGRAPHIC, None),
    ],
    ids=["configured-US", "configured-CL", "default-country"],
)
def test_country_render_uses_presentation_settings_by_canonical_id(
    case: CountryPresentationCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    country_id, centre, outline, expected_projection, expected_centre = case
    payload = country_circle_document()
    payload["result"]["country"]["id"] = country_id
    payload["result"]["circle"]["centre"] = {"lat": centre[0], "lon": centre[1]}
    answer = circle_of(payload)
    current_country_frame = build_country_frame
    observed: list[tuple[CountryFrameSettings, BaseGeometry]] = []

    def capture_country_frame(
        built: Cap,
        selected: BaseGeometry,
        settings: CountryFrameSettings | None = None,
    ) -> Frame:
        frame_settings = settings or CountryFrameSettings()
        observed.append((frame_settings, selected))
        return current_country_frame(built, selected, frame_settings)

    monkeypatch.setattr(render_map_module, "country_frame", capture_country_frame)
    figure = render(answer, PLATE_CARREE, coastline=None, country_outline=outline)
    try:
        frame_settings, selected_outline = observed[0]
        assert frame_settings.projection == expected_projection
        observed_centre = frame_settings.projection_parameters
        assert (
            (observed_centre.get("lat_0"), observed_centre.get("lon_0"))
            if observed_centre is not None and "lat_0" in observed_centre
            else None
        ) == expected_centre
        assert selected_outline.equals(outline)
        if country_id == "US":
            assert frame_settings.projection_parameters == {"lon_0": -96.0, "lat_0": 44.0}
            assert frame_settings.extent == (
                -5397723.3,
                -2111802.2,
                2419813.8,
                3835144.3,
            )
    finally:
        plt.close(figure)


def test_country_render_rejects_an_invalid_presentation_config(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    config_path = tmp_path / "presentation.toml"
    settings = presentation_config_module.PRESENTATION_CONFIG.read_text(encoding="utf-8")
    config_path.write_text(
        settings.replace(
            '[country_maps.US]\nprojection = "orthographic"',
            '[country_maps.US]\nprojection = "mercator"',
        ),
        encoding="utf-8",
    )
    monkeypatch.setattr(presentation_config_module, "PRESENTATION_CONFIG", config_path)
    payload = country_circle_document()
    payload["result"]["country"]["id"] = "US"
    payload["result"]["circle"]["centre"] = {"lat": 44.0, "lon": -96.0}

    with pytest.raises(ValueError, match=r"presentation\.toml:.*country_maps\.US"):
        render(
            circle_of(payload),
            PLATE_CARREE,
            coastline=None,
            country_outline=box(-110, 30, -80, 50),
        )


def test_chile_extent_hides_remote_components_without_filtering_outline() -> None:
    result = (
        Path(__file__).resolve().parents[2]
        / "results"
        / ("country-gpw-2020-gisco-2024-2-CL-half.json")
    )
    document = json.loads(result.read_text(encoding="utf-8"))
    original_document = json.dumps(document, sort_keys=True)
    answer = circle_of(document)
    outline = MultiPolygon(
        [box(-175 + 2 * index, -30, -174.5 + 2 * index, -29.5) for index in range(10)]
        + [box(-75.7, -56.5, -66.5, -17.6)],
    )
    original = outline.wkb
    original_circle = answer.circle.model_dump_json()
    settings = load_presentation_config().country_maps["CL"]
    assert isinstance(settings, CountryMap)
    assert isinstance(outline, MultiPolygon)
    assert settings.extent is not None
    assert settings.extent.units == "degrees"
    rendered_outline, frame_settings = render_map_module.country_map_display("CL", outline)
    assert rendered_outline is outline
    assert frame_settings.extent is not None
    assert sum(component.bounds[2] < frame_settings.extent[0] for component in outline.geoms) == (
        EXPECTED_OFFSCREEN_CHILE_COMPONENTS
    )
    view = build_country_frame(
        cap(answer.circle.centre, answer.circle.radius_km, answer.circle.earth_radius_km),
        rendered_outline,
        frame_settings,
    )
    assert view.horizon.bounds == pytest.approx(
        (-75.724713888, -56.528847748, -66.418757789, -17.498400217)
    )
    assert outline.wkb == original

    figure = render(answer, ORTHOGRAPHIC, coastline=None, country_outline=outline)
    plt.close(figure)
    assert json.dumps(document, sort_keys=True) == original_document
    assert answer.circle.model_dump_json() == original_circle
    assert outline.wkb == original


@pytest.mark.parametrize("projection", [PLATE_CARREE, ORTHOGRAPHIC])
def test_world_render_uses_its_frame_without_country_presentation(
    projection: str, monkeypatch: pytest.MonkeyPatch
) -> None:
    current_frame = world_frame
    observed: list[str] = []

    def capture_frame(selected_projection: str, built: Cap) -> Frame:
        observed.append(selected_projection)
        return current_frame(selected_projection, built)

    def refuse_country_display(*_args: object) -> None:
        message = "world rendering consulted country presentation settings"
        raise AssertionError(message)

    monkeypatch.setattr(render_map_module, "frame", capture_frame)
    monkeypatch.setattr(render_map_module, "country_map_display", refuse_country_display)
    monkeypatch.setattr(render_map_module, "country_frame", refuse_country_display)
    figure = render(circle_of(document()), projection, coastline=None)
    try:
        assert observed == [projection]
    finally:
        plt.close(figure)


@pytest.mark.parametrize(("radius_km", "expected_patches"), [(0.0, 2), (200.0, 3)])
def test_country_figure_renders_zero_radius_as_only_its_centre_marker(
    radius_km: float,
    expected_patches: int,
    tmp_path: Path,
) -> None:
    answer, registry, _ = country_fixture(
        tmp_path,
        [boundary_feature("DE", "Germany", 9)],
    )
    answer = answer.model_copy(
        update={"circle": answer.circle.model_copy(update={"radius_km": radius_km})},
    )
    assert answer.country is not None
    country = load_outline(answer, registry, tmp_path)
    assert country is not None
    figure = render(
        answer,
        ORTHOGRAPHIC,
        coastline=None,
        country_outline=country,
    )
    try:
        axes = figure.axes[0]
        assert len(axes.patches) == expected_patches
        assert len(axes.lines) == 1
        assert axes.lines[0].get_marker() == "+"
    finally:
        plt.close(figure)


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
    assert country is not None
    circle = answer.circle
    view = build_country_frame(
        cap(circle.centre, circle.radius_km, circle.earth_radius_km),
        country,
    )
    figure = render(
        answer,
        ORTHOGRAPHIC,
        coastline=None,
        country_outline=country,
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


def test_country_figure_shows_only_the_approved_visible_footer(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    use_coastline_fixture(tmp_path, monkeypatch)
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
    assert boundary is not None
    population_credit = citation(REGISTRY, DATASET)
    boundary_credit = citation(REGISTRY, answer.country.boundary_dataset)
    coastline, _ = registered_coastline(REGISTRY)
    footer_calls: list[tuple[float, str, float]] = []
    current_annotate = render_map_module.annotate

    def capture_annotate(
        figure: Figure, y: float, text: str, size: float, alignment: str = "baseline"
    ) -> None:
        if normalised(text).startswith(normalised(VISIBLE_FOOTER)):
            footer_calls.append((y, alignment, size))
        current_annotate(figure, y, text, size, alignment)

    monkeypatch.setattr(render_map_module, "annotate", capture_annotate)
    figure = render(
        answer,
        ORTHOGRAPHIC,
        coastline=coastline,
        country_outline=boundary,
    )
    try:
        canvas = FigureCanvasAgg(figure)
        canvas.draw()
        footer = figure.texts[-1].get_window_extent(canvas.get_renderer())
        axes = figure.axes[0].get_window_extent()
        rendered_text = normalised("\n".join(artist.get_text() for artist in figure.texts))
        assert "A country with an exceptionally long published name:" in rendered_text
        assert rendered_text.count(normalised(VISIBLE_FOOTER)) == 1
        assert "Population credit:" not in rendered_text
        assert normalised(population_credit) not in rendered_text
        assert normalised(boundary_credit) not in rendered_text
        assert citation(REGISTRY, "coastline-1to110m") == ""
        assert footer_calls == [(FOOTER_BOTTOM, "bottom", FOOTER_SIZE)]
        assert figure.texts[-1].get_position() == (0.5, FOOTER_BOTTOM)
        assert footer.x0 >= 0
        assert footer.x1 <= figure.bbox.x1
        assert (footer.x0 + footer.x1) / 2 == pytest.approx(figure.bbox.width / 2, abs=1)
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


def test_country_title_is_one_line_and_uses_approved_rounding() -> None:
    payload = country_circle_document()
    payload["result"]["country"]["id"] = "AD"
    payload["result"]["country"]["name"] = "Andorra"
    payload["result"]["circle"].update(
        {
            "radius_km": 6,
            "centre": {"lat": 42.5083, "lon": 1.5583},
            "population": 33892,
            "share_achieved": 0.5258,
            "nearest_settlement": {
                "name": "Andorra la Vella",
                "country": "Andorra",
                "distance_km": 5,
                "short_form": False,
            },
        }
    )
    title = title_of(circle_of(payload))

    assert title == (
        "Andorra: 6 km circle holding 53% of the population "
        "(33,892 people); Center: 42.5083, 1.5583"
    )
    assert "\n" not in title


def test_a_document_naming_a_dataset_the_registry_lacks_is_refused() -> None:
    with pytest.raises(UnregisteredDatasetError) as caught:
        citation(REGISTRY, "gpw-v4")
    assert caught.value.key == "gpw-v4"
    assert "gpw-v4" in str(caught.value)


def test_the_registered_coastline_uses_its_registry_path_and_credit(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    path = use_coastline_fixture(tmp_path, monkeypatch)
    coastline, credit = registered_coastline(REGISTRY)

    assert path.is_file()
    assert coastline.bounds == (-20, 0, 20, 0)
    assert credit == citation(REGISTRY, COASTLINE_DATASET)


def test_registered_map_sources_use_the_global_default_and_country_override() -> None:
    world = registered_map_sources(REGISTRY, None)
    france = registered_map_sources(REGISTRY, "FR")
    switzerland = registered_map_sources(REGISTRY, "CH")

    assert world.terrain.path == REGISTRY.datasets["etopo-2022-60s"].path
    assert france.terrain.path == world.terrain.path
    assert switzerland.terrain.path == REGISTRY.datasets["swisstopo-dhm25-200m"].path
    assert [name for name, _ in world.attributions] == ["Terrain"]
    assert switzerland.attributions == (("Terrain", "©swisstopo"),)


def test_registered_map_source_missing_from_registry_is_refused() -> None:
    with pytest.raises(UnregisteredDatasetError, match="etopo-2022-60s"):
        registered_map_sources(Registry(datasets={}), None)


def test_map_context_loader_filters_roads_without_fetched_inputs(tmp_path: Path) -> None:
    sources = fixture_map_sources("FR")
    for dataset in (sources.lakes, sources.rivers, sources.roads, sources.places):
        path = dataset.file(tmp_path)
        path.parent.mkdir(parents=True, exist_ok=True)
        features: list[dict[str, object]] = []
        if dataset == sources.roads:
            for authority, min_zoom, start in (
                ("FRA", 4, [1, 2]),
                ("XNR", 4, [1, 2]),
                ("FRA", 5, [1, 2]),
                ("FRA", None, [1, 2]),
                ("FRA", 4, [100, 50]),
            ):
                features.append(
                    {
                        "type": "Feature",
                        "properties": {"sov_a3": authority, "min_zoom": min_zoom},
                        "geometry": {
                            "type": "LineString",
                            "coordinates": [start, [start[0] + 2, start[1] + 2]],
                        },
                    },
                )
        if dataset == sources.places:
            for name, population, feature_class in (
                ("Town", 120_000, "City"),
                ("Capital", 500_000, "Admin-0 capital"),
                ("Village", 50_000, "City"),
            ):
                features.append(
                    {
                        "type": "Feature",
                        "properties": {
                            "NAME": name,
                            "POP_MAX": population,
                            "FEATURECLA": feature_class,
                        },
                        "geometry": {"type": "Point", "coordinates": [2, 48]},
                    },
                )
        path.write_text(json.dumps({"type": "FeatureCollection", "features": features}))

    layers = load_map_layers(sources, tmp_path, box(0, 0, 5, 5))

    assert layers.roads == (LineString([(1, 2), (3, 4)]),)
    assert layers.lakes == ()
    assert layers.rivers == ()
    assert [(name, capital) for _, name, _, capital in layers.places[:2]] == [
        ("Capital", True),
        ("Town", False),
    ]


def test_city_labels_keep_priority_and_do_not_overlap() -> None:
    figure, axes = plt.subplots(figsize=(4, 3))
    axes.set_xlim(0, 10)
    axes.set_ylim(0, 10)
    labels: list[tuple[Text, float, float, bool]] = []
    for name, x, y, capital in (
        ("Capital", 5.0, 5.0, True),
        ("Nearby", 5.01, 5.0, False),
        ("Distant", 8.0, 8.0, False),
    ):
        label = cast(
            Text,
            axes.annotate(  # pyright: ignore[reportUnknownMemberType] — Matplotlib kwargs: Unknown
                name, (x, y), visible=False
            ),
        )
        labels.append((label, x, y, capital))

    try:
        place_city_labels(axes, labels)
        assert labels[0][0].get_visible()
        assert not labels[1][0].get_visible()
        assert labels[2][0].get_visible()
        canvas = cast(FigureCanvasAgg, figure.canvas)
        renderer = canvas.get_renderer()
        bounds = [label.get_window_extent(renderer) for label, *_ in labels if label.get_visible()]
        assert not bounds[0].overlaps(bounds[1])
    finally:
        plt.close(figure)


def test_map_layers_render_in_order_below_circle_and_centre(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    answer = circle_of(country_circle_document())
    selected_country = box(5, 47, 15, 55)
    base_sources = fixture_map_sources("DE")
    sources = MapSources(
        base_sources.terrain,
        base_sources.lakes,
        base_sources.rivers,
        base_sources.roads,
        base_sources.places,
        terrain_overlays=(base_sources.terrain,),
    )
    prepared: list[TerrainRaster | Sequence[TerrainRaster]] = []

    def capture_terrain(
        dataset: TerrainRaster | Sequence[TerrainRaster],
        view: Frame,
        *,
        root: Path = REPO_ROOT,
        include_below_sea_level: bool = False,
    ) -> tuple[np.ndarray, tuple[float, float, float, float]]:
        prepared.append(dataset)
        return transparent_terrain(
            dataset,
            view,
            root=root,
            include_below_sea_level=include_below_sea_level,
        )

    monkeypatch.setattr(render_map_module, "prepare_terrain", capture_terrain)
    layers = MapLayers(
        sources,
        lakes=(box(9, 50, 10, 51),),
        rivers=(LineString([(8, 50), (9, 51)]),),
        roads=(LineString([(10, 50), (11, 51)]),),
        places=((Point(10, 51), "Example", 150_000, False),),
        root=REPO_ROOT,
    )

    figure = render(
        answer,
        PLATE_CARREE,
        coastline=None,
        country_outline=selected_country,
        map_layers=layers,
    )
    try:
        axes = figure.axes[0]
        assert [image.get_zorder() for image in axes.images] == [
            0,
            render_map_module.TERRAIN_LAYER,
        ]
        assert axes.images[1].get_clip_path() is not None
        assert prepared == [sources.terrain_overlays]
        assert any(patch.get_zorder() == render_map_module.WATER_LAYER for patch in axes.patches)
        assert any(
            collection.get_zorder() == render_map_module.ROAD_LAYER
            for collection in axes.collections
        )
        assert any(
            collection.get_zorder() == render_map_module.PLACE_LAYER
            for collection in axes.collections
        )
        assert any(text.get_text() == "Example" for text in axes.texts)
        assert any(patch.get_zorder() == render_map_module.CIRCLE_LAYER for patch in axes.patches)
        assert any(line.get_zorder() == render_map_module.CENTRE_LAYER for line in axes.lines)
        assert layers.root == REPO_ROOT
    finally:
        plt.close(figure)


def test_world_map_skips_empty_projected_context_lines() -> None:
    sources = fixture_map_sources(None)
    layers = MapLayers(
        sources, lakes=(), rivers=(LineString(),), roads=(), places=(), root=REPO_ROOT
    )

    figure = render(
        circle_of(document()),
        PLATE_CARREE,
        coastline=None,
        map_layers=layers,
    )
    plt.close(figure)


def test_render_file_uses_registered_coastline_and_credits_world_figures_once(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    coastline_path = use_coastline_fixture(tmp_path, monkeypatch)

    def fixture_boundaries(_answer: Answer, _registry: Registry) -> dict[str, BaseGeometry]:
        return {"fixture": box(70, 15, 90, 35)}

    def fixture_territory_share(_circle: Circle, _territories: Iterable[BaseGeometry]) -> float:
        return 0.375

    monkeypatch.setattr(
        render_map_module,
        "load_map_boundaries",
        fixture_boundaries,
    )
    monkeypatch.setattr(
        territory_coverage_module,
        "territory_share",
        fixture_territory_share,
    )
    source = tmp_path / "map.json"
    source_document = territory_document()
    source.write_text(json.dumps(source_document), encoding="utf-8")
    annotations: list[str] = []
    original_annotate = render_map_module.annotate

    def capture_annotate(
        figure: Figure, y: float, text: str, size: float, alignment: str = "baseline"
    ) -> None:
        annotations.append(text)
        original_annotate(figure, y, text, size, alignment)

    monkeypatch.setattr(render_map_module, "annotate", capture_annotate)
    basemap_paths: list[Path] = []
    current_basemap = render_map_module.basemap

    def capture_basemap(path: Path) -> BaseGeometry:
        basemap_paths.append(path)
        return current_basemap(path)

    monkeypatch.setattr(render_map_module, "basemap", capture_basemap)
    output = tmp_path / "map.png"
    render_file(source, output)

    metadata = png_info(output)
    assert basemap_paths == [coastline_path]
    assert "Title" in metadata
    assert "Copyright" in metadata
    assert normalised(citation(REGISTRY, DATASET)) in normalised(str(metadata["Copyright"]))
    assert "Coastline data:" in str(metadata["Copyright"])
    assert str(metadata["Copyright"]).count(COASTLINE_CREDIT) == 1
    assert "© Andrii Melnyk, 2026" in str(metadata["Copyright"])
    answer = circle_of(source_document)
    assert metadata["Title"] == title_of(answer)
    assert metadata["Title"].endswith("Circle covers 38% of Earth\u2019s land area")
    assert metadata["Description"] == (
        f"Answered from population dataset {DATASET} at decimation k=10"
    )
    xmp = str(metadata["XML:com.adobe.xmp"])
    assert 'pc:population="1254363867.93"' in xmp
    assert 'pc:territoryShare="0.375"' in xmp
    sidecar = json.loads((tmp_path / "map.json").read_text(encoding="utf-8"))
    assert sidecar["result"] == source_document["result"]
    assert sidecar["render_metadata"]["Title"] == metadata["Title"]
    assert sidecar["render_metadata"]["Description"] == metadata["Description"]
    assert sidecar["render_metadata"]["Copyright"] == metadata["Copyright"]
    assert sidecar["render_metadata"]["XML:com.adobe.xmp"]["population"] == WORLD_POPULATION
    assert sidecar["render_metadata"]["XML:com.adobe.xmp"]["territoryShare"] == TERRITORY_SHARE
    observed_annotations = [" ".join(annotation.split()) for annotation in annotations]
    assert observed_annotations[:2] == [metadata["Title"], metadata["Description"]]
    assert observed_annotations[2].startswith(VISIBLE_FOOTER)
    assert citation(REGISTRY, "etopo-2022-60s") in observed_annotations[2]
    assert observed_annotations[2].endswith("See full attribution in image metadata")


def test_the_footer_artist_shows_the_approved_visible_credit() -> None:
    figure = render(circle_of(document()), PLATE_CARREE, coastline=None)
    drawn_text = " ".join(normalised(artist.get_text()) for artist in figure.texts)
    assert normalised(VISIBLE_FOOTER) in drawn_text
    assert "See full attribution in image metadata" in drawn_text
    assert MARKER not in drawn_text


def test_world_country_boundaries_use_the_world_highlight_fill() -> None:
    figure = render(
        circle_of(document()),
        PLATE_CARREE,
        coastline=None,
        other_countries=[box(-20, -10, -10, 0), box(10, 0, 20, 10)],
    )
    try:
        country_patches = [
            patch for patch in figure.axes[0].patches if patch.get_zorder() == COUNTRY_LAYER
        ]
        assert len(country_patches) == 1
        assert country_patches[0].get_linewidth() == OUTLINE_WIDTH
        assert country_patches[0].get_facecolor()[:3] == to_rgb(COUNTRY)
    finally:
        plt.close(figure)


@pytest.mark.parametrize("projection", [PLATE_CARREE, ORTHOGRAPHIC])
def test_country_breakdown_maps_keep_the_approved_footer_inside_the_figure(
    projection: str,
    tmp_path: Path,
) -> None:
    answer = circle_of(country_document(18))
    figure = render(answer, projection, coastline=None)
    try:
        output = tmp_path / f"countries-{projection}.png"
        figure.savefig(output, dpi=120)  # pyright: ignore[reportUnknownMemberType] — matplotlib kwargs
        canvas = FigureCanvasAgg(figure)
        canvas.draw()
        footer = figure.texts[-1].get_window_extent(canvas.get_renderer())
        axes = figure.axes[0].get_window_extent()
        rendered_text = "\n".join(artist.get_text() for artist in figure.texts)
        assert footer.x0 >= 0
        assert footer.x1 <= figure.bbox.x1
        assert footer.y0 >= 0
        assert footer.y1 <= axes.y0
        assert VISIBLE_FOOTER in rendered_text
        assert "Boundary credit: © EuroGeographics" not in rendered_text
        assert output.read_bytes().startswith(PNG_MAGIC)
    finally:
        plt.close(figure)


def test_the_raster_title_uses_the_rounded_radius_share_population_and_centre() -> None:
    answer = circle_of(document())
    figure = render(answer, ORTHOGRAPHIC, coastline=None)
    drawn_text = " ".join(normalised(artist.get_text()) for artist in figure.texts)
    assert "1,000 km circle" in drawn_text
    assert "16% of the population" in drawn_text
    assert "1,254,363,868 people" in drawn_text
    assert f"{CENTRE_LAT:.4f}, {CENTRE_LON:.4f}" in drawn_text
    assert "Center:" in drawn_text
    assert f"Answered from population dataset {DATASET} at decimation k=10" in drawn_text


def test_country_map_image_dimensions_preserve_area_and_cap_wide_maps() -> None:
    landscape = render_map_module.figure_size((0.0, 0.0, 11 / 6, 1.0), country_map=True)
    portrait = render_map_module.figure_size((0.0, 0.0, 0.5, 1.0), country_map=True)
    extra_wide = render_map_module.figure_size((0.0, 0.0, 3.0, 1.0), country_map=True)
    world = render_map_module.figure_size((0.0, 0.0, 0.5, 1.0), country_map=False)

    assert (landscape[0] * 200, landscape[1] * 200) == pytest.approx((2200, 1200))
    assert portrait[0] / portrait[1] == pytest.approx(0.5)
    assert portrait[0] * portrait[1] * 200**2 == pytest.approx(2_640_000)
    assert extra_wide[0] * 200 == pytest.approx(2200)
    assert extra_wide[1] / extra_wide[0] == pytest.approx(1 / 3)
    assert world == (11.0, 6.0)


def test_measured_and_nested_documents_use_the_one_line_title() -> None:
    assert title_of(circle_of(document())) == (
        "1,000 km circle holding 16% of the population "
        "(1,254,363,868 people); Center: 25.1250, 79.7080"
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
        "2,639 km circle holding 25% of the population (52,635 people); Center: -85.0000, 105.0000"
    )


def test_map_title_label_replaces_the_generic_subject_and_keeps_technical_details() -> None:
    answer = circle_of(document())
    assert title_of(answer, "Most populous 1000km circle") == (
        "Most populous 1000km circle holding 16% of the population "
        "(1,254,363,868 people); Center: 25.1250, 79.7080"
    )


def test_country_title_names_the_country_and_population_dataset() -> None:
    payload = country_circle_document()
    payload["result"]["circle"]["territory_share"] = TERRITORY_SHARE
    answer = circle_of(payload)
    assert title_of(answer) == (
        "Germany: 200 km circle holding 50% of the population "
        "(40 people); Center: 51.0000, 10.0000; Circle covers 38% of Germany"
    )


def test_title_formats_territory_percentage_to_two_significant_digits() -> None:
    payload = document()
    payload["result"]["territory_share"] = 0.01234
    assert title_of(circle_of(payload)).endswith("Circle covers 1.2% of Earth\u2019s land area")

    payload["result"]["territory_share"] = 0.00001234
    assert title_of(circle_of(payload)).endswith("Circle covers 0.0012% of Earth\u2019s land area")


def test_country_renderer_uses_attribution_carried_by_the_selected_combination(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    payload = country_circle_document()
    payload["result"]["selection"] = {
        "combination_id": "local-preview",
        "resolution": 2,
        "population_source": {
            "name": DATASET,
            "sha256": "b" * 64,
            "attribution": "Selected population credit",
        },
        "boundary_source": {
            "name": "gisco-countries-2024-1m",
            "sha256": "a" * 64,
            "attribution": "Selected boundary credit",
        },
    }
    source = tmp_path / "country.json"
    source.write_text(json.dumps(payload), encoding="utf-8")
    output = tmp_path / "country.png"
    annotations: list[str] = []
    current_annotate = render_map_module.annotate

    def capture_annotate(
        figure: Figure, y: float, text: str, size: float, alignment: str = "baseline"
    ) -> None:
        annotations.append(" ".join(text.split()))
        current_annotate(figure, y, text, size, alignment)

    monkeypatch.setattr(render_map_module, "annotate", capture_annotate)

    def selected_outline(_answer: Answer, _registry: Registry) -> dict[str, BaseGeometry]:
        return {"DE": box(5, 47, 15, 55)}

    monkeypatch.setattr(render_map_module, "load_map_boundaries", selected_outline)

    def refuse_coastline(_registry: Registry) -> tuple[BaseGeometry, str]:
        message = "no-coastlines loaded coastline"
        raise AssertionError(message)

    monkeypatch.setattr(render_map_module, "registered_coastline", refuse_coastline)
    render_file(source, output, no_coastlines=True)
    assert annotations[-1].startswith(VISIBLE_FOOTER)
    assert "Terrain data:" in annotations[-1]
    assert annotations[-1].endswith("See full attribution in image metadata")
    assert output.read_bytes().startswith(PNG_MAGIC)
    metadata = png_info(output)
    assert "Title" in metadata
    assert "Description" in metadata
    assert "Copyright" in metadata
    assert "Selected population credit" in str(metadata["Copyright"])
    assert "Selected boundary credit" in str(metadata["Copyright"])


def test_local_country_boundary_without_a_registry_row_has_no_outline() -> None:
    payload = country_circle_document()
    payload["result"]["country"]["boundary_dataset"] = "file:data/boundaries/local.geojson"
    payload["result"]["country"]["boundary_revision"] = None
    payload["result"]["selection"] = {
        "combination_id": "local-preview",
        "resolution": 2,
        "population_source": {"name": DATASET},
        "boundary_source": {"name": "file:data/boundaries/local.geojson"},
    }
    assert load_outline(circle_of(payload), REGISTRY) is None


@pytest.mark.parametrize("projection", [PLATE_CARREE, ORTHOGRAPHIC])
def test_a_complete_figure_is_written_where_it_is_told(
    projection: str, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # The figure a test writes goes to tmp_path: no rendered map enters the repository, which is the
    # first project invariant and the reason there is no baseline image to compare against.
    source = tmp_path / "circle.json"
    source.write_text(json.dumps(document()), encoding="utf-8")
    output = tmp_path / "figures" / "map.png"
    use_coastline_fixture(tmp_path, monkeypatch)

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
    use_coastline_fixture(tmp_path, monkeypatch)
    monkeypatch.setattr(render_map_module, "load_map_boundaries", empty_boundaries)
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

    original_copyright = render_map_module.copyright_of

    def capture_copyright(
        answer: Answer,
        attribution: str,
        coastline_attribution: str | None,
        boundary_attribution: str | None,
        map_attributions: tuple[tuple[str, str], ...] = (),
    ) -> str:
        drawn.append(attribution)
        return original_copyright(
            answer,
            attribution,
            coastline_attribution,
            boundary_attribution,
            map_attributions,
        )

    monkeypatch.setattr("population_circles.render_map.copyright_of", capture_copyright)

    named = document()
    named["provenance"]["dataset"] = "second-raster"
    source = tmp_path / "circle.json"
    source.write_text(json.dumps(named), encoding="utf-8")

    assert (
        main(
            ["--input", str(source), "--output", str(tmp_path / "map.png"), "--no-coastlines"],
        )
        == 0
    )
    assert drawn == ["the credit the second dataset requires"]
