from collections import Counter
from contextlib import redirect_stdout
from copy import deepcopy
import hashlib
import io
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from bench import main as bench_main
from merman_bench.drawing import OBSERVER, audit_drawings
from merman_bench.node import _variant_recipes, collect_node_matrix
from merman_bench.reference import NODE_PRODUCTS, compare_reference
from merman_bench.report_node import load_matrix, load_rounds


HERE = Path(__file__).resolve().parent
SCRATCH = HERE.parents[1] / "target"

def browser_reply(rows, **changes):
    data = dict(browser="Chrome/test", observer_sha256=hashlib.sha256(OBSERVER.read_bytes()).hexdigest(), results=rows)
    return subprocess.CompletedProcess([], 0, json.dumps({**data, **changes}).encode(), b"")


def node_report(surface, engine, times=((1, 100), (2, 2), (100, 1)), fmt="svg", *, wasm=False):
    node = surface == "node"
    scale = 1 if engine == "merman" else 2
    requests = [
        dict(fixture="shared", diagram_id=f"id-{i}", source_sha256="a" * 64)
        for i in range(len(times[0]))
    ]
    requests[0]["expected_labels"] = ["Alpha"]
    row = dict(id="doc", requests=requests)
    data = dict(
        schema_version=1,
        surface=surface,
        engine=engine,
        plan_sha256="b" * 64,
        collector_sha256="c" * 64,
        samples=len(times),
        host=dict(hostname="bench", platform="win32", arch="x64"),
        scenarios=[row],
    )
    if fmt == "png":
        data["png_decoder"] = {"name": "Pillow", "version": "test"}
    batches = []
    for sample in times:
        results = []
        for i, elapsed in enumerate(sample):
            item = dict(status="ok", elapsed_ms=elapsed * scale)
            item["output_bytes" if node else "bytes"] = 100 * scale + i
            item["output_sha256" if node else "sha256"] = str(scale) * 64
            if fmt == "png":
                item.update(width=300 * scale, height=200)
            results.append(item)
        batches.append(
            {"results": results, "complete_batch_ms" if node else "wall_ms": (sum(sample) + 10) * scale}
        )
    if node:
        merman = engine == "merman"
        scope = "node-entry-and-transport" if merman else "node-entry"
        if wasm:
            package, backend, roles = "@mermanjs/node-wasm", "node-wasm", ["entry", "wasm-glue", "wasm-module"]
        elif merman:
            package, backend, roles = "@mermanjs/node", "napi", ["entry", "native-addon"]
        else:
            package, backend, roles = "pele", "javascript", ["entry"]
        data.update(
            output_format=fmt,
            process_lifecycle="fresh-per-scenario",
            backend=backend,
            package=dict(name=package, version="1.2.3",
                         entry=f"/products/{engine}/entry", entry_sha256="f" * 64),
            render_api="MermanEngine.renderSvgSync" if merman else "render",
        )
        cold = deepcopy(batches[0])
        for item in cold["results"]:
            item["elapsed_ms"] = 0.01
        cold["complete_batch_ms"] = 1
        row.update(cold_batch=cold, warm_samples=batches)
    else:
        scope, roles = "command-prefix", ["executable"]
        data.update(
            format=fmt,
            binary=engine,
            command_prefix=[f"/products/{engine}/executable"],
            version="1.2.3",
            version_probe=dict(status="ok"),
            process_lifecycle="fresh-process-per-diagram",
            timeout_seconds=60,
            font_files=[],
            extra_args=[],
            render_environment={},
        )
        row["batches"] = batches
    data["artifact_identity"] = dict(scope=scope, status="recorded", files=[
        dict(role=role, path=f"/products/{engine}/{role}", bytes=7, sha256="f" * 64) for role in roles
    ])
    return data


class NodeMatrixTests(unittest.TestCase):


    def test_only_matching_passed_drawings_release_a_declared_scenario_ratio(self):
        for left, right in [("passed", "passed"), ("passed", "failed"), ("unavailable", "passed"), (None, None)]:
            products = {name: {"doc": {"status": state}} for name, state in (("merman", left), ("pele", right)) if state}
            raw = [node_report("node", name) for name in ("merman", "pele")]
            for report in raw:
                report["scenarios"][0]["requests"][0]["drawing_required"] = True
            result = compare_reference(*raw, drawing_products=products)
            row = result["scenarios"][0]
            self.assertEqual(row["drawing_checks"], {"left": left or "not-checked", "right": right or "not-checked"})
            for key in ("complete_batch_ms", "common_render_ms"):
                self.assertGreater(row[key]["left"], 0)
                self.assertEqual(row[key]["right_over_left"] is not None, left == right == "passed")

    def test_failed_peer_keeps_the_other_comparison_and_raw_inputs(self):
        SCRATCH.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=SCRATCH) as tmp:
            root = Path(tmp)
            source = "flowchart LR\nA[Alpha] --> B"
            request = dict(fixture="example", diagram_id="id-0", source_sha256=hashlib.sha256(source.encode()).hexdigest(),
                           expected_labels=["Alpha"])
            data = dict(schema_version=1, scenarios=[dict(id="doc", requests=[dict(request, source=source)])], topologies=[])
            plan_bytes = json.dumps(data).encode()
            args = SimpleNamespace(plan=root / "input.json", out_dir=root / "results", peer=None, samples=2,
                                   timeout_seconds=30, merman_module=None, check_drawings=False, chrome=None)
            args.plan.write_bytes(plan_bytes)
            order = []

            def collect(cmd, *, cwd, stdout):
                name = cmd[cmd.index("--engine") + 1]
                order.append(name)
                if name == "pele":
                    stdout.write(b'{"error":"could not load the package"}')
                    return SimpleNamespace(returncode=1)
                report = node_report("node", "merman" if name == "merman" else "pele", [(1,), (2,)])
                manifest = json.loads((cwd / "package.json").read_bytes())["dependencies"]
                package = "@mermanjs/node" if name == "merman" else "mermaid"
                report["package"].update(name=package, version=manifest[package])
                report.update(engine=name, plan_sha256=hashlib.sha256(plan_bytes).hexdigest(),
                              node_project={"cwd": str(cwd)})
                if name == "mermaid-v13":
                    report["render_api"] = "mermaid.render"
                report["scenarios"][0]["requests"] = [request]
                stdout.write(json.dumps(report).encode())
                return SimpleNamespace(returncode=0)

            with patch("merman_bench.node.subprocess.run", side_effect=collect):
                with self.assertRaisesRegex(ValueError, "incomplete Node collection"):
                    collect_node_matrix(args)
                with self.assertRaises(FileExistsError):
                    collect_node_matrix(args)
            self.assertEqual(order, ["merman", "pele", "mermaid-v13"])
            self.assertEqual((args.out_dir / "plan.json").read_bytes(), plan_bytes)
            coverage = json.loads((args.out_dir / "coverage.json").read_bytes())
            self.assertEqual(coverage["collection"]["status"], "incomplete")
            self.assertEqual(coverage["collection"]["errors"], {"pele": "could not load the package"})
            self.assertEqual(coverage["comparisons"][0]["right"]["engine"], "mermaid-v13")
            self.assertFalse((args.out_dir / "merman-vs-pele.json").exists())
            self.assertIn("| doc | 1/1 | 1 |", (args.out_dir / "summary.md").read_text(encoding="utf-8"))

    def test_matrix_preserves_drawings_and_both_pairs_across_throw_exit_and_timeout(self):
        SCRATCH.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=SCRATCH) as tmp:
            root = Path(tmp)
            project = root / "node-reference"
            project.mkdir()
            for file in ("collectors/node_runner.mjs", "workloads.json", "merman_bench/svg.py"):
                (root / file).parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(HERE / file, root / file)
            for file in ("package.json", "package-lock.json"):
                shutil.copyfile(HERE / "node-reference" / file, project / file)
            versions = json.loads((project / "package.json").read_bytes())["dependencies"]
            svg = "'<svg><text>Alpha</text></svg>'"
            for name, code in {
                "@mermanjs/node": f"""export function createNodeEngine() {{ return {{
                    runtimeCatalog: {{package_version: {json.dumps(versions['@mermanjs/node'])}}},
                    renderSvgSync(source, {{optionsJson}}) {{
                        if (source === 'throw') throw new Error('test render exception');
                        if (source === 'exit') process.exit(73);
                        if (source === 'timeout') Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0);
                        const {{svg: {{diagram_id}}}} = JSON.parse(optionsJson);
                        return {svg}.replace('<svg>', '<svg id="' + diagram_id + '">');
                    }}, dispose() {{}}
                }}; }}""",
                "pele": f"export const render = () => ({{ svg: {svg} }});",
                "mermaid": f"export default {{ async render() {{ return {{ svg: {svg} }}; }} }};",
            }.items():
                pkg = project / "node_modules" / name
                pkg.mkdir(parents=True)
                (pkg / "package.json").write_text(json.dumps(dict(name=name, version=versions[name], exports="./index.mjs")))
                (pkg / "index.mjs").write_text(code)

            names = ["before", "throw", "exit", "timeout", "after"]
            rows = [dict(id=name, requests=[dict(source=name, fixture="one", diagram_id=name, expected_labels=["Alpha"],
                                                  drawing_facts={"label_widths": {"A": "Alpha"}})]) for name in names]
            plan = root / "input.json"
            plan.write_text(json.dumps(dict(schema_version=1, scenarios=rows)))
            dest = root / "bundle"
            args = SimpleNamespace(plan=plan, out_dir=dest, peer=None, samples=1, timeout_seconds=4,
                                   merman_module=None, check_drawings=True, chrome=None)
            def audit(plan, reports, *, chrome):
                # Only substitute the browser verdicts. The audit reads the actual retained SVGs.
                fact = dict(kind="label-width", node="A", label="Alpha", status="unavailable")
                reply = browser_reply([[fact]] * (2 + 2 * len(names)))
                with patch("merman_bench.drawing.subprocess.run", return_value=reply):
                    return audit_drawings(plan, reports, chrome=chrome)

            with patch("merman_bench.node.HERE", root), patch("merman_bench.node.audit_drawings", audit):
                with self.assertRaisesRegex(ValueError, "incomplete Node collection"):
                    collect_node_matrix(args)
            coverage = json.loads((dest / "coverage.json").read_bytes())
            self.assertEqual(set(coverage["collection"]["errors"]), {"merman"})
            failures = coverage["collection"]["runs"]["merman"]["worker_failures"]
            self.assertEqual([(f["id"], f["status"]) for f in failures], [("exit", "failed"), ("timeout", "timeout")])
            products = json.loads((dest / "drawing-audit.json").read_bytes())["products"]
            for product in products:
                observed = [n for n, row in product["scenarios"].items() if "artifact" in row["cases"][0]]
                self.assertEqual(observed, ["before", "after"] if product["engine"] == "merman" else names)
            cases = products[0]["scenarios"]
            self.assertEqual([cases[n]["cases"][0]["reason"] for n in names[1:-1]], ["renderer-failed", "worker-failed", "worker-timeout"])
            self.assertIn("artifact", cases["after"]["cases"][0])
            for peer, summary in zip(("pele", "mermaid-v13"), coverage["comparisons"], strict=True):
                compared = json.loads((dest / f"merman-vs-{peer}.json").read_bytes())["scenarios"]
                self.assertEqual([r["id"] for r in compared], names)
                self.assertEqual(summary["unreported_scenarios"], [])
                self.assertEqual([r["requests"][0]["left"]["state"] for r in compared],
                                 ["stable", "missing", "unmeasured", "unmeasured", "stable"])
                self.assertTrue(all(r["requests"][0]["right"]["state"] == "stable" for r in compared))
                self.assertTrue(all(r["common_render_ms"] is None and r["complete_batch_ms"] is None for r in compared[1:-1]))
                self.assertIsNotNone(compared[-1]["complete_batch_ms"]["left"])
            markdown = (dest / "summary.md").read_text(encoding="utf-8")
            self.assertIn("timeout · worker timeout · requests unmeasured", markdown)
            self.assertIn("| after | 1/1 | 1 |", markdown)


class ProcessRoundTests(unittest.TestCase):


    def setUp(self):
        SCRATCH.mkdir(exist_ok=True)
        tmp = tempfile.TemporaryDirectory(dir=SCRATCH)
        self.addCleanup(tmp.cleanup)
        root = Path(tmp.name)
        self.args = SimpleNamespace(plan=root / "input.json", out_dir=root / "bundle", peer=None,
                                    process_rounds=2, samples=2, timeout_seconds=30,
                                    merman_module=None, check_drawings=False, chrome=None)
        self.freeze({"doc": ["z", "a"], "control": ["c"]})
        self.calls = []
        self.edit = lambda round_no, report: None
        collector = patch("merman_bench.node.subprocess.run", side_effect=self.collect)
        self.addCleanup(collector.stop)
        collector.start()

    def freeze(self, documents):
        scenarios = [dict(id=name, requests=[dict(fixture="shared", source=key, diagram_id=key, expected_labels=[key])
                                             for key in keys]) for name, keys in documents.items()]
        self.args.plan.write_bytes(json.dumps(dict(schema_version=1, scenarios=scenarios)).encode())

    def collect(self, cmd, *, cwd, stdout):
        engine = cmd[cmd.index("--engine") + 1]
        label = cmd[cmd.index("--variant-label") + 1] if "--variant-label" in cmd else engine
        round_no = int(Path(stdout.name).parent.name)
        self.calls.append((round_no, label))
        plan = Path(cmd[cmd.index("--plan") + 1]).read_bytes()
        self.assertEqual(plan, self.args.plan.read_bytes())
        samples = int(cmd[cmd.index("--samples") + 1])
        product = NODE_PRODUCTS[engine]
        package = product["package"]
        version = json.loads((cwd / "package.json").read_bytes())["dependencies"][package]
        raw = node_report("node", engine)
        entry = raw["artifact_identity"]["files"][0]
        if "--module" in cmd:
            entry["path"] = cmd[cmd.index("--module") + 1]
        raw.update(plan_sha256=hashlib.sha256(plan).hexdigest(), samples=samples, timeout_seconds=self.args.timeout_seconds,
                   package=dict(name=package, version=version, entry=entry["path"], entry_sha256=entry["sha256"]),
                   render_api=product["render_api"], node_project=dict(cwd=str(cwd), digests={
                       f: hashlib.sha256((cwd / f).read_bytes()).hexdigest() for f in ("package.json", "package-lock.json")}),
                   scenarios=[])
        if "--variant-label" in cmd:
            value = next((arg.removeprefix("--options-json=") for arg in cmd if arg.startswith("--options-json=")), None)
            raw["variant"] = dict(label=label, options_json=value)
            if "--site-layout" in cmd:
                raw["variant"]["site_layout"] = cmd[cmd.index("--site-layout") + 1]
        variant = raw.get("variant")
        omitted = engine == "merman" and variant is not None and variant["options_json"] is None
        raw["diagram_id_mode"] = "omitted" if omitted else "plan"
        raw["warmup"] = {f"max_{key}": int(cmd[cmd.index(f"--warmup-{key}") + 1]) if f"--warmup-{key}" in cmd else default
                         for key, default in (("calls", 20), ("ms", 150))}
        for index, scenario in enumerate(json.loads(plan)["scenarios"]):
            requests = scenario["requests"]
            times = [tuple(range(1, len(requests) + 1))] * samples
            row, = node_report("node", engine, times)["scenarios"]
            row.update(index=index, id=scenario["id"])
            for i, request in enumerate(requests):
                row["requests"][i] = {k: v for k, v in request.items() if k not in ("source", "drawing_facts")}
                row["requests"][i]["source_sha256"] = hashlib.sha256(request["source"].encode()).hexdigest()
                if "drawing_facts" in request:
                    row["requests"][i]["drawing_required"] = True
            if variant and "site_layout" in variant:
                row["layout_plans"] = [dict(diagram_type="flowchart-v2", required_capability_ids=[f"layout-{variant['site_layout']}"])
                                       for _ in requests]
            if omitted:
                for batch in [row["cold_batch"], *row["warm_samples"]]:
                    batch["elapsed_ms"] = batch["complete_batch_ms"]
                    batch["complete_batch_ms"] = None
                    for result in batch["results"]:
                        result["status"] = "diagram-id-not-applied"
            raw["scenarios"].append(row)
        raw["planned_scenarios"] = len(raw["scenarios"])
        self.edit(round_no, raw)
        stdout.write(json.dumps(raw).encode())
        return SimpleNamespace(returncode=int(bool(raw.get("error") or raw.get("status") == "failed")))

    def run_matrix(self, *, incomplete=False):
        if incomplete:
            with self.assertRaisesRegex(ValueError, "incomplete Node collection.*process-rounds.json"):
                collect_node_matrix(self.args)
        else:
            self.assertEqual(collect_node_matrix(self.args), self.args.out_dir / "summary.md")
        return json.loads((self.args.out_dir / "process-rounds.json").read_bytes())

    def publish_rounds(self):
        markdown = self.args.out_dir.parent / "reader.md"
        receipt = markdown.with_suffix(".json")
        with redirect_stdout(io.StringIO()):
            self.assertEqual(bench_main(["report", "--node-rounds", str(self.args.out_dir), "--date", "2026-11-25",
                                         "--out", str(markdown)]), 0)
            self.assertEqual(bench_main(["report", "--from", str(receipt), "--check"]), 0)
        return json.loads(receipt.read_bytes())["measurements"]["node_rounds"], markdown.read_text(encoding="utf-8")

    def test_order_and_paired_distribution_from_each_workers_warm_median(self):
        self.args.process_rounds = 6
        left = [2, 4, 10, 20, 30, 40]
        right = [8, 4, 2, 10, 60, 20]

        def set_times(round_no, raw):
            value = (left if raw["engine"] == "merman" else right)[round_no - 1]
            for row in raw["scenarios"]:
                row["cold_batch"]["complete_batch_ms"] = 50_000
                for batch, millis in zip(row["warm_samples"], (value - 1, value + 1), strict=True):
                    batch["complete_batch_ms"] = millis

        self.edit = set_times
        report = self.run_matrix()
        self.assertEqual((report["classification"], report["process_rounds"], report["warm_batches_per_worker"]),
                         ("diagnostic", 6, 2))
        observed = [r["order"] for r in report["rounds"]]
        self.assertEqual(self.calls, [(i, engine) for i, order in enumerate(observed, 1) for engine in order])
        self.assertEqual(len(set(map(tuple, observed))), 6)
        for forward, reverse in zip(observed[::2], observed[1::2], strict=True):
            self.assertEqual(forward, reverse[::-1])
        for positions in zip(*observed, strict=True):
            self.assertEqual(Counter(positions), {engine: 2 for engine in NODE_PRODUCTS})
        for peer in ("pele", "mermaid-v13"):
            row = report["comparisons"][peer]["doc"]
            self.assertEqual((row["status"], row["eligible_pairs"]), ("complete", 6))
            self.assertEqual(row["paired_right_over_left"], dict(min=0.2, median=0.75, max=4))
            self.assertEqual(row["direction_counts"], dict(below_one=3, equal_one=1, above_one=2))
            self.assertEqual(row["per_process_median_ms"]["left"], dict(min=2, median=15, max=40))
            self.assertEqual([x["right_over_left"] for x in row["rounds"]], [4, 1, 0.2, 0.5, 2, 0.5])
        for item in report["rounds"]:
            directory = (self.args.out_dir / item["coverage"]).parent
            compared = json.loads((directory / "merman-vs-pele.json").read_bytes())
            self.assertEqual([r["diagram_id"] for r in compared["scenarios"][0]["requests"]], ["z", "a"])
        summary = (self.args.out_dir / "summary.md").read_text(encoding="utf-8")
        self.assertIn("6 fresh workers per product and scenario", summary)
        self.assertIn("2 warm batches", summary)
        with self.assertRaises(FileExistsError):
            collect_node_matrix(self.args)

    def test_named_variants_use_balanced_default_rounds_and_keep_exact_options(self):
        module = self.args.plan.parent / "candidate entry.mjs"
        options = ' { "site_config": { "layout": "dagre" }, "svg": { "pipeline": "readable" } } '
        self.args.variant = ["base.0=merman::{}", f"explicit=merman:{module}:{options}"]
        self.args.variant_layout = ["base.0=dagre", "explicit=dagre"]
        self.args.process_rounds = None
        self.args.warmup_calls = 4
        plan = json.loads(self.args.plan.read_bytes())
        plan["scenarios"][1]["family"] = "flowchart"
        self.args.plan.write_bytes(json.dumps(plan).encode())

        def slower_variant(round_no, raw):
            if raw["variant"]["label"] == "explicit":
                for row in raw["scenarios"]:
                    for batch in row["warm_samples"]:
                        batch["complete_batch_ms"] *= 2

        self.edit = slower_variant
        report = self.run_matrix()
        self.assertEqual(self.calls, [(1, "base.0"), (1, "explicit"), (2, "explicit"), (2, "base.0")])
        self.assertEqual(report["left"], "base.0")
        self.assertEqual(set(report["reference_identities"]), {"base.0", "explicit"})
        self.assertEqual(report["comparisons"]["explicit"]["doc"]["eligible_pairs"], 2)
        recipes = json.loads((self.args.out_dir / "variants.json").read_bytes())
        self.assertEqual(recipes["variants"], [dict(label="base.0", engine="merman", module=None, options_json="{}", site_layout="dagre"),
                                               dict(label="explicit", engine="merman", module=str(module), options_json=options, site_layout="dagre")])
        self.assertEqual(recipes["warmup_overrides"], {"max_calls": 4})
        directory = self.args.out_dir / "rounds" / "001"
        comparison = json.loads((directory / "base.0-vs-explicit.json").read_bytes())
        self.assertEqual(comparison["render_settings"], "declared Node variants")
        self.assertEqual(comparison["left"]["variant"], dict(label="base.0", options_json="{}", site_layout="dagre"))
        self.assertEqual(comparison["right"]["variant"], dict(label="explicit", options_json=options, site_layout="dagre"))
        self.assertEqual(json.loads((directory / "explicit.json").read_bytes())["warmup"], {"max_calls": 4, "max_ms": 150})
        projected, markdown = self.publish_rounds()
        self.assertEqual(projected["baseline"], "base.0")
        for label in ("base.0", "explicit"):
            product = projected["products"][label]
            self.assertEqual((product["engine"], product["package"], product["backend"]), ("merman", "@mermanjs/node", "napi"))
            self.assertEqual(product["transport_sha256"], {"native-addon": "f" * 64})
            self.assertIn(f"{label} — Merman {product['version']} (N-API)", markdown)
        doc = next(row for row in projected["latency"] if row["scenario"] == "doc")
        self.assertEqual((doc["baseline_ms"], doc["peer_ms"], doc["ratio"]["median"]), (13, 26, 2))
        self.assertEqual(projected["coverage"]["explicit"]["families"]["baseline_stable"], 2)
        self.assertIn("The baseline variant is base.0.", markdown)
        self.assertIn("| base.0 ms | Peer ms | Peer / base.0 (range) |", markdown)
        self.assertIn("| doc | explicit | 2 / 2 | 13.000 | 26.000 | 2.000 (2.000–2.000) | Timing only. base.0 faster", markdown)
        self.assertIn("| doc | explicit | 2/2 |", markdown)
        self.assertNotIn(json.dumps(str(module)), json.dumps(projected))
        shutil.copyfile(self.args.out_dir / "plan.json", directory / "plan.json")
        with self.assertRaisesRegex(ValueError, "use --node-rounds with the finalized Node variant bundle"):
            load_matrix(directory / "coverage.json")

    def test_named_report_withholds_ratio_for_repeatable_but_different_svg(self):
        self.args.variant = ["direct=merman::{}", "with_json=merman::{ } "]
        self.args.variant_layout = ["direct=dagre", "with_json=dagre"]

        def change_svg(round_no, raw):
            if raw["variant"]["label"] == "with_json":
                doc = raw["scenarios"][0]
                for batch in [doc["cold_batch"], *doc["warm_samples"]]:
                    batch["results"][1]["output_sha256"] = "b" * 64

        self.edit = change_svg
        finalized = self.run_matrix()["comparisons"]["with_json"]
        self.assertEqual(finalized["doc"]["eligible_pairs"], 0)
        self.assertIn("different SVG bytes", finalized["doc"]["rounds"][0]["withheld_reason"])
        self.assertEqual(finalized["control"]["eligible_pairs"], 2)
        comparison = self.args.out_dir / "rounds" / "001" / "direct-vs-with_json.json"
        doc = json.loads(comparison.read_bytes())["scenarios"][0]
        self.assertEqual(doc["common_stable"], 2)
        self.assertEqual(doc["complete_batch_ms"], dict(left=13, right=13, right_over_left=None))

        projected, markdown = self.publish_rounds()
        rows = {row["scenario"]: row for row in projected["latency"]}
        self.assertEqual(projected["status"], "complete")
        self.assertEqual(projected["coverage"]["with_json"]["common_stable"], 6)
        self.assertEqual(rows["doc"]["reasons"], ["different-svg"])
        self.assertIsNone(rows["doc"]["ratio"])
        self.assertEqual(rows["control"]["ratio"]["median"], 1)
        self.assertIn(r"| doc | with\_json | 2 / 2 | — | — | — | Warm ratio withheld.", markdown)
        self.assertIn("Same-backend variants returned different SVG bytes", markdown)

    def test_named_rounds_check_every_finalized_product_identity(self):
        self.args.variant = ["upstream=pele", "current=merman::{}", "candidate=merman::{}"]
        finalized = self.run_matrix()
        bundle = self.args.out_dir
        rounds_path = bundle / "process-rounds.json"
        verified = load_rounds(rounds_path)
        self.assertEqual(verified["baseline"], "upstream")
        self.assertEqual(list(verified["products"]), ["upstream", "current", "candidate"])
        self.assertEqual(verified["products"]["upstream"]["engine"], "pele")
        self.assertTrue(all(row["eligible_pairs"] == 2 for row in verified["latency"]))

        for label in verified["products"]:
            with self.subTest(label=label):
                wrong = deepcopy(finalized)
                files = wrong["reference_identities"][label]["product"]["artifact_identity"]["files"]
                files[-1]["sha256"] = "d" * 64
                rounds_path.write_bytes(json.dumps(wrong).encode())
                projected = load_rounds(rounds_path)
                self.assertEqual(projected["status"], "incomplete")
                for row in projected["latency"]:
                    if label in ("upstream", row["peer"]):
                        self.assertEqual(row["reasons"], ["evidence-mismatch"])
                        self.assertIsNone(row["ratio"])
                    else:
                        self.assertEqual(row["eligible_pairs"], 2)
                self.assertTrue(all(row["workers"] == (0 if row["variant"] == label else 2)
                                    for row in projected["startup"]))

        rounds_path.write_bytes(json.dumps(finalized).encode())
        worker_path = bundle / "rounds" / "002" / "candidate.json"
        worker = json.loads(worker_path.read_bytes())
        worker["artifact_identity"]["files"][-1]["sha256"] = "d" * 64
        worker_path.write_bytes(json.dumps(worker).encode())
        projected = load_rounds(rounds_path)
        for row in projected["latency"]:
            self.assertEqual(row["eligible_pairs"], 1 if row["peer"] == "candidate" else 2)
            if row["peer"] == "candidate":
                self.assertEqual(row["reasons"], ["evidence-mismatch"])
                self.assertIsNone(row["ratio"])

    def test_replay_rechecks_each_scenario_against_its_raw_requests_layout_and_batch(self):
        self.args.variant = ["matched=merman::{}", "peer=pele"]
        self.args.variant_layout = ["matched=dagre"]
        self.run_matrix()
        bundle = self.args.out_dir
        report = bundle / "rounds" / "002" / "matched.json"
        original = json.loads(report.read_bytes())
        self.assertTrue(all(row["eligible_pairs"] == 2 for row in load_rounds(bundle)["latency"]))

        for change in ("source", "layout", "batch", "old-collector"):
            with self.subTest(change=change):
                raw = deepcopy(original)
                row = raw["scenarios"][0]
                if change == "source":
                    row["requests"][0]["source_sha256"] = "0" * 64
                elif change == "layout":
                    row["layout_plans"][0]["required_capability_ids"] = []
                elif change == "batch":
                    row["warm_samples"][0]["complete_batch_ms"] += 1
                else:
                    del raw["diagram_id_mode"]
                report.write_bytes(json.dumps(raw).encode())
                projected = {row["scenario"]: row for row in load_rounds(bundle)["latency"]}
                self.assertEqual(projected["doc"]["eligible_pairs"], 1)
                reason = "diagram-id-unverified" if change == "old-collector" else "evidence-mismatch"
                self.assertEqual(projected["doc"]["reasons"], [reason])
                self.assertIsNone(projected["doc"]["ratio"])
                self.assertEqual(projected["control"]["eligible_pairs"], 1 if change == "old-collector" else 2)

    def test_no_options_bridge_keeps_raw_observations_and_withholds_comparisons(self):
        self.args.variant = ["matched=merman::{}", "no-options=merman", "peer=pele"]
        rounds = self.run_matrix()
        self.assertEqual(rounds["comparisons"]["no-options"]["doc"]["eligible_pairs"], 0)
        self.assertEqual(rounds["comparisons"]["peer"]["doc"]["eligible_pairs"], 2)
        raw = json.loads((self.args.out_dir / "rounds" / "001" / "no-options.json").read_bytes())
        self.assertGreater(raw["scenarios"][0]["warm_samples"][0]["elapsed_ms"], 0)
        projected = [row for row in load_rounds(self.args.out_dir)["latency"] if row["peer"] == "no-options"]
        self.assertTrue(all(row["reasons"] == ["diagram-id-not-applied"] and row["ratio"] is None for row in projected))

    def test_incomplete_collectors_keep_other_rounds_peers_and_scenarios(self):
        def fail(round_no, raw):
            if round_no == 1 and raw["engine"] == "mermaid-v13":
                raw["error"] = "package could not load"
            if round_no == 2 and raw["engine"] == "merman":
                raw["status"] = "failed"
                raw["scenarios"][0].update(status="timeout", error="worker timed out", unmeasured_requests=2,
                                           cold_batch=None, warm_samples=[], init_ms=None, first_render_ms=None)

        self.edit = fail
        report = self.run_matrix(incomplete=True)
        self.assertEqual(len(self.calls), 6)
        self.assertEqual(report["collection_status"], "incomplete")
        pele = report["comparisons"]["pele"]
        self.assertEqual((pele["control"]["status"], pele["control"]["eligible_pairs"]), ("complete", 2))
        self.assertEqual((pele["doc"]["status"], pele["doc"]["eligible_pairs"]), ("partial", 1))
        self.assertIn("merman worker timeout", pele["doc"]["rounds"][1]["withheld_reason"])
        v13 = report["comparisons"]["mermaid-v13"]["doc"]
        self.assertEqual((v13["status"], v13["eligible_pairs"]), ("unavailable", 0))
        self.assertIn("package could not load", v13["rounds"][0]["withheld_reason"])
        for row in (pele["doc"], v13):
            for key in ("per_process_median_ms", "paired_right_over_left", "direction_counts"):
                self.assertIsNone(row[key])

    def test_stable_within_each_worker_can_drift_at_one_document_occurrence(self):
        self.args.peer = ["pele"]

        def change_output(round_no, raw):
            if round_no == 2 and raw["engine"] == "pele":
                row = raw["scenarios"][0]
                for batch in [row["cold_batch"], *row["warm_samples"]]:
                    batch["results"][1]["output_sha256"] = "e" * 64

        self.edit = change_output
        report = self.run_matrix()
        self.assertEqual(self.calls, [(1, "merman"), (1, "pele"), (2, "pele"), (2, "merman")])
        doc = report["comparisons"]["pele"]["doc"]
        self.assertIn("pele output changed at request 2 (a)", doc["rounds"][1]["withheld_reason"])
        self.assertIsNone(doc["paired_right_over_left"])
        self.assertTrue(report["comparisons"]["pele"]["control"]["paired_right_over_left"])

    def test_missing_unknown_or_changed_artifact_identity_keeps_raw_timings(self):
        self.args.peer = ["pele"]
        root = self.args.out_dir.parent
        for case in ("missing", "unknown", "changed", "collector"):
            with self.subTest(case=case):
                self.args.out_dir = root / case

                def change_identity(round_no, raw):
                    if round_no == 2 and case == "collector":
                        raw["collector_sha256"] = "e" * 64
                    elif round_no == 2 and raw["engine"] == "merman":
                        if case == "missing":
                            del raw["artifact_identity"]
                        elif case == "unknown":
                            raw["artifact_identity"].update(status="unknown", reason="unobserved addon")
                        else:
                            raw["artifact_identity"]["files"][1]["sha256"] = "b" * 64

                self.edit = change_identity
                report = self.run_matrix()
                doc = report["comparisons"]["pele"]["doc"]
                self.assertIsNone(doc["direction_counts"])
                reason = doc["rounds"][1]["withheld_reason"]
                self.assertIn("identity changed" if case in ("changed", "collector") else "identity is unknown", reason)
                self.assertIn(reason, (self.args.out_dir / "summary.md").read_text(encoding="utf-8"))
                directory = (self.args.out_dir / report["rounds"][1]["coverage"]).parent
                raw = json.loads((directory / "merman.json").read_bytes())
                self.assertGreater(raw["scenarios"][0]["warm_samples"][0]["complete_batch_ms"], 0)

    def test_rounds_require_even_count_at_least_two_before_writing(self):
        for count in (-2, 0, 1, 3, 2.0, True):
            self.args.process_rounds = count
            with self.subTest(count=count), self.assertRaisesRegex(ValueError, "even integer of at least 2"):
                collect_node_matrix(self.args)
        self.assertFalse(self.args.out_dir.exists())
        self.assertEqual(self.calls, [])
