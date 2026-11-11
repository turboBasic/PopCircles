import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, TypedDict
from urllib.parse import quote

from jinja2 import Environment, FileSystemLoader, select_autoescape
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
TEMPLATES = Environment(
    loader=FileSystemLoader(Path(__file__).parent),
    autoescape=select_autoescape(["html"]),
)


@dataclass(frozen=True)
class Drawn:
    document: Path
    figure: Path
    country_name: str | None = None
    country_id: str | None = None


@dataclass(frozen=True)
class Skipped:
    document: Path
    kind: str


class PageCountry(TypedDict):
    name: str
    anchor: str
    figures: list[Drawn]
    missing: bool


def page(drawn: list[Drawn], missing: list[str]) -> str:
    countries: dict[tuple[str, str], list[Drawn]] = {}
    world: list[Drawn] = []
    for one in drawn:
        if one.country_name is None or one.country_id is None:
            world.append(one)
        else:
            countries.setdefault((one.country_name, one.country_id), []).append(one)

    ordered = sorted(countries.items(), key=lambda item: (item[0][0].casefold(), item[0][1]))
    page_countries: list[PageCountry] = [
        {
            "name": name,
            "anchor": f"country-result-{quote(identifier, safe='')}",
            "figures": figures,
            "missing": False,
        }
        for (name, identifier), figures in ordered
    ]
    page_countries.extend(
        {
            "name": name,
            "anchor": f"country-missing-{quote(name, safe='')}",
            "figures": [],
            "missing": True,
        }
        for name in missing
    )
    page_countries.sort(key=lambda country: (country["name"].casefold(), country["anchor"]))
    navigation_groups: dict[str, list[PageCountry]] = {}
    for country in page_countries:
        navigation_groups.setdefault(country["name"][:1].upper() or "#", []).append(country)

    return TEMPLATES.get_template("gallery.html").render(
        title=TITLE,
        navigation_groups=navigation_groups.items(),
        countries=page_countries,
        world=world,
    )


def country_result_document(
    sweep: dict[str, Any], selection: dict[str, Any], swept_country: dict[str, Any]
) -> dict[str, Any]:
    country = swept_country["country"]
    record = next(record for record in swept_country["records"] if record["share"] == HALF_SHARE)
    return {
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
            "circle": record["circle"],
        },
    }


def cached_sweep(
    path: Path, selection: dict[str, Any], countries: list[dict[str, Any]]
) -> dict[str, Any] | None:
    try:
        sweep: Any = json.loads(path.read_text(encoding="utf-8"))
        if Envelope.model_validate(sweep).document != "country-sweep":
            return None
        if sweep["result"]["selection"] != selection:
            return None
        swept_countries = sweep["result"]["countries"]
        expected = [(country["id"], country["name"]) for country in countries]
        actual = [
            (country["country"]["id"], country["country"]["name"]) for country in swept_countries
        ]
        if actual != expected:
            return None
        for country in swept_countries:
            if sum(record["share"] == HALF_SHARE for record in country["records"]) != 1:
                return None
            circle_of(country_result_document(sweep, selection, country))
    except OSError, ValueError, TypeError, KeyError, IndexError:
        return None
    return sweep


def country_documents(
    invoke: Invoke, corpus: Path, *, refresh_country_sweep: bool = False
) -> tuple[list[str], dict[str, dict[str, Any]]]:
    listed = child("country list", ["country", "list"], invoke)
    require_success("country list", listed)
    listing: Any = json.loads(listed.stdout)
    if listing.get("document") != "country-list":
        message = "country list: Rust CLI returned a document other than country-list"
        raise ValueError(message)
    selection: dict[str, Any] = listing["result"]["selection"]
    countries: list[dict[str, Any]] = listing["result"]["countries"]
    country_ids = [country["id"] for country in countries]
    if len(country_ids) != len(set(country_ids)):
        message = "country list: Rust CLI returned duplicate feature IDs"
        raise ValueError(message)

    sweep_filename = (
        f"country-sweep-{quote(selection['combination_id'], safe='')}-"
        f"{selection['resolution']}-half.json"
    )
    sweep = (
        None
        if refresh_country_sweep
        else cached_sweep(corpus / sweep_filename, selection, countries)
    )
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
    if sweep is None:
        print(
            f"Gallery: calculating 50% circles for {len(countries)} countries",
            file=sys.stderr,
            flush=True,
        )
        swept = child("country sweep", arguments, invoke)
        require_success("country sweep", swept)
        sweep = json.loads(swept.stdout)
        if sweep.get("document") != "country-sweep":
            message = "country sweep: Rust CLI returned a document other than country-sweep"
            raise ValueError(message)
    else:
        print(
            f"Gallery: reusing saved 50% country sweep ({len(countries)} countries)",
            file=sys.stderr,
            flush=True,
        )

    sweep_selection = sweep["result"]["selection"]
    if sweep_selection != listing["result"]["selection"]:
        message = "country sweep: result selection differs from the configured country list"
        raise ValueError(message)
    expected = {country["id"]: country for country in countries}
    result_ids: set[str] = set()
    documents: dict[str, dict[str, Any]] = {}
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
        document = country_result_document(sweep, sweep_selection, swept_country)
        filename = (
            f"country-{quote(selection['combination_id'], safe='')}-"
            f"{selection['resolution']}-{quote(country_id, safe='')}-half.json"
        )
        documents[filename] = document

    documents[sweep_filename] = sweep
    print(
        f"Gallery: prepared {len(documents)} country result documents",
        file=sys.stderr,
        flush=True,
    )
    return (
        [country["name"] for country in countries if country["id"] not in result_ids],
        documents,
    )


def build(
    corpus: Path,
    output: Path,
    invoke: Invoke = invoke_cli,
    *,
    refresh_country_sweep: bool = False,
) -> tuple[list[Drawn], list[Skipped]]:
    # One basemap union and one registry read for the whole run, which is the only reason this is a
    # second caller of `render` rather than a loop over `render_map.main`.
    registry = load()
    coastline, coastline_attribution = registered_coastline(registry)
    output.mkdir(parents=True, exist_ok=True)
    (output / "index.html").unlink(missing_ok=True)
    corpus.mkdir(parents=True, exist_ok=True)
    missing, generated = country_documents(
        invoke, corpus, refresh_country_sweep=refresh_country_sweep
    )

    drawn: list[Drawn] = []
    skipped: list[Skipped] = []
    unavailable = set(missing)
    sources = [
        (source, json.loads(source.read_text(encoding="utf-8")))
        for source in sorted(corpus.glob("*.json"))
        if source.name not in generated
    ]
    sources.extend((corpus / name, payload) for name, payload in generated.items())
    sources.sort(key=lambda item: item[0].name)
    print(
        f"Gallery: rendering {len(sources)} result documents",
        file=sys.stderr,
        flush=True,
    )
    for index, (source, payload) in enumerate(sources, start=1):
        envelope = Envelope.model_validate(payload)
        if envelope.document not in CIRCLE_KINDS and envelope.document != "country-smallest":
            print(
                f"Gallery: [{index}/{len(sources)}] skipping {source.name} ({envelope.document})",
                file=sys.stderr,
                flush=True,
            )
            skipped.append(Skipped(source, envelope.document))
            if source.name in generated:
                (output / source.name).write_text(json.dumps(payload, indent=2), encoding="utf-8")
            continue
        if (
            envelope.document == "country-smallest"
            and payload["result"]["circle"]["target"]["total"] == 0
        ):
            print(
                f"Gallery: [{index}/{len(sources)}] skipping {source.name} (zero population)",
                file=sys.stderr,
                flush=True,
            )
            skipped.append(Skipped(source, "zero-population country"))
            unavailable.add(payload["result"]["country"]["name"])
            (output / source.name).unlink(missing_ok=True)
            (output / f"{source.stem}.png").unlink(missing_ok=True)
            continue
        try:
            print(
                f"Gallery: [{index}/{len(sources)}] rendering {source.name}",
                file=sys.stderr,
                flush=True,
            )
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
        drawn.append(
            Drawn(
                source,
                target,
                answer.country.name if answer.country is not None else None,
                answer.country.id if answer.country is not None else None,
            )
        )
        print(
            f"Gallery: [{index}/{len(sources)}] wrote {target.name}",
            file=sys.stderr,
            flush=True,
        )

    for name, payload in generated.items():
        (corpus / name).write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    (output / "index.html").write_text(page(drawn, sorted(unavailable)), encoding="utf-8")
    print(
        f"Gallery: complete ({len(drawn)} maps, {len(skipped)} skipped)",
        file=sys.stderr,
        flush=True,
    )
    return drawn, skipped


def parse(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Render every renderable document in a corpus.")
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--refresh-country-sweep", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse(argv)
    corpus: Path = args.input
    output: Path = args.output

    drawn, skipped = build(corpus, output, refresh_country_sweep=args.refresh_country_sweep)
    for one in drawn:
        print(one.figure)
    for one in skipped:
        if one.kind == "zero-population country":
            print(f"{one.document}: skipped, country population is zero")
        else:
            print(f"{one.document}: skipped, kind {one.kind!r} carries no circle to draw")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
