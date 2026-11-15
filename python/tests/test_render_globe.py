from argparse import ArgumentTypeError
from pathlib import Path

import pytest

from population_circles import render_globe
from population_circles.circle_document import Coordinate
from population_circles.circle_geometry import HALF_TURN_DEG, POLE_LAT


def test_render_latitude_sequence_crosses_poles_without_flipping(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    rendered: list[tuple[float, float, float, Path]] = []

    def capture_render(
        centre: Coordinate,
        output: Path,
        _detalization: float,
        rotation: float,
    ) -> None:
        rendered.append((centre.lat, centre.lon, rotation, output))

    monkeypatch.setattr(render_globe, "render_globe", capture_render)

    sequence = render_globe.render_latitude_sequence(
        Coordinate(lat=0, lon=20),
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
    with pytest.raises(ArgumentTypeError):
        render_globe.parse_center(f"{latitude},20")
