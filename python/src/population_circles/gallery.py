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
    country_name: str | None = None
    country_id: str | None = None


@dataclass(frozen=True)
class Skipped:
    document: Path
    kind: str


def page(drawn: list[Drawn], missing: list[str]) -> str:
    countries: dict[tuple[str, str], list[Drawn]] = {}
    world: list[Drawn] = []
    for one in drawn:
        if one.country_name is None or one.country_id is None:
            world.append(one)
        else:
            countries.setdefault((one.country_name, one.country_id), []).append(one)

    def figure(one: Drawn) -> str:
        name = html.escape(one.figure.name, quote=True)
        title = html.escape(one.figure.stem)
        document = html.escape(one.document.name, quote=True)
        return (
            f'<figure><a class="map-link" href="{name}"><img src="{name}" alt="{title}" '
            f'loading="lazy"></a><figcaption><a href="{document}">View result data</a>'
            "</figcaption></figure>"
        )

    country_navigation = "\n".join(
        f'<a href="#country-{quote(identifier, safe="")}">{html.escape(name)}</a>'
        for name, identifier in sorted(countries, key=lambda item: (item[0].casefold(), item[1]))
    )
    country_sections = "\n".join(
        f'<details class="country" id="country-{quote(identifier, safe="")}">'
        f"<summary><span>{html.escape(name)}</span>"
        f'<span class="map-count">{len(figures)} map{"s" if len(figures) != 1 else ""}</span>'
        "</summary><div class=figure-grid>"
        + "\n".join(figure(one) for one in figures)
        + "</div></details>"
        for (name, identifier), figures in sorted(
            countries.items(), key=lambda item: (item[0][0].casefold(), item[0][1])
        )
    )
    world_section = (
        '<section class="world"><h2>World maps</h2><div class=figure-grid>'
        + "\n".join(figure(one) for one in world)
        + "</div></section>"
        if world
        else ""
    )
    without_result = (
        '<details class="missing"><summary>Countries without a 50% result '
        f'<span class="map-count">{len(missing)}</span></summary><ul>'
        + "\n".join(f"<li>{html.escape(name)}</li>" for name in missing)
        + "</ul></details>"
        if missing
        else ""
    )
    return (
        '<!doctype html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n'
        '<meta name="viewport" content="width=device-width, initial-scale=1">\n'
        f'<meta name="description" content="{html.escape(TITLE, quote=True)}">\n'
        f"<title>{html.escape(TITLE)} · PopCircles</title>\n"
        '<link rel="preconnect" href="https://fonts.googleapis.com">\n'
        '<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>\n'
        '<link href="https://fonts.googleapis.com/css2?family=DM+Sans:wght@400;500;600;700&'
        'family=Manrope:wght@500;600;700;800&display=swap" rel="stylesheet">\n'
        "<style>\n"
        ":root{color-scheme:dark;--bg:#0b1020;--surface:#121a2b;--surface-2:#19243a;"
        "--text:#eef3ff;--muted:#a4b0c5;--accent:#80e0c0;--line:#29364d;"
        'font-family:"DM Sans",system-ui,sans-serif}*{box-sizing:border-box}body{margin:0;'
        "color:var(--text);background:radial-gradient(ellipse at 50% -20%,#203456 0,"
        "var(--bg) 55%);font-size:16px;line-height:1.55}a{color:inherit}"
        ".wrap{width:min(1240px,calc(100% - 2rem));margin:auto}.hero{padding:4rem 0 2rem}"
        ".eyebrow{color:var(--accent);font-size:.75rem;font-weight:700;letter-spacing:.16em;"
        "text-transform:uppercase}.hero h1{font-family:Manrope,system-ui,sans-serif;"
        "font-size:clamp(2rem,5vw,4rem);line-height:1.08;max-width:850px;margin:.7rem 0 1rem;"
        "letter-spacing:-.045em}.hero p{max-width:720px;color:var(--muted);margin:0}"
        ".country-nav-wrap{position:sticky;top:0;z-index:2;padding:.8rem 0;"
        "background:#0b1020eF;border-block:1px solid var(--line);backdrop-filter:blur(16px)}"
        ".country-nav-label{font-size:.75rem;color:var(--muted);margin:0 0 .55rem}"
        ".country-nav{display:flex;flex-wrap:wrap;gap:.45rem;max-height:6.5rem;overflow:auto}"
        ".country-nav a{font-size:.8rem;text-decoration:none;padding:.3rem .65rem;"
        "border:1px solid var(--line);border-radius:999px;color:var(--muted)}"
        ".country-nav a:hover,.country-nav a:focus-visible{color:var(--text);"
        "border-color:var(--accent);outline:none}.content{padding:1.5rem 0 4rem}"
        ".country,.missing{background:var(--surface);border:1px solid var(--line);"
        "border-radius:1rem;margin:0 0 .7rem;overflow:hidden}.country{scroll-margin-top:9rem}"
        ".country>summary,.missing>summary{"
        "display:flex;align-items:center;justify-content:space-between;gap:1rem;"
        "padding:1rem 1.2rem;font-family:Manrope,system-ui,sans-serif;"
        "font-size:1.08rem;font-weight:700;cursor:pointer;"
        "list-style:none}.country>summary::-webkit-details-marker,"
        ".missing>summary::-webkit-details-marker{"
        "display:none}.country>summary:focus-visible,.missing>summary:focus-visible{"
        "outline:2px solid var(--accent);outline-offset:-3px}.country[open]>summary{"
        "border-bottom:1px solid var(--line);background:var(--surface-2)}"
        ".country>summary::after{content:'⌄';color:var(--accent);font-size:1.2rem;"
        "transition:transform .2s}.country[open]>summary::after{transform:rotate(180deg)}"
        '.map-count{font:500 .78rem "DM Sans",system-ui,sans-serif;color:var(--muted);'
        "white-space:nowrap}.figure-grid{display:grid;grid-template-columns:repeat(auto-fit,"
        "minmax(min(100%,420px),1fr));gap:1rem;padding:1rem}.world h2{font-family:Manrope,"
        "system-ui,sans-serif;margin:2rem 0 .5rem}.world figure,.country figure{margin:0;"
        "background:#0d1422;border:1px solid var(--line);border-radius:.8rem;overflow:hidden}"
        ".map-link{display:block}.map-link img{display:block;width:100%;height:auto}"
        "figcaption{padding:.75rem 1rem;color:var(--muted);font-size:.85rem}"
        "figcaption a{text-decoration-color:#65758e;text-underline-offset:3px}"
        ".missing{margin-top:1.5rem}.missing ul{columns:2;color:var(--muted);"
        "padding:0 2.5rem 1rem}"
        "footer{border-top:1px solid var(--line);padding:1.5rem 0;color:var(--muted);"
        "font-size:.8rem}"
        "@media(max-width:600px){.hero{padding-top:2.5rem}.country-nav{flex-wrap:nowrap;"
        "max-height:none;overflow-x:auto}.missing ul{columns:1}}"
        "</style>\n</head>\n<body>\n<div class=wrap>\n"
        "<header class=hero><div class=eyebrow>PopCircles · Map gallery</div>"
        f"<h1>{html.escape(TITLE)}</h1>"
        "<p>Explore the smallest circle holding half of each country's population, alongside "
        "selected world population circles. Choose a country below to jump to its map."
        "</p></header>\n"
        "<div class=country-nav-wrap><p class=country-nav-label>Jump to a country</p>"
        f'<nav class=country-nav aria-label="Countries">{country_navigation}</nav></div>\n'
        f"<main class=content>{world_section}{country_sections}{without_result}</main>\n"
        "<footer>Population circle results with source documents and data credits.</footer>\n"
        "</div>\n</body>\n</html>\n"
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
        swept = child("country sweep", arguments, invoke)
        require_success("country sweep", swept)
        sweep = json.loads(swept.stdout)
        if sweep.get("document") != "country-sweep":
            message = "country sweep: Rust CLI returned a document other than country-sweep"
            raise ValueError(message)

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
    sources = [
        (source, json.loads(source.read_text(encoding="utf-8")))
        for source in sorted(corpus.glob("*.json"))
        if source.name not in generated
    ]
    sources.extend((corpus / name, payload) for name, payload in generated.items())
    for source, payload in sorted(sources, key=lambda item: item[0].name):
        envelope = Envelope.model_validate(payload)
        if envelope.document not in CIRCLE_KINDS and envelope.document != "country-smallest":
            skipped.append(Skipped(source, envelope.document))
            if source.name in generated:
                (output / source.name).write_text(json.dumps(payload, indent=2), encoding="utf-8")
            continue
        if (
            envelope.document == "country-smallest"
            and payload["result"]["circle"]["target"]["total"] == 0
        ):
            skipped.append(Skipped(source, "zero-population country"))
            (output / source.name).unlink(missing_ok=True)
            (output / f"{source.stem}.png").unlink(missing_ok=True)
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
        drawn.append(
            Drawn(
                source,
                target,
                answer.country.name if answer.country is not None else None,
                answer.country.id if answer.country is not None else None,
            )
        )

    for name, payload in generated.items():
        (corpus / name).write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    (output / "index.html").write_text(page(drawn, missing), encoding="utf-8")
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
