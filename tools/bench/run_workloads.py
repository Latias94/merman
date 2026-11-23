"""Freeze ordered inputs, observe public Rust calls, and compare matching observations."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
from statistics import median
import subprocess
import sys

from corpus_utils import load_corpus, select_corpus_fixtures
from workload_topology import topology_source


ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
OPERATIONS = (
    "svg-data",
    "svg",
    "layout-json",
    "prepare-drop",
    "prepare-retained",
    "static-svg",
    "resvg-svg",
)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(path):
    return json.loads(path.read_bytes())


def write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(data, ensure_ascii=False, indent=2, allow_nan=False) + "\n",
        encoding="utf-8",
        newline="\n",
    )


def scenario(name, origin, sources, labels=None, *, allow_unlabeled=False):
    if not re.fullmatch(r"[a-zA-Z0-9_-]+", name):
        raise ValueError(f"invalid scenario ID: {name!r}")
    requests = []
    for i, (fixture, text) in enumerate(sources):
        if not text.strip():
            raise ValueError(f"empty source in {name}: {fixture}")
        request = {
            "fixture": fixture,
            "source": text,
            "source_sha256": digest(text.encode()),
            "diagram_id": f"wb-{name}-{i + 1}",
        }
        if labels is not None and (fixture in labels or not allow_unlabeled):
            expected = labels.get(fixture)
            if not isinstance(expected, list) or not all(
                isinstance(label, str) and label.strip() for label in expected
            ):
                raise ValueError(f"review expected labels for {name}/{fixture}")
            request["expected_labels"] = expected
        requests.append(request)
    if not requests:
        raise ValueError(f"empty scenario: {name}")
    return {"id": name, "origin": origin, "requests": requests}


def materialize(args):
    catalog = load(HERE / "workloads.json")
    corpus = load_corpus(HERE / "corpus.json")
    registered = {f.name: ROOT / f.source for f in corpus.fixtures}
    for name, path in catalog["fixture_files"].items():
        if name in registered:
            raise ValueError(f"workload file shadows a registered corpus fixture: {name}")
        registered[name] = ROOT / path
    inputs = dict(catalog["inputs"])
    for variant in ("repeated", "unique"):
        messages = (f"A->>B: event-{i if variant == 'unique' else 0:03d}\n" for i in range(100))
        inputs[f"sequence-100-{variant}"] = "sequenceDiagram\n" + "".join(messages)
    if inputs.keys() & registered.keys():
        raise ValueError("workload input shadows a fixture file")
    cases = []
    for recipe in catalog["scenarios"]:
        if args.suite == "quick" and not recipe.get("quick") and recipe["id"] not in args.scenario:
            continue
        if "document" in recipe:
            path = recipe["document"]
            text = (ROOT / path).read_text(encoding="utf-8")
            # The curated guide uses standalone fences. Bind labels to occurrences in reading order.
            blocks = re.findall(
                r"^```mermaid\n(.*?)^```[ \t]*$", text, flags=re.MULTILINE | re.DOTALL
            )
            labels = recipe["expected_labels"]
            if len(blocks) != len(labels):
                raise ValueError(f"review the changed fence population in {path}")
            sources = [(f"block-{i}", block) for i, block in enumerate(blocks, 1)]
            cases.append(scenario(recipe["id"], path, sources, labels))
            continue
        sequence = recipe["fixtures"] * recipe.get("repeat", 1)
        for fixture in sequence:
            if fixture not in inputs:
                inputs[fixture] = registered[fixture].read_text(encoding="utf-8")
        cases.append(
            scenario(
                recipe["id"],
                recipe.get("origin", "merman"),
                [(f, inputs[f]) for f in sequence],
                catalog["expected_labels"],
            )
        )

    for suite in corpus.suites:
        name = f"corpus-{suite}"
        if name in args.scenario:
            fixtures = select_corpus_fixtures(corpus, suite)
            sources = [(f.name, registered[f.name].read_text(encoding="utf-8")) for f in fixtures]
            cases.append(scenario(name, f"tools/bench/corpus.json#{suite}", sources,
                                  catalog["expected_labels"], allow_unlabeled=True))

    for manifest in args.document:
        doc = load(manifest)
        labels = doc.get("expected_labels", {})
        if not isinstance(labels, dict) or labels.keys() - set(doc["files"]):
            raise ValueError(f"expected_labels must refer to files in {manifest}")
        cases.append(
            scenario(
                doc["id"],
                doc["origin"],
                [(f, (manifest.parent / f).read_text(encoding="utf-8")) for f in doc["files"]],
                labels, allow_unlabeled=True,
            )
        )
    if args.pele_fixtures:
        upstream = load(args.pele_fixtures / "manifest.json")
        for entry in upstream["diagrams"]:
            name = entry["id"].replace("/", "-")
            cases.append(
                scenario(
                    name,
                    upstream["source"],
                    [(entry["id"], (args.pele_fixtures / entry["file"]).read_text(encoding="utf-8"))],
                )
            )

    topologies = []
    spec = catalog["topologies"][args.suite]
    for shape in spec["shapes"]:
        for nodes in spec["nodes"]:
            source, edges = topology_source(shape, nodes)
            name = f"{shape}-{nodes}"
            topologies.append(
                {
                    "id": name,
                    "nodes": nodes,
                    "edges": edges,
                    "source": source,
                    "source_sha256": digest(source.encode()),
                }
            )
            if name in args.scenario:
                cases.append(scenario(name, "merman/topology", [(name, source)]))
    names = [s["id"] for s in cases]
    if len(names) != len(set(names)):
        raise ValueError("duplicate scenario ID in workload sources")
    if args.scenario:
        by_id = {s["id"]: s for s in cases}
        cases = [by_id[name] for name in args.scenario]
        topologies = [row for row in topologies if row["id"] in args.scenario]
    return {"schema_version": 1, "suite": args.suite, "scenarios": cases, "topologies": topologies}


def build_runner(metric):
    name = "public_workload_memory" if metric == "memory" else "public_workloads"
    # Path dependencies have the same crate names and versions in two revision worktrees.
    # Keep a reusable build slot per checkout even when the caller normally shares targets.
    parent = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    target = (ROOT / parent).resolve() / "public-workloads" / digest(str(ROOT).encode())[:12]
    cmd = [
        "cargo",
        "build",
        "--locked",
        "--release",
        "--target-dir",
        str(target),
        "-p",
        "merman",
        "--bench",
        name,
        "--message-format=json-render-diagnostics",
    ]
    result = subprocess.run(cmd, cwd=ROOT, stdout=subprocess.PIPE, check=True)
    artifacts = [json.loads(line) for line in result.stdout.splitlines()]
    paths = [
        a["executable"]
        for a in artifacts
        if a.get("reason") == "compiler-artifact" and a["target"]["name"] == name and a["executable"]
    ]
    if len(paths) != 1:
        raise ValueError(f"expected one executable for {name}: {paths}")
    return Path(paths[0])


def child(runner, job):
    result = subprocess.run(
        [runner, "--workload-stdin"], input=json.dumps(job).encode(), stdout=subprocess.PIPE, check=True
    )
    return json.loads(result.stdout)


def run_rust(args):
    raw = args.plan.read_bytes()
    plan = json.loads(raw)
    if plan["schema_version"] != 1:
        raise ValueError("unsupported workload plan")
    work = args.metric == "work"
    if work and (args.site_layout or args.site_css_bytes or args.operation != "svg-data"):
        raise ValueError(
            "work observes default Flowchart geometry and accepts no site or operation override"
        )
    site = {}
    if args.site_css_bytes:
        rule = ".r{opacity:0.8;}"
        count, pad = divmod(args.site_css_bytes, len(rule))
        site["themeCSS"] = rule * count + " " * pad
    if args.site_layout:
        site["layout"] = args.site_layout
    selected = plan["topologies" if work else "scenarios"]
    by_id = {r["id"]: r for r in selected}
    if not selected or len(by_id) != len(selected):
        raise ValueError("empty selection or duplicate workload ID")
    if args.scenario:
        selected = [by_id[name] for name in args.scenario]
    runner = args.runner.resolve() if args.runner else build_runner(args.metric)
    options = {
        "metric": args.metric,
        "operation": "layout-work" if work else args.operation,
        "warmups": 0 if work else args.warmups,
        "site_css_bytes": args.site_css_bytes,
        "site_layout": args.site_layout,
        "samples": 1 if work else args.samples,
    }
    rows = {}
    fingerprints = set()

    def observe(job):
        row = child(runner, job)
        fingerprints.add(row.pop("harness_sha256"))
        if len(fingerprints) != 1:
            raise ValueError("workload runner changed during observation")
        return row

    for case in selected:
        print(case["id"], file=sys.stderr, flush=True)
        if work:
            if digest(case["source"].encode()) != case["source_sha256"]:
                raise ValueError(f"input digest mismatch in {case['id']}")
            rows[case["id"]] = observe({"task": "topology", **case})
            continue
        requests = case["requests"]
        if any(digest(r["source"].encode()) != r["source_sha256"] for r in requests):
            raise ValueError(f"input digest mismatch in {case['id']}")
        job = {
            "task": "measure",
            "metric": args.metric,
            "operation": args.operation,
            "requests": requests,
            "warmups": args.warmups,
            "site": site or None,
        }
        signatures = observe({**job, "verify_only": True})["outputs"]
        sizes = [s["observed_size"] for s in signatures]
        samples = [observe(job) for _ in range(args.samples)]
        if (
            len(signatures) != len(requests)
            or any(s["sizes"] != sizes for s in samples)
            or observe({**job, "verify_only": True})["outputs"] != signatures
        ):
            raise ValueError(f"unstable output in {case['id']}")
        rows[case["id"]] = {
            "completed": len(requests),
            "outputs": signatures,
            "samples": [s["metrics"] for s in samples],
        }
    return {
        "schema_version": 1,
        "surface": "rust",
        "options": options,
        "plan_sha256": digest(raw),
        "host": [platform.system(), platform.machine(), platform.node()],
        "runner_sha256": digest(runner.read_bytes()),
        "harness_sha256": fingerprints.pop(),
        "revision": args.revision
        or (
            None
            if args.runner
            else subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        ),
        "rows": rows,
    }


def compare(base, head):
    if base.get("surface") != "rust" or head.get("surface") != "rust":
        from workload_reference import compare_reference

        return compare_reference(base, head)
    for key in ("schema_version", "surface", "plan_sha256", "options", "host"):
        if base[key] != head[key]:
            raise ValueError(f"incompatible reports: {key}")
    if list(base["rows"]) != list(head["rows"]):
        raise ValueError("different workload selections")
    if base["harness_sha256"] != head["harness_sha256"]:
        return {
            "schema_version": 1,
            "classification": "harness-changed",
            "plan_sha256": base["plan_sha256"],
            "base_harness": base["harness_sha256"],
            "head_harness": head["harness_sha256"],
            "rows": {},
        }
    differences = {}
    for name, old in base["rows"].items():
        new = head["rows"][name]
        if base["options"]["metric"] == "work":
            a = (old["boundary"] or {}).get("minimum_passing_budget")
            b = (new["boundary"] or {}).get("minimum_passing_budget")
            same = old["high_budget"] == new["high_budget"] and old["high"] == new["high"]
            differences[name] = {
                "status": "same-high-budget-result" if same else "output-changed",
                "default_status": [old["default"]["status"], new["default"]["status"]],
                "base_minimum": a,
                "head_minimum": b,
                "delta": b - a if same and a is not None and b is not None else None,
            }
            continue
        if old["outputs"] != new["outputs"]:
            differences[name] = {"status": "output-changed"}
            continue
        metrics = {}
        for k in old["samples"][0]:
            a, b = (median(s[k] for s in r["samples"]) for r in (old, new))
            metrics[k] = {"base": a, "head": b, "delta": b - a}
            if a > 0:
                metrics[k]["change_percent"] = 100 * (b - a) / a
        differences[name] = {"status": "same-output", "completed": new["completed"], "metrics": metrics}
    return {
        "schema_version": 1,
        "plan_sha256": base["plan_sha256"],
        "options": base["options"],
        "base_runner": base["runner_sha256"],
        "head_runner": head["runner_sha256"],
        "classification": "diagnostic",
        "rows": differences,
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    matrix = sub.add_parser("node-matrix", help="collect and compare the locked Node products")
    matrix.add_argument("--plan", type=Path, required=True)
    matrix.add_argument("--out-dir", type=Path, required=True, help="directory for a new result bundle")
    matrix.add_argument("--peer", action="append", help="select a registered peer (repeatable, default: all)")
    matrix.add_argument("--samples", type=int, default=3)
    matrix.add_argument("--timeout-seconds", type=float, default=300, help="time budget per scenario")
    matrix.add_argument("--merman-module", type=Path, help="public Node entry of a local Merman candidate")
    plan = sub.add_parser("plan", help="freeze portable source bytes and occurrence IDs")
    plan.add_argument("--suite", choices=["quick", "full"], default="quick")
    plan.add_argument(
        "--document", type=Path, action="append", default=[], help="ordered local .mmd file manifest"
    )
    plan.add_argument(
        "--pele-fixtures", type=Path, help="pinned mermaid-v13 packages/bench/fixtures/pele directory"
    )
    run = sub.add_parser("rust", help="run each sample in a fresh child process")
    run.add_argument("--plan", required=True, type=Path)
    run.add_argument("--metric", choices=["clock", "memory", "work"], default="memory")
    run.add_argument("--operation", choices=OPERATIONS, default="svg-data")
    run.add_argument(
        "--warmups", type=int, default=1, help="untimed workload passes before each observation"
    )
    run.add_argument(
        "--samples",
        type=int,
        default=3,
        help="fresh processes per workload (work is exact and runs once)",
    )
    run.add_argument("--site-css-bytes", type=int, default=0)
    run.add_argument("--site-layout", choices=["dagre", "elk"])
    run.add_argument(
        "--runner", type=Path, help="use a frozen executable built with the matching metric"
    )
    run.add_argument("--revision", help="revision label for a frozen runner")
    diff = sub.add_parser("compare", help="compare two reports for the same surface and plan")
    diff.add_argument("base", type=Path)
    diff.add_argument("head", type=Path)
    coverage = sub.add_parser("coverage", help="inventory planned sources and verified labels")
    coverage.add_argument("--plan", required=True, type=Path, help="frozen workload plan")
    coverage.add_argument(
        "--comparison", type=Path, action="append", default=[],
        help="external workload comparison JSON (repeatable)",
    )
    coverage.add_argument("--out", type=Path, help="write JSON to a file (default: stdout)")
    for command in (plan, run):
        command.add_argument(
            "--scenario", action="append", default=[], help="select an ID (repeatable)"
        )
    for command in (plan, run, diff):
        command.add_argument("--out", required=True, type=Path)
    args = parser.parse_args(argv)
    if hasattr(args, "scenario") and len(set(args.scenario)) != len(args.scenario):
        parser.error("a scenario may only be selected once")
    if args.action == "rust" and (args.samples <= 0 or args.warmups < 0 or args.site_css_bytes < 0):
        parser.error("samples must be positive; warmups and site CSS size must be nonnegative")
    try:
        if args.action == "node-matrix":
            from workload_matrix import collect_node_matrix

            print(collect_node_matrix(args))
            return 0
        if args.action == "plan":
            result = materialize(args)
        elif args.action == "rust":
            result = run_rust(args)
        elif args.action == "coverage":
            from workload_coverage import summarize_coverage

            comparisons = (load(path) for path in args.comparison)
            result = summarize_coverage(args.plan.read_bytes(), comparisons)
        else:
            result = compare(load(args.base), load(args.head))
        if args.out:
            write(args.out, result)
        else:
            text = json.dumps(result, ensure_ascii=False, indent=2, allow_nan=False) + "\n"
            sys.stdout.buffer.write(text.encode("utf-8"))
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")
    return 0


if __name__ == "__main__":
    main()
