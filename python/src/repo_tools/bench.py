import argparse
import hashlib
import json
import os
import platform
import resource
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
import tomllib
import uuid
from collections.abc import Mapping, Sequence
from datetime import UTC, datetime
from pathlib import Path
from typing import cast

REPO_ROOT = Path(
    os.environ.get("POPCIRCLES_BENCH_ROOT", Path(__file__).resolve().parents[3])
).resolve()
DEFAULT_HISTORY_DIR = REPO_ROOT / "benchmarks/history"
SAMPLE_COUNT = 3
MAX_WORKERS = 256
MAX_SHARE_PERCENT = 100
SMOKE_ITERATIONS = 100
COMPARISON_SMOKE_WORKERS = 4
COMPATIBILITY_FIELDS = (
    "workload",
    "workload_version",
    "parameters",
    "input_identity",
    "cpu",
    "os",
    "rust_toolchain",
    "build_profile",
    "cache_mode",
)
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
        "schema_version": 2,
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
        comparison_smoke()
        print("synthetic benchmark smoke passed")


def compatibility_mismatches(
    before: Mapping[str, object], after: Mapping[str, object]
) -> list[dict[str, object]]:
    mismatches: list[dict[str, object]] = []
    for field in COMPATIBILITY_FIELDS:
        left = before.get(field)
        right = after.get(field)
        if (
            left != right
            or left is None
            or right is None
            or (field == "cpu" and (left == "unavailable" or right == "unavailable"))
        ):
            mismatches.append({"field": field, "before": left, "after": right})
    return mismatches


def comparison_summary(
    before: list[float], after: list[float], orders: list[list[str]]
) -> dict[str, object]:
    if len(before) != SAMPLE_COUNT or len(after) != SAMPLE_COUNT or len(orders) != SAMPLE_COUNT:
        raise ValueError("comparison requires exactly three paired samples")  # noqa: EM101, TRY003
    pairs = [
        {"order": order, "before_seconds": left, "after_seconds": right}
        for order, left, right in zip(orders, before, after, strict=True)
    ]
    return {
        "pairs": pairs,
        "medians_seconds": {
            "before": statistics.median(before),
            "after": statistics.median(after),
        },
    }


def paired_result(
    before: list[float],
    after: list[float],
    orders: list[list[str]],
    mismatches: list[dict[str, object]],
) -> dict[str, object]:
    result = comparison_summary(before, after, orders)
    before_median = statistics.median(before)
    after_median = statistics.median(after)
    result["medians_seconds"] = (
        None if mismatches else {"before": before_median, "after": after_median}
    )
    result["delta_seconds"] = None if mismatches else after_median - before_median
    result["delta_percent"] = (
        (after_median / before_median - 1) * 100 if not mismatches and before_median else None
    )
    return result


def country_answers_equal(
    before: Sequence[Mapping[str, object]], after: Sequence[Mapping[str, object]]
) -> bool:
    before_signature = before[0].get("answer_signature") if before else None
    after_signature = after[0].get("answer_signature") if after else None
    return (
        before_signature is not None
        and before_signature == after_signature
        and all(item.get("answer_signature") == before_signature for item in before)
        and all(item.get("answer_signature") == after_signature for item in after)
    )


def answer_signature_smoke() -> None:
    signature = {"row": 7, "col": 11, "population_bits": float(42).hex()}
    before = [{"answer_signature": signature}]
    after = [{"answer_signature": signature}]
    if not country_answers_equal(before, after):
        raise ValueError("matching answer signatures were rejected")  # noqa: EM101, TRY003
    mismatch = {"row": 8, "col": 11, "population_bits": float(42).hex()}
    if country_answers_equal(before, [{"answer_signature": mismatch}]):
        raise ValueError("mismatched answer signatures were accepted")  # noqa: EM101, TRY003
    if country_answers_equal([{}], [{}]):
        raise ValueError("missing answer signatures were accepted")  # noqa: EM101, TRY003


def comparison_smoke() -> None:
    fields: dict[str, object] = {
        "workload": "synthetic",
        "workload_version": 1,
        "parameters": {"iterations_per_sample": 100, "samples": 1},
        "input_identity": "synthetic:v1",
        "cpu": "fixture CPU",
        "os": "fixture OS",
        "rust_toolchain": "fixture rustc",
        "build_profile": "synthetic-python",
        "cache_mode": "synthetic-no-cache",
    }
    before = {**fields, "commit": "a" * 40, "worker_count": 1}
    after = {**fields, "commit": "b" * 40, "worker_count": 4}
    orders = [["before", "after"], ["after", "before"], ["before", "after"]]
    summary = comparison_summary([3.0, 2.0, 4.0], [6.0, 4.0, 8.0], orders)
    expected_pairs = [
        {"order": order, "before_seconds": left, "after_seconds": right}
        for order, left, right in zip(orders, [3.0, 2.0, 4.0], [6.0, 4.0, 8.0], strict=True)
    ]
    if summary["pairs"] != expected_pairs or summary["medians_seconds"] != {
        "before": 3.0,
        "after": 6.0,
    }:
        raise ValueError("synthetic comparison fixture lost pair order, samples, or medians")  # noqa: EM101, TRY003
    if compatibility_mismatches(before, after):
        raise ValueError("commit identity must not prevent a compatible comparison")  # noqa: EM101, TRY003
    answer_signature_smoke()
    if before.get("worker_count") != 1 or after.get("worker_count") != COMPARISON_SMOKE_WORKERS:
        raise ValueError("worker counts must not prevent a compatible comparison")  # noqa: EM101, TRY003
    compatible = paired_result([3.0, 2.0, 4.0], [6.0, 4.0, 8.0], orders, [])
    if compatible["delta_seconds"] != 3.0:  # noqa: PLR2004 — asserted synthetic fixture delta
        raise ValueError("compatible fixture did not produce a numeric delta")  # noqa: EM101, TRY003
    for field in COMPATIBILITY_FIELDS:
        incompatible = {**after, field: object()}
        mismatches = compatibility_mismatches(before, incompatible)
        if [item["field"] for item in mismatches] != [field]:
            raise ValueError(f"comparison fixture did not identify the `{field}` mismatch")  # noqa: EM102, TRY003
        result = paired_result([3.0, 2.0, 4.0], [6.0, 4.0, 8.0], orders, mismatches)
        if result["medians_seconds"] is not None or result["delta_seconds"] is not None:
            raise ValueError(f"`{field}` mismatch did not suppress the numeric comparison")  # noqa: EM102, TRY003
    unavailable = {**after, "cpu": "unavailable"}
    if [item["field"] for item in compatibility_mismatches(before, unavailable)] != ["cpu"]:
        raise ValueError("comparison fixture accepted an unavailable CPU identity")  # noqa: EM101, TRY003


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

    compare = workloads.add_parser("compare")
    compare.add_argument("--before", required=True)
    compare.add_argument("--after", required=True)
    compare.add_argument("--before-workers", type=worker_count, required=True)
    compare.add_argument("--after-workers", type=worker_count, required=True)
    compare.add_argument(
        "target_workload",
        choices=[
            "synthetic",
            "table-build",
            "kernel",
            "circle-evaluation",
            "world-search",
            "country-search",
        ],
    )
    compare.add_argument("workload_args", nargs=argparse.REMAINDER)
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
        "bench",
        1,
        cache_mode,
    )


def search_record(args: argparse.Namespace) -> dict[str, object]:
    binary = build_cli()
    raw: list[float] = []
    answers: list[dict[str, object]] = []
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
            answer = json_object(json.loads(str(measured["stdout"])))
            if args.workload == "country-search":
                answer = json_object(answer.get("result"))
                circle = json_object(answer.get("circle"))
                row, col, population = (
                    circle.get("row"),
                    circle.get("col"),
                    circle.get("population"),
                )
                if (
                    not isinstance(row, int)
                    or isinstance(row, bool)
                    or not isinstance(col, int)
                    or isinstance(col, bool)
                    or not isinstance(population, int | float)
                    or isinstance(population, bool)
                ):
                    raise TypeError("country search returned an invalid answer signature")  # noqa: EM101, TRY003
                answers.append(
                    {
                        "row": row,
                        "col": col,
                        "population_bits": float(population).hex(),
                    }
                )
            raw.append(json_number(measured["wall_time_seconds"], "wall time"))
            cpu_time += json_number(measured["cpu_time_seconds"], "CPU time")
            peak_rss = max(peak_rss or 0, int(json_number(measured["peak_rss_bytes"], "peak RSS")))
    result = record(
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
    if args.workload == "country-search":
        if len({json.dumps(answer, sort_keys=True) for answer in answers}) != 1:
            raise ValueError("country search answer changed between benchmark runs")  # noqa: EM101, TRY003
        result["answer_signature"] = answers[0]
    return result


def run(args: argparse.Namespace) -> dict[str, object]:
    if args.workload == "synthetic":
        return synthetic_record(args.iterations, args.samples)
    if args.workload in {"table-build", "kernel", "circle-evaluation"}:
        return component_record(args)
    return search_record(args)


def workload_argv(
    args: argparse.Namespace, history: Path, samples: int, workers: int | None = None
) -> list[str]:
    arguments = [args.workload]
    for name, original in vars(args).items():
        if name in {"workload", "history_dir", "samples"} or original is None:
            continue
        argument = workers if name == "workers" and workers is not None else original
        value = repo_path(argument) if isinstance(argument, Path) else argument
        arguments.extend([f"--{name.replace('_', '-')}", str(value)])
    arguments.extend(["--samples", str(samples), "--history-dir", str(history)])
    return arguments


def resolve_commit(revision: str) -> str:
    return command_output(
        "git", "rev-parse", "--verify", "--end-of-options", f"{revision}^{{commit}}"
    ).strip()


def run_revision(
    worktree: Path,
    commit: str,
    args: argparse.Namespace,
    history: Path,
    workers: int | None = None,
) -> tuple[dict[str, object], Path]:
    entrypoint = Path(__file__).resolve()
    result = subprocess.run(  # noqa: S603 — fixed Python entrypoint and parsed workload argv
        [sys.executable, str(entrypoint), *workload_argv(args, history, 1, workers)],
        cwd=worktree,
        env={**os.environ, "POPCIRCLES_BENCH_ROOT": str(worktree)},
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode:
        detail = result.stderr.strip() or result.stdout.strip()
        message = f"benchmark on {commit[:12]} failed: {detail}"
        raise RuntimeError(message)
    lines = result.stdout.strip().splitlines()
    if not lines:
        message = f"benchmark on {commit[:12]} returned no record path"
        raise ValueError(message)
    path = Path(lines[-1])
    if not path.is_absolute():
        path = worktree / path
    measured = json_object(json.loads(path.read_text(encoding="utf-8")))
    if measured.get("commit") != commit:
        message = f"benchmark record does not identify revision {commit[:12]}"
        raise ValueError(message)
    if len(json_samples(measured.get("raw_samples_seconds"))) != 1:
        raise ValueError("paired comparison expects one raw sample per invocation")  # noqa: EM101, TRY003
    return measured, path


def share_inputs(worktree: Path, workload: argparse.Namespace) -> None:
    with (worktree / "data/registry.toml").open("rb") as registry_file:
        registry = tomllib.load(registry_file)["datasets"]
    if workload.workload == "world-search":
        names = [workload.dataset]
    elif workload.workload == "country-search":
        with (worktree / "data/config.toml").open("rb") as config_file:
            selection = tomllib.load(config_file)["combinations"][workload.combination]
        resolution = selection["resolutions"][str(workload.resolution)]
        link_input(worktree, resolution["local"])
        names = [selection["population_source"], selection["boundary_source"]]
    else:
        return
    names.append("populated-places-1to10m")
    for name in names:
        dataset = registry.get(name.removeprefix("dataset:"))
        if dataset is not None:
            link_input(worktree, dataset["path"])


def link_input(worktree: Path, relative: str) -> None:
    path = Path(relative)
    if path.is_absolute() or ".." in path.parts:
        message = f"input path escapes the worktree: {relative}"
        raise ValueError(message)
    source = REPO_ROOT / path
    target = worktree / path
    if source.exists() and not target.exists():
        target.parent.mkdir(parents=True, exist_ok=True)
        target.symlink_to(source.resolve(), target_is_directory=source.is_dir())


def compare(args: argparse.Namespace, workload: argparse.Namespace) -> Path:  # noqa: PLR0915 — measured pass order is explicit
    before_commit = resolve_commit(args.before)
    after_commit = resolve_commit(args.after)
    if before_commit == after_commit:
        raise ValueError("comparison revisions must be different commits")  # noqa: EM101, TRY003
    history = repo_path(workload.history_dir)
    if history.resolve() == DEFAULT_HISTORY_DIR and command_output(
        "git",
        "status",
        "--porcelain",
        "--untracked-files=normal",
        "--",
        ".",
        ":(exclude)benchmarks/history/**",
    ):
        raise ValueError("commit the benchmark source before writing a comparison")  # noqa: EM101, TRY003

    before_samples: list[dict[str, object]] = []
    after_samples: list[dict[str, object]] = []
    before_paths: list[str] = []
    after_paths: list[str] = []
    orders = [["before", "after"], ["after", "before"], ["before", "after"]]
    with tempfile.TemporaryDirectory(prefix="popcircles-compare-") as temporary:
        root = Path(temporary)
        before_tree = root / "before"
        after_tree = root / "after"
        command_output(
            "git", "worktree", "add", "--detach", "--quiet", str(before_tree), before_commit
        )
        try:
            command_output(
                "git", "worktree", "add", "--detach", "--quiet", str(after_tree), after_commit
            )
            try:
                share_inputs(before_tree, workload)
                share_inputs(after_tree, workload)
                first_before, path = run_revision(
                    before_tree, before_commit, workload, history, args.before_workers
                )
                before_pass = first_before
                before_paths.append(path.name)
                first_after, path = run_revision(
                    after_tree, after_commit, workload, history, args.after_workers
                )
                after_pass = first_after
                after_paths.append(path.name)
                with tempfile.TemporaryDirectory(prefix="popcircles-warmup-") as warmup:
                    warm_history = Path(warmup)
                    run_revision(
                        before_tree, before_commit, workload, warm_history, args.before_workers
                    )
                    run_revision(
                        after_tree, after_commit, workload, warm_history, args.after_workers
                    )
                for order in orders:
                    for side in order:
                        tree, commit = (
                            (before_tree, before_commit)
                            if side == "before"
                            else (after_tree, after_commit)
                        )
                        measured, path = run_revision(
                            tree,
                            commit,
                            workload,
                            history,
                            args.before_workers if side == "before" else args.after_workers,
                        )
                        if side == "before":
                            before_samples.append(measured)
                            before_paths.append(path.name)
                        else:
                            after_samples.append(measured)
                            after_paths.append(path.name)
            finally:
                if after_tree.exists():
                    command_output("git", "worktree", "remove", "--force", str(after_tree))
        finally:
            command_output("git", "worktree", "remove", "--force", str(before_tree))

    mismatches = compatibility_mismatches(before_pass, after_pass)
    before_answer = before_pass.get("answer_signature")
    after_answer = after_pass.get("answer_signature")
    answer_equal = (
        country_answers_equal([before_pass, *before_samples], [after_pass, *after_samples])
        if workload.workload == "country-search"
        else None
    )
    before_values = [json_samples(item["raw_samples_seconds"])[0] for item in before_samples]
    after_values = [json_samples(item["raw_samples_seconds"])[0] for item in after_samples]
    first_pass = {
        "label": "first pass",
        "before_seconds": json_samples(before_pass["raw_samples_seconds"])[0],
        "after_seconds": json_samples(after_pass["raw_samples_seconds"])[0],
    }
    paired = paired_result(before_values, after_values, orders, mismatches)
    output: dict[str, object] = {
        "schema_version": 2,
        "workload": workload.workload,
        "before_commit": before_commit,
        "after_commit": after_commit,
        "parameters": before_pass.get("parameters"),
        "compatibility_mismatches": mismatches,
        "answer_signatures": {"before": before_answer, "after": after_answer},
        "answers_equal": answer_equal,
        "first_pass": first_pass,
        "warmup": "one untimed invocation per revision",
        "paired_samples": paired["pairs"],
        "records": {"before": before_paths, "after": after_paths},
        "medians_seconds": paired["medians_seconds"],
        "delta_seconds": paired["delta_seconds"],
        "delta_percent": paired["delta_percent"],
    }
    history.mkdir(parents=True, exist_ok=True)
    timestamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%S.%fZ")
    result = history / f"{timestamp}-{uuid.uuid4().hex[:8]}-comparison.json"
    with result.open("x", encoding="utf-8") as target:
        target.write(json.dumps(output, indent=2, sort_keys=True) + "\n")
    return result


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
    if args.workload == "compare":
        workload_args = parser().parse_args([args.target_workload, *args.workload_args])
        result_path = compare(args, workload_args)
        print(result_path)
        if json.loads(result_path.read_text(encoding="utf-8")).get("answers_equal") is False:
            message = "country search answer signatures differ"
            raise SystemExit(message)
        return
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
