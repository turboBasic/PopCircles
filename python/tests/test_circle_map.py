import json
import subprocess
import sys
from dataclasses import replace
from pathlib import Path
from typing import Literal
from unittest.mock import Mock

import pytest
from matplotlib import pyplot as plt
from matplotlib.figure import Figure
from shapely.geometry import box
from shapely.geometry.base import BaseGeometry
from typer.testing import CliRunner

import population_circles.application.use_cases.render_map as render_map_use_case
from population_circles.application.errors import PopulationApplicationError
from population_circles.application.ports import PopulationRunner
from population_circles.application.use_cases.circle_map import (
    CircleMapRequest,
    search_and_render_circle_map,
)
from population_circles.documents.result_document import Answer
from population_circles.infrastructure import popcircles_cli
from population_circles.infrastructure.file_publishing import write_bytes_atomically
from population_circles.infrastructure.geodata import map_layers
from population_circles.infrastructure.geodata.dataset_registry import Registry, load
from population_circles.presentation import cli
from population_circles.presentation.maps import metadata as map_metadata
from population_circles.presentation.maps import render_map
from population_circles.presentation.maps.frame import ORTHOGRAPHIC
from test_render_map import (
    DATASET,
    PNG_MAGIC,
    document,
    empty_boundaries,
    fixture_map_layers,
    fixture_registered_map_sources,
    transparent_terrain,
)

CUSTOM_EARTH_RADIUS_KM = 7000.0


@pytest.fixture(autouse=True)
def use_empty_boundary_fixtures(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(cli, "load_map_boundaries", empty_boundaries)
    monkeypatch.setattr(map_layers, "registered_map_sources", fixture_registered_map_sources)
    monkeypatch.setattr(map_layers, "load_map_layers", fixture_map_layers)
    monkeypatch.setattr(render_map, "prepare_terrain", transparent_terrain)


@pytest.fixture
def options(tmp_path: Path) -> CircleMapRequest:
    output = tmp_path / "figure.png"
    document_path = output.with_suffix(".json")
    return CircleMapRequest(
        dataset=DATASET,
        share="50",
        cache=tmp_path / "cache",
        output=output,
        document=document_path,
        ledger=document_path.with_suffix(".radii.json"),
        no_coastlines=True,
    )


def intersecting_boundaries(_answer: Answer, _registry: Registry) -> dict[str, BaseGeometry]:
    return {"fixture": box(79, 24, 81, 26)}


class Children:
    def __init__(self, codes: list[int], stdout: bytes | None = None) -> None:
        self.codes = iter(codes)
        self.stdout = json.dumps(document()).encode() if stdout is None else stdout
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


def boundary_loader(_answer: Answer) -> dict[str, BaseGeometry]:
    return {}


def intersecting_boundary_loader(answer: Answer) -> dict[str, BaseGeometry]:
    return intersecting_boundaries(answer, load())


def run_circle_map(
    options: CircleMapRequest,
    runner: PopulationRunner = popcircles_cli.invoke_popcircles_cli,
) -> None:
    search_and_render_circle_map(
        options,
        runner,
        load_boundaries=boundary_loader,
        publish_file=write_bytes_atomically,
        render_file=cli.render_map_file,
    )


def test_reuse_forwards_every_parameter_and_draws_this_document(options: CircleMapRequest) -> None:
    options = replace(options, decimate="10", spacing="4", projection=ORTHOGRAPHIC)
    children = Children([0, 0])
    run_circle_map(options, children)
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
    published = json.loads(options.document.read_bytes())
    original = json.loads(children.stdout)
    assert published["result"] == original["result"]
    assert published["render_metadata"]["Description"] == (
        f"Answered from population dataset {DATASET} at decimation k=10"
    )
    assert options.output.read_bytes().startswith(PNG_MAGIC)


@pytest.mark.parametrize("code", [1, 2, 4, -9])
def test_a_failed_probe_never_builds_or_renders(options: CircleMapRequest, code: int) -> None:
    options.document.write_text("older document")
    children = Children([code])
    with pytest.raises(PopulationApplicationError, match="cache probe: Rust CLI exited"):
        run_circle_map(options, children)
    assert len(children.calls) == 1
    assert options.document.read_text() == "older document"
    assert not options.output.exists()


def test_only_a_missing_or_incompatible_probe_builds(options: CircleMapRequest) -> None:
    children = Children([3, 0, 0])
    run_circle_map(options, children)
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
    options: CircleMapRequest,
    codes: list[int],
    stage: str,
) -> None:
    options.document.write_text("older document")
    children = Children(codes)
    with pytest.raises(PopulationApplicationError, match=f"{stage}: Rust CLI exited"):
        run_circle_map(options, children)
    assert len(children.calls) == len(codes)
    assert options.document.read_text() == "older document"
    assert not options.output.exists()


@pytest.mark.parametrize("payload", [b"", b"{", b"[]", b'{"schema_version":99}'])
def test_rejected_documents_are_retained_and_never_drawn(
    options: CircleMapRequest, payload: bytes
) -> None:
    options.document.write_text("older document")
    with pytest.raises(PopulationApplicationError, match=r"render:.*") as caught:
        run_circle_map(options, Children([0, 0], payload))
    assert str(options.document) in str(caught.value)
    assert options.document.read_bytes() == payload
    assert not options.output.exists()


@pytest.mark.parametrize("override", [False, True])
def test_a_render_failure_keeps_this_runs_enriched_document(
    options: CircleMapRequest, monkeypatch: pytest.MonkeyPatch, *, override: bool
) -> None:
    if override:
        options = replace(options, document=options.document.parent / "nested" / "answer.json")

    monkeypatch.setattr(
        sys.modules[__name__],
        "boundary_loader",
        intersecting_boundary_loader,
    )
    monkeypatch.setattr(
        render_map_use_case,
        "enrich_document",
        Mock(side_effect=AssertionError("pre-enriched document should not be recalculated")),
    )
    children = Children([0, 0])
    with pytest.raises(
        PopulationApplicationError,
        match=r"unsupported projection.*current search document retained",
    ) as caught:
        run_circle_map(replace(options, projection="unknown"), children)
    assert str(options.document) in str(caught.value)
    published = json.loads(options.document.read_bytes())
    assert published["provenance"]["dataset"] == DATASET
    assert published["result"]["territory_share"] > 0.0


def test_save_failure_closes_the_figure_and_retains_the_document(
    options: CircleMapRequest,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    before = plt.get_fignums()

    def fail(_self: Figure, *_args: object, **_kwargs: object) -> None:
        message = "cannot save figure"
        raise OSError(message)

    monkeypatch.setattr(Figure, "savefig", fail)
    monkeypatch.setattr(
        sys.modules[__name__],
        "boundary_loader",
        intersecting_boundary_loader,
    )
    children = Children([0, 0])
    with pytest.raises(PopulationApplicationError, match="render: cannot save figure"):
        run_circle_map(options, children)
    assert "territory_share" in json.loads(options.document.read_bytes())["result"]
    assert plt.get_fignums() == before


def test_draw_failure_closes_the_figure(
    options: CircleMapRequest, monkeypatch: pytest.MonkeyPatch
) -> None:
    before = plt.get_fignums()

    def fail(*_args: object) -> None:
        message = "cannot draw geometry"
        raise ValueError(message)

    monkeypatch.setattr(render_map, "draw", fail)
    with pytest.raises(PopulationApplicationError, match="render: cannot draw geometry"):
        run_circle_map(options, Children([0, 0]))
    assert plt.get_fignums() == before


@pytest.mark.parametrize(
    "collision", ["output-document", "output-ledger", "document-ledger", "hardlink"]
)
def test_colliding_destinations_are_refused_before_children(
    options: CircleMapRequest,
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
    with pytest.raises(
        PopulationApplicationError,
        match="destinations: image, document and ledger must differ",
    ):
        run_circle_map(options, children)
    assert children.calls == []


def test_publication_failure_preserves_the_existing_document(
    options: CircleMapRequest,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    options.document.write_text("older document")

    def fail(_self: Path, _target: Path) -> Path:
        message = "replace denied"
        raise PermissionError(message)

    monkeypatch.setattr(Path, "replace", fail)
    with pytest.raises(PopulationApplicationError, match=r"document publication:.*replace denied"):
        run_circle_map(options, Children([0, 0]))
    assert options.document.read_text() == "older document"
    assert not options.output.exists()
    assert list(options.document.parent.glob(f".{options.document.name}.*")) == []


def test_missing_executable_reports_the_launch_stage(
    options: CircleMapRequest, monkeypatch: pytest.MonkeyPatch
) -> None:
    def missing(*_args: object, **_kwargs: object) -> subprocess.CompletedProcess[bytes]:
        message = "missing mise executable"
        raise FileNotFoundError(message)

    monkeypatch.setattr(subprocess, "run", missing)
    with pytest.raises(
        PopulationApplicationError,
        match="cache probe: could not launch Rust CLI: missing mise",
    ):
        run_circle_map(options)


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


def test_main_reports_child_diagnostic_without_masking_it(
    options: CircleMapRequest,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def missing(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[bytes]:
        if "build" in arguments:
            sys.stderr.write("missing raster; run mise run data:get\n")
        return subprocess.CompletedProcess(arguments, 3, b"")

    monkeypatch.setattr(subprocess, "run", missing)
    result = CliRunner().invoke(
        cli.app,
        [
            "circle",
            "--dataset",
            DATASET,
            "--share",
            "50",
            "--cache",
            str(options.cache),
            "--output",
            str(options.output),
        ],
    )
    assert result.exit_code == 1
    assert "missing raster; run mise run data:get" in result.output
    assert "table build: Rust CLI exited 3" in result.output


def test_success_uses_the_documents_credit_and_earth_model(
    options: CircleMapRequest,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    seen_credits: list[str] = []
    original = render_map.render
    original_copyright = map_metadata.copyright_of
    render_spy = Mock(wraps=original)

    def capture_copyright(
        answer: Answer,
        attribution: str,
        coastline_attribution: str | None,
        boundary_attribution: str | None,
        map_attributions: tuple[tuple[str, str], ...] = (),
    ) -> str:
        seen_credits.append(attribution)
        return original_copyright(
            answer,
            attribution,
            coastline_attribution,
            boundary_attribution,
            map_attributions,
        )

    monkeypatch.setattr(render_map, "render", render_spy)
    monkeypatch.setattr(map_metadata, "copyright_of", capture_copyright)
    payload = document()
    payload["earth_model"]["radius_km"] = CUSTOM_EARTH_RADIUS_KM
    run_circle_map(options, Children([0, 0], json.dumps(payload).encode()))
    assert render_spy.call_args is not None
    render_answer = render_spy.call_args.args[0]
    assert isinstance(render_answer, Answer)
    assert render_answer.circle.earth_radius_km == CUSTOM_EARTH_RADIUS_KM
    assert seen_credits == [map_layers.citation(load(), DATASET)]
