#!/usr/bin/env python3
"""
Compare `merman` and `mermaid-rs-renderer` Criterion benchmarks.

This script is intended for local regression tracking. It runs a filtered subset of
end-to-end benchmarks in both repos, parses Criterion's summary, and writes a Markdown
report with mid-point estimates and ratios.

Requirements:
- Python 3.10+
- `cargo` available on PATH
- Local checkout of `repo-ref/mermaid-rs-renderer` (not a git submodule)
"""

from __future__ import annotations

import argparse
import datetime as _dt
import os
import platform
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


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


def run(cmd: list[str], cwd: Path) -> str:
    proc = subprocess.run(
        cmd,
        cwd=str(cwd),
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    if proc.returncode != 0:
        raise RuntimeError(
            f"command failed (exit {proc.returncode}) in {cwd}\n"
            f"$ {' '.join(cmd)}\n\n{proc.stdout}"
        )
    return proc.stdout


def git_head(cwd: Path) -> str | None:
    try:
        out = run(["git", "rev-parse", "HEAD"], cwd=cwd).strip()
        return out if out else None
    except Exception:
        return None


def rustc_verbose() -> str:
    try:
        out = subprocess.run(
            ["rustc", "-Vv"],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            check=False,
        ).stdout.strip()
        return out
    except Exception:
        return "unknown"


_LINE_NAME_ONLY = re.compile(r"^(?P<prefix>[A-Za-z0-9_\-]+)/(?P<name>[A-Za-z0-9_\-]+)\s*$")
_LINE_TIME_ONLY = re.compile(r"^\s*time:\s*\[(?P<body>.+?)\]\s*$")
_LINE_INLINE = re.compile(
    r"^(?P<prefix>[A-Za-z0-9_\-]+)/(?P<name>[A-Za-z0-9_\-]+)\s+time:\s*\[(?P<body>.+?)\]\s*$"
)


def parse_criterion_times(text: str, prefix: str) -> dict[str, TimeEstimate]:
    """
    Parse Criterion output and return mid estimates by benchmark name.
    """
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
    # Be tolerant of formatting (unicode micro sign, ANSI escapes, varying whitespace).
    body = strip_ansi(body).strip()
    pairs = re.findall(r"([0-9]+(?:\.[0-9]+)?)\s*([A-Za-zµμ]+)", body)
    if len(pairs) < 2:
        return None
    mid_value, mid_unit = pairs[1]
    try:
        return TimeEstimate(float(mid_value), mid_unit)
    except ValueError:
        return None


def write_markdown(
    out_path: Path,
    *,
    filter_expr: str,
    sample_size: int,
    warm_up: int,
    measurement: int,
    env_lines: list[str],
    rows: Iterable[tuple[str, float, float]],
    merman_rev: str | None,
    mmdr_rev: str | None,
) -> None:
    out_path.parent.mkdir(parents=True, exist_ok=True)

    ts = _dt.datetime.now(_dt.timezone.utc).astimezone().strftime("%Y-%m-%d %H:%M:%S %z")
    rustc = rustc_verbose()

    def fmt_rev(label: str, rev: str | None) -> str:
        if rev is None:
            return f"- {label}: unknown"
        return f"- {label}: `{rev}`"

    lines: list[str] = []
    lines.append("# Renderer Performance Comparison")
    lines.append("")
    lines.append("> Generated by `tools/bench/compare_mermaid_renderers.py`.")
    lines.append("")
    lines.append("## Environment")
    lines.append("")
    lines.append(f"- Timestamp: \"{ts}\"")
    for l in env_lines:
        lines.append(l)
    lines.append(fmt_rev("merman", merman_rev))
    lines.append(fmt_rev("mermaid-rs-renderer", mmdr_rev))
    lines.append("- Rust:")
    lines.append("")
    lines.append("```")
    lines.append(rustc)
    lines.append("```")
    lines.append("")
    lines.append("## Method")
    lines.append("")
    lines.append("- `merman`: `cargo bench -p merman --features render --bench pipeline -- ...`")
    lines.append("- `mermaid-rs-renderer` (mmdr): `cargo bench --bench renderer -- ...`")
    lines.append(f"- Filter: \"{filter_expr}\"")
    lines.append(
        f"- Sample size: {sample_size}, warm-up: {warm_up}s, measurement: {measurement}s"
    )
    lines.append("")
    lines.append("## Results (end_to_end, mid estimate)")
    lines.append("")
    lines.append("| benchmark | merman | mermaid-rs-renderer | ratio (merman / mmdr) |")
    lines.append("|---|---:|---:|---:|")

    any_rows = False
    for name, merman_ns, mmdr_ns in sorted(rows, key=lambda r: r[0]):
        any_rows = True
        ratio = merman_ns / mmdr_ns if mmdr_ns > 0 else float("inf")
        lines.append(
            f"| end_to_end/{name} | {pretty_time(merman_ns)} | {pretty_time(mmdr_ns)} | {ratio:.1f}x |"
        )
    if not any_rows:
        lines.append("| (no matches) | - | - | - |")

    lines.append("")
    lines.append("## Notes")
    lines.append("")
    lines.append(
        "- `merman` is parity-focused (upstream Mermaid SVG DOM gates) and optimized for deterministic alignment."
    )
    lines.append(
        "- `mermaid-rs-renderer` is a different renderer with different goals and coverage; raw performance numbers are not directly comparable to visual/DOM parity."
    )
    lines.append(
        "- Treat these as **local regression tracking** numbers. Always re-run on the same machine/toolchain for meaningful comparisons."
    )
    lines.append("")

    out_path.write_text("\n".join(lines), encoding="utf-8")


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--mmdr-dir",
        default="repo-ref/mermaid-rs-renderer",
        help="Path to a local checkout of mermaid-rs-renderer (default: repo-ref/mermaid-rs-renderer).",
    )
    ap.add_argument(
        "--out",
        default="docs/performance/COMPARISON.md",
        help="Where to write the Markdown report.",
    )
    ap.add_argument(
        "--filter",
        default=r"end_to_end/(flowchart_tiny|sequence_tiny|state_tiny|class_tiny)",
        help="Criterion regex filter (passed to both benches).",
    )
    ap.add_argument("--sample-size", type=int, default=20)
    ap.add_argument("--warm-up", type=int, default=1)
    ap.add_argument("--measurement", type=int, default=1)
    args = ap.parse_args(argv)

    repo_root = Path(__file__).resolve().parents[2]
    mmdr_dir = (repo_root / args.mmdr_dir).resolve()
    out_path = (repo_root / args.out).resolve()

    if not mmdr_dir.exists():
        raise SystemExit(
            f"missing mermaid-rs-renderer checkout: {mmdr_dir}\n"
            "expected a local clone at that path (no submodules)."
        )

    # Run benches.
    merman_cmd = [
        "cargo",
        "bench",
        "-p",
        "merman",
        "--features",
        "render",
        "--bench",
        "pipeline",
        "--",
        "--noplot",
        "--sample-size",
        str(args.sample_size),
        "--warm-up-time",
        str(args.warm_up),
        "--measurement-time",
        str(args.measurement),
        args.filter,
    ]
    mmdr_cmd = [
        "cargo",
        "bench",
        "--bench",
        "renderer",
        "--",
        "--noplot",
        "--sample-size",
        str(args.sample_size),
        "--warm-up-time",
        str(args.warm_up),
        "--measurement-time",
        str(args.measurement),
        args.filter,
    ]

    print("[bench] merman:", " ".join(merman_cmd))
    merman_out = run(merman_cmd, cwd=repo_root)

    print("[bench] mermaid-rs-renderer:", " ".join(mmdr_cmd))
    mmdr_out = run(mmdr_cmd, cwd=mmdr_dir)

    merman_times = parse_criterion_times(merman_out, prefix="end_to_end")
    mmdr_times = parse_criterion_times(mmdr_out, prefix="end_to_end")

    common_names = sorted(set(merman_times.keys()) & set(mmdr_times.keys()))
    rows: list[tuple[str, float, float]] = []
    for name in common_names:
        rows.append((name, merman_times[name].to_nanos(), mmdr_times[name].to_nanos()))

    env_lines = [
        f"- OS: \"{platform.platform()}\"",
        f"- Machine: \"{platform.machine()}\"",
        f"- CPU: \"{platform.processor() or 'unknown'}\"",
        f"- Python: \"{platform.python_version()}\"",
    ]
    write_markdown(
        out_path,
        filter_expr=args.filter,
        sample_size=args.sample_size,
        warm_up=args.warm_up,
        measurement=args.measurement,
        env_lines=env_lines,
        rows=rows,
        merman_rev=git_head(repo_root),
        mmdr_rev=git_head(mmdr_dir),
    )

    print("Wrote:", out_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
