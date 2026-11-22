import math
from pathlib import Path
from typing import cast

import numpy as np
from matplotlib.colors import LightSource, LinearSegmentedColormap, Normalize
from numpy.typing import NDArray
from rasterio import open as open_raster
from rasterio.crs import CRS
from rasterio.enums import Resampling
from rasterio.errors import WindowError
from rasterio.io import DatasetReader
from rasterio.transform import from_bounds
from rasterio.warp import reproject, transform_bounds
from rasterio.windows import Window
from rasterio.windows import from_bounds as window_from_bounds

from population_circles.dataset_registry import REPO_ROOT, TerrainRaster
from population_circles.map_frame import Frame

TARGET_HEIGHT = 1440
MIN_WIDTH = 720
MAX_WIDTH = 1800
DESTINATION_NODATA = -99999.0
HILLSHADE_AZIMUTH = 315
HILLSHADE_ALTITUDE = 45
HILLSHADE_ALPHA = 0.34
NEGATIVE_ELEVATION_STOP: tuple[float, tuple[int, int, int]] = (-100, (238, 241, 208))

ELEVATION_STOPS: tuple[tuple[float, tuple[int, int, int]], ...] = (
    (0, (229, 238, 196)),
    (100, (220, 235, 184)),
    (250, (204, 226, 160)),
    (400, (185, 216, 140)),
    (600, (166, 202, 122)),
    (800, (154, 185, 110)),
    (1000, (181, 190, 120)),
    (1500, (219, 209, 143)),
    (2200, (201, 167, 122)),
    (3000, (170, 129, 104)),
    (4000, (135, 111, 112)),
    (5000, (148, 127, 134)),
    (6500, (196, 190, 193)),
    (7600, (226, 221, 218)),
    (8848, (250, 248, 242)),
)


def palette() -> tuple[LinearSegmentedColormap, Normalize]:
    minimum, maximum = NEGATIVE_ELEVATION_STOP[0], ELEVATION_STOPS[-1][0]
    scale = Normalize(vmin=minimum, vmax=maximum, clip=True)
    stops = (NEGATIVE_ELEVATION_STOP, *ELEVATION_STOPS)
    colours = LinearSegmentedColormap.from_list(
        "popcircles-terrain",
        [(scale(elevation), tuple(channel / 255 for channel in rgb)) for elevation, rgb in stops],
        N=256,
    )
    return colours, scale


def paint(
    elevation: NDArray[np.float32],
    valid: NDArray[np.bool_],
    bounds: tuple[float, float, float, float],
) -> NDArray[np.uint8]:
    rgba = np.zeros((*elevation.shape, 4), dtype=np.uint8)
    colours, scale = palette()
    rgba[valid] = np.rint(colours(scale(elevation[valid])) * 255).astype(np.uint8)

    xmin, ymin, xmax, ymax = bounds
    shade = LightSource(azdeg=HILLSHADE_AZIMUTH, altdeg=HILLSHADE_ALTITUDE).hillshade(
        np.where(valid, elevation, 0.0),
        vert_exag=1.0,
        dx=(xmax - xmin) / elevation.shape[1],
        dy=(ymax - ymin) / elevation.shape[0],
        fraction=1.0,
    )
    rgba[valid, :3] = np.rint(
        rgba[valid, :3].astype(np.float32) * (1 - HILLSHADE_ALPHA)
        + shade[valid, None] * 255 * HILLSHADE_ALPHA
    ).astype(np.uint8)
    return rgba


def _source_path(dataset: TerrainRaster, root: Path) -> str:
    path = dataset.file(root).resolve()
    if dataset.archive_member is None:
        return path.as_posix()
    return f"zip://{path.as_posix()}!{dataset.archive_member}"


def _check_source(dataset: TerrainRaster, source: DatasetReader) -> None:
    # Rasterio leaves the CRS unset for ESRI ASCII Grid; in that one format the registered publisher
    # metadata supplies it. Everything else must agree with the checksum-pinned registry row.
    if (source.width, source.height) != (dataset.width, dataset.height):
        message = (
            f"terrain source {dataset.path!r} has dimensions that differ from its registry row"
        )
        raise ValueError(message)
    if source.nodata != dataset.nodata:
        message = f"terrain source {dataset.path!r} has nodata that differs from its registry row"
        raise ValueError(message)
    # ASCII Grid sources may omit CRS metadata even though the Rasterio stubs mark it present.
    actual_crs = cast(CRS | None, source.crs)
    if actual_crs is not None and actual_crs.to_epsg() != dataset.epsg:
        message = f"terrain source {dataset.path!r} has a CRS that differs from its registry row"
        raise ValueError(message)
    transform = source.transform
    actual = (transform.c, transform.f, transform.a, transform.e)
    expected = (dataset.x_origin, dataset.y_origin, dataset.x_step, dataset.y_step)
    matches_transform = all(
        math.isclose(left, right, rel_tol=0.0, abs_tol=1e-9)
        for left, right in zip(actual, expected, strict=True)
    )
    if not matches_transform:
        message = (
            f"terrain source {dataset.path!r} has a transform that differs from its registry row"
        )
        raise ValueError(message)


def prepare_terrain(
    dataset: TerrainRaster,
    frame: Frame,
    *,
    root: Path = REPO_ROOT,
    include_below_sea_level: bool = False,
) -> tuple[NDArray[np.uint8], tuple[float, float, float, float]]:
    bounds = frame.horizon.bounds
    xmin, ymin, xmax, ymax = bounds
    width = max(MIN_WIDTH, min(MAX_WIDTH, round(TARGET_HEIGHT * (xmax - xmin) / (ymax - ymin))))
    height = TARGET_HEIGHT
    destination_transform = from_bounds(xmin, ymin, xmax, ymax, width, height)
    elevation = np.full((height, width), DESTINATION_NODATA, dtype=np.float32)
    path = dataset.file(root)
    if not path.is_file():
        message = f"terrain source is missing at {path}; run `mise run data:get`"
        raise ValueError(message)

    with open_raster(_source_path(dataset, root)) as source:
        _check_source(dataset, source)
        source_crs = cast(CRS | None, source.crs) or CRS.from_epsg(dataset.epsg)
        destination_crs = CRS.from_string(frame.crs.to_string())
        projected_bounds = transform_bounds(
            destination_crs,
            source_crs,
            xmin,
            ymin,
            xmax,
            ymax,
            densify_pts=41,
        )
        west, south, east, north = projected_bounds
        longitude_ranges = (
            ((west, east),)
            if west <= east
            else ((west, source.bounds.right), (source.bounds.left, east))
        )
        windows: list[Window] = []
        for left, right in longitude_ranges:
            try:
                window = (
                    window_from_bounds(left, south, right, north, transform=source.transform)
                    .round_offsets()
                    .round_lengths()
                    .intersection(Window(0, 0, source.width, source.height))
                )
            except WindowError:
                continue
            if window.width > 0 and window.height > 0:
                windows.append(window)
        if not windows:
            message = f"terrain source {dataset.path!r} does not overlap the map frame"
            raise ValueError(message)
        for window in windows:
            pixels = source.read(1, window=window)
            piece = np.full_like(elevation, DESTINATION_NODATA)
            reproject(
                source=pixels,
                destination=piece,
                src_transform=source.window_transform(window),
                src_crs=source_crs,
                src_nodata=source.nodata,
                dst_transform=destination_transform,
                dst_crs=destination_crs,
                dst_nodata=DESTINATION_NODATA,
                resampling=Resampling.bilinear,
            )
            valid_piece = np.isfinite(piece) & (piece != DESTINATION_NODATA)
            elevation[valid_piece] = piece[valid_piece]

    valid = np.isfinite(elevation) & (elevation != DESTINATION_NODATA)
    if not include_below_sea_level:
        valid &= elevation > 0
    return paint(elevation, valid, bounds), bounds
