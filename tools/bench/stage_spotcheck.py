#!/usr/bin/env python3
"""
Stage spot-check for merman vs mermaid-rs-renderer (mmdr).

This script runs a small, stable set of Criterion benchmarks for a few fixtures and produces a
compact, stage-by-stage report:

- parse
- layout
- render (merman) / render_svg (mmdr)
- end_to_end

It is designed for quick perf triage (identify which stage moved) rather than long-running,
high-confidence benchmarking.
"""

from __future__ import annotations

import argparse
import math
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


@dataclass(frozen=True)
class Duration:
    micros: float
    raw: str


TIME_LINE_RE = re.compile(
    r"^(?P<bench>[A-Za-z0-9_/-]+)\s+time:\s+\[(?P<body>[^\]]+)\]\s*$"
)
TIME_ONLY_RE = re.compile(r"^\s*time:\s+\[(?P<body>[^\]]+)\]\s*$")


def parse_duration(token: str) -> Duration:
    token = token.strip()
    m = re.match(r"^(?P<num>[0-9.]+)\s*(?P<unit>ns|µs|us|ms|s)$", token)
    if not m:
        raise ValueError(f"Unrecognized duration token: {token!r}")
    num = float(m.group("num"))
    unit = m.group("unit")

    if unit == "ns":
        micros = num / 1_000.0
    elif unit in ("µs", "us"):
        micros = num
    elif unit == "ms":
        micros = num * 1_000.0
    elif unit == "s":
        micros = num * 1_000_000.0
    else:
        raise ValueError(f"Unrecognized duration unit: {unit!r}")

    return Duration(micros=micros, raw=token)


def gmean(values: Iterable[float]) -> float:
    vals = [v for v in values if v > 0.0 and math.isfinite(v)]
    if not vals:
        return float("nan")
    return math.exp(sum(math.log(v) for v in vals) / len(vals))


def run(cmd: list[str], cwd: Path) -> str:
    proc = subprocess.run(
        cmd,
        cwd=str(cwd),
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if proc.returncode != 0:
        raise RuntimeError(
            f"command failed (exit {proc.returncode}) in {cwd}\n$ {' '.join(cmd)}\n\n{proc.stdout}"
        )
    return proc.stdout


def extract_mid_time(output: str, expected_bench: str) -> Duration:
    def parse_body(body: str) -> Duration:
        parts = body.strip().split()

        tokens: list[str]
        if len(parts) == 6:
            tokens = [
                f"{parts[0]} {parts[1]}",
                f"{parts[2]} {parts[3]}",
                f"{parts[4]} {parts[5]}",
            ]
        elif len(parts) == 3:
            tokens = [parts[0], parts[1], parts[2]]
        else:
            raise RuntimeError(f"Unrecognized Criterion time format: {body!r}")

        return parse_duration(tokens[1])

    lines = [ln.rstrip("\n") for ln in output.splitlines()]

    # Format A: single line: "<bench> time: [lo mid hi]".
    for line in lines:
        m = TIME_LINE_RE.match(line.strip())
        if not m:
            continue
        if m.group("bench") != expected_bench:
            continue
        return parse_body(m.group("body"))

    # Format B: two lines:
    #   <bench>
    #       time: [lo mid hi]
    for i, line in enumerate(lines):
        if line.strip() != expected_bench:
            continue
        for j in range(i + 1, min(i + 6, len(lines))):
            m = TIME_ONLY_RE.match(lines[j])
            if not m:
                continue
            return parse_body(m.group("body"))

    raise RuntimeError(f"Could not find Criterion time line for {expected_bench!r}")


def cargo_bench_cmd(
    *,
    sample_size: int,
    warm_up: int,
    measurement: int,
    exact: str,
    package: str | None,
    features: str | None,
    bench: str,
) -> list[str]:
    cmd = ["cargo", "bench"]
    if package:
        cmd.extend(["-p", package])
    if features:
        cmd.extend(["--features", features])
    cmd.extend(["--bench", bench, "--"])
    cmd.extend(
        [
            "--noplot",
            "--sample-size",
            str(sample_size),
            "--warm-up-time",
            str(warm_up),
            "--measurement-time",
            str(measurement),
            "--discard-baseline",
            "--exact",
            exact,
        ]
    )
    return cmd


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--fixtures",
        default="flowchart_medium,class_medium",
        help="Comma-separated fixture names (default: flowchart_medium,class_medium).",
    )
    ap.add_argument("--sample-size", type=int, default=20)
    ap.add_argument("--warm-up", type=int, default=1)
    ap.add_argument("--measurement", type=int, default=1)
    ap.add_argument(
        "--mmdr-dir",
        default="repo-ref/mermaid-rs-renderer",
        help="Path to a local checkout of mermaid-rs-renderer.",
    )
    ap.add_argument(
        "--out",
        default="",
        help="Optional output Markdown path (relative to repo root).",
    )
    args = ap.parse_args(argv)

    repo_root = Path(__file__).resolve().parents[2]
    mmdr_dir = (repo_root / args.mmdr_dir).resolve()

    fixtures = [x.strip() for x in args.fixtures.split(",") if x.strip()]
    if not fixtures:
        raise SystemExit("No fixtures specified.")

    stages_merman = ["parse", "layout", "render", "end_to_end"]
    stages_mmdr = ["parse", "layout", "render_svg", "end_to_end"]

    rows: list[tuple[str, str, Duration | None, Duration | None, float | None]] = []

    for fixture in fixtures:
        for stage in stages_merman:
            mmdr_stage = "render_svg" if stage == "render" else stage

            merman_exact = f"{stage}/{fixture}"
            mmdr_exact = f"{mmdr_stage}/{fixture}"

            merman_out = run(
                cargo_bench_cmd(
                    sample_size=args.sample_size,
                    warm_up=args.warm_up,
                    measurement=args.measurement,
                    exact=merman_exact,
                    package="merman",
                    features="render",
                    bench="pipeline",
                ),
                cwd=repo_root,
            )
            merman_mid = extract_mid_time(merman_out, expected_bench=merman_exact)

            mmdr_out = run(
                cargo_bench_cmd(
                    sample_size=args.sample_size,
                    warm_up=args.warm_up,
                    measurement=args.measurement,
                    exact=mmdr_exact,
                    package=None,
                    features=None,
                    bench="renderer",
                ),
                cwd=mmdr_dir,
            )
            mmdr_mid = extract_mid_time(mmdr_out, expected_bench=mmdr_exact)

            ratio = merman_mid.micros / mmdr_mid.micros if mmdr_mid.micros > 0 else float("nan")
            rows.append((fixture, stage, merman_mid, mmdr_mid, ratio))

    lines: list[str] = []
    lines.append("# Stage Spot-check (merman vs mermaid-rs-renderer)")
    lines.append("")
    lines.append("This report is intended for quick perf triage (stage attribution).")
    lines.append("")
    lines.append("## Parameters")
    lines.append("")
    lines.append(f"- sample-size: `{args.sample_size}`")
    lines.append(f"- warm-up: `{args.warm_up}s`")
    lines.append(f"- measurement: `{args.measurement}s`")
    lines.append(f"- fixtures: `{', '.join(fixtures)}`")
    lines.append("")
    lines.append("## Results (mid estimate)")
    lines.append("")
    lines.append("| fixture | stage | merman | mmdr | ratio |")
    lines.append("|---|---|---:|---:|---:|")
    for fixture, stage, merman_mid, mmdr_mid, ratio in rows:
        lines.append(
            f"| `{fixture}` | `{stage}` | {merman_mid.raw} | {mmdr_mid.raw} | {ratio:.2f}x |"
        )

    lines.append("")
    lines.append("## Summary (geometric mean of ratios)")
    lines.append("")
    for stage in stages_merman:
        ratios = [r for f, s, _, _, r in rows if s == stage and r is not None]
        lines.append(f"- `{stage}`: `{gmean(ratios):.2f}x`")

    out = "\n".join(lines) + "\n"
    if args.out:
        out_path = (repo_root / args.out).resolve()
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(out, encoding="utf-8")
        print(f"Wrote: {out_path}")
    else:
        sys.stdout.write(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
