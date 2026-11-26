import json
from dataclasses import dataclass
from pathlib import Path
from tempfile import NamedTemporaryFile
from typing import cast

from population_circles.applications.population_engine import (
    PopulationRunner,
    WorkflowError,
    check_returncode,
    invoke,
    run_stage,
)
from population_circles.data.dataset_registry import load
from population_circles.documents.result_document import circle_of
from population_circles.geography.country_boundaries import load_map_boundaries
from population_circles.geography.territory_share import enrich_document
from population_circles.maps import render_map
from population_circles.maps.frame import PLATE_CARREE


@dataclass(frozen=True)
class Options:
    dataset: str
    share: str
    cache: Path
    output: Path
    document: Path
    ledger: Path
    decimate: str = "1"
    spacing: str | None = None
    projection: str = PLATE_CARREE
    no_coastlines: bool = False


def check_destinations(options: Options) -> None:
    destinations = (options.output, options.document, options.ledger)
    for index, path in enumerate(destinations):
        for other in destinations[index + 1 :]:
            if path.resolve() == other.resolve() or (
                path.exists() and other.exists() and path.samefile(other)
            ):
                message = (
                    f"destinations: image, document and ledger must differ: {path} and {other}"
                )
                raise WorkflowError(message)


def publish(destination: Path, content: bytes) -> None:
    temporary: Path | None = None
    try:
        destination.parent.mkdir(parents=True, exist_ok=True)
        with NamedTemporaryFile(
            dir=destination.parent, prefix=f".{destination.name}.", delete=False
        ) as stream:
            temporary = Path(stream.name)
            stream.write(content)
        temporary.replace(destination)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def _enriched_search_result(content: bytes) -> bytes:
    try:
        document = json.loads(content)
    except json.JSONDecodeError:
        return content
    if not isinstance(document, dict):
        return content
    payload = cast(dict[str, object], document)
    if payload.get("schema_version") not in {1, 2}:
        return content
    answer = circle_of(payload)
    boundaries = load_map_boundaries(answer, load())
    enrich_document(payload, boundaries)
    return (json.dumps(payload, indent=2, ensure_ascii=False) + "\n").encode()


def search_and_render_circle_map(options: Options, invoke: PopulationRunner = invoke) -> None:
    try:
        check_destinations(options)
    except OSError as error:
        message = f"destinations: {error}"
        raise WorkflowError(message) from error
    common = [
        "--dataset",
        options.dataset,
        "--cache",
        str(options.cache),
        "--decimate",
        options.decimate,
    ]
    probe = run_stage("cache probe", ["table", "query", *common], invoke)
    if probe.returncode == 3:  # noqa: PLR2004 — Rust's missing/incompatible input exit code
        build = run_stage("table build", ["table", "build", *common], invoke)
        check_returncode("table build", build)
    else:
        check_returncode("cache probe", probe)
    search = [
        "smallest-for-share",
        *common,
        "--share",
        options.share,
        "--ledger",
        str(options.ledger),
    ]
    if options.spacing is not None:
        search.extend(["--spacing", options.spacing])
    result = run_stage("search", search, invoke)
    check_returncode("search", result)
    try:
        publish(options.document, _enriched_search_result(result.stdout))
    except OSError as error:
        message = f"document publication: could not replace {options.document}: {error}"
        raise WorkflowError(message) from error
    try:
        render_map.render_file(
            options.document,
            options.output,
            options.projection,
            no_coastlines=options.no_coastlines,
            coverage_enriched=True,
        )
    except Exception as error:
        # Rendering errors vary by library; the retained document locates the failure.
        message = f"render: {error}; current search document retained at {options.document}"
        raise WorkflowError(message) from error
    sidecar = options.output.with_suffix(".json")
    if sidecar.resolve() != options.document.resolve():
        try:
            publish(options.document, sidecar.read_bytes())
        except OSError as error:
            message = f"document publication: could not replace {options.document}: {error}"
            raise WorkflowError(message) from error
