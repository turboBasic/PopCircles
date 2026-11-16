import re
import tomllib
from pathlib import Path
from typing import Annotated, Literal, Self

from pydantic import BaseModel, ConfigDict, Field, ValidationError, field_validator, model_validator

REPO_ROOT = Path(__file__).resolve().parents[3]
PRESENTATION_CONFIG = REPO_ROOT / "data" / "presentation.toml"
Projection = Literal["plate-carree", "orthographic"]
CountryId = Annotated[str, Field(pattern=r"^[A-Z]{2,3}$")]


class _Model(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid", strict=True)


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


class ComponentBounds(_Model):
    west: float = Field(ge=-180, le=180)
    south: float = Field(ge=-90, le=90)
    east: float = Field(ge=-180, le=180)
    north: float = Field(ge=-90, le=90)

    @model_validator(mode="after")
    def ordered_bounds(self) -> Self:
        if self.west >= self.east:
            message = "west must be less than east"
            raise ValueError(message)
        if self.south >= self.north:
            message = "south must be less than north"
            raise ValueError(message)
        return self


class CountryMap(_Model):
    projection: Projection
    centre_lat: float | None = Field(default=None, ge=-90, le=90)
    centre_lon: float | None = Field(default=None, ge=-180, le=180)
    component_bounds: ComponentBounds | None = None

    @model_validator(mode="after")
    def paired_centre(self) -> Self:
        if (self.centre_lat is None) != (self.centre_lon is None):
            message = "centre_lat and centre_lon must be set together"
            raise ValueError(message)
        return self


class PresentationConfig(_Model):
    map_text: MapText
    gallery: Gallery
    country_maps: dict[str, CountryMap | Projection]

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

    @property
    def default_projection(self) -> Projection:
        value = self.country_maps["default_projection"]
        if not isinstance(value, str):
            message = "country_maps.default_projection must be a projection"
            raise TypeError(message)
        return value


def parse(text: str, *, source: Path = PRESENTATION_CONFIG) -> PresentationConfig:
    try:
        return PresentationConfig.model_validate(tomllib.loads(text))
    except (ValidationError, tomllib.TOMLDecodeError) as error:
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
