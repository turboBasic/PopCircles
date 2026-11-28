from pathlib import Path

from PIL import Image

from population_circles.presentation.gallery.page_data import Drawn, page_data


def test_gallery_data_uses_existing_country_anchors_and_map_names(tmp_path: Path) -> None:
    france = tmp_path / "france.png"
    germany = tmp_path / "germany.png"
    Image.new("RGB", (100, 200)).save(france)
    Image.new("RGB", (200, 100)).save(germany)
    data = page_data(
        [
            Drawn(Path("germany.json"), germany, "Germany", "DE"),
            Drawn(Path("france.json"), france, "France", "FR"),
        ],
        ["Missing country"],
    )

    assert [country["anchor"] for country in data["countries"]] == [
        "country-result-FR",
        "country-result-DE",
        "country-missing-Missing%20country",
    ]
    assert [figure["map"] for country in data["countries"] for figure in country["figures"]] == [
        "france.png",
        "germany.png",
    ]
    assert [
        figure["aspect_ratio"] for country in data["countries"] for figure in country["figures"]
    ] == [0.5, 2.0]
