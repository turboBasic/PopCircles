import argparse
import hashlib
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, TypedDict, cast
from urllib.parse import quote
from uuid import uuid4

import matplotlib as mpl
from jinja2 import Environment, FileSystemLoader, select_autoescape
from matplotlib import font_manager
from matplotlib import pyplot as plt
from shapely.geometry.base import BaseGeometry

from population_circles.circle_document import CIRCLE_KINDS, Envelope, circle_of
from population_circles.circle_map import Invoke, child, invoke_cli, require_success
from population_circles.country_outline import load_outline
from population_circles.dataset_registry import REPO_ROOT, Registry, load
from population_circles.map_frame import PLATE_CARREE
from population_circles.render_map import citation, registered_coastline, render

TITLE = "The smallest circle on the globe holding a share of world population"
HALF_SHARE = 0.5
HALF_SHARE_PERCENT = "50"
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
RENDER_INPUTS = tuple(
    sorted(
        [
            *Path(__file__).parent.rglob("*.py"),
            *Path(__file__).parent.rglob("*.html"),
            REPO_ROOT / "data" / "config.toml",
            REPO_ROOT / "data" / "registry.toml",
            REPO_ROOT / "mise.toml",
            REPO_ROOT / "pyproject.toml",
            REPO_ROOT / "uv.lock",
        ],
        key=lambda path: path.as_posix(),
    )
)
TEMPLATES = Environment(
    loader=FileSystemLoader(Path(__file__).parent),
    autoescape=select_autoescape(["html"]),
)


@dataclass(frozen=True)
class Drawn:
    document: Path
    figure: Path
    country_name: str | None = None
    country_id: str | None = None


@dataclass(frozen=True)
class Skipped:
    document: Path
    kind: str


class PageCountry(TypedDict):
    name: str
    anchor: str
    figures: list[Drawn]
    missing: bool


@dataclass(frozen=True)
class MapRenderContext:
    registry: Registry
    coastline: BaseGeometry
    coastline_attribution: str
    output: Path
    cache_directory: Path | None
    digest: str


def render_context_digest() -> str:
    digest = hashlib.sha256()
    digest.update(sys.version.encode())
    digest.update(mpl.__version__.encode())
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
) -> str:
    digest = hashlib.sha256()
    for value in (
        context.encode(),
        document,
        coastline.wkb,
        outline.wkb if outline is not None else b"",
    ):
        digest.update(len(value).to_bytes(8, "big"))
        digest.update(value)
    return digest.hexdigest()


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


def render_document(
    source: Path,
    payload: dict[str, Any],
    context: MapRenderContext,
    index: int,
    total: int,
) -> tuple[Drawn, bool]:
    try:
        answer = circle_of(payload)
        population_attribution = (
            answer.selection.population_source.attribution if answer.selection is not None else None
        ) or citation(context.registry, answer.provenance.dataset)
        outline_and_credit = None
        if answer.country is not None:
            boundary_attribution = (
                answer.selection.boundary_source.attribution
                if answer.selection is not None
                else None
            ) or citation(context.registry, answer.country.boundary_dataset)
            outline_and_credit = (load_outline(answer, context.registry), boundary_attribution)
        target = context.output / f"{source.stem}.png"
        outline = outline_and_credit[0] if outline_and_credit is not None else None
        cache_key = (
            map_cache_key(
                context.digest,
                json.dumps(payload, sort_keys=True).encode(),
                context.coastline,
                outline,
            )
            if context.cache_directory is not None
            else None
        )
        if (
            context.cache_directory is not None
            and cache_key is not None
            and reuse_cached_map(context.cache_directory, cache_key, target)
        ):
            (context.output / source.name).write_text(
                json.dumps(payload, indent=2), encoding="utf-8"
            )
            print(
                f"Gallery: [{index}/{total}] reused {target.name}",
                file=sys.stderr,
                flush=True,
            )
            return (
                Drawn(
                    source,
                    target,
                    answer.country.name if answer.country is not None else None,
                    answer.country.id if answer.country is not None else None,
                ),
                True,
            )
        print(
            f"Gallery: [{index}/{total}] rendering {source.name}",
            file=sys.stderr,
            flush=True,
        )
        figure = render(
            answer,
            PLATE_CARREE,
            population_attribution,
            coastline=context.coastline,
            coastline_attribution=context.coastline_attribution,
            outline_and_credit=outline_and_credit,
        )
    except Exception as error:
        # The exception carries the document, because a traceback out of pydantic or
        # matplotlib names neither. Re-raised rather than collected: `index.html` is written
        # after the loop, so a failure publishes no gallery rather than one with a hole in it.
        error.add_note(f"raised while rendering {source}")
        raise
    figure.savefig(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
        target,
        dpi=200,
    )
    plt.close(figure)
    if context.cache_directory is not None and cache_key is not None:
        store_cached_map(context.cache_directory, cache_key, target)
    # The document travels beside its figure, so the caption's link resolves on the published
    # site rather than pointing back into a tree the reader does not have.
    (context.output / source.name).write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(
        f"Gallery: [{index}/{total}] wrote {target.name}",
        file=sys.stderr,
        flush=True,
    )
    return (
        Drawn(
            source,
            target,
            answer.country.name if answer.country is not None else None,
... (+416 lines) [see remaining: rtk proxy git -c core.fsmonitor=false show HEAD:python/src/population_circles/gallery.py | tail -n +247]
