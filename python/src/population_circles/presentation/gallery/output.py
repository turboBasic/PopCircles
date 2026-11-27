import shutil
import subprocess
from pathlib import Path

from PIL import Image, ImageChops

from population_circles.application.use_cases.gallery import GalleryFigure
from population_circles.infrastructure.geodata.dataset_registry import REPO_ROOT
from population_circles.presentation.maps.render_map import OCEAN

HEADER_OCEAN_MATCH_TOLERANCE = 8
HEADER_CROP_ASPECT_RATIO = 2
HEADER_CROP_ASPECT_TOLERANCE = 2


def build_frontend(output: Path) -> None:
    assets = output / "assets"
    if assets.is_symlink():
        assets.unlink()
    elif assets.is_dir():
        shutil.rmtree(assets)
    npm = shutil.which("npm")
    if npm is None:
        message = "npm is required; run this gallery build through mise"
        raise FileNotFoundError(message)
    subprocess.run(  # noqa: S603 — argv only; npm runs the pinned repository frontend build
        [
            npm,
            "--prefix",
            str(REPO_ROOT / "frontend"),
            "run",
            "build",
            "--",
            "--outDir",
            str(output.resolve()),
        ],
        check=True,
    )


def crop_header_background(source: Path, target: Path) -> None:
    with Image.open(source) as original:
        image = original.convert("RGB")
        ocean = Image.new("RGB", image.size, OCEAN)
        lookup = [255 if value < HEADER_OCEAN_MATCH_TOLERANCE else 0 for value in range(256)]
        mask = ImageChops.difference(image, ocean).convert("L").point(lookup)
        bounds = mask.getbbox()
        if bounds is None:
            # shortcut: fallback assumes render's 11x6 world axes; update when those bounds change.
            left, top, right = (
                round(image.width * 0.113),
                round(image.height * 0.131),
                round(image.width * 0.887),
            )
            bottom = round(image.height * 0.839)
        else:
            left, top, right, _ = bounds
            # The 2:1 Plate Carree frame extends beyond the ocean pixels at its land-filled edge.
            bottom = top + round((right - left) / HEADER_CROP_ASPECT_RATIO) - 1
        if bottom <= top or bottom > image.height:
            raise ValueError
        image.crop((left, top, right, bottom)).save(target)


def write_header_background(drawn: list[GalleryFigure], output: Path) -> None:
    world_map = next((one.figure for one in drawn if one.country_name is None), None)
    if world_map is not None:
        crop_header_background(world_map, output / "header-background.png")
