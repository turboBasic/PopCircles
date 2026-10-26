import io
import sys
from collections.abc import Callable
from pathlib import Path

import pytest

from population_circles.dataset_registry import PopulationRaster, Registry, load, parse
from repo_tools import fetch_data
from repo_tools.fetch_data import FetchError, acquire

PAYLOAD = b"hello"
PAYLOAD_SHA = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"

KEY = "thing-1to2m"
RELATIVE = f"data/settlements/{KEY}.geojson"
SOURCE = "https://example.invalid/upstream/thing"


def registry_text(
    *,
    key: str = KEY,
    fetch_url: str | None = "https://example.invalid/thing.geojson",
    sha256: str = PAYLOAD_SHA,
    size: int = len(PAYLOAD),
    attribution: str = "",
) -> str:
    fetch = "" if fetch_url is None else f'fetch_url = "{fetch_url}"\n'
    relative = f"data/settlements/{key}.geojson"
    return (
        f"[datasets.{key}]\n"
        f'kind = "settlement-vector"\n'
        f'path = "{relative}"\n'
        f"bytes = {size}\n"
        f'sha256 = "{sha256}"\n'
        f"{fetch}"
        f'source_url = "{SOURCE}"\n'
        f'licence = "CC BY 4.0"\n'
        f'licence_url = "https://example.invalid/licence"\n'
        f'attribution = "{attribution}"\n'
    )


def serving(payload: bytes) -> Callable[..., io.BytesIO]:
    def opener(_url: str, *_args: object, **_kwargs: object) -> io.BytesIO:
        return io.BytesIO(payload)

    return opener


def refusing() -> Callable[..., io.BytesIO]:
    def opener(_url: str, *_args: object, **_kwargs: object) -> io.BytesIO:
        message = "a present, verified file must not be downloaded again"
        raise AssertionError(message)

    return opener


def target(root: Path) -> Path:
    return root / RELATIVE


def siblings(root: Path) -> list[str]:
    directory = target(root).parent
    return sorted(p.name for p in directory.iterdir()) if directory.is_dir() else []


def test_a_matching_payload_is_placed(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr("urllib.request.urlopen", serving(PAYLOAD))
    acquire(parse(registry_text()), tmp_path)
    assert target(tmp_path).read_bytes() == PAYLOAD
    assert siblings(tmp_path) == [target(tmp_path).name]


def test_the_registered_coarse_dataset_uses_verified_fetch(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    key = "population-count-2020-10arcmin"
    dataset = load().datasets[key]
    assert isinstance(dataset, PopulationRaster)
    # The isolated fetch fixture has no native source row.
    synthetic = dataset.model_copy(
        update={
            "bytes": len(PAYLOAD),
            "sha256": PAYLOAD_SHA,
            "derived_from_dataset": None,
            "derived_from_sha256": None,
        }
    )
    monkeypatch.setattr("urllib.request.urlopen", serving(PAYLOAD))
    acquire(Registry(datasets={key: synthetic}), tmp_path)
    assert synthetic.file(tmp_path).read_bytes() == PAYLOAD


def test_a_wrong_payload_places_nothing_and_leaves_no_part(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # The claim the .part-then-rename dance exists to make: a reader never sees a file that failed
    # verification, and a refused fetch leaves nothing behind to confuse the next run.
    monkeypatch.setattr("urllib.request.urlopen", serving(b"wrong" * 1))
    with pytest.raises(FetchError, match="hashed to") as raised:
        acquire(parse(registry_text(size=len(b"wrong"))), tmp_path)
    # The acceptance criterion says the message names the fix, so it is asserted rather than left as
    # a constant nobody reads.
    assert "mise run data:get" in str(raised.value)
    assert not target(tmp_path).exists()
    assert siblings(tmp_path) == []


def test_a_truncated_response_is_named_by_its_length_not_its_hash(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # A short read raises nothing, since urlopen closes the connection instead, so the length check
    # catches it — and runs before the digest, so it reports without hashing the whole file.
    monkeypatch.setattr("urllib.request.urlopen", serving(PAYLOAD[:2]))
    with pytest.raises(FetchError, match="arrived as 2 bytes") as raised:
        acquire(parse(registry_text()), tmp_path)
    assert "mise run data:get" in str(raised.value)
    assert not target(tmp_path).exists()
    assert siblings(tmp_path) == []


def test_a_present_and_verified_file_is_not_downloaded_again(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr("urllib.request.urlopen", refusing())
    target(tmp_path).parent.mkdir(parents=True)
    target(tmp_path).write_bytes(PAYLOAD)
    acquire(parse(registry_text()), tmp_path)
    assert target(tmp_path).read_bytes() == PAYLOAD


def test_a_committed_dataset_that_is_absent_names_where_it_came_from(tmp_path: Path) -> None:
    # No fetch_url means the file is a Git blob, so this command cannot repair its absence and says
    # what would, rather than reporting success over a dataset that is not there.
    with pytest.raises(FetchError, match=SOURCE):
        acquire(parse(registry_text(fetch_url=None)), tmp_path)
    assert not target(tmp_path).exists()


def test_a_fetch_url_that_is_not_https_is_refused(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr("urllib.request.urlopen", serving(PAYLOAD))
    with pytest.raises(FetchError, match="scheme"):
        acquire(parse(registry_text(fetch_url="http://example.invalid/thing.geojson")), tmp_path)
    assert not target(tmp_path).exists()
    assert siblings(tmp_path) == []


def test_nothing_is_owed_for_an_empty_attribution(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr("urllib.request.urlopen", serving(PAYLOAD))
    assert acquire(parse(registry_text()), tmp_path) == []


def test_a_non_empty_attribution_is_returned_to_be_printed(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr("urllib.request.urlopen", serving(PAYLOAD))
    owed = acquire(parse(registry_text(attribution="Credit where it is due.")), tmp_path)
    assert len(owed) == 1
    assert "Credit where it is due." in owed[0]


def test_fetch_data_defaults_to_every_dataset(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    registry = parse("\n".join(registry_text(key=key) for key in ("first", "second")))
    requested: list[str] = []

    def opener(url: str, *_args: object, **_kwargs: object) -> io.BytesIO:
        requested.append(url)
        return io.BytesIO(PAYLOAD)

    monkeypatch.setattr("urllib.request.urlopen", opener)
    monkeypatch.setattr(fetch_data, "load", lambda: registry)
    monkeypatch.setattr(fetch_data, "REPO_ROOT", tmp_path)
    monkeypatch.setattr(sys, "argv", ["fetch-data"])
    assert fetch_data.main() == 0
    assert requested == ["https://example.invalid/thing.geojson"] * 2


def test_fetch_data_selects_one_dataset(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    registry = parse("\n".join(registry_text(key=key) for key in ("first", "second")))
    requested: list[str] = []

    def opener(url: str, *_args: object, **_kwargs: object) -> io.BytesIO:
        requested.append(url)
        return io.BytesIO(PAYLOAD)

    monkeypatch.setattr("urllib.request.urlopen", opener)
    monkeypatch.setattr(fetch_data, "load", lambda: registry)
    monkeypatch.setattr(fetch_data, "REPO_ROOT", tmp_path)
    monkeypatch.setattr(sys, "argv", ["fetch-data", "--dataset", "second"])
    assert fetch_data.main() == 0
    assert requested == ["https://example.invalid/thing.geojson"]
    assert (tmp_path / "data/settlements/second.geojson").is_file()
    assert not (tmp_path / "data/settlements/first.geojson").exists()


def test_fetch_data_refuses_an_unknown_dataset_before_request(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    registry = parse(registry_text())
    monkeypatch.setattr("urllib.request.urlopen", refusing())
    monkeypatch.setattr(fetch_data, "load", lambda: registry)
    monkeypatch.setattr(fetch_data, "REPO_ROOT", tmp_path)
    monkeypatch.setattr(sys, "argv", ["fetch-data", "--dataset", "missing"])
    assert fetch_data.main() == 1
    assert "unknown dataset 'missing'" in capsys.readouterr().err
    assert not target(tmp_path).exists()


def test_bad_selected_download_leaves_no_target_file(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr("urllib.request.urlopen", serving(b"wrong"))
    with pytest.raises(FetchError, match="hashed to"):
        acquire(parse(registry_text()), tmp_path, KEY)
    assert not target(tmp_path).exists()
    assert siblings(tmp_path) == []


def test_a_synthetic_registry_never_resolves_into_this_checkout(tmp_path: Path) -> None:
    # The reason `file` takes a root: without it every test above would have been reading and
    # writing the real data/ directory while appearing to pass against a fixture.
    dataset = parse(registry_text()).datasets[KEY]
    assert dataset.file(tmp_path) == target(tmp_path)
    assert tmp_path in dataset.file(tmp_path).parents
