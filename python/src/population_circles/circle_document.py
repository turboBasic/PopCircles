from abc import ABC, abstractmethod
from collections.abc import Mapping
from typing import Annotated, Literal, Self

from pydantic import BaseModel, ConfigDict, Field, model_validator

# The highest `report::SCHEMA_VERSION` this reader was written against. A document above it may have
# renamed or removed a field, which is the only thing that constant rises for, so it is refused
# rather than read optimistically.
SCHEMA_VERSION = 1

# The published `Document::KIND` values. Held as a Literal so an unrecognised kind is a schema
# refusal naming `document`, not a KeyError three frames later.
DocumentKind = Literal[
    "distance",
    "grid",
    "table-build",
    "table-query",
    "circle",
    "most-populous",
    "smallest-circle",
    "smallest",
    "sweep",
    "country-list",
    "country-smallest",
    "country-sweep",
]

# frozen because nothing downstream of the boundary may edit what the document said, and
# `extra="ignore"` because that is the format's own instruction to consumers (`report.rs` "Growth"):
# a field added additively must not turn into a refusal here.
_CONFIG = ConfigDict(frozen=True, extra="ignore")


class UnsupportedDocumentError(ValueError):
    def __init__(self, kind: str) -> None:
        super().__init__(f"document kind {kind!r} carries no circle to draw")
        self.kind = kind


class MissingDatasetError(ValueError):
    def __init__(self) -> None:
        super().__init__(
            "the document names no dataset under provenance, so there is no attribution for a "
            "figure to carry; rebuild the table with `table build --dataset <name>`",
        )


class MissingTableError(ValueError):
    def __init__(self) -> None:
        super().__init__(
            "the document names no table under provenance, so a figure could not say what answered "
            "it; the digest and the decimation together are what identify one",
        )


class Coordinate(BaseModel):
    model_config = _CONFIG

    lat: float
    lon: float


class EarthModel(BaseModel):
    model_config = _CONFIG

    model: Literal["sphere"]
    radius_km: float


class Circle(BaseModel):
    model_config = _CONFIG

    centre: Coordinate
    radius_km: float
    earth_radius_km: float


class Provenance(BaseModel):
    model_config = _CONFIG

    dataset: str
    table: str | None
    decimation: int | None = None


class CountryIdentity(BaseModel):
    model_config = _CONFIG

    id: Annotated[str, Field(min_length=1, pattern=r"\S")]
    name: Annotated[str, Field(min_length=1, pattern=r"\S")]
    boundary_dataset: Annotated[str, Field(min_length=1)]
    boundary_revision: Annotated[str, Field(min_length=1)] | None = None
    boundary_digest: Annotated[str, Field(pattern=r"^[0-9a-fA-F]{64}$")]


class CountrySource(BaseModel):
    model_config = _CONFIG

    name: Annotated[str, Field(min_length=1)]
    sha256: Annotated[str, Field(pattern=r"^[0-9a-fA-F]{64}$")] | None = None
    attribution: Annotated[str, Field(min_length=1)] | None = None


class CountrySelection(BaseModel):
    model_config = _CONFIG

    combination_id: Annotated[str, Field(min_length=1)]
    resolution: Annotated[int, Field(gt=0)]
    population_source: CountrySource
    boundary_source: CountrySource


class Settlement(BaseModel):
    model_config = _CONFIG

    name: Annotated[str, Field(min_length=1)]
    country: Annotated[str, Field(min_length=1)]
    distance_km: Annotated[float, Field(ge=0, allow_inf_nan=False)]
    short_form: bool


class CountryBreakdownEntry(BaseModel):
    model_config = _CONFIG

    id: Annotated[str, Field(min_length=1, pattern=r"\S")]
    name: Annotated[str, Field(min_length=1, pattern=r"\S")]
    population_inside: Annotated[float, Field(ge=0, allow_inf_nan=False)]
    country_total_population: Annotated[float, Field(ge=0, allow_inf_nan=False)]
    fraction_of_country_population: Annotated[float, Field(ge=0, le=1, allow_inf_nan=False)] | None
    fraction_of_circle_population: Annotated[float, Field(ge=0, le=1, allow_inf_nan=False)] | None


class _CountryOrigin(BaseModel):
    model_config = _CONFIG

    lat: Annotated[float, Field(allow_inf_nan=False)]
    lon: Annotated[float, Field(allow_inf_nan=False)]


class _CountryGrid(BaseModel):
    model_config = _CONFIG

    width: Annotated[int, Field(gt=0)]
    height: Annotated[int, Field(gt=0)]
    origin: _CountryOrigin
    lon_step: Annotated[float, Field(gt=0, allow_inf_nan=False)]
    lat_step: Annotated[float, Field(lt=0, allow_inf_nan=False)]
    middle_row_cell_area_km2: Annotated[float, Field(gt=0, allow_inf_nan=False)]
    spans_full_turn: bool


class CountryBreakdownProvenance(BaseModel):
    model_config = _CONFIG

    boundary_dataset: Annotated[str, Field(min_length=1)]
    boundary_revision: Annotated[str, Field(min_length=1)]
    boundary_digest: Annotated[str, Field(pattern=r"^[0-9a-fA-F]{64}$")]
    attribution: Annotated[str, Field(min_length=1)]
    effective_grid: _CountryGrid
    decimation: Annotated[int, Field(gt=0)]
    accounting_route: Annotated[str, Field(min_length=1)]
    membership_rule_version: Annotated[int, Field(gt=0)]
    ownership_rule_version: Annotated[int, Field(gt=0)]
    aggregation_order_version: Annotated[int, Field(gt=0)]


class CountryReconciliation(BaseModel):
    model_config = _CONFIG

    unassigned_population: Annotated[float, Field(ge=0, allow_inf_nan=False)]
    bound_persons: Annotated[float, Field(ge=0, allow_inf_nan=False)]


class CountryBreakdown(BaseModel):
    model_config = _CONFIG

    entries: list[CountryBreakdownEntry]
    reconciliation: CountryReconciliation
    provenance: CountryBreakdownProvenance

    @model_validator(mode="after")
    def _entry_ids_are_unique(self) -> Self:
        ids = [entry.id for entry in self.entries]
        if len(ids) != len(set(ids)):
            message = "country breakdown identifiers must be unique"
            raise ValueError(message)
        return self


class Caption(BaseModel):
    model_config = _CONFIG

    population: float
    share: float
    settlement: Settlement | None = None
    country_breakdown: CountryBreakdown | None = None


class Answer(BaseModel):
    model_config = _CONFIG

    circle: Circle
    caption: Caption
    provenance: Provenance
    country: CountryIdentity | None = None
    selection: CountrySelection | None = None


class _DocumentProvenance(BaseModel):
    model_config = _CONFIG

    # Optional in the format, per `report.rs` "Growth", and required by this reader: a figure whose
    # credit cannot be read off its own document would be credited from whatever the renderer was
    # written against, so a document that cannot say is refused rather than drawn.
    dataset: str | None = None
    # Both optional in the format for `dataset`'s reason, and both required by this reader together:
    # the caption a figure owes names the table, and neither half names one alone.
    digest: str | None = None
    decimation: int | None = None


class Envelope(BaseModel):
    model_config = _CONFIG

    schema_version: Annotated[int, Field(le=SCHEMA_VERSION)]
    document: DocumentKind
    earth_model: EarthModel
    # Absent from a document whose command read no cached table, which is every kind this reader
    # refuses anyway.
    provenance: _DocumentProvenance | None = None


class _Payload(BaseModel, ABC):
    model_config = _CONFIG

    @abstractmethod
    def as_answer(self, earth_radius_km: float, provenance: Provenance) -> Answer: ...


class _Measured(_Payload):
    centre: Coordinate
    radius_km: float
    population: float
    share_of_total: float
    nearest_settlement: Settlement | None = None
    country_breakdown: CountryBreakdown | None = None

    def as_answer(self, earth_radius_km: float, provenance: Provenance) -> Answer:
        return Answer(
            circle=Circle(
                centre=self.centre,
                radius_km=self.radius_km,
                earth_radius_km=earth_radius_km,
            ),
            caption=Caption(
                population=self.population,
                share=self.share_of_total,
                settlement=self.nearest_settlement,
                country_breakdown=self.country_breakdown,
            ),
            provenance=provenance,
        )


class _Smallest(BaseModel):
    model_config = _CONFIG

    centre: Coordinate
    radius_km: float
    population: float
    share_achieved: float
    nearest_settlement: Settlement | None = None
    country_breakdown: CountryBreakdown | None = None


class _Searched(_Payload):
    circle: _Smallest

    def as_answer(self, earth_radius_km: float, provenance: Provenance) -> Answer:
        return Answer(
            circle=Circle(
                centre=self.circle.centre,
                radius_km=self.circle.radius_km,
                earth_radius_km=earth_radius_km,
            ),
            caption=Caption(
                population=self.circle.population,
                share=self.circle.share_achieved,
                settlement=self.circle.nearest_settlement,
                country_breakdown=self.circle.country_breakdown,
            ),
            provenance=provenance,
        )


class _CountrySearched(_Payload):
    country: CountryIdentity
    population_dataset: Annotated[str, Field(min_length=1)]
    decimation: Annotated[int, Field(gt=0)]
    selection: CountrySelection | None = None
    circle: _Smallest

    def as_answer(self, earth_radius_km: float, provenance: Provenance) -> Answer:
        answer = _Searched(circle=self.circle).as_answer(earth_radius_km, provenance)
        return Answer(
            circle=answer.circle,
            caption=answer.caption,
            provenance=answer.provenance,
            country=self.country,
            selection=self.selection,
        )


# The two payload shapes a circle arrives in, and which kinds use which. `smallest` nests its circle
# under a ledger that belongs to the run rather than to any one circle, which is why it is not the
# same shape as the other two rather than a superset of them.
_PAYLOADS: Mapping[str, type[_Payload]] = {
    "circle": _Measured,
    "most-populous": _Measured,
    "smallest": _Searched,
    "country-smallest": _CountrySearched,
}


class _SweepRecord(_Smallest):
    model_config = _CONFIG


class SweepRecords(BaseModel):
    model_config = _CONFIG

    records: list[_SweepRecord]


CIRCLE_KINDS = frozenset({"circle", "most-populous", "smallest"})


def circle_of(document: Mapping[str, object]) -> Answer:
    envelope = Envelope.model_validate(document)
    payload = _PAYLOADS.get(envelope.document)
    if payload is None:
        raise UnsupportedDocumentError(envelope.document)
    if payload is _CountrySearched:
        country_result = payload.model_validate(document.get("result"))
        return country_result.as_answer(
            envelope.earth_model.radius_km,
            Provenance(
                dataset=country_result.population_dataset,
                table=None,
                decimation=country_result.decimation,
            ),
        )
    provenance = envelope.provenance
    dataset = provenance.dataset if provenance else None
    if dataset is None:
        raise MissingDatasetError
    if provenance is None or provenance.digest is None or provenance.decimation is None:
        raise MissingTableError
    return payload.model_validate(document.get("result")).as_answer(
        envelope.earth_model.radius_km,
        Provenance(
            dataset=dataset,
            table=f"{provenance.digest} at decimation {provenance.decimation}",
        ),
    )
