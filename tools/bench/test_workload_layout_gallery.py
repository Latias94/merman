import json
from pathlib import Path
import re
import shutil
import subprocess
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch
from xml.etree import ElementTree as ET

import run_workloads as bench
import workload_layout_gallery as gallery


CONSUMER = bench.HERE / "node-reference"


def gallery_data(path):
    html = path.read_text(encoding="utf-8")
    data = re.search(r'<script type="application/json" id="gallery">(.*?)</script>', html, re.S)[1]
    return json.loads(data)


class LayoutGalleryTests(unittest.TestCase):
    def test_pele_agentflow_counts_visible_nodes_and_edges_inside_containers(self):
        svg = ('<svg xmlns="http://www.w3.org/2000/svg" class="pele pele-agentflow" viewBox="0 0 40 30">'
               '<g class="pele-cluster pele-flow"/><g class="pele-node pele-shape-collapsed"/>'
               '<g class="pele-node pele-shape-rounded"/><g class="pele-edge pele-edge-sequence"/></svg>')
        card = gallery.svg_card(svg, "pele")
        self.assertEqual((card["nodes"], card["edges"]), (2, 1))

    def test_default_gallery_preserves_source_and_only_calls_native_renderers(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / "input.mmd"
            original = 'flowchart TD\r\nA["</script>"] --> B\r\n'
            source.write_bytes(original.encode())
            out = root / "gallery.html"
            with (patch.object(gallery, "render_pele_cards", side_effect=AssertionError("unexpected Node call")),
                  patch.object(gallery, "render_card", return_value={"error": "native diagnostic"})):
                self.assertEqual(bench.main(["layout-gallery", str(source), "--runner", "unused", "--out", str(out)]), 0)
            data = gallery_data(out)
            case, = data["cases"]
            self.assertEqual(case["source"], original)
            self.assertEqual(list(case["cards"]), ["dagre", "elk", "shoal"])
            self.assertNotIn("pele", data)

    def test_layout_override_keeps_the_authored_source_and_checks_the_runner_receipt(self):
        original = "---\nconfig:\n  layout: dagre\n  htmlLabels: false\n---\nagentflow-beta LR\nA --> B\n"
        backend = {"dagre": [], "elk": ["layout-elk"], "shoal": ["layout-shoal"]}

        def draw(_runner, job):
            request, = job["requests"]
            self.assertEqual(request["source"], original)
            self.assertTrue(job["lock_site_layout"])
            name = job["site"]["layout"]
            return dict(
                site_layout_locked=True,
                outputs=[f'<svg id="{request["diagram_id"]}" viewBox="0 0 40 30">'
                         '<rect width="40" height="30"/></svg>'],
                layout_evidence=[dict(diagram_id=request["diagram_id"], family="agentflow",
                                      source_sha256=bench.digest(original.encode()),
                                      required_capabilities=backend[name])],
            )

        with TemporaryDirectory() as tmp, patch.object(gallery, "child", side_effect=draw) as render:
            root = Path(tmp)
            source = root / "agentflow.mmd"
            source.write_text(original, encoding="utf-8")
            out = root / "gallery.html"
            self.assertEqual(bench.main(["layout-gallery", str(source), "--override-frontmatter-layout",
                                         "--runner", "unused", "--out", str(out)]), 0)
            case, = gallery_data(out)["cases"]
            self.assertEqual(case["source"], original)
            self.assertTrue(case["nativeLayoutLocked"])
            self.assertEqual(render.call_count, 3)
            self.assertTrue(all("svg" in card for card in case["cards"].values()))
            render.side_effect = lambda runner, job: {**draw(runner, job), "site_layout_locked": False}
            with self.assertRaisesRegex(ValueError, "confirms the site layout lock"):
                gallery.render_card(root / "runner", dict(source=original, diagram_id="locked"),
                                    "shoal", None, lock_layout=True)

    def test_missing_pele_install_names_the_locked_package_and_install_command(self):
        with TemporaryDirectory() as tmp, patch.object(gallery, "HERE", Path(tmp)):
            consumer = Path(tmp) / "node-reference"
            consumer.mkdir()
            shutil.copy(CONSUMER / "package-lock.json", consumer)
            with self.assertRaisesRegex(ValueError, r"pele@1\.13\.0.*npm ci --prefix tools/bench/node-reference"):
                gallery.render_pele_cards([])

    def test_node_median_uses_matched_warm_render_calls_and_checks_the_retained_bytes(self):
        with TemporaryDirectory() as tmp:
            drawings = Path(tmp)
            path = drawings / "0-0.svg"
            svg = b'<svg class="pele-flowchart" viewBox="0 0 40 30"><rect width="40" height="30"/></svg>'
            path.write_bytes(svg)
            result = dict(status="ok", output_sha256=bench.digest(svg))
            row = dict(id="example", requests=[{"drawing_artifact": path.name}], coverage={"full_batch_comparable": True},
                       warm_samples=[{"elapsed_ms": 100, "results": [dict(result, elapsed_ms=t)]} for t in (3, 7, 2)])
            self.assertEqual(gallery.retained_pele_card(drawings, 0, row)["nodeTiming"],
                             {"medianMs": 3, "warmMs": [3, 7, 2], "svgSha256": bench.digest(svg)})
            row["coverage"]["full_batch_comparable"] = False
            self.assertNotIn("nodeTiming", gallery.retained_pele_card(drawings, 0, row))
            path.write_bytes(svg + b"\n")
            with self.assertRaisesRegex(ValueError, "differs from the validated output"):
                gallery.retained_pele_card(drawings, 0, row)

    @unittest.skipUnless((CONSUMER / "node_modules/pele/package.json").is_file(), "install the locked Node consumer")
    def test_public_pele_svg_for_each_occurrence_and_an_isolated_validation_error(self):
        cycle = 'flowchart LR\r\n A["回路"] --> B[Queue]\r\n B --> C[Finish]\r\n C --> A\r\n'
        branch = 'flowchart TD\n A[Input] --> B[Left]\n A --> C[Right]\n'
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            local = root / "preview.mmd"
            local.write_bytes(cycle.encode())
            request = dict(fixture=local.name, source=branch, diagram_id="gallery-1", source_sha256=bench.digest(branch.encode()))
            plan = root / "plan.json"
            # Repeated names and public IDs must still select the SVG for each individual source.
            bench.write(plan, dict(schema_version=1, scenarios=[
                dict(id="missing-label", requests=[dict(request, expected_labels=["Absent"])]),
                dict(id="unreviewed", requests=[dict(request, expected_outcome="unreviewed")]),
                dict(id=local.name, requests=[dict(request, expected_labels=["Left", "Right"])]),
            ]))
            out = root / "gallery.html"
            with patch.object(gallery, "render_card", return_value={"error": "native diagnostic"}):
                self.assertEqual(bench.main(["layout-gallery", str(local), "--plan", str(plan), "--pele",
                                             "--runner", "unused", "--out", str(out)]), 0)
            data = gallery_data(out)

        self.assertEqual(data["pele"]["version"], "1.13.0")
        self.assertRegex(data["pele"]["nodeVersion"], r"^v\d+\.")
        local, failed, unreviewed, frozen = data["cases"]
        self.assertEqual([local["source"], frozen["source"]], [cycle, branch])
        for case in data["cases"]:
            self.assertEqual(list(case["cards"]), ["dagre", "elk", "shoal", "pele"])
            self.assertNotIn("medianNs", case["cards"]["pele"])
        self.assertEqual(set(failed["cards"]["pele"]), {"error"})
        self.assertIn("Absent", failed["cards"]["pele"]["error"])
        self.assertNotIn("nodeTiming", unreviewed["cards"]["pele"])
        self.assertEqual(unreviewed["cards"]["pele"]["svg"], frozen["cards"]["pele"]["svg"])

        render = """
          import {readFileSync} from 'node:fs';
          import {render} from 'pele';
          const sources = JSON.parse(readFileSync(0, 'utf8'));
          process.stdout.write(JSON.stringify(sources.map(source => render(source, {idPrefix: 'gallery-1', now: 0}).svg)));
        """
        expected = json.loads(subprocess.check_output(["node", "--input-type=module", "--eval", render],
                                                     cwd=CONSUMER, input=json.dumps([cycle, branch]).encode()))
        for case, svg, edges in zip((local, frozen), expected, (3, 2), strict=True):
            card = case["cards"]["pele"]
            self.assertEqual(card["svg"], svg)
            self.assertEqual(len(card["nodeTiming"]["warmMs"]), 3)
            self.assertEqual(card["nodeTiming"]["svgSha256"], bench.digest(svg.encode()))
            self.assertGreaterEqual(card["nodeTiming"]["medianMs"], 0)
            self.assertEqual((card["nodes"], card["edges"]), (3, edges))
            dimensions = ET.fromstring(svg).get("viewBox").split()[2:]
            self.assertEqual([card["width"], card["height"]], list(map(float, dimensions)))
        self.assertIsNotNone(ET.fromstring(local["cards"]["pele"]["svg"]).find(".//*[@data-id='L_C_A_0']"))


if __name__ == "__main__":
    unittest.main()
