#!/usr/bin/env python3
"""Plan, collect, and verify Merman performance evidence."""

from __future__ import annotations

import argparse
import sys
from importlib import import_module


COMMANDS = {
    "plan": ("workloads", "plan", "Freeze ordered workload inputs"),
    "rust": ("workloads", "rust", "Observe Rust clock, memory, or work"),
    "stages": ("stages", None, "Attribute Rust time and allocations by phase"),
    "layouts": ("workloads", "rust-layouts", "Compare native layout backends"),
    "node": ("workloads", "node-matrix", "Compare Node products in balanced rounds"),
    "cli": ("cli", None, "Measure CLI subprocesses"),
    "memory": ("memory", None, "Measure native memory scale curves"),
    "ab": ("ab", None, "Compare matched Criterion executables"),
    "compare": ("workloads", "compare", "Compare workload reports"),
    "coverage": ("workloads", "coverage", "Inspect workload coverage"),
    "drawings": ("workloads", "drawings", "Validate measured SVG drawings"),
    "gallery": ("workloads", "layout-gallery", "Write an offline layout gallery"),
    "report": ("workloads", "report", "Write a Markdown evidence report"),
    "calibrate-layout": ("layout_calibration", None, "Calibrate layout work ceilings"),
    "criterion-list": ("criterion", None, "Check registered Criterion benchmark IDs"),
    "sync-fixtures": ("pipeline_fixtures", None, "Check or refresh compiled fixtures"),
    "ci": ("ci", None, "Plan or admit a CI performance lane"),
    "weekly": ("weekly", None, "Record the weekly workload baseline"),
    "summary": ("comment", None, "Render a workflow measurement summary"),
}


def main(argv: list[str] | None = None) -> int:
    args = sys.argv[1:] if argv is None else argv
    parser = argparse.ArgumentParser(
        description=__doc__,
        epilog="Commands\n" + "\n".join(
            f"  {command:<18} {description}" for command, (_, _, description) in COMMANDS.items()
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("command", choices=COMMANDS, metavar="COMMAND")
    if not args:
        parser.print_help()
        return 2
    command = parser.parse_args(args[:1]).command
    module_name, action, _ = COMMANDS[command]
    module = import_module(f"merman_bench.{module_name}")
    forwarded = args[1:] if action is None else [action, *args[1:]]
    return module.main(forwarded)


if __name__ == "__main__":
    raise SystemExit(main())
