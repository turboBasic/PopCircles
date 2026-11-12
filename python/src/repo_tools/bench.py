"""Write immutable benchmark history records."""

from __future__ import annotations

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
import uuid
from datetime import UTC, datetime
from pathlib import Path

SAMPLE_COUNT = 3
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
        [executable(name), *arguments], text=True, stderr=stderr
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
    if platform.processor():
        return platform.processor()
    try:
        if sys.platform == "darwin":
            return command_output(
                "sysctl", "-n", "machdep.cpu.brand_string", stderr=subprocess.DEVNULL
            ).strip()
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.lower().startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError, subprocess.CalledProcessError:
        pass
    return platform.machine()


def run_synthetic(iterations: int) -> tuple[list[float], float, float]:
    samples: list[float] = []
    started = time.perf_counter()
    cpu_started = time.process_time()
    value = b"popcircles synthetic benchmark v1"
    for _ in range(SAMPLE_COUNT):
        sample_started = time.perf_counter()
        for _ in range(iterations):
            value = hashlib.sha256(value).digest()
        samples.append(time.perf_counter() - sample_started)
    _ = value
    return samples, time.perf_counter() - started, time.process_time() - cpu_started


def make_record(iterations: int) -> dict[str, object]:
    rust_toolchain = command_output("rustc", "-Vv").splitlines()[0]
    samples, wall_time, cpu_time = run_synthetic(iterations)
    return {
        "schema_version": 1,
        "commit": command_output("git", "rev-parse", "HEAD").strip(),
        "workload": "synthetic",
        "workload_version": 1,
        "parameters": {"iterations_per_sample": iterations, "samples": len(samples)},
        "input_identity": "synthetic:sha256-chain-v1",
        "cpu": cpu_name(),
        "os": f"{platform.system()} {platform.release()} ({platform.machine()})",
        "host_memory_bytes": host_memory_bytes(),
        "rust_toolchain": rust_toolchain,
        "build_profile": "release",
        "worker_count": 1,
        "cache_mode": "synthetic-no-cache",
        "raw_samples_seconds": samples,
        "wall_time_seconds": wall_time,
        "cpu_time_seconds": cpu_time,
        "peak_rss_bytes": peak_rss_bytes(),
    }


def write_record(history_dir: Path, iterations: int, *, require_clean: bool = True) -> Path:
    if require_clean and command_output("git", "status", "--porcelain", "--untracked-files=normal"):
        print("commit the benchmark source before writing a history record", file=sys.stderr)
        raise SystemExit(1)
    history_dir.mkdir(parents=True, exist_ok=True)
    timestamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%S.%fZ")
    path = history_dir / f"{timestamp}-{uuid.uuid4().hex[:8]}-synthetic.json"
    payload = json.dumps(make_record(iterations), indent=2, sort_keys=True) + "\n"
    with path.open("x", encoding="utf-8") as record:
        record.write(payload)
    return path


def smoke() -> None:
    with tempfile.TemporaryDirectory(prefix="popcircles-bench-") as temporary:
        history = Path(temporary)
        first = write_record(history, 100, require_clean=False)
        before = first.read_bytes()
        second = write_record(history, 100, require_clean=False)
        if first == second or first.read_bytes() != before:
            print("benchmark history path was reused or overwritten", file=sys.stderr)
            raise SystemExit(1)
        for path in (first, second):
            record = json.loads(path.read_text())
            if record.keys() < REQUIRED or len(record["raw_samples_seconds"]) != SAMPLE_COUNT:
                print("benchmark smoke record is incomplete", file=sys.stderr)
                raise SystemExit(1)
        print("synthetic benchmark smoke passed")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workload", choices=["synthetic"], default="synthetic")
    parser.add_argument("--history-dir", type=Path, default=Path("benchmarks/history"))
    parser.add_argument("--iterations", type=int, default=10_000)
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    if args.iterations <= 0:
        parser.error("--iterations must be positive")
    if args.smoke:
        smoke()
    else:
        repository_history = Path("benchmarks/history").resolve()
        print(
            write_record(
                args.history_dir,
                args.iterations,
                require_clean=args.history_dir.resolve() == repository_history,
            )
        )


if __name__ == "__main__":
    main()
