import math
import re
import tempfile
import xml.etree.ElementTree as ET
from collections.abc import Sequence
from dataclasses import dataclass
from io import BytesIO
from pathlib import Path
from xml.sax.saxutils import escape

import resvg_py
from PIL import Image
from shapely.geometry import MultiPolygon
from shapely.geometry.base import BaseGeometry
from shapely.geometry.polygon import Polygon

from population_circles.data.data_config import country_boundary_dataset_key
from population_circles.data.dataset_registry import (
    REPO_ROOT,
    BoundaryVector,
    load,
)
from population_circles.documents.result_document import Coordinate
from population_circles.geography.circle_geometry import (
    HALF_TURN_DEG,
    POLE_LAT,
    cap,
    hemisphere_km,
    polygons,
)
from population_circles.maps.frame import LIMB_SHAVE, ORTHOGRAPHIC, frame, project
from population_circles.maps.layers import UnregisteredDatasetError, basemap

VIEW_SIZE = 512
VIEW_CENTRE = VIEW_SIZE / 2
GLOBE_RADIUS = 240
# The rendered projection is normalized to the globe, so its sphere radius has no visual effect.
PROJECTION_RADIUS_KM = 1.0
OUTPUT_SIZE = 200
MIN_GIF_FRAMES = 2
PIXEL_METRES = PROJECTION_RADIUS_KM * 1000 / GLOBE_RADIUS * VIEW_SIZE / OUTPUT_SIZE
MIN_TOPOLOGY_AREA = PIXEL_METRES**2
SIMPLIFY_METRES = PIXEL_METRES
SVG_NAMESPACE = "http://www.w3.org/2000/svg"


def simplify_projected_land(land: BaseGeometry, detalization: float = 1) -> BaseGeometry:
    visible = MultiPolygon(
        [
            Polygon(
                polygon.exterior,
                [ring for ring in polygon.interiors if Polygon(ring).area >= MIN_TOPOLOGY_AREA],
            )
            for polygon in polygons(land)
            if polygon.area >= MIN_TOPOLOGY_AREA
        ],
    )
    return visible.simplify(SIMPLIFY_METRES / detalization, preserve_topology=True)


def parse_center(value: str) -> Coordinate:
    try:
        latitude, longitude = (float(part) for part in value.split(","))
    except ValueError as error:
        message = "centre must be LATITUDE,LONGITUDE"
        raise ValueError(message) from error
    if not math.isfinite(latitude) or not -POLE_LAT <= latitude <= POLE_LAT:
        message = "center latitude must be between -90 and 90"
        raise ValueError(message)
    if abs(latitude) == POLE_LAT:
        message = "center latitude cannot be -90 or 90"
        raise ValueError(message)
    if not math.isfinite(longitude) or not -HALF_TURN_DEG <= longitude <= HALF_TURN_DEG:
        message = "center longitude must be between -180 and 180"
        raise ValueError(message)
    return Coordinate(lat=latitude, lon=longitude)


def svg_land(
    centre: Coordinate,
    rotation_degrees: float = 0,
    detalization: float = 1,
) -> str:
    registry = load()
    dataset_key = country_boundary_dataset_key()
    dataset = registry.datasets.get(dataset_key)
    if not isinstance(dataset, BoundaryVector):
        raise UnregisteredDatasetError(dataset_key)
    land = basemap(dataset.file(REPO_ROOT))
    limb = hemisphere_km(PROJECTION_RADIUS_KM) * (1.0 - LIMB_SHAVE)
    view = frame(ORTHOGRAPHIC, cap(centre, limb, PROJECTION_RADIUS_KM))
    land = simplify_projected_land(project(view, land), detalization)
    radius_metres = PROJECTION_RADIUS_KM * 1000

    def coordinate(x: float, y: float) -> tuple[float, float]:
        return (
            VIEW_CENTRE + x / radius_metres * GLOBE_RADIUS,
            VIEW_CENTRE - y / radius_metres * GLOBE_RADIUS,
        )

    def project_polygon(polygon: Polygon) -> str:
        rings = [polygon.exterior, *polygon.interiors]
        return " ".join(
            "M "
            + " L ".join(
                f"{projected_x:.1f} {projected_y:.1f}"
                for projected_x, projected_y in (coordinate(x, y) for x, y in ring.coords)
            )
            + " Z"
            for ring in rings
        )

    paths = "\n".join(f'    <path d="{project_polygon(polygon)}"/>' for polygon in polygons(land))
    rotation = (
        f' transform="rotate({rotation_degrees:g} {VIEW_CENTRE:g} {VIEW_CENTRE:g})"'
        if rotation_degrees % 360
        else ""
    )
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {VIEW_SIZE} {VIEW_SIZE}">\n'
        f"  <metadata>{escape(dataset.attribution)}</metadata>\n"
        f'  <circle cx="{VIEW_CENTRE:g}" cy="{VIEW_CENTRE:g}" '
        f'r="{GLOBE_RADIUS}" fill="#016DB8"/>\n'
        f'  <g{rotation} fill="#4CDD9F" fill-rule="evenodd">\n'
        f"{paths}\n"
        "  </g>\n"
        "</svg>\n"
    )


def render_globe(
    centre: Coordinate,
    output: Path,
    detalization: float = 1,
    rotation_degrees: float = 0,
) -> None:
    if not math.isfinite(detalization) or detalization <= 0:
        message = "detalization must be finite and greater than zero"
        raise ValueError(message)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(svg_land(centre, rotation_degrees, detalization), encoding="utf-8")


def parse_svg(path: Path) -> ET.Element:
    source = path.read_text(encoding="utf-8")
    if re.search(r"<!\s*(?:DOCTYPE|ENTITY)\b", source, re.IGNORECASE):
        message = "SVG document types and entities are not supported"
        raise ValueError(message)
    return ET.fromstring(source)


def render_gif(
    output: Path,
    sequence_svg: Path | None = None,
    *,
    svg_sprite: Path | None = None,
) -> None:
    if sequence_svg is not None and svg_sprite is None:
        match = re.fullmatch(r"(.+)-(\d+)\.svg", sequence_svg.name)
        if match is None:
            message = "SVG output must be numbered, for example earth-globe-24.svg"
            raise ValueError(message)
        prefix = match.group(1)
        frames = sorted(
            (
                path
                for path in sequence_svg.parent.glob(f"{prefix}-*.svg")
                if re.fullmatch(rf"{re.escape(prefix)}-\d+\.svg", path.name)
            ),
            key=lambda path: int(path.stem.removeprefix(f"{prefix}-")),
        )
        if len(frames) < MIN_GIF_FRAMES:
            message = f"Need at least two numbered SVG frames matching {prefix}-<step>.svg"
            raise ValueError(message)
        sources: Sequence[Path | ET.Element] = frames
    elif svg_sprite is not None and sequence_svg is None:
        sprite = parse_svg(svg_sprite)
        symbols = sprite.findall(f"{{{SVG_NAMESPACE}}}symbol")
        if len(symbols) < MIN_GIF_FRAMES:
            message = "Need at least two SVG symbols in the sprite"
            raise ValueError(message)
        sources = symbols
    else:
        message = "Provide exactly one of sequence_svg or svg_sprite"
        raise ValueError(message)

    output.parent.mkdir(parents=True, exist_ok=True)
    images: list[Image.Image] = []
    for source in sources:
        if isinstance(source, Path):
            png = resvg_py.svg_to_bytes(svg_path=str(source), width=200, height=200)
        else:
            root = ET.Element(
                f"{{{SVG_NAMESPACE}}}svg",
                {"viewBox": source.attrib["viewBox"]},
            )
            root.extend(source)
            svg = ET.tostring(root, encoding="unicode")
            png = resvg_py.svg_to_bytes(svg_string=svg, width=200, height=200)
        with Image.open(BytesIO(png)) as image:
            images.append(image.convert("RGB"))
    images[0].save(
        output,
        save_all=True,
        append_images=images[1:],
        duration=120,
        loop=0,
    )


def render_svg_sprite(sequence_svg: Path, output: Path) -> None:
    match = re.fullmatch(r"(.+)-(\d+)\.svg", sequence_svg.name)
    if match is None:
        message = "SVG output must be numbered, for example earth-globe-24.svg"
        raise ValueError(message)
    prefix = match.group(1)
    frames = sorted(
        (
            path
            for path in sequence_svg.parent.glob(f"{prefix}-*.svg")
            if re.fullmatch(rf"{re.escape(prefix)}-\d+\.svg", path.name)
        ),
        key=lambda path: int(path.stem.removeprefix(f"{prefix}-")),
    )
    root = ET.Element(f"{{{SVG_NAMESPACE}}}svg")
    attribution = None
    for index, path in enumerate(frames, start=1):
        frame = parse_svg(path)
        if attribution is None:
            attribution = frame.find(f"{{{SVG_NAMESPACE}}}metadata")
            if attribution is not None:
                root.append(ET.Element(f"{{{SVG_NAMESPACE}}}metadata"))
                root[-1].text = attribution.text
        symbol = ET.SubElement(
            root,
            f"{{{SVG_NAMESPACE}}}symbol",
            {"id": f"frame-{index:02}", "viewBox": frame.attrib["viewBox"]},
        )
        symbol.extend(child for child in frame if child.tag != f"{{{SVG_NAMESPACE}}}metadata")
    ET.register_namespace("", SVG_NAMESPACE)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(ET.tostring(root, encoding="unicode") + "\n", encoding="utf-8")


def render_latitude_sequence(
    centre: Coordinate,
    output: Path,
    steps: int,
    detalization: float = 1,
) -> Path:
    prefix = re.sub(r"-\d+$", "", output.stem)
    sequence_svg = output.with_name(f"{prefix}-{steps:02}.svg")
    for step in range(steps):
        latitude = (centre.lat + step * 2 * HALF_TURN_DEG / steps) % (2 * HALF_TURN_DEG)
        if math.isclose(latitude, HALF_TURN_DEG / 2, abs_tol=1e-9):
            latitude -= 1e-6
        elif math.isclose(latitude, 3 * HALF_TURN_DEG / 2, abs_tol=1e-9):
            latitude += 1e-6
        if latitude <= HALF_TURN_DEG / 2:
            centre_latitude, longitude_offset, rotation = latitude, 0, 0
        elif latitude <= 3 * HALF_TURN_DEG / 2:
            centre_latitude, longitude_offset, rotation = (
                HALF_TURN_DEG - latitude,
                HALF_TURN_DEG,
                HALF_TURN_DEG,
            )
        else:
            centre_latitude, longitude_offset, rotation = latitude - 2 * HALF_TURN_DEG, 0, 0
        longitude = (centre.lon + longitude_offset + HALF_TURN_DEG) % (
            2 * HALF_TURN_DEG
        ) - HALF_TURN_DEG
        path = output.with_name(f"{prefix}-{step + 1:02}.svg")
        render_globe(
            Coordinate(lat=centre_latitude, lon=longitude),
            path,
            detalization,
            rotation,
        )
    return sequence_svg


@dataclass(frozen=True)
class RenderOptions:
    centre: Coordinate
    output: Path
    output_gif: Path | None = None
    svg_sprite: bool = False
    latitude_steps: int | None = None
    detalization: float = 1


class GifRenderError(Exception):
    pass


def render_command(
    options: RenderOptions,
) -> Path | None:
    centre = options.centre
    output = options.output
    output_gif = options.output_gif
    svg_sprite = options.svg_sprite
    latitude_steps = options.latitude_steps
    detalization = options.detalization
    sequence_svg: Path | None = output
    if latitude_steps is None:
        render_globe(centre, output, detalization)
    elif svg_sprite:
        with tempfile.TemporaryDirectory() as directory:
            sequence_svg = render_latitude_sequence(
                centre,
                Path(directory) / output.name,
                latitude_steps,
                detalization,
            )
            render_svg_sprite(sequence_svg, output)
        sequence_svg = None
    else:
        sequence_svg = render_latitude_sequence(centre, output, latitude_steps, detalization)
    if output_gif:
        try:
            if svg_sprite:
                render_gif(output_gif, svg_sprite=output)
            else:
                render_gif(output_gif, sequence_svg)
        except (OSError, ValueError) as error:
            raise GifRenderError(str(error)) from error
    return output if svg_sprite else sequence_svg
