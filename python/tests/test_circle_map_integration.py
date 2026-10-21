import json
import subprocess
import sys
from pathlib import Path

import pytest

from population_circles import render_map
from population_circles.circle_map import REPO_ROOT, Options, WorkflowError, workflow
from population_circles.dataset_registry import parse
from test_render_map import PNG_MAGIC


def run(arguments: list[str], directory: Path) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(  # noqa: S603 — test-owned argument vectors, no shell
        arguments,
        cwd=directory,
        capture_output=True,
        check=True,
    )


@pytest.fixture(scope="session")
def numeric_binary() -> Path:
    run(["mise", "run", "build"], REPO_ROOT)
    metadata = json.loads(
        run(["cargo", "metadata", "--no-deps", "--format-version", "1"], REPO_ROOT).stdout
    )
    return Path(metadata["target_directory"]) / "release" / "popcircles"


def test_real_build_reuse_and_missing_raster(
    tmp_path: Path,
    numeric_binary: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    run(
        [
            "cargo",
            "run",
            "--release",
            "-p",
            "popcircles-cli",
            "--example",
            "map_fixture",
            "--",
            str(tmp_path),
        ],
        REPO_ROOT,
    )
    registry = parse((tmp_path / "data/registry.toml").read_text())
    monkeypatch.setattr(render_map, "load", lambda: registry)
    options = Options(
        dataset="tiny",
        share="50",
        cache=tmp_path / "table",
        output=tmp_path / "map.png",
        document=tmp_path / "map.json",
        ledger=tmp_path / "map.radii.json",
    )
    calls: list[list[str]] = []
    results: list[subprocess.CompletedProcess[bytes]] = []

    def invoke(arguments: list[str]) -> subprocess.CompletedProcess[bytes]:
        calls.append(arguments)
        result = subprocess.run(  # noqa: S603 — the real numeric binary, no shell
            [str(numeric_binary), *arguments],
            cwd=tmp_path,
            capture_output=True,
            check=False,
        )
        sys.stderr.write(result.stderr.decode())
        results.append(result)
        return result

    workflow(options, invoke)
    assert [arguments[:2] for arguments in calls[:2]] == [["table", "query"], ["table", "build"]]
    assert options.output.read_bytes().startswith(PNG_MAGIC)
    first = json.loads(options.document.read_bytes())
    assert first["provenance"]["dataset"] == "tiny"
    assert options.document.read_bytes() == results[-1].stdout
    assert first["result"]["circle"]["target"]["total"] == 28.0  # noqa: PLR2004 — generated cells sum

    (tmp_path / "data/tiny.tif").unlink()
    calls.clear()
    results.clear()
    options.output.unlink()
    workflow(options, invoke)
    assert len(calls) == 2  # noqa: PLR2004 — checked query and search, with no build
    assert calls[0][:2] == ["table", "query"]
    assert calls[1][0] == "smallest-for-share"
    assert options.output.read_bytes().startswith(PNG_MAGIC)
    current = options.document.read_bytes()
    assert (
        json.loads(current)["provenance"]["digest"]
        == json.loads(results[0].stdout)["result"]["digest"]
    )
    assert current == results[-1].stdout

    for cache_file in tmp_path.glob("table.*"):
        cache_file.unlink()
    calls.clear()
    options.output.unlink()
    with pytest.raises(WorkflowError, match="table build: Rust CLI exited 3"):
        workflow(options, invoke)
    assert "mise run data:get" in capsys.readouterr().err
    assert options.document.read_bytes() == current
    assert not options.output.exists()
