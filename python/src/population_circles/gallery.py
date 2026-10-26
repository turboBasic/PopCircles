import argparse
import html
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from matplotlib import pyplot as plt

from population_circles.circle_document import CIRCLE_KINDS, Envelope, circle_of
from population_circles.dataset_registry import load
from population_circles.map_frame import PLATE_CARREE
from population_circles.render_map import citation, registered_coastline, render

TITLE = "The smallest circle on the globe holding a share of world population"


@dataclass(frozen=True)
class Drawn:
    document: Path
    figure: Path


@dataclass(frozen=True)
class Skipped:
    document: Path
    kind: str


def page(drawn: list[Drawn], skipped: list[Skipped]) -> str:
    # Nothing about a figure is written here: the share, the radius, the centre, the table and the
    # attribution are inside the PNG, drawn from the document by `render`. A caption in this page
    # would be a second place those live, and the one nobody regenerates.
    figures = "\n".join(
        f'<figure><img src="{html.escape(one.figure.name)}" alt="{html.escape(one.figure.stem)}">'
        f'<figcaption><a href="{html.escape(one.document.name)}">'
        f"{html.escape(one.document.name)}</a></figcaption></figure>"
        for one in drawn
    )
    kinds = "\n".join(
        f"<li>{html.escape(one.document.name)} — kind <code>{html.escape(one.kind)}</code></li>"
        for one in skipped
    )
    # Stated rather than absent: which kinds a reader can draw is the reader's business (ADR 0011),
    # so a corpus document with no figure is named here instead of leaving a reader to wonder.
    not_drawn = (
        f"<h2>Committed documents with no figure</h2>\n<ul>\n{kinds}\n</ul>" if kinds else ""
    )
    return (
        '<!doctype html>\n<html lang="en">\n<meta charset="utf-8">\n'
        f"<title>{html.escape(TITLE)}</title>\n<h1>{html.escape(TITLE)}</h1>\n"
        f"{figures}\n{not_drawn}\n</html>\n"
    )


def build(corpus: Path, output: Path) -> tuple[list[Drawn], list[Skipped]]:
    # One basemap union and one registry read for the whole run, which is the only reason this is a
    # second caller of `render` rather than a loop over `render_map.main`.
    registry = load()
    coastline, coastline_attribution = registered_coastline(registry)
    output.mkdir(parents=True, exist_ok=True)

    drawn: list[Drawn] = []
    skipped: list[Skipped] = []
    for source in sorted(corpus.glob("*.json")):
        payload: Any = json.loads(source.read_text(encoding="utf-8"))
        envelope = Envelope.model_validate(payload)
        if envelope.document not in CIRCLE_KINDS:
            skipped.append(Skipped(source, envelope.document))
            continue
        try:
            answer = circle_of(payload)
            figure = render(
                answer,
                PLATE_CARREE,
                citation(registry, answer.provenance.dataset),
                coastline=coastline,
                coastline_attribution=coastline_attribution,
            )
        except Exception as error:
            # The exception carries the document, because a traceback out of pydantic or
            # matplotlib names neither. Re-raised rather than collected: `index.html` is written
            # after the loop, so a failure publishes no gallery rather than one with a hole in it.
            error.add_note(f"raised while rendering {source}")
            raise
        target = output / f"{source.stem}.png"
        figure.savefig(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
            target,
            dpi=200,
        )
        plt.close(figure)
        # The document travels beside its figure, so the caption's link resolves on the published
        # site rather than pointing back into a tree the reader does not have.
        (output / source.name).write_text(json.dumps(payload, indent=2), encoding="utf-8")
        drawn.append(Drawn(source, target))

    (output / "index.html").write_text(page(drawn, skipped), encoding="utf-8")
    return drawn, skipped


def parse(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Render every renderable document in a corpus.")
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse(argv)
    corpus: Path = args.input
    output: Path = args.output

    drawn, skipped = build(corpus, output)
    for one in drawn:
        print(one.figure)
    for one in skipped:
        print(f"{one.document}: skipped, kind {one.kind!r} carries no circle to draw")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
