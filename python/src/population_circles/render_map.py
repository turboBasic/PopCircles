import argparse
import json
import textwrap
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import cast

import matplotlib as mpl
from matplotlib import pyplot as plt
from matplotlib.axes import Axes
from matplotlib.backends.backend_agg import FigureCanvasAgg
from matplotlib.collections import LineCollection
from matplotlib.figure import Figure
from matplotlib.patches import PathPatch
from matplotlib.path import Path as DrawPath
from shapely.geometry import shape
from shapely.geometry.base import BaseGeometry
from shapely.geometry.polygon import Polygon
from shapely.ops import unary_union

from population_circles.centre_caption import plain_centre
from population_circles.circle_document import Answer, circle_of
from population_circles.circle_geometry import cap, linestrings, polygons
from population_circles.country_caption import lines_for
from population_circles.country_outline import load_outline
from population_circles.dataset_registry import Registry, load
from population_circles.map_frame import (
    PLATE_CARREE,
    PROJECTIONS,
    Frame,
    country_frame,
    frame,
    graticule,
    project,
)

# Every figure this writes goes to a file, so an interactive backend is never wanted — and asking
# for one is what fails on a machine with no display, rather than falling back.
mpl.use("Agg")

# The committed basemap. Natural Earth 110m, public domain, committed and small enough to read on
# every render — `data/registry.toml` holds its terms and `data/README.md` its provenance.
COASTLINE = (
    Path(__file__).resolve().parents[3] / "data" / "boundaries" / "coastline-1to110m.geojson"
)


class UnregisteredDatasetError(ValueError):
    def __init__(self, key: str) -> None:
        super().__init__(f"the document names dataset {key!r}, which the registry does not carry")
        self.key = key


class MissingCountryOutlineError(ValueError):
    def __init__(self) -> None:
        super().__init__("country result rendering requires its registered boundary outline")


def citation(registry: Registry, key: str) -> str:
    # Keyed by the document rather than by this file: `data/registry.toml` owns the wording a
    # licence requires, and which row is owed it is the answer's own property. The registry arrives
    # as an argument so a caller can select from one it parsed itself, and `render` still takes only
    # the text — nothing below here resolves a dataset.
    dataset = registry.datasets.get(key)
    if dataset is None:
        raise UnregisteredDatasetError(key)
    return dataset.attribution


# Bottom to top: the graticule under the coastlines, the circle over both, its centre over that, and
# the frame's own outline last so no fill reaching the limb draws over it.
GRATICULE_LAYER = 1
COASTLINE_LAYER = 2
CIRCLE_LAYER = 3
COUNTRY_LAYER = 2.5
CENTRE_LAYER = 4
HORIZON_LAYER = 5


def basemap(path: Path) -> BaseGeometry:
    features: Sequence[Mapping[str, dict[str, object]]] = json.loads(
        path.read_text(encoding="utf-8"),
    )["features"]
    return unary_union([shape(feature["geometry"]) for feature in features])


def compound(polygon: Polygon) -> DrawPath:
    # Exterior first and every hole after it, in one path: a cap holding both poles covers the world
    # bar one region, and that region is a hole rather than a second part.
    return DrawPath.make_compound_path(
        DrawPath(list(polygon.exterior.coords)),
        *(DrawPath(list(ring.coords)) for ring in polygon.interiors),
    )


def fill_circle(axes: Axes, geometry: BaseGeometry) -> None:
    for polygon in polygons(geometry):
        axes.add_patch(
            PathPatch(
                compound(polygon),
                facecolor="crimson",
                edgecolor="darkred",
                alpha=0.45,
                linewidth=0.8,
                zorder=CIRCLE_LAYER,
            ),
        )


def outline(axes: Axes, geometry: BaseGeometry, layer: float = HORIZON_LAYER) -> None:
    for polygon in polygons(geometry):
        axes.add_patch(
            PathPatch(
                compound(polygon),
                facecolor="none",
                edgecolor="0.4",
                linewidth=0.6,
                zorder=layer,
            ),
        )


def stroke(axes: Axes, geometry: BaseGeometry, colour: str, width: float, layer: float) -> None:
    axes.add_collection(
        LineCollection(
            [list(line.coords) for line in linestrings(geometry)],
            colors=colour,
            linewidths=width,
            zorder=layer,
        ),
    )


def title_of(answer: Answer) -> str:
    # The table is in the caption because a reader cannot otherwise tell a 30 arc-second answer
    # from a decimated one, and the two differ by more than rounding.
    circle = answer.circle
    source = (
        f"population dataset {answer.provenance.dataset} "
        f"at decimation k={answer.provenance.decimation}"
        if answer.country is not None
        else f"table {answer.provenance.table}"
    )
    return (
        f"{circle.radius_km:,.0f} km circle holding {answer.caption.share:.2%} of the population\n"
        f"{answer.caption.population:,.0f} people, "
        f"centred {textwrap.fill(plain_centre(answer), width=44, subsequent_indent='         ')}\n"
        f"answered from {source}"
    )


def annotate(
    figure: Figure,
    y: float,
    text: str,
    size: float,
    alignment: str = "baseline",
) -> None:
    # matplotlib types Figure.text's keyword arguments as Unknown, so the ignore lives here once
    # rather than at each of the two callers.
    figure.text(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
        0.5,
        y,
        text,
        horizontalalignment="center",
        verticalalignment=alignment,
        fontsize=size,
        parse_math=False,
    )


def draw(axes: Axes, view: Frame, coastline: BaseGeometry | None) -> None:
    stroke(axes, project(view, graticule()), "0.75", 0.3, GRATICULE_LAYER)
    if coastline is not None:
        stroke(axes, project(view, coastline), "dimgrey", 0.5, COASTLINE_LAYER)
    if view.country_outline is not None:
        outline(axes, view.country_outline, COUNTRY_LAYER)
    fill_circle(axes, view.circle)
    outline(axes, view.horizon)


def render(
    answer: Answer,
    projection: str,
    attribution: str,
    *,
    # The basemap arrives drawn rather than named, so nothing under here reads a file and a caller
    # drawing several figures unions the coastlines once. `None` is a figure without them.
    coastline: BaseGeometry | None,
    outline_and_credit: tuple[BaseGeometry | None, str] | None = None,
) -> Figure:
    if answer.country is not None and outline_and_credit is None:
        raise MissingCountryOutlineError
    country_outline, boundary_attribution = outline_and_credit or (None, None)
    circle = answer.circle
    built = cap(circle.centre, circle.radius_km, circle.earth_radius_km)
    view = (
        country_frame(built, country_outline)
        if country_outline is not None
        else frame(projection, built)
    )
    centre = view.to_frame.transform(circle.centre.lon, circle.centre.lat)

    figure = plt.figure(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
        figsize=(11.0, 6.0),
    )
    try:
        title = title_of(answer)
        country_lines = lines_for(answer)
        if answer.country is not None:
            footer_parts = [
                f"Country: {answer.country.name} ({answer.country.id})",
                f"Boundary credit: {boundary_attribution}",
                f"Population credit: {attribution}",
            ]
        elif country_lines:
            footer_parts = [*country_lines, f"Population credit: {attribution}"]
        else:
            footer_parts = [attribution]
        footer = "\n".join(textwrap.fill(part, width=118) for part in footer_parts)
        has_country_footer = answer.country is not None or bool(country_lines)
        # Room for the title above and the wrapped citation below, since both are placed at figure
        # coordinates rather than left to a layout engine.
        title_lines = title.count("\n") + 1
        top = 0.87 - 0.035 * max(0, title_lines - 3)
        figure.subplots_adjust(left=0.03, right=0.97, top=top, bottom=0.16)
        axes = figure.add_subplot(1, 1, 1)
        # Equal: degrees of longitude and latitude have the same length on the
        # frame stated in degrees, and the other frame is metres in both directions.
        axes.set_aspect("equal")
        # The horizon replaces the spines: one frame's outline is a rectangle and
        # the other's is a disc, and only one of those is an axes frame.
        axes.set_axis_off()
        west, south, east, north = view.horizon.bounds
        axes.set_xlim(west, east)
        axes.set_ylim(south, north)

        draw(axes, view, coastline)
        axes.plot(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
            [centre[0]],
            [centre[1]],
            marker="+",
            color="black",
            markersize=9,
            zorder=CENTRE_LAYER,
        )

        # Anchored at its top, so an added title line grows into the margin rather than off
        # the top of the figure — which is silent, since nothing about a clipped artist fails.
        annotate(figure, 0.985, title, 12.0, "top")
        # Wrapped: at this width a one-line citation runs off both edges of the figure,
        # which is an attribution nobody can read and so not an attribution.
        annotate(figure, 0.04 / 6.0 if has_country_footer else 0.055, footer, 6.5, "bottom")
        if has_country_footer:
            canvas = cast(FigureCanvasAgg, figure.canvas)
            canvas.draw()
            footer_height = figure.texts[-1].get_window_extent(canvas.get_renderer()).height
            footer_height /= figure.dpi
            height = 6.0 + max(0.0, footer_height + 0.12 - 0.96)
            figure.set_size_inches(11.0, height)
            figure.texts[-1].set_position((0.5, 0.04 / height))
            figure.subplots_adjust(
                left=0.03,
                right=0.97,
                top=top,
                bottom=(footer_height + 0.12) / height,
            )
    except BaseException:
        plt.close(figure)
        raise
    return figure


def parse(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Render a circle from a popcircles document.")
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--projection", choices=PROJECTIONS, default=PLATE_CARREE)
    parser.add_argument("--no-coastlines", action="store_true")
    return parser.parse_args(argv)


def render_file(
    input_path: Path,
    output_path: Path,
    projection: str = PLATE_CARREE,
    *,
    no_coastlines: bool = False,
) -> None:
    document = json.loads(input_path.read_text(encoding="utf-8"))
    answer = circle_of(document)
    registry = load()
    population_attribution = (
        answer.selection.population_source.attribution if answer.selection is not None else None
    ) or citation(registry, answer.provenance.dataset)
    boundary_attribution = (
        answer.selection.boundary_source.attribution if answer.selection is not None else None
    )
    figure = render(
        answer,
        projection,
        population_attribution,
        coastline=None if no_coastlines else basemap(COASTLINE),
        outline_and_credit=(
            (
                load_outline(answer, registry),
                boundary_attribution or citation(registry, answer.country.boundary_dataset),
            )
            if answer.country is not None
            else None
        ),
    )
    try:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        # No bbox_inches="tight": the title and citation use figure coordinates; cropping
        # to the drawn content moves them off the positions they were given.
        figure.savefig(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
            output_path,
            dpi=200,
        )
    finally:
        plt.close(figure)


def main(argv: list[str] | None = None) -> int:
    args = parse(argv)
    render_file(args.input, args.output, args.projection, no_coastlines=args.no_coastlines)
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
