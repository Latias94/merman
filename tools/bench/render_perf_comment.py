#!/usr/bin/env python3
"""Render the pull request comment body for performance regression reports."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


MARKER = "<!-- merman-perf-regression -->"
DEFAULT_TITLE = "Merman Performance Regression"


def fmt_percent(value: Any) -> str:
    if not isinstance(value, (int, float)):
        return "-"
    return f"{float(value):+.2f}%"


def fmt_time(value: Any) -> str:
    if not isinstance(value, (int, float)):
        return "-"
    nanos = float(value)
    if nanos < 1e3:
        return f"{nanos:.2f} ns"
    if nanos < 1e6:
        return f"{nanos / 1e3:.2f} us"
    if nanos < 1e9:
        return f"{nanos / 1e6:.2f} ms"
    return f"{nanos / 1e9:.2f} s"


def load_report(path: Path) -> dict[str, Any] | None:
    if not path.exists():
        return None
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None
    return data if isinstance(data, dict) else None


def status_label(summary: dict[str, Any]) -> str:
    gate = str(summary.get("gate_status") or "unknown")
    failures = int(summary.get("failures") or 0)
    warnings = int(summary.get("warnings") or 0)
    if gate == "fail":
        return "failed"
    if warnings:
        return "passed with warnings"
    if failures:
        return "failed"
    if gate == "pass":
        return "passed"
    return gate


def signal_rows(report: dict[str, Any], limit: int = 8) -> list[dict[str, Any]]:
    rows = report.get("rows") if isinstance(report.get("rows"), list) else []
    priority = {"fail": 0, "warn": 1, "improved": 2}
    selected = [
        row for row in rows
        if isinstance(row, dict) and str(row.get("status")) in priority
    ]
    selected.sort(
        key=lambda row: (
            priority.get(str(row.get("status")), 99),
            -abs(float(row.get("change_percent") or 0.0)),
            str(row.get("benchmark") or ""),
        )
    )
    return selected[:limit]


def render_comment(
    report: dict[str, Any] | None,
    *,
    run_url: str,
    artifact_name: str,
    marker: str = MARKER,
    title: str = DEFAULT_TITLE,
) -> str:
    lines: list[str] = [marker, f"## {title}"]
    lines.append("")

    if report is None:
        lines.append("Status: `report unavailable`")
        lines.append("")
        lines.append(
            "The performance job did not produce a parseable self-comparison report. "
            "Check the workflow logs for setup, build, or benchmark runner errors."
        )
        lines.append("")
        lines.append(f"Run: {run_url}")
        return "\n".join(lines) + "\n"

    summary = report.get("summary") if isinstance(report.get("summary"), dict) else {}
    method = report.get("method") if isinstance(report.get("method"), dict) else {}
    selection = report.get("selection") if isinstance(report.get("selection"), dict) else {}
    comparison = report.get("comparison") if isinstance(report.get("comparison"), dict) else {}

    lines.append(f"Status: `{status_label(summary)}`")
    lines.append("")
    lines.append(
        f"- Comparison: `{comparison.get('base_label') or 'base'}` -> "
        f"`{comparison.get('head_label') or 'head'}`"
    )
    lines.append(
        f"- Suite: `{selection.get('suite') or selection.get('filter') or 'unknown'}`"
    )
    lines.append(f"- Preset: `{method.get('preset') or 'unknown'}`")
    lines.append(f"- Comparable benchmarks: `{summary.get('comparable', 0)}`")
    lines.append(f"- Failures: `{summary.get('failures', 0)}`")
    lines.append(f"- Warnings: `{summary.get('warnings', 0)}`")
    lines.append(f"- Improvements: `{summary.get('improvements', 0)}`")
    lines.append(f"- Geomean change: `{fmt_percent(summary.get('geomean_change_percent'))}`")
    lines.append(
        f"- Thresholds: warn `+{float(method.get('warn_threshold_percent') or 0.0):.2f}%`, "
        f"fail `+{float(method.get('fail_threshold_percent') or 0.0):.2f}%`"
    )
    lines.append("")

    signals = signal_rows(report)
    if signals:
        lines.append("| benchmark | base | head | change | status |")
        lines.append("|---|---:|---:|---:|---|")
        for row in signals:
            lines.append(
                f"| `{row.get('benchmark', '-')}` | {fmt_time(row.get('base_ns'))} | "
                f"{fmt_time(row.get('head_ns'))} | {fmt_percent(row.get('change_percent'))} | "
                f"`{row.get('status', '-')}` |"
            )
        if len(signals) < len(
            [
                row for row in report.get("rows", [])
                if isinstance(row, dict) and str(row.get("status")) in {"fail", "warn", "improved"}
            ]
        ):
            lines.append("")
            lines.append("Only the largest signal rows are shown here; see the artifact for the full report.")
    else:
        lines.append("No benchmark crossed the warning or failure threshold.")

    lines.append("")
    lines.append(f"Full report: [`{artifact_name}` artifact]({run_url})")
    lines.append("")
    lines.append(
        "Note: this PR signal currently compares same-runner mid estimates against percentage "
        "thresholds. Treat small movements as prompts for a longer run, not final proof."
    )
    return "\n".join(lines) + "\n"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Render a performance PR comment.")
    ap.add_argument("--json", required=True, help="Self-comparison JSON report.")
    ap.add_argument("--out", required=True, help="Markdown comment output path.")
    ap.add_argument("--run-url", required=True, help="GitHub Actions run URL.")
    ap.add_argument("--artifact", default="perf-regression", help="Artifact name.")
    ap.add_argument(
        "--marker",
        default=MARKER,
        help="Sticky PR comment marker used to locate the existing comment.",
    )
    ap.add_argument(
        "--title",
        default=DEFAULT_TITLE,
        help="Heading used in the rendered PR comment.",
    )
    args = ap.parse_args(argv)

    body = render_comment(
        load_report(Path(args.json)),
        run_url=args.run_url,
        artifact_name=args.artifact,
        marker=args.marker,
        title=args.title,
    )
    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(body, encoding="utf-8")
    print("Wrote:", out_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
