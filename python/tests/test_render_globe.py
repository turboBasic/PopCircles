# pyright: reportPrivateUsage=false
# These tests intentionally exercise private implementation details.

import xml.etree.ElementTree as ET
from io import BytesIO
from pathlib import Path

import pytest
from PIL import Image
from typer.testing import CliRunner

from population_circles.domain.geography import HALF_TURN_DEG, POLE_LAT, GeographicCoordinate
from population_circles.presentation import cli
from population_circles.presentation.maps import animated_globe
from population_circles.presentation.maps.animated_globe import (
    _parse_svg as parse_svg,
)
from population_circles.presentation.maps.animated_globe import (
    _render_gif as render_gif,
)
from population_circles.presentation.maps.animated_globe import (
    _render_latitude_sequence as render_latitude_sequence,
)
from population_circles.presentation.maps.animated_globe import (
    _render_svg_sprite as render_svg_sprite,
)

SVG = "http://www.w3.org/2000/svg"
CLI_ERROR_EXIT = 2


def write_frame(path: Path, colour: str) -> None:
    path.write_text(
        f'<svg xmlns="{SVG}" viewBox="0 0 512 512">\n'
        "  <metadata>Globe attribution</metadata>\n"
        '  <circle cx="256" cy="256" r="240" fill="#016DB8"/>\n'
        f'  <g fill="{colour}" fill-rule="evenodd"><path d="M 1 2 Z"/></g>\n'
        "</svg>\n",
        encoding="utf-8",
    )


def test_render_latitude_sequence_crosses_poles_without_flipping(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    rendered: list[tuple[float, float, float, Path]] = []

    def capture_render(
        centre: GeographicCoordinate,
        output: Path,
        _detalization: float,
        rotation: float,
    ) -> None:
        rendered.append((centre.lat, centre.lon, rotation, output))

    monkeypatch.setattr(animated_globe, "_render_globe", capture_render)

    sequence = render_latitude_sequence(
        GeographicCoordinate(lat=0, lon=20),
        tmp_path / "earth-globe-72.svg",
        8,
        detalization=0.5,
    )

    assert (rendered[0][0], rendered[0][1], rendered[0][2]) == (0, 20, 0)
    assert all(abs(latitude) < POLE_LAT for latitude, _, _, _ in rendered)
    assert [rotation for _, _, rotation, _ in rendered] == [
        0,
        0,
        0,
        HALF_TURN_DEG,
        HALF_TURN_DEG,
        HALF_TURN_DEG,
        0,
        0,
    ]
    assert [path.name for _, _, _, path in rendered] == [
        "earth-globe-01.svg",
        "earth-globe-02.svg",
        "earth-globe-03.svg",
        "earth-globe-04.svg",
        "earth-globe-05.svg",
        "earth-globe-06.svg",
        "earth-globe-07.svg",
        "earth-globe-08.svg",
    ]
    assert sequence == tmp_path / "earth-globe-08.svg"


@pytest.mark.parametrize("latitude", [-90, 90])
def test_center_rejects_poles(latitude: float) -> None:
    with pytest.raises(ValueError, match="center latitude cannot be -90 or 90"):
        animated_globe.parse_center(f"{latitude},20")


def test_render_svg_sprite_preserves_frames_and_shared_attribution(tmp_path: Path) -> None:
    write_frame(tmp_path / "globe-01.svg", "#4CDD9F")
    write_frame(tmp_path / "globe-02.svg", "#4CDD9F")
    output = tmp_path / "globe.svg"

    render_svg_sprite(tmp_path / "globe-02.svg", output)

    root = ET.parse(output).getroot()
    symbols = root.findall(f"{{{SVG}}}symbol")
    assert [symbol.attrib for symbol in symbols] == [
        {"id": "frame-01", "viewBox": "0 0 512 512"},
        {"id": "frame-02", "viewBox": "0 0 512 512"},
    ]
    assert len(root.findall(f"{{{SVG}}}metadata")) == 1
    assert root.findtext(f"{{{SVG}}}metadata") == "Globe attribution"
    assert [len(symbol.findall(f"{{{SVG}}}circle")) for symbol in symbols] == [1, 1]
    groups = [symbol.find(f"{{{SVG}}}g") for symbol in symbols]
    assert all(group is not None for group in groups)
    assert [group.attrib["fill-rule"] for group in groups if group is not None] == [
        "evenodd",
        "evenodd",
    ]


@pytest.mark.parametrize(
    ("sequence_svg", "svg_sprite"),
    [(None, None), (Path("sequence-02.svg"), Path("sprite.svg"))],
)
def test_render_gif_requires_exactly_one_source(
    tmp_path: Path,
    sequence_svg: Path | None,
    svg_sprite: Path | None,
) -> None:
    with pytest.raises(ValueError, match="exactly one"):
        render_gif(
            tmp_path / "out.gif",
            sequence_svg,
            svg_sprite=svg_sprite,
        )


@pytest.mark.parametrize("source_kind", ["sequence", "sprite"])
def test_render_gif_preserves_source_frame_order(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    source_kind: str,
) -> None:
    calls: list[str] = []
    png = BytesIO()
    Image.new("RGB", (1, 1)).save(png, format="PNG")
    png_bytes = png.getvalue()

    def svg_to_bytes(
        *,
        svg_path: str | None = None,
        svg_string: str | None = None,
        width: int,
        height: int,
    ) -> bytes:
        assert (width, height) == (200, 200)
        calls.append(svg_path or svg_string or "")
        return png_bytes

    monkeypatch.setattr("resvg_py.svg_to_bytes", svg_to_bytes)
    if source_kind == "sequence":
        first = tmp_path / "globe-01.svg"
        second = tmp_path / "globe-02.svg"
        write_frame(first, "#111111")
        write_frame(second, "#222222")
        render_gif(tmp_path / "out.gif", second)
        assert calls == [str(first), str(second)]
    else:
        sprite = tmp_path / "globe.svg"
        sprite.write_text(
            f'<svg xmlns="{SVG}"><symbol id="b" viewBox="0 0 512 512">'
            '<circle fill="#111111"/></symbol><symbol id="a" viewBox="0 0 512 512">'
            '<circle fill="#222222"/></symbol></svg>',
            encoding="utf-8",
        )
        render_gif(tmp_path / "out.gif", svg_sprite=sprite)
        assert 'fill="#111111"' in calls[0]
        assert 'fill="#222222"' in calls[1]


def test_svg_sprite_requires_latitude_steps() -> None:
    result = CliRunner().invoke(
        cli.app,
        ["globe-animation", "--center", "0,0", "--output", "globe.svg", "--svg-sprite"],
    )
    assert result.exit_code == CLI_ERROR_EXIT


def test_renderer_value_error_keeps_runtime_failure_status(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def fail(*_args: object, **_kwargs: object) -> None:
        message = "bad registered boundary"
        raise ValueError(message)

    monkeypatch.setattr(animated_globe, "_render_globe", fail)

    result = CliRunner().invoke(
        cli.app,
        ["globe-animation", "--center", "0,0", "--output", "globe.svg"],
    )

    assert result.exit_code == 1
    assert isinstance(result.exception, ValueError)


def test_parse_svg_rejects_entities(tmp_path: Path) -> None:
    svg = tmp_path / "unsafe.svg"
    svg.write_text(
        '<!DOCTYPE svg [<!ENTITY text "unsafe">]><svg>&text;</svg>',
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="entities"):
        parse_svg(svg)
