import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
import zlib
from concurrent.futures import ThreadPoolExecutor
from contextlib import redirect_stdout
from unittest.mock import patch

if __package__:
    from . import workload_cli as cli
    from .workload_svg import validate_svg
else:
    import workload_cli as cli
    from workload_svg import validate_svg


ROOT = Path(__file__).resolve().parents[2]
FAKE_CLI = """\
import json, os, sys, time
from pathlib import Path
from xml.etree import ElementTree as ET

args = sys.argv[1:]
if args == ['--version']:
    print('fixture-cli 1.0')
    sys.exit(0)
def option(name):
    return args[args.index(name) + 1]
if args[0] == 'render':
    source, output, diagram = Path(args[args.index('--format') - 1]), Path(option('--output')), option('--svg-id')
else:
    source, output, diagram = Path(option('-i')), Path(option('-o')), option('-I')
mode = source.read_bytes()
if mode == b'nonzero':
    output.write_bytes(b'<svg><rect/></svg>')
    print('render failed', file=sys.stderr)
    sys.exit(9)
if mode == b'missing':
    sys.exit(0)
if mode == b'timeout':
    time.sleep(15)
if mode in (b'reject', b'partial', b'host'):
    if mode == b'partial':
        output.write_bytes(b'partial')
    if mode == b'host':
        print('Error: Failed to launch the browser', file=sys.stderr)
    else:
        print('Generating single mermaid chart', flush=True)
        print('Error: Parse error on line 10:', file=sys.stderr)
        print("------^\\nExpecting 'NODE_STRING', got 'EOF'", file=sys.stderr)
    print('at renderer\\n' * 250, file=sys.stderr)
    sys.exit(1)
if output.suffix == '.png':
    output.write_bytes(Path(__file__).with_suffix('.png').read_bytes())
else:
    svg = ET.Element('svg', id=diagram)
    if mode == b'empty':
        svg.set('viewBox', '0 0 17 17')
        output.write_bytes(ET.tostring(svg))
        sys.exit(0)
    if mode == b'error':
        svg.set('aria-roledescription', 'error')
    ET.SubElement(svg, 'desc').text = json.dumps(dict(pid=os.getpid(), args=args))
    ET.SubElement(svg, 'style').text = '.error-icon { fill: red; }'
    ET.SubElement(svg, 'text').text = 'Syntax error in text'
    output.write_bytes(ET.tostring(svg))
"""


def chunk(tag, data):
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data))


def png(width=2, height=1, compressed=None):
    ihdr = struct.pack(">2I5B", width, height, 8, 0, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(b"\0\0\xff") if compressed is None else compressed)
        + chunk(b"IEND", b"")
    )


class WorkloadCliTests(unittest.TestCase):
    def setUp(self):
        temp = ROOT / "target" / "bench" / "tests"
        temp.mkdir(parents=True, exist_ok=True)
        directory = tempfile.TemporaryDirectory(dir=temp, prefix="cli paths & %VARIABLE% ! ")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.fake = self.root / "fake.py"
        self.fake.write_text(FAKE_CLI, encoding="utf-8")
        self.fake.with_suffix(".png").write_bytes(png())
        self.plan = self.root / "plan.json"

    def write_plan(self, *sources):
        requests = [
            dict(fixture="../test", source=source, diagram_id=f"id & %PATH% ! {i}")
            for i, source in enumerate(sources)
        ]
        plan = dict(
            schema_version=1,
            suite="quick",
            scenarios=[dict(id="one", requests=requests)],
            topologies=[dict(id="not-a-cli-scenario")],
        )
        raw = json.dumps(plan, indent=2, ensure_ascii=False).replace("\n", "\r\n").encode()
        self.plan.write_bytes(raw)
        return hashlib.sha256(raw).hexdigest()

    def args(self, engine, fmt, samples):
        return [
            "--plan",
            str(self.plan),
            "--engine",
            engine,
            "--binary",
            str(self.fake),
            "--format",
            fmt,
            "--samples",
            str(samples),
            "--output-dir",
            str(self.root / "out"),
        ]

    def set_outcomes(self, *outcomes):
        plan = json.loads(self.plan.read_bytes())
        for request, outcome in zip(plan["scenarios"][0]["requests"], outcomes, strict=True):
            request["expected_outcome"] = outcome
        self.plan.write_text(json.dumps(plan), encoding="utf-8")

    def measure(self, engine="merman", fmt="svg", samples=1, extra=()):
        output = io.StringIO()
        with (
            patch.object(cli, "command_prefix", return_value=[sys.executable, "-B", str(self.fake)]),
            redirect_stdout(output),
        ):
            code = cli.main([*self.args(engine, fmt, samples), *extra])
        return code, json.loads(output.getvalue())

    def test_order_frozen_bytes_new_processes_and_literal_options(self):
        source = 'flowchart LR\r\nA["中文"] --> B\r\n'
        digest = self.write_plan(source, source)
        font = self.root / "font & name.ttf"
        font.write_bytes(b"fixed font")
        extra = ["--font-file", str(font), "--extra-arg=-c", '--extra-arg=space "quotes" & %PATH% !']
        code, report = self.measure(samples=2, extra=extra)
        self.assertEqual(
            (code, report["suite"], report["version"], report["plan_sha256"]),
            (0, "quick", "fixture-cli 1.0", digest),
        )
        self.assertTrue(all(report["host"][key] for key in ("hostname", "platform", "arch")))
        self.assertEqual(report["extra_args"], ["-c", 'space "quotes" & %PATH% !'])
        self.assertEqual((report["completed"], report["failed"]), (4, 0))
        self.assertEqual(
            report["font_files"][0]["sha256"], hashlib.sha256(font.read_bytes()).hexdigest()
        )
        (scenario,) = report["scenarios"]
        run = Path(report["run_dir"])
        request = scenario["requests"][0]
        self.assertEqual((run / request["input"]).read_bytes(), source.encode())
        self.assertEqual(request["source_sha256"], hashlib.sha256(source.encode()).hexdigest())
        first = scenario["batches"][0]["results"][0]
        self.assertEqual(scenario["first_render_ms"], first["elapsed_ms"])
        self.assertEqual(scenario["batch_samples_ms"], [b["wall_ms"] for b in scenario["batches"]])
        pids = set()
        for batch in scenario["batches"]:
            self.assertGreaterEqual(batch["wall_ms"], sum(r["elapsed_ms"] for r in batch["results"]))
            for req, row in zip(scenario["requests"], batch["results"]):
                svg = ET.parse(run / row["output"]).getroot()
                self.assertEqual(svg.get("id"), req["diagram_id"])
                meta = json.loads(svg.findtext("desc"))
                self.assertIn('space "quotes" & %PATH% !', meta["args"])
                self.assertIn(str(font), meta["args"])
                self.assertGreater(max(row["resources"].values()), 0)
                pids.add(meta["pid"])
        self.assertEqual(len(pids), 4)

    def test_retry_exit_and_timeout_do_not_count_partial_batches(self):
        self.write_plan("good")
        code, old = self.measure()
        self.assertEqual(code, 0)
        self.write_plan("missing", "nonzero", "timeout", "good")
        code, report = self.measure(extra=["--timeout-seconds=1"])
        self.assertEqual((code, report["completed"], report["failed"]), (1, 1, 3))
        self.assertNotEqual(old["run_dir"], report["run_dir"])
        (scenario,) = report["scenarios"]
        self.assertIsNone(scenario["first_render_ms"])
        self.assertEqual(scenario["batch_samples_ms"], [None])
        rows = scenario["batches"][0]["results"]
        self.assertEqual([r["status"] for r in rows], ["invalid_output", "exit_error", "timeout", "ok"])
        self.assertEqual(rows[1]["returncode"], 9)
        self.assertIn("render failed", rows[1]["log_tail"])
        self.assertTrue((Path(old["run_dir"]) / rows[0]["output"]).is_file())
        self.assertFalse((Path(report["run_dir"]) / rows[0]["output"]).exists())

    def test_same_version_script_at_one_path_has_distinct_product_bytes(self):
        self.write_plan("good")
        identities = []
        for build in ("A", "B"):
            self.fake.write_text(FAKE_CLI + f"\n# build {build}\n", encoding="utf-8")
            code, report = self.measure()
            self.assertEqual((code, report["version"]), (0, "fixture-cli 1.0"))
            identity = report["artifact_identity"]
            self.assertEqual((identity["scope"], identity["status"]), ("command-prefix", "recorded"))
            expected = [("executable", Path(sys.executable)), ("script", self.fake)]
            self.assertEqual(identity["files"], [dict(role=role, path=str(path), bytes=path.stat().st_size,
                                                     sha256=hashlib.sha256(path.read_bytes()).hexdigest())
                                                 for role, path in expected])
            identities.append(identity["files"])
        self.assertEqual(identities[0][0], identities[1][0])
        self.assertNotEqual(identities[0][1]["sha256"], identities[1][1]["sha256"])

    def test_changed_command_during_collection_preserves_results_with_unknown_identity(self):
        self.write_plan("good")
        self.fake.write_text(FAKE_CLI + "\nwith open(__file__, 'a') as f: f.write('\\n# replaced\\n')\n")
        code, report = self.measure()
        self.assertEqual((code, report["completed"]), (0, 1))
        identity = report["artifact_identity"]
        self.assertEqual(identity["status"], "unknown")
        self.assertIn(str(self.fake), identity["reason"])
        self.assertEqual([file["role"] for file in identity["files"]], ["executable"])

    @unittest.skipUnless(os.name == "nt", "Windows job accounting")
    def test_peak_job_commit_includes_simultaneous_parent_and_child_allocations(self):
        allocation = "import sys; data = bytearray(int(sys.argv[1])); data[::4096] = b'x' * len(data[::4096])"
        parent = allocation + "\nimport subprocess\nsubprocess.run([sys.executable, '-B', '-c', sys.argv[2], sys.argv[3]], check=True)"
        # Start the interpreter directly so Windows venv launchers do not join the two-process job.
        python = sys._base_executable
        size = 32 * 1024 * 1024
        peaks = []
        with ThreadPoolExecutor(max_workers=1) as pool:
            for child_size in (0, size):
                result = cli.invoke([python, "-B", "-c", parent, str(size), allocation, str(child_size)],
                                    15, self.root, pool)
                self.assertEqual(result["status"], "ok", result)
                resources = result["resources"]
                self.assertEqual(resources["job_processes"], 2)
                self.assertGreater(resources["peak_working_set_bytes"], 0)
                peaks.append(resources["peak_job_commit_bytes"])
        self.assertGreater(peaks[1] - peaks[0], size * 0.75)

    def test_png_dimensions_and_error_diagram_postflight(self):
        self.write_plan("good", "error")
        code, report = self.measure(engine="mmdc", fmt="png", samples=2)
        self.assertEqual((code, report["completed"], report["failed"]), (1, 2, 2))
        self.assertEqual(len(report["png_svg_checks"]), 2)
        for batch in report["scenarios"][0]["batches"]:
            good, bad = batch["results"]
            self.assertEqual((good["width"], good["height"], good["status"]), (2, 1, "ok"))
            self.assertEqual(
                (bad["status"], bad["error"]), ("failed_svg_check", "Mermaid error diagram")
            )

    def test_empty_error_svg_and_corrupt_png(self):
        path = self.root / "output"
        for data in (
            b"",
            b"<html/>",
            b"<svg/>",
            b"<svg><text> </text></svg>",
            b"<svg><foreignObject><div/></foreignObject></svg>",
            b"<svg><defs><path/></defs></svg>",
            b'<svg><g><path class="error-icon"/></g></svg>',
            b"<svg><text>",
        ):
            with self.subTest(svg=data):
                path.write_bytes(data)
                with self.assertRaises((ValueError, ET.ParseError)):
                    cli.verify_output(path, "svg")
        image = png()
        for data in (b"", image[:24], png(0, 1), image[:-12], image[:35] + bytes(4) + image[39:]):
            with self.subTest(png=data):
                with self.assertRaises(ValueError):
                    cli.png_size(data)

    def test_png_content_uses_composited_pixels_even_with_a_good_svg_sibling(self):
        from PIL import Image

        self.write_plan("good")
        path = self.fake.with_suffix(".png")
        cases = (
            ([(255, 255, 255, 255)] * 2, False),
            ([(40, 90, 130, 0), (220, 5, 9, 0)], False),
            ([(0, 0, 0, 0), (255, 255, 255, 255)], False),
            ([(0, 0, 0, 128), (127, 127, 127, 255)], False),
            ([(2, 3, 4, 255)] * 2, False),
            ([(2, 3, 4, 0), (2, 3, 4, 255)], True),
        )
        for pixels, painted in cases:
            with self.subTest(pixels=pixels):
                img = Image.new("RGBA", (2, 1))
                img.putdata(pixels)
                img.save(path)
                code, report = self.measure(fmt="png")
                row = report["scenarios"][0]["batches"][0]["results"][0]
                self.assertEqual(row["nonuniform_on_white"], painted)
                self.assertEqual((row["width"], row["height"]), (2, 1))
                self.assertEqual(row["sha256"], hashlib.sha256(path.read_bytes()).hexdigest())
                self.assertEqual((code, row["status"]), (0, "ok") if painted else (1, "invalid_output"))
                if not painted:
                    self.assertEqual(row["artifact_status"], "empty-png")
                    self.assertEqual(report["png_svg_checks"], [])

    def test_palette_transparency_and_16_bit_gray(self):
        from PIL import Image

        path = self.fake.with_suffix(".png")
        palette = Image.frombytes("P", (2, 1), b"\x00\x01")
        palette.putpalette([150, 0, 0, 20, 160, 99])
        palette.save(path, transparency=b"\x00\x00")
        with self.assertRaisesRegex(ValueError, "nonuniform"):
            cli.verify_output(path, "png")
        palette.save(path, transparency=b"\x00\xff")
        self.assertTrue(cli.verify_output(path, "png")["nonuniform_on_white"])
        gray = Image.frombytes("I;16", (2, 1), struct.pack("<HH", 32000, 65535))
        gray.save(path)
        self.assertTrue(cli.verify_output(path, "png")["nonuniform_on_white"])
        gray.save(path, transparency=32000)
        self.assertFalse(cli.verify_output(path, "png", expected_outcome="allow-empty")["nonuniform_on_white"])

    def test_png_requires_decodable_pixels_and_strict_framing_within_budget(self):
        path = self.fake.with_suffix(".png")
        deflate = zlib.compress(b"\x00\x00\xff")
        image = png()
        for bad in (
            png(compressed=b"not zlib"), png(compressed=deflate[:4]),
            png(compressed=zlib.compress(b"\x00")), png(compressed=zlib.compress(b"\xff\x00\xff")),
            image + b"trailing", image[:33] + png(1, 2)[8:33] + image[33:], png(4097, 4096),
        ):
            path.write_bytes(bad)
            for outcome in cli.EXPECTED_OUTCOMES:
                with self.subTest(outcome=outcome, bad=bad[:50]), self.assertRaises(ValueError):
                    cli.verify_output(path, "png", expected_outcome=outcome)
        with patch.object(cli, "MAX_PNG_BYTES", 48), patch.object(cli, "png_decoder") as decoder:
            path.write_bytes(png())
            with self.assertRaisesRegex(ValueError, "encoded bytes"):
                cli.verify_output(path, "png")
            result = {"status": "ok", "output": path.name}
            cli.verify(result, path.parent, [])
            self.assertEqual(result["status"], "observer-limit")
            decoder.assert_not_called()

    def test_missing_png_decoder_is_a_setup_error_before_any_render(self):
        self.write_plan("good")
        with patch.dict(sys.modules, {"PIL": None}), patch.object(cli, "invoke") as invoke:
            code, report = self.measure(fmt="png")
        self.assertEqual((code, report["status"]), (2, "failed"))
        self.assertIn("python -m pip install -r tools/bench/requirements-png.txt", report["error"])
        invoke.assert_not_called()

    def test_rejections_are_diagnostics_separate_from_completed_renders(self):
        for fmt in ("svg", "png"):
            with self.subTest(fmt=fmt):
                self.write_plan("reject", "good")
                self.set_outcomes("reject", "draw")
                code, report = self.measure(engine="mmdc", fmt=fmt, samples=2)
                self.assertEqual(
                    (code, report["completed"], report["expected_rejections"], report["failed"]), (0, 2, 2, 0)
                )
                (scenario,) = report["scenarios"]
                self.assertIsNone(scenario["first_render_ms"])
                self.assertEqual(scenario["batch_samples_ms"], [None, None])
                self.assertEqual([r["expected_outcome"] for r in scenario["requests"]], ["reject", "draw"])
                for batch in scenario["batches"]:
                    rejected, drawn = batch["results"]
                    self.assertEqual(
                        (rejected["status"], rejected["returncode"], rejected["expected_outcome"]),
                        ("expected-rejection", 1, "reject"),
                    )
                    self.assertIn("Parse error on line 10:", rejected["diagnostic"])
                    self.assertNotIn("Parse error", rejected["log_tail"])
                    self.assertGreater(rejected["elapsed_ms"], 0)
                    self.assertFalse((Path(report["run_dir"]) / rejected["output"]).exists())
                    self.assertNotIn("sha256", rejected)
                    self.assertEqual(drawn["status"], "ok")

    def test_rendered_missing_partial_and_host_results_do_not_satisfy_reject(self):
        self.write_plan("good", "missing", "partial", "host", "error", "timeout")
        self.set_outcomes(*(["reject"] * 6))
        code, report = self.measure(extra=["--timeout-seconds=1"])
        self.assertEqual((code, report["expected_rejections"], report["failed"]), (1, 0, 6))
        rows = report["scenarios"][0]["batches"][0]["results"]
        self.assertEqual(
            [r["status"] for r in rows],
            ["unexpected-success", "invalid_output", "exit_error", "exit_error", "invalid_output", "timeout"],
        )
        self.assertEqual(rows[4]["artifact_status"], "error-svg")
        self.assertTrue(rows[0]["sha256"])

    def test_sized_blank_png_respects_outcome_and_svg_sidecar(self):
        self.write_plan("empty", "empty", "empty", "empty")
        self.set_outcomes("allow-empty", "draw", "reject", "unreviewed")
        self.fake.with_suffix(".png").write_bytes(png(compressed=zlib.compress(b"\x00\xff\xff")))
        code, report = self.measure(fmt="png")
        self.assertEqual(
            (code, report["completed"], report["expected_rejections"], report["failed"]), (1, 2, 0, 2)
        )
        rows = report["scenarios"][0]["batches"][0]["results"]
        self.assertEqual([r["status"] for r in rows], ["ok", "invalid_output", "unexpected-success", "ok"])
        self.assertEqual([r["nonuniform_on_white"] for r in rows], [False] * 4)
        self.assertEqual([rows[i]["output_class"] for i in (0, 3)], ["empty", "empty"])
        self.assertEqual(report["png_decoder"]["name"], "Pillow")

    def test_sibling_cache_respects_outcome_and_keeps_the_png_observation(self):
        self.write_plan("empty", "empty")
        self.set_outcomes("allow-empty", "draw")
        plan = json.loads(self.plan.read_bytes())
        first, second = plan["scenarios"][0]["requests"]
        second["diagram_id"] = first["diagram_id"]
        self.plan.write_text(json.dumps(plan), encoding="utf-8")
        _, report = self.measure(fmt="png")
        allowed, required = report["scenarios"][0]["batches"][0]["results"]
        self.assertEqual((allowed["status"], allowed["output_class"]), ("ok", "draw"))
        self.assertEqual(report["png_svg_checks"][0]["output_class"], "empty")
        self.assertEqual(required["status"], "failed_svg_check")
        self.assertNotEqual(allowed["svg_check"], required["svg_check"])

    def test_outcome_validated_before_launch_and_defaults_to_draw(self):
        self.write_plan("good")
        _, report = self.measure()
        self.assertEqual(report["scenarios"][0]["requests"][0]["expected_outcome"], "draw")
        for outcome in (None, "", "unknown", []):
            with self.subTest(outcome=outcome), patch.object(cli, "invoke") as invoke:
                self.set_outcomes(outcome)
                code, report = self.measure()
                invoke.assert_not_called()
                self.assertEqual(code, 2)
                self.assertIn("invalid expected_outcome", report["error"])

    def test_visible_labels_use_text_nodes_and_join_inline_runs(self):
        svg = self.root / "labels.svg"
        svg.write_text(
            """<svg xmlns="http://www.w3.org/2000/svg" id="root-only">
          <style>.css-only { fill: blue; }</style><title>title-only</title><desc>desc-only</desc>
          <defs><text>defs-only</text></defs><g class="class-only"><text>A</text>
          <text><tspan>h</tspan><tspan>i</tspan></text><foreignObject>
          <div xmlns="http://www.w3.org/1999/xhtml" id="html-id-only"><style>hidden-css</style>
          <p><b>Read</b> &amp; <i>write</i></p><p>B</p><script>script-only</script></div>
          </foreignObject></g></svg>""",
            encoding="utf-8",
        )
        cli.verify_output(svg, "svg", ["A", "B", "hi", "Read & write"])
        for value in (
            "root-only",
            "css-only",
            "title-only",
            "desc-only",
            "defs-only",
            "class-only",
            "html-id-only",
            "hidden-css",
            "script-only",
            "writ",
        ):
            with (
                self.subTest(label=value),
                self.assertRaisesRegex(ValueError, "missing expected SVG labels"),
            ):
                cli.verify_output(svg, "svg", [value])

    def test_required_facts_match_either_complete_label(self):
        grid = ["Electricity grid", " Electricity  grid 245.32 "]
        labels = ["H2 conversion", grid]
        sink = "<text>H2 conversion</text>"
        cases = [
            (sink + "<text>Electricity grid</text>", []),
            (sink + "<foreignObject><p>Electricity grid\n245.32</p></foreignObject>", []),
            ("<text>Electricity grid</text>", ["H2 conversion"]),
            (sink + "<rect/>", [grid]),
            (sink + "<text>Electricity grid 245.33</text>", [grid]),
            (sink + "<text>Electricity </text><text>grid 245.32</text>", [grid]),
            ("<rect/>", labels),
        ]
        for elements, missing in cases:
            with self.subTest(elements=elements):
                result = validate_svg(f"<svg>{elements}</svg>", labels)
                if missing:
                    self.assertEqual(result["status"], "missing-labels")
                    self.assertEqual(result["missing_labels"], missing)
                else:
                    self.assertEqual(result, {"status": "ok"})

    def test_png_sibling_checks_each_label_expectation(self):
        self.write_plan("good", "good")
        plan = json.loads(self.plan.read_bytes())
        first, second = plan["scenarios"][0]["requests"]
        short, full = "Syntax error", "Syntax error in text"
        first["expected_labels"] = [[short, full]]
        second.update(diagram_id=first["diagram_id"], expected_labels=[[short], full])
        self.plan.write_text(json.dumps(plan), encoding="utf-8")
        code, report = self.measure(fmt="png", samples=2)
        self.assertEqual((code, report["completed"], report["failed"]), (1, 2, 2))
        checks = [check["output"] for check in report["png_svg_checks"]]
        self.assertEqual(len(set(checks)), 2)
        (scenario,) = report["scenarios"]
        self.assertEqual(
            [r["expected_labels"] for r in scenario["requests"]],
            [first["expected_labels"], second["expected_labels"]],
        )
        for batch in scenario["batches"]:
            good, bad = batch["results"]
            self.assertEqual([good["svg_check"], bad["svg_check"]], checks)
            self.assertEqual((good["status"], bad["status"]), ("ok", "failed_svg_check"))
            self.assertEqual(bad["error"], f"missing expected SVG labels: {[[short]]!r}")

    def test_invalid_label_groups_fail_before_launch(self):
        self.write_plan("good")
        plan = json.loads(self.plan.read_bytes())
        request = plan["scenarios"][0]["requests"][0]
        for labels in ("plain", [""], [[]], [["valid", " \t"]], [[["nested"]]], [["valid", 1]]):
            with self.subTest(labels=labels), patch.object(cli, "invoke") as invoke:
                request["expected_labels"] = labels
                self.plan.write_text(json.dumps(plan), encoding="utf-8")
                code, report = self.measure()
                invoke.assert_not_called()
                self.assertEqual(code, 2)
                self.assertIn("invalid expected_labels", report["error"])

    def test_bad_input_digest_reports_raw_plan_hash_without_launching(self):
        self.write_plan("good")
        plan = json.loads(self.plan.read_bytes())
        plan["scenarios"][0]["requests"][0]["source_sha256"] = "wrong"
        self.plan.write_text(json.dumps(plan), encoding="utf-8")
        with patch.object(cli, "invoke") as invoke:
            code, report = self.measure()
            invoke.assert_not_called()
        self.assertEqual(code, 2)
        self.assertIn("source_sha256 mismatch", report["error"])
        self.assertEqual(report["plan_sha256"], hashlib.sha256(self.plan.read_bytes()).hexdigest())

    @unittest.skipUnless(
        os.name == "nt" and shutil.which("node"), "requires Node and Windows npm shims"
    )
    def test_windows_npm_shims_pass_literal_argv(self):
        self.write_plan("good")
        js = """const fs = require('node:fs'); const args = process.argv.slice(2);
if (args[0] === '--version') console.log('npm-test 1.0');
else fs.writeFileSync(args[args.indexOf('-o')+1], '<svg><text>'+JSON.stringify(args).replaceAll('&','&amp;')+'</text></svg>');"""
        for install in ("local", "global"):
            with self.subTest(install=install):
                modules = self.root / install / "node_modules"
                package = modules / "@mermaid-js" / "mermaid-cli"
                package.mkdir(parents=True)
                (package / "package.json").write_text(json.dumps({"bin": {"mmdc": "entry.cjs"}}))
                (package / "entry.cjs").write_text(js)
                shim = (modules / ".bin" if install == "local" else modules.parent) / "mmdc.cmd"
                shim.parent.mkdir(exist_ok=True)
                shim.write_text("@exit /b 75\n")
                self.assertEqual(
                    cli.command_prefix("mmdc", str(shim.with_suffix(""))),
                    cli.command_prefix("mmdc", str(shim)),
                )
                args = self.args("mmdc", "svg", 1)
                args[args.index("--binary") + 1] = str(shim)
                literal = 'value "literal" & %PATH% | ! ^'
                proc = subprocess.run(
                    [sys.executable, "-B", cli.__file__, *args, f"--extra-arg={literal}"],
                    env={**os.environ, "PUPPETEER_EXECUTABLE_PATH": str(self.fake)},
                    capture_output=True,
                    check=True,
                    timeout=15,
                )
                report = json.loads(proc.stdout)
                identity = report["artifact_identity"]
                self.assertEqual(identity["status"], "recorded")
                self.assertEqual([file["path"] for file in identity["files"]], report["command_prefix"])
                self.assertEqual(Path(identity["files"][1]["path"]), package / "entry.cjs")
                self.assertEqual(identity["files"][1]["sha256"], hashlib.sha256((package / "entry.cjs").read_bytes()).hexdigest())
                self.assertEqual(identity["browser"]["status"], "unobserved")
                self.assertEqual(identity["browser"]["configured_executable"]["sha256"], hashlib.sha256(self.fake.read_bytes()).hexdigest())
                self.assertIn("committed bytes", report["resource_scope"]["peak_job_commit_bytes"])
                row = report["scenarios"][0]["batches"][0]["results"][0]
                echo = json.loads(ET.parse(Path(report["run_dir"]) / row["output"]).findtext("text"))
                self.assertIn(literal, echo)
                self.assertEqual(echo[echo.index("-I") + 1], "id & %PATH% ! 0")


if __name__ == "__main__":
    unittest.main()
