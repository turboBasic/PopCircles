import json
import math
from collections.abc import Callable, Mapping, Sequence
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
from matplotlib.text import Text
from shapely import get_coordinates
from shapely.geometry import MultiLineString, Point
from shapely.geometry.base import BaseGeometry
from shapely.geometry.polygon import Polygon

from population_circles.application.use_cases.render_map import PreparedMapRender
from population_circles.documents.result_document import Answer
from population_circles.domain.circle_geometry import cap, linestrings, polygons
from population_circles.domain.geography import GeographicCoordinate
from population_circles.infrastructure.geodata.dataset_registry import TerrainRaster
from population_circles.infrastructure.geodata.map_layers import MapLayers
from population_circles.presentation.maps import metadata as map_metadata
from population_circles.presentation.maps.frame import (
    CountryFrameSettings,
    Frame,
    country_frame,
    frame,
    graticule,
    project,
)
from population_circles.presentation.maps.labels import (
    MAP_OUTPUT_DPI,
    PLACE_LAYER,
    draw_country_labels,
    place_city_labels,
)
from population_circles.presentation.maps.presentation_config import (
    CountryMap,
)
from population_circles.presentation.maps.presentation_config import (
    load as load_presentation_config,
)
from population_circles.presentation.maps.terrain import prepare_terrain

# Every figure this writes goes to a file, so an interactive backend is never wanted — and asking
# for one is what fails on a machine with no display, rather than falling back.
mpl.use("Agg")


class MissingCountryOutlineError(ValueError):
    def __init__(self) -> None:
        super().__init__("country result rendering requires its registered boundary outline")


# Bottom to top: coastlines, country fills, graticule, circle, its centre, and the frame's own
# outline last so no fill reaching the limb draws over it.
COASTLINE_LAYER = 2
COUNTRY_LAYER = 2.5
GRATICULE_LAYER = 2.75
WATER_LAYER = 2.6
ROAD_LAYER = 2.65
MIN_WORLD_CITY_POPULATION = 5_000_000
CIRCLE_LAYER = 3
OCEAN = "#DFFFFD"
COUNTRY = "#65CBC1"
CONTEXT_COUNTRY_FILL = "#F4F4F4"
LAKE = "#9BD8E5"
RIVER = "#65B7CE"
ROAD = "#B56F52"
OUTLINE_WIDTH = 0.6
CENTRE_LAYER = 4
HORIZON_LAYER = 5
TERRAIN_LAYER = 1
COUNTRY_FILL_ALPHA = 0.14
MAP_OUTPUT_WIDTH = 11.0 * MAP_OUTPUT_DPI
MAP_OUTPUT_HEIGHT = 6.0 * MAP_OUTPUT_DPI
MAP_OUTPUT_AREA = MAP_OUTPUT_WIDTH * MAP_OUTPUT_HEIGHT


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


def title_of(answer: Answer, label: str | None = None) -> str:
    circle = answer.circle
    country = f"{answer.country.name}: " if answer.country else ""
    centre = f"{circle.centre.lat:.4f}, {circle.centre.lon:.4f}"
    population_percent = math.floor(answer.caption.share * 100 + 0.5)
    subject = label or f"{circle.radius_km:,.0f} km circle"
    title = (
        f"{country}{subject} holding "
        f"{population_percent}% of the population "
        f"({answer.caption.population:,.0f} people); Center: {centre}"
    )
    if answer.caption.territory_share is not None:
        territory = answer.country.name if answer.country else "Earth\u2019s land area"
        percentage = answer.caption.territory_share * 100
        decimals = max(0, 1 - math.floor(math.log10(percentage))) if percentage else 0
        percent = f"{percentage:.{decimals}f}"
        title += f"; Circle covers {percent}% of {territory}"
    return title


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


def other_country_labels(
    boundaries: Mapping[str, BaseGeometry] | None, answer: Answer
) -> list[tuple[BaseGeometry, str]]:
    if boundaries is None or answer.country is None:
        return []
    names = cast(Mapping[str, str], getattr(boundaries, "names", {}))
    return [
        (geometry, names[identifier])
        for identifier, geometry in boundaries.items()
        if identifier != answer.country.id and identifier in names
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


def figure_size(
    bounds: tuple[float, float, float, float], *, country_map: bool
) -> tuple[float, float]:
    if not country_map:
        return 11.0, 6.0
    west, south, east, north = bounds
    aspect = (east - west) / (north - south)
    width = math.sqrt(MAP_OUTPUT_AREA * aspect)
    height = math.sqrt(MAP_OUTPUT_AREA / aspect)
    if width > MAP_OUTPUT_WIDTH:
        width = MAP_OUTPUT_WIDTH
        height = width / aspect
    return width / MAP_OUTPUT_DPI, height / MAP_OUTPUT_DPI


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


def _projected_context(
    view: Frame,
    geometries: Sequence[BaseGeometry],
    country_clip: BaseGeometry | None,
) -> list[BaseGeometry]:
    projected: list[BaseGeometry] = []
    for geometry in geometries:
        clipped = geometry
        if country_clip is not None:
            if not clipped.intersects(country_clip):
                continue
            clipped = clipped.intersection(country_clip)
        item = project(view, clipped)
        if all(math.isfinite(value) for point in get_coordinates(item) for value in point):
            projected.append(item)
    return projected


def draw_context(
    axes: Axes,
    view: Frame,
    layers: MapLayers,
    country_clip: BaseGeometry | None,
) -> None:
    lake_paths = [
        compound(polygon)
        for geometry in _projected_context(view, layers.lakes, country_clip)
        for polygon in polygons(geometry)
    ]
    if lake_paths:
        axes.add_patch(
            PathPatch(
                DrawPath.make_compound_path(*lake_paths),
                facecolor=LAKE,
                edgecolor=RIVER,
                linewidth=0.25,
                zorder=WATER_LAYER,
            ),
        )
    for geometries, colour, width, layer in (
        (layers.rivers, RIVER, 0.45, WATER_LAYER),
        (layers.roads if country_clip is not None else (), ROAD, 0.65, ROAD_LAYER),
    ):
        lines = [
            line
            for geometry in _projected_context(view, geometries, country_clip)
            for line in linestrings(geometry)
            if not line.is_empty
        ]
        if lines:
            stroke(
                axes,
                MultiLineString([list(line.coords) for line in lines]),
                colour,
                width,
                layer,
            )
    if country_clip is not None:
        places = [place for place in layers.places if country_clip.covers(place[0])][:10]
    else:
        places = [place for place in layers.places if place[2] >= MIN_WORLD_CITY_POPULATION]
    if places:
        labels: list[tuple[Text, float, float, bool]] = []
        for geometry, name, _, capital in places:
            point = cast(Point, project(view, geometry))
            if point.is_empty or not all(math.isfinite(value) for value in point.coords[0]):
                continue
            axes.scatter(  # pyright: ignore[reportUnknownMemberType] — Matplotlib kwargs: Unknown
                [point.x],
                [point.y],
                s=15 if capital else 8,
                color="#59435f" if capital else "#545454",
                zorder=PLACE_LAYER,
            )
            label = axes.annotate(  # pyright: ignore[reportUnknownMemberType] — Matplotlib kwargs: Unknown
                name,
                (point.x, point.y),
                xytext=(0, 0),
                textcoords="offset points",
                fontsize=7 if not capital else 8,
                color="#454545",
                weight="bold" if capital else "normal",
                zorder=PLACE_LAYER,
                visible=False,
            )
            labels.append((label, point.x, point.y, capital))
        place_city_labels(axes, labels)


def draw(  # noqa: PLR0913, PLR0917 — map layers and country clipping are distinct display inputs
    axes: Axes,
    view: Frame,
    coastline: BaseGeometry | None,
    other_countries: Sequence[BaseGeometry] = (),
    map_layers: MapLayers | None = None,
    country_clip: BaseGeometry | None = None,
    country_labels: Sequence[tuple[BaseGeometry, str]] = (),
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
                facecolor=(COUNTRY if view.country_outline is None else CONTEXT_COUNTRY_FILL),
                alpha=COUNTRY_FILL_ALPHA if view.country_outline is None else 1.0,
                edgecolor="0.4",
                linewidth=OUTLINE_WIDTH,
                zorder=COUNTRY_LAYER,
            ),
        )
    if country_labels:
        draw_country_labels(axes, view, country_labels)
    if view.country_outline is not None:
        for polygon in polygons(view.country_outline):
            axes.add_patch(
                PathPatch(
                    compound(polygon),
                    facecolor=COUNTRY,
                    alpha=COUNTRY_FILL_ALPHA,
                    edgecolor="0.4",
                    linewidth=1,
                    zorder=COUNTRY_LAYER,
                ),
            )
    if map_layers is not None:
        draw_context(axes, view, map_layers, country_clip)
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


def draw_map_layers(  # noqa: PLR0913, PLR0917 — each argument describes an independent map layer
    axes: Axes,
    view: Frame,
    coastline: BaseGeometry | None,
    other_countries: Sequence[BaseGeometry],
    map_layers: MapLayers | None,
    country_clip: BaseGeometry | None,
    country_labels: Sequence[tuple[BaseGeometry, str]],
) -> None:
    fill_ocean(axes, view)
    if map_layers is not None:
        draw_image = cast(Callable[..., AxesImage], axes.imshow)  # pyright: ignore[reportUnknownMemberType] — Matplotlib kwargs: Unknown
        west, south, east, north = view.horizon.bounds

        def draw_terrain(
            dataset: TerrainRaster | Sequence[TerrainRaster],
            *,
            include_below_sea_level: bool = False,
        ) -> AxesImage:
            if include_below_sea_level:
                image, _ = prepare_terrain(
                    dataset,
                    view,
                    root=map_layers.root,
                    include_below_sea_level=True,
                )
            else:
                image, _ = prepare_terrain(
                    dataset,
                    view,
                    root=map_layers.root,
                )
            return draw_image(
                image,
                extent=(west, east, south, north),
                origin="upper",
                interpolation="bilinear",
                zorder=TERRAIN_LAYER,
            )

        def clip_to_country(image: AxesImage) -> None:
            if country_clip is None or view.country_outline is None:
                return
            paths = [compound(polygon) for polygon in polygons(view.country_outline)]
            if paths:
                image.set_clip_path(
                    PathPatch(
                        DrawPath.make_compound_path(*paths),
                        transform=axes.transData,
                    )
                )

        if map_layers.sources.terrain_overlays:
            terrain = draw_terrain(
                map_layers.sources.terrain_overlays,
                include_below_sea_level=country_clip is not None,
            )
        else:
            terrain = draw_terrain(map_layers.sources.terrain)
        clip_to_country(terrain)
    draw(axes, view, coastline, other_countries, map_layers, country_clip, country_labels)


def render(  # noqa: PLR0913 — distinct geometry and display inputs stay explicit
    answer: Answer,
    projection: str,
    *,
    # A caller drawing several figures unions the coastline once; `None` omits it.
    coastline: BaseGeometry | None,
    country_outline: BaseGeometry | None = None,
    other_countries: Sequence[BaseGeometry] = (),
    country_labels: Sequence[tuple[BaseGeometry, str]] = (),
    render_text: tuple[bool, bool] = (True, True),
    map_layers: MapLayers | None = None,
    title_label: str | None = None,
) -> Figure:
    if answer.country is not None and country_outline is None:
        raise MissingCountryOutlineError
    circle = answer.circle
    centre = GeographicCoordinate(lat=circle.centre.lat, lon=circle.centre.lon)
    built = cap(centre, circle.radius_km, circle.earth_radius_km)
    if answer.country is not None:
        country_outline, frame_settings = country_map_display(answer.country.id, country_outline)
    else:
        frame_settings = None
    view = (
        country_frame(built, country_outline, frame_settings)
        if country_outline is not None
        else frame(projection, built)
    )
    map_attributions = map_layers.sources.attributions if map_layers is not None else ()
    centre = view.to_frame.transform(circle.centre.lon, circle.centre.lat)

    figure = plt.figure(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
        figsize=figure_size(view.horizon.bounds, country_map=answer.country is not None),
    )
    try:
        title = title_of(answer, title_label)
        footer = map_metadata.visible_credits(map_attributions)
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
                wrap_to_width(figure, map_metadata.description_of(answer), 7.0),
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
        draw_map_layers(
            axes,
            view,
            coastline,
            other_countries,
            map_layers,
            country_outline,
            country_labels,
        )
        axes.plot(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
            [centre[0]],
            [centre[1]],
            marker="+",
            color="black",
            markersize=9,
            zorder=CENTRE_LAYER,
        )
        annotate(figure, 0.04, wrap_to_width(figure, footer, 6.5), 6.5, "bottom")
    except BaseException:
        plt.close(figure)
        raise
    return figure


def country_map_display(
    country_id: str, outline: BaseGeometry | None
) -> tuple[BaseGeometry, CountryFrameSettings]:
    if outline is None:
        raise MissingCountryOutlineError
    config = load_presentation_config()
    settings = config.country_maps.get(country_id)
    if not isinstance(settings, CountryMap):
        return outline, CountryFrameSettings(projection=config.default_projection)
    extent = None
    if settings.extent is not None:
        extent = (
            settings.extent.x_min,
            settings.extent.y_min,
            settings.extent.x_max,
            settings.extent.y_max,
        )
    return outline, CountryFrameSettings(
        projection=settings.projection,
        projection_parameters=settings.projection_parameters,
        extent=extent,
    )


def render_prepared(prepared: PreparedMapRender) -> None:
    map_layers = prepared.map_layers
    if not isinstance(map_layers, MapLayers):
        message = "prepared map layers must be MapLayers"
        raise TypeError(message)
    figure = render(
        prepared.answer,
        prepared.projection,
        coastline=prepared.coastline,
        country_outline=prepared.country_outline,
        other_countries=prepared.other_countries,
        render_text=prepared.render_text,
        map_layers=map_layers,
        country_labels=prepared.country_labels,
        title_label=prepared.title_label,
    )
    title = title_of(prepared.answer, prepared.title_label)
    copyright_text = map_metadata.copyright_of(
        prepared.answer,
        prepared.population_attribution,
        prepared.coastline_attribution,
        prepared.country_boundary_attribution,
        prepared.source_attributions,
    )
    description = map_metadata.description_of(prepared.answer)
    properties = map_metadata.xmp_properties(
        prepared.answer,
        title,
        description,
        copyright_text,
        prepared.source_attributions,
    )
    metadata_values = map_metadata.metadata_json_fields(prepared.answer, properties)
    sidecar_path = prepared.output_path.with_suffix(".json")
    try:
        prepared.output_path.parent.mkdir(parents=True, exist_ok=True)
        # No bbox_inches="tight": the title and citation use figure coordinates; cropping
        # to the drawn content moves them off the positions they were given.
        figure.savefig(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
            prepared.output_path,
            dpi=MAP_OUTPUT_DPI,
            metadata=map_metadata.png_metadata(title, copyright_text, description, properties),
            pil_kwargs={
                "exif": map_metadata.exif_gps_packet(
                    prepared.answer.circle.centre.lat, prepared.answer.circle.centre.lon
                )
            },
        )
        prepared.document["render_metadata"] = metadata_values
        sidecar_path.write_text(
            json.dumps(prepared.document, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
        )
    finally:
        plt.close(figure)
