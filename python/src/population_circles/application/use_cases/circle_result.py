import json
from collections.abc import Callable, Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import cast

from shapely.geometry.base import BaseGeometry

from population_circles.application.errors import PopulationApplicationError
from population_circles.application.ports import PopulationRunner
from population_circles.application.use_cases.territory_coverage import enrich_document
from population_circles.documents.result_document import Answer, circle_of

BoundaryLoader = Callable[[Answer], Mapping[str, BaseGeometry]]
FilePublisher = Callable[[Path, bytes], None]


@dataclass(frozen=True)
class CircleResultRequest:
    dataset: str
    share: str
    cache: Path
    document: Path
    ledger: Path
    decimate: str = "1"
    spacing: str | None = None


@dataclass(frozen=True)
class CircleResultCollaborators:
    run_population_command: PopulationRunner
    load_boundaries: BoundaryLoader
    publish_file: FilePublisher


def _validate_distinct_result_paths(request: CircleResultRequest) -> None:
    if request.document.resolve() == request.ledger.resolve() or (
        request.document.exists()
        and request.ledger.exists()
        and request.document.samefile(request.ledger)
    ):
        message = (
            "destinations: result document and radii ledger must differ: "
            f"{request.document} and {request.ledger}"
        )
        raise PopulationApplicationError(message)


def _enrich_circle_result_with_territory_coverage(
    content: bytes, load_boundaries: BoundaryLoader
) -> bytes:
    try:
        document = json.loads(content)
        payload = cast(dict[str, object], document)
        answer = circle_of(payload)
    except (json.JSONDecodeError, ValueError) as error:
        message = f"search: Rust CLI returned an invalid circle result: {error}"
        raise PopulationApplicationError(message) from error
    enrich_document(payload, load_boundaries(answer))
    return (json.dumps(payload, indent=2, ensure_ascii=False) + "\n").encode()


def search_and_publish_circle_result(
    request: CircleResultRequest,
    collaborators: CircleResultCollaborators,
) -> None:
    try:
        _validate_distinct_result_paths(request)
    except OSError as error:
        message = f"destinations: {error}"
        raise PopulationApplicationError(message) from error
    common = [
        "--dataset",
        request.dataset,
        "--cache",
        str(request.cache),
        "--decimate",
        request.decimate,
    ]
    probe = collaborators.run_population_command(
        "cache probe", ["table", "query", *common], accepted_exit_codes=(0, 3)
    )
    if probe.returncode == 3:  # noqa: PLR2004 — Rust's missing/incompatible input exit code
        collaborators.run_population_command("table build", ["table", "build", *common])
    search = [
        "smallest-for-share",
        *common,
        "--share",
        request.share,
        "--ledger",
        str(request.ledger),
    ]
    if request.spacing is not None:
        search.extend(["--spacing", request.spacing])
    result = collaborators.run_population_command("search", search)
    try:
        collaborators.publish_file(
            request.document,
            _enrich_circle_result_with_territory_coverage(
                result.stdout, collaborators.load_boundaries
            ),
        )
    except OSError as error:
        message = f"result publication: could not replace {request.document}: {error}"
        raise PopulationApplicationError(message) from error
