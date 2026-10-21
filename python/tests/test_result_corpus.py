import json
from pathlib import Path
from typing import Any

import pytest

from population_circles.centre_caption import markdown_centre
from population_circles.circle_document import CIRCLE_KINDS, Envelope, circle_of

# The only check a committed document faces (ADR 0011), and through the reader's own boundary rather
# than a second parser — so a schema bump fails here rather than in a gallery build.
CORPUS = sorted((Path(__file__).resolve().parents[2] / "results").glob("*.json"))
ENRICHED_CIRCLE_RECORDS = 11


def test_the_corpus_holds_documents() -> None:
    assert CORPUS


@pytest.mark.parametrize("document", CORPUS, ids=[path.stem for path in CORPUS])
def test_a_committed_document_parses_and_names_its_dataset(document: Path) -> None:
    payload: Any = json.loads(document.read_text(encoding="utf-8"))
    envelope = Envelope.model_validate(payload)
    assert envelope.provenance is not None
    assert envelope.provenance.dataset is not None
    if envelope.document in CIRCLE_KINDS:
        assert circle_of(payload)


def test_every_circle_record_exposes_optional_settlement_context() -> None:
    records: list[tuple[Path, dict[str, Any]]] = []
    for document in CORPUS:
        payload: Any = json.loads(document.read_text(encoding="utf-8"))
        result = payload["result"]
        document_records = result.get("records", [])
        if "circle" in result:
            document_records = [*document_records, result["circle"]]
        if "centre" in result:
            document_records = [*document_records, result]
        records.extend((document, record) for record in document_records)

    contexts: list[dict[str, Any]] = []
    for document, record in records:
        context = record.get("nearest_settlement")
        if context is not None:
            assert set(context) == {"name", "country", "distance_km", "short_form"}, document.name
            contexts.append(context)

    assert len(contexts) == ENRICHED_CIRCLE_RECORDS


def test_readme_centre_fragment_is_generated_from_the_flagship_document() -> None:
    document = next(path for path in CORPUS if path.name == "world-half-30arcsec.json")
    payload: Any = json.loads(document.read_text(encoding="utf-8"))
    fragment = markdown_centre(circle_of(payload))
    readme = document.parents[1] / "README.md"

    assert fragment in readme.read_text(encoding="utf-8")
