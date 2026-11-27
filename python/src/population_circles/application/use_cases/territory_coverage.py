from collections.abc import Callable, Mapping
from dataclasses import dataclass
from typing import cast

from shapely.geometry.base import BaseGeometry

from population_circles.documents.result_document import (
    Answer,
    Caption,
    Circle,
    CountryIdentity,
    Envelope,
    Provenance,
    circle_of,
)
from population_circles.domain.geography import GeographicCoordinate, SphericalCircle
from population_circles.domain.territory_coverage import NoTerritoryAreaError, territory_share

BoundaryLoader = Callable[[Answer], Mapping[str, BaseGeometry] | None]


@dataclass(frozen=True)
class _Enrichment:
    document: Mapping[str, object]
    earth_radius: float
    boundaries: Mapping[str, BaseGeometry] | None
    load_country_boundaries: BoundaryLoader | None
    load_map_boundaries: BoundaryLoader | None


def _load_boundaries(
    context: _Enrichment, answer: Answer, *, country: bool
) -> Mapping[str, BaseGeometry] | None:
    if context.boundaries is not None:
        return context.boundaries
    loader = context.load_country_boundaries if country else context.load_map_boundaries
    if loader is None:
        kind = "country" if country else "map"
        message = f"{kind} boundaries were not supplied to territory coverage"
        raise ValueError(message)
    return loader(answer)


def _enrich_circle(
    circle_data: dict[str, object],
    context: _Enrichment,
    country_data: Mapping[str, object] | None = None,
) -> None:
    circle = Circle.model_validate({**circle_data, "earth_radius_km": context.earth_radius})
    if circle.radius_km == 0.0:
        circle_data.pop("territory_share", None)
        return
    if country_data is not None:
        country = CountryIdentity.model_validate(country_data)
        answer = Answer(
            circle=circle,
            caption=Caption(population=0.0, share=0.0),
            provenance=Provenance(dataset="country-result", table=None),
            country=country,
        )
        country_boundaries = _load_boundaries(context, answer, country=True)
        if country_boundaries is None:
            message = f"country boundary for {country.id!r} is not registered"
            raise ValueError(message)
        geometries = [country_boundaries[country.id]] if country.id in country_boundaries else []
    else:
        answer = circle_of(context.document)
        loaded_boundaries = _load_boundaries(context, answer, country=False)
        map_boundaries: Mapping[str, BaseGeometry] = (
            loaded_boundaries if loaded_boundaries is not None else {}
        )
        geometries = (
            [map_boundaries[answer.country.id]]
            if answer.country is not None and answer.country.id in map_boundaries
            else []
            if answer.country is not None
            else list(map_boundaries.values())
        )
    domain_circle = SphericalCircle(
        centre=GeographicCoordinate(lat=circle.centre.lat, lon=circle.centre.lon),
        radius_km=circle.radius_km,
        earth_radius_km=circle.earth_radius_km,
    )
    try:
        circle_data["territory_share"] = territory_share(domain_circle, geometries)
    except NoTerritoryAreaError:
        circle_data.pop("territory_share", None)


def enrich_document(
    document: dict[str, object],
    boundaries: Mapping[str, BaseGeometry] | None = None,
    *,
    load_country_boundaries: BoundaryLoader | None = None,
    load_map_boundaries: BoundaryLoader | None = None,
) -> dict[str, object]:
    envelope = Envelope.model_validate(document)
    result = cast(dict[str, object], document["result"])
    context = _Enrichment(
        document,
        envelope.earth_model.radius_km,
        boundaries,
        load_country_boundaries,
        load_map_boundaries,
    )

    def enrich(
        circle_data: dict[str, object], country_data: Mapping[str, object] | None = None
    ) -> None:
        _enrich_circle(circle_data, context, country_data)

    if envelope.document == "country-sweep":
        for country_entry in cast(list[dict[str, object]], result["countries"]):
            identity = cast(Mapping[str, object], country_entry["country"])
            for record in cast(list[dict[str, object]], country_entry["records"]):
                enrich(cast(dict[str, object], record["circle"]), identity)
    elif envelope.document in {"smallest", "country-smallest"}:
        enrich(cast(dict[str, object], result["circle"]))
    else:
        enrich(result)
    return document
