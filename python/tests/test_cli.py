from pathlib import Path
from unittest.mock import Mock

from _pytest.monkeypatch import MonkeyPatch
from typer.testing import CliRunner

from population_circles import cli
from population_circles.gallery import build as gallery_build
from population_circles.maps import render_globe, render_map
from population_circles.workflows import circle_map

runner = CliRunner()
INVALID_ARGUMENT_EXIT = 2


def test_help_lists_commands() -> None:
    result = runner.invoke(cli.app, ["--help"])

    assert result.exit_code == 0
    assert "circle" in result.stdout
    assert "map" in result.stdout
    assert "globe" in result.stdout
    assert "gallery" in result.stdout


def test_circle_requires_dataset() -> None:
    result = runner.invoke(cli.app, ["circle"])

    assert result.exit_code == INVALID_ARGUMENT_EXIT
    assert "--dataset" in result.output


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
    monkeypatch.setattr(circle_map, "workflow", workflow)
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
    monkeypatch.setattr(circle_map, "workflow", workflow)
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
                "map.png",
            ],
        ),
        runner.invoke(cli.app, ["map", "--input", "answer.json", "--output", "map.png"]),
        runner.invoke(cli.app, ["globe", "--center", "0,0", "--output", "globe.svg"]),
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
    assert gallery.call_args.args == (Path("corpus"), Path("gallery"))
