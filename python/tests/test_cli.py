from unittest.mock import Mock

from _pytest.monkeypatch import MonkeyPatch
from typer.testing import CliRunner

from population_circles import circle_map_workflow, cli
from population_circles.gallery import build as gallery_build
from population_circles.maps import render_globe, render_map

runner = CliRunner()
INVALID_ARGUMENT_EXIT = 2


def test_help_lists_commands() -> None:
    result = runner.invoke(cli.app, ["--help"])

    assert result.exit_code == 0
    assert "map" in result.stdout
    assert "render" in result.stdout
    assert "gallery" in result.stdout


def test_map_requires_dataset() -> None:
    result = runner.invoke(cli.app, ["map"])

    assert result.exit_code == INVALID_ARGUMENT_EXIT
    assert "--dataset" in result.output


def test_map_rejects_invalid_projection() -> None:
    result = runner.invoke(
        cli.app,
        [
            "map",
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


def test_handlers_dispatch_to_existing_entrypoints(monkeypatch: MonkeyPatch) -> None:
    map_main = Mock(return_value=0)
    render_main = Mock(return_value=0)
    globe_main = Mock(return_value=0)
    gallery_main = Mock(return_value=0)
    monkeypatch.setattr(circle_map_workflow, "main", map_main)
    monkeypatch.setattr(render_map, "main", render_main)
    monkeypatch.setattr(render_globe, "main", globe_main)
    monkeypatch.setattr(gallery_build, "main", gallery_main)

    commands = [
        (
            [
                "map",
                "--dataset",
                "world",
                "--share",
                "50",
                "--cache",
                "cache",
                "--output",
                "map.png",
            ],
            map_main,
            ["--dataset", "world"],
        ),
        (
            ["render", "map", "--input", "answer.json", "--output", "map.png"],
            render_main,
            ["--input", "answer.json"],
        ),
        (
            ["render", "globe", "--center", "0,0", "--output", "globe.svg"],
            globe_main,
            ["--center", "0,0"],
        ),
        (
            ["gallery", "build", "--input", "corpus", "--output", "gallery"],
            gallery_main,
            ["--input", "corpus"],
        ),
    ]

    for arguments, main, expected in commands:
        result = runner.invoke(cli.app, arguments)
        assert result.exit_code == 0, result.stdout
        main.assert_called_once()
        assert main.call_args.args[0][:2] == expected
        main.reset_mock()
