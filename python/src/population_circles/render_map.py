import argparse
import json
import math
import textwrap
import xml.etree.ElementTree as ET
from collections.abc import Callable, Mapping, Sequence
from pathlib import Path
from typing import cast

import matplotlib as mpl
from matplotlib import pyplot as plt
from matplotlib.axes import Axes
from matplotlib.collections import LineCollection
from matplotlib.figure import Figure
from matplotlib.image import AxesImage
from matplotlib.patches import PathPatch
from matplotlib.path import Path as DrawPath
from shapely import get_coordinates
from shapely.geometry import shape
from shapely.geometry.base import BaseGeometry
from shapely.geometry.polygon import Polygon
from shapely.ops import unary_union

from population_circles.circle_document import Answer, circle_of
from population_circles.circle_geometry import cap, linestrings, polygons
from population_circles.country_outline import load_map_boundaries
from population_circles.dataset_registry import (
    REPO_ROOT,
    BoundaryVector,
    MapTextPlacement,
    Registry,
    coastline_dataset_key,
    load,
    map_text_placement,
)
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

COASTLINE_DATASET = coastline_dataset_key()


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
OCEAN = "#DFFFFD"
COUNTRY = "#65CBC1"
OUTLINE_WIDTH = 0.6
CENTRE_LAYER = 4
HORIZON_LAYER = 5


def basemap(path: Path) -> BaseGeometry:
    features: Sequence[Mapping[str, dict[str, object]]] = json.loads(
        path.read_text(encoding="utf-8"),
    )["features"]
    return unary_union([shape(feature["geometry"]) for feature in features])


def registered_coastline(registry: Registry) -> tuple[BaseGeometry, str]:
    dataset = registry.datasets.get(coastline_dataset_key())
    if not isinstance(dataset, BoundaryVector):
        raise UnregisteredDatasetError(coastline_dataset_key())
    return basemap(dataset.file(REPO_ROOT)), dataset.attribution


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


def outline(
    axes: Axes,
    geometry: BaseGeometry,
    layer: float = HORIZON_LAYER,
    *,
    facecolor: str = "none",
    linewidth: float = OUTLINE_WIDTH,
) -> None:
    for polygon in polygons(geometry):
        axes.add_patch(
            PathPatch(
                compound(polygon),
                facecolor=facecolor,
                edgecolor="0.4",
                linewidth=linewidth,
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
    circle = answer.circle
    country = f"{answer.country.name} ({answer.country.id}): " if answer.country else ""
    centre = f"{circle.centre.lat}, {circle.centre.lon}"
    settlement = answer.caption.settlement
    if settlement is not None:
        distance = int(settlement.distance_km + 0.5)
        centre += f", {distance} km from {settlement.name}"
    return (
        f"{country}{circle.radius_km:,.0f} km circle holding "
        f"{answer.caption.share:.1%} of the population "
        f"({answer.caption.population:,.0f} people); Center: {centre}"
    )


def footer_text() -> str:
    return (
        "© Andrii Melnyk, 2026, https://github.com/turboBasic/PopCircles · "
        "© EuroGeographics for the administrative boundaries · "
        "See full attribution in image metadata"
    )


def copyright_of(
    answer: Answer,
    attribution: str,
    coastline_attribution: str | None,
    boundary_attribution: str | None,
) -> str:
    parts = ["© Andrii Melnyk, 2026, https://github.com/turboBasic/PopCircles"]
    if answer.country is not None:
        parts.append(f"Population data: {attribution}")
        parts.append(f"Administrative boundaries: {boundary_attribution}")
    elif answer.caption.country_breakdown is not None:
        parts.append(
            f"Administrative boundaries: {answer.caption.country_breakdown.provenance.attribution}"
        )
        parts.append(f"Population data: {attribution}")
    else:
        parts.append(f"Population data: {attribution}")
    if coastline_attribution:
        parts.append(f"Coastline data: {coastline_attribution}")
    return "\n".join(textwrap.fill(part, width=118) for part in parts)


def xmp_properties(answer: Answer) -> dict[str, str | float | int | None]:
    circle = answer.circle
    properties: dict[str, str | float | int | None] = {
        "population": answer.caption.population,
        "populationShare": answer.caption.share,
        "radiusKm": circle.radius_km,
        "centerLatitude": circle.centre.lat,
        "centerLongitude": circle.centre.lon,
        "populationDataset": answer.provenance.dataset,
        "decimation": answer.provenance.decimation,
    }
    if answer.country is not None:
        properties["countryName"] = answer.country.name
        properties["countryCode"] = answer.country.id
    if answer.caption.settlement is not None:
        properties["nearestSettlement"] = answer.caption.settlement.name
        properties["nearestSettlementCountry"] = answer.caption.settlement.country
        properties["distanceToSettlementKm"] = answer.caption.settlement.distance_km
    return properties


def xmp_packet(properties: Mapping[str, str | float | int]) -> str:
    namespaces = {
        "x": "adobe:ns:meta/",
        "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
        "pc": "https://github.com/turboBasic/PopCircles/ns/1.0/",
    }
    for prefix, uri in namespaces.items():
        ET.register_namespace(prefix, uri)
    xmp = ET.Element(f"{{{namespaces['x']}}}xmpmeta")
    rdf = ET.SubElement(xmp, f"{{{namespaces['rdf']}}}RDF")
    description = ET.SubElement(rdf, f"{{{namespaces['rdf']}}}Description")
    for name, value in properties.items():
        if value is not None:
            description.set(f"{{{namespaces['pc']}}}{name}", str(value))
    return ET.tostring(xmp, encoding="unicode", xml_declaration=True)


def description_of(answer: Answer) -> str:
    return (
        f"Answered from population dataset {answer.provenance.dataset} "
        f"at decimation k={answer.provenance.decimation}"
    )


def metadata_json_fields(
    title: str,
    description: str,
    copyright_text: str,
    properties: Mapping[str, str | float | int | None],
    placement: tuple[MapTextPlacement, MapTextPlacement],
) -> dict[str, object]:
    title_placement, credits_placement = placement
    values: dict[str, object] = {
        "Description": description,
        "XML:com.adobe.xmp": dict(properties),
    }
    if title_placement == "metadata":
        values["Title"] = title
    if credits_placement == "metadata":
        values["Copyright"] = copyright_text
    return values


def png_metadata(
    title: str,
    copyright_text: str,
    placement: tuple[MapTextPlacement, MapTextPlacement],
    description: str,
    properties: Mapping[str, str | float | int | None],
) -> dict[str, str]:
    title_placement, credits_placement = placement
    metadata: dict[str, str] = {}
    if title_placement == "metadata":
        metadata["Title"] = title
    metadata["Description"] = description
    if credits_placement == "metadata":
        metadata["Copyright"] = copyright_text
    metadata["XML:com.adobe.xmp"] = xmp_packet(
        {name: value for name, value in properties.items() if value is not None}
    )
    return metadata


def other_countries(
    boundaries: Mapping[str, BaseGeometry] | None, answer: Answer
) -> list[BaseGeometry]:
    if boundaries is None:
        return []
    return [
        geometry
        for identifier, geometry in boundaries.items()
        if answer.country is None or identifier != answer.country.id
    ]


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


def draw(
    axes: Axes,
    view: Frame,
    coastline: BaseGeometry | None,
    other_countries: Sequence[BaseGeometry] = (),
) -> None:
    stroke(axes, project(view, graticule()), "0.75", 0.3, GRATICULE_LAYER)
    if coastline is not None:
        stroke(axes, project(view, coastline), "dimgrey", 0.5, COASTLINE_LAYER)
    country_paths: list[DrawPath] = []
    for geometry in other_countries:
        projected = project(view, geometry)
        if all(math.isfinite(value) for point in get_coordinates(projected) for value in point):
            country_paths.extend(compound(polygon) for polygon in polygons(projected))
    if country_paths:
        axes.add_patch(
            PathPatch(
                DrawPath.make_compound_path(*country_paths),
                facecolor="white",
                edgecolor="0.4",
                linewidth=OUTLINE_WIDTH,
                zorder=COUNTRY_LAYER,
            ),
        )
    if view.country_outline is not None:
        outline(axes, view.country_outline, COUNTRY_LAYER, facecolor=COUNTRY, linewidth=1)
    fill_circle(axes, view.circle)
    outline(axes, view.horizon)


def fill_ocean(axes: Axes, view: Frame) -> None:
    west, south, east, north = view.horizon.bounds
    draw_image = cast(Callable[..., AxesImage], axes.imshow)  # pyright: ignore[reportUnknownMemberType] — Matplotlib kwargs: Unknown
    ocean = draw_image(
        [[(223 / 255, 1.0, 253 / 255, 1.0)]],
        extent=(west, east, south, north),
        zorder=0,
    )
    horizon = DrawPath.make_compound_path(
        *(compound(polygon) for polygon in polygons(view.horizon))
    )
    ocean.set_clip_path(PathPatch(horizon, transform=axes.transData))


def render(  # noqa: PLR0913 — distinct geometry and display inputs stay explicit
    answer: Answer,
    projection: str,
    *,
    # The basemap arrives drawn rather than named, so nothing under here reads a file and a caller
    # drawing several figures unions the coastlines once. `None` is a figure without them.
    coastline: BaseGeometry | None,
    country_outline: BaseGeometry | None = None,
    other_countries: Sequence[BaseGeometry] = (),
    text_placement: tuple[MapTextPlacement, MapTextPlacement] = ("raster", "raster"),
) -> Figure:
    if answer.country is not None and country_outline is None:
        raise MissingCountryOutlineError
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
        footer = footer_text()
        # Keep room for the title and centered footer, since both use figure coordinates.
        title_lines = title.count("\n") + 1
        title_top = 0.87 - 0.035 * max(0, title_lines - 3)
        top = title_top if text_placement[0] == "raster" else 0.98
        bottom = 0.16 if text_placement[1] == "raster" else 0.03
        figure.subplots_adjust(left=0.03, right=0.97, top=top, bottom=bottom)
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
        if view.country_outline is not None or other_countries:
            fill_ocean(axes, view)

        draw(axes, view, coastline, other_countries)
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
        if text_placement[0] == "raster":
            annotate(figure, 0.985, title, 12.0, "top")
        if text_placement[1] == "raster":
            annotate(figure, 0.04, footer, 6.5, "bottom")
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
    coastline, coastline_attribution = (
        (None, None) if no_coastlines else registered_coastline(registry)
    )
    text_placement = map_text_placement()
    boundaries = load_map_boundaries(answer, registry)
    selected = boundaries.get(answer.country.id) if answer.country is not None else None
    country_boundary_attribution = None
    if answer.country is not None:
        boundary_credit = boundary_attribution or citation(
            registry, answer.country.boundary_dataset
        )
        country_boundary_attribution = boundary_credit
    figure = render(
        answer,
        projection,
        coastline=coastline,
        country_outline=selected,
        text_placement=text_placement,
        **({"other_countries": other_countries(boundaries, answer)} if boundaries else {}),
    )
    title = title_of(answer)
    copyright_text = copyright_of(
        answer,
        population_attribution,
        coastline_attribution,
        country_boundary_attribution,
    )
    description = description_of(answer)
    properties = xmp_properties(answer)
    metadata_values = metadata_json_fields(
        title, description, copyright_text, properties, text_placement
    )
    sidecar_path = output_path.with_suffix(".json")
    try:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        # No bbox_inches="tight": the title and citation use figure coordinates; cropping
        # to the drawn content moves them off the positions they were given.
        figure.savefig(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
            output_path,
            dpi=200,
            metadata=png_metadata(title, copyright_text, text_placement, description, properties),
        )
        document["render_metadata"] = metadata_values
        sidecar_path.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
    finally:
        plt.close(figure)


def main(argv: list[str] | None = None) -> int:
    args = parse(argv)
    render_file(args.input, args.output, args.projection, no_coastlines=args.no_coastlines)
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
