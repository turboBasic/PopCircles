import math
from pathlib import Path
from typing import Annotated

import typer

from population_circles import composition
from population_circles.application.errors import GalleryPathError, PopulationApplicationError
from population_circles.application.use_cases import circle_map, gallery_build, render_map
from population_circles.application.use_cases.gallery_build import GalleryBuildRequest
from population_circles.application.use_cases.render_map import RenderMapFileRequest
from population_circles.infrastructure import popcircles_cli
from population_circles.infrastructure.file_publishing import write_bytes_atomically
from population_circles.infrastructure.geodata.country_boundaries import (
    load_map_boundaries,
)
from population_circles.infrastructure.geodata.dataset_registry import load
from population_circles.presentation.maps import render_globe
from population_circles.presentation.maps.frame import PLATE_CARREE, PROJECTIONS

app = typer.Typer(no_args_is_help=True)


def _render_circle_map_file(
    input_path: Path,
    output_path: Path,
    projection: str = PLATE_CARREE,
    *,
    no_coastlines: bool = False,
    coverage_enriched: bool = False,
) -> None:
    render_map.render_file(
        RenderMapFileRequest(
            input_path,
            output_path,
            projection,
            no_coastlines,
            coverage_enriched,
        ),
        composition.render_map_collaborators(),
    )


@app.command("circle", help="Search for a population circle and render its map.")
def circle_command(
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
    if projection not in PROJECTIONS:
        message = f"invalid choice: {projection!r}; choose from {', '.join(PROJECTIONS)}"
        raise typer.BadParameter(message)
    search_document = document if document is not None else output.with_suffix(".json")
    circle_map_request = circle_map.CircleMapRequest(
        dataset=dataset,
        share=share,
        cache=cache,
        output=output,
        document=search_document,
        ledger=ledger if ledger is not None else search_document.with_suffix(".radii.json"),
        decimate=decimate,
        spacing=spacing,
        projection=projection,
        no_coastlines=no_coastlines,
    )
    if Path.cwd().resolve() != popcircles_cli.REPO_ROOT:
        typer.echo(
            f"visualizer circle: run from the checkout root: {popcircles_cli.REPO_ROOT}",
            err=True,
        )
        raise typer.Exit(1)
    try:
        circle_map.search_and_render_circle_map(
            circle_map_request,
            popcircles_cli.invoke_popcircles_cli,
            load_boundaries=lambda answer: load_map_boundaries(answer, load()),
            publish_file=write_bytes_atomically,
            render_file=_render_circle_map_file,
        )
    except PopulationApplicationError as error:
        typer.echo(f"visualizer circle: {error}", err=True)
        raise typer.Exit(1) from error
    typer.echo(f"image: {circle_map_request.output}\ndocument: {circle_map_request.document}")


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
        centre = render_globe.parse_center(center)
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
        rendered = render_globe.render_command(
            render_globe.RenderOptions(
                centre,
                output,
                output_gif,
                svg_sprite,
                latitude_steps,
                detalization,
            )
        )
    except render_globe.GifRenderError as error:
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
        drawn, skipped = gallery_build.build(
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
