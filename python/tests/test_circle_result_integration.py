import json
import subprocess
import sys
from pathlib import Path
from typing import Any

import pytest

from population_circles.application.errors import PopulationApplicationError
from population_circles.application.ports import PopulationRunner
from population_circles.application.use_cases.circle_result import (
    CircleResultCollaborators,
    CircleResultRequest,
    search_and_publish_circle_result,
)
from population_circles.documents.result_document import SweepRecords, circle_of
from population_circles.infrastructure.file_publishing import write_bytes_atomically
from population_circles.infrastructure.popcircles_cli import REPO_ROOT


def run_circle_result(request: CircleResultRequest, runner: PopulationRunner) -> None:
    search_and_publish_circle_result(
        request,
        CircleResultCollaborators(runner, lambda _answer: {}, write_bytes_atomically),
    )


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


@pytest.mark.xdist_group(name="numeric-binary")
def test_real_build_reuse_and_missing_raster(
    tmp_path: Path,
    numeric_binary: Path,
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
    request = CircleResultRequest(
        dataset="tiny",
        share="50",
        cache=tmp_path / "table",
        document=tmp_path / "map.json",
        ledger=tmp_path / "map.radii.json",
    )
    calls: list[list[str]] = []
    results: list[subprocess.CompletedProcess[bytes]] = []

    def invoke(
        stage: str,
        arguments: list[str],
        *,
        accepted_exit_codes: tuple[int, ...] = (0,),
    ) -> subprocess.CompletedProcess[bytes]:
        calls.append(arguments)
        result = subprocess.run(  # noqa: S603 — the real numeric binary, no shell
            [str(numeric_binary), *arguments],
            cwd=tmp_path,
            capture_output=True,
            check=False,
        )
        sys.stderr.write(result.stderr.decode())
        if result.returncode not in accepted_exit_codes:
            message = f"{stage}: Rust CLI exited {result.returncode}; see child diagnostic above"
            raise PopulationApplicationError(message)
        results.append(result)
        return result

    run_circle_result(request, invoke)
    assert [arguments[:2] for arguments in calls[:2]] == [["table", "query"], ["table", "build"]]
    first = json.loads(request.document.read_bytes())
    assert first["provenance"]["dataset"] == "tiny"
    assert first["result"] == json.loads(results[-1].stdout)["result"]
    assert first["result"]["circle"]["target"]["total"] == 28.0  # noqa: PLR2004 — generated cells sum

    (tmp_path / "data/tiny.tif").unlink()
    calls.clear()
    results.clear()
    with pytest.raises(PopulationApplicationError, match="search: Rust CLI exited 3"):
        run_circle_result(request, invoke)
    assert len(calls) == 2  # noqa: PLR2004 — cached table query then raster-dependent circle
    assert calls[0][:2] == ["table", "query"]
    assert calls[1][0] == "smallest-for-share"
    current = request.document.read_bytes()
    assert "mise run data:get" in capsys.readouterr().err

    for cache_file in tmp_path.glob("table.*"):
        cache_file.unlink()
    calls.clear()
    with pytest.raises(PopulationApplicationError, match="table build: Rust CLI exited 3"):
        run_circle_result(request, invoke)
    assert "mise run data:get" in capsys.readouterr().err
    assert request.document.read_bytes() == current


@pytest.mark.xdist_group(name="numeric-binary")
def test_rust_result_documents_cross_the_python_boundary_with_optional_settlements(
    tmp_path: Path,
    numeric_binary: Path,
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
    places = [
        {
            "type": "Feature",
            "properties": {"NAME": f"Place {lat:g} {lon:g}", "ADM0NAME": "Fixture"},
            "geometry": {"type": "Point", "coordinates": [lon, lat]},
        }
        for lat in (45.0, -45.0)
        for lon in (-135.0, -45.0, 45.0, 135.0)
    ]
    (tmp_path / "data/settlements/populated-places-1to10m.geojson").write_text(
        json.dumps({"type": "FeatureCollection", "features": places}),
        encoding="utf-8",
    )
    cache = tmp_path / "table"
    common = ["--dataset", "tiny", "--cache", str(cache), "--decimate", "1"]
    run([str(numeric_binary), "table", "build", *common], tmp_path)

    documents: dict[str, dict[str, Any]] = {}
    for kind, command in (
        (
            "circle",
            ["population-at", *common, "--lat", "48", "--lon", "11", "--radius-km", "1200"],
        ),
        (
            "most-populous",
            ["most-populous", *common, "--radius-km", "3000", "--spacing", "4"],
        ),
        (
            "smallest",
            [
                "smallest-for-share",
                *common,
                "--share",
                "25",
                "--spacing",
                "4",
                "--ledger",
                str(tmp_path / "smallest.radii.json"),
            ],
        ),
    ):
        documents[kind] = json.loads(run([str(numeric_binary), *command], tmp_path).stdout)

    for kind, document in documents.items():
        answer = circle_of(document)
        assert answer.caption.country_breakdown is not None
        assert (
            answer.caption.country_breakdown.provenance.boundary_dataset
            == "gisco-countries-2024-1m"
        )
        assert answer.caption.settlement is not None
        assert answer.caption.settlement.name == (
            f"Place {answer.circle.centre.lat:g} {answer.circle.centre.lon:g}"
        )
        assert answer.caption.settlement.country == "Fixture"
        assert answer.caption.settlement.short_form is True
        assert answer.caption.settlement.distance_km == 0

        legacy = json.loads(json.dumps(document))
        circle_payload = legacy["result"]
        if kind == "smallest":
            circle_payload = circle_payload["circle"]
        del circle_payload["nearest_settlement"]
        del circle_payload["country_breakdown"]
        legacy_answer = circle_of(legacy)
        assert legacy_answer.caption.settlement is None
        assert legacy_answer.circle == answer.circle
        assert legacy_answer.caption.population == answer.caption.population
        assert legacy_answer.caption.share == answer.caption.share

    sweep = json.loads(
        run(
            [
                str(numeric_binary),
                "sweep",
                *common,
                "--from",
                "25",
                "--to",
                "50",
                "--step",
                "25",
                "--spacing",
                "4",
                "--ledger",
                str(tmp_path / "sweep.radii.json"),
            ],
            tmp_path,
        ).stdout
    )
    sweep_records = SweepRecords.model_validate(sweep["result"]).records
    assert len(sweep_records) == 2  # noqa: PLR2004 — 25% and 50% fixture sweep
    assert all(record.country_breakdown is not None for record in sweep_records)
    assert all(
        record["nearest_settlement"]["distance_km"] == 0 for record in sweep["result"]["records"]
    )
