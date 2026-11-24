"""Attribute the public SVG operation across selected layout backends."""

from __future__ import annotations

import argparse
from pathlib import Path
import subprocess

from .workloads import LAYOUTS, build_runner, digest, run_rust, write, write_chrome_trace


def parse_layouts(value: str) -> list[str]:
    layouts = [layout.strip() for layout in value.split(",")]
    if any(layout not in LAYOUTS for layout in layouts) or len(set(layouts)) != len(layouts):
        raise argparse.ArgumentTypeError("select distinct layouts from dagre, elk, shoal")
    return layouts


def collect(args: argparse.Namespace) -> tuple[dict, bool]:
    raw_plan = args.plan.read_bytes()
    shoal = "shoal" in args.layouts
    runner = args.runner or build_runner("stages", shoal=shoal)
    memory_runner = None
    if args.with_memory:
        memory_runner = args.memory_runner or build_runner("stage-memory", shoal=shoal)
    runner = runner.resolve()
    if memory_runner is not None:
        memory_runner = memory_runner.resolve()
    runner_hash = digest(runner.read_bytes())
    memory_hash = digest(memory_runner.read_bytes()) if memory_runner else None
    reports = {}
    for layout in args.layouts:
        options = argparse.Namespace(
            plan=args.plan,
            scenario=args.scenario,
            warmups=args.warmups,
            samples=args.samples,
            metric="stages",
            operation="svg-data",
            site_layout=layout,
            site_css_bytes=0,
            resource_profile=args.resource_profile,
            verify_measured_svg=False,
            check_drawings=False,
            chrome=None,
            chrome_trace=args.chrome_trace,
            runner=args.runner,
            revision=None,
            with_memory=args.with_memory,
            memory_runner=args.memory_runner,
        )
        report = run_rust(options, raw=raw_plan, runner=runner, memory_runner=memory_runner)
        if (report["runner_sha256"] != runner_hash or report.get("memory_runner_sha256") != memory_hash):
            raise ValueError("workload executable changed during the stage collection")
        reports[layout] = report
    scenarios = {
        name: {
            "cross_backend_eligible": len(reports) > 1 and all(
                report["rows"][name].get("status") is None
                and report["rows"][name]["stage_layout"]["cross_backend_eligible"]
                for report in reports.values()
            ),
        }
        for name in next(iter(reports.values()))["rows"]
    }
    rows = {
        f"{layout}/{name}": row
        for layout, report in reports.items()
        for name, row in report["rows"].items()
    }
    if args.chrome_trace:
        write_chrome_trace(args.chrome_trace, {"rows": rows})
    success = all(row.get("status") in (None, "expected-rejection") for row in rows.values())
    return {"schema_version": 1, "surface": "rust-stages", "layouts": reports, "scenarios": scenarios}, success


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="bench.py stages", description=__doc__)
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument("--layouts", type=parse_layouts, default=list(LAYOUTS))
    parser.add_argument("--scenario", action="append", default=[])
    parser.add_argument("--with-memory", action="store_true")
    parser.add_argument("--runner", type=Path, help="reuse a workload timing runner")
    parser.add_argument("--memory-runner", type=Path, help="reuse a stage allocation runner")
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--resource-profile", choices=("interactive", "constrained"))
    parser.add_argument("--chrome-trace", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)
    if args.samples < 1 or args.warmups < 0:
        parser.error("samples must be positive and warmups must be nonnegative")
    if len(args.scenario) != len(set(args.scenario)):
        parser.error("a scenario may only be selected once")
    if args.memory_runner and not args.with_memory:
        parser.error("--memory-runner requires --with-memory")
    inputs = {path.resolve() for path in (args.plan, args.runner, args.memory_runner) if path}
    outputs = {args.out.resolve()}
    if args.chrome_trace:
        outputs.add(args.chrome_trace.resolve())
    if inputs & outputs or len(outputs) != (2 if args.chrome_trace else 1):
        parser.error("choose report and trace paths distinct from each other and the inputs")
    try:
        report, success = collect(args)
        write(args.out, report)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")
    return int(not success)
