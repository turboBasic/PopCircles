from collections.abc import Sequence
from pathlib import Path

import numpy as np
from matplotlib.colors import LightSource, LinearSegmentedColormap, Normalize
from numpy.typing import NDArray

from population_circles.infrastructure.geodata.dataset_registry import REPO_ROOT, TerrainRaster
from population_circles.infrastructure.geodata.terrain import TerrainFrame, load_terrain
from population_circles.presentation.maps.frame import Frame

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


def prepare_terrain(
    dataset: TerrainRaster | Sequence[TerrainRaster],
    frame: Frame,
    *,
    root: Path = REPO_ROOT,
    include_below_sea_level: bool = False,
) -> tuple[NDArray[np.uint8], tuple[float, float, float, float]]:
    loaded = load_terrain(
        dataset,
        TerrainFrame(frame.horizon.bounds, frame.crs.to_string(), frame.geographic_footprint),
        root=root,
        include_below_sea_level=include_below_sea_level,
    )
    return paint(loaded.elevation, loaded.valid, loaded.padded_bounds)[1:-1, 1:-1], loaded.bounds
