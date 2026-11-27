import json
import subprocess
from pathlib import Path
from typing import Literal

import pytest
from shapely.geometry import box
from shapely.geometry.base import BaseGeometry

from population_circles.application.errors import PopulationApplicationError
from population_circles.application.ports import PopulationRunner
from population_circles.application.use_cases.circle_result import (
    CircleResultCollaborators,
    CircleResultRequest,
    search_and_publish_circle_result,
)
from population_circles.documents.result_document import Answer
from population_circles.infrastructure import popcircles_cli
from population_circles.infrastructure.file_publishing import write_bytes_atomically
from test_circle_document import DATASET, SEARCHED_SHARE, envelope, searched


@pytest.fixture
def result_request(tmp_path: Path) -> CircleResultRequest:
    result = tmp_path / "half-world.json"
    return CircleResultRequest(
        dataset=DATASET,
        share="50",
        cache=tmp_path / "cache",
        document=result,
        ledger=result.with_suffix(".radii.json"),
    )


class Children:
    def __init__(self, codes: list[int], stdout: bytes | None = None) -> None:
        self.codes = iter(codes)
        self.stdout = (
            json.dumps(envelope("smallest", searched())).encode() if stdout is None else stdout
        )
        self.calls: list[list[str]] = []

    def __call__(
        self,
        stage: str,
        arguments: list[str],
        *,
        accepted_exit_codes: tuple[int, ...] = (0,),
    ) -> subprocess.CompletedProcess[bytes]:
        self.calls.append(arguments)
        result = subprocess.CompletedProcess(arguments, next(self.codes), self.stdout)
        if result.returncode not in accepted_exit_codes:
            message = f"{stage}: Rust CLI exited {result.returncode}; see child diagnostic above"
            raise PopulationApplicationError(message)
        return result


def boundaries(_answer: Answer) -> dict[str, BaseGeometry]:
    return {}


def non_intersecting_boundaries(_answer: Answer) -> dict[str, BaseGeometry]:
    return {"fixture": box(79, 24, 81, 26)}


def collaborators(runner: PopulationRunner) -> CircleResultCollaborators:
    return CircleResultCollaborators(runner, boundaries, write_bytes_atomically)


def run_circle_result(
    request: CircleResultRequest,
    runner: PopulationRunner,
) -> None:
    search_and_publish_circle_result(request, collaborators(runner))


def test_search_publishes_result_and_forwards_every_parameter(
    result_request: CircleResultRequest,
) -> None:
    request = CircleResultRequest(
        result_request.dataset,
        result_request.share,
        result_request.cache,
        result_request.document,
        result_request.ledger,
        decimate="10",
        spacing="4",
    )
    children = Children([3, 0, 0])
    search_and_publish_circle_result(
        request,
        CircleResultCollaborators(children, non_intersecting_boundaries, write_bytes_atomically),
    )
    common = ["--dataset", DATASET, "--cache", str(request.cache), "--decimate", "10"]
    assert children.calls == [
        ["table", "query", *common],
        ["table", "build", *common],
        [
            "smallest-for-share",
            *common,
            "--share",
            "50",
            "--ledger",
            str(request.ledger),
            "--spacing",
            "4",
        ],
    ]
    published = json.loads(request.document.read_bytes())
    assert published["document"] == "smallest"
    assert published["result"]["circle"]["share_achieved"] == SEARCHED_SHARE
    assert published["result"]["circle"]["territory_share"] == 0.0


@pytest.mark.parametrize("code", [1, 2, 4, -9])
def test_a_failed_probe_preserves_the_existing_result(
    result_request: CircleResultRequest, code: int
) -> None:
    result_request.document.write_text("older result")
    children = Children([code])
    with pytest.raises(PopulationApplicationError, match="cache probe: Rust CLI exited"):
        run_circle_result(result_request, children)
    assert len(children.calls) == 1
    assert result_request.document.read_text() == "older result"


def test_only_a_missing_or_incompatible_probe_builds(result_request: CircleResultRequest) -> None:
    children = Children([3, 0, 0])
    run_circle_result(result_request, children)
    common = [
        "--dataset",
        DATASET,
        "--cache",
        str(result_request.cache),
        "--decimate",
        "1",
    ]
    assert children.calls == [
        ["table", "query", *common],
        ["table", "build", *common],
        [
            "smallest-for-share",
            *common,
            "--share",
            "50",
            "--ledger",
            str(result_request.ledger),
        ],
    ]
    assert json.loads(result_request.document.read_bytes())["document"] == "smallest"


@pytest.mark.parametrize(
    ("codes", "stage"), [([3, 3], "table build"), ([0, 3], "search"), ([3, 0, 1], "search")]
)
def test_failed_build_or_search_keeps_the_previous_result(
    result_request: CircleResultRequest,
    codes: list[int],
    stage: str,
) -> None:
    result_request.document.write_text("older result")
    children = Children(codes)
    with pytest.raises(PopulationApplicationError, match=f"{stage}: Rust CLI exited"):
        run_circle_result(result_request, children)
    assert len(children.calls) == len(codes)
    assert result_request.document.read_text() == "older result"


@pytest.mark.parametrize("payload", [b"", b"{", b"[]", b'{"schema_version":99}'])
def test_invalid_search_results_do_not_replace_the_existing_file(
    result_request: CircleResultRequest, payload: bytes
) -> None:
    result_request.document.write_text("older result")
    with pytest.raises(PopulationApplicationError, match="invalid circle result"):
        run_circle_result(result_request, Children([0, 0], payload))
    assert result_request.document.read_text() == "older result"


@pytest.mark.parametrize("collision", ["same", "alias", "hardlink"])
def test_colliding_result_and_ledger_paths_are_refused_before_search(
    result_request: CircleResultRequest,
    collision: Literal["same", "alias", "hardlink"],
) -> None:
    match collision:
        case "same":
            request = CircleResultRequest(
                result_request.dataset,
                result_request.share,
                result_request.cache,
                result_request.document,
                result_request.document,
            )
        case "alias":
            request = CircleResultRequest(
                result_request.dataset,
                result_request.share,
                result_request.cache,
                result_request.document,
                result_request.document.parent / "." / result_request.document.name,
            )
        case "hardlink":
            result_request.document.touch()
            request = CircleResultRequest(
                result_request.dataset,
                result_request.share,
                result_request.cache,
                result_request.document,
                result_request.document.with_suffix(".radii.json"),
            )
            request.ledger.hardlink_to(request.document)
    children = Children([])
    with pytest.raises(
        PopulationApplicationError,
        match="result document and radii ledger must differ",
    ):
        run_circle_result(request, children)
    assert children.calls == []


def test_publication_failure_preserves_the_existing_result(
    result_request: CircleResultRequest,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    result_request.document.write_text("older result")

    def fail(_self: Path, _target: Path) -> Path:
        message = "replace denied"
        raise PermissionError(message)

    monkeypatch.setattr(Path, "replace", fail)
    with pytest.raises(PopulationApplicationError, match=r"result publication:.*replace denied"):
        run_circle_result(result_request, Children([0, 0]))
    assert result_request.document.read_text() == "older result"
    assert list(result_request.document.parent.glob(f".{result_request.document.name}.*")) == []


def test_invoke_popcircles_cli_runs_from_checkout_and_inherits_stderr(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    captured: list[tuple[list[str], Path, int]] = []

    def capture(
        arguments: list[str], *, cwd: Path, stdout: int, check: bool
    ) -> subprocess.CompletedProcess[bytes]:
        assert check is False
        captured.append((arguments, cwd, stdout))
        return subprocess.CompletedProcess(arguments, 0, b"answer")

    monkeypatch.setattr(subprocess, "run", capture)
    assert (
        popcircles_cli.invoke_popcircles_cli(
            "table query", ["table", "query", "--dataset", "literal;$(value)"]
        ).stdout
        == b"answer"
    )
    assert captured == [
        (
            [
                "mise",
                "run",
                "popcircles-cli",
                "--",
                "table",
                "query",
                "--dataset",
                "literal;$(value)",
            ],
            popcircles_cli.REPO_ROOT,
            subprocess.PIPE,
        )
    ]


def test_invoke_popcircles_cli_rejects_unaccepted_return_codes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def fail(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[bytes]:
        return subprocess.CompletedProcess(arguments, 7, b"diagnostic")

    monkeypatch.setattr(subprocess, "run", fail)
    with pytest.raises(PopulationApplicationError, match="table build: Rust CLI exited 7"):
        popcircles_cli.invoke_popcircles_cli("table build", ["table", "build"])
