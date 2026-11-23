import hashlib
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from test_workload_reference import report as node_report
from workload_matrix import collect_node_matrix


class NodeMatrixTests(unittest.TestCase):
    def test_failed_peer_keeps_the_other_comparison_and_raw_inputs(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = "flowchart LR\nA[Alpha] --> B"
            request = dict(fixture="example", diagram_id="id-0", source_sha256=hashlib.sha256(source.encode()).hexdigest(),
                           expected_labels=["Alpha"])
            data = dict(schema_version=1, scenarios=[dict(id="doc", requests=[dict(request, source=source)])], topologies=[])
            plan_bytes = json.dumps(data).encode()
            args = SimpleNamespace(plan=root / "input.json", out_dir=root / "results", peer=None, samples=2,
                                   timeout_seconds=30, merman_module=None)
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
                report.update(engine=name, plan_sha256=hashlib.sha256(plan_bytes).hexdigest(),
                              package={"name": package, "version": manifest[package]}, node_project={"cwd": str(cwd)})
                if name == "mermaid-v13":
                    report["render_api"] = "mermaid.render"
                report["scenarios"][0]["requests"] = [request]
                stdout.write(json.dumps(report).encode())
                return SimpleNamespace(returncode=0)

            with patch("workload_matrix.subprocess.run", side_effect=collect):
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
