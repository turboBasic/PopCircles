import functools
import hashlib
import json
from pathlib import Path
from typing import Any, NoReturn, cast

from shapely.geometry import shape
from shapely.geometry.base import BaseGeometry

from population_circles.documents.result_document import Answer, CountryIdentity
from population_circles.infrastructure.geodata.data_config import country_boundary_dataset_key
from population_circles.infrastructure.geodata.dataset_registry import (
    REPO_ROOT,
    BoundaryVector,
    Registry,
)


class BoundaryIdentityError(ValueError):
    pass


class CountryBoundaries(dict[str, BaseGeometry]):
    names: dict[str, str]

    def __init__(self, geometries: dict[str, BaseGeometry], names: dict[str, str]) -> None:
        super().__init__(geometries)
        self.names = names


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


def _boundaries_from_file(
    path: Path, source_id_field: str, display_name_field: str | None = None
) -> CountryBoundaries:
    collection = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(collection, dict):
        refuse(f"country boundary file {path} has no feature collection")
    collection = cast(dict[str, object], collection)
    features = collection.get("features")
    if not isinstance(features, list):
        refuse(f"country boundary file {path} has no feature list")
    boundaries: dict[str, BaseGeometry] = {}
    names: dict[str, str] = {}
    for raw_feature in cast(list[object], features):
        if not isinstance(raw_feature, dict):
            refuse("country boundary file contains a malformed feature")
        feature = cast(dict[str, object], raw_feature)
        properties = feature.get("properties")
        if not isinstance(properties, dict):
            refuse(f"country boundary feature has no {source_id_field} identifier")
        properties = cast(dict[str, object], properties)
        identifier = _feature_id(feature, source_id_field)
        if identifier in boundaries:
            refuse(f"country boundary file has duplicate {source_id_field} {identifier!r}")
        geometry = feature.get("geometry")
        if not isinstance(geometry, dict):
            refuse(f"country boundary feature {identifier!r} has no geometry")
        boundaries[identifier] = shape(cast(dict[str, Any], geometry))
        if display_name_field is not None:
            name = properties.get(display_name_field)
            if not isinstance(name, str) or not name:
                refuse(f"country boundary feature {identifier!r} has no {display_name_field} name")
            names[identifier] = name
    return CountryBoundaries(boundaries, names)


def _registered_boundaries(dataset: BoundaryVector, root: Path) -> CountryBoundaries:
    if dataset.source_id_field is None:
        refuse("country boundary dataset has no registered identifier field", BoundaryIdentityError)
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
    return _cached_boundaries(path, dataset.source_id_field, dataset.display_name_field, digest)


# Gallery maps share one verified boundary snapshot; keep its parsed geometries for the process.
@functools.cache
def _cached_boundaries(
    path: Path, source_id_field: str, display_name_field: str | None, _verified_digest: str
) -> CountryBoundaries:
    return _boundaries_from_file(path, source_id_field, display_name_field)


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
    if dataset.source_id_field != source_id_field:
        refuse(
            "country boundary identifier field does not match the registry", BoundaryIdentityError
        )
    boundaries = _registered_boundaries(dataset, root)
    if country.id not in boundaries:
        refuse(f"country boundary file has no feature with {source_id_field}={country.id!r}")
    return boundaries


def load_map_boundaries(
    answer: Answer, registry: Registry, root: Path = REPO_ROOT
) -> dict[str, BaseGeometry]:
    if answer.country is not None:
        boundaries = load_country_boundaries(answer, registry, root)
        return boundaries or {}
    provenance = (
        answer.caption.country_breakdown.provenance if answer.caption.country_breakdown else None
    )
    key = provenance.boundary_dataset if provenance is not None else country_boundary_dataset_key()
    dataset = registry.datasets.get(key)
    if dataset is None and provenance is None:
        return {}
    if not isinstance(dataset, BoundaryVector) or dataset.revision is None:
        refuse(f"country boundary dataset {key!r} is not registered", BoundaryIdentityError)
    if provenance is not None and (
        provenance.boundary_revision != dataset.revision
        or provenance.boundary_digest.casefold() != dataset.sha256.casefold()
    ):
        refuse(
            f"country boundary identity for map does not match dataset {key!r}",
            BoundaryIdentityError,
        )
    return _registered_boundaries(dataset, root)


def load_outline(answer: Answer, registry: Registry, root: Path = REPO_ROOT) -> BaseGeometry | None:
    boundaries = load_country_boundaries(answer, registry, root)
    if boundaries is None or answer.country is None:
        return None
    return boundaries[answer.country.id]
