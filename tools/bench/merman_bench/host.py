"""Host provenance and Criterion receipt helpers."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any
import json
import os
import platform
import re
import subprocess
import sys


DEFAULT_COMMAND_TIMEOUT_SECONDS = 30 * 60


DEFAULT_METADATA_TIMEOUT_SECONDS = 30


_ANSI_RE = re.compile(r"\x1b\[[0-9;?]*[A-Za-z]")


def strip_ansi(text: str) -> str:
    return _ANSI_RE.sub("", text)


@dataclass(frozen=True)
class TimeEstimate:
    value: float
    unit: str

    def to_nanos(self) -> float:
        u = strip_ansi(self.unit).strip()
        u = u.replace("μ", "µ")
        if u == "ns":
            return self.value
        if u in ("us", "µs", "μs"):
            return self.value * 1e3
        if u == "ms":
            return self.value * 1e6
        if u == "s":
            return self.value * 1e9
        raise ValueError(f"unknown time unit: {self.unit!r}")


def pretty_time(nanos: float) -> str:
    if nanos < 1e3:
        return f"{nanos:.2f} ns"
    if nanos < 1e6:
        return f"{nanos / 1e3:.2f} µs"
    if nanos < 1e9:
        return f"{nanos / 1e6:.2f} ms"
    return f"{nanos / 1e9:.2f} s"


def run(
    cmd: list[str],
    cwd: Path,
    *,
    env: dict[str, str] | None = None,
    timeout_seconds: int = DEFAULT_COMMAND_TIMEOUT_SECONDS,
    merge_stderr: bool = True,
) -> str:
    proc_env = os.environ.copy()
    if env:
        proc_env.update(env)
    try:
        proc = subprocess.run(
            cmd,
            cwd=str(cwd),
            env=proc_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT if merge_stderr else subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace",
            check=False,
            timeout=timeout_seconds,
        )
    except subprocess.TimeoutExpired as error:
        output = error.stdout or ""
        raise RuntimeError(
            f"command timed out after {timeout_seconds}s in {cwd}\n"
            f"$ {' '.join(cmd)}\n\n{output}"
        ) from error
    if proc.returncode != 0:
        raise RuntimeError(
            f"command failed (exit {proc.returncode}) in {cwd}\n"
            f"$ {' '.join(cmd)}\n\n{proc.stdout}{proc.stderr or ''}"
        )
    return proc.stdout


_SKIP_LINE = re.compile(
    r"^\[bench\]\[skip\]\[(?P<group>[A-Za-z0-9_\-]+)\]\s+"
    r"(?P<name>[A-Za-z0-9_\-]+):\s*(?P<reason>.+)$"
)


def parse_skip_lines(text: str) -> dict[str, list[str]]:
    skipped: dict[str, list[str]] = {}
    for raw in text.splitlines():
        line = strip_ansi(raw.rstrip("\r\n"))
        m = _SKIP_LINE.match(line)
        if not m:
            continue
        group = m.group("group")
        name = m.group("name")
        skipped.setdefault(group, []).append(name)
    for k in list(skipped.keys()):
        skipped[k] = sorted(set(skipped[k]))
    return skipped


def git_head(cwd: Path) -> str | None:
    try:
        out = run(["git", "rev-parse", "HEAD"], cwd=cwd).strip()
        return out if out else None
    except Exception:
        return None


def rustc_verbose(*, toolchain: str | None = None, cwd: Path | None = None) -> str:
    try:
        command = ["rustc"]
        if toolchain:
            command.append(f"+{toolchain}")
        command.append("-Vv")
        out = subprocess.run(
            command,
            cwd=str(cwd) if cwd is not None else None,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            check=False,
            timeout=DEFAULT_METADATA_TIMEOUT_SECONDS,
        ).stdout.strip()
        return out
    except Exception:
        return "unknown"


def best_effort_cpu_model() -> str:
    try:
        if sys.platform.startswith("win"):
            out = subprocess.run(
                [
                    "powershell",
                    "-NoProfile",
                    "-Command",
                    "(Get-CimInstance Win32_Processor | Select-Object -First 1 -ExpandProperty Name)",
                ],
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                encoding="utf-8",
                errors="replace",
                check=False,
                timeout=DEFAULT_METADATA_TIMEOUT_SECONDS,
            ).stdout.strip()
            if out:
                return out
        elif sys.platform == "darwin":
            out = subprocess.run(
                ["sysctl", "-n", "machdep.cpu.brand_string"],
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                encoding="utf-8",
                errors="replace",
                check=False,
                timeout=DEFAULT_METADATA_TIMEOUT_SECONDS,
            ).stdout.strip()
            if out:
                return out
        else:
            out = subprocess.run(
                ["lscpu"],
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                encoding="utf-8",
                errors="replace",
                check=False,
                timeout=DEFAULT_METADATA_TIMEOUT_SECONDS,
            ).stdout
            for line in out.splitlines():
                if ":" not in line:
                    continue
                k, v = line.split(":", 1)
                if k.strip().lower() == "model name" and v.strip():
                    return v.strip()
    except Exception:
        pass
    return platform.processor() or "unknown"


def expand_filter_to_exact_benches(filter_expr: str) -> list[str]:
    """
    Expand a limited, common "group/(a|b|c)" filter form into exact benchmark names.

    Criterion <=0.5 treats the positional filter argument as a regex, while Criterion >=0.8 treats
    it as a substring match. `mermaid-rs-renderer` currently uses Criterion >=0.8, so a regex-style
    filter like "end_to_end/(a|b)" would match nothing there.
    """
    text = filter_expr.strip()
    m = re.fullmatch(r"(?P<prefix>[A-Za-z0-9_-]+)/\((?P<body>[^)]+)\)", text)
    if not m:
        return [text]

    prefix = m.group("prefix")
    alts = [p.strip() for p in m.group("body").split("|") if p.strip()]
    out: list[str] = []
    for name in alts:
        if not re.fullmatch(r"[A-Za-z0-9_-]+", name):
            return [text]
        out.append(f"{prefix}/{name}")
    return out or [text]


_LINE_NAME_ONLY = re.compile(r"^(?P<prefix>[A-Za-z0-9_\-]+)/(?P<name>[A-Za-z0-9_\-]+)\s*$")


_LINE_TIME_ONLY = re.compile(r"^\s*time:\s*\[(?P<body>.+?)\]\s*$")


_LINE_INLINE = re.compile(
    r"^(?P<prefix>[A-Za-z0-9_\-]+)/(?P<name>[A-Za-z0-9_\-]+)\s+"
    r"time:\s*\[(?P<body>.+?)\]\s*$"
)


def parse_criterion_times(text: str, prefix: str) -> dict[str, TimeEstimate]:
    """Parse Criterion output and return mid estimates by benchmark name."""
    times: dict[str, TimeEstimate] = {}
    cur: str | None = None

    for raw in text.splitlines():
        line = strip_ansi(raw.rstrip("\r\n"))

        m_inline = _LINE_INLINE.match(line)
        if m_inline and m_inline.group("prefix") == prefix:
            name = m_inline.group("name")
            estimate = _parse_bracket_time(m_inline.group("body"))
            if estimate is not None:
                times[name] = estimate
            cur = None
            continue

        m_name = _LINE_NAME_ONLY.match(line)
        if m_name and m_name.group("prefix") == prefix:
            cur = m_name.group("name")
            continue

        if cur is not None:
            m_time = _LINE_TIME_ONLY.match(line)
            if m_time:
                estimate = _parse_bracket_time(m_time.group("body"))
                if estimate is not None:
                    times[cur] = estimate
                cur = None

    return times


def _parse_bracket_time(body: str) -> TimeEstimate | None:
    # Criterion prints: "<lo> <unit> <mid> <unit> <hi> <unit>".
    body = strip_ansi(body).strip()
    pairs = re.findall(r"([0-9]+(?:\.[0-9]+)?)\s*([A-Za-zµμ]+)", body)
    if len(pairs) < 2:
        return None
    mid_value, mid_unit = pairs[1]
    try:
        return TimeEstimate(float(mid_value), mid_unit)
    except ValueError:
        return None


def split_exact_bench(exact: str) -> tuple[str, str]:
    if "/" not in exact:
        return "", exact
    return exact.split("/", 1)


def write_json_report(path: Path, report: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
