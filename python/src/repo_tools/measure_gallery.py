import hashlib
import importlib.metadata
import json
import platform
import sys
import time
from pathlib import Path

import numpy as np
from numpy.typing import NDArray
from PIL import Image

from population_circles.gallery import rendering as gallery_rendering
from population_circles.infrastructure.geodata.dataset_registry import (
    REPO_ROOT,
    TerrainRaster,
    load,
)
from population_circles.maps import render_map
from population_circles.maps.frame import Frame
from population_circles.maps.layers import registered_coastline
from population_circles.maps.presentation_config import map_text_rendering
from population_circles.maps.terrain import prepare_terrain

ids = ("FR", "NL", "CH", "CL", "JP", "BR", "NP", "CN")
corpus = REPO_ROOT / "results"
output = REPO_ROOT / "build/gh-355-phase3/maps"
cache = REPO_ROOT / "build/gh-355-phase3/cache"
output.mkdir(parents=True, exist_ok=True)
cache.mkdir(parents=True, exist_ok=True)
registry = load()
coastline, coastline_credit = registered_coastline(registry)
context = gallery_rendering.MapRenderContext(
    registry,
    coastline,
    coastline_credit,
    output,
    cache,
    gallery_rendering.render_context_digest(),
    map_text_rendering(),
)
original_prepare = prepare_terrain
terrain_seconds = [0.0]


def timed_prepare(
    dataset: TerrainRaster,
    frame: Frame,
    *,
    root: Path = REPO_ROOT,
) -> tuple[NDArray[np.uint8], tuple[float, float, float, float]]:
    started = time.perf_counter()
    result = original_prepare(dataset, frame, root=root)
    terrain_seconds[0] += time.perf_counter() - started
    return result


prepare_terrain_attribute = "prepare_terrain"
setattr(render_map, prepare_terrain_attribute, timed_prepare)
cold: dict[str, dict[str, object]] = {}
warm: dict[str, dict[str, object]] = {}
before: dict[str, str] = {}
total = len(ids)
for index, country_id in enumerate(ids, start=1):
    source = next(corpus.glob(f"country-*-{country_id}-half.json"))
    before[country_id] = hashlib.sha256(source.read_bytes()).hexdigest()
    payload = json.loads(source.read_text())
    terrain_seconds[0] = 0.0
    started = time.perf_counter()
    drawn, reused = gallery_rendering.render_document(
        gallery_rendering.RenderJob(source, payload, index, total), context
    )
    if reused:
        raise AssertionError
    elapsed = time.perf_counter() - started
    with Image.open(drawn.figure) as image:
        dimensions = image.size
    cold[country_id] = {
        "source": str(source.relative_to(REPO_ROOT)),
        "figure": str(drawn.figure.relative_to(REPO_ROOT)),
        "seconds_total": round(elapsed, 3),
        "seconds_terrain_prepare": round(terrain_seconds[0], 3),
        "seconds_render_and_write": round(max(0.0, elapsed - terrain_seconds[0]), 3),
        "dimensions": dimensions,
        "bytes": drawn.figure.stat().st_size,
        "reused_cache": reused,
    }
for index, country_id in enumerate(ids, start=1):
    source = next(corpus.glob(f"country-*-{country_id}-half.json"))
    payload = json.loads(source.read_text())
    started = time.perf_counter()
    drawn, reused = gallery_rendering.render_document(
        gallery_rendering.RenderJob(source, payload, index, total), context
    )
    elapsed = time.perf_counter() - started
    warm[country_id] = {"seconds_total": round(elapsed, 3), "reused_cache": reused}
    if not reused:
        raise AssertionError
    if hashlib.sha256(source.read_bytes()).hexdigest() != before[country_id]:
        raise AssertionError

report = {
    "machine": f"{platform.machine()} {platform.platform()}",
    "python": sys.version.split()[0],
    "rasterio": importlib.metadata.version("rasterio"),
    "matplotlib": importlib.metadata.version("matplotlib"),
    "source_resolution": {
        "etopo-2022-60s": "60 arc-seconds",
        "swisstopo-dhm25-200m": "200 m",
    },
    "cold": cold,
    "warm": warm,
    "source_documents_unchanged": True,
}
path = REPO_ROOT / "build/gh-355-phase3/measurements.json"
path.write_text(json.dumps(report, indent=2) + "\n")
