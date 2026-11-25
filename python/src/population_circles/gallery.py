import argparse
import json
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, cast
from urllib.parse import quote

from PIL import Image, ImageChops

from population_circles import gallery_country_sweep as country_sweep_module
from population_circles.circle_map_workflow import Invoke, child, invoke_cli, require_success
from population_circles.country_boundaries import load_map_boundaries
from population_circles.dataset_registry import REPO_ROOT, Registry, load
from population_circles.gallery_country_sweep import (
    HALF_SHARE,
    CountryRefresh,
    CountrySweepRequest,
)
from population_circles.gallery_page import page_data
from population_circles.gallery_rendering import (
    Drawn,
    MapRenderContext,
    RenderJob,
    render_context_digest,
    render_documents,
)
from population_circles.map_frame import ORTHOGRAPHIC
from population_circles.map_layers import registered_coastline
from population_circles.presentation_config import load as load_presentation_config
from population_circles.render_map import OCEAN
from population_circles.result_document import CIRCLE_KINDS, Envelope, circle_of
from population_circles.territory_share import enrich_document

HEADER_OCEAN_MATCH_TOLERANCE = 8
HEADER_CROP_ASPECT_RATIO = 2
HEADER_CROP_ASPECT_TOLERANCE = 2


@dataclass(frozen=True)
class Skipped:
    document: Path
    kind: str


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


def crop_header_background(source: Path, target: Path) -> None:
    with Image.open(source) as original:
        image = original.convert("RGB")
        ocean = Image.new("RGB", image.size, OCEAN)
        lookup = [255 if value < HEADER_OCEAN_MATCH_TOLERANCE else 0 for value in range(256)]
        mask = ImageChops.difference(image, ocean).convert("L").point(lookup)
        bounds = mask.getbbox()
        if bounds is None:
            # shortcut: fallback assumes render's 11x6 world axes; update when those bounds change.
            left, top, right = (
                round(image.width * 0.113),
                round(image.height * 0.131),
                round(image.width * 0.887),
            )
            bottom = round(image.height * 0.839)
        else:
            left, top, right, _ = bounds
            # The 2:1 Plate Carree frame extends beyond the ocean pixels at its land-filled edge.
            bottom = top + round((right - left) / HEADER_CROP_ASPECT_RATIO) - 1
        if bottom <= top or bottom > image.height:
            raise ValueError
        image.crop((left, top, right, bottom)).save(target)


def write_header_background(drawn: list[Drawn], output: Path) -> None:
    world_map = next((one.figure for one in drawn if one.country_name is None), None)
    if world_map is not None:
        crop_header_background(world_map, output / "header-background.png")


def enrich_source(source: Path, payload: dict[str, Any], registry: Registry) -> dict[str, Any]:
    answer = circle_of(payload)
    enriched = enrich_document(payload, load_map_boundaries(answer, registry))
    if enriched != payload:
        source.write_text(
            json.dumps(enriched, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
        )
    return enriched


def render_jobs_for(
    source: Path, payload: dict[str, Any], index: int, total: int
) -> tuple[RenderJob, ...]:
    job = RenderJob(source, payload, index, total)
    answer = circle_of(payload)
    share = (
        payload["result"]["circle"]["target"]["share"]
        if payload["document"] == "smallest"
        else answer.caption.share
    )
    if answer.country is None and share == HALF_SHARE:
        return job, RenderJob(source, payload, index, total, ORTHOGRAPHIC, "-orthographic")
    return (job,)


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
    excluded = country_sweep_module.excluded_country_ids()
    sources, excluded_sources = omit_excluded_sources(sources, output, excluded)
    skipped.extend(excluded_sources)
    render_jobs: list[RenderJob] = []
    try:
        (output / "header-background.png").unlink(missing_ok=True)
        build_frontend(output)
        for index, (source, source_payload) in enumerate(sources, start=1):
            envelope = Envelope.model_validate(source_payload)
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
                        json.dumps(source_payload, indent=2, ensure_ascii=False) + "\n",
                        encoding="utf-8",
                    )
                continue
            payload = enrich_source(source, source_payload, context.registry)
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
            render_jobs.extend(render_jobs_for(source, payload, index, len(sources)))

        render_jobs = [
            RenderJob(
                job.source,
                job.payload,
                index,
                len(render_jobs),
                job.projection,
                job.figure_suffix,
            )
            for index, job in enumerate(render_jobs, start=1)
        ]
        print(
            f"Gallery: rendering {len(render_jobs)} maps from {len(sources)} result documents",
            file=sys.stderr,
            flush=True,
        )
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
        write_header_background(drawn, output)

        for name, payload in generated.items():
            destination = corpus / name
            content = (json.dumps(payload, indent=2, ensure_ascii=False) + "\n").encode()
            if destination.is_file() and same_country_answer(destination.read_bytes(), content):
                continue
            destination.write_bytes(content)
        data = page_data(drawn, sorted(unavailable))
        (output / "gallery-data.json").write_text(
            json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
        )
    except Exception:
        (output / "index.html").unlink(missing_ok=True)
        (output / "gallery-data.json").unlink(missing_ok=True)
        raise
    print(
        f"Gallery: complete ({len(drawn)} maps from {len(sources)} result documents, "
        f"{len(skipped)} skipped; "
        f"{reused_maps} reused, {rendered_maps} rendered)",
        file=sys.stderr,
        flush=True,
    )
    return drawn, skipped


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
            payload.pop("render_metadata", None)
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


def country_documents(
    invoke: Invoke,
    corpus: Path,
    *,
    country_refresh: CountryRefresh | None = None,
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
    sweep = country_sweep_module.country_sweep(
        CountrySweepRequest(
            invoke,
            corpus,
            selection,
            countries,
            country_refresh,
            report_country_sweep,
        )
    )
    sweep_selection = sweep["result"]["selection"]
    if sweep_selection != listing["result"]["selection"]:
        message = "country sweep: result selection differs from the configured country list"
        raise ValueError(message)
    missing = country_sweep_module.missing_country_names(sweep, countries)
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
        document = country_sweep_module.country_result_document(
            sweep, sweep_selection, swept_country
        )
        filename = (
            f"country-{quote(selection['combination_id'], safe='')}-"
            f"{selection['resolution']}-{quote(country_id, safe='')}-half.json"
        )
        documents[filename] = document

    documents[sweep_filename] = country_sweep_module.stable_country_sweep(sweep)
    print(
        f"Gallery: prepared {len(documents)} country result documents",
        file=sys.stderr,
        flush=True,
    )
    return (
        missing,
        documents,
    )


def report_country_sweep(message: str) -> None:
    print(message, file=sys.stderr, flush=True)


def build(
    corpus: Path,
    output: Path,
    invoke: Invoke = invoke_cli,
    *,
    country_refresh: CountryRefresh | None = None,
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
    presentation = load_presentation_config()
    render_context = MapRenderContext(
        registry,
        coastline,
        coastline_attribution,
        output,
        cache_directory,
        context_digest,
        (presentation.map_text.render_title, presentation.map_text.render_description),
        map_titles=presentation.map_titles,
    )
    (output / "index.html").unlink(missing_ok=True)
    (output / "gallery-data.json").unlink(missing_ok=True)
    corpus.mkdir(parents=True, exist_ok=True)
    if country_refresh == "coverage":
        country_sweep_module.refresh_coverage(corpus)
    missing, generated = country_documents(
        invoke,
        corpus,
        country_refresh=country_refresh,
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
    refresh = parser.add_mutually_exclusive_group()
    refresh.add_argument(
        "--refresh-country-sweep", dest="country_refresh", action="store_const", const="sweep"
    )
    refresh.add_argument(
        "--refresh-country-coverage", dest="country_refresh", action="store_const", const="coverage"
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse(argv)
    corpus: Path = args.input
    output: Path = args.output

    drawn, skipped = build(
        corpus,
        output,
        country_refresh=args.country_refresh,
    )
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
