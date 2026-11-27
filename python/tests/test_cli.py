import subprocess
import sys
from pathlib import Path
from unittest.mock import Mock

from _pytest.monkeypatch import MonkeyPatch
from typer.testing import CliRunner

from population_circles import composition
from population_circles.application.errors import PopulationApplicationError
from population_circles.application.use_cases import circle_result, gallery_build, render_map
from population_circles.application.use_cases.circle_result import CircleResultRequest
from population_circles.application.use_cases.render_map import RenderMapFileRequest
from population_circles.presentation import cli
from population_circles.presentation.maps import render_globe

runner = CliRunner()
INVALID_ARGUMENT_EXIT = 2


def test_help_lists_commands() -> None:
    result = runner.invoke(cli.app, ["--help"])

    assert result.exit_code == 0
    assert "circle" in result.stdout
    assert "map" in result.stdout
    assert "globe-animation" in result.stdout
    assert "animated globe as SVG or GIF" in result.stdout
    assert "gallery" in result.stdout


def test_gallery_rejects_identical_paths_with_an_actionable_cli_error(
    tmp_path: Path, monkeypatch: MonkeyPatch
) -> None:
    def unused_collaborators() -> Mock:
        return Mock()

    monkeypatch.setattr(composition, "gallery_build_collaborators", unused_collaborators)
    path = tmp_path / "gallery"

    result = runner.invoke(
        cli.app,
        ["gallery", "--input", str(path), "--output", str(path)],
    )

    assert result.exit_code == INVALID_ARGUMENT_EXIT
    assert "non-overlapping" in result.output
    assert "directories" in result.output
    assert not path.exists()


def test_gallery_preserves_application_error_reporting_for_workflow_failures(
    tmp_path: Path, monkeypatch: MonkeyPatch
) -> None:
    error = PopulationApplicationError("country workflow failed")

    def fail_build(*_args: object, **_kwargs: object) -> None:
        raise error

    monkeypatch.setattr(gallery_build, "build", fail_build)

    result = runner.invoke(
        cli.app,
        ["gallery", "--input", str(tmp_path / "corpus"), "--output", str(tmp_path / "gallery")],
    )

    assert result.exit_code == 1
    assert result.exception is error


def test_circle_requires_dataset() -> None:
    result = runner.invoke(
        cli.app,
        ["circle", "--share", "50", "--cache", "cache", "--output", "map.png"],
    )

    assert result.exit_code == INVALID_ARGUMENT_EXIT


def test_circle_rejects_render_options() -> None:
    result = runner.invoke(
        cli.app,
        [
            "circle",
            "--dataset",
            "world",
            "--share",
            "50",
            "--cache",
            "cache",
            "--output",
            "result.json",
            "--projection",
            "invalid",
        ],
    )

    assert result.exit_code == INVALID_ARGUMENT_EXIT


def test_circle_uses_output_as_result_and_defaults_ledger(monkeypatch: MonkeyPatch) -> None:
    workflow = Mock()
    collaborators = Mock()
    monkeypatch.setattr(circle_result, "search_and_publish_circle_result", workflow)
    monkeypatch.setattr(
        composition, "circle_result_collaborators", Mock(return_value=collaborators)
    )
    document = Path("result.json")

    result = runner.invoke(
        cli.app,
        [
            "circle",
            "--dataset",
            "world",
            "--share",
            "50",
            "--cache",
            "cache",
            "--output",
            str(document),
        ],
    )

    assert result.exit_code == 0
    options = workflow.call_args.args[0]
    assert options.document == document
    assert options.ledger == document.with_suffix(".radii.json")
    assert workflow.call_args.args[1] is collaborators
    assert "result: result.json" in result.output


def test_handlers_dispatch_to_existing_entrypoints(monkeypatch: MonkeyPatch) -> None:
    workflow = Mock()
    render_file = Mock()
    circle_collaborators = Mock()
    map_collaborators = Mock()
    render_command = Mock(return_value=Path("globe.svg"))
    gallery = Mock(return_value=([], []))
    monkeypatch.setattr(circle_result, "search_and_publish_circle_result", workflow)
    monkeypatch.setattr(
        composition, "circle_result_collaborators", Mock(return_value=circle_collaborators)
    )
    monkeypatch.setattr(
        composition, "render_map_collaborators", Mock(return_value=map_collaborators)
    )
    monkeypatch.setattr(render_map, "render_file", render_file)
    monkeypatch.setattr(render_globe, "render_command", render_command)
    monkeypatch.setattr(gallery_build, "build", gallery)

    results = [
        runner.invoke(
            cli.app,
            [
                "circle",
                "--dataset",
                "world",
                "--share",
                "50",
                "--cache",
                "cache",
                "--output",
                "result.json",
            ],
        ),
        runner.invoke(cli.app, ["map", "--input", "answer.json", "--output", "map.png"]),
        runner.invoke(cli.app, ["globe-animation", "--center", "0,0", "--output", "globe.svg"]),
        runner.invoke(cli.app, ["gallery", "--input", "corpus", "--output", "gallery"]),
    ]

    assert [result.exit_code for result in results] == [0, 0, 0, 0]
    options = workflow.call_args.args[0]
    assert options.dataset == "world"
    assert options.share == "50"
    assert options == CircleResultRequest(
        "world", "50", Path("cache"), Path("result.json"), Path("result.radii.json")
    )
    assert workflow.call_args.args[1] is circle_collaborators
    render_file.assert_called_once_with(
        RenderMapFileRequest(Path("answer.json"), Path("map.png"), "plate-carree"),
        map_collaborators,
    )
    render_command.assert_called_once()
    assert gallery.call_args.args[0].corpus == Path("corpus")
    assert gallery.call_args.args[0].output == Path("gallery")


def test_circle_reports_child_diagnostics_without_rendering(monkeypatch: MonkeyPatch) -> None:
    def missing(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[bytes]:
        if "build" in arguments:
            sys.stderr.write("missing raster; run mise run data:get\n")
        return subprocess.CompletedProcess(arguments, 3, b"")

    monkeypatch.setattr(subprocess, "run", missing)
    result = runner.invoke(
        cli.app,
        [
            "circle",
            "--dataset",
            "population-count-2020-30arcsec",
            "--share",
            "50",
            "--cache",
            "cache",
            "--output",
            "result.json",
        ],
    )
    assert result.exit_code == 1
    assert "missing raster; run mise run data:get" in result.output
    assert "table build: Rust CLI exited 3" in result.output
