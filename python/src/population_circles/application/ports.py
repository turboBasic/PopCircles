from typing import Protocol


class PopulationCommandResult(Protocol):
    @property
    def returncode(self) -> int: ...

    @property
    def stdout(self) -> bytes: ...


class PopulationRunner(Protocol):
    def __call__(
        self,
        stage: str,
        arguments: list[str],
        *,
        accepted_exit_codes: tuple[int, ...] = (0,),
    ) -> PopulationCommandResult: ...
