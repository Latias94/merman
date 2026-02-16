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
import json
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


_SKIP_LINE = re.compile(
    r"^\[bench\]\[skip\]\[(?P<group>[A-Za-z0-9_\-]+)\]\s+(?P<name>[A-Za-z0-9_\-]+):\s*(?P<reason>.+)$"
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

    This helper supports the default filter shape used by this repo and returns a list of exact
    benchmark names that we can run with `--exact` in both projects.
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


_LIST_LINE = re.compile(r"^(?P<bench>[A-Za-z0-9_/-]+):\s*benchmark\s*$")


def list_criterion_benches(
    *,
    cwd: Path,
    bench_bin: str,
    package: str | None,
    features: str | None,
) -> set[str]:
    cmd: list[str] = ["cargo", "bench"]
    if package:
        cmd.extend(["-p", package])
    if features:
        cmd.extend(["--features", features])
    cmd.extend(["--bench", bench_bin, "--", "--list"])
    out = run(cmd, cwd=cwd)
    benches: set[str] = set()
    for raw in out.splitlines():
        line = strip_ansi(raw).strip()
        m = _LIST_LINE.match(line)
        if not m:
            continue
        benches.add(m.group("bench"))
    return benches


def write_markdown(
    out_path: Path,
    *,
    filter_expr: str,
    exact_benches: list[str],
    sample_size: int,
    warm_up: int,
    measurement: int,
    env_lines: list[str],
    rows: Iterable[tuple[str, float, float, float | None]],
    merman_rev: str | None,
    mmdr_rev: str | None,
    mermaid_js_rev: str | None,
    skipped_merman: dict[str, list[str]] | None,
    missing_merman: list[str] | None,
    missing_mmdr: list[str] | None,
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
    lines.append(fmt_rev("mermaid-js", mermaid_js_rev))
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
    if exact_benches:
        benches_str = ", ".join(f"`{b}`" for b in exact_benches)
        lines.append(f"- Exact benches: {benches_str}")
    lines.append(
        f"- Sample size: {sample_size}, warm-up: {warm_up}s, measurement: {measurement}s"
    )
    lines.append("")
    lines.append("## Results (end_to_end, mid estimate)")
    lines.append("")
    lines.append(
        "| benchmark | merman | mermaid-rs-renderer | mermaid-js (puppeteer) | ratio (merman / mmdr) | ratio (merman / mermaid-js) |"
    )
    lines.append("|---|---:|---:|---:|---:|---:|")

    def fmt_ratio(v: float) -> str:
        if not (v > 0) or v == float("inf"):
            return "inf"
        if v < 0.1:
            return f"{v:.2f}x"
        return f"{v:.1f}x"

    any_rows = False
    for name, merman_ns, mmdr_ns, mermaid_js_ns in sorted(rows, key=lambda r: r[0]):
        any_rows = True
        ratio_mmdr = merman_ns / mmdr_ns if mmdr_ns > 0 else float("inf")
        if mermaid_js_ns is None or mermaid_js_ns <= 0:
            lines.append(
                f"| end_to_end/{name} | {pretty_time(merman_ns)} | {pretty_time(mmdr_ns)} | - | {fmt_ratio(ratio_mmdr)} | - |"
            )
        else:
            ratio_js = merman_ns / mermaid_js_ns
            lines.append(
                f"| end_to_end/{name} | {pretty_time(merman_ns)} | {pretty_time(mmdr_ns)} | {pretty_time(mermaid_js_ns)} | {fmt_ratio(ratio_mmdr)} | {fmt_ratio(ratio_js)} |"
            )
    if not any_rows:
        lines.append("| (no matches) | - | - | - | - | - |")

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

    if skipped_merman:
        skipped = skipped_merman.get("end_to_end") or []
        if skipped:
            lines.append("## Skipped (merman)")
            lines.append("")
            lines.append(
                "These fixtures were present but skipped because `merman` returned a parse/layout/render error during the pre-check."
            )
            lines.append("")
            lines.append(", ".join(f"`{n}`" for n in skipped))
            lines.append("")

    if missing_merman:
        lines.append("## Missing benches (merman)")
        lines.append("")
        lines.append(
            "These benches were requested by the filter but are not present in `merman`'s Criterion list."
        )
        lines.append("")
        lines.append(", ".join(f"`{n}`" for n in missing_merman))
        lines.append("")

    if missing_mmdr:
        lines.append("## Missing benches (mermaid-rs-renderer)")
        lines.append("")
        lines.append(
            "These benches were requested by the filter but are not present in `mermaid-rs-renderer`'s Criterion list."
        )
        lines.append("")
        lines.append(", ".join(f"`{n}`" for n in missing_mmdr))
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
        "--mermaid-cli-dir",
        default="tools/mermaid-cli",
        help="Path to the local Node toolchain used for upstream Mermaid rendering (default: tools/mermaid-cli).",
    )
    ap.add_argument(
        "--out",
        default="docs/performance/COMPARISON.md",
        help="Where to write the Markdown report.",
    )
    ap.add_argument(
        "--filter",
        default=r"end_to_end/(flowchart_tiny|flowchart_medium|flowchart_large|sequence_tiny|sequence_medium|state_tiny|state_medium|class_tiny|class_medium)",
        help="Criterion regex filter (passed to both benches).",
    )
    ap.add_argument("--sample-size", type=int, default=20)
    ap.add_argument("--warm-up", type=int, default=1)
    ap.add_argument("--measurement", type=int, default=1)
    args = ap.parse_args(argv)

    repo_root = Path(__file__).resolve().parents[2]
    mmdr_dir = (repo_root / args.mmdr_dir).resolve()
    mermaid_cli_dir = (repo_root / args.mermaid_cli_dir).resolve()
    out_path = (repo_root / args.out).resolve()

    if not mmdr_dir.exists():
        raise SystemExit(
            f"missing mermaid-rs-renderer checkout: {mmdr_dir}\n"
            "expected a local clone at that path (no submodules)."
        )

    requested = expand_filter_to_exact_benches(args.filter)

    # Keep reports stable when the two repos differ in which benchmarks exist (e.g. one repo may
    # omit certain fixtures under `end_to_end/*`). We still record what was missing for traceability.
    merman_benches = list_criterion_benches(
        cwd=repo_root,
        bench_bin="pipeline",
        package="merman",
        features="render",
    )
    mmdr_benches = list_criterion_benches(
        cwd=mmdr_dir,
        bench_bin="renderer",
        package=None,
        features=None,
    )
    missing_merman = sorted(b for b in requested if b not in merman_benches)
    missing_mmdr = sorted(b for b in requested if b not in mmdr_benches)
    benches = [b for b in requested if b in merman_benches and b in mmdr_benches]
    if not benches:
        raise SystemExit(
            "filter expanded to no runnable benches after intersecting repo benchmark lists.\n"
            f"filter: {args.filter!r}\n"
            f"missing (merman): {missing_merman}\n"
            f"missing (mmdr): {missing_mmdr}"
        )

    def bench_exact(
        *,
        cwd: Path,
        bench_bin: str,
        package: str | None,
        features: str | None,
        exact: str,
    ) -> str:
        cmd: list[str] = ["cargo", "bench"]
        if package:
            cmd.extend(["-p", package])
        if features:
            cmd.extend(["--features", features])
        cmd.extend(["--bench", bench_bin, "--"])
        cmd.extend(
            [
                "--noplot",
                "--sample-size",
                str(args.sample_size),
                "--warm-up-time",
                str(args.warm_up),
                "--measurement-time",
                str(args.measurement),
                "--discard-baseline",
                "--exact",
                exact,
            ]
        )
        return run(cmd, cwd=cwd)

    # Run benches (exact) so both Criterion CLI variants behave consistently.
    merman_times: dict[str, TimeEstimate] = {}
    mmdr_times: dict[str, TimeEstimate] = {}
    merman_out_all: list[str] = []

    for exact in benches:
        prefix = exact.split("/", 1)[0]

        print(
            "[bench] merman:",
            f"cargo bench -p merman --features render --bench pipeline -- ... --exact {exact}",
        )
        out = bench_exact(
            cwd=repo_root,
            bench_bin="pipeline",
            package="merman",
            features="render",
            exact=exact,
        )
        merman_out_all.append(out)
        merman_times.update(parse_criterion_times(out, prefix=prefix))

        print(
            "[bench] mermaid-rs-renderer:",
            f"cargo bench --bench renderer -- ... --exact {exact}",
        )
        out = bench_exact(
            cwd=mmdr_dir,
            bench_bin="renderer",
            package=None,
            features=None,
            exact=exact,
        )
        mmdr_times.update(parse_criterion_times(out, prefix=prefix))

    skipped_merman = parse_skip_lines("\n".join(merman_out_all))

    mermaid_js_results: dict[str, float] = {}
    mermaid_js_rev: str | None = None
    mermaid_js_meta: dict[str, str] = {}
    if mermaid_cli_dir.exists():
        # Bench upstream Mermaid JS rendering in a single long-lived headless Chromium instance.
        bench_in = repo_root / "target" / "bench" / "mermaid_js_input.json"
        bench_out = repo_root / "target" / "bench" / "mermaid_js_output.json"
        bench_in.parent.mkdir(parents=True, exist_ok=True)

        fixtures_dir = repo_root / "crates" / "merman" / "benches" / "fixtures"
        fixtures: dict[str, str] = {}
        if fixtures_dir.exists():
            for p in fixtures_dir.glob("*.mmd"):
                fixtures[p.stem] = p.read_text(encoding="utf-8")

        wanted = {b.split("/", 1)[1] for b in benches if b.startswith("end_to_end/")}
        if wanted:
            fixtures = {name: text for name, text in fixtures.items() if name in wanted}

        bench_in.write_text(
            json.dumps(
                {
                    "fixtures": fixtures,
                    "configPath": "mermaid-config.json",
                    "theme": "default",
                    "seed": "1",
                    "width": 800,
                    "warmupMs": args.warm_up * 1000,
                    "measureMs": args.measurement * 1000,
                },
                indent=2,
            ),
            encoding="utf-8",
        )

        script = repo_root / "tools" / "bench" / "mermaid_js_bench.cjs"
        print("[bench] mermaid-js (puppeteer): node", script)
        try:
            run(
                [
                    "node",
                    str(script),
                    "--in",
                    str(bench_in),
                    "--out",
                    str(bench_out),
                ],
                cwd=mermaid_cli_dir,
            )
        except Exception as e:
            print("[bench] mermaid-js: skipped (benchmark failed)")
            print("reason:", str(e).splitlines()[0] if str(e) else repr(e))
            mermaid_js_results = {}
            mermaid_js_rev = None
            mermaid_js_meta = {}
            bench_out = None

        if bench_out is not None and bench_out.exists():
            data = json.loads(bench_out.read_text(encoding="utf-8", errors="replace"))
            if isinstance(data.get("meta"), dict):
                mermaid_js_meta = {
                    k: str(v) for k, v in data.get("meta").items() if isinstance(k, str)
                }
            for name, v in (data.get("results") or {}).items():
                med = v.get("median_ns")
                if isinstance(med, (int, float)) and med > 0:
                    mermaid_js_results[name] = float(med)

            # Prefer meta, then fall back to package-lock parsing.
            if mermaid_js_meta.get("mermaid"):
                mermaid_js_rev = "mermaid@" + mermaid_js_meta["mermaid"]
            else:
                lock = mermaid_cli_dir / "package-lock.json"
                if lock.exists():
                    try:
                        lock_data = json.loads(lock.read_text(encoding="utf-8", errors="replace"))
                        ver = (
                            (lock_data.get("packages") or {})
                            .get("node_modules/mermaid", {})
                            .get("version")
                        )
                        if isinstance(ver, str) and ver.strip():
                            mermaid_js_rev = "mermaid@" + ver.strip()
                    except Exception:
                        mermaid_js_rev = None
    else:
        print("[bench] mermaid-js: skipped (missing tools/mermaid-cli)")

    common_names = sorted(set(merman_times.keys()) & set(mmdr_times.keys()))
    rows: list[tuple[str, float, float, float | None]] = []
    for name in common_names:
        js_ns = mermaid_js_results.get(name)
        rows.append(
            (
                name,
                merman_times[name].to_nanos(),
                mmdr_times[name].to_nanos(),
                js_ns,
            )
        )

    env_lines = [
        f"- OS: \"{platform.platform()}\"",
        f"- Machine: \"{platform.machine()}\"",
        f"- CPU: \"{best_effort_cpu_model()}\"",
        f"- Python: \"{platform.python_version()}\"",
    ]
    if mermaid_js_meta.get("node"):
        env_lines.append(f"- Node: \"{mermaid_js_meta['node']}\"")
    if mermaid_js_meta.get("chromium"):
        env_lines.append(f"- Chromium: \"{mermaid_js_meta['chromium']}\"")
    if mermaid_js_meta.get("puppeteer"):
        env_lines.append(f"- Puppeteer: \"{mermaid_js_meta['puppeteer']}\"")
    if mermaid_js_meta.get("mermaid_cli"):
        env_lines.append(f"- mermaid-cli: \"{mermaid_js_meta['mermaid_cli']}\"")
    write_markdown(
        out_path,
        filter_expr=args.filter,
        exact_benches=benches,
        sample_size=args.sample_size,
        warm_up=args.warm_up,
        measurement=args.measurement,
        env_lines=env_lines,
        rows=rows,
        merman_rev=git_head(repo_root),
        mmdr_rev=git_head(mmdr_dir),
        mermaid_js_rev=mermaid_js_rev,
        skipped_merman=skipped_merman,
        missing_merman=missing_merman,
        missing_mmdr=missing_mmdr,
    )

    print("Wrote:", out_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
