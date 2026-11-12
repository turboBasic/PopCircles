import hashlib
import json
from pathlib import Path
from typing import Any, NoReturn, cast

from shapely.geometry import shape
from shapely.geometry.base import BaseGeometry

from population_circles.circle_document import Answer, CountryIdentity
from population_circles.dataset_registry import REPO_ROOT, BoundaryVector, Registry


class BoundaryIdentityError(ValueError):
    pass


def refuse(message: str, error: type[Exception] = ValueError) -> NoReturn:
    raise error(message)


def _feature_id(feature: dict[str, object], source_id_field: str) -> str:
    properties = feature.get("properties")
    if not isinstance(properties, dict):
        refuse(f"country boundary feature has no {source_id_field} identifier")
    properties = cast(dict[str, object], properties)
    identifier = properties.get(source_id_field)
    if not isinstance(identifier, str) or not identifier:
        refuse(f"country boundary feature has no {source_id_field} identifier")
    return identifier


def _registered_boundary(
    answer: Answer,
    registry: Registry,
) -> tuple[CountryIdentity, BoundaryVector, str]:
    country = answer.country
    if country is None:
        refuse("the result document does not identify a country")
    dataset = registry.datasets.get(country.boundary_dataset)
    if not isinstance(dataset, BoundaryVector):
        refuse(
            f"country boundary dataset {country.boundary_dataset!r} is not registered",
            BoundaryIdentityError,
        )
    if dataset.source_id_field is None or dataset.revision is None:
        refuse(
            f"country boundary dataset {country.boundary_dataset!r} has no registered identity",
            BoundaryIdentityError,
        )
    if (
        country.boundary_revision is None
        or dataset.revision != country.boundary_revision
        or dataset.sha256.casefold() != country.boundary_digest.casefold()
    ):
        refuse(
            f"country boundary identity for {country.id!r} does not match the registry",
            BoundaryIdentityError,
        )
    return country, dataset, dataset.source_id_field


def _boundaries_from_file(path: Path, source_id_field: str) -> dict[str, BaseGeometry]:
    collection = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(collection, dict) or not isinstance(collection.get("features"), list):
        refuse(f"country boundary file {path} has no feature collection")
    boundaries: dict[str, BaseGeometry] = {}
    for raw_feature in collection["features"]:
        if not isinstance(raw_feature, dict):
            refuse("country boundary file contains a malformed feature")
        feature = cast(dict[str, object], raw_feature)
        identifier = _feature_id(feature, source_id_field)
        if identifier in boundaries:
            refuse(f"country boundary file has duplicate {source_id_field} {identifier!r}")
        geometry = feature.get("geometry")
        if not isinstance(geometry, dict):
            refuse(f"country boundary feature {identifier!r} has no geometry")
        boundaries[identifier] = shape(cast(dict[str, Any], geometry))
    return boundaries


def load_country_boundaries(
    answer: Answer, registry: Registry, root: Path = REPO_ROOT
) -> dict[str, BaseGeometry] | None:
    if (
        answer.country is not None
        and answer.selection is not None
        and answer.selection.boundary_source.name.startswith("file:")
        and answer.country.boundary_dataset not in registry.datasets
    ):
        return None
    country, dataset, source_id_field = _registered_boundary(answer, registry)
    path = dataset.file(root)
    if not path.is_file():
        refuse(
            f"country boundary file is missing at {path}; run mise run data:get",
            FileNotFoundError,
        )
    if path.stat().st_size != dataset.bytes:
        refuse(
            f"country boundary file {path} does not match the registry; run mise run data:get",
            BoundaryIdentityError,
        )
    with path.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    if digest != dataset.sha256:
        refuse(
            f"country boundary file {path} does not match the registry; run mise run data:get",
            BoundaryIdentityError,
        )
    boundaries = _boundaries_from_file(path, source_id_field)
    if country.id not in boundaries:
        refuse(f"country boundary file has no feature with {source_id_field}={country.id!r}")
    return boundaries


def load_outline(answer: Answer, registry: Registry, root: Path = REPO_ROOT) -> BaseGeometry | None:
    boundaries = load_country_boundaries(answer, registry, root)
    if boundaries is None or answer.country is None:
        return None
    return boundaries[answer.country.id]
