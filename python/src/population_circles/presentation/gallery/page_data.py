from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import NotRequired, TypedDict
from urllib.parse import quote

from PIL import Image

from population_circles.application.use_cases.country_sweep import (
    FigureCaption as _FigureCaption,
)
from population_circles.application.use_cases.country_sweep import (
    GalleryFigure,
)

__all__ = ["Drawn", "page_data"]


@dataclass(frozen=True)
class Drawn:
    document: Path
    figure: Path
    country_name: str | None = None
    country_id: str | None = None
    caption: _FigureCaption | None = None


class _PageFigure(TypedDict):
    map: str
    document: str
    country_id: str | None
    alt: str
    aspect_ratio: float
    caption: NotRequired[_FigureCaption]


class _PageCountry(TypedDict):
    name: str
    anchor: str
    figures: list[_PageFigure]
    missing: bool


class _PageData(TypedDict):
    world: list[_PageFigure]
    countries: list[_PageCountry]


def _page_figure(one: GalleryFigure) -> _PageFigure:
    with Image.open(one.figure) as image:
        aspect_ratio = image.width / image.height
    figure: _PageFigure = {
        "map": one.figure.name,
        "document": one.document.name,
        "country_id": one.country_id,
        "alt": one.figure.stem,
        "aspect_ratio": aspect_ratio,
    }
    if one.caption is not None:
        figure["caption"] = one.caption
    return figure


def page_data(drawn: Sequence[GalleryFigure], missing: list[str]) -> _PageData:
    countries: dict[tuple[str, str], list[GalleryFigure]] = {}
    world: list[GalleryFigure] = []
    for one in drawn:
        if one.country_name is None or one.country_id is None:
            world.append(one)
        else:
            countries.setdefault((one.country_name, one.country_id), []).append(one)

    ordered = sorted(countries.items(), key=lambda item: (item[0][0].casefold(), item[0][1]))
    page_countries: list[_PageCountry] = [
        {
            "name": name,
            "anchor": f"country-result-{quote(identifier, safe='')}",
            "figures": [_page_figure(figure) for figure in figures],
            "missing": False,
        }
        for (name, identifier), figures in ordered
    ]
    page_countries.extend(
        {
            "name": name,
            "anchor": f"country-missing-{quote(name, safe='')}",
            "figures": [],
            "missing": True,
        }
        for name in missing
    )
    page_countries.sort(key=lambda country: (country["name"].casefold(), country["anchor"]))
    return {"world": [_page_figure(one) for one in world], "countries": page_countries}
