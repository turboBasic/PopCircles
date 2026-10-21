import argparse
import json
from pathlib import Path
from urllib.parse import urlencode

from population_circles.circle_document import Answer, circle_of


def coordinate_text(answer: Answer) -> str:
    centre = answer.circle.centre
    latitude = f"{abs(centre.lat):.2f} {'N' if centre.lat >= 0 else 'S'}"
    longitude = f"{abs(centre.lon):.2f} {'E' if centre.lon >= 0 else 'W'}"
    return f"{latitude}, {longitude}"


def settlement_text(answer: Answer) -> str | None:
    settlement = answer.caption.settlement
    if settlement is None:
        return None
    name = f"{settlement.name}, {settlement.country}"
    if settlement.short_form:
        return name
    distance = int(settlement.distance_km + 0.5)
    return f"{distance} km from {name}"


def markdown_centre(answer: Answer) -> str:
    centre = answer.circle.centre
    coordinates = coordinate_text(answer)
    query = urlencode({"mlat": centre.lat, "mlon": centre.lon})
    linked = f"[{coordinates}](https://www.openstreetmap.org/?{query})"
    place = settlement_text(answer)
    if place is None:
        return linked
    markdown_punctuation = r"\\`*_{}[]()#+-.!|>"
    escaped = "".join(
        f"\\{character}" if character in markdown_punctuation else character for character in place
    )
    return f"{linked} ({escaped})"


def plain_centre(answer: Answer) -> str:
    place = settlement_text(answer)
    centre = answer.circle.centre
    coordinates = f"{centre.lat:.4f}, {centre.lon:.4f}"
    return coordinates if place is None else f"{coordinates}, {place}"


def main() -> None:
    parser = argparse.ArgumentParser(description="Format a circle document's centre as Markdown")
    parser.add_argument("--input", required=True, type=Path)
    arguments = parser.parse_args()
    document = json.loads(arguments.input.read_text(encoding="utf-8"))
    print(markdown_centre(circle_of(document)))


if __name__ == "__main__":
    main()
