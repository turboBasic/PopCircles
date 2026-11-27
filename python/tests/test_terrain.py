import hashlib
from pathlib import Path

import numpy as np
import pytest
import rasterio
from matplotlib.colors import LightSource, to_rgba
from pyproj import CRS, Transformer
from rasterio.transform import from_origin
from shapely.geometry import Point, box

from population_circles.infrastructure.geodata.dataset_registry import TerrainRaster
from population_circles.maps.frame import Frame
from population_circles.maps.terrain import ELEVATION_STOPS, paint, palette, prepare_terrain

NODATA = -9999.0
OPAQUE = 255


def write_dem(path: Path) -> TerrainRaster:
    values = np.tile(np.linspace(0, 8848, 80, dtype=np.float32), (80, 1))
    values[30:50, 30:50] = NODATA
    with rasterio.open(
        path,
        "w",
        driver="GTiff",
        width=values.shape[1],
        height=values.shape[0],
        count=1,
        dtype="float32",
        crs="EPSG:4326",
        transform=from_origin(-2, 2, 0.05, 0.05),
        nodata=NODATA,
    ) as output:
        output.write(values, 1)
    content = path.read_bytes()
    return TerrainRaster(
        kind="terrain-raster",
        path=path.name,
        bytes=len(content),
        sha256=hashlib.sha256(content).hexdigest(),
        source_url="https://example.invalid/source",
        licence="test",
        licence_url="https://example.invalid/licence",
        attribution="",
        format="GeoTIFF",
        width=80,
        height=80,
        epsg=4326,
        x_origin=-2,
        y_origin=2,
        x_step=0.05,
        y_step=-0.05,
        nodata=NODATA,
    )


def write_tile(path: Path, values: np.ndarray, x_origin: float) -> TerrainRaster:
    with rasterio.open(
        path,
        "w",
        driver="GTiff",
        width=values.shape[1],
        height=values.shape[0],
        count=1,
        dtype="float32",
        crs="EPSG:4326",
        transform=from_origin(x_origin, 2, 0.05, 0.05),
    ) as output:
        output.write(values, 1)
    content = path.read_bytes()
    return TerrainRaster(
        kind="terrain-raster",
        path=path.name,
        bytes=len(content),
        sha256=hashlib.sha256(content).hexdigest(),
        source_url="https://example.invalid/source",
        licence="test",
        licence_url="https://example.invalid/licence",
        attribution="",
        format="GeoTIFF",
        width=values.shape[1],
        height=values.shape[0],
        epsg=4326,
        x_origin=x_origin,
        y_origin=2,
        x_step=0.05,
        y_step=-0.05,
    )


def projected_frame(epsg: int = 3857) -> Frame:
    crs = CRS.from_epsg(epsg)
    horizon = box(-100_000, -100_000, 100_000, 100_000)
    return Frame(
        crs=crs,
        to_frame=Transformer.from_crs("EPSG:4326", crs, always_xy=True),
        visible=box(-2, -2, 2, 2),
        circle=Point(0, 0),
        horizon=horizon,
        geographic_footprint=box(-0.9, -0.9, 0.9, 0.9),
    )


def test_prepare_terrain_aligns_crs_transform_and_nodata(tmp_path: Path) -> None:
    dataset = write_dem(tmp_path / "test-dem.tif")
    view = projected_frame()

    pixels, bounds = prepare_terrain(dataset, view, root=tmp_path)

    assert bounds == view.horizon.bounds
    assert pixels.shape == (1440, 1440, 4)
    assert (bounds[2] - bounds[0]) / pixels.shape[1] == pytest.approx(200_000 / 1440)
    assert (bounds[3] - bounds[1]) / pixels.shape[0] == pytest.approx(200_000 / 1440)
    # The central nodata block stays transparent at its projected location; terrain around it
    # remains visible.
    assert pixels[720, 720, 3] == 0
    assert pixels[150, 1300, 3] == OPAQUE


def test_prepare_terrain_mosaics_tiles_before_hillshade(tmp_path: Path) -> None:
    values = np.tile(np.linspace(100, 4000, 80, dtype=np.float32), (80, 1))
    full = write_tile(tmp_path / "full.tif", values, -2)
    west = write_tile(tmp_path / "west.tif", values[:, :40], -2)
    east = write_tile(tmp_path / "east.tif", values[:, 40:], 0)
    view = projected_frame()

    expected, _ = prepare_terrain(full, view, root=tmp_path)
    actual, _ = prepare_terrain((west, east), view, root=tmp_path)

    assert np.array_equal(actual, expected)


def test_prepare_terrain_refuses_a_registry_crs_that_disagrees_with_the_raster(
    tmp_path: Path,
) -> None:
    dataset = write_dem(tmp_path / "test-dem.tif").model_copy(update={"epsg": 3857})

    with pytest.raises(ValueError, match="CRS that differs from its registry row"):
        prepare_terrain(dataset, projected_frame(), root=tmp_path)


def test_prepare_terrain_names_the_fetch_task_when_the_source_is_missing(tmp_path: Path) -> None:
    dataset = write_dem(tmp_path / "test-dem.tif")
    path = tmp_path / dataset.path
    path.unlink()

    with pytest.raises(ValueError, match=r"test-dem\.tif.*mise run data:get"):
        prepare_terrain(dataset, projected_frame(), root=tmp_path)


def test_elevation_palette_keeps_all_pinned_stops() -> None:
    colours, scale = palette()
    for elevation, rgb in ELEVATION_STOPS:
        observed = np.rint(np.asarray(colours(scale(elevation)))[:3] * 255)
        assert observed == pytest.approx(rgb, abs=6)
    assert tuple(np.rint(np.asarray(colours(scale(0)))[:3] * 255)) == pytest.approx(
        ELEVATION_STOPS[0][1], abs=6
    )
    assert tuple(np.rint(np.asarray(colours(scale(8848)))[:3] * 255)) == ELEVATION_STOPS[-1][1]


def test_hillshade_uses_the_pinned_light_and_is_deterministic() -> None:
    elevation = np.array(
        [[0, 100, 250], [400, 600, 800], [1000, 1500, 2200]],
        dtype=np.float32,
    )
    valid = np.ones(elevation.shape, dtype=np.bool_)
    valid[0, 0] = False
    bounds = (0.0, 0.0, 30.0, 20.0)

    first = paint(elevation, valid, bounds)
    second = paint(elevation, valid, bounds)

    assert np.array_equal(first, second)
    assert first[0, 0, 3] == 0
    assert first[1, 1, 3] == OPAQUE
    shade = LightSource(azdeg=315, altdeg=45).hillshade(
        np.where(valid, elevation, 0.0),
        vert_exag=1.0,
        dx=10.0,
        dy=20 / 3,
        fraction=1.0,
    )
    source = np.rint(np.asarray(to_rgba("#A6CA7A")) * 255).astype(np.uint8)
    expected = np.rint(
        source[:3] * (1 - 0.34) + shade[1, 1] * 255 * 0.34,
    ).astype(np.uint8)
    assert np.allclose(first[1, 1, :3], expected, atol=1)
