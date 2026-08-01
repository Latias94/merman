#!/usr/bin/env python3
"""Render the pull request comment body for performance regression reports."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Any


MARKER = "<!-- merman-perf-regression -->"
DEFAULT_TITLE = "Merman Performance Regression"

V2_OUTCOME_LABELS = {
    "diagnostic_advisory": "diagnostic advisory",
    "confirmed_non_regression": "confirmed non-regression",
    "confirmed_regression": "confirmed regression",
    "inconclusive": "inconclusive",
    "contract_failure": "contract failure",
}

V2_OUTCOME_EXIT_CODES = {
    "diagnostic_advisory": 0,
    "confirmed_non_regression": 0,
    "confirmed_regression": 1,
    "contract_failure": 2,
    "inconclusive": 3,
}

ROW_OUTCOME_LABELS = {
    **V2_OUTCOME_LABELS,
    "confirmed_improvement": "confirmed improvement",
}


def fmt_percent(value: Any) -> str:
    if not _is_finite_number(value):
        return "-"
    return f"{float(value):+.2f}%"


def fmt_time(value: Any) -> str:
    if not _is_finite_number(value):
        return "-"
    nanos = float(value)
    magnitude = abs(nanos)
    if magnitude < 1e3:
        return f"{nanos:.2f} ns"
    if magnitude < 1e6:
        return f"{nanos / 1e3:.2f} us"
    if magnitude < 1e9:
        return f"{nanos / 1e6:.2f} ms"
    return f"{nanos / 1e9:.2f} s"


def _is_finite_number(value: Any) -> bool:
    return (
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and math.isfinite(float(value))
    )


def _mapping(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def _markdown_cell(value: Any) -> str:
    return str(value if value is not None else "-").replace("|", "\\|").replace("\n", " ")


def _confidence_label(value: Any) -> str:
    if not _is_finite_number(value):
        return "95%"
    confidence = float(value)
    if confidence <= 1.0:
        confidence *= 100.0
    return f"{confidence:g}%"


def _quality_label(report: dict[str, Any], summary: dict[str, Any], method: dict[str, Any]) -> str:
    quality = (
        report.get("evidence_quality")
        or summary.get("evidence_quality")
        or method.get("evidence_quality")
    )
    if isinstance(quality, str) and quality:
        return quality.replace("_", " ")
    if isinstance(quality, dict):
        for key in ("label", "status", "outcome"):
            value = quality.get(key)
            if isinstance(value, str) and value:
                return value.replace("_", " ")
        return "see report artifact"
    if method.get("evidence_mode") == "diagnostic":
        return "diagnostic only"
    return "not reported"


def _relative_bounds(bounds: dict[str, Any]) -> dict[str, Any]:
    direct = bounds.get("relative_percent")
    if isinstance(direct, dict):
        return direct

    log_ratio = bounds.get("log_ratio")
    if not isinstance(log_ratio, dict):
        return {}
    converted: dict[str, Any] = {}
    for key in ("estimate", "lower", "upper"):
        value = log_ratio.get(key)
        if _is_finite_number(value):
            converted[key] = math.expm1(float(value)) * 100.0
    return converted


def _fmt_bounds(
    bounds: dict[str, Any],
    *,
    formatter: Any,
) -> str:
    estimate = formatter(bounds.get("estimate"))
    lower = formatter(bounds.get("lower"))
    upper = formatter(bounds.get("upper"))
    return f"{estimate} [{lower}, {upper}]"


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
    if gate == "fail" or failures:
        return "failed"
    if warnings:
        return "passed with warnings"
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


def _render_v1_comment(
    report: dict[str, Any] | None,
    *,
    run_url: str,
    artifact_name: str,
    marker: str = MARKER,
    title: str = DEFAULT_TITLE,
    process_exit_code: int | None = None,
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
        if process_exit_code is not None:
            lines.extend(["", f"Process exit code: `{process_exit_code}`"])
        lines.append("")
        lines.append(f"Run: {run_url}")
        return "\n".join(lines) + "\n"

    summary = report.get("summary") if isinstance(report.get("summary"), dict) else {}
    method = report.get("method") if isinstance(report.get("method"), dict) else {}
    selection = report.get("selection") if isinstance(report.get("selection"), dict) else {}
    comparison = report.get("comparison") if isinstance(report.get("comparison"), dict) else {}

    lines.append(f"Status: `{status_label(summary)}`")
    lines.append("")
    lines.append("- Report schema: `1`")
    lines.append("- Evidence quality: `legacy diagnostic`")
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
        "Note: schema v1 is a legacy diagnostic. It compares same-runner mid estimates against "
        "percentage thresholds and cannot accept or reject a performance candidate."
    )
    return "\n".join(lines) + "\n"


def _render_unsupported_schema(
    report: dict[str, Any],
    *,
    run_url: str,
    artifact_name: str,
    marker: str,
    title: str,
) -> str:
    schema = report.get("schema_version")
    schema_label = "missing" if schema is None else str(schema)
    lines = [
        marker,
        f"## {title}",
        "",
        "Status: `unsupported report schema`",
        "",
        f"- Report schema: `{schema_label}`",
        "- Supported schemas: `1` (legacy diagnostic), `2` (paired evidence)",
        "",
        "This report is not eligible for a performance decision. Update the producer or the "
        "comment consumer, then rerun the comparison.",
        "",
        f"Full report: [`{artifact_name}` artifact]({run_url})",
    ]
    return "\n".join(lines) + "\n"


def _v2_row_outcome(row: dict[str, Any]) -> str:
    outcome = row.get("outcome") or row.get("regression_outcome") or row.get("status")
    label = ROW_OUTCOME_LABELS.get(str(outcome))
    if label is not None:
        return label
    return str(outcome).replace("_", " ") if outcome else "not reported"


def _candidate_decision(report: dict[str, Any], summary: dict[str, Any]) -> str | None:
    for owner in (summary, report):
        for key in (
            "candidate_decision",
            "candidate_outcome",
            "admission_outcome",
            "admission",
        ):
            value = owner.get(key)
            if isinstance(value, str) and value in {"accepted", "rejected", "inconclusive"}:
                return value
            if isinstance(value, dict):
                nested = value.get("outcome") or value.get("decision")
                if nested in {"accepted", "rejected", "inconclusive"}:
                    return str(nested)
    return None


def _v2_decision_text(outcome: str, candidate_decision: str | None) -> str:
    if candidate_decision is not None:
        return f"Candidate decision: `{candidate_decision}`."
    messages = {
        "diagnostic_advisory": (
            "Candidate decision: `diagnostic only`; this schedule cannot accept or reject a candidate."
        ),
        "confirmed_non_regression": (
            "Candidate decision: `not accepted by this result alone`; non-regression is confirmed, "
            "but acceptance also requires mirrored improvement and every mandatory gate."
        ),
        "confirmed_regression": (
            "Candidate decision: `rejected`; both regression bounds cleared their thresholds."
        ),
        "inconclusive": (
            "Candidate decision: `inconclusive`; follow the report's bounded retest contract."
        ),
        "contract_failure": (
            "Candidate decision: `not admissible`; the evidence contract failed."
        ),
    }
    return messages[outcome]


def _v2_rows(report: dict[str, Any], limit: int = 8) -> tuple[list[dict[str, Any]], int]:
    raw_rows = report.get("rows")
    rows = [row for row in raw_rows if isinstance(row, dict)] if isinstance(raw_rows, list) else []
    priority = {
        "contract_failure": 0,
        "confirmed_regression": 1,
        "inconclusive": 2,
        "diagnostic_advisory": 3,
        "confirmed_improvement": 4,
        "confirmed_non_regression": 5,
    }
    rows.sort(
        key=lambda row: (
            priority.get(
                str(row.get("outcome") or row.get("regression_outcome") or row.get("status")),
                99,
            ),
            str(row.get("benchmark") or ""),
        )
    )
    return rows[:limit], len(rows)


def _v2_exit_contract_errors(
    summary: dict[str, Any],
    *,
    process_exit_code: int | None,
) -> list[str]:
    outcome = summary.get("outcome")
    expected_exit = V2_OUTCOME_EXIT_CODES.get(outcome)
    errors: list[str] = []
    report_exit = summary.get("exit_code")

    if expected_exit is None:
        errors.append(f"unsupported or missing summary outcome `{_markdown_cell(outcome)}`")

    if not isinstance(report_exit, int) or isinstance(report_exit, bool):
        errors.append("summary exit code is missing or is not an integer")
    elif expected_exit is not None and report_exit != expected_exit:
        errors.append(
            f"summary exit code `{report_exit}` does not match outcome `{outcome}` "
            f"(expected `{expected_exit}`)"
        )

    if process_exit_code is not None:
        if (
            not isinstance(process_exit_code, int)
            or isinstance(process_exit_code, bool)
            or process_exit_code not in {0, 1, 2, 3}
        ):
            errors.append("process exit code must be one of `0`, `1`, `2`, or `3`")
        elif isinstance(report_exit, int) and not isinstance(report_exit, bool):
            if process_exit_code != report_exit:
                errors.append(
                    f"process exit code `{process_exit_code}` does not match report exit code "
                    f"`{report_exit}`"
                )

    return errors


def _v2_report_contract_errors(
    report: dict[str, Any],
    *,
    process_exit_code: int | None,
) -> list[str]:
    summary = _mapping(report.get("summary"))
    method = _mapping(report.get("method"))
    errors = _v2_exit_contract_errors(
        summary,
        process_exit_code=process_exit_code,
    )

    raw_contract_errors = report.get("contract_errors", [])
    if not isinstance(raw_contract_errors, list):
        errors.append("contract_errors must be a list")
        raw_contract_errors = ["malformed contract_errors"]

    raw_rows = report.get("rows")
    if not isinstance(raw_rows, list):
        errors.append("rows must be a list")
        return errors
    if not raw_rows and not raw_contract_errors:
        errors.append("report has neither benchmark rows nor contract errors")

    allowed_row_outcomes = {
        "diagnostic_advisory",
        "confirmed_non_regression",
        "confirmed_regression",
        "inconclusive",
        "contract_failure",
    }
    row_outcomes: list[str] = []
    for index, row in enumerate(raw_rows):
        if not isinstance(row, dict):
            errors.append(f"row {index} is not an object")
            row_outcomes.append("contract_failure")
            continue
        row_outcome = row.get("outcome")
        if row_outcome not in allowed_row_outcomes:
            errors.append(f"row {index} has unsupported outcome `{_markdown_cell(row_outcome)}`")
            row_outcomes.append("contract_failure")
            continue
        row_outcomes.append(str(row_outcome))

    evidence_mode = method.get("evidence_mode")
    discovery_only = method.get("discovery_only", False)
    if evidence_mode not in {"diagnostic", "confirmation"}:
        errors.append("method.evidence_mode must be diagnostic or confirmation")
    if not isinstance(discovery_only, bool):
        errors.append("method.discovery_only must be a boolean")
        discovery_only = False

    if raw_contract_errors or "contract_failure" in row_outcomes:
        aggregate_outcome = "contract_failure"
    elif "confirmed_regression" in row_outcomes:
        aggregate_outcome = "confirmed_regression"
    elif "inconclusive" in row_outcomes:
        aggregate_outcome = "inconclusive"
    elif discovery_only or evidence_mode == "diagnostic":
        aggregate_outcome = "diagnostic_advisory"
    else:
        aggregate_outcome = "confirmed_non_regression"

    summary_outcome = summary.get("outcome")
    if summary_outcome in V2_OUTCOME_EXIT_CODES and summary_outcome != aggregate_outcome:
        errors.append(
            f"summary outcome `{summary_outcome}` does not match aggregate row outcome "
            f"`{aggregate_outcome}`"
        )
    return errors


def _comment_contract_errors(
    report: dict[str, Any] | None,
    *,
    process_exit_code: int | None,
) -> list[str]:
    if report is None:
        return ["performance report is missing or is not parseable"]
    schema = report.get("schema_version")
    if schema == 1 and not isinstance(schema, bool):
        return []
    if schema == 2 and not isinstance(schema, bool):
        return _v2_report_contract_errors(
            report,
            process_exit_code=process_exit_code,
        )
    return ["performance report schema is unsupported"]


def _render_v2_contract_failure(
    errors: list[str],
    *,
    run_url: str,
    artifact_name: str,
    marker: str,
    title: str,
) -> str:
    lines = [
        marker,
        f"## {title}",
        "",
        "Status: `contract failure`",
        "",
        "- Report schema: `2`",
    ]
    lines.extend(f"- Evidence contract: {error}" for error in errors)
    lines.extend(
        [
            "",
            "The report outcome and exit status are not self-consistent. This evidence is not "
            "eligible for a performance decision.",
            "",
            f"Full report: [`{artifact_name}` artifact]({run_url})",
        ]
    )
    return "\n".join(lines) + "\n"


def _render_v2_comment(
    report: dict[str, Any],
    *,
    run_url: str,
    artifact_name: str,
    marker: str,
    title: str,
    process_exit_code: int | None,
) -> str:
    summary = _mapping(report.get("summary"))
    method = _mapping(report.get("method"))
    selection = _mapping(report.get("selection"))
    comparison = _mapping(report.get("comparison"))
    recipes = _mapping(report.get("recipes"))
    outcome = summary.get("outcome")

    exit_contract_errors = _v2_report_contract_errors(
        report,
        process_exit_code=process_exit_code,
    )
    if exit_contract_errors:
        return _render_v2_contract_failure(
            exit_contract_errors,
            run_url=run_url,
            artifact_name=artifact_name,
            marker=marker,
            title=title,
        )

    evidence_mode = method.get("evidence_mode") or report.get("evidence_mode") or "not reported"
    relative_threshold = method.get("relative_threshold_percent")
    absolute_threshold = method.get("absolute_threshold_ns")
    confidence_contract = _mapping(method.get("confidence_contract"))
    confidence = _confidence_label(
        confidence_contract.get("component_confidence_level")
        or method.get("confidence_level")
    )
    pair_count = method.get("pair_count", summary.get("pair_count", "not reported"))
    required_pairs = method.get("required_pairs", summary.get("required_pairs", "not reported"))
    groups = _mapping(selection.get("groups"))
    base_recipe = _mapping(recipes.get("base"))
    head_recipe = _mapping(recipes.get("head"))

    lines = [marker, f"## {title}", "", f"Status: `{V2_OUTCOME_LABELS[outcome]}`", ""]
    lines.extend(
        [
            "- Report schema: `2`",
            f"- Evidence mode: `{_markdown_cell(evidence_mode)}`",
            f"- Evidence quality: `{_markdown_cell(_quality_label(report, summary, method))}`",
            f"- Comparison: `{_markdown_cell(comparison.get('base_label') or 'base')}` -> "
            f"`{_markdown_cell(comparison.get('head_label') or 'head')}`",
            f"- Suite: `{_markdown_cell(selection.get('suite') or selection.get('filter') or 'unknown')}`",
            f"- Preset: `{_markdown_cell(method.get('preset') or 'unknown')}`",
            f"- Public operation groups: `{_markdown_cell(groups.get('base') or selection.get('group') or 'unknown')}` -> "
            f"`{_markdown_cell(groups.get('head') or selection.get('group') or 'unknown')}`",
            f"- Logical operations per estimate: base "
            f"`{_markdown_cell(base_recipe.get('logical_operations') or 'unknown')}`, head "
            f"`{_markdown_cell(head_recipe.get('logical_operations') or 'unknown')}`",
            f"- Comparable benchmarks: `{summary.get('comparable', 0)}`",
            f"- Balanced pairs: `{pair_count}` measured / `{required_pairs}` required",
            f"- Decision thresholds: relative `{fmt_percent(relative_threshold)}`, "
            f"absolute `{fmt_time(absolute_threshold)}`",
            f"- Simultaneous confidence: `{_confidence_label(confidence_contract.get('simultaneous_confidence_level'))}` "
            f"via `{_markdown_cell(confidence_contract.get('multiplicity_adjustment') or 'unknown')}`, "
            f"family `{_markdown_cell(confidence_contract.get('family_size') or 'unknown')}` at component "
            f"`{_confidence_label(confidence_contract.get('component_confidence_level'))}`",
        ]
    )
    lines.extend(["", _v2_decision_text(outcome, _candidate_decision(report, summary)), ""])

    shown_rows, total_rows = _v2_rows(report)
    if shown_rows:
        lines.append(
            f"| base benchmark | head benchmark | base | head | relative {confidence} bounds | "
            f"absolute {confidence} bounds | outcome |"
        )
        lines.append("|---|---|---:|---:|---:|---:|---|")
        for row in shown_rows:
            bounds = _mapping(row.get("bounds"))
            relative = _relative_bounds(bounds)
            absolute = _mapping(bounds.get("absolute_ns"))
            lines.append(
                f"| `{_markdown_cell(row.get('base_benchmark') or row.get('benchmark'))}` | "
                f"`{_markdown_cell(row.get('head_benchmark') or row.get('benchmark'))}` | "
                f"{fmt_time(row.get('base_ns'))} | "
                f"{fmt_time(row.get('head_ns'))} | "
                f"{_fmt_bounds(relative, formatter=fmt_percent)} | "
                f"{_fmt_bounds(absolute, formatter=fmt_time)} | "
                f"`{_markdown_cell(_v2_row_outcome(row))}` |"
            )
        if len(shown_rows) < total_rows:
            lines.extend(
                [
                    "",
                    "Only the highest-priority evidence rows are shown here; see the artifact for "
                    "the complete paired observations.",
                ]
            )
    else:
        lines.append("No comparable benchmark rows were reported; see the artifact for coverage details.")

    lines.extend(
        [
            "",
            f"Full report: [`{artifact_name}` artifact]({run_url})",
            "",
            "Note: schema v2 decisions use paired relative and absolute bounds. A suite exit of "
            "zero does not by itself accept a candidate; acceptance is an explicit report outcome "
            "after mirrored improvement and mandatory correctness gates.",
        ]
    )
    return "\n".join(lines) + "\n"


def render_comment(
    report: dict[str, Any] | None,
    *,
    run_url: str,
    artifact_name: str,
    marker: str = MARKER,
    title: str = DEFAULT_TITLE,
    process_exit_code: int | None = None,
) -> str:
    if report is None:
        return _render_v1_comment(
            report,
            run_url=run_url,
            artifact_name=artifact_name,
            marker=marker,
            title=title,
            process_exit_code=process_exit_code,
        )

    schema = report.get("schema_version")
    if schema == 1 and not isinstance(schema, bool):
        return _render_v1_comment(
            report,
            run_url=run_url,
            artifact_name=artifact_name,
            marker=marker,
            title=title,
        )
    if schema == 2 and not isinstance(schema, bool):
        return _render_v2_comment(
            report,
            run_url=run_url,
            artifact_name=artifact_name,
            marker=marker,
            title=title,
            process_exit_code=process_exit_code,
        )
    return _render_unsupported_schema(
        report,
        run_url=run_url,
        artifact_name=artifact_name,
        marker=marker,
        title=title,
    )


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
    ap.add_argument(
        "--process-exit-code",
        type=int,
        help="Actual compare_self.py exit code for schema v2 contract validation.",
    )
    args = ap.parse_args(argv)

    report = load_report(Path(args.json))
    body = render_comment(
        report,
        run_url=args.run_url,
        artifact_name=args.artifact,
        marker=args.marker,
        title=args.title,
        process_exit_code=args.process_exit_code,
    )
    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(body, encoding="utf-8")
    print("Wrote:", out_path)
    contract_errors = _comment_contract_errors(
        report,
        process_exit_code=args.process_exit_code,
    )
    if contract_errors:
        print("Performance comment contract failed: " + "; ".join(contract_errors))
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
