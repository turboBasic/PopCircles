import json
import re
import stat
import subprocess
import time
import zipfile
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import cast

import pytest

from repo_tools import publish_gallery as pg


def write_json(path: Path, payload: dict[str, object]) -> None:
    path.write_text(json.dumps(payload), encoding="utf-8")


def country_result(kind: str, combination: str = "fixture") -> dict[str, object]:
    return {
        "document": kind,
        "result": {"selection": {"combination_id": combination, "resolution": 10}},
    }


def no_sleep(_seconds: float) -> None:
    pass


@dataclass
class FakeGitHub:
    releases: list[dict[str, object]] = field(
        default_factory=lambda: cast(list[dict[str, object]], [])
    )
    runs: list[tuple[int, str]] = field(default_factory=lambda: cast(list[tuple[int, str]], []))
    fail: str | None = None
    commands: list[tuple[str, ...]] = field(default_factory=lambda: cast(list[tuple[str, ...]], []))
    uploaded: list[str] = field(default_factory=lambda: cast(list[str], []))
    zip_files: list[tuple[str, ...]] = field(
        default_factory=lambda: cast(list[tuple[str, ...]], [])
    )
    watched: list[int] = field(default_factory=lambda: cast(list[int], []))

    def __call__(self, *args: str, cwd: Path = pg.REPO_ROOT) -> subprocess.CompletedProcess[str]:  # noqa: C901, PLR0912
        del cwd
        self.commands.append(args)
        output = ""
        if args[:3] == ("git", "rev-parse", "--abbrev-ref"):
            output = "main\n"
        elif args[:3] == ("gh", "auth", "status"):
            if self.fail == "auth":
                return self.failed(args, "not authenticated")
        elif args[:3] == ("gh", "release", "list"):
            output = json.dumps(self.releases)
        elif args[:3] == ("gh", "release", "create"):
            self.releases.append({"tagName": pg.RELEASE_TAG, "isDraft": True})
        elif args[:3] == ("gh", "release", "upload"):
            if self.fail == "upload":
                return self.failed(args, "upload denied")
            archive = Path(args[-1])
            with zipfile.ZipFile(archive) as zipped:
                self.zip_files.append(tuple(zipped.namelist()))
            self.uploaded.append(archive.name)
            for release in self.releases:
                if release["tagName"] == pg.RELEASE_TAG:
                    assets = cast(list[str], release.setdefault("assets", []))
                    assets.append(archive.name)
        elif args[:3] == ("gh", "run", "list"):
            if args[args.index("--json") + 1] == "databaseId":
                output = "\n".join(str(identifier) for identifier, _ in self.runs)
            else:
                output = "\n".join(f"{identifier}\t{url}" for identifier, url in self.runs)
        elif args[:3] == ("gh", "workflow", "run"):
            if self.fail == "dispatch":
                return self.failed(args, "dispatch denied")
            identifier = max((run_id for run_id, _ in self.runs), default=0) + 1
            self.runs.insert(0, (identifier, f"https://github.test/runs/{identifier}"))
        elif args[:3] == ("gh", "run", "watch"):
            identifier = int(args[3])
            self.watched.append(identifier)
            if self.fail == "watch":
                return self.failed(args, "deployment failed")
        else:
            raise AssertionError(f"unexpected command: {args}")  # noqa: EM102, TRY003
        return subprocess.CompletedProcess(args, 0, stdout=output, stderr="")

    @staticmethod
    def failed(args: tuple[str, ...], message: str) -> subprocess.CompletedProcess[str]:
        return subprocess.CompletedProcess(args, 1, stdout="", stderr=message)


@pytest.fixture
def layout(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> tuple[Path, Path]:
    results = tmp_path / "results"
    results.mkdir()
    write_json(results / "world.json", {"document": "most-populous"})
    write_json(results / "country-sweep-other.json", country_result("country-sweep", "other"))
    build = tmp_path / "build"
    site = build / "gallery"
    site.mkdir(parents=True)
    (site / "index.html").write_text("previous site", encoding="utf-8")
    config = tmp_path / "data" / "config.toml"
    registry = tmp_path / "data" / "registry.toml"
    config.parent.mkdir()
    config.write_text("[defaults]\n", encoding="utf-8")
    registry.write_text("[datasets]\n", encoding="utf-8")
    monkeypatch.setattr(pg, "REPO_ROOT", tmp_path)
    monkeypatch.setattr(pg, "RESULTS", results)
    monkeypatch.setattr(pg, "BUILD", build)
    monkeypatch.setattr(pg, "SITE", site)
    monkeypatch.setattr(pg, "CONFIG", config)
    monkeypatch.setattr(pg, "REGISTRY", registry)
    return results, site


def successful_build(
    seen: list[tuple[Path, bool]] | None = None,
) -> Callable[..., tuple[list[object], list[object]]]:
    def build(
        corpus: Path, output: Path, *, refresh_country_sweep: bool
    ) -> tuple[list[object], list[object]]:
        if seen is not None:
            seen.append((corpus, refresh_country_sweep))
        assert (corpus / "world.json").is_file()
        assert corpus != pg.RESULTS
        output.mkdir()
        (output / "index.html").write_text("new site", encoding="utf-8")
        (output / "map.png").write_bytes(b"map")
        write_json(corpus / "country-sweep-fixture-10-half.json", country_result("country-sweep"))
        write_json(corpus / "country-fixture-10-AA-half.json", country_result("country-smallest"))
        return [], []

    return build


def test_publish_builds_in_isolation_persists_only_country_outputs_and_waits_for_run_id(
    layout: tuple[Path, Path], monkeypatch: pytest.MonkeyPatch
) -> None:
    results, site = layout
    previous = {path.name: path.read_bytes() for path in results.iterdir()}
    github = FakeGitHub(
        releases=[{"tagName": pg.RELEASE_TAG, "isDraft": True, "assets": ["old.zip"]}],
        runs=[(41, "https://github.test/runs/41")],
    )
    seen: list[tuple[Path, bool]] = []
    monkeypatch.setattr(pg, "run", github)
    monkeypatch.setattr(pg, "build_gallery", successful_build(seen))
    monkeypatch.setattr(time, "sleep", no_sleep)

    pg.publish(refresh_country_sweep=True)

    assert len(seen) == 1
    assert seen[0][1]
    assert seen[0][0] != results
    assert (site / "index.html").read_text(encoding="utf-8") == "new site"
    assert (site / ".nojekyll").is_file()
    assert (results / "world.json").read_bytes() == previous["world.json"]
    assert (results / "country-sweep-other.json").read_bytes() == previous[
        "country-sweep-other.json"
    ]
    assert json.loads((results / "country-sweep-fixture-10-half.json").read_text())["document"] == (
        "country-sweep"
    )
    assert json.loads((results / "country-fixture-10-AA-half.json").read_text())["document"] == (
        "country-smallest"
    )
    assert len(github.uploaded) == 1
    assert re.fullmatch(r"gallery-\d{8}T\d{6}Z-[0-9a-f]{32}\.zip", github.uploaded[0])
    assert ".nojekyll" in github.zip_files[0]
    assert "index.html" in github.zip_files[0]
    assert github.watched == [42]
    dispatch = next(
        command for command in github.commands if command[:3] == ("gh", "workflow", "run")
    )
    assert f"asset_name={github.uploaded[0]}" in dispatch
    assert not any(command[:3] == ("gh", "release", "create") for command in github.commands)


def test_failed_render_preserves_existing_site_and_result_documents(
    layout: tuple[Path, Path], monkeypatch: pytest.MonkeyPatch
) -> None:
    results, site = layout
    old_results = {path.name: path.read_bytes() for path in results.iterdir()}
    old_site = (site / "index.html").read_bytes()
    github = FakeGitHub()

    def fail_build(corpus: Path, output: Path, *, refresh_country_sweep: bool) -> None:
        del refresh_country_sweep
        write_json(corpus / "country-sweep-new.json", country_result("country-sweep"))
        output.mkdir()
        raise ValueError("render failed")  # noqa: EM101, TRY003

    monkeypatch.setattr(pg, "run", github)
    monkeypatch.setattr(pg, "build_gallery", fail_build)

    with pytest.raises(pg.PublishGalleryError, match="gallery build failed: render failed"):
        pg.publish()

    assert {path.name: path.read_bytes() for path in results.iterdir()} == old_results
    assert (site / "index.html").read_bytes() == old_site
    assert not any(command[:2] == ("gh", "release") for command in github.commands)


def test_publish_creates_the_single_draft_handoff_release(
    layout: tuple[Path, Path], monkeypatch: pytest.MonkeyPatch
) -> None:
    del layout
    github = FakeGitHub(runs=[(5, "https://github.test/runs/5")])
    monkeypatch.setattr(pg, "run", github)
    monkeypatch.setattr(pg, "build_gallery", successful_build())
    monkeypatch.setattr(time, "sleep", no_sleep)

    pg.publish()

    creates = [command for command in github.commands if command[:3] == ("gh", "release", "create")]
    assert len(creates) == 1
    assert "--draft" in creates[0]
    assert github.releases[0]["tagName"] == pg.RELEASE_TAG


def test_authentication_failure_is_actionable_before_build(
    layout: tuple[Path, Path], monkeypatch: pytest.MonkeyPatch
) -> None:
    del layout
    github = FakeGitHub(fail="auth")
    monkeypatch.setattr(pg, "run", github)

    with pytest.raises(pg.PublishGalleryError, match="provide GH_TOKEN"):
        pg.publish()

    assert not any(command[:2] == ("gh", "release") for command in github.commands)


@pytest.mark.parametrize("failure", ["upload", "dispatch", "watch"])
def test_remote_failures_keep_existing_release_assets(
    layout: tuple[Path, Path], monkeypatch: pytest.MonkeyPatch, failure: str
) -> None:
    del layout
    github = FakeGitHub(
        releases=[{"tagName": pg.RELEASE_TAG, "isDraft": True, "assets": ["previous.zip"]}],
        runs=[(10, "https://github.test/runs/10")],
        fail=failure,
    )
    monkeypatch.setattr(pg, "run", github)
    monkeypatch.setattr(pg, "build_gallery", successful_build())
    monkeypatch.setattr(time, "sleep", no_sleep)

    with pytest.raises(pg.PublishGalleryError):
        pg.publish()

    release = github.releases[0]
    assets = cast(list[str], release["assets"])
    assert "previous.zip" in assets
    if failure == "upload":
        assert assets == ["previous.zip"]
    else:
        assert len(assets) == 2  # noqa: PLR2004
        assert github.uploaded[0] in assets
    assert not any(command[:3] == ("gh", "release", "delete-asset") for command in github.commands)


def test_asset_names_are_unique() -> None:
    assert pg.new_asset_name() != pg.new_asset_name()


@pytest.mark.parametrize("entry", ["../outside.txt", "/outside.txt", "folder/../../outside.txt"])
def test_stage_rejects_unsafe_paths_before_writing_site(
    layout: tuple[Path, Path], monkeypatch: pytest.MonkeyPatch, tmp_path: Path, entry: str
) -> None:
    _, site = layout
    archive_path = tmp_path / "unsafe.zip"
    with zipfile.ZipFile(archive_path, "w") as archive:
        archive.writestr("index.html", "new")
        archive.writestr(entry, "escape")
    asset_name = pg.new_asset_name()
    github = StageGitHub(asset_name, archive_path)
    monkeypatch.setattr(pg, "run", github)
    old_site = (site / "index.html").read_bytes()

    with pytest.raises(pg.PublishGalleryError, match="unsafe ZIP entry"):
        pg.stage(asset_name)

    assert (site / "index.html").read_bytes() == old_site
    assert not (tmp_path / "outside.txt").exists()


def test_stage_rejects_symlink_entries(
    layout: tuple[Path, Path], monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    del layout
    archive_path = tmp_path / "symlink.zip"
    with zipfile.ZipFile(archive_path, "w") as archive:
        archive.writestr("index.html", "new")
        entry = zipfile.ZipInfo("link")
        entry.external_attr = (stat.S_IFLNK | 0o777) << 16
        archive.writestr(entry, "outside")
    asset_name = pg.new_asset_name()
    monkeypatch.setattr(pg, "run", StageGitHub(asset_name, archive_path))

    with pytest.raises(pg.PublishGalleryError, match="unsafe ZIP entry"):
        pg.stage(asset_name)


def test_stage_requires_exact_asset_from_draft_release(
    layout: tuple[Path, Path], monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    del layout
    asset_name = pg.new_asset_name()
    archive_path = tmp_path / "site.zip"
    with zipfile.ZipFile(archive_path, "w") as archive:
        archive.writestr("index.html", "new")
    github = StageGitHub(asset_name, archive_path, assets=[asset_name, asset_name])
    monkeypatch.setattr(pg, "run", github)

    with pytest.raises(pg.PublishGalleryError, match="expected one"):
        pg.stage(asset_name)

    assert not any(command[:3] == ("gh", "release", "download") for command in github.commands)


def test_stage_rejects_wrong_asset_and_archive_without_root_index(
    layout: tuple[Path, Path], monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    del layout
    asset_name = pg.new_asset_name()
    archive_path = tmp_path / "site.zip"
    with zipfile.ZipFile(archive_path, "w") as archive:
        archive.writestr("nested/index.html", "new")
    github = StageGitHub(asset_name, archive_path, assets=["other.zip"])
    monkeypatch.setattr(pg, "run", github)
    with pytest.raises(pg.PublishGalleryError, match="expected one"):
        pg.stage(asset_name)

    github.assets = [asset_name]
    with pytest.raises(pg.PublishGalleryError, match=r"root index\.html"):
        pg.stage(asset_name)


def test_cleanup_removes_only_superseded_zip_assets(
    layout: tuple[Path, Path], monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    del layout
    selected, old = pg.new_asset_name(), pg.new_asset_name()
    github = StageGitHub(selected, tmp_path / "unused.zip", assets=[selected, old, "notes.txt"])
    monkeypatch.setattr(pg, "run", github)

    pg.cleanup(selected)

    deletes = [
        command for command in github.commands if command[:3] == ("gh", "release", "delete-asset")
    ]
    assert deletes == [("gh", "release", "delete-asset", pg.RELEASE_TAG, old, "--yes")]


@dataclass
class StageGitHub:
    selected: str
    archive: Path
    assets: list[str] | None = None
    commands: list[tuple[str, ...]] = field(default_factory=lambda: cast(list[tuple[str, ...]], []))

    def __call__(self, *args: str, cwd: Path = pg.REPO_ROOT) -> subprocess.CompletedProcess[str]:
        del cwd
        self.commands.append(args)
        if args[:3] == ("gh", "release", "view"):
            assets = self.assets if self.assets is not None else [self.selected]
            output = json.dumps({"isDraft": True, "assets": [{"name": name} for name in assets]})
        elif args[:3] == ("gh", "release", "download"):
            destination = Path(args[args.index("--dir") + 1]) / self.selected
            destination.write_bytes(self.archive.read_bytes())
            output = ""
        elif args[:3] == ("gh", "release", "delete-asset"):
            output = ""
        else:
            raise AssertionError(args)
        return subprocess.CompletedProcess(args, 0, stdout=output, stderr="")
