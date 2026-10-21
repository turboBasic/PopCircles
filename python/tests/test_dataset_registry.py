import hashlib
import struct
from pathlib import PurePosixPath

import pytest
from pydantic import ValidationError

from population_circles.dataset_registry import (
    REGISTRY,
    REPO_ROOT,
    BoundaryVector,
    PopulationRaster,
    SettlementVector,
    load,
    parse,
)

# The bit pattern crates/popcircles/src/raster.rs asserts of its own GPW_NODATA, in a unit test that
# mise run test and CI both run. Pinning against that, rather than against the same literal in a
# deselected integration test, compares two values a gate enforces instead of two spellings.
RUST_NODATA_BITS = 0xFF7FFFFD

# Named here rather than imported from the renderer: this file tests the registry, and which row a
# figure happens to credit is the renderer's business.
RASTER_KEY = "population-count-2020-30arcsec"
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
    committed = [d for d in datasets.values() if isinstance(d, BoundaryVector)]
    assert committed
    for dataset in committed:
        assert dataset.fetch_url is None


def test_an_attribution_is_present_even_when_nothing_is_owed() -> None:
    # Empty rather than absent is the whole point: a consumer reads one field for every dataset.
    attributions = {key: d.attribution for key, d in load().datasets.items()}
    assert attributions["coastline-1to110m"] == ""
    assert "CIESIN" in attributions[RASTER_KEY]


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
    text = REGISTRY.read_text(encoding="utf-8") + "\nepsg = 4326\n"
    with pytest.raises(ValidationError, match="epsg"):
        parse(text)
