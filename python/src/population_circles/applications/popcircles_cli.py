import subprocess
from pathlib import Path
from typing import Protocol

from population_circles.applications.errors import PopulationApplicationError

REPO_ROOT = Path(__file__).resolve().parents[4]


class PopulationRunner(Protocol):
    def __call__(
        self,
        stage: str,
        arguments: list[str],
        *,
        accepted_exit_codes: tuple[int, ...] = (0,),
    ) -> subprocess.CompletedProcess[bytes]: ...


def invoke_popcircles_cli(
    stage: str,
    arguments: list[str],
    *,
    accepted_exit_codes: tuple[int, ...] = (0,),
) -> subprocess.CompletedProcess[bytes]:
    try:
        result = subprocess.run(  # noqa: S603 — argument vector, never a shell
            ["mise", "run", "popcircles-cli", "--", *arguments],  # noqa: S607 — checkout's mise task
            cwd=REPO_ROOT,
            stdout=subprocess.PIPE,
            check=False,
        )
    except OSError as error:
        message = f"{stage}: could not launch Rust CLI: {error}"
        raise PopulationApplicationError(message) from error
    if result.returncode not in accepted_exit_codes:
        message = f"{stage}: Rust CLI exited {result.returncode}; see child diagnostic above"
        raise PopulationApplicationError(message)
    return result
