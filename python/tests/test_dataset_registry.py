import hashlib
import struct
from pathlib import Path, PurePosixPath

import pytest
from pydantic import ValidationError

from population_circles.infrastructure.geodata.dataset_registry import (
    REGISTRY,
    REPO_ROOT,
    BoundaryVector,
    MapContextVector,
    PopulationRaster,
    SettlementVector,
    TerrainRaster,
    load,
    parse,
)

# The bit pattern crates/popcircles/src/raster.rs asserts of its own GPW_NODATA, in a unit test that
# mise run test and CI both run. Pinning against that, rather than against the same literal in a
# deselected integration test, compares two values a gate enforces instead of two spellings.
RUST_NODATA_BITS = 0xFF7FFFFD
COUNTRY_FEATURE_COUNT = 263

# Named here rather than imported from the renderer: this file tests the registry, and which row a
# figure happens to credit is the renderer's business.
RASTER_KEY = "population-count-2020-30arcsec"
COARSE_RASTER_KEY = "population-count-2020-10arcmin"
GISCO_COASTLINE_KEY = "CNTR_BN_01M_2024_4326_COASTL"
GISCO_COASTLINE_ATTRIBUTION = "© EuroGeographics for the administrative boundaries"
SETTLEMENT_ROW = """
[datasets.example-places]
kind = "settlement-vector"
path = "data/settlements/example-places.geojson"
bytes = 5
sha256 = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
fetch_url = "https://example.invalid/example-places.geojson"
source_url = "https://example.invalid/source"
licence = "Public domain"
licence_url = "https://example.invalid/licence"
attribution = ""
"""
TERRAIN_ROW = """
[datasets.example-terrain]
kind = "terrain-raster"
path = "data/terrain/example-terrain.tif"
bytes = 5
sha256 = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
fetch_url = "https://example.invalid/example-terrain.tif"
source_url = "https://example.invalid/source"
licence = "Test licence"
licence_url = "https://example.invalid/licence"
attribution = ""
format = "GeoTIFF"
width = 10
height = 20
epsg = 4326
x_origin = -180.0
y_origin = 90.0
x_step = 0.016666666666666666
y_step = -0.016666666666666666
nodata = -99999.0
"""
MAP_CONTEXT_ROW = """
[datasets.example-lakes]
kind = "map-context-vector"
path = "data/context/example-lakes.geojson"
bytes = 5
sha256 = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
fetch_url = "https://example.invalid/example-lakes.geojson"
source_url = "https://example.invalid/source"
licence = "Test licence"
licence_url = "https://example.invalid/licence"
attribution = ""
format = "GeoJSON"
crs = "OGC:CRS84"
feature_count = 2
revision = "Test 1.0"
"""
COUNTRY_BOUNDARY_ROW = """
[datasets.countries-2024]
kind = "boundary-vector"
path = "data/boundaries/countries-2024.geojson"
bytes = 5
sha256 = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
fetch_url = "https://example.invalid/countries-2024.geojson"
source_url = "https://example.invalid/source"
licence = "Test licence"
licence_url = "https://example.invalid/licence"
attribution = "Required credit"
format = "GeoJSON"
crs = "EPSG:4326"
axis_order = "longitude-latitude"
feature_count = 263
revision = "2024"
source_id_field = "CNTR_ID"
display_name_field = "CNTR_NAME"
identity_policy = "ADR 0015"
"""


def test_repository_root_is_the_checkout() -> None:
    assert Path(__file__).resolve().parents[2] == REPO_ROOT


def test_every_key_is_its_paths_filename_stem() -> None:
    # The rule that makes a key, a file, a release asset and a heading in data/README.md one string.
    for key, dataset in load().datasets.items():
        assert PurePosixPath(dataset.path).stem == key


def test_a_key_that_is_not_its_stem_is_refused() -> None:
    # The negative half: the check fires at load, not when data:get happens to look.
    text = REGISTRY.read_text(encoding="utf-8").replace(
        f"[datasets.{RASTER_KEY}]",
        "[datasets.population-count-2020]",
    )
    with pytest.raises(ValidationError, match="whose stem is"):
        parse(text)


def test_the_nodata_literal_narrows_to_the_f32_rust_pins() -> None:
    # Compared bit for bit by the reader, so "close enough" is not a property this value may have.
    raster = load().datasets[RASTER_KEY]
    assert isinstance(raster, PopulationRaster)
    assert struct.unpack("<I", struct.pack("<f", raster.nodata))[0] == RUST_NODATA_BITS


def test_the_coarse_population_names_its_direct_source_and_grid() -> None:
    datasets = load().datasets
    coarse = datasets[COARSE_RASTER_KEY]
    source = datasets[RASTER_KEY]
    assert isinstance(coarse, PopulationRaster)
    assert isinstance(source, PopulationRaster)
    assert (coarse.width, coarse.height) == (2160, 1080)
    assert coarse.derived_from_dataset == RASTER_KEY
    assert coarse.derived_from_sha256 == source.sha256


def test_a_derived_population_source_must_match_the_registered_digest() -> None:
    text = REGISTRY.read_text(encoding="utf-8").replace(
        'derived_from_sha256 = "956993aa500774aed548c8e1af1a3a68fc164577be82ca799d4ae8568d445e9d"',
        f'derived_from_sha256 = "{"0" * 64}"',
    )
    with pytest.raises(ValidationError, match="does not match its registered source identity"):
        parse(text)


def test_a_committed_row_describes_the_file_on_disk() -> None:
    # bytes and sha256 are what a reader verifies, so a stale one is worse than none. Only the rows
    # with no `fetch_url`: those files are committed, so they are present on every clone.
    committed = [d for d in load().datasets.values() if d.fetch_url is None]
    assert committed
    for dataset in committed:
        content = dataset.file(REPO_ROOT).read_bytes()
        assert len(content) == dataset.bytes
        assert hashlib.sha256(content).hexdigest() == dataset.sha256


@pytest.mark.raster
def test_a_fetched_row_describes_the_file_once_it_has_been_fetched() -> None:
    # Deselected by default and never in CI: a fetched dataset is absent from the checkout CI runs
    # on, so asserting its length there fails for having no file rather than for a wrong
    # figure — platform.md "Testing". `mise run test:python-raster` is what runs it.
    fetched = [d for d in load().datasets.values() if d.fetch_url is not None]
    assert fetched
    for dataset in fetched:
        content = dataset.file(REPO_ROOT).read_bytes()
        assert len(content) == dataset.bytes
        assert hashlib.sha256(content).hexdigest() == dataset.sha256


def test_the_committed_dataset_has_no_fetch_url_and_the_published_one_does() -> None:
    datasets = load().datasets
    published = datasets[RASTER_KEY]
    assert published.fetch_url is not None
    assert published.fetch_url.endswith(PurePosixPath(published.path).name)
    committed = [
        d for d in datasets.values() if isinstance(d, BoundaryVector) and d.fetch_url is None
    ]
    assert committed
    for dataset in committed:
        assert dataset.fetch_url is None


def test_an_attribution_is_present_even_when_nothing_is_owed() -> None:
    # Empty rather than absent is the whole point: a consumer reads one field for every dataset.
    attributions = {key: d.attribution for key, d in load().datasets.items()}
    assert attributions["coastline-1to110m"] == ""
    assert "CIESIN" in attributions[RASTER_KEY]


def test_gisco_coastline_is_registered_once_with_its_required_credit() -> None:
    matches = [key for key in load().datasets if key == GISCO_COASTLINE_KEY]
    dataset = load().datasets[GISCO_COASTLINE_KEY]

    assert matches == [GISCO_COASTLINE_KEY]
    assert isinstance(dataset, BoundaryVector)
    assert dataset.path == f"data/boundaries/{GISCO_COASTLINE_KEY}.geojson"
    assert dataset.revision == "Countries 2024 snapshot downloaded 2026-10-26"
    assert dataset.attribution == GISCO_COASTLINE_ATTRIBUTION
    assert GISCO_COASTLINE_ATTRIBUTION in (REPO_ROOT / "data/README.md").read_text()


def test_an_unknown_kind_is_refused_rather_than_read_as_a_bare_dataset() -> None:
    text = REGISTRY.read_text(encoding="utf-8").replace(
        'kind = "boundary-vector"',
        'kind = "coastline"',
    )
    with pytest.raises(ValidationError):
        parse(text)


def test_a_synthetic_settlement_vector_needs_no_grid_fields() -> None:
    dataset = parse(SETTLEMENT_ROW).datasets["example-places"]

    assert isinstance(dataset, SettlementVector)
    assert dataset.path == "data/settlements/example-places.geojson"


def test_a_synthetic_terrain_row_requires_complete_grid_metadata() -> None:
    dataset = parse(TERRAIN_ROW).datasets["example-terrain"]

    assert isinstance(dataset, TerrainRaster)
    assert (dataset.width, dataset.height, dataset.epsg) == (10, 20, 4326)
    with pytest.raises(ValidationError, match="x_step"):
        parse(TERRAIN_ROW.replace("x_step = 0.016666666666666666\n", ""))
    with pytest.raises(ValidationError, match="grid steps must be nonzero"):
        parse(TERRAIN_ROW.replace("x_step = 0.016666666666666666", "x_step = 0"))


def test_an_ascii_grid_terrain_archive_must_name_its_member() -> None:
    text = TERRAIN_ROW.replace('format = "GeoTIFF"', 'format = "ESRI ASCII Grid"')

    with pytest.raises(ValidationError, match="must name its archive member"):
        parse(text)


def test_a_synthetic_map_context_vector_requires_source_metadata() -> None:
    dataset = parse(MAP_CONTEXT_ROW).datasets["example-lakes"]

    assert isinstance(dataset, MapContextVector)
    assert (dataset.crs, dataset.feature_count, dataset.revision) == (
        "OGC:CRS84",
        2,
        "Test 1.0",
    )
    with pytest.raises(ValidationError, match="feature_count"):
        parse(MAP_CONTEXT_ROW.replace("feature_count = 2\n", ""))


def test_every_selected_map_context_source_is_registered() -> None:
    datasets = load().datasets
    keys = (
        "natural-earth-10m-lakes-v5.0.0",
        "natural-earth-10m-rivers-lake-centerlines-v5.0.0",
        "natural-earth-10m-roads-v5.0.0",
    )

    assert all(isinstance(datasets[key], MapContextVector) for key in keys)


def test_a_settlement_vector_key_must_match_its_file_stem() -> None:
    with pytest.raises(ValidationError, match="whose stem is"):
        parse(SETTLEMENT_ROW.replace("example-places.geojson", "other-places.geojson"))


def test_a_raster_missing_a_grid_field_does_not_construct() -> None:
    # The reason the union is discriminated rather than one model with optional grid fields: an
    # incomplete raster is refused here instead of surfacing as a None eight callers downstream.
    text = REGISTRY.read_text(encoding="utf-8").replace("epsg = 4326\n", "")
    with pytest.raises(ValidationError, match="epsg"):
        parse(text)


def test_a_boundary_carrying_grid_fields_does_not_construct() -> None:
    # The other half, and what `extra="forbid"` is load-bearing for: a grid on a vector row is a
    # copy-paste, and ignoring it would let the file say something the reader silently drops.
    text = COUNTRY_BOUNDARY_ROW + "\nepsg = 4326\n"
    with pytest.raises(ValidationError, match="epsg"):
        parse(text)


def test_a_country_boundary_accepts_complete_metadata_and_fetch_delivery() -> None:
    dataset = parse(COUNTRY_BOUNDARY_ROW).datasets["countries-2024"]

    assert isinstance(dataset, BoundaryVector)
    assert dataset.format == "GeoJSON"
    assert dataset.crs == "EPSG:4326"
    assert dataset.axis_order == "longitude-latitude"
    assert dataset.feature_count == COUNTRY_FEATURE_COUNT
    assert dataset.revision == "2024"
    assert dataset.source_id_field == "CNTR_ID"
    assert dataset.display_name_field == "CNTR_NAME"
    assert dataset.identity_policy == "ADR 0015"
    assert dataset.fetch_url is not None
    assert dataset.path == "data/boundaries/countries-2024.geojson"


@pytest.mark.parametrize(
    ("field", "value"),
    [("feature_count", "0"), ("crs", '"EPSG:3857"')],
)
def test_a_country_boundary_rejects_invalid_metadata(field: str, value: str) -> None:
    original = "feature_count = 263" if field == "feature_count" else 'crs = "EPSG:4326"'
    text = COUNTRY_BOUNDARY_ROW.replace(original, f"{field} = {value}")

    with pytest.raises(ValidationError):
        parse(text)


def test_a_country_boundary_rejects_missing_metadata() -> None:
    with pytest.raises(ValidationError, match="metadata must be complete"):
        parse(COUNTRY_BOUNDARY_ROW.replace('source_id_field = "CNTR_ID"\n', ""))


def test_a_country_boundary_key_must_match_its_file_stem() -> None:
    with pytest.raises(ValidationError, match="whose stem is"):
        parse(COUNTRY_BOUNDARY_ROW.replace("countries-2024.geojson", "other.geojson"))


def test_a_boundary_rejects_unknown_fields() -> None:
    with pytest.raises(ValidationError, match="Extra inputs are not permitted"):
        parse(COUNTRY_BOUNDARY_ROW + 'unexpected = "value"\n')
