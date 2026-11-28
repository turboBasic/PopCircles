import math
from pathlib import Path
from typing import Annotated

import typer

from population_circles import composition
from population_circles.application.errors import GalleryPathError, PopulationApplicationError
from population_circles.application.use_cases import build_gallery, circle_result, render_map
from population_circles.application.use_cases.build_gallery import GalleryBuildRequest
from population_circles.application.use_cases.circle_result import CircleResultRequest
from population_circles.application.use_cases.render_map import RenderMapFileRequest
from population_circles.infrastructure import popcircles_cli
from population_circles.presentation.maps import animated_globe
from population_circles.presentation.maps.frame import PLATE_CARREE, PROJECTIONS

app = typer.Typer(no_args_is_help=True)


@app.command("circle", help="Search for a population circle and save its result document.")
def circle_command(
    dataset: Annotated[str, typer.Option()],
    share: Annotated[
        str, typer.Option(help="Population share in whole percent; validated by Rust.")
    ],
    cache: Annotated[Path, typer.Option()],
    output: Annotated[Path, typer.Option(help="Result JSON destination.")],
    ledger: Annotated[
        Path | None,
        typer.Option(help="Radii ledger; defaults to the document path with .radii.json suffix."),
    ] = None,
    decimate: Annotated[str, typer.Option()] = "1",
    spacing: Annotated[str | None, typer.Option()] = None,
) -> None:
    circle_result_request = CircleResultRequest(
        dataset=dataset,
        share=share,
        cache=cache,
        document=output,
        ledger=ledger if ledger is not None else output.with_suffix(".radii.json"),
        decimate=decimate,
        spacing=spacing,
    )
    if Path.cwd().resolve() != popcircles_cli.REPO_ROOT:
        typer.echo(
            f"visualizer circle: run from the checkout root: {popcircles_cli.REPO_ROOT}",
            err=True,
        )
        raise typer.Exit(1)
    try:
        circle_result.search_and_publish_circle_result(
            circle_result_request,
            composition.circle_result_collaborators(),
        )
    except PopulationApplicationError as error:
        typer.echo(f"visualizer circle: {error}", err=True)
        raise typer.Exit(1) from error
    typer.echo(f"result: {circle_result_request.document}")


@app.command("map", help="Render a map from an existing result document.")
def render_map_command(
    input_path: Annotated[Path, typer.Option("--input")],
    output: Annotated[Path, typer.Option()],
    projection: Annotated[str, typer.Option()] = PLATE_CARREE,
    no_coastlines: Annotated[bool, typer.Option()] = False,
) -> None:
    if projection not in PROJECTIONS:
        message = f"invalid choice: {projection!r}; choose from {', '.join(PROJECTIONS)}"
        raise typer.BadParameter(message)
    render_map.render_file(
        RenderMapFileRequest(input_path, output, projection, no_coastlines),
        composition.render_map_collaborators(),
    )
    typer.echo(output)


@app.command("globe-animation", help="Render an animated globe as SVG or GIF.")
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
    try:
        centre = animated_globe.parse_center(center)
    except ValueError as error:
        raise typer.BadParameter(str(error)) from error
    if not math.isfinite(detalization) or detalization <= 0:
        message = "detalization must be finite and greater than zero"
        raise typer.BadParameter(message)
    if latitude_steps is not None and latitude_steps <= 0:
        message = "latitude-steps must be greater than zero"
        raise typer.BadParameter(message)
    if svg_sprite and latitude_steps is None:
        message = "--svg-sprite requires --latitude-steps"
        raise typer.BadParameter(message)
    try:
        rendered = animated_globe.render_command(
            animated_globe.RenderOptions(
                centre,
                output,
                output_gif,
                svg_sprite,
                latitude_steps,
                detalization,
            )
        )
    except animated_globe.GifRenderError as error:
        raise typer.BadParameter(str(error)) from error
    if rendered is not None:
        typer.echo(rendered)


@app.command("gallery", help="Build the static gallery from result documents.")
def gallery_build_command(
    input_path: Annotated[Path, typer.Option("--input")],
    output: Annotated[Path, typer.Option()],
    refresh_country_sweep: Annotated[bool, typer.Option("--refresh-country-sweep")] = False,
    refresh_country_coverage: Annotated[bool, typer.Option("--refresh-country-coverage")] = False,
) -> None:
    if refresh_country_sweep and refresh_country_coverage:
        message = "--refresh-country-sweep and --refresh-country-coverage are mutually exclusive"
        raise typer.BadParameter(message)
    country_refresh = (
        "sweep" if refresh_country_sweep else "coverage" if refresh_country_coverage else None
    )
    try:
        drawn, skipped = build_gallery.build(
            GalleryBuildRequest(input_path, output, country_refresh),
            composition.gallery_build_collaborators(),
        )
    except GalleryPathError as error:
        raise typer.BadParameter(str(error)) from error
    for one in drawn:
        typer.echo(one.figure)
    for one in skipped:
        if one.kind == "zero-population country":
            typer.echo(f"{one.document}: skipped, country population is zero")
        else:
            typer.echo(f"{one.document}: skipped, kind {one.kind!r} carries no circle to draw")


if __name__ == "__main__":
    app()
