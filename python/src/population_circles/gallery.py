import argparse
import html
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any
from urllib.parse import quote

from matplotlib import pyplot as plt

from population_circles.circle_document import CIRCLE_KINDS, Envelope, circle_of
from population_circles.circle_map import Invoke, child, invoke_cli, require_success
from population_circles.country_outline import load_outline
from population_circles.dataset_registry import load
from population_circles.map_frame import PLATE_CARREE
from population_circles.render_map import citation, registered_coastline, render

TITLE = "The smallest circle on the globe holding a share of world population"
HALF_SHARE = 0.5
HALF_SHARE_PERCENT = "50"


@dataclass(frozen=True)
class Drawn:
    document: Path
    figure: Path


@dataclass(frozen=True)
class Skipped:
    document: Path
    kind: str


def page(drawn: list[Drawn], missing: list[str]) -> str:
    # Nothing about a figure is written here: the share, the radius, the centre, the table and the
    # attribution are inside the PNG, drawn from the document by `render`. A caption in this page
    # would be a second place those live, and the one nobody regenerates.
    figures = "\n".join(
        f'<figure><img src="{html.escape(one.figure.name)}" alt="{html.escape(one.figure.stem)}">'
        f'<figcaption><a href="{html.escape(one.document.name)}">'
        f"{html.escape(one.document.name)}</a></figcaption></figure>"
        for one in drawn
    )
    without_result = (
        "<h2>Countries without a 50% result</h2>\n<ul>\n"
        + "\n".join(f"<li>{html.escape(name)}</li>" for name in missing)
        + "\n</ul>"
        if missing
        else ""
    )
    return (
        '<!doctype html>\n<html lang="en">\n<meta charset="utf-8">\n'
        f"<title>{html.escape(TITLE)}</title>\n<h1>{html.escape(TITLE)}</h1>\n"
        f"{figures}\n{without_result}\n</html>\n"
    )


def country_documents(corpus: Path, invoke: Invoke) -> list[str]:
    listed = child("country list", ["country", "list"], invoke)
    require_success("country list", listed)
    listing: Any = json.loads(listed.stdout)
    if listing.get("document") != "country-list":
        message = "country list: Rust CLI returned a document other than country-list"
        raise ValueError(message)
    selection: Any = listing["result"]["selection"]
    countries: list[dict[str, Any]] = listing["result"]["countries"]
    country_ids = [country["id"] for country in countries]
    if len(country_ids) != len(set(country_ids)):
        message = "country list: Rust CLI returned duplicate feature IDs"
        raise ValueError(message)

    arguments = [
        "country",
        "sweep",
        "--combination",
        selection["combination_id"],
        "--resolution",
        str(selection["resolution"]),
        "--from",
        HALF_SHARE_PERCENT,
        "--to",
        HALF_SHARE_PERCENT,
        "--step",
        "1",
    ]
    for country_id in country_ids:
        arguments.extend(["--country", country_id])
    swept = child("country sweep", arguments, invoke)
    require_success("country sweep", swept)
    sweep: Any = json.loads(swept.stdout)
    if sweep.get("document") != "country-sweep":
        message = "country sweep: Rust CLI returned a document other than country-sweep"
        raise ValueError(message)

    selection = sweep["result"]["selection"]
    if selection["combination_id"] != listing["result"]["selection"]["combination_id"] or (
        selection["resolution"] != listing["result"]["selection"]["resolution"]
    ):
        message = "country sweep: result selection differs from the configured country list"
        raise ValueError(message)
    expected = {country["id"]: country for country in countries}
    result_ids: set[str] = set()
    for swept_country in sweep["result"]["countries"]:
        country = swept_country["country"]
        country_id = country["id"]
        if country_id not in expected or country_id in result_ids:
            message = f"country sweep: unexpected or duplicate feature ID {country_id!r}"
            raise ValueError(message)
        records = [record for record in swept_country["records"] if record["share"] == HALF_SHARE]
        if len(records) != 1:
            message = f"country sweep: {country_id!r} has no unique 50% result"
            raise ValueError(message)
        result_ids.add(country_id)
        document = {
            "schema_version": sweep["schema_version"],
            "document": "country-smallest",
            "tool": sweep["tool"],
            "tool_version": sweep["tool_version"],
            "earth_model": sweep["earth_model"],
            "result": {
                "selection": selection,
                "country": country,
                "population_dataset": selection["population_source"]["name"],
                "decimation": selection["resolution"],
                "ledger": swept_country["ledger"],
                "circle": records[0]["circle"],
            },
        }
        filename = (
            f"country-{quote(selection['combination_id'], safe='')}-"
            f"{selection['resolution']}-{quote(country_id, safe='')}-half.json"
        )
        (corpus / filename).write_text(
            json.dumps(document, separators=(",", ":")) + "\n", encoding="utf-8"
        )

    sweep_filename = (
        f"country-sweep-{quote(selection['combination_id'], safe='')}-"
        f"{selection['resolution']}-half.json"
    )
    (corpus / sweep_filename).write_bytes(swept.stdout)
    return [country["name"] for country in countries if country["id"] not in result_ids]


def build(
    corpus: Path, output: Path, invoke: Invoke = invoke_cli
) -> tuple[list[Drawn], list[Skipped]]:
    # One basemap union and one registry read for the whole run, which is the only reason this is a
    # second caller of `render` rather than a loop over `render_map.main`.
    registry = load()
    coastline, coastline_attribution = registered_coastline(registry)
    output.mkdir(parents=True, exist_ok=True)
    (output / "index.html").unlink(missing_ok=True)
    missing = country_documents(corpus, invoke)

    drawn: list[Drawn] = []
    skipped: list[Skipped] = []
    for source in sorted(corpus.glob("*.json")):
        payload: Any = json.loads(source.read_text(encoding="utf-8"))
        envelope = Envelope.model_validate(payload)
        if envelope.document not in CIRCLE_KINDS and envelope.document != "country-smallest":
            skipped.append(Skipped(source, envelope.document))
            continue
        try:
            answer = circle_of(payload)
            population_attribution = (
                answer.selection.population_source.attribution
                if answer.selection is not None
                else None
            ) or citation(registry, answer.provenance.dataset)
            outline_and_credit = None
            if answer.country is not None:
                boundary_attribution = (
                    answer.selection.boundary_source.attribution
                    if answer.selection is not None
                    else None
                ) or citation(registry, answer.country.boundary_dataset)
                outline_and_credit = (load_outline(answer, registry), boundary_attribution)
            figure = render(
                answer,
                PLATE_CARREE,
                population_attribution,
                coastline=coastline,
                coastline_attribution=coastline_attribution,
                outline_and_credit=outline_and_credit,
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

    (output / "index.html").write_text(page(drawn, missing), encoding="utf-8")
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
