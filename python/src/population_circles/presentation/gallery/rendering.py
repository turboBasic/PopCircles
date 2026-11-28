import concurrent.futures
import contextvars
import hashlib
import importlib.metadata
import json
import os
import sys
import time
from collections.abc import Mapping
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import cast
from uuid import uuid4

import matplotlib as mpl
from matplotlib import font_manager
from matplotlib import pyplot as plt
from matplotlib.figure import Figure
from shapely.geometry.base import BaseGeometry

from population_circles.application.use_cases.country_sweep import FigureCaption, RenderJob
from population_circles.application.use_cases.render_map import (
    MapBoundaries,
    other_countries,
    other_country_labels,
)
from population_circles.documents.result_document import Answer, circle_of
from population_circles.infrastructure.geodata import map_layers as map_data
from population_circles.infrastructure.geodata.country_boundaries import load_map_boundaries
from population_circles.infrastructure.geodata.dataset_registry import REPO_ROOT, Registry
from population_circles.presentation.gallery.page_data import Drawn
from population_circles.presentation.maps import source_selection
from population_circles.presentation.maps.frame import PLATE_CARREE
from population_circles.presentation.maps.metadata import (
    copyright_of,
    description_of,
    exif_gps_packet,
    metadata_json_fields,
    png_metadata,
    xmp_properties,
)
from population_circles.presentation.maps.render_map import render, title_of

_PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
_RENDER_MODULES = (
    "application/use_cases/render_map.py",
    "application/use_cases/territory_coverage.py",
    "documents/result_document.py",
    "domain/circle_geometry.py",
    "domain/geography.py",
    "domain/territory_coverage.py",
    "infrastructure/geodata/country_boundaries.py",
    "infrastructure/geodata/data_config.py",
    "infrastructure/geodata/dataset_registry.py",
    "infrastructure/geodata/map_layers.py",
    "infrastructure/geodata/terrain.py",
    "presentation/gallery/rendering.py",
    "presentation/maps/frame.py",
    "presentation/maps/labels.py",
    "presentation/maps/metadata.py",
    "presentation/maps/presentation_config.py",
    "presentation/maps/render_map.py",
    "presentation/maps/source_selection.py",
    "presentation/maps/terrain.py",
)
_RENDER_INPUTS = tuple(
    sorted(
        [
            *(Path(__file__).resolve().parents[2] / name for name in _RENDER_MODULES),
            REPO_ROOT / "data" / "config.toml",
            REPO_ROOT / "data" / "presentation.toml",
            REPO_ROOT / "data" / "registry.toml",
        ],
        key=lambda path: path.as_posix(),
    )
)


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
    source_digests: dict[Path, str] = field(default_factory=dict[Path, str], repr=False)


@dataclass(frozen=True)
class _RenderPlan:
    job: RenderJob
    answer: Answer
    boundaries: MapBoundaries
    map_sources: map_data.MapSources
    outline: BaseGeometry | None
    target: Path
    title_label: str | None
    title: str
    copyright_text: str
    description: str
    properties: dict[str, object]
    cache_key: str | None


_WORKER_CONTEXT: contextvars.ContextVar[MapRenderContext | None] = contextvars.ContextVar(
    "gallery_render_context", default=None
)


def _init_render_worker(context: MapRenderContext) -> None:
    _WORKER_CONTEXT.set(context)


def _render_document_in_worker(
    job: RenderJob,
) -> tuple[Drawn, bool]:
    context = _WORKER_CONTEXT.get()
    if context is None:
        message = "gallery render worker was not initialized"
        raise RuntimeError(message)
    return _render_document(job, context)


def _render_worker_count(map_count: int) -> int:
    return min(2, max(1, (os.process_cpu_count() or 1) - 1), map_count)


def render_documents(
    jobs: list[RenderJob], context: object
) -> tuple[list[tuple[Drawn, bool]], int, float]:
    if not isinstance(context, MapRenderContext):
        message = "Gallery render context must be MapRenderContext"
        raise TypeError(message)
    if context.cache_directory is not None:
        context = replace(context, source_digests=_source_digests(jobs, context))
    workers = _render_worker_count(len(jobs))
    started = time.perf_counter()
    if workers == 1:
        results = [_render_document(job, context) for job in jobs]
    elif workers > 1:
        with concurrent.futures.ProcessPoolExecutor(
            max_workers=workers,
            initializer=_init_render_worker,
            initargs=(context,),
        ) as pool:
            results = list(pool.map(_render_document_in_worker, jobs))
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
    for path in _RENDER_INPUTS:
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


def _map_cache_key(
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


def _map_source_paths(sources: map_data.MapSources, root: Path) -> tuple[Path, ...]:
    datasets = (
        sources.terrain,
        *sources.terrain_overlays,
        sources.lakes,
        sources.rivers,
        sources.roads,
        sources.places,
    )
    return tuple(dataset.file(root) for dataset in datasets)


def _source_digest(path: Path) -> str:
    if not path.is_file():
        return "missing"
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def _source_identity(paths: tuple[Path, ...], digests: dict[Path, str]) -> bytes:
    return json.dumps(
        [(path.as_posix(), digests.get(path) or _source_digest(path)) for path in paths],
        separators=(",", ":"),
    ).encode()


def _source_digests(jobs: list[RenderJob], context: MapRenderContext) -> dict[Path, str]:
    digests: dict[Path, str] = {}
    for job in jobs:
        answer = circle_of(job.payload)
        sources = source_selection.registered_map_sources(
            context.registry,
            answer.country.id if answer.country is not None else None,
        )
        for path in _map_source_paths(sources, context.data_root):
            if path not in digests:
                digests[path] = _source_digest(path)
    return digests


def _figure_caption(answer: Answer) -> FigureCaption:
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


def _reuse_cached_map(cache: Path, key: str, target: Path) -> bool:
    entry = cache / target.stem
    image_path = entry / "map.png"
    metadata_path = entry / "identity.json"
    try:
        image = image_path.read_bytes()
        metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
        if not image.startswith(_PNG_MAGIC) or metadata != {
            "key": key,
            "sha256": hashlib.sha256(image).hexdigest(),
        }:
            return False
        target.write_bytes(image)
    except OSError, UnicodeDecodeError, json.JSONDecodeError:
        return False
    return True


def _store_cached_map(cache: Path, key: str, target: Path) -> None:
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


def _select_map_boundaries(
    answer: Answer, context: MapRenderContext
) -> tuple[MapBoundaries, BaseGeometry | None, str | None]:
    boundaries = load_map_boundaries(answer, context.registry)
    selection = MapBoundaries(
        boundaries,
        cast(Mapping[str, str], getattr(boundaries, "names", {})),
    )
    if answer.country is None:
        return selection, None, None

    outline = boundaries.get(answer.country.id)
    attribution = (
        answer.selection.boundary_source.attribution if answer.selection is not None else None
    ) or map_data.citation(context.registry, answer.country.boundary_dataset)
    return selection, outline, attribution


def _prepare_map_metadata(
    answer: Answer,
    context: MapRenderContext,
    map_sources: map_data.MapSources,
    boundary_attribution: str | None,
    title_label: str | None,
) -> tuple[str, str, str, dict[str, object]]:
    population_attribution = (
        answer.selection.population_source.attribution if answer.selection is not None else None
    ) or map_data.citation(context.registry, answer.provenance.dataset)
    title = title_of(answer, title_label)
    copyright_text = copyright_of(
        answer,
        population_attribution,
        context.coastline_attribution,
        boundary_attribution,
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
    return title, copyright_text, description, properties


def _cache_key_for_render(
    payload: dict[str, object],
    context: MapRenderContext,
    map_sources: map_data.MapSources,
    outline: BaseGeometry | None,
    projection: str,
) -> str | None:
    if context.cache_directory is None:
        return None
    source_key = _source_identity(
        _map_source_paths(map_sources, context.data_root), context.source_digests
    )
    return _map_cache_key(
        context.digest,
        json.dumps(payload, sort_keys=True).encode() + source_key,
        context.coastline,
        outline,
        projection,
    )


def _draw_gallery_map(
    plan: _RenderPlan,
    context: MapRenderContext,
) -> Figure:
    map_layers = map_data.load_map_layers(
        plan.map_sources,
        root=context.data_root,
        country_outline=plan.outline,
    )
    return render(
        plan.answer,
        plan.job.projection,
        coastline=context.coastline,
        country_outline=plan.outline,
        other_countries=other_countries(plan.boundaries, plan.answer),
        country_labels=other_country_labels(plan.boundaries, plan.answer),
        render_text=context.render_text,
        map_layers=map_layers,
        title_label=plan.title_label,
    )


def _drawn_figure(source: Path, target: Path, answer: Answer) -> Drawn:
    return Drawn(
        source,
        target,
        answer.country.name if answer.country is not None else None,
        answer.country.id if answer.country is not None else None,
        _figure_caption(answer),
    )


def _copy_document_for_gallery(
    payload: dict[str, object], source: Path, context: MapRenderContext, figure_suffix: str
) -> None:
    if not figure_suffix:
        (context.output / source.name).write_text(
            json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
        )


def _reuse_rendered_document(
    plan: _RenderPlan,
    context: MapRenderContext,
) -> tuple[Drawn, bool] | None:
    if (
        context.cache_directory is None
        or plan.cache_key is None
        or not _reuse_cached_map(context.cache_directory, plan.cache_key, plan.target)
    ):
        return None
    _copy_document_for_gallery(plan.job.payload, plan.job.source, context, plan.job.figure_suffix)
    print(
        f"Gallery: [{plan.job.index}/{plan.job.total}] reused {plan.target.name}",
        file=sys.stderr,
        flush=True,
    )
    return _drawn_figure(plan.job.source, plan.target, plan.answer), True


def _save_rendered_map(
    figure: Figure,
    plan: _RenderPlan,
) -> None:
    try:
        figure.savefig(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
            plan.target,
            dpi=200,
            metadata=png_metadata(
                plan.title, plan.copyright_text, plan.description, plan.properties
            ),
            pil_kwargs={
                "exif": exif_gps_packet(
                    plan.answer.circle.centre.lat, plan.answer.circle.centre.lon
                )
            },
        )
    finally:
        plt.close(figure)


def _render_document(job: RenderJob, context: MapRenderContext) -> tuple[Drawn, bool]:
    source = job.source
    try:
        answer = circle_of(job.payload)
        boundaries, outline, boundary_attribution = _select_map_boundaries(answer, context)
        map_sources = source_selection.registered_map_sources(
            context.registry,
            answer.country.id if answer.country is not None else None,
        )
        target = context.output / f"{source.stem}{job.figure_suffix}.png"
        title_label = context.map_titles.get(target.stem)
        title, copyright_text, description, properties = _prepare_map_metadata(
            answer, context, map_sources, boundary_attribution, title_label
        )
        job.payload["render_metadata"] = metadata_json_fields(answer, properties)
        cache_key = _cache_key_for_render(
            job.payload, context, map_sources, outline, job.projection
        )
        plan = _RenderPlan(
            job,
            answer,
            boundaries,
            map_sources,
            outline,
            target,
            title_label,
            title,
            copyright_text,
            description,
            properties,
            cache_key,
        )
        reused = _reuse_rendered_document(plan, context)
        if reused is not None:
            return reused

        print(
            f"Gallery: [{job.index}/{job.total}] rendering {plan.target.name}",
            file=sys.stderr,
            flush=True,
        )
        figure = _draw_gallery_map(plan, context)
    except Exception as error:
        # The early-built page is removed by `_build_gallery_output` if a map fails.
        # Re-raise rather than publishing a partial gallery.
        error.add_note(f"raised while rendering {source}")
        raise

    _save_rendered_map(figure, plan)
    if context.cache_directory is not None and plan.cache_key is not None:
        _store_cached_map(context.cache_directory, plan.cache_key, plan.target)
    # The document travels beside its figure, so the caption's link resolves on the published
    # site rather than pointing back into a tree the reader does not have.
    _copy_document_for_gallery(job.payload, source, context, job.figure_suffix)
    print(
        f"Gallery: [{job.index}/{job.total}] wrote {plan.target.name}",
        file=sys.stderr,
        flush=True,
    )
    return _drawn_figure(source, plan.target, plan.answer), False
