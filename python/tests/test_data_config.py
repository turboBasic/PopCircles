from population_circles.data.data_config import coastline_dataset_key, country_boundary_dataset_key


def test_dataset_selectors_read_configured_dataset_keys() -> None:
    assert coastline_dataset_key() == "CNTR_BN_01M_2024_4326_COASTL"
    assert country_boundary_dataset_key() == "gisco-countries-2024-1m"
