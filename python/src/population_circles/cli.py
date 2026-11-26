from pathlib import Path
from typing import Annotated

import typer

from population_circles import circle_map_workflow
from population_circles.gallery import build as gallery_build
from population_circles.maps import render_globe, render_map
from population_circles.maps.frame import PLATE_CARREE

app = typer.Typer(no_args_is_help=True)
render_app = typer.Typer(no_args_is_help=True)
gallery_app = typer.Typer(no_args_is_help=True)


@app.command("map")
def map_command(
    dataset: Annotated[str, typer.Option()],
    share: Annotated[
        str, typer.Option(help="Population share in whole percent; validated by Rust.")
    ],
    cache: Annotated[Path, typer.Option()],
    output: Annotated[Path, typer.Option()],
    document: Annotated[
        Path | None, typer.Option(help="Search JSON; defaults to the image path with .json suffix.")
    ] = None,
    ledger: Annotated[
        Path | None,
        typer.Option(help="Radii ledger; defaults to the document path with .radii.json suffix."),
    ] = None,
    decimate: Annotated[str, typer.Option()] = "1",
    spacing: Annotated[str | None, typer.Option()] = None,
    projection: Annotated[str, typer.Option()] = PLATE_CARREE,
    no_coastlines: Annotated[bool, typer.Option()] = False,
) -> None:
    arguments = [
        "--dataset",
        dataset,
        "--share",
        share,
        "--cache",
        str(cache),
        "--output",
        str(output),
    ]
    if document is not None:
        arguments.extend(("--document", str(document)))
    if ledger is not None:
        arguments.extend(("--ledger", str(ledger)))
    arguments.extend(("--decimate", decimate, "--projection", projection))
    if spacing is not None:
        arguments.extend(("--spacing", spacing))
    if no_coastlines:
        arguments.append("--no-coastlines")
    raise typer.Exit(circle_map_workflow.main(arguments))


@render_app.command("map")
def render_map_command(
    input_path: Annotated[Path, typer.Option("--input")],
    output: Annotated[Path, typer.Option()],
    projection: Annotated[str, typer.Option()] = PLATE_CARREE,
    no_coastlines: Annotated[bool, typer.Option()] = False,
) -> None:
    arguments = ["--input", str(input_path), "--output", str(output), "--projection", projection]
    if no_coastlines:
        arguments.append("--no-coastlines")
    raise typer.Exit(render_map.main(arguments))


@render_app.command("globe")
def render_globe_command(
    center: Annotated[str, typer.Option("--center", "--centre", metavar="LAT,LON")],
    output: Annotated[Path, typer.Option()],
    output_gif: Annotated[Path | None, typer.Option()] = None,
    svg_sprite: Annotated[bool, typer.Option()] = False,
    latitude_steps: Annotated[
        int | None, typer.Option(help="render this many frames from pole to pole")
    ] = None,
    detalization: Annotated[float, typer.Option()] = 1,
) -> None:
    arguments = ["--center", center, "--output", str(output), "--detalization", str(detalization)]
    if output_gif is not None:
        arguments.extend(("--output-gif", str(output_gif)))
    if svg_sprite:
        arguments.append("--svg-sprite")
    if latitude_steps is not None:
        arguments.extend(("--latitude-steps", str(latitude_steps)))
    raise typer.Exit(render_globe.main(arguments))


@gallery_app.command("build")
def gallery_build_command(
    input_path: Annotated[Path, typer.Option("--input")],
    output: Annotated[Path, typer.Option()],
    refresh_country_sweep: Annotated[bool, typer.Option("--refresh-country-sweep")] = False,
    refresh_country_coverage: Annotated[bool, typer.Option("--refresh-country-coverage")] = False,
) -> None:
    arguments = ["--input", str(input_path), "--output", str(output)]
    if refresh_country_sweep:
        arguments.append("--refresh-country-sweep")
    if refresh_country_coverage:
        arguments.append("--refresh-country-coverage")
    raise typer.Exit(gallery_build.main(arguments))


app.add_typer(render_app, name="render")
app.add_typer(gallery_app, name="gallery")


if __name__ == "__main__":
    app()
