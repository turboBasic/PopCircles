import textwrap
import xml.etree.ElementTree as ET
from collections.abc import Mapping, Sequence
from typing import cast

from PIL import ExifTags, Image
from PIL.TiffImagePlugin import IFDRational

from population_circles.documents.result_document import Answer

_MINUTES_PER_DEGREE = 60


def _footer_text() -> str:
    return (
        "© Andrii Melnyk, 2026, https://github.com/turboBasic/PopCircles · "
        "© EuroGeographics for the administrative boundaries"
    )


def visible_credits(map_attributions: Sequence[tuple[str, str]]) -> str:
    additions = [f"{name} data: {attribution}" for name, attribution in map_attributions]
    return " · ".join((_footer_text(), *additions, "See full attribution in image metadata"))


def copyright_of(
    answer: Answer,
    attribution: str,
    coastline_attribution: str | None,
    boundary_attribution: str | None,
    map_attributions: Sequence[tuple[str, str]] = (),
) -> str:
    parts = ["© Andrii Melnyk, 2026, https://github.com/turboBasic/PopCircles"]
    if answer.country is not None:
        parts.append(f"Population data: {attribution}")
        parts.append(f"Administrative boundaries: {boundary_attribution}")
    elif answer.caption.country_breakdown is not None:
        parts.append(
            f"Administrative boundaries: {answer.caption.country_breakdown.provenance.attribution}"
        )
        parts.append(f"Population data: {attribution}")
    else:
        parts.append(f"Population data: {attribution}")
    if coastline_attribution:
        parts.append(f"Coastline data: {coastline_attribution}")
    parts.extend(f"{name} data: {credit}" for name, credit in map_attributions)
    return "\n".join(textwrap.fill(part, width=118) for part in parts)


def xmp_properties(
    answer: Answer,
    title: str,
    description: str,
    copyright_text: str,
    map_attributions: Sequence[tuple[str, str]] = (),
) -> dict[str, object]:
    circle = answer.circle
    properties: dict[str, object] = {
        "population": answer.caption.population,
        "populationShare": answer.caption.share,
        "radiusKm": circle.radius_km,
        "centerLatitude": circle.centre.lat,
        "centerLongitude": circle.centre.lon,
        "populationDataset": answer.provenance.dataset,
        "decimation": answer.provenance.decimation,
    }
    if answer.caption.territory_share is not None:
        properties["territoryShare"] = answer.caption.territory_share
    if answer.country is not None:
        properties["photoshop:Country"] = answer.country.name
        properties["Iptc4xmpCore:CountryCode"] = answer.country.id
        properties["countryName"] = answer.country.name
        properties["countryCode"] = answer.country.id
    settlement = answer.caption.settlement
    place: dict[str, object] = {
        "exif:GPSLatitude": _gps_coordinate(circle.centre.lat, "N", "S"),
        "exif:GPSLongitude": _gps_coordinate(circle.centre.lon, "E", "W"),
    }
    if answer.country is not None:
        place["Iptc4xmpExt:CountryName"] = answer.country.name
        place["Iptc4xmpExt:CountryCode"] = answer.country.id
    if settlement is not None:
        place["Iptc4xmpExt:City"] = settlement.name
        if answer.country is None:
            place["Iptc4xmpExt:CountryName"] = settlement.country
        properties["nearestSettlement"] = settlement.name
        properties["nearestSettlementCountry"] = settlement.country
        properties["distanceToSettlementKm"] = settlement.distance_km
    properties["Iptc4xmpExt:LocationShown"] = [place]
    properties["dc:creator"] = ["Andrii Melnyk"]
    properties["Iptc4xmpCore:CreatorContactInfo"] = {
        "Iptc4xmpCore:CiUrlWork": "https://github.com/turboBasic/PopCircles"
    }
    properties["dc:rights"] = "© Andrii Melnyk, 2026"
    properties["dc:title"] = title
    properties["dc:description"] = description
    properties["photoshop:Credit"] = visible_credits(map_attributions)
    properties["fullAttribution"] = copyright_text
    return {name: value for name, value in properties.items() if value is not None}


def _gps_coordinate(value: float, positive: str, negative: str) -> str:
    degrees = int(abs(value))
    minutes = round((abs(value) - degrees) * _MINUTES_PER_DEGREE, 5)
    if minutes == _MINUTES_PER_DEGREE:
        degrees += 1
        minutes = 0
    hemisphere = positive if value >= 0 else negative
    return f"{degrees},{minutes:g}{hemisphere}"


def _xmp_creator(description: ET.Element, value: object, namespaces: Mapping[str, str]) -> None:
    sequence = ET.SubElement(description, f"{{{namespaces['dc']}}}creator")
    items = ET.SubElement(sequence, f"{{{namespaces['rdf']}}}Seq")
    for creator in cast(list[str], value):
        ET.SubElement(items, f"{{{namespaces['rdf']}}}li").text = creator


def _xmp_alt_text(
    description: ET.Element, name: str, value: object, namespaces: Mapping[str, str]
) -> None:
    local_name = name.split(":", maxsplit=1)[1]
    prop = ET.SubElement(description, f"{{{namespaces['dc']}}}{local_name}")
    items = ET.SubElement(prop, f"{{{namespaces['rdf']}}}Alt")
    item = ET.SubElement(items, f"{{{namespaces['rdf']}}}li")
    item.set("{http://www.w3.org/XML/1998/namespace}lang", "x-default")
    item.text = cast(str, value)


def _xmp_creator_contact(
    description: ET.Element, value: object, namespaces: Mapping[str, str]
) -> None:
    prop = ET.SubElement(description, f"{{{namespaces['Iptc4xmpCore']}}}CreatorContactInfo")
    prop.set(f"{{{namespaces['rdf']}}}parseType", "Resource")
    for field, text in cast(dict[str, str], value).items():
        _, local_name = field.split(":", maxsplit=1)
        ET.SubElement(prop, f"{{{namespaces['Iptc4xmpCore']}}}{local_name}").text = text


def _xmp_location_shown(
    description: ET.Element, value: object, namespaces: Mapping[str, str]
) -> None:
    prop = ET.SubElement(description, f"{{{namespaces['Iptc4xmpExt']}}}LocationShown")
    bag = ET.SubElement(prop, f"{{{namespaces['rdf']}}}Bag")
    for location in cast(list[dict[str, object]], value):
        item = ET.SubElement(bag, f"{{{namespaces['rdf']}}}li")
        item.set(f"{{{namespaces['rdf']}}}parseType", "Resource")
        for field, text in location.items():
            prefix, local_name = field.split(":", maxsplit=1)
            ET.SubElement(item, f"{{{namespaces[prefix]}}}{local_name}").text = str(text)


def _xmp_scalar(
    description: ET.Element, name: str, value: object, namespaces: Mapping[str, str]
) -> None:
    prefix, separator, local_name = name.partition(":")
    if not separator:
        prefix, local_name = "pc", name
    description.set(f"{{{namespaces[prefix]}}}{local_name}", str(value))


def _xmp_packet(properties: Mapping[str, object]) -> str:
    namespaces = {
        "x": "adobe:ns:meta/",
        "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
        "dc": "http://purl.org/dc/elements/1.1/",
        "photoshop": "http://ns.adobe.com/photoshop/1.0/",
        "Iptc4xmpCore": "http://iptc.org/std/Iptc4xmpCore/1.0/xmlns/",
        "Iptc4xmpExt": "http://iptc.org/std/Iptc4xmpExt/2008-02-29/",
        "exif": "http://ns.adobe.com/exif/1.0/",
        "pc": "https://github.com/turboBasic/PopCircles/ns/1.0/",
    }
    for prefix, uri in namespaces.items():
        ET.register_namespace(prefix, uri)
    xmp = ET.Element(f"{{{namespaces['x']}}}xmpmeta")
    rdf = ET.SubElement(xmp, f"{{{namespaces['rdf']}}}RDF")
    description = ET.SubElement(rdf, f"{{{namespaces['rdf']}}}Description")
    description.set(f"{{{namespaces['rdf']}}}about", "")
    for name, value in properties.items():
        if name == "dc:creator":
            _xmp_creator(description, value, namespaces)
        elif name in {"dc:rights", "dc:title", "dc:description"}:
            _xmp_alt_text(description, name, value, namespaces)
        elif name == "Iptc4xmpCore:CreatorContactInfo":
            _xmp_creator_contact(description, value, namespaces)
        elif name == "Iptc4xmpExt:LocationShown":
            _xmp_location_shown(description, value, namespaces)
        else:
            _xmp_scalar(description, name, value, namespaces)
    return ET.tostring(xmp, encoding="unicode", xml_declaration=True)


def description_of(answer: Answer) -> str:
    return (
        f"Answered from population dataset {answer.provenance.dataset} "
        f"at decimation k={answer.provenance.decimation}"
    )


def metadata_json_fields(
    answer: Answer,
    properties: Mapping[str, object],
) -> dict[str, object]:
    title = cast(str, properties["dc:title"])
    description = cast(str, properties["dc:description"])
    copyright_text = cast(str, properties["fullAttribution"])
    return {
        "Title": title,
        "Description": description,
        "Copyright": copyright_text,
        "XML:com.adobe.xmp": dict(properties),
        "EXIF:GPSInfo": _exif_gps_values(answer.circle.centre.lat, answer.circle.centre.lon),
    }


def _exif_gps_values(latitude: float, longitude: float) -> dict[str, object]:
    def dms(value: float) -> list[list[int]]:
        total = round(abs(value) * 3_600_000)
        degrees, remainder = divmod(total, 3_600_000)
        minutes, seconds = divmod(remainder, 60_000)
        return [[degrees, 1], [minutes, 1], [seconds, 1000]]

    return {
        "GPSVersionID": [2, 3, 0, 0],
        "GPSLatitudeRef": "N" if latitude >= 0 else "S",
        "GPSLatitude": dms(latitude),
        "GPSLongitudeRef": "E" if longitude >= 0 else "W",
        "GPSLongitude": dms(longitude),
        "GPSMapDatum": "WGS-84",
    }


def exif_gps_packet(latitude: float, longitude: float) -> bytes:
    values = _exif_gps_values(latitude, longitude)
    gps = {
        ExifTags.GPS.GPSVersionID: bytes(cast(list[int], values["GPSVersionID"])),
        ExifTags.GPS.GPSLatitudeRef: cast(str, values["GPSLatitudeRef"]),
        ExifTags.GPS.GPSLatitude: tuple(
            IFDRational(numerator, denominator)
            for numerator, denominator in cast(list[list[int]], values["GPSLatitude"])
        ),
        ExifTags.GPS.GPSLongitudeRef: cast(str, values["GPSLongitudeRef"]),
        ExifTags.GPS.GPSLongitude: tuple(
            IFDRational(numerator, denominator)
            for numerator, denominator in cast(list[list[int]], values["GPSLongitude"])
        ),
        ExifTags.GPS.GPSMapDatum: "WGS-84",
    }
    exif = Image.Exif()
    exif[ExifTags.IFD.GPSInfo] = gps
    return exif.tobytes()


def png_metadata(
    title: str,
    copyright_text: str,
    description: str,
    properties: Mapping[str, object],
) -> dict[str, str]:
    return {
        "Title": title,
        "Description": description,
        "Copyright": copyright_text,
        "XML:com.adobe.xmp": _xmp_packet(
            {name: value for name, value in properties.items() if value is not None}
        ),
    }
