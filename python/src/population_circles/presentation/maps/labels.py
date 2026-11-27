import math
import textwrap
from collections.abc import Sequence
from typing import Any, Literal, cast

import matplotlib.patheffects as path_effects
from matplotlib.axes import Axes
from matplotlib.backends.backend_agg import FigureCanvasAgg
from matplotlib.text import Text
from matplotlib.transforms import Bbox
from shapely import get_coordinates, make_valid, maximum_inscribed_circle
from shapely.geometry import LineString, Point, box
from shapely.geometry.base import BaseGeometry
from shapely.ops import unary_union

from population_circles.presentation.maps.frame import Frame, project

PLACE_LAYER = 2.85
MAP_OUTPUT_DPI = 200
MIN_COUNTRY_LABEL_LAND_SHARE = 0.001
COUNTRY_LABEL_WRAP_WIDTH = 12
COUNTRY_LABEL_WRAP_MIN_LENGTH = 16
COUNTRY_LABEL_OUTLINE_PX = 2


def place_city_labels(
    axes: Axes,
    labels: list[tuple[Text, float, float, bool]],
) -> int:
    canvas = cast(FigureCanvasAgg, axes.figure.canvas)
    canvas.draw()
    renderer = canvas.get_renderer()
    label_ids = {id(label) for label, _, _, _ in labels}
    protected = [
        label.get_window_extent(renderer)
        for label in axes.texts
        if id(label) not in label_ids and label.get_zorder() <= PLACE_LAYER
    ]
    marker_padding = 2 * renderer.dpi / 72
    marker_boxes: list[Bbox] = []
    retained_anchors: list[tuple[float, float]] = []
    retained_indices: set[int] = set()
    label_spacing = 12 * renderer.dpi / 72
    for _, x, y, _ in labels:
        marker_x, marker_y = axes.transData.transform((x, y))
        marker_boxes.append(
            Bbox.from_bounds(
                marker_x - marker_padding,
                marker_y - marker_padding,
                marker_padding * 2,
                marker_padding * 2,
            ),
        )
    for label_index, (_, x, y, capital) in enumerate(labels):
        coordinates = axes.transData.transform((x, y))
        anchor = float(coordinates[0]), float(coordinates[1])
        if not capital and any(
            math.hypot(anchor[0] - placed_x, anchor[1] - placed_y) < label_spacing
            for placed_x, placed_y in retained_anchors
        ):
            continue
        retained_anchors.append(anchor)
        retained_indices.add(label_index)
    axes_box = axes.bbox
    directions = ((1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1))
    candidates: list[
        tuple[
            int,
            int,
            Literal["left", "center", "right"],
            Literal["bottom", "center", "top"],
        ]
    ] = [
        (
            round(distance * dx),
            round(distance * dy),
            "left" if dx > 0 else "right" if dx < 0 else "center",
            "bottom" if dy > 0 else "top" if dy < 0 else "center",
        )
        for distance in (4, 6, 8)
        for dx, dy in directions
    ]
    candidates.sort(key=lambda candidate: candidate[0] ** 2 + candidate[1] ** 2)
    placed: list[Bbox] = []
    for label_index, (label, x, y, _) in enumerate(labels):
        if label_index not in retained_indices:
            continue
        for dx, dy, horizontal, vertical in candidates:
            label.set_position((dx, dy))
            label.set_horizontalalignment(horizontal)
            label.set_verticalalignment(vertical)
            label.set_visible(True)
            bounds = label.get_window_extent(renderer).padded(renderer.dpi / 72)
            collides = any(bounds.overlaps(other) for other in (*protected, *placed)) or any(
                bounds.overlaps(marker)
                for index, marker in enumerate(marker_boxes)
                if index != label_index
            )
            if not (
                axes_box.contains(*bounds.min) and axes_box.contains(*bounds.max) and not collides
            ):
                label.set_visible(False)
                continue
            placed.append(bounds)
            if dx**2 + dy**2 > 6**2:
                axes.annotate(  # pyright: ignore[reportUnknownMemberType] — Matplotlib kwargs: Unknown
                    "",
                    xy=(x, y),
                    xytext=(dx, dy),
                    textcoords="offset points",
                    arrowprops={
                        "arrowstyle": "-",
                        "color": "#68645f",
                        "linewidth": 0.45,
                        "shrinkA": 1.5,
                        "shrinkB": 2.5,
                    },
                    zorder=PLACE_LAYER - 0.1,
                )
            break
        else:
            label.set_visible(False)
    return sum(label.get_visible() for label, _, _, _ in labels)


def _visible_country_geometry(view: Frame, geometry: BaseGeometry) -> BaseGeometry | None:
    projected = project(view, geometry)
    if not all(math.isfinite(value) for point in get_coordinates(projected) for value in point):
        return None
    if not projected.is_valid:
        projected = make_valid(projected)
    visible = projected.intersection(view.horizon)
    return None if visible.is_empty else visible


def draw_country_labels(
    axes: Axes,
    view: Frame,
    labels: Sequence[tuple[BaseGeometry, str]],
) -> None:
    canvas = cast(FigureCanvasAgg, axes.figure.canvas)
    canvas.draw()
    renderer = canvas.get_renderer()
    axes_box = axes.bbox
    inverse = axes.transData.inverted()
    target = view.country_outline
    visible_land = [target] if target is not None else []
    countries: list[tuple[BaseGeometry, str, Point]] = []
    for geometry, name in labels:
        visible = _visible_country_geometry(view, geometry)
        if visible is None:
            continue
        visible_land.append(visible)
        inscribed = cast(LineString, maximum_inscribed_circle(cast(Any, visible)))
        anchor = (
            Point(*inscribed.coords[0])
            if not inscribed.is_empty
            else visible.representative_point()
        )
        countries.append((visible, name, anchor))
    land_area = unary_union(visible_land).intersection(view.horizon).area
    countries_with_share = [
        (visible.area / land_area, visible, name, anchor)
        for visible, name, anchor in countries
        if land_area > 0 and visible.area / land_area >= MIN_COUNTRY_LABEL_LAND_SHARE
    ]
    countries_with_share.sort(key=lambda country: (-country[0], country[2]))
    placed: list[Bbox] = []
    for _, visible, name, anchor in countries_with_share:
        label = axes.annotate(  # pyright: ignore[reportUnknownMemberType] — Matplotlib kwargs: Unknown
            textwrap.fill(name, width=COUNTRY_LABEL_WRAP_WIDTH)
            if len(name) >= COUNTRY_LABEL_WRAP_MIN_LENGTH
            else name,
            (anchor.x, anchor.y),
            xytext=(0, 0),
            textcoords="offset points",
            visible=False,
            clip_on=True,
            fontsize=8,
            color="#454545",
            path_effects=[
                path_effects.withStroke(
                    linewidth=72 * COUNTRY_LABEL_OUTLINE_PX / MAP_OUTPUT_DPI,
                    foreground="white",
                ),
            ],
            zorder=PLACE_LAYER,
            horizontalalignment="center",
            verticalalignment="center",
        )
        best: tuple[float, Bbox, tuple[float, float]] | None = None
        offsets = [
            (float(x), float(y))
            for radius in (0, 4, 8, 12, 16, 20, 24, 28, 32)
            for x in range(-radius, radius + 1, 4)
            for y in range(-radius, radius + 1, 4)
            if max(abs(x), abs(y)) == radius
        ]
        for offset in offsets:
            label.set_position(offset)
            label.set_visible(True)
            bounds = label.get_window_extent(renderer)
            padded = bounds.padded(renderer.dpi / 72)
            if not axes_box.contains(*padded.min) or not axes_box.contains(*padded.max):
                continue
            if any(padded.overlaps(other) for other in placed):
                continue
            corners = inverse.transform(((bounds.x0, bounds.y0), (bounds.x1, bounds.y1)))
            label_box = box(*corners[0], *corners[1])
            target_overlap = (
                label_box.intersection(target).area / label_box.area if target is not None else 0.0
            )
            country_overlap = label_box.intersection(visible).area / label_box.area
            centre_inside_country = visible.covers(label_box.centroid)
            displacement = math.hypot(*offset)
            # Keep the label centred on its country when it fits, then limit overlap with China.
            score = (
                displacement
                + target_overlap * 8
                - country_overlap * 40
                - (32 if centre_inside_country else 0)
            )
            candidate = (score, padded, offset)
            if best is None or candidate[0] < best[0]:
                best = candidate
        if best is None:
            label.set_visible(False)
        else:
            _, bounds, offset = best
            label.set_position(offset)
            placed.append(bounds)
