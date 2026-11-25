from typing import NotRequired, TypedDict
from urllib.parse import quote

from PIL import Image

from population_circles.gallery.rendering import Drawn, FigureCaption


class PageFigure(TypedDict):
    map: str
    document: str
    country_id: str | None
    alt: str
    aspect_ratio: float
    caption: NotRequired[FigureCaption]


class PageCountry(TypedDict):
    name: str
    anchor: str
    figures: list[PageFigure]
    missing: bool


class PageData(TypedDict):
    world: list[PageFigure]
    countries: list[PageCountry]


def page_figure(one: Drawn) -> PageFigure:
    with Image.open(one.figure) as image:
        aspect_ratio = image.width / image.height
    figure: PageFigure = {
        "map": one.figure.name,
        "document": one.document.name,
        "country_id": one.country_id,
        "alt": one.figure.stem,
        "aspect_ratio": aspect_ratio,
    }
    if one.caption is not None:
        figure["caption"] = one.caption
    return figure


def page_data(drawn: list[Drawn], missing: list[str]) -> PageData:
    countries: dict[tuple[str, str], list[Drawn]] = {}
    world: list[Drawn] = []
    for one in drawn:
        if one.country_name is None or one.country_id is None:
            world.append(one)
        else:
            countries.setdefault((one.country_name, one.country_id), []).append(one)

    ordered = sorted(countries.items(), key=lambda item: (item[0][0].casefold(), item[0][1]))
    page_countries: list[PageCountry] = [
        {
            "name": name,
            "anchor": f"country-result-{quote(identifier, safe='')}",
            "figures": [page_figure(figure) for figure in figures],
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
    return {"world": [page_figure(one) for one in world], "countries": page_countries}
