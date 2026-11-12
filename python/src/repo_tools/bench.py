import argparse
import hashlib
import json
import os
import platform
import resource
import shutil
import subprocess
import sys
import tempfile
import time
import tomllib
import uuid
from collections.abc import Mapping
from datetime import UTC, datetime
from pathlib import Path
from typing import cast

REPO_ROOT = Path(__file__).resolve().parents[3]
DEFAULT_HISTORY_DIR = REPO_ROOT / "benchmarks/history"
SAMPLE_COUNT = 3
MAX_WORKERS = 256
MAX_SHARE_PERCENT = 100
SMOKE_ITERATIONS = 100
REQUIRED = {
    "schema_version",
    "commit",
    "workload",
    "workload_version",
    "parameters",
    "input_identity",
    "cpu",
    "os",
    "host_memory_bytes",
    "rust_toolchain",
    "build_profile",
    "worker_count",
    "cache_mode",
    "raw_samples_seconds",
    "wall_time_seconds",
    "cpu_time_seconds",
    "peak_rss_bytes",
}


def executable(name: str) -> str:
    path = shutil.which(name)
    if path is None:
        raise FileNotFoundError(name)
    return path


def command_output(name: str, *arguments: str, stderr: int | None = None) -> str:
    return subprocess.check_output(  # noqa: S603 — fixed command names and argv, never a shell
        [executable(name), *arguments], text=True, stderr=stderr, cwd=REPO_ROOT
    )


def host_memory_bytes() -> int | None:
    try:
        return os.sysconf("SC_PHYS_PAGES") * os.sysconf("SC_PAGE_SIZE")
    except AttributeError, OSError, ValueError:
        return None


def peak_rss_bytes() -> int | None:
    value = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    if value <= 0:
        return None
    return int(value if sys.platform == "darwin" else value * 1024)


def cpu_name() -> str:
    if sys.platform == "darwin":
        try:
            return command_output(
                "sysctl", "-n", "machdep.cpu.brand_string", stderr=subprocess.DEVNULL
            ).strip()
        except OSError, subprocess.CalledProcessError:
            return "unavailable"
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.lower().startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        pass
    return platform.processor() or "unavailable"


def run_synthetic(iterations: int, sample_count: int) -> tuple[list[float], float, float]:
    samples: list[float] = []
    started = time.perf_counter()
    cpu_started = time.process_time()
    value = b"popcircles synthetic benchmark v1"
    for _ in range(sample_count):
        sample_started = time.perf_counter()
        for _ in range(iterations):
            value = hashlib.sha256(value).digest()
        samples.append(time.perf_counter() - sample_started)
    _ = value
    return samples, time.perf_counter() - started, time.process_time() - cpu_started


def system_fields() -> dict[str, object]:
    return {
        "commit": command_output("git", "rev-parse", "HEAD").strip(),
        "cpu": cpu_name(),
        "os": f"{platform.system()} {platform.release()} ({platform.machine()})",
        "host_memory_bytes": host_memory_bytes(),
        "rust_toolchain": command_output("rustc", "-Vv").splitlines()[0],
    }


def record(  # noqa: PLR0913, PLR0917 — fields map directly to the record schema
    workload: str,
    parameters: Mapping[str, object],
    input_identity: str,
    raw_samples: list[float],
    wall_time: float,
    cpu_time: float,
    peak_rss: int | None,
    build_profile: str,
    worker_count: int,
    cache_mode: str,
) -> dict[str, object]:
    return {
        "schema_version": 1,
        "workload": workload,
        "workload_version": 1,
        "parameters": parameters,
        "input_identity": input_identity,
        **system_fields(),
        "build_profile": build_profile,
        "worker_count": worker_count,
        "cache_mode": cache_mode,
        "raw_samples_seconds": raw_samples,
        "wall_time_seconds": wall_time,
        "cpu_time_seconds": cpu_time,
        "peak_rss_bytes": peak_rss,
    }


def write_record(
    history_dir: Path, result: dict[str, object], *, require_clean: bool = True
) -> Path:
    if require_clean and command_output(
        "git",
        "status",
        "--porcelain",
        "--untracked-files=normal",
        "--",
        ".",
        ":(exclude)benchmarks/history/**",
    ):
        print("commit the benchmark source before writing a history record", file=sys.stderr)
        raise SystemExit(1)
    history_dir.mkdir(parents=True, exist_ok=True)
    timestamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%S.%fZ")
    path = history_dir / f"{timestamp}-{uuid.uuid4().hex[:8]}-{result['workload']}.json"
    payload = json.dumps(result, indent=2, sort_keys=True) + "\n"
    with path.open("x", encoding="utf-8") as record:
        record.write(payload)
    return path


def synthetic_record(iterations: int, samples: int) -> dict[str, object]:
    raw, wall_time, cpu_time = run_synthetic(iterations, samples)
    result = record(
        "synthetic",
        {"iterations_per_sample": iterations, "samples": samples},
        "synthetic:sha256-chain-v1",
        raw,
        wall_time,
        cpu_time,
        peak_rss_bytes(),
        "synthetic-python",
        1,
        "synthetic-no-cache",
    )
    if result.keys() < REQUIRED:
        raise ValueError("benchmark record is missing required fields")  # noqa: EM101, TRY003
    return result


def smoke() -> None:
    with tempfile.TemporaryDirectory(prefix="popcircles-bench-") as temporary:
        history = Path(temporary)
        first = write_record(
            history, synthetic_record(SMOKE_ITERATIONS, SAMPLE_COUNT), require_clean=False
        )
        before = first.read_bytes()
        second = write_record(
            history, synthetic_record(SMOKE_ITERATIONS, SAMPLE_COUNT), require_clean=False
        )
        if first == second or first.read_bytes() != before:
            print("benchmark history path was reused or overwritten", file=sys.stderr)
            raise SystemExit(1)
        for path in (first, second):
            record = json.loads(path.read_text())
            if record.keys() < REQUIRED or len(record["raw_samples_seconds"]) != SAMPLE_COUNT:
                print("benchmark smoke record is incomplete", file=sys.stderr)
                raise SystemExit(1)
        print("synthetic benchmark smoke passed")


def positive_int(value: str) -> int:
    number = int(value)
    if number <= 0:
        raise argparse.ArgumentTypeError("must be positive")  # noqa: EM101, TRY003
    return number


def positive_float(value: str) -> float:
    number = float(value)
    if number <= 0:
        raise argparse.ArgumentTypeError("must be positive")  # noqa: EM101, TRY003
    return number


def share_percent(value: str) -> int:
    number = positive_int(value)
    if number > MAX_SHARE_PERCENT:
        raise argparse.ArgumentTypeError("must be between 1 and 100")  # noqa: EM101, TRY003
    return number


def worker_count(value: str) -> int:
    number = positive_int(value)
    if number > MAX_WORKERS:
        raise argparse.ArgumentTypeError(f"must be at most {MAX_WORKERS}")  # noqa: EM102, TRY003
    return number


def add_record_options(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--history-dir", type=Path, default=DEFAULT_HISTORY_DIR)
    parser.add_argument("--samples", type=positive_int, default=SAMPLE_COUNT)


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    workloads = root.add_subparsers(dest="workload", required=True)

    synthetic = workloads.add_parser("synthetic")
    synthetic.add_argument("--iterations", type=positive_int, default=10_000)
    add_record_options(synthetic)

    table = workloads.add_parser("table-build")
    table.add_argument("--width", type=positive_int, required=True)
    table.add_argument("--height", type=positive_int, required=True)
    table.add_argument("--factor", type=positive_int, required=True)
    table.add_argument("--mode", choices=["discard", "write"], default="discard")
    add_record_options(table)

    kernel = workloads.add_parser("kernel")
    kernel.add_argument("--width", type=positive_int, required=True)
    kernel.add_argument("--height", type=positive_int, required=True)
    kernel.add_argument("--radius-km", type=positive_float, required=True)
    kernel.add_argument("--rows", type=positive_int, default=128)
    add_record_options(kernel)

    circle = workloads.add_parser("circle-evaluation")
    circle.add_argument("--radius-km", type=positive_float, required=True)
    circle.add_argument("--centres", type=positive_int, required=True)
    circle.add_argument("--cache", type=Path)
    add_record_options(circle)

    world = workloads.add_parser("world-search")
    world.add_argument("--dataset", required=True)
    world.add_argument("--cache", type=Path, required=True)
    world.add_argument("--radius-km", type=positive_float, required=True)
    world.add_argument("--spacing", type=positive_int, required=True)
    world.add_argument("--workers", type=worker_count, default=1)
    add_record_options(world)

    country = workloads.add_parser("country-search")
    country.add_argument("--combination", required=True)
    country.add_argument("--resolution", type=positive_int, required=True)
    country.add_argument("--country", required=True)
    country.add_argument("--share", type=share_percent, required=True)
    country.add_argument("--spacing", type=positive_int, required=True)
    country.add_argument("--workers", type=worker_count, default=1)
    add_record_options(country)
    return root


def build_bench(name: str) -> Path:
    result = subprocess.run(  # noqa: S603 — fixed Cargo argv, no shell
        [executable("cargo"), "bench", "--locked", "--no-run", "-p", "popcircles", "--bench", name],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    output = result.stdout + result.stderr
    for raw_line in output.splitlines():
        line = raw_line.strip()
        if line.startswith("Executable ") and "(" in line and line.endswith(")"):
            candidate = Path(line.rsplit("(", 1)[1][:-1])
            candidate = candidate if candidate.is_absolute() else REPO_ROOT / candidate
            if candidate.is_file():
                return candidate
    raise RuntimeError(f"Cargo did not report the `{name}` benchmark executable")  # noqa: EM102, TRY003


def build_cli() -> Path:
    subprocess.run(  # noqa: S603 — fixed Cargo argv, no shell
        [executable("cargo"), "build", "--locked", "--release", "-p", "popcircles-cli"],
        cwd=REPO_ROOT,
        check=True,
    )
    return REPO_ROOT / "build/target/release/popcircles"


def run_captured(command: list[str], *, workers: int = 1) -> dict[str, object]:
    request = json.dumps({"command": command, "workers": workers})
    result = subprocess.run(  # noqa: S603 — fixed Python worker argv, no shell
        [sys.executable, str(Path(__file__).resolve()), "--capture", request],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    measured = json_object(json.loads(result.stdout))
    if measured["returncode"] != 0:
        raise RuntimeError(str(measured["stderr"] or f"workload exited {measured['returncode']}"))
    return measured


def json_object(value: object) -> dict[str, object]:
    if not isinstance(value, dict):
        raise TypeError("benchmark worker returned invalid JSON")  # noqa: EM101, TRY003
    entries = cast(dict[object, object], value)
    if not all(isinstance(key, str) for key in entries):
        raise ValueError("benchmark worker returned non-string object keys")  # noqa: EM101, TRY003
    return cast(dict[str, object], entries)


def json_number(value: object, name: str) -> float:
    if not isinstance(value, int | float) or isinstance(value, bool):
        raise TypeError(f"benchmark worker returned invalid {name}")  # noqa: EM102, TRY003
    return float(value)


def json_samples(value: object) -> list[float]:
    if not isinstance(value, list):
        raise TypeError("benchmark returned invalid samples")  # noqa: EM101, TRY003
    return [json_number(sample, "sample") for sample in cast(list[object], value)]


def capture(command: list[str], workers: int) -> None:
    executable_path = Path(command[0]).resolve()
    allowed = (REPO_ROOT / "build/target/release").resolve()
    if not executable_path.is_relative_to(allowed):
        print("benchmark worker executable must come from build/target/release", file=sys.stderr)
        raise SystemExit(2)
    os.environ["RAYON_NUM_THREADS"] = str(workers)
    started = time.perf_counter()
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    result = subprocess.run(  # noqa: S603 — validated argv, never a shell
        command, cwd=REPO_ROOT, capture_output=True, text=True, check=False
    )
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    value = {
        "returncode": result.returncode,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "wall_time_seconds": time.perf_counter() - started,
        "cpu_time_seconds": after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime,
        "peak_rss_bytes": int(
            after.ru_maxrss if sys.platform == "darwin" else after.ru_maxrss * 1024
        ),
    }
    print(json.dumps(value))


def repo_path(path: Path) -> Path:
    return path if path.is_absolute() else REPO_ROOT / path


def cache_identity(base: Path) -> str:
    header = Path(f"{repo_path(base)}.header.json")
    return hashlib.sha256(header.read_bytes()).hexdigest()


def country_identity(combination: str, resolution: int) -> str:
    with (REPO_ROOT / "data/config.toml").open("rb") as source:
        config = tomllib.load(source)
    try:
        local = config["combinations"][combination]["resolutions"][str(resolution)]["local"]
    except KeyError as error:
        message = f"unknown prepared country selection: {combination}/{resolution}"
        raise ValueError(message) from error
    manifest = REPO_ROOT / local / "manifest.json"
    return hashlib.sha256(manifest.read_bytes()).hexdigest()


def component_record(args: argparse.Namespace) -> dict[str, object]:
    bench_name = {"table-build": "table_build", "kernel": "kernel", "circle-evaluation": "circle"}[
        args.workload
    ]
    executable_path = build_bench(bench_name)
    command = [str(executable_path)]
    if args.workload == "table-build":
        command.extend(
            [
                "--width",
                str(args.width),
                "--height",
                str(args.height),
                "--factor",
                str(args.factor),
                "--mode",
                args.mode,
                "--samples",
                str(args.samples),
            ]
        )
        parameters = {
            "width": args.width,
            "height": args.height,
            "factor": args.factor,
            "mode": args.mode,
            "samples": args.samples,
        }
        identity = "synthetic:generated-raster-v1"
        cache_mode = "synthetic-cache-write" if args.mode == "write" else "synthetic-no-cache"
    elif args.workload == "kernel":
        command.extend(
            [
                "--width",
                str(args.width),
                "--height",
                str(args.height),
                "--radius-km",
                str(args.radius_km),
                "--rows",
                str(args.rows),
                "--samples",
                str(args.samples),
            ]
        )
        parameters = {
            "width": args.width,
            "height": args.height,
            "radius_km": args.radius_km,
            "rows": args.rows,
            "samples": args.samples,
        }
        identity = "synthetic:whole-globe-grid"
        cache_mode = "no-cache"
    else:
        command.extend(
            [
                "--radius-km",
                str(args.radius_km),
                "--centres",
                str(args.centres),
                "--samples",
                str(args.samples),
            ]
        )
        if args.cache is not None:
            command.extend(["--cache", str(repo_path(args.cache))])
            identity = cache_identity(args.cache)
            cache_mode = "mapped"
        else:
            identity = "synthetic:resident-ramp-v1"
            cache_mode = "resident"
        parameters = {
            "radius_km": args.radius_km,
            "centres": args.centres,
            "cache": str(args.cache) if args.cache else None,
            "samples": args.samples,
        }
    measured = run_captured(command)
    detail = json_object(json.loads(str(measured["stdout"])))
    raw = json_samples(detail.get("samples_seconds"))
    return record(
        args.workload,
        parameters,
        identity,
        raw,
        json_number(measured["wall_time_seconds"], "wall time"),
        json_number(measured["cpu_time_seconds"], "CPU time"),
        measured["peak_rss_bytes"] if isinstance(measured["peak_rss_bytes"], int) else None,
        "release",
        1,
        cache_mode,
    )


def search_record(args: argparse.Namespace) -> dict[str, object]:
    binary = build_cli()
    raw: list[float] = []
    cpu_time = 0.0
    peak_rss: int | None = None
    parameters: dict[str, object]
    if args.workload == "world-search":
        identity = cache_identity(args.cache)
        parameters = {
            "dataset": args.dataset,
            "cache": str(args.cache),
            "radius_km": args.radius_km,
            "spacing": args.spacing,
            "workers": args.workers,
            "samples": args.samples,
        }
    else:
        identity = country_identity(args.combination, args.resolution)
        parameters = {
            "combination": args.combination,
            "resolution": args.resolution,
            "country": args.country,
            "share": args.share,
            "spacing": args.spacing,
            "workers": args.workers,
            "samples": args.samples,
        }
    with tempfile.TemporaryDirectory(prefix="popcircles-bench-") as temporary:
        for index in range(args.samples):
            if args.workload == "world-search":
                command = [
                    str(binary),
                    "--log-level",
                    "error",
                    "most-populous",
                    "--dataset",
                    args.dataset,
                    "--cache",
                    str(repo_path(args.cache)),
                    "--radius-km",
                    str(args.radius_km),
                    "--spacing",
                    str(args.spacing),
                ]
            else:
                command = [
                    str(binary),
                    "--log-level",
                    "error",
                    "country",
                    "smallest",
                    "--combination",
                    args.combination,
                    "--resolution",
                    str(args.resolution),
                    "--country",
                    args.country,
                    "--share",
                    str(args.share),
                    "--spacing",
                    str(args.spacing),
                    "--ledger",
                    str(Path(temporary) / f"ledger-{index}.json"),
                ]
            measured = run_captured(command, workers=args.workers)
            json_object(json.loads(str(measured["stdout"])))
            raw.append(json_number(measured["wall_time_seconds"], "wall time"))
            cpu_time += json_number(measured["cpu_time_seconds"], "CPU time")
            peak_rss = max(peak_rss or 0, int(json_number(measured["peak_rss_bytes"], "peak RSS")))
    return record(
        args.workload,
        parameters,
        identity,
        raw,
        sum(raw),
        cpu_time,
        peak_rss,
        "release",
        args.workers,
        "mapped",
    )


def run(args: argparse.Namespace) -> dict[str, object]:
    if args.workload == "synthetic":
        return synthetic_record(args.iterations, args.samples)
    if args.workload in {"table-build", "kernel", "circle-evaluation"}:
        return component_record(args)
    return search_record(args)


def main() -> None:
    if len(sys.argv) > 1 and sys.argv[1] == "--capture":
        request = json_object(json.loads(sys.argv[2]))
        command = request.get("command")
        workers = request.get("workers")
        if not isinstance(command, list):
            raise SystemExit("benchmark worker command must be a list of strings")  # noqa: EM101, TRY003
        command_items = cast(list[object], command)
        if not command_items or not all(isinstance(item, str) for item in command_items):
            raise SystemExit("benchmark worker command must be a list of strings")  # noqa: EM101, TRY003
        command = cast(list[str], command_items)
        if (
            not isinstance(workers, int)
            or isinstance(workers, bool)
            or not 1 <= workers <= MAX_WORKERS
        ):
            raise SystemExit("benchmark worker count must be between 1 and 256")  # noqa: EM101, TRY003
        capture(command, workers)
        return
    if sys.argv[1:] == ["--smoke"]:
        smoke()
        return
    args = parser().parse_args()
    result = run(args)
    print(
        write_record(
            args.history_dir,
            result,
            require_clean=args.history_dir.resolve() == DEFAULT_HISTORY_DIR,
        )
    )


if __name__ == "__main__":
    main()
