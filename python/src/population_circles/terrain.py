import math
from collections.abc import Sequence
from contextlib import ExitStack
from pathlib import Path
from typing import cast

import numpy as np
from matplotlib.colors import LightSource, LinearSegmentedColormap, Normalize
from numpy.typing import NDArray
from rasterio import open as open_raster
from rasterio.crs import CRS
from rasterio.enums import Resampling
from rasterio.io import DatasetReader
from rasterio.merge import merge
from rasterio.transform import from_bounds
from rasterio.warp import reproject, transform_bounds

from population_circles.dataset_registry import REPO_ROOT, TerrainRaster
from population_circles.map_frame import Frame

TARGET_HEIGHT = 1440
MIN_WIDTH = 720
MAX_WIDTH = 1800
GEOGRAPHIC_CRS = CRS.from_epsg(4326)
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
    dataset: TerrainRaster | Sequence[TerrainRaster],
    frame: Frame,
    *,
    root: Path = REPO_ROOT,
    include_below_sea_level: bool = False,
) -> tuple[NDArray[np.uint8], tuple[float, float, float, float]]:
    datasets = (dataset,) if isinstance(dataset, TerrainRaster) else tuple(dataset)
    if not datasets:
        message = "at least one terrain source is required"
        raise ValueError(message)
    bounds = frame.horizon.bounds
    xmin, ymin, xmax, ymax = bounds
    width = max(MIN_WIDTH, min(MAX_WIDTH, round(TARGET_HEIGHT * (xmax - xmin) / (ymax - ymin))))
    height = TARGET_HEIGHT
    pixel_width = (xmax - xmin) / width
    pixel_height = (ymax - ymin) / height
    padded_bounds = (
        xmin - pixel_width,
        ymin - pixel_height,
        xmax + pixel_width,
        ymax + pixel_height,
    )
    destination_transform = from_bounds(*padded_bounds, width + 2, height + 2)
    elevation = np.full((height + 2, width + 2), DESTINATION_NODATA, dtype=np.float32)
    paths = [dataset.file(root) for dataset in datasets]
    for path in paths:
        if not path.is_file():
            message = f"terrain source is missing at {path}; run `mise run data:get`"
            raise ValueError(message)

    with ExitStack() as stack:
        sources = [stack.enter_context(open_raster(_source_path(item, root))) for item in datasets]
        for item, source in zip(datasets, sources, strict=True):
            _check_source(item, source)
        source_crs = cast(CRS | None, sources[0].crs) or CRS.from_epsg(datasets[0].epsg)
        projected_bounds = transform_bounds(
            GEOGRAPHIC_CRS,
            source_crs,
            *frame.visible.bounds,
            densify_pts=41,
        )
        west, south, east, north = projected_bounds
        source_west = min(source.bounds.left for source in sources)
        source_east = max(source.bounds.right for source in sources)
        longitude_ranges = (
            ((west, east),) if west <= east else ((west, source_east), (source_west, east))
        )
        ranges = [
            (left, right)
            for left, right in longitude_ranges
            if left < right
            and any(
                left < source.bounds.right
                and right > source.bounds.left
                and south < source.bounds.top
                and north > source.bounds.bottom
                for source in sources
            )
        ]
        if not ranges:
            names = ", ".join(dataset.path for dataset in datasets)
            message = f"terrain sources {names!r} do not overlap the map frame"
            raise ValueError(message)
        destination_crs = CRS.from_string(frame.crs.to_string())
        for left, right in ranges:
            mosaic, mosaic_transform = merge(
                sources,
                bounds=(left, south, right, north),
                nodata=DESTINATION_NODATA,
                dtype="float32",
                method="first",
            )
            reproject(
                source=mosaic[0],
                destination=elevation,
                src_transform=mosaic_transform,
                src_crs=source_crs,
                src_nodata=DESTINATION_NODATA,
                dst_transform=destination_transform,
                dst_crs=destination_crs,
                dst_nodata=DESTINATION_NODATA,
                resampling=Resampling.bilinear,
                init_dest_nodata=False,
            )

    valid = np.isfinite(elevation) & (elevation != DESTINATION_NODATA)
    if not include_below_sea_level:
        valid &= elevation > 0
    return paint(elevation, valid, padded_bounds)[1:-1, 1:-1], bounds
