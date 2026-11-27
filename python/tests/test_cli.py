from pathlib import Path
from unittest.mock import Mock

from _pytest.monkeypatch import MonkeyPatch
from typer.testing import CliRunner

from population_circles.application.errors import PopulationApplicationError
from population_circles.application.ports import PopulationRunner
from population_circles.application.use_cases import circle_map, gallery_build
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
    def unused_collaborators(_runner: PopulationRunner) -> Mock:
        return Mock()

    monkeypatch.setattr(cli, "gallery_build_collaborators", unused_collaborators)
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


def test_circle_rejects_invalid_projection() -> None:
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
            "map.png",
            "--projection",
            "invalid",
        ],
    )

    assert result.exit_code == INVALID_ARGUMENT_EXIT


def test_circle_overrides_set_document_and_default_ledger(monkeypatch: MonkeyPatch) -> None:
    workflow = Mock()
    monkeypatch.setattr(circle_map, "search_and_render_circle_map", workflow)
    document = Path("other.json")

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
            "map.png",
            "--document",
            str(document),
        ],
    )

    assert result.exit_code == 0
    options = workflow.call_args.args[0]
    assert options.document == document
    assert options.ledger == document.with_suffix(".radii.json")


def test_handlers_dispatch_to_existing_entrypoints(monkeypatch: MonkeyPatch) -> None:
    workflow = Mock()
    render_file = Mock()
    render_command = Mock(return_value=Path("globe.svg"))
    gallery = Mock(return_value=([], []))
    monkeypatch.setattr(circle_map, "search_and_render_circle_map", workflow)
    monkeypatch.setattr(cli, "render_map_file", render_file)
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
                "map.png",
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
    assert options.document == Path("map.json")
    assert options.ledger == Path("map.radii.json")
    render_file.assert_called_once_with(
        Path("answer.json"), Path("map.png"), "plate-carree", no_coastlines=False
    )
    render_command.assert_called_once()
    assert gallery.call_args.args[0].corpus == Path("corpus")
    assert gallery.call_args.args[0].output == Path("gallery")
