import argparse
import subprocess
import sys
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from tempfile import NamedTemporaryFile

from population_circles import render_map
from population_circles.map_frame import PLATE_CARREE, PROJECTIONS

REPO_ROOT = Path(__file__).resolve().parents[3]
Invoke = Callable[[list[str]], subprocess.CompletedProcess[bytes]]


@dataclass(frozen=True)
class Options:
    dataset: str
    share: str
    cache: Path
    output: Path
    document: Path
    ledger: Path
    decimate: str = "1"
    spacing: str | None = None
    projection: str = PLATE_CARREE
    no_coastlines: bool = False


class WorkflowError(Exception):
    pass


def invoke_cli(arguments: list[str]) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(  # noqa: S603 — argument vector, never a shell
        ["mise", "run", "cli", "--", *arguments],  # noqa: S607 — checkout's mise task
        cwd=REPO_ROOT,
        stdout=subprocess.PIPE,
        check=False,
    )


def child(stage: str, arguments: list[str], invoke: Invoke) -> subprocess.CompletedProcess[bytes]:
    try:
        return invoke(arguments)
    except OSError as error:
        message = f"{stage}: could not launch Rust CLI: {error}"
        raise WorkflowError(message) from error


def require_success(stage: str, result: subprocess.CompletedProcess[bytes]) -> None:
    if result.returncode != 0:
        message = f"{stage}: Rust CLI exited {result.returncode}; see child diagnostic above"
        raise WorkflowError(message)


def check_destinations(options: Options) -> None:
    destinations = (options.output, options.document, options.ledger)
    for index, path in enumerate(destinations):
        for other in destinations[index + 1 :]:
            if path.resolve() == other.resolve() or (
                path.exists() and other.exists() and path.samefile(other)
            ):
                message = (
                    f"destinations: image, document and ledger must differ: {path} and {other}"
                )
                raise WorkflowError(message)


def publish(destination: Path, content: bytes) -> None:
    temporary: Path | None = None
    try:
        destination.parent.mkdir(parents=True, exist_ok=True)
        with NamedTemporaryFile(
            dir=destination.parent, prefix=f".{destination.name}.", delete=False
        ) as stream:
            temporary = Path(stream.name)
            stream.write(content)
        temporary.replace(destination)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def workflow(options: Options, invoke: Invoke = invoke_cli) -> None:
    try:
        check_destinations(options)
    except OSError as error:
        message = f"destinations: {error}"
        raise WorkflowError(message) from error
    common = [
        "--dataset",
        options.dataset,
        "--cache",
        str(options.cache),
        "--decimate",
        options.decimate,
    ]
    probe = child("cache probe", ["table", "query", *common], invoke)
    if probe.returncode == 3:  # noqa: PLR2004 — Rust's missing/incompatible input exit code
        require_success("table build", child("table build", ["table", "build", *common], invoke))
    else:
        require_success("cache probe", probe)
    search = [
        "smallest-for-share",
        *common,
        "--share",
        options.share,
        "--ledger",
        str(options.ledger),
    ]
    if options.spacing is not None:
        search.extend(["--spacing", options.spacing])
    result = child("search", search, invoke)
    require_success("search", result)
    try:
        publish(options.document, result.stdout)
    except OSError as error:
        message = f"document publication: could not replace {options.document}: {error}"
        raise WorkflowError(message) from error
    try:
        render_map.render_file(
            options.document,
            options.output,
            options.projection,
            no_coastlines=options.no_coastlines,
        )
    except Exception as error:
        # Rendering errors vary by library; the retained document locates the failure.
        message = f"render: {error}; current search document retained at {options.document}"
        raise WorkflowError(message) from error


def parse(argv: list[str] | None = None) -> Options:
    parser = argparse.ArgumentParser(
        description="Search a named dataset and save its circle map (from the checkout root)."
    )
    parser.add_argument("--dataset", required=True)
    parser.add_argument(
        "--share", required=True, help="Population share in whole percent; validated by Rust."
    )
    parser.add_argument("--cache", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument(
        "--document", type=Path, help="Search JSON; defaults to the image path with .json suffix."
    )
    parser.add_argument(
        "--ledger",
        type=Path,
        help="Radii ledger; defaults to the document path with .radii.json suffix.",
    )
    parser.add_argument("--decimate", default="1")
    parser.add_argument("--spacing")
    parser.add_argument("--projection", choices=PROJECTIONS, default=PLATE_CARREE)
    parser.add_argument("--no-coastlines", action="store_true")
    args = parser.parse_args(argv)
    document: Path = (
        args.document if args.document is not None else args.output.with_suffix(".json")
    )
    ledger: Path = args.ledger if args.ledger is not None else document.with_suffix(".radii.json")
    return Options(
        dataset=args.dataset,
        share=args.share,
        cache=args.cache,
        output=args.output,
        document=document,
        ledger=ledger,
        decimate=args.decimate,
        spacing=args.spacing,
        projection=args.projection,
        no_coastlines=args.no_coastlines,
    )


def main(argv: list[str] | None = None) -> int:
    options = parse(argv)
    if Path.cwd().resolve() != REPO_ROOT:
        sys.stderr.write(f"circle-map: run from the checkout root: {REPO_ROOT}\n")
        return 1
    try:
        workflow(options)
    except WorkflowError as error:
        sys.stderr.write(f"circle-map: {error}\n")
        return 1
    sys.stdout.write(f"image: {options.output}\ndocument: {options.document}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
