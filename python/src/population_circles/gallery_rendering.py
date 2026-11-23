import concurrent.futures
import contextvars
import hashlib
import importlib.metadata
import json
import os
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, NotRequired, TypedDict, cast
from uuid import uuid4

import matplotlib as mpl
from matplotlib import font_manager
from matplotlib import pyplot as plt
from shapely.geometry.base import BaseGeometry

from population_circles import map_layers as map_data
from population_circles.circle_document import Answer, circle_of
from population_circles.country_outline import load_map_boundaries
from population_circles.dataset_registry import REPO_ROOT, Registry
from population_circles.map_frame import PLATE_CARREE
from population_circles.map_metadata import (
    copyright_of,
    description_of,
    exif_gps_packet,
    metadata_json_fields,
    png_metadata,
    xmp_properties,
)
from population_circles.render_map import (
    other_countries,
    other_country_labels,
    render,
    title_of,
)

PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
RENDER_INPUTS = tuple(
    sorted(
        [
            *Path(__file__).parent.rglob("*.py"),
            REPO_ROOT / "data" / "config.toml",
            REPO_ROOT / "data" / "presentation.toml",
            REPO_ROOT / "data" / "registry.toml",
        ],
        key=lambda path: path.as_posix(),
    )
)


class SettlementCaption(TypedDict):
    name: str
    country: str
    distance_km: float


class FigureCaption(TypedDict):
    radius_km: float
    share: float
    population: float
    centre_lat: float
    centre_lon: float
    settlement: SettlementCaption | None
    territory_share: NotRequired[float]
    territory_name: NotRequired[str]


@dataclass(frozen=True)
class Drawn:
    document: Path
    figure: Path
    country_name: str | None = None
    country_id: str | None = None
    caption: FigureCaption | None = None


@dataclass(frozen=True)
class RenderJob:
    source: Path
    payload: dict[str, Any]
    index: int
    total: int
    projection: str = PLATE_CARREE
    figure_suffix: str = ""


@dataclass(frozen=True)
class MapRenderContext:
    registry: Registry
    coastline: BaseGeometry
    coastline_attribution: str
    output: Path
    cache_directory: Path | None
    digest: str
    render_text: tuple[bool, bool] = (False, False)
    data_root: Path = REPO_ROOT
    map_titles: dict[str, str] = field(default_factory=dict[str, str])


_WORKER_CONTEXT: contextvars.ContextVar[MapRenderContext | None] = contextvars.ContextVar(
    "gallery_render_context", default=None
)


def init_render_worker(context: MapRenderContext) -> None:
    _WORKER_CONTEXT.set(context)


def render_document_in_worker(
    job: RenderJob,
) -> tuple[Drawn, bool]:
    context = _WORKER_CONTEXT.get()
    if context is None:
        message = "gallery render worker was not initialized"
        raise RuntimeError(message)
    return render_document(job, context)


def render_worker_count(map_count: int) -> int:
    return min(2, max(1, (os.process_cpu_count() or 1) - 1), map_count)


def render_documents(
    jobs: list[RenderJob], context: MapRenderContext
) -> tuple[list[tuple[Drawn, bool]], int, float]:
    workers = render_worker_count(len(jobs))
    started = time.perf_counter()
    if workers == 1:
        results = [render_document(job, context) for job in jobs]
    elif workers > 1:
        with concurrent.futures.ProcessPoolExecutor(
            max_workers=workers,
            initializer=init_render_worker,
            initargs=(context,),
        ) as pool:
            results = list(pool.map(render_document_in_worker, jobs))
    else:
        results = []
    return results, workers, time.perf_counter() - started


def render_context_digest() -> str:
    digest = hashlib.sha256()
    digest.update(sys.version.encode())
    for package in ("matplotlib", "pyproj", "rasterio", "shapely"):
        digest.update(importlib.metadata.version(package).encode())
    digest.update(
        json.dumps(
            {key: repr(mpl.rcParams[key]) for key in sorted(mpl.rcParams)},
            sort_keys=True,
        ).encode()
    )
    for path in RENDER_INPUTS:
        digest.update(path.as_posix().encode())
        digest.update(path.read_bytes())
    font_path = Path(
        cast(
            str,
            font_manager.findfont(font_manager.FontProperties()),  # pyright: ignore[reportUnknownMemberType] — Matplotlib's FontManager stub leaves path types partial.
        )
    )
    digest.update(font_path.as_posix().encode())
    digest.update(font_path.read_bytes())
    return digest.hexdigest()


def map_cache_key(
    context: str,
    document: bytes,
    coastline: BaseGeometry,
    outline: BaseGeometry | None,
    projection: str = PLATE_CARREE,
) -> str:
    digest = hashlib.sha256()
    for value in (
        context.encode(),
        document,
        coastline.wkb,
        outline.wkb if outline is not None else b"",
        projection.encode(),
    ):
        digest.update(len(value).to_bytes(8, "big"))
        digest.update(value)
    return digest.hexdigest()


def figure_caption(answer: Answer) -> FigureCaption:
    settlement = answer.caption.settlement
    caption: FigureCaption = {
        "radius_km": answer.circle.radius_km,
        "share": answer.caption.share,
        "population": answer.caption.population,
        "centre_lat": answer.circle.centre.lat,
        "centre_lon": answer.circle.centre.lon,
        "settlement": (
            {
                "name": settlement.name,
                "country": settlement.country,
                "distance_km": settlement.distance_km,
            }
            if settlement is not None
            else None
        ),
    }
    if answer.caption.territory_share is not None:
        caption["territory_share"] = answer.caption.territory_share
        caption["territory_name"] = (
            answer.country.name if answer.country else "Earth\u2019s land area"
        )
    return caption


def reuse_cached_map(cache: Path, key: str, target: Path) -> bool:
    entry = cache / target.stem
    image_path = entry / "map.png"
    metadata_path = entry / "identity.json"
    try:
        image = image_path.read_bytes()
        metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
        if not image.startswith(PNG_MAGIC) or metadata != {
            "key": key,
            "sha256": hashlib.sha256(image).hexdigest(),
        }:
            return False
        target.write_bytes(image)
    except OSError, UnicodeDecodeError, json.JSONDecodeError:
        return False
    return True


def store_cached_map(cache: Path, key: str, target: Path) -> None:
    entry = cache / target.stem
    temporary = entry / f".{uuid4().hex}.tmp"
    try:
        image = target.read_bytes()
        metadata = json.dumps({"key": key, "sha256": hashlib.sha256(image).hexdigest()})
        entry.mkdir(parents=True, exist_ok=True)
        temporary.write_bytes(image)
        temporary.replace(entry / "map.png")
        temporary.write_text(metadata, encoding="utf-8")
        temporary.replace(entry / "identity.json")
    except OSError:
        temporary.unlink(missing_ok=True)


def render_document(job: RenderJob, context: MapRenderContext) -> tuple[Drawn, bool]:
    source = job.source
    payload = job.payload
    projection = job.projection
    figure_suffix = job.figure_suffix
    try:
        answer = circle_of(payload)
        boundaries = load_map_boundaries(answer, context.registry)
        selected = boundaries.get(answer.country.id) if answer.country is not None else None
        map_sources = map_data.registered_map_sources(
            context.registry,
            answer.country.id if answer.country is not None else None,
        )
        population_attribution = (
            answer.selection.population_source.attribution if answer.selection is not None else None
        ) or map_data.citation(context.registry, answer.provenance.dataset)
        outline_and_credit = None
        if answer.country is not None:
            boundary_attribution = (
                answer.selection.boundary_source.attribution
                if answer.selection is not None
                else None
            ) or map_data.citation(context.registry, answer.country.boundary_dataset)
            outline_and_credit = (selected, boundary_attribution)
        target = context.output / f"{source.stem}{figure_suffix}.png"
        title_label = context.map_titles.get(target.stem)
        title = title_of(answer, title_label)
        copyright_text = copyright_of(
            answer,
            population_attribution,
            context.coastline_attribution,
            outline_and_credit[1] if outline_and_credit is not None else None,
            map_sources.attributions,
        )
        description = description_of(answer)
        properties = xmp_properties(
            answer,
            title,
            description,
            copyright_text,
            map_sources.attributions,
        )
        payload["render_metadata"] = metadata_json_fields(answer, properties)
        outline = outline_and_credit[0] if outline_and_credit is not None else None
        cache_key = (
            map_cache_key(
                context.digest,
                json.dumps(payload, sort_keys=True).encode(),
                context.coastline,
                outline,
                projection,
            )
            if context.cache_directory is not None
            else None
        )
        if (
            context.cache_directory is not None
            and cache_key is not None
            and reuse_cached_map(context.cache_directory, cache_key, target)
        ):
            if not figure_suffix:
                (context.output / source.name).write_text(
                    json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
                )
            print(
                f"Gallery: [{job.index}/{job.total}] reused {target.name}",
                file=sys.stderr,
                flush=True,
            )
            return (
                Drawn(
                    source,
                    target,
                    answer.country.name if answer.country is not None else None,
                    answer.country.id if answer.country is not None else None,
                    figure_caption(answer),
                ),
                True,
            )
        print(
            f"Gallery: [{job.index}/{job.total}] rendering {target.name}",
            file=sys.stderr,
            flush=True,
        )
        map_layers = map_data.load_map_layers(
            map_sources,
            root=context.data_root,
            country_outline=outline,
        )
        figure = render(
            answer,
            projection,
            coastline=context.coastline,
            country_outline=outline,
            other_countries=other_countries(boundaries, answer),
            country_labels=other_country_labels(boundaries, answer),
            render_text=context.render_text,
            map_layers=map_layers,
            title_label=title_label,
        )
    except Exception as error:
        # The early-built page is removed by `build_gallery_output` if a map fails.
        # Re-raise rather than publishing a partial gallery.
        error.add_note(f"raised while rendering {source}")
        raise
    figure.savefig(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
        target,
        dpi=200,
        metadata=png_metadata(
            title,
            copyright_text,
            description,
            properties,
        ),
        pil_kwargs={"exif": exif_gps_packet(answer.circle.centre.lat, answer.circle.centre.lon)},
    )
    plt.close(figure)
    if context.cache_directory is not None and cache_key is not None:
        store_cached_map(context.cache_directory, cache_key, target)
    # The document travels beside its figure, so the caption's link resolves on the published
    # site rather than pointing back into a tree the reader does not have.
    if not figure_suffix:
        (context.output / source.name).write_text(
            json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
        )
    print(
        f"Gallery: [{job.index}/{job.total}] wrote {target.name}",
        file=sys.stderr,
        flush=True,
    )
    return (
        Drawn(
            source,
            target,
            answer.country.name if answer.country is not None else None,
            answer.country.id if answer.country is not None else None,
            figure_caption(answer),
        ),
        False,
    )
