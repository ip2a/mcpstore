from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from dataclasses import asdict, dataclass
from datetime import datetime
from pathlib import Path
from typing import Iterable
from zipfile import ZipFile


ROOT = Path(__file__).resolve().parents[2]
EXAMPLE_ROOT = ROOT / "python" / "example_local"
PYTHON_SRC = ROOT / "python" / "src"
LOCAL_EXTENSION = PYTHON_SRC / "mcpstore" / "_rust.abi3.so"
WHEEL_OUT = ROOT / "target" / "example_local_wheels"
REPORT_DIR = ROOT / "logs" / "example_local"

DEFAULT_EXAMPLES = [
    "quick_start.py",
    "quick_flow/basic_flow.py",
    "for_store/标准链路.py",
    "for_store/proxy_object/basic_flow_with_tool_proxy.py",
    "for_store/proxy_object/basic_flow_with_service_proxy.py",
    "for_store/proxy_object/basic_flow_with_agent_proxy.py",
]
EXPANDED_SKIP_PARTS = {
    "__pycache__",
    "api",
    "scripts",
}
EXPANDED_SKIP_MARKERS = (
    "redis",
    "langchain",
    "studio",
    "hub_",
    "only_db",
    "async",
    "pengyu",
    "临时",
    "auth",
    "market",
    "dataspace",
    "session",
)
EXPANDED_SKIP_FILES = {
    "example_utils.py",
}


@dataclass
class ExampleResult:
    example: str
    status: str
    category: str
    returncode: int
    duration_secs: float
    summary: str
    log_path: str


def repo_python() -> str:
    venv_python = ROOT / ".venv" / "bin" / "python"
    if venv_python.exists():
        return str(venv_python)
    return sys.executable


def run_command(
    args: list[str],
    *,
    cwd: Path | None = None,
    timeout: int | None = None,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        cwd=str(cwd or ROOT),
        text=True,
        capture_output=True,
        timeout=timeout,
        env=env,
        check=False,
    )


def refresh_pyo3_extension() -> Path:
    WHEEL_OUT.mkdir(parents=True, exist_ok=True)
    build = run_command(
        [
            "uv",
            "run",
            "--with",
            "maturin",
            "maturin",
            "build",
            "--manifest-path",
            "rust/bindings/python/Cargo.toml",
            "--out",
            str(WHEEL_OUT),
        ]
    )
    if build.returncode != 0:
        raise RuntimeError(build.stdout + "\n" + build.stderr)

    wheels = sorted(WHEEL_OUT.glob("mcpstore_python-*.whl"), key=lambda path: path.stat().st_mtime)
    if not wheels:
        raise RuntimeError(f"no mcpstore_python wheel found in {WHEEL_OUT}")
    wheel = wheels[-1]

    with tempfile.TemporaryDirectory(prefix="mcpstore-wheel-") as tmp:
        tmp_path = Path(tmp)
        with ZipFile(wheel) as archive:
            archive.extract("_rust/_rust.abi3.so", tmp_path)
        extracted = tmp_path / "_rust" / "_rust.abi3.so"
        LOCAL_EXTENSION.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(extracted, LOCAL_EXTENSION)
    return wheel


def classify_failure(output: str) -> tuple[str, str]:
    text = output.strip()
    lowered = text.lower()
    if "modulenotfounderror" in lowered or "no module named" in lowered:
        return "missing_dependency", first_error_line(text)
    if "certificate is expired" in lowered or "invalidcertificate" in lowered:
        return "external_tls", first_error_line(text)
    if "attributeerror:" in lowered or "keyerror:" in lowered or "typeerror:" in lowered:
        return "python_api_contract", first_error_line(text)
    if "connection refused" in lowered or "timed out" in lowered or "connecterror" in lowered:
        return "external_service", first_error_line(text)
    if "redis" in lowered and ("error" in lowered or "exception" in lowered):
        return "external_service", first_error_line(text)
    if "http://127.0.0.1:21923" in lowered and ("error" in lowered or "failed" in lowered):
        return "external_service", first_error_line(text)
    return "runtime_failure", first_error_line(text)


def first_error_line(text: str) -> str:
    for line in reversed(text.splitlines()):
        stripped = line.strip()
        if stripped:
            return stripped[:300]
    return ""


def text_value(value: str | bytes | None) -> str:
    if value is None:
        return ""
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    return value


def expanded_examples() -> list[Path]:
    paths: list[Path] = []
    for path in sorted(EXAMPLE_ROOT.rglob("*.py")):
        relative = path.relative_to(EXAMPLE_ROOT).as_posix()
        if any(part in EXPANDED_SKIP_PARTS for part in path.relative_to(EXAMPLE_ROOT).parts):
            continue
        if path.name in EXPANDED_SKIP_FILES:
            continue
        if any(marker in relative for marker in EXPANDED_SKIP_MARKERS):
            continue
        paths.append(path)
    return paths


def example_targets(selected: Iterable[str] | None, preset: str) -> list[Path]:
    if selected:
        return [EXAMPLE_ROOT / item for item in selected]
    if preset == "expanded":
        return expanded_examples()
    return [EXAMPLE_ROOT / item for item in DEFAULT_EXAMPLES]


def run_example(path: Path, *, timeout: int) -> ExampleResult:
    started = datetime.now()
    pythonpath_parts = [str(path.parent), str(EXAMPLE_ROOT), str(PYTHON_SRC)]
    existing_pythonpath = os.environ.get("PYTHONPATH")
    if existing_pythonpath:
        pythonpath_parts.append(existing_pythonpath)
    env = os.environ.copy()
    env["PYTHONPATH"] = os.pathsep.join(pythonpath_parts)
    relative = path.relative_to(EXAMPLE_ROOT).as_posix()
    print(f"[running] {relative}", flush=True)
    try:
        proc = run_command([repo_python(), str(path)], timeout=timeout, env=env)
        timed_out = False
    except subprocess.TimeoutExpired as error:
        proc = subprocess.CompletedProcess(
            args=error.cmd,
            returncode=124,
            stdout=text_value(error.stdout),
            stderr=text_value(error.stderr) + f"\nTimed out after {timeout} seconds",
        )
        timed_out = True
    duration = (datetime.now() - started).total_seconds()

    log_name = relative.replace("/", "__") + ".log"
    log_path = REPORT_DIR / log_name
    log_path.parent.mkdir(parents=True, exist_ok=True)
    output = text_value(proc.stdout)
    stderr = text_value(proc.stderr)
    if stderr:
        output = output + ("\n" if output else "") + stderr
    log_path.write_text(output, encoding="utf-8")

    if proc.returncode == 0:
        result = ExampleResult(
            example=relative,
            status="passed",
            category="ok",
            returncode=0,
            duration_secs=duration,
            summary="passed",
            log_path=str(log_path.relative_to(ROOT)),
        )
        print(f"[passed] {relative} duration={duration:.2f}s", flush=True)
        return result

    if timed_out:
        result = ExampleResult(
            example=relative,
            status="failed",
            category="timeout",
            returncode=proc.returncode,
            duration_secs=duration,
            summary=f"Timed out after {timeout} seconds",
            log_path=str(log_path.relative_to(ROOT)),
        )
        print(f"[failed] {relative} category=timeout duration={duration:.2f}s", flush=True)
        return result

    category, summary = classify_failure(output)
    result = ExampleResult(
        example=relative,
        status="failed",
        category=category,
        returncode=proc.returncode,
        duration_secs=duration,
        summary=summary,
        log_path=str(log_path.relative_to(ROOT)),
    )
    print(f"[failed] {relative} category={category} duration={duration:.2f}s summary={summary}", flush=True)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description="Refresh the local PyO3 extension and run example_local contract smoke checks.")
    parser.add_argument(
        "--example",
        action="append",
        dest="examples",
        help="Relative path under python/example_local. Repeatable.",
    )
    parser.add_argument(
        "--preset",
        choices=("core", "expanded"),
        default="core",
        help="Example selection preset when --example is not provided.",
    )
    parser.add_argument("--timeout", type=int, default=90, help="Per-example timeout in seconds.")
    args = parser.parse_args()

    REPORT_DIR.mkdir(parents=True, exist_ok=True)
    wheel = refresh_pyo3_extension()
    targets = example_targets(args.examples, args.preset)

    results = [run_example(path, timeout=args.timeout) for path in targets]
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    report_path = REPORT_DIR / f"contract_report_{timestamp}.json"
    payload = {
        "generated_at": timestamp,
        "wheel": str(wheel.relative_to(ROOT)),
        "extension": str(LOCAL_EXTENSION.relative_to(ROOT)),
        "python": repo_python(),
        "preset": args.preset,
        "results": [asdict(item) for item in results],
    }
    report_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")

    print(f"wheel: {payload['wheel']}")
    print(f"extension: {payload['extension']}")
    print(f"report: {report_path.relative_to(ROOT)}")
    for item in results:
        print(
            f"[{item.status}] {item.example} "
            f"category={item.category} rc={item.returncode} "
            f"summary={item.summary}"
        )

    return 0 if all(item.status == "passed" for item in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
