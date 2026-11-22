import math
import re
import tomllib
from pathlib import Path
from typing import Annotated, Literal, Self

from pydantic import BaseModel, ConfigDict, Field, ValidationError, field_validator, model_validator

REPO_ROOT = Path(__file__).resolve().parents[3]
PRESENTATION_CONFIG = REPO_ROOT / "data" / "presentation.toml"
CountryProjection = Literal[
    "plate-carree",
    "orthographic",
    "albers-equal-area",
    "transverse-mercator",
]
DefaultProjection = Literal["plate-carree", "orthographic"]
ExtentUnits = Literal["metres", "degrees"]
CountryId = Annotated[str, Field(pattern=r"^[A-Z]{2,3}$")]
PROJECTION_PARAMETERS: dict[str, frozenset[str]] = {
    "plate-carree": frozenset(),
    "orthographic": frozenset({"lon_0", "lat_0"}),
    "albers-equal-area": frozenset({"lon_0", "lat_0", "lat_1", "lat_2"}),
    "transverse-mercator": frozenset({"lon_0", "lat_0", "k_0"}),
}
OPTIONAL_PROJECTION_PARAMETERS: frozenset[str] = frozenset({"x_0", "y_0"})
MIN_LATITUDE = -90
MAX_LATITUDE = 90
MIN_LONGITUDE = -180
MAX_LONGITUDE = 180
MIN_SCALE_FACTOR = 0.0


class _Model(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid", strict=True)


def _validate_projection_parameters(
    projection: CountryProjection, parameters: dict[str, float]
) -> None:
    required = PROJECTION_PARAMETERS[projection]
    actual = frozenset(parameters)
    allowed = required | (
        OPTIONAL_PROJECTION_PARAMETERS if projection != "plate-carree" else frozenset[str]()
    )
    missing = required - actual
    unknown = actual - allowed
    if missing or unknown:
        message = (
            f"{projection} projection_parameters missing {sorted(missing)} "
            f"or unsupported {sorted(unknown)}"
        )
        raise ValueError(message)
    if not all(math.isfinite(value) for value in parameters.values()):
        message = "projection parameters must be finite"
        raise ValueError(message)
    for name in ("lat_0", "lat_1", "lat_2"):
        value = parameters.get(name)
        if value is not None and not MIN_LATITUDE <= value <= MAX_LATITUDE:
            message = f"{name} must be between {MIN_LATITUDE} and {MAX_LATITUDE} degrees"
            raise ValueError(message)
    lon_0 = parameters.get("lon_0")
    if lon_0 is not None and not MIN_LONGITUDE <= lon_0 <= MAX_LONGITUDE:
        message = f"lon_0 must be between {MIN_LONGITUDE} and {MAX_LONGITUDE} degrees"
        raise ValueError(message)
    k_0 = parameters.get("k_0")
    if k_0 is not None and k_0 <= MIN_SCALE_FACTOR:
        message = "k_0 must be positive"
        raise ValueError(message)
    if projection == "albers-equal-area" and parameters["lat_1"] == -parameters["lat_2"]:
        message = "lat_1 and lat_2 cannot be opposite parallels"
        raise ValueError(message)


class MapText(_Model):
    render_title: bool
    render_description: bool


class Gallery(_Model):
    excluded_country_ids: list[CountryId]

    @field_validator("excluded_country_ids")
    @classmethod
    def unique_country_ids(cls, value: list[str]) -> list[str]:
        if len(value) != len(set(value)):
            message = "country IDs must be unique"
            raise ValueError(message)
        return value


class MapSources(_Model):
    terrain_default: Annotated[str, Field(min_length=1)]
    terrain_overrides: dict[CountryId, Annotated[str, Field(min_length=1)]] = Field(
        default_factory=dict,
    )
    terrain_overlays: dict[
        CountryId,
        Annotated[list[Annotated[str, Field(min_length=1)]], Field(min_length=1)],
    ] = Field(default_factory=dict)
    lakes: Annotated[str, Field(min_length=1)]
    rivers: Annotated[str, Field(min_length=1)]
    roads: Annotated[str, Field(min_length=1)]
    places: Annotated[str, Field(min_length=1)]


class MapExtent(_Model):
    units: ExtentUnits
    x_min: float
    y_min: float
    x_max: float
    y_max: float

    @model_validator(mode="after")
    def finite_ordered_bounds(self) -> Self:
        if not all(
            math.isfinite(value) for value in (self.x_min, self.y_min, self.x_max, self.y_max)
        ):
            message = "extent coordinates must be finite"
            raise ValueError(message)
        if self.x_min >= self.x_max or self.y_min >= self.y_max:
            message = "extent minimums must be less than maximums"
            raise ValueError(message)
        return self


class CountryMap(_Model):
    projection: CountryProjection
    projection_parameters: dict[str, float] = Field(default_factory=dict)
    extent: MapExtent | None = None

    @model_validator(mode="after")
    def valid_projection_parameters(self) -> Self:
        _validate_projection_parameters(self.projection, self.projection_parameters)
        if self.extent is not None:
            expected_units = "degrees" if self.projection == "plate-carree" else "metres"
            if self.extent.units != expected_units:
                message = f"{self.projection} extents must use {expected_units}"
                raise ValueError(message)
        return self


class PresentationConfig(_Model):
    map_text: MapText
    map_titles: dict[str, Annotated[str, Field(min_length=1)]] = Field(default_factory=dict)
    gallery: Gallery
    map_sources: MapSources
    country_maps: dict[str, CountryMap | DefaultProjection]

    @model_validator(mode="after")
    def valid_country_map_keys(self) -> Self:
        if "default_projection" not in self.country_maps:
            message = "country_maps.default_projection is required"
            raise ValueError(message)
        for country_id in self.country_maps.keys() - {"default_projection"}:
            if re.fullmatch(r"[A-Z]{2,3}", country_id) is None:
                message = f"country_maps.{country_id} must be a canonical country ID"
                raise ValueError(message)
            if isinstance(self.country_maps[country_id], str):
                message = f"country_maps.{country_id} must be a country map table"
                raise TypeError(message)
        return self

    @field_validator("map_titles")
    @classmethod
    def valid_map_title_keys(cls, value: dict[str, str]) -> dict[str, str]:
        if any(re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", key) is None for key in value):
            message = "map_titles keys must be lowercase map filename stems"
            raise ValueError(message)
        return value

    @property
    def default_projection(self) -> DefaultProjection:
        value = self.country_maps["default_projection"]
        if not isinstance(value, str) or value not in ("orthographic", "plate-carree"):
            message = "country_maps.default_projection must be a projection"
            raise TypeError(message)
        return value


def parse(text: str, *, source: Path = PRESENTATION_CONFIG) -> PresentationConfig:
    try:
        return PresentationConfig.model_validate(tomllib.loads(text))
    except (TypeError, ValidationError, tomllib.TOMLDecodeError) as error:
        if isinstance(error, ValidationError):
            details = "; ".join(
                f"{'.'.join(str(part) for part in issue['loc'])}: {issue['msg']}"
                for issue in error.errors()
            )
        else:
            details = str(error)
        message = f"{source}: {details}"
        raise ValueError(message) from error


def load() -> PresentationConfig:
    return parse(PRESENTATION_CONFIG.read_text(encoding="utf-8"))


def map_text_rendering() -> tuple[bool, bool]:
    settings = load().map_text
    return settings.render_title, settings.render_description
