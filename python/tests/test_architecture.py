import ast
import sys
from pathlib import Path

import pytest

PACKAGE_ROOT = Path(__file__).resolve().parents[1] / "src" / "population_circles"
DOMAIN_GEOMETRY_DEPENDENCIES = {"pyproj", "shapely"}
INFRASTRUCTURE_APPLICATION_CONTRACTS = {
    "population_circles.application.errors",
    "population_circles.application.ports",
}


def imports_from_tree(tree: ast.Module, package: tuple[str, ...] = ()) -> tuple[str, ...]:
    imported: list[str] = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            imported.extend(alias.name for alias in node.names)
        elif isinstance(node, ast.ImportFrom):
            if node.level:
                parent = package[: len(package) - (node.level - 1)]
                module = (
                    ".".join((*parent, *node.module.split(".")))
                    if node.module
                    else ".".join(parent)
                )
            else:
                module = node.module
            if module is None:
                continue
            imported.append(module)
            imported.extend(f"{module}.{alias.name}" for alias in node.names if alias.name != "*")
    return tuple(imported)


def imports(path: Path) -> tuple[str, ...]:
    package = ("population_circles", *path.relative_to(PACKAGE_ROOT).parts[:-1])
    return imports_from_tree(ast.parse(path.read_text(encoding="utf-8")), package)


def dependency_failures(boundary: str, imported: tuple[str, ...]) -> list[str]:
    failures: list[str] = []
    if boundary == "domain":
        failures.extend(
            module
            for module in imported
            if module.startswith("population_circles.")
            and not module.startswith("population_circles.domain")
        )
        failures.extend(
            module
            for module in imported
            if not module.startswith("population_circles")
            and module.partition(".")[0] not in sys.stdlib_module_names
            and module.partition(".")[0] not in DOMAIN_GEOMETRY_DEPENDENCIES
        )
    elif boundary == "application":
        failures.extend(
            module
            for module in imported
            if module.startswith(
                ("population_circles.infrastructure", "population_circles.presentation")
            )
        )
    elif boundary == "infrastructure":
        failures.extend(
            module
            for module in imported
            if module.startswith("population_circles.application.")
            and not any(
                module == contract or module.startswith(f"{contract}.")
                for contract in INFRASTRUCTURE_APPLICATION_CONTRACTS
            )
        )
    return failures


def test_python_packages_follow_the_declared_dependency_direction() -> None:
    failures: list[str] = []
    for boundary in ("domain", "application", "infrastructure"):
        for path in sorted((PACKAGE_ROOT / boundary).rglob("*.py")):
            failures.extend(
                f"{path.relative_to(PACKAGE_ROOT)} imports {imported}"
                for imported in dependency_failures(boundary, imports(path))
            )

    presentation_implementation = [
        path for path in (PACKAGE_ROOT / "presentation").rglob("*.py") if "use_cases" in path.parts
    ]
    failures.extend(
        f"{path.relative_to(PACKAGE_ROOT)} owns an application use case"
        for path in presentation_implementation
    )
    assert failures == []


@pytest.mark.parametrize(
    ("boundary", "forbidden"),
    [
        ("domain", "population_circles.documents.result_document"),
        ("application", "population_circles.infrastructure.geodata.dataset_registry"),
        ("application", "population_circles.presentation.maps.render_map"),
        ("infrastructure", "population_circles.application.use_cases.render_map"),
    ],
)
def test_recognized_forbidden_dependencies_fail_the_gate(boundary: str, forbidden: str) -> None:
    assert dependency_failures(boundary, (forbidden,)) == [forbidden]


@pytest.mark.parametrize(
    ("boundary", "source", "forbidden"),
    [
        (
            "application",
            "from population_circles import infrastructure",
            "population_circles.infrastructure",
        ),
        (
            "domain",
            "from population_circles.documents import result_document",
            "population_circles.documents.result_document",
        ),
        (
            "infrastructure",
            "from population_circles.application import use_cases",
            "population_circles.application.use_cases",
        ),
    ],
)
def test_from_import_aliases_cannot_bypass_the_gate(
    boundary: str, source: str, forbidden: str
) -> None:
    imported = imports_from_tree(ast.parse(source))

    assert forbidden in dependency_failures(boundary, imported)


@pytest.mark.parametrize(
    ("boundary", "package", "source", "forbidden"),
    [
        (
            "application",
            ("population_circles", "application"),
            "from .. import infrastructure",
            "population_circles.infrastructure",
        ),
        (
            "domain",
            ("population_circles", "domain"),
            "from .. import documents",
            "population_circles.documents",
        ),
        (
            "infrastructure",
            ("population_circles", "infrastructure"),
            "from ..application import use_cases",
            "population_circles.application.use_cases",
        ),
    ],
)
def test_relative_imports_cannot_bypass_the_gate(
    boundary: str, package: tuple[str, ...], source: str, forbidden: str
) -> None:
    imported = imports_from_tree(ast.parse(source), package)

    assert forbidden in dependency_failures(boundary, imported)
