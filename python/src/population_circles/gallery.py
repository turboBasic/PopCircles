import argparse
import concurrent.futures
import contextvars
import hashlib
import importlib.metadata
import json
import os
import shutil
import subprocess
import sys
import time
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any, NotRequired, TypedDict, cast
from urllib.parse import quote
from uuid import uuid4

import matplotlib as mpl
from matplotlib import font_manager
from matplotlib import pyplot as plt
from shapely.geometry.base import BaseGeometry

from population_circles.circle_document import CIRCLE_KINDS, Answer, Envelope, circle_of
from population_circles.circle_map import Invoke, child, invoke_cli, require_success
from population_circles.country_outline import load_map_boundaries
from population_circles.dataset_registry import (
    REPO_ROOT,
    MapTextPlacement,
    Registry,
    load,
    map_text_placement,
)
from population_circles.map_frame import PLATE_CARREE
from population_circles.render_map import (
    citation,
    copyright_of,
    description_of,
    metadata_json_fields,
    other_countries,
    png_metadata,
    registered_coastline,
    render,
    title_of,
    xmp_properties,
)

HALF_SHARE = 0.5
HALF_SHARE_PERCENT = "50"
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
RENDER_INPUTS = tuple(
    sorted(
        [
            *Path(__file__).parent.rglob("*.py"),
            REPO_ROOT / "data" / "config.toml",
            REPO_ROOT / "data" / "gallery.toml",
            REPO_ROOT / "data" / "registry.toml",
        ],
        key=lambda path: path.as_posix(),
    )
)


def excluded_country_ids() -> set[str]:
    config = tomllib.loads((REPO_ROOT / "data" / "gallery.toml").read_text(encoding="utf-8"))
    return set(config["excluded_country_ids"])


class SettlementCaption(TypedDict):
    name: str
    country: str
    distance_km: float


class CountryCaption(TypedDict):
    radius_km: float
    share: float
    population: float
    centre_lat: float
    centre_lon: float
    settlement: SettlementCaption | None


@dataclass(frozen=True)
class Drawn:
    document: Path
    figure: Path
    country_name: str | None = None
    country_id: str | None = None
    caption: CountryCaption | None = None


@dataclass(frozen=True)
class Skipped:
    document: Path
    kind: str


class PageFigure(TypedDict):
    map: str
    document: str
    country_id: str | None
    alt: str
    caption: NotRequired[CountryCaption]


class PageCountry(TypedDict):
    name: str
    anchor: str
    figures: list[PageFigure]
    missing: bool


class PageData(TypedDict):
    world: list[PageFigure]
    countries: list[PageCountry]


@dataclass(frozen=True)
class MapRenderContext:
    registry: Registry
    coastline: BaseGeometry
    coastline_attribution: str
    output: Path
    cache_directory: Path | None
    digest: str
    text_placement: tuple[MapTextPlacement, MapTextPlacement] = ("metadata", "metadata")


_WORKER_CONTEXT: contextvars.ContextVar[MapRenderContext | None] = contextvars.ContextVar(
    "gallery_render_context", default=None
)


def init_render_worker(context: MapRenderContext) -> None:
    _WORKER_CONTEXT.set(context)


def render_document_in_worker(
    job: tuple[Path, dict[str, Any], int, int],
) -> tuple[Drawn, bool]:
    context = _WORKER_CONTEXT.get()
    if context is None:
        message = "gallery render worker was not initialized"
        raise RuntimeError(message)
    source, payload, index, total = job
    return render_document(source, payload, context, index, total)


def render_worker_count(map_count: int) -> int:
    return min(max(1, (os.process_cpu_count() or 1) - 1), map_count)


def render_documents(
    jobs: list[tuple[Path, dict[str, Any], int, int]], context: MapRenderContext
) -> tuple[list[tuple[Drawn, bool]], int, float]:
    workers = render_worker_count(len(jobs))
    started = time.perf_counter()
    if workers == 1:
        results = [
            render_document(source, payload, context, index, total)
            for source, payload, index, total in jobs
        ]
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
    for package in ("matplotlib", "pyproj", "shapely"):
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


def country_caption(answer: Answer) -> CountryCaption | None:
    if answer.country is None:
        return None
    settlement = answer.caption.settlement
    return {
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
        boundaries = load_map_boundaries(answer, context.registry)
        selected = boundaries.get(answer.country.id) if answer.country is not None else None
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
            outline_and_credit = (selected, boundary_attribution)
        title = title_of(answer)
        copyright_text = copyright_of(
            answer,
            population_attribution,
            context.coastline_attribution,
            outline_and_credit[1] if outline_and_credit is not None else None,
        )
        description = description_of(answer)
        properties = xmp_properties(answer)
        payload["render_metadata"] = metadata_json_fields(
            title, description, copyright_text, properties, context.text_placement
        )
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
                    country_caption(answer),
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
            coastline=context.coastline,
            country_outline=outline,
            other_countries=other_countries(boundaries, answer),
            text_placement=context.text_placement,
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
            context.text_placement,
            description,
            properties,
        ),
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
            answer.country.id if answer.country is not None else None,
            country_caption(answer),
        ),
        False,
    )


def page_figure(one: Drawn) -> PageFigure:
    figure: PageFigure = {
        "map": one.figure.name,
        "document": one.document.name,
        "country_id": one.country_id,
        "alt": one.figure.stem,
    }
    if one.caption is not None:
        figure["caption"] = one.caption
    return figure


def page_data(drawn: list[Drawn], missing: list[str]) -> PageData:
    countries: dict[tuple[str, str], list[Drawn]] = {}
    world: list[Drawn] = []
    for one in drawn:
        if one.country_name is None or one.country_id is None:
            world.append(one)
        else:
            countries.setdefault((one.country_name, one.country_id), []).append(one)

    ordered = sorted(countries.items(), key=lambda item: (item[0][0].casefold(), item[0][1]))
    page_countries: list[PageCountry] = [
        {
            "name": name,
            "anchor": f"country-result-{quote(identifier, safe='')}",
            "figures": [page_figure(figure) for figure in figures],
            "missing": False,
        }
        for (name, identifier), figures in ordered
    ]
    page_countries.extend(
        {
            "name": name,
            "anchor": f"country-missing-{quote(name, safe='')}",
            "figures": [],
            "missing": True,
        }
        for name in missing
    )
    page_countries.sort(key=lambda country: (country["name"].casefold(), country["anchor"]))
    return {"world": [page_figure(one) for one in world], "countries": page_countries}


def build_frontend(output: Path) -> None:
    assets = output / "assets"
    if assets.is_symlink():
        assets.unlink()
    elif assets.is_dir():
        shutil.rmtree(assets)
    npm = shutil.which("npm")
    if npm is None:
        message = "npm is required; run this gallery build through mise"
        raise FileNotFoundError(message)
    subprocess.run(  # noqa: S603 — argv only; npm runs the pinned repository frontend build
        [
            npm,
            "--prefix",
            str(REPO_ROOT / "frontend"),
            "run",
            "build",
            "--",
            "--outDir",
            str(output.resolve()),
        ],
        check=True,
    )


def build_gallery_output(
    corpus: Path,
    sources: list[tuple[Path, dict[str, Any]]],
    generated: dict[str, dict[str, Any]],
    missing: list[str],
    context: MapRenderContext,
) -> tuple[list[Drawn], list[Skipped]]:
    output = context.output
    drawn: list[Drawn] = []
    skipped: list[Skipped] = []
    reused_maps = rendered_maps = 0
    unavailable = set(missing)
    excluded = excluded_country_ids()
    sources, excluded_sources = omit_excluded_sources(sources, output, excluded)
    skipped.extend(excluded_sources)
    render_jobs: list[tuple[Path, dict[str, Any], int, int]] = []
    try:
        build_frontend(output)
        print(
            f"Gallery: rendering {len(sources)} result documents",
            file=sys.stderr,
            flush=True,
        )
        for index, (source, payload) in enumerate(sources, start=1):
            envelope = Envelope.model_validate(payload)
            if envelope.document not in CIRCLE_KINDS and envelope.document != "country-smallest":
                print(
                    f"Gallery: [{index}/{len(sources)}] skipping {source.name} "
                    f"({envelope.document})",
                    file=sys.stderr,
                    flush=True,
                )
                skipped.append(Skipped(source, envelope.document))
                if source.name in generated:
                    (output / source.name).write_text(
                        json.dumps(payload, indent=2), encoding="utf-8"
                    )
                continue
            if (
                envelope.document == "country-smallest"
                and payload["result"]["circle"]["target"]["total"] == 0
            ):
                print(
                    f"Gallery: [{index}/{len(sources)}] skipping {source.name} (zero population)",
                    file=sys.stderr,
                    flush=True,
                )
                skipped.append(Skipped(source, "zero-population country"))
                unavailable.add(payload["result"]["country"]["name"])
                (output / source.name).unlink(missing_ok=True)
                (output / f"{source.stem}.png").unlink(missing_ok=True)
                continue
            render_jobs.append((source, payload, index, len(sources)))

        drawn_results, workers, render_seconds = render_documents(render_jobs, context)
        for drawn_map, reused in drawn_results:
            drawn.append(drawn_map)
            if reused:
                reused_maps += 1
            else:
                rendered_maps += 1
        print(
            f"Gallery: maps used {workers} workers and took {render_seconds:.2f}s",
            file=sys.stderr,
            flush=True,
        )

        for name, payload in generated.items():
            destination = corpus / name
            content = (json.dumps(payload, indent=2) + "\n").encode()
            if destination.is_file() and same_country_answer(destination.read_bytes(), content):
                continue
            destination.write_bytes(content)
        data = page_data(drawn, sorted(unavailable))
        (output / "gallery-data.json").write_text(
            json.dumps(data, indent=2) + "\n", encoding="utf-8"
        )
    except Exception:
        (output / "index.html").unlink(missing_ok=True)
        (output / "gallery-data.json").unlink(missing_ok=True)
        raise
    print(
        f"Gallery: complete ({len(drawn)} maps, {len(skipped)} skipped; "
        f"{reused_maps} reused, {rendered_maps} rendered)",
        file=sys.stderr,
        flush=True,
    )
    return drawn, skipped


def omit_excluded_sources(
    sources: list[tuple[Path, dict[str, Any]]], output: Path, excluded: set[str]
) -> tuple[list[tuple[Path, dict[str, Any]]], list[Skipped]]:
    rendered: list[tuple[Path, dict[str, Any]]] = []
    skipped: list[Skipped] = []
    for source, payload in sources:
        if payload.get("result", {}).get("country", {}).get("id") in excluded:
            (output / source.name).unlink(missing_ok=True)
            (output / f"{source.stem}.png").unlink(missing_ok=True)
            skipped.append(Skipped(source, "excluded country"))
        else:
            rendered.append((source, payload))
    return rendered, skipped


def country_result_document(
    sweep: dict[str, Any], selection: dict[str, Any], swept_country: dict[str, Any]
) -> dict[str, Any]:
    country = swept_country["country"]
    record = next(record for record in swept_country["records"] if record["share"] == HALF_SHARE)
    return {
        "schema_version": sweep["schema_version"],
        "document": "country-smallest",
        "tool": sweep["tool"],
        "tool_version": sweep["tool_version"],
        "earth_model": sweep["earth_model"],
        "result": {
            "selection": selection,
            "country": country,
            "population_dataset": selection["population_source"]["name"],
            "decimation": selection["resolution"],
            "ledger": swept_country["ledger"],
            "circle": {key: value for key, value in record["circle"].items() if key != "stats"},
        },
    }


def stable_country_sweep(sweep: dict[str, Any]) -> dict[str, Any]:
    return {
        **sweep,
        "result": {
            **sweep["result"],
            "countries": [
                {
                    **country,
                    "records": [
                        {
                            **record,
                            "circle": {
                                key: value
                                for key, value in record["circle"].items()
                                if key != "stats"
                            },
                        }
                        for record in country["records"]
                    ],
                }
                for country in sweep["result"]["countries"]
            ],
        },
    }


def same_country_answer(existing: bytes, generated: bytes) -> bool:
    try:
        previous_value: Any = json.loads(existing)
        current_value: Any = json.loads(generated)
        if not isinstance(previous_value, dict) or not isinstance(current_value, dict):
            return False
        previous = cast(dict[str, Any], previous_value)
        current = cast(dict[str, Any], current_value)
        document = current.get("document")
        if document not in {"country-sweep", "country-smallest"}:
            return False
        if previous.get("document") != document:
            return False
        for payload in (previous, current):
            result = payload["result"]
            if document == "country-smallest":
                result["circle"].pop("stats", None)
            else:
                for country in result["countries"]:
                    for record in country["records"]:
                        record["circle"].pop("stats", None)
    except AttributeError, KeyError, TypeError, UnicodeDecodeError, json.JSONDecodeError:
        return False
    else:
        return previous == current


def missing_country_names(sweep: dict[str, Any], countries: list[dict[str, Any]]) -> list[str]:
    expected = {country["id"]: country["name"] for country in countries}
    accounted_for: set[str] = set()
    for country in sweep["result"]["countries"]:
        identity = country["country"]
        country_id = identity["id"]
        if country_id not in expected or expected[country_id] != identity["name"]:
            message = f"country sweep: unexpected feature ID {country_id!r}"
            raise ValueError(message)
        if country_id in accounted_for:
            message = f"country sweep: duplicate feature ID {country_id!r}"
            raise ValueError(message)
        accounted_for.add(country_id)
    missing: list[str] = []
    for country in sweep["result"]["countries_without_population_cells"]:
        country_id = country["id"]
        if country_id not in expected or expected[country_id] != country["name"]:
            message = f"country sweep: unexpected unavailable feature ID {country_id!r}"
            raise ValueError(message)
        if country_id in accounted_for:
            message = f"country sweep: duplicate feature ID {country_id!r}"
            raise ValueError(message)
        accounted_for.add(country_id)
        missing.append(country_id)
    if accounted_for != expected.keys():
        message = "country sweep: result does not account for every configured country"
        raise ValueError(message)
    excluded = excluded_country_ids()
    return [expected[country_id] for country_id in missing if country_id not in excluded]


def cached_sweep(
    path: Path, selection: dict[str, Any], countries: list[dict[str, Any]]
) -> dict[str, Any] | None:
    try:
        sweep: Any = json.loads(path.read_text(encoding="utf-8"))
        if Envelope.model_validate(sweep).document != "country-sweep":
            return None
        if sweep["result"]["selection"] != selection:
            return None
        swept_countries = sweep["result"]["countries"]
        missing_country_names(sweep, countries)
        for country in swept_countries:
            if sum(record["share"] == HALF_SHARE for record in country["records"]) != 1:
                return None
            circle_of(country_result_document(sweep, selection, country))
    except OSError, ValueError, TypeError, KeyError, IndexError:
        return None
    return sweep


def report_sweep_work(sweep: dict[str, Any]) -> None:
    evaluated = reused = 0
    for country in sweep["result"]["countries"]:
        for record in country["records"]:
            stats = record["circle"].get("stats", {})
            evaluated += stats.get("radii_evaluated", 0)
            reused += stats.get("radii_reused", 0)
    print(
        f"Gallery: country sweep evaluated {evaluated} radii and reused {reused}",
        file=sys.stderr,
        flush=True,
    )


def country_documents(
    invoke: Invoke, corpus: Path, *, refresh_country_sweep: bool = False
) -> tuple[list[str], dict[str, dict[str, Any]]]:
    listed = child("country list", ["country", "list"], invoke)
    require_success("country list", listed)
    listing: Any = json.loads(listed.stdout)
    if listing.get("document") != "country-list":
        message = "country list: Rust CLI returned a document other than country-list"
        raise ValueError(message)
    selection: dict[str, Any] = listing["result"]["selection"]
    countries: list[dict[str, Any]] = listing["result"]["countries"]
    country_ids = [country["id"] for country in countries]
    if len(country_ids) != len(set(country_ids)):
        message = "country list: Rust CLI returned duplicate feature IDs"
        raise ValueError(message)

    sweep_filename = (
        f"country-sweep-{quote(selection['combination_id'], safe='')}-"
        f"{selection['resolution']}-half.json"
    )
    sweep = (
        None
        if refresh_country_sweep
        else cached_sweep(corpus / sweep_filename, selection, countries)
    )
    arguments = [
        "country",
        "sweep",
        "--combination",
        selection["combination_id"],
        "--resolution",
        str(selection["resolution"]),
        "--from",
        HALF_SHARE_PERCENT,
        "--to",
        HALF_SHARE_PERCENT,
        "--step",
        "1",
    ]
    for country_id in country_ids:
        arguments.extend(["--country", country_id])
    if sweep is None:
        print(
            f"Gallery: calculating 50% circles for {len(countries)} countries",
            file=sys.stderr,
            flush=True,
        )
        swept = child("country sweep", arguments, invoke)
        require_success("country sweep", swept)
        sweep = json.loads(swept.stdout)
        if sweep.get("document") != "country-sweep":
            message = "country sweep: Rust CLI returned a document other than country-sweep"
            raise ValueError(message)
        report_sweep_work(sweep)
    else:
        print(
            "Gallery: reused saved 50% country sweep; "
            f"sweep work avoided for {len(countries)} countries",
            file=sys.stderr,
            flush=True,
        )

    sweep_selection = sweep["result"]["selection"]
    if sweep_selection != listing["result"]["selection"]:
        message = "country sweep: result selection differs from the configured country list"
        raise ValueError(message)
    missing = missing_country_names(sweep, countries)
    expected = {country["id"]: country for country in countries}
    result_ids: set[str] = set()
    documents: dict[str, dict[str, Any]] = {}
    for swept_country in sweep["result"]["countries"]:
        country = swept_country["country"]
        country_id = country["id"]
        if country_id not in expected or country_id in result_ids:
            message = f"country sweep: unexpected or duplicate feature ID {country_id!r}"
            raise ValueError(message)
        records = [record for record in swept_country["records"] if record["share"] == HALF_SHARE]
        if len(records) != 1:
            message = f"country sweep: {country_id!r} has no unique 50% result"
            raise ValueError(message)
        result_ids.add(country_id)
        document = country_result_document(sweep, sweep_selection, swept_country)
        filename = (
            f"country-{quote(selection['combination_id'], safe='')}-"
            f"{selection['resolution']}-{quote(country_id, safe='')}-half.json"
        )
        documents[filename] = document

    documents[sweep_filename] = stable_country_sweep(sweep)
    print(
        f"Gallery: prepared {len(documents)} country result documents",
        file=sys.stderr,
        flush=True,
    )
    return (
        missing,
        documents,
    )


def build(
    corpus: Path,
    output: Path,
    invoke: Invoke = invoke_cli,
    *,
    refresh_country_sweep: bool = False,
    cache_directory: Path | None = None,
) -> tuple[list[Drawn], list[Skipped]]:
    # One basemap union and one registry read for the whole run, which is the only reason this is a
    # second caller of `render` rather than a loop over `render_map.main`.
    registry = load()
    coastline, coastline_attribution = registered_coastline(registry)
    output.mkdir(parents=True, exist_ok=True)
    cache_directory = cache_directory or output.parent / ".gallery-cache"
    try:
        cache_directory.mkdir(parents=True, exist_ok=True)
    except OSError:
        cache_directory = None
    context_digest = render_context_digest()
    render_context = MapRenderContext(
        registry,
        coastline,
        coastline_attribution,
        output,
        cache_directory,
        context_digest,
        map_text_placement(),
    )
    (output / "index.html").unlink(missing_ok=True)
    (output / "gallery-data.json").unlink(missing_ok=True)
    corpus.mkdir(parents=True, exist_ok=True)
    missing, generated = country_documents(
        invoke, corpus, refresh_country_sweep=refresh_country_sweep
    )

    sources = [
        (source, json.loads(source.read_text(encoding="utf-8")))
        for source in sorted(corpus.glob("*.json"))
        if source.name not in generated
    ]
    sources.extend((corpus / name, payload) for name, payload in generated.items())
    sources.sort(key=lambda item: item[0].name)
    return build_gallery_output(
        corpus,
        sources,
        generated,
        missing,
        render_context,
    )


def parse(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Render every renderable document in a corpus.")
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--refresh-country-sweep", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse(argv)
    corpus: Path = args.input
    output: Path = args.output

    drawn, skipped = build(corpus, output, refresh_country_sweep=args.refresh_country_sweep)
    for one in drawn:
        print(one.figure)
    for one in skipped:
        if one.kind == "zero-population country":
            print(f"{one.document}: skipped, country population is zero")
        else:
            print(f"{one.document}: skipped, kind {one.kind!r} carries no circle to draw")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
