import functools
import tomllib
from collections.abc import Mapping
from pathlib import Path, PurePosixPath
from typing import Annotated, Literal, Self, cast

from pydantic import BaseModel, ConfigDict, Field, model_validator

# Resolved from this file's own location, the same way `render_map.py` reaches the committed
# basemap and for the same reason — the project is installed editable, so the checkout is beside
# the package. FU-20 covers both of these together.
REPO_ROOT = Path(__file__).resolve().parents[3]
REGISTRY = REPO_ROOT / "data" / "registry.toml"
SHA256_HEX_LENGTH = 64

# `extra="forbid"` where `circle_document.py` uses `extra="ignore"`, and the difference is which
# side of a boundary the producer sits on. That file consumes a format Rust writes and grows
# additively, so an unknown field must not become a refusal. This file and this reader land in one
# commit, so an unknown key is a typo — and a field added for Rust turns pytest red until the model
# gains it, which is what keeps two readers of one file agreeing.
_CONFIG = ConfigDict(frozen=True, extra="forbid")


class _Dataset(BaseModel):
    model_config = _CONFIG

    path: str
    bytes: int
    sha256: str
    source_url: str
    licence: str
    licence_url: str
    # Empty where nothing is owed, never absent, so a consumer reads one field for every dataset.
    attribution: str
    # Absent for a dataset carried in the repository rather than fetched.
    fetch_url: str | None = None

    # Takes the root rather than closing over `REPO_ROOT`: `path` is repository-relative, and a
    # registry parsed from text that came from somewhere else must not resolve into this checkout.
    def file(self, root: Path) -> Path:
        return root / self.path


class PopulationRaster(_Dataset):
    kind: Literal["population-raster"]

    width: int
    height: int
    origin_lat: float
    origin_lon: float
    lat_step: float
    lon_step: float
    epsg: int
    nodata: float
    derived_from_dataset: str | None = None
    derived_from_sha256: str | None = None

    @model_validator(mode="after")
    def _derived_source_is_complete(self) -> Self:
        if (self.derived_from_dataset is None) != (self.derived_from_sha256 is None):
            message = "derived population source must name both its dataset and SHA-256"
            raise ValueError(message)
        if self.derived_from_sha256 is not None and (
            len(self.derived_from_sha256) != SHA256_HEX_LENGTH
            or any(char not in "0123456789abcdef" for char in self.derived_from_sha256)
        ):
            message = (
                "derived population source SHA-256 must be 64 lowercase hexadecimal characters"
            )
            raise ValueError(message)
        return self


class BoundaryVector(_Dataset):
    kind: Literal["boundary-vector"]

    # Optional for existing basemap vectors; country inputs carry the complete description together.
    format: Literal["GeoJSON"] | None = None
    crs: Literal["EPSG:4326"] | None = None
    axis_order: Literal["longitude-latitude"] | None = None
    feature_count: int | None = Field(default=None, gt=0)
    revision: str | None = None
    source_id_field: str | None = None
    identity_policy: str | None = None

    @model_validator(mode="after")
    def _country_metadata_is_complete(self) -> Self:
        metadata = (
            self.format,
            self.crs,
            self.axis_order,
            self.feature_count,
            self.source_id_field,
            self.identity_policy,
        )
        if any(value is not None for value in metadata) and any(
            value is None for value in metadata
        ):
            message = "country boundary metadata must be complete"
            raise ValueError(message)
        return self


class SettlementVector(_Dataset):
    kind: Literal["settlement-vector"]


# Discriminated on `kind` so a raster without a grid does not construct, rather than every reader
# re-checking that the eight grid fields are present.
Dataset = Annotated[
    PopulationRaster | BoundaryVector | SettlementVector, Field(discriminator="kind")
]


class Registry(BaseModel):
    model_config = _CONFIG

    datasets: Mapping[str, Dataset]

    @model_validator(mode="after")
    def _key_is_the_filename_stem(self) -> Self:
        for key, dataset in self.datasets.items():
            stem = PurePosixPath(dataset.path).stem
            if stem != key:
                message = f"dataset {key!r} has path {dataset.path!r}, whose stem is {stem!r}"
                raise ValueError(message)
            if isinstance(dataset, PopulationRaster) and dataset.derived_from_dataset is not None:
                source = self.datasets.get(dataset.derived_from_dataset)
                if not isinstance(source, PopulationRaster) or (
                    source.sha256 != dataset.derived_from_sha256
                    or source.derived_from_dataset is not None
                ):
                    message = f"dataset {key!r} does not match its registered source identity"
                    raise ValueError(message)
        return self


def parse(text: str) -> Registry:
    return Registry.model_validate(tomllib.loads(text))


@functools.cache
def load() -> Registry:
    return parse(REGISTRY.read_text(encoding="utf-8"))


MapTextPlacement = Literal["metadata", "raster"]


def map_text_placement() -> tuple[MapTextPlacement, MapTextPlacement]:
    path = REPO_ROOT / "data" / "config.toml"
    config = tomllib.loads(path.read_text(encoding="utf-8"))
    settings = config.get("map_text", {})
    if not isinstance(settings, dict):
        message = f"{path}: map_text must be a table"
        raise TypeError(message)
    values = cast(dict[str, object], settings)
    placements: list[MapTextPlacement] = []
    for field in ("title", "credits"):
        value = values.get(field, "metadata")
        if value not in ("metadata", "raster"):
            message = f"{path}: map_text.{field} must be 'metadata' or 'raster'"
            raise ValueError(message)
        placements.append(cast(MapTextPlacement, value))
    return placements[0], placements[1]


@functools.cache
def coastline_dataset_key() -> str:
    config = tomllib.loads((REPO_ROOT / "data" / "config.toml").read_text(encoding="utf-8"))
    defaults = config.get("defaults")
    if not isinstance(defaults, dict):
        message = "data/config.toml must set defaults.coastline_dataset"
        raise TypeError(message)
    key = cast(dict[str, object], defaults).get("coastline_dataset")
    if not isinstance(key, str) or not key:
        message = "data/config.toml must set defaults.coastline_dataset"
        raise ValueError(message)
    return key


@functools.cache
def country_boundary_dataset_key() -> str:
    config = tomllib.loads((REPO_ROOT / "data" / "config.toml").read_text(encoding="utf-8"))
    defaults = config.get("defaults")
    combinations = config.get("combinations")
    if not isinstance(defaults, dict) or not isinstance(combinations, dict):
        message = "data/config.toml must define a default country combination"
        raise TypeError(message)
    combinations = cast(dict[str, object], combinations)
    name = cast(dict[str, object], defaults).get("country_combination")
    if not isinstance(name, str) or name not in combinations:
        message = "data/config.toml defaults.country_combination is not configured"
        raise ValueError(message)
    combination = combinations[name]
    if not isinstance(combination, dict):
        message = f"data/config.toml combination {name!r} is malformed"
        raise TypeError(message)
    source = cast(dict[str, object], combination).get("boundary_source")
    prefix = "dataset:"
    if not isinstance(source, str) or not source.startswith(prefix):
        message = f"data/config.toml combination {name!r} must use a registered boundary dataset"
        raise ValueError(message)
    return source.removeprefix(prefix)
