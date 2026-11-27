import json
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol, cast
from urllib.parse import quote

from population_circles.application.ports import PopulationRunner
from population_circles.application.use_cases import gallery as country_sweep_module
from population_circles.application.use_cases.gallery import (
    HALF_SHARE,
    CountryRefresh,
    CountrySweepCollaborators,
    CountrySweepRequest,
    GalleryFigure,
    RenderJob,
)
from population_circles.documents.result_document import CIRCLE_KINDS, Envelope, circle_of


class GalleryRenderContext(Protocol):
    @property
    def output(self) -> Path: ...


@dataclass(frozen=True)
class GalleryBuildCollaborators:
    run_population_command: PopulationRunner
    prepare_render_context: Callable[[Path, Path | None], GalleryRenderContext]
    enrich_source: Callable[[Path, dict[str, Any]], dict[str, Any]]
    country_sweep: CountrySweepCollaborators
    build_frontend: Callable[[Path], None]
    write_header: Callable[[list[GalleryFigure], Path], None]
    render_documents: Callable[
        [list[RenderJob], GalleryRenderContext],
        tuple[list[tuple[GalleryFigure, bool]], int, float],
    ]
    page_data: Callable[[Sequence[GalleryFigure], list[str]], object]
    report: Callable[[str], None]


@dataclass(frozen=True)
class GalleryBuildRequest:
    corpus: Path
    output: Path
    country_refresh: CountryRefresh | None = None
    cache_directory: Path | None = None


@dataclass(frozen=True)
class Skipped:
    document: Path
    kind: str


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
        return job, RenderJob(source, payload, index, total, "orthographic", "-orthographic")
    return (job,)


@dataclass(frozen=True)
class GalleryOutputRequest:
    corpus: Path
    sources: list[tuple[Path, dict[str, Any]]]
    generated: dict[str, dict[str, Any]]
    missing: list[str]
    context: GalleryRenderContext


def build_gallery_output(  # noqa: PLR0915 — ordered cleanup and publication steps stay explicit
    request: GalleryOutputRequest,
    collaborators: GalleryBuildCollaborators,
) -> tuple[list[GalleryFigure], list[Skipped]]:
    corpus = request.corpus
    sources = request.sources
    generated = request.generated
    context = request.context
    output = context.output
    drawn: list[GalleryFigure] = []
    skipped: list[Skipped] = []
    reused_maps = rendered_maps = 0
    unavailable = set(request.missing)
    excluded = collaborators.country_sweep.excluded_country_ids
    sources, excluded_sources = omit_excluded_sources(sources, output, excluded)
    skipped.extend(excluded_sources)
    render_jobs: list[RenderJob] = []
    try:
        (output / "header-background.png").unlink(missing_ok=True)
        collaborators.build_frontend(output)
        for index, (source, source_payload) in enumerate(sources, start=1):
            envelope = Envelope.model_validate(source_payload)
            if envelope.document not in CIRCLE_KINDS and envelope.document != "country-smallest":
                collaborators.report(
                    f"Gallery: [{index}/{len(sources)}] skipping {source.name} "
                    f"({envelope.document})"
                )
                skipped.append(Skipped(source, envelope.document))
                if source.name in generated:
                    (output / source.name).write_text(
                        json.dumps(source_payload, indent=2, ensure_ascii=False) + "\n",
                        encoding="utf-8",
                    )
                continue
            payload = collaborators.enrich_source(source, source_payload)
            if (
                envelope.document == "country-smallest"
                and payload["result"]["circle"]["target"]["total"] == 0
            ):
                collaborators.report(
                    f"Gallery: [{index}/{len(sources)}] skipping {source.name} (zero population)",
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
        collaborators.report(
            f"Gallery: rendering {len(render_jobs)} maps from {len(sources)} result documents",
        )
        drawn_results, workers, render_seconds = collaborators.render_documents(
            render_jobs, context
        )
        for drawn_map, reused in drawn_results:
            drawn.append(drawn_map)
            if reused:
                reused_maps += 1
            else:
                rendered_maps += 1
        collaborators.report(
            f"Gallery: maps used {workers} workers and took {render_seconds:.2f}s",
        )
        collaborators.write_header(drawn, output)

        for name, payload in generated.items():
            destination = corpus / name
            content = (json.dumps(payload, indent=2, ensure_ascii=False) + "\n").encode()
            if destination.is_file() and same_country_answer(destination.read_bytes(), content):
                continue
            destination.write_bytes(content)
        data = collaborators.page_data(drawn, sorted(unavailable))
        (output / "gallery-data.json").write_text(
            json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
        )
    except Exception:
        (output / "index.html").unlink(missing_ok=True)
        (output / "gallery-data.json").unlink(missing_ok=True)
        raise
    collaborators.report(
        f"Gallery: complete ({len(drawn)} maps from {len(sources)} result documents, "
        f"{len(skipped)} skipped; "
        f"{reused_maps} reused, {rendered_maps} rendered)"
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
    run_population_command: PopulationRunner,
    corpus: Path,
    collaborators: CountrySweepCollaborators,
    *,
    country_refresh: CountryRefresh | None = None,
) -> tuple[list[str], dict[str, dict[str, Any]]]:
    listed = run_population_command("country list", ["country", "list"])
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
            run_population_command,
            corpus,
            selection,
            countries,
            country_refresh,
            collaborators,
        )
    )
    sweep_selection = sweep["result"]["selection"]
    if sweep_selection != listing["result"]["selection"]:
        message = "country sweep: result selection differs from the configured country list"
        raise ValueError(message)
    missing = country_sweep_module.missing_country_names(
        sweep, countries, collaborators.excluded_country_ids
    )
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
    collaborators.report(
        f"Gallery: prepared {len(documents)} country result documents",
    )
    return (
        missing,
        documents,
    )


def build(
    request: GalleryBuildRequest,
    collaborators: GalleryBuildCollaborators,
) -> tuple[list[GalleryFigure], list[Skipped]]:
    corpus = request.corpus
    output = request.output
    country_refresh = request.country_refresh
    output.mkdir(parents=True, exist_ok=True)
    render_context = collaborators.prepare_render_context(output, request.cache_directory)
    (output / "index.html").unlink(missing_ok=True)
    (output / "gallery-data.json").unlink(missing_ok=True)
    corpus.mkdir(parents=True, exist_ok=True)
    if country_refresh == "coverage":
        country_sweep_module.refresh_coverage(
            corpus, collaborators.country_sweep.enrich_saved_document
        )
    missing, generated = country_documents(
        collaborators.run_population_command,
        corpus,
        collaborators.country_sweep,
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
        GalleryOutputRequest(corpus, sources, generated, missing, render_context),
        collaborators,
    )
