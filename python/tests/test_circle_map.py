import json
import subprocess
import sys
from dataclasses import replace
from pathlib import Path
from typing import Literal

import pytest
from matplotlib import pyplot as plt
from matplotlib.figure import Figure
from shapely.geometry.base import BaseGeometry

from population_circles import circle_map, render_map
from population_circles.circle_document import Answer
from population_circles.circle_map import Options, WorkflowError, parse, workflow
from population_circles.dataset_registry import load
from population_circles.map_frame import ORTHOGRAPHIC
from test_render_map import DATASET, PNG_MAGIC, document


@pytest.fixture
def options(tmp_path: Path) -> Options:
    return parse(
        [
            "--dataset",
            DATASET,
            "--share",
            "50",
            "--cache",
            str(tmp_path / "cache"),
            "--output",
            str(tmp_path / "figure.png"),
            "--no-coastlines",
        ]
    )


class Children:
    def __init__(self, codes: list[int], stdout: bytes | None = None) -> None:
        self.codes = iter(codes)
        self.stdout = json.dumps(document()).encode() if stdout is None else stdout
        self.calls: list[list[str]] = []

    def __call__(self, arguments: list[str]) -> subprocess.CompletedProcess[bytes]:
        self.calls.append(arguments)
        return subprocess.CompletedProcess(arguments, next(self.codes), self.stdout)


def test_reuse_forwards_every_parameter_and_draws_this_document(options: Options) -> None:
    options = replace(options, decimate="10", spacing="4", projection=ORTHOGRAPHIC)
    children = Children([0, 0])
    workflow(options, children)
    common = ["--dataset", DATASET, "--cache", str(options.cache), "--decimate", "10"]
    assert children.calls == [
        ["table", "query", *common],
        [
            "smallest-for-share",
            *common,
            "--share",
            "50",
            "--ledger",
            str(options.ledger),
            "--spacing",
            "4",
        ],
    ]
    assert options.document.read_bytes() == children.stdout
    assert options.output.read_bytes().startswith(PNG_MAGIC)


@pytest.mark.parametrize("code", [1, 2, 4, -9])
def test_a_failed_probe_never_builds_or_renders(options: Options, code: int) -> None:
    options.document.write_text("older document")
    children = Children([code])
    with pytest.raises(WorkflowError, match="cache probe: Rust CLI exited"):
        workflow(options, children)
    assert len(children.calls) == 1
    assert options.document.read_text() == "older document"
    assert not options.output.exists()


def test_only_a_missing_or_incompatible_probe_builds(options: Options) -> None:
    children = Children([3, 0, 0])
    workflow(options, children)
    common = ["--dataset", DATASET, "--cache", str(options.cache), "--decimate", "1"]
    assert children.calls == [
        ["table", "query", *common],
        ["table", "build", *common],
        ["smallest-for-share", *common, "--share", "50", "--ledger", str(options.ledger)],
    ]
    assert options.output.read_bytes().startswith(PNG_MAGIC)


@pytest.mark.parametrize(
    ("codes", "stage"), [([3, 3], "table build"), ([0, 3], "search"), ([3, 0, 1], "search")]
)
def test_failed_build_or_search_keeps_the_previous_document(
    options: Options,
    codes: list[int],
    stage: str,
) -> None:
    options.document.write_text("older document")
    children = Children(codes)
    with pytest.raises(WorkflowError, match=f"{stage}: Rust CLI exited"):
        workflow(options, children)
    assert len(children.calls) == len(codes)
    assert options.document.read_text() == "older document"
    assert not options.output.exists()


@pytest.mark.parametrize("payload", [b"", b"{", b"[]", b'{"schema_version":99}'])
def test_rejected_documents_are_retained_and_never_drawn(options: Options, payload: bytes) -> None:
    options.document.write_text("older document")
    with pytest.raises(WorkflowError, match=r"render:.*") as caught:
        workflow(options, Children([0, 0], payload))
    assert str(options.document) in str(caught.value)
    assert options.document.read_bytes() == payload
    assert not options.output.exists()


@pytest.mark.parametrize("override", [False, True])
def test_a_render_failure_keeps_this_runs_exact_stdout(options: Options, *, override: bool) -> None:
    if override:
        options = replace(options, document=options.document.parent / "nested" / "answer.json")
    children = Children([0, 0])
    with pytest.raises(
        WorkflowError, match=r"unsupported projection.*current search document retained"
    ) as caught:
        workflow(replace(options, projection="unknown"), children)
    assert str(options.document) in str(caught.value)
    assert options.document.read_bytes() == children.stdout
    assert json.loads(options.document.read_bytes())["provenance"]["dataset"] == DATASET


def test_save_failure_closes_the_figure_and_retains_the_document(
    options: Options,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    before = plt.get_fignums()

    def fail(_self: Figure, *_args: object, **_kwargs: object) -> None:
        message = "cannot save figure"
        raise OSError(message)

    monkeypatch.setattr(Figure, "savefig", fail)
    children = Children([0, 0])
    with pytest.raises(WorkflowError, match="render: cannot save figure"):
        workflow(options, children)
    assert options.document.read_bytes() == children.stdout
    assert plt.get_fignums() == before


def test_draw_failure_closes_the_figure(options: Options, monkeypatch: pytest.MonkeyPatch) -> None:
    before = plt.get_fignums()

    def fail(*_args: object) -> None:
        message = "cannot draw geometry"
        raise ValueError(message)

    monkeypatch.setattr(render_map, "draw", fail)
    with pytest.raises(WorkflowError, match="render: cannot draw geometry"):
        workflow(options, Children([0, 0]))
    assert plt.get_fignums() == before


@pytest.mark.parametrize(
    "collision", ["output-document", "output-ledger", "document-ledger", "hardlink"]
)
def test_colliding_destinations_are_refused_before_children(
    options: Options,
    collision: Literal["output-document", "output-ledger", "document-ledger", "hardlink"],
) -> None:
    match collision:
        case "output-document":
            options = replace(options, document=options.output.parent / "." / options.output.name)
        case "output-ledger":
            options = replace(options, ledger=options.output)
        case "document-ledger":
            options = replace(options, ledger=options.document)
        case "hardlink":
            options.output.touch()
            options.document.hardlink_to(options.output)
    children = Children([])
    with pytest.raises(WorkflowError, match="destinations: image, document and ledger must differ"):
        workflow(options, children)
    assert children.calls == []


def test_publication_failure_preserves_the_existing_document(
    options: Options,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    options.document.write_text("older document")

    def fail(_self: Path, _target: Path) -> Path:
        message = "replace denied"
        raise PermissionError(message)

    monkeypatch.setattr(Path, "replace", fail)
    with pytest.raises(WorkflowError, match=r"document publication:.*replace denied"):
        workflow(options, Children([0, 0]))
    assert options.document.read_text() == "older document"
    assert not options.output.exists()
    assert list(options.document.parent.glob(f".{options.document.name}.*")) == []


def test_overrides_determine_both_document_and_default_ledger(tmp_path: Path) -> None:
    base = ["--dataset", DATASET, "--share", "50", "--cache", "cache", "--output", "map.png"]
    chosen = tmp_path / "other.json"
    options = parse([*base, "--document", str(chosen)])
    assert options.document == chosen
    assert options.ledger == chosen.with_suffix(".radii.json")
    assert parse([*base, "--ledger", "custom.json"]).ledger == Path("custom.json")


def test_missing_executable_reports_the_launch_stage(options: Options) -> None:
    def missing(_arguments: list[str]) -> subprocess.CompletedProcess[bytes]:
        message = "missing mise executable"
        raise FileNotFoundError(message)

    with pytest.raises(WorkflowError, match="cache probe: could not launch Rust CLI: missing mise"):
        workflow(options, missing)


def test_actual_child_uses_checkout_argv_and_inherits_stderr(
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
        circle_map.invoke_cli(["table", "query", "--dataset", "literal;$(value)"]).stdout
        == b"answer"
    )
    assert captured == [
        (
            ["mise", "run", "cli", "--", "table", "query", "--dataset", "literal;$(value)"],
            circle_map.REPO_ROOT,
            subprocess.PIPE,
        )
    ]


def test_main_reports_child_diagnostic_without_masking_it(
    options: Options,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    def missing(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[bytes]:
        if "build" in arguments:
            sys.stderr.write("missing raster; run mise run data:get\n")
        return subprocess.CompletedProcess(arguments, 3, b"")

    monkeypatch.setattr(subprocess, "run", missing)
    assert (
        circle_map.main(
            [
                "--dataset",
                DATASET,
                "--share",
                "50",
                "--cache",
                str(options.cache),
                "--output",
                str(options.output),
            ]
        )
        == 1
    )
    diagnostic = capsys.readouterr().err
    assert "missing raster; run mise run data:get" in diagnostic
    assert "table build: Rust CLI exited 3" in diagnostic


def test_success_uses_the_documents_credit_and_earth_model(
    options: Options,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    seen: list[tuple[float, str]] = []
    original = render_map.render

    def capture(  # noqa: PLR0913 — mirror the renderer call to assert its earth model
        answer: Answer,
        projection: str,
        attribution: str,
        *,
        coastline: BaseGeometry | None,
        coastline_attribution: str | None = None,
        outline_and_credit: tuple[BaseGeometry, str] | None = None,
    ) -> Figure:
        seen.append((answer.circle.earth_radius_km, attribution))
        return original(
            answer,
            projection,
            attribution,
            coastline=coastline,
            coastline_attribution=coastline_attribution,
            outline_and_credit=outline_and_credit,
        )

    monkeypatch.setattr(render_map, "render", capture)
    payload = document()
    payload["earth_model"]["radius_km"] = 7000.0
    workflow(options, Children([0, 0], json.dumps(payload).encode()))
    assert seen == [(7000.0, render_map.citation(load(), DATASET))]
