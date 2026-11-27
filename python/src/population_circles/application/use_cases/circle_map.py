import json
from collections.abc import Callable, Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol, cast

from shapely.geometry.base import BaseGeometry

from population_circles.application.errors import PopulationApplicationError
from population_circles.application.ports import PopulationRunner
from population_circles.application.use_cases.territory_coverage import enrich_document
from population_circles.documents.result_document import Answer, circle_of

BoundaryLoader = Callable[[Answer], Mapping[str, BaseGeometry]]
FilePublisher = Callable[[Path, bytes], None]


class CircleMapRenderer(Protocol):
    def __call__(
        self,
        input_path: Path,
        output_path: Path,
        projection: str,
        *,
        no_coastlines: bool = False,
        coverage_enriched: bool = False,
    ) -> None: ...


@dataclass(frozen=True)
class CircleMapRequest:
    dataset: str
    share: str
    cache: Path
    output: Path
    document: Path
    ledger: Path
    decimate: str = "1"
    spacing: str | None = None
    projection: str = "plate-carree"
    no_coastlines: bool = False


def _validate_distinct_circle_map_output_paths(options: CircleMapRequest) -> None:
    destinations = (options.output, options.document, options.ledger)
    for index, path in enumerate(destinations):
        for other in destinations[index + 1 :]:
            if path.resolve() == other.resolve() or (
                path.exists() and other.exists() and path.samefile(other)
            ):
                message = (
                    f"destinations: image, document and ledger must differ: {path} and {other}"
                )
                raise PopulationApplicationError(message)


def _enrich_search_document_with_territory_coverage(
    content: bytes, load_boundaries: BoundaryLoader
) -> bytes:
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
    boundaries = load_boundaries(answer)
    enrich_document(payload, boundaries)
    return (json.dumps(payload, indent=2, ensure_ascii=False) + "\n").encode()


def search_and_render_circle_map(
    options: CircleMapRequest,
    run_population_command: PopulationRunner,
    *,
    load_boundaries: BoundaryLoader,
    publish_file: FilePublisher,
    render_file: CircleMapRenderer,
) -> None:
    try:
        _validate_distinct_circle_map_output_paths(options)
    except OSError as error:
        message = f"destinations: {error}"
        raise PopulationApplicationError(message) from error
    common = [
        "--dataset",
        options.dataset,
        "--cache",
        str(options.cache),
        "--decimate",
        options.decimate,
    ]
    probe = run_population_command(
        "cache probe", ["table", "query", *common], accepted_exit_codes=(0, 3)
    )
    if probe.returncode == 3:  # noqa: PLR2004 — Rust's missing/incompatible input exit code
        run_population_command("table build", ["table", "build", *common])
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
    result = run_population_command("search", search)
    try:
        publish_file(
            options.document,
            _enrich_search_document_with_territory_coverage(result.stdout, load_boundaries),
        )
    except OSError as error:
        message = f"document publication: could not replace {options.document}: {error}"
        raise PopulationApplicationError(message) from error
    try:
        render_file(
            options.document,
            options.output,
            options.projection,
            no_coastlines=options.no_coastlines,
            coverage_enriched=True,
        )
    except Exception as error:
        # Rendering errors vary by library; the retained document locates the failure.
        message = f"render: {error}; current search document retained at {options.document}"
        raise PopulationApplicationError(message) from error
    sidecar = options.output.with_suffix(".json")
    if sidecar.resolve() != options.document.resolve():
        try:
            publish_file(options.document, sidecar.read_bytes())
        except OSError as error:
            message = f"document publication: could not replace {options.document}: {error}"
            raise PopulationApplicationError(message) from error
