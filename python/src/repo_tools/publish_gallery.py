import argparse
import json
import re
import shutil
import subprocess
import sys
import time
import zipfile
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from stat import S_ISLNK
from tempfile import TemporaryDirectory
from uuid import uuid4

from population_circles.gallery import build as build_gallery
from population_circles.gallery import same_country_answer

REPO_ROOT = Path(__file__).resolve().parents[3]
RESULTS = REPO_ROOT / "results"
BUILD = REPO_ROOT / "build"
SITE = BUILD / "gallery"
CONFIG = REPO_ROOT / "data" / "config.toml"
REGISTRY = REPO_ROOT / "data" / "registry.toml"
RELEASE_TAG = "gallery-handoff"
WORKFLOW = "gallery.yml"
WORKFLOW_REF = "main"
RUN_TIMEOUT_S = 90
POLL_S = 3
ASSET_NAME = re.compile(r"gallery-(?P<created>\d{8}T\d{12}Z)-[0-9a-f]{32}\.zip\Z")


class PublishGalleryError(Exception):
    pass


def run(*args: str, cwd: Path = REPO_ROOT) -> subprocess.CompletedProcess[str]:
    return subprocess.run(  # noqa: S603 — argv only; Git and gh arguments are fixed or validated
        args, cwd=cwd, capture_output=True, text=True, check=False
    )


def checked(*args: str) -> str:
    result = run(*args)
    if result.returncode:
        detail = result.stderr.strip()
        message = f"{' '.join(args[:2])} failed"
        if detail:
            message = f"{message}: {detail}"
        raise PublishGalleryError(message)
    return result.stdout.strip()


def require_inputs() -> None:
    missing = [path for path in (CONFIG, REGISTRY) if not path.is_file()]
    if not RESULTS.is_dir():
        missing.append(RESULTS)
    if missing:
        names = ", ".join(str(path.relative_to(REPO_ROOT)) for path in missing)
        raise PublishGalleryError(f"required gallery inputs are missing: {names}")  # noqa: EM102, TRY003


def replace_site(staged: Path) -> None:
    BUILD.mkdir(parents=True, exist_ok=True)
    backup = BUILD / f".gallery-backup-{uuid4().hex}"
    moved_old = False
    if SITE.exists() or SITE.is_symlink():
        SITE.replace(backup)
        moved_old = True
    try:
        staged.replace(SITE)
    except OSError:
        if moved_old:
            backup.replace(SITE)
        raise
    if moved_old:
        if backup.is_dir() and not backup.is_symlink():
            shutil.rmtree(backup)
        else:
            backup.unlink()


def persist_country_results(corpus: Path) -> None:
    for source in sorted(corpus.glob("*.json")):
        destination = RESULTS / source.name
        content = source.read_bytes()
        if destination.is_file():
            previous = destination.read_bytes()
            if previous == content or same_country_answer(previous, content):
                continue
        try:
            payload = json.loads(content)
        except json.JSONDecodeError:
            continue
        if payload.get("document") not in {"country-sweep", "country-smallest"}:
            continue
        temporary = RESULTS / f".{source.name}.{uuid4().hex}.tmp"
        try:
            temporary.write_bytes(content)
            temporary.replace(destination)
        finally:
            temporary.unlink(missing_ok=True)


def zip_site(site: Path, destination: Path) -> None:
    (site / ".nojekyll").touch()
    with zipfile.ZipFile(destination, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for source in sorted(site.rglob("*")):
            if source.is_file():
                archive.write(source, source.relative_to(site).as_posix())


def release_assets() -> list[str]:
    output = checked("gh", "release", "view", RELEASE_TAG, "--json", "isDraft,assets")
    release = json.loads(output)
    if not release["isDraft"]:
        raise PublishGalleryError(f"release {RELEASE_TAG!r} is not a draft")  # noqa: EM102, TRY003
    return [asset["name"] for asset in release["assets"]]


def safe_asset_name(name: str) -> re.Match[str]:
    match = ASSET_NAME.fullmatch(name)
    if match is None:
        message = "asset name is not a gallery handoff ZIP"
        raise PublishGalleryError(message)
    return match


def stage(asset_name: str) -> None:
    safe_asset_name(asset_name)
    BUILD.mkdir(parents=True, exist_ok=True)
    assets = release_assets()
    if assets.count(asset_name) != 1:
        raise PublishGalleryError(f"expected one {asset_name!r} asset on {RELEASE_TAG}")  # noqa: EM102, TRY003
    with TemporaryDirectory(prefix=".gallery-stage-", dir=BUILD) as temporary:
        workspace = Path(temporary)
        checked(
            "gh", "release", "download", RELEASE_TAG, "--pattern", asset_name, "--dir", temporary
        )
        archive_path = workspace / asset_name
        if not archive_path.is_file():
            message = "release download did not produce the requested asset"
            raise PublishGalleryError(message)
        extracted = workspace / "site"
        extracted.mkdir()
        try:
            with zipfile.ZipFile(archive_path) as archive:
                members = archive.infolist()
                names: set[str] = set()
                for member in members:
                    name = member.filename
                    path = PurePosixPath(name)
                    mode = member.external_attr >> 16
                    if (
                        not name
                        or "\\" in name
                        or path.is_absolute()
                        or ".." in path.parts
                        or path.as_posix() != name.rstrip("/")
                        or name in names
                        or S_ISLNK(mode)
                    ):
                        raise PublishGalleryError(f"unsafe ZIP entry: {name!r}")  # noqa: EM102, TRY003
                    names.add(name)
                if "index.html" not in names:
                    message = "gallery ZIP has no root index.html"
                    raise PublishGalleryError(message)
                for member in members:
                    destination = extracted.joinpath(*PurePosixPath(member.filename).parts)
                    if member.is_dir():
                        destination.mkdir(parents=True, exist_ok=True)
                    else:
                        destination.parent.mkdir(parents=True, exist_ok=True)
                        destination.write_bytes(archive.read(member))
        except (OSError, zipfile.BadZipFile) as error:
            message = f"gallery ZIP could not be staged: {error}"
            raise PublishGalleryError(message) from error
        replace_site(extracted)


def cleanup(asset_name: str) -> None:
    selected = safe_asset_name(asset_name)
    assets = release_assets()
    if assets.count(asset_name) != 1:
        raise PublishGalleryError(f"expected one {asset_name!r} asset on {RELEASE_TAG}")  # noqa: EM102, TRY003
    selected_time = selected["created"]
    for name in assets:
        match = ASSET_NAME.fullmatch(name)
        if name != asset_name and match is not None and match["created"] < selected_time:
            checked("gh", "release", "delete-asset", RELEASE_TAG, name, "--yes")


def new_asset_name() -> str:
    timestamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%S%fZ")
    return f"gallery-{timestamp}-{uuid4().hex}.zip"


def ensure_draft_release() -> None:
    rows = json.loads(
        checked("gh", "release", "list", "--limit", "1000", "--json", "tagName,isDraft")
    )
    release = next((item for item in rows if item["tagName"] == RELEASE_TAG), None)
    if release is None:
        checked(
            "gh",
            "release",
            "create",
            RELEASE_TAG,
            "--draft",
            "--title",
            "PopCircles gallery handoff",
            "--notes",
            "Temporary handoff for the manually deployed PopCircles gallery.",
        )
    elif not release["isDraft"]:
        raise PublishGalleryError(f"release {RELEASE_TAG!r} exists but is not a draft")  # noqa: EM102, TRY003


def latest_run_id() -> int:
    output = checked(
        "gh",
        "run",
        "list",
        "--workflow",
        WORKFLOW,
        "--event",
        "workflow_dispatch",
        "--branch",
        WORKFLOW_REF,
        "--limit",
        "100",
        "--json",
... (+137 lines) [see remaining: rtk proxy git -c core.fsmonitor=false show HEAD:python/src/repo_tools/publish_gallery.py | tail -n +232]
