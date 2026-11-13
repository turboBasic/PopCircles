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
from matplotlib.backends.backend_agg import FigureCanvasAgg
from matplotlib.collections import LineCollection
from matplotlib.figure import Figure
from matplotlib.font_manager import FontProperties
from matplotlib.image import AxesImage
from matplotlib.patches import PathPatch
from matplotlib.path import Path as DrawPath
from PIL import ExifTags, Image
from PIL.TiffImagePlugin import IFDRational
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
    Registry,
    coastline_dataset_key,
    load,
    map_text_rendering,
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
MINUTES_PER_DEGREE = 60


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
    country = f"{answer.country.name}: " if answer.country else ""
    centre = f"{circle.centre.lat:.4f}, {circle.centre.lon:.4f}"
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


def xmp_properties(
    answer: Answer, title: str, description: str, copyright_text: str
) -> dict[str, object]:
    circle = answer.circle
    properties: dict[str, object] = {
        "population": answer.caption.population,
        "populationShare": answer.caption.share,
        "radiusKm": circle.radius_km,
        "centerLatitude": circle.centre.lat,
        "centerLongitude": circle.centre.lon,
        "populationDataset": answer.provenance.dataset,
        "decimation": answer.provenance.decimation,
    }
    if answer.country is not None:
        properties["photoshop:Country"] = answer.country.name
        properties["Iptc4xmpCore:CountryCode"] = answer.country.id
        properties["countryName"] = answer.country.name
        properties["countryCode"] = answer.country.id
    settlement = answer.caption.settlement
    place: dict[str, object] = {
        "exif:GPSLatitude": gps_coordinate(circle.centre.lat, "N", "S"),
        "exif:GPSLongitude": gps_coordinate(circle.centre.lon, "E", "W"),
    }
    if answer.country is not None:
        place["Iptc4xmpExt:CountryName"] = answer.country.name
        place["Iptc4xmpExt:CountryCode"] = answer.country.id
    if settlement is not None:
        place["Iptc4xmpExt:City"] = settlement.name
        if answer.country is None:
            place["Iptc4xmpExt:CountryName"] = settlement.country
        properties["nearestSettlement"] = settlement.name
        properties["nearestSettlementCountry"] = settlement.country
        properties["distanceToSettlementKm"] = settlement.distance_km
    properties["Iptc4xmpExt:LocationShown"] = [place]
    properties["dc:creator"] = ["Andrii Melnyk"]
    properties["Iptc4xmpCore:CreatorContactInfo"] = {
        "Iptc4xmpCore:CiUrlWork": "https://github.com/turboBasic/PopCircles"
    }
    properties["dc:rights"] = "© Andrii Melnyk, 2026"
    properties["dc:title"] = title
    properties["dc:description"] = description
    properties["photoshop:Credit"] = footer_text()
    properties["fullAttribution"] = copyright_text
    return {name: value for name, value in properties.items() if value is not None}


def gps_coordinate(value: float, positive: str, negative: str) -> str:
    degrees = int(abs(value))
    minutes = round((abs(value) - degrees) * MINUTES_PER_DEGREE, 5)
    if minutes == MINUTES_PER_DEGREE:
        degrees += 1
        minutes = 0
    hemisphere = positive if value >= 0 else negative
    return f"{degrees},{minutes:g}{hemisphere}"


def _xmp_creator(description: ET.Element, value: object, namespaces: Mapping[str, str]) -> None:
    sequence = ET.SubElement(description, f"{{{namespaces['dc']}}}creator")
    items = ET.SubElement(sequence, f"{{{namespaces['rdf']}}}Seq")
    for creator in cast(list[str], value):
        ET.SubElement(items, f"{{{namespaces['rdf']}}}li").text = creator


def _xmp_alt_text(
    description: ET.Element, name: str, value: object, namespaces: Mapping[str, str]
) -> None:
    local_name = name.split(":", maxsplit=1)[1]
    prop = ET.SubElement(description, f"{{{namespaces['dc']}}}{local_name}")
    items = ET.SubElement(prop, f"{{{namespaces['rdf']}}}Alt")
    item = ET.SubElement(items, f"{{{namespaces['rdf']}}}li")
    item.set("{http://www.w3.org/XML/1998/namespace}lang", "x-default")
    item.text = cast(str, value)


def _xmp_creator_contact(
    description: ET.Element, value: object, namespaces: Mapping[str, str]
) -> None:
    prop = ET.SubElement(description, f"{{{namespaces['Iptc4xmpCore']}}}CreatorContactInfo")
    prop.set(f"{{{namespaces['rdf']}}}parseType", "Resource")
    for field, text in cast(dict[str, str], value).items():
        _, local_name = field.split(":", maxsplit=1)
        ET.SubElement(prop, f"{{{namespaces['Iptc4xmpCore']}}}{local_name}").text = text


def _xmp_location_shown(
    description: ET.Element, value: object, namespaces: Mapping[str, str]
) -> None:
    prop = ET.SubElement(description, f"{{{namespaces['Iptc4xmpExt']}}}LocationShown")
    bag = ET.SubElement(prop, f"{{{namespaces['rdf']}}}Bag")
    for location in cast(list[dict[str, object]], value):
        item = ET.SubElement(bag, f"{{{namespaces['rdf']}}}li")
        item.set(f"{{{namespaces['rdf']}}}parseType", "Resource")
        for field, text in location.items():
            prefix, local_name = field.split(":", maxsplit=1)
            ET.SubElement(item, f"{{{namespaces[prefix]}}}{local_name}").text = str(text)


def _xmp_scalar(
    description: ET.Element, name: str, value: object, namespaces: Mapping[str, str]
) -> None:
    prefix, separator, local_name = name.partition(":")
    if not separator:
        prefix, local_name = "pc", name
    description.set(f"{{{namespaces[prefix]}}}{local_name}", str(value))


def xmp_packet(properties: Mapping[str, object]) -> str:
    namespaces = {
        "x": "adobe:ns:meta/",
        "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
        "dc": "http://purl.org/dc/elements/1.1/",
        "photoshop": "http://ns.adobe.com/photoshop/1.0/",
        "Iptc4xmpCore": "http://iptc.org/std/Iptc4xmpCore/1.0/xmlns/",
        "Iptc4xmpExt": "http://iptc.org/std/Iptc4xmpExt/2008-02-29/",
        "exif": "http://ns.adobe.com/exif/1.0/",
        "pc": "https://github.com/turboBasic/PopCircles/ns/1.0/",
    }
    for prefix, uri in namespaces.items():
        ET.register_namespace(prefix, uri)
    xmp = ET.Element(f"{{{namespaces['x']}}}xmpmeta")
    rdf = ET.SubElement(xmp, f"{{{namespaces['rdf']}}}RDF")
    description = ET.SubElement(rdf, f"{{{namespaces['rdf']}}}Description")
    description.set(f"{{{namespaces['rdf']}}}about", "")
    for name, value in properties.items():
        if name == "dc:creator":
            _xmp_creator(description, value, namespaces)
        elif name in {"dc:rights", "dc:title", "dc:description"}:
            _xmp_alt_text(description, name, value, namespaces)
        elif name == "Iptc4xmpCore:CreatorContactInfo":
            _xmp_creator_contact(description, value, namespaces)
        elif name == "Iptc4xmpExt:LocationShown":
            _xmp_location_shown(description, value, namespaces)
        else:
            _xmp_scalar(description, name, value, namespaces)
    return ET.tostring(xmp, encoding="unicode", xml_declaration=True)


def description_of(answer: Answer) -> str:
    return (
        f"Answered from population dataset {answer.provenance.dataset} "
        f"at decimation k={answer.provenance.decimation}"
    )


def metadata_json_fields(
    answer: Answer,
    properties: Mapping[str, object],
) -> dict[str, object]:
    title = cast(str, properties["dc:title"])
    description = cast(str, properties["dc:description"])
    copyright_text = cast(str, properties["fullAttribution"])
    return {
        "Title": title,
        "Description": description,
        "Copyright": copyright_text,
        "XML:com.adobe.xmp": dict(properties),
        "EXIF:GPSInfo": exif_gps_values(answer.circle.centre.lat, answer.circle.centre.lon),
    }


def exif_gps_values(latitude: float, longitude: float) -> dict[str, object]:
    def dms(value: float) -> list[list[int]]:
        total = round(abs(value) * 3_600_000)
        degrees, remainder = divmod(total, 3_600_000)
        minutes, seconds = divmod(remainder, 60_000)
        return [[degrees, 1], [minutes, 1], [seconds, 1000]]

    return {
        "GPSVersionID": [2, 3, 0, 0],
        "GPSLatitudeRef": "N" if latitude >= 0 else "S",
        "GPSLatitude": dms(latitude),
        "GPSLongitudeRef": "E" if longitude >= 0 else "W",
        "GPSLongitude": dms(longitude),
        "GPSMapDatum": "WGS-84",
    }


def exif_gps_packet(latitude: float, longitude: float) -> bytes:
    values = exif_gps_values(latitude, longitude)
    gps = {
        ExifTags.GPS.GPSVersionID: bytes(cast(list[int], values["GPSVersionID"])),
        ExifTags.GPS.GPSLatitudeRef: cast(str, values["GPSLatitudeRef"]),
        ExifTags.GPS.GPSLatitude: tuple(
            IFDRational(numerator, denominator)
            for numerator, denominator in cast(list[list[int]], values["GPSLatitude"])
        ),
        ExifTags.GPS.GPSLongitudeRef: cast(str, values["GPSLongitudeRef"]),
        ExifTags.GPS.GPSLongitude: tuple(
            IFDRational(numerator, denominator)
            for numerator, denominator in cast(list[list[int]], values["GPSLongitude"])
        ),
        ExifTags.GPS.GPSMapDatum: "WGS-84",
    }
    exif = Image.Exif()
    exif[ExifTags.IFD.GPSInfo] = gps
    return exif.tobytes()


def png_metadata(
    title: str,
    copyright_text: str,
    description: str,
    properties: Mapping[str, object],
) -> dict[str, str]:
    return {
        "Title": title,
        "Description": description,
        "Copyright": copyright_text,
        "XML:com.adobe.xmp": xmp_packet(
            {name: value for name, value in properties.items() if value is not None}
        ),
    }


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


def wrap_to_width(figure: Figure, text: str, size: float) -> str:
    canvas = cast(FigureCanvasAgg, figure.canvas)
    renderer = canvas.get_renderer()
    font = FontProperties(size=size)
    max_width = figure.bbox.width * 0.94
    lines: list[str] = []
    line = ""
    for word in text.split():
        candidate = f"{line} {word}".strip()
        width, _, _ = renderer.get_text_width_height_descent(candidate, font, ismath=False)
        if line and width > max_width:
            lines.append(line)
            line = word
        else:
            line = candidate
    if line:
        lines.append(line)
    return "\n".join(lines)


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
        facecolor = COUNTRY if view.country_outline is None else "white"
        axes.add_patch(
            PathPatch(
                DrawPath.make_compound_path(*country_paths),
                facecolor=facecolor,
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
    render_text: tuple[bool, bool] = (True, True),
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
        canvas = cast(FigureCanvasAgg, figure.canvas)
        header_bottom = 0.985
        if render_text[0]:
            annotate(figure, 0.985, wrap_to_width(figure, title, 12.0), 12.0, "top")
            canvas.draw()
            header_bottom = figure.texts[-1].get_window_extent(canvas.get_renderer()).y0
            header_bottom /= figure.bbox.height
        if render_text[1]:
            description_top = header_bottom - 0.015 if render_text[0] else 0.985
            annotate(
                figure,
                description_top,
                wrap_to_width(figure, description_of(answer), 7.0),
                7.0,
                "top",
            )
            canvas.draw()
            header_bottom = figure.texts[-1].get_window_extent(canvas.get_renderer()).y0
            header_bottom /= figure.bbox.height
        top = min(0.87, header_bottom - 0.05) if any(render_text) else 0.98
        bottom = 0.16
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
    render_text = map_text_rendering()
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
        render_text=render_text,
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
    properties = xmp_properties(answer, title, description, copyright_text)
    metadata_values = metadata_json_fields(answer, properties)
    sidecar_path = output_path.with_suffix(".json")
    try:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        # No bbox_inches="tight": the title and citation use figure coordinates; cropping
        # to the drawn content moves them off the positions they were given.
        figure.savefig(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
            output_path,
            dpi=200,
            metadata=png_metadata(title, copyright_text, description, properties),
            pil_kwargs={
                "exif": exif_gps_packet(answer.circle.centre.lat, answer.circle.centre.lon)
            },
        )
        document["render_metadata"] = metadata_values
        sidecar_path.write_text(
            json.dumps(document, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
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
