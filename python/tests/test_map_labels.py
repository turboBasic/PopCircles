from typing import cast

from matplotlib import pyplot as plt
from matplotlib.backends.backend_agg import FigureCanvasAgg
from matplotlib.text import Text
from pyproj import CRS, Transformer
from shapely.geometry import Point, box

from population_circles import map_labels
from population_circles.map_frame import Frame


def test_city_labels_keep_priority_and_do_not_overlap() -> None:
    figure, axes = plt.subplots(figsize=(4, 3))
    axes.set_xlim(0, 10)
    axes.set_ylim(0, 10)
    labels: list[tuple[Text, float, float, bool]] = []
    for name, x, y, capital in (
        ("Capital", 5.0, 5.0, True),
        ("Nearby", 5.01, 5.0, False),
        ("Distant", 8.0, 8.0, False),
    ):
        label = cast(
            Text,
            axes.annotate(  # pyright: ignore[reportUnknownMemberType] — Matplotlib kwargs: Unknown
                name, (x, y), visible=False
            ),
        )
        labels.append((label, x, y, capital))

    try:
        expected_visible_labels = 2
        assert map_labels.place_city_labels(axes, labels) == expected_visible_labels
        assert labels[0][0].get_visible()
        assert not labels[1][0].get_visible()
        assert labels[2][0].get_visible()
        canvas = cast(FigureCanvasAgg, figure.canvas)
        renderer = canvas.get_renderer()
        bounds = [
            label.get_window_extent(renderer).padded(renderer.dpi / 72)
            for label, *_ in labels
            if label.get_visible()
        ]
        assert all(
            axes.bbox.contains(*bound.min) and axes.bbox.contains(*bound.max) for bound in bounds
        )
        assert not bounds[0].overlaps(bounds[1])
    finally:
        plt.close(figure)


def test_city_labels_avoid_existing_low_layer_text() -> None:
    figure, axes = plt.subplots(figsize=(4, 3))
    axes.set_xlim(0, 10)
    axes.set_ylim(0, 10)
    protected = cast(
        Text,
        axes.annotate(  # pyright: ignore[reportUnknownMemberType] — Matplotlib kwargs: Unknown
            "Existing label",
            (5, 5),
            horizontalalignment="center",
            verticalalignment="center",
            zorder=map_labels.PLACE_LAYER,
        ),
    )
    label = cast(
        Text,
        axes.annotate("City", (5, 5), visible=False),  # pyright: ignore[reportUnknownMemberType] — Matplotlib kwargs: Unknown
    )

    try:
        assert map_labels.place_city_labels(axes, [(label, 5, 5, False)]) == 1
        assert label.get_visible()
        renderer = cast(FigureCanvasAgg, figure.canvas).get_renderer()
        label_bounds = label.get_window_extent(renderer).padded(renderer.dpi / 72)
        assert not label_bounds.overlaps(protected.get_window_extent(renderer))
    finally:
        plt.close(figure)


def test_country_labels_filter_by_land_share_sort_and_wrap_names() -> None:
    figure, axes = plt.subplots(figsize=(8, 4))
    axes.set_xlim(-100, 100)
    axes.set_ylim(-50, 50)
    world = box(-180, -90, 180, 90)
    crs = CRS.from_epsg(4326)
    view = Frame(
        crs=crs,
        to_frame=Transformer.from_crs(crs, crs, always_xy=True),
        visible=world,
        circle=Point(0, 0),
        horizon=world,
        geographic_footprint=world,
    )
    labels = [
        (box(40, 0, 50, 10), "Beta"),
        (box(-50, 0, -40, 10), "Alpha Long Country"),
        (box(70, 0, 90, 20), "Zulu"),
        (box(-80, 0, -79.9, 0.1), "Tiny"),
    ]

    try:
        map_labels.draw_country_labels(axes, view, labels)
        texts = axes.texts
        assert [text.get_text() for text in texts] == [
            "Zulu",
            "Alpha Long\nCountry",
            "Beta",
        ]
        renderer = cast(FigureCanvasAgg, figure.canvas).get_renderer()
        bounds = [text.get_window_extent(renderer).padded(renderer.dpi / 72) for text in texts]
        assert all(
            axes.bbox.contains(*bound.min) and axes.bbox.contains(*bound.max) for bound in bounds
        )
        assert all(text.get_visible() for text in texts)
        assert all(
            not first.overlaps(second)
            for index, first in enumerate(bounds)
            for second in bounds[index + 1 :]
        )
    finally:
        plt.close(figure)
