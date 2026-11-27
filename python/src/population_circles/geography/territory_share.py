from collections.abc import Mapping
from dataclasses import dataclass
from typing import cast

from shapely.geometry.base import BaseGeometry

from population_circles.data.dataset_registry import Registry, load
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
from population_circles.geography.country_boundaries import (
    load_country_boundaries,
    load_map_boundaries,
)


@dataclass(frozen=True)
class _Enrichment:
    document: Mapping[str, object]
    earth_radius: float
    registry: Registry
    boundaries: Mapping[str, BaseGeometry] | None


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
        selected_boundaries = (
            load_country_boundaries(answer, context.registry)
            if context.boundaries is None
            else context.boundaries
        )
        if selected_boundaries is None:
            message = f"country boundary for {country.id!r} is not registered"
            raise ValueError(message)
        geometries = [selected_boundaries[country.id]] if country.id in selected_boundaries else []
    else:
        answer = circle_of(context.document)
        selected_boundaries = (
            load_map_boundaries(answer, context.registry)
            if context.boundaries is None
            else context.boundaries
        )
        geometries = (
            [selected_boundaries[answer.country.id]]
            if answer.country is not None and answer.country.id in selected_boundaries
            else []
            if answer.country is not None
            else list(selected_boundaries.values())
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
    document: dict[str, object], boundaries: Mapping[str, BaseGeometry] | None = None
) -> dict[str, object]:
    kind = Envelope.model_validate(document).document
    registry = load()
    earth_radius = Envelope.model_validate(document).earth_model.radius_km
    result = cast(dict[str, object], document["result"])
    context = _Enrichment(document, earth_radius, registry, boundaries)

    def enrich(
        circle_data: dict[str, object], country_data: Mapping[str, object] | None = None
    ) -> None:
        _enrich_circle(circle_data, context, country_data)

    if kind == "country-sweep":
        for country_entry in cast(list[dict[str, object]], result["countries"]):
            identity = cast(Mapping[str, object], country_entry["country"])
            for record in cast(list[dict[str, object]], country_entry["records"]):
                enrich(cast(dict[str, object], record["circle"]), identity)
    elif kind in {"smallest", "country-smallest"}:
        enrich(cast(dict[str, object], result["circle"]))
    else:
        enrich(result)
    return document
