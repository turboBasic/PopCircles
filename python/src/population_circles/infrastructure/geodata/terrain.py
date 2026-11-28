import math
from collections.abc import Sequence
from contextlib import ExitStack
from dataclasses import dataclass
from pathlib import Path
from typing import cast

import numpy as np
from numpy.typing import NDArray
from rasterio import open as open_raster
from rasterio.crs import CRS
from rasterio.enums import Resampling
from rasterio.io import DatasetReader
from rasterio.merge import merge
from rasterio.transform import from_bounds
from rasterio.warp import reproject, transform_bounds
from shapely.geometry.base import BaseGeometry

from population_circles.domain.circle_geometry import polygons
from population_circles.infrastructure.geodata.dataset_registry import REPO_ROOT, TerrainRaster

_TARGET_HEIGHT = 1440
_MIN_WIDTH = 720
_MAX_WIDTH = 1800
_GEOGRAPHIC_CRS = CRS.from_epsg(4326)
_DESTINATION_NODATA = -99999.0


@dataclass(frozen=True)
class TerrainFrame:
    bounds: tuple[float, float, float, float]
    crs: str
    geographic_footprint: BaseGeometry


@dataclass(frozen=True)
class LoadedTerrain:
    elevation: NDArray[np.float32]
    valid: NDArray[np.bool_]
    padded_bounds: tuple[float, float, float, float]
    bounds: tuple[float, float, float, float]


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


def load_terrain(
    dataset: TerrainRaster | Sequence[TerrainRaster],
    frame: TerrainFrame,
    *,
    root: Path = REPO_ROOT,
    include_below_sea_level: bool = False,
) -> LoadedTerrain:
    datasets = (dataset,) if isinstance(dataset, TerrainRaster) else tuple(dataset)
    if not datasets:
        message = "at least one terrain source is required"
        raise ValueError(message)
    xmin, ymin, xmax, ymax = frame.bounds
    width = max(_MIN_WIDTH, min(_MAX_WIDTH, round(_TARGET_HEIGHT * (xmax - xmin) / (ymax - ymin))))
    height = _TARGET_HEIGHT
    pixel_width = (xmax - xmin) / width
    pixel_height = (ymax - ymin) / height
    padded_bounds = (
        xmin - pixel_width,
        ymin - pixel_height,
        xmax + pixel_width,
        ymax + pixel_height,
    )
    destination_transform = from_bounds(*padded_bounds, width + 2, height + 2)
    elevation = np.full((height + 2, width + 2), _DESTINATION_NODATA, dtype=np.float32)
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
        footprint_bounds = [
            transform_bounds(
                _GEOGRAPHIC_CRS,
                source_crs,
                *part.bounds,
                densify_pts=41,
            )
            for part in polygons(frame.geographic_footprint)
        ]
        source_windows = [
            bounds
            for bounds in footprint_bounds
            if bounds[0] < bounds[2]
            and bounds[1] < bounds[3]
            and any(
                bounds[0] < source.bounds.right
                and bounds[2] > source.bounds.left
                and bounds[1] < source.bounds.top
                and bounds[3] > source.bounds.bottom
                for source in sources
            )
        ]
        if not source_windows:
            names = ", ".join(dataset.path for dataset in datasets)
            message = f"terrain sources {names!r} do not overlap the map frame"
            raise ValueError(message)
        destination_crs = CRS.from_string(frame.crs)
        for source_bounds in source_windows:
            mosaic, mosaic_transform = merge(
                sources,
                bounds=source_bounds,
                nodata=_DESTINATION_NODATA,
                dtype="float32",
                method="first",
            )
            reproject(
                source=mosaic[0],
                destination=elevation,
                src_transform=mosaic_transform,
                src_crs=source_crs,
                src_nodata=_DESTINATION_NODATA,
                dst_transform=destination_transform,
                dst_crs=destination_crs,
                dst_nodata=_DESTINATION_NODATA,
                resampling=Resampling.bilinear,
                init_dest_nodata=False,
            )

    valid = np.isfinite(elevation) & (elevation != _DESTINATION_NODATA)
    if not include_below_sea_level:
        valid &= elevation > 0
    return LoadedTerrain(elevation, valid, padded_bounds, frame.bounds)
