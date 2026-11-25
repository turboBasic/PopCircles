import functools
import tomllib
from pathlib import Path
from typing import cast

_CONFIG_PATH = Path(__file__).resolve().parents[3] / "data" / "config.toml"


@functools.cache
def _load_config() -> dict[str, object]:
    return cast(dict[str, object], tomllib.loads(_CONFIG_PATH.read_text(encoding="utf-8")))


@functools.cache
def coastline_dataset_key() -> str:
    defaults = _load_config().get("defaults")
    if not isinstance(defaults, dict):
        message = "data/config.toml must set defaults.coastline_dataset"
        raise TypeError(message)
    key = cast(dict[str, object], defaults).get("coastline_dataset")
    if not isinstance(key, str) or not key:
        message = "data/config.toml must set defaults.coastline_dataset"
        raise ValueError(message)
    return key


@functools.cache
def country_boundary_dataset_key() -> str:
    config = _load_config()
    defaults = config.get("defaults")
    combinations = config.get("combinations")
    if not isinstance(defaults, dict) or not isinstance(combinations, dict):
        message = "data/config.toml must define a default country combination"
        raise TypeError(message)
    combinations = cast(dict[str, object], combinations)
    name = cast(dict[str, object], defaults).get("country_combination")
    if not isinstance(name, str) or name not in combinations:
        message = "data/config.toml defaults.country_combination is not configured"
        raise ValueError(message)
    combination = combinations[name]
    if not isinstance(combination, dict):
        message = f"data/config.toml combination {name!r} is malformed"
        raise TypeError(message)
    source = cast(dict[str, object], combination).get("boundary_source")
    prefix = "dataset:"
    if not isinstance(source, str) or not source.startswith(prefix):
        message = f"data/config.toml combination {name!r} must use a registered boundary dataset"
        raise ValueError(message)
    return source.removeprefix(prefix)
