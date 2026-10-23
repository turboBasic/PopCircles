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
    if dataset.revision != country.boundary_revision or dataset.sha256.casefold() != (
        country.boundary_digest.casefold()
    ):
        refuse(
            f"country boundary identity for {country.id!r} does not match the registry",
            BoundaryIdentityError,
        )
    return country, dataset, dataset.source_id_field


def _feature_geometry(
    path: Path,
    source_id_field: str,
    country_id: str,
) -> BaseGeometry:
    collection = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(collection, dict):
        refuse(f"country boundary file {path} has no feature collection")
    collection = cast(dict[str, object], collection)
    features = collection.get("features")
    if not isinstance(features, list):
        refuse(f"country boundary file {path} has no feature list")
    features = cast(list[object], features)
    selected: dict[str, object] | None = None
    identifiers: set[str] = set()
    for raw_feature in features:
        if not isinstance(raw_feature, dict):
            refuse("country boundary file contains a malformed feature")
        feature = cast(dict[str, object], raw_feature)
        identifier = _feature_id(feature, source_id_field)
        if identifier in identifiers:
            refuse(f"country boundary file has duplicate {source_id_field} {identifier!r}")
        identifiers.add(identifier)
        if identifier == country_id:
            selected = feature
    if selected is None:
        refuse(f"country boundary file has no feature with {source_id_field}={country_id!r}")
    geometry = selected.get("geometry")
    if not isinstance(geometry, dict):
        refuse(f"country boundary feature {country_id!r} has no geometry")
    return shape(cast(dict[str, Any], geometry))


def load_outline(answer: Answer, registry: Registry, root: Path = REPO_ROOT) -> BaseGeometry:
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
    return _feature_geometry(path, source_id_field, country.id)
