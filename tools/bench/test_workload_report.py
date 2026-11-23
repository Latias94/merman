import argparse
from contextlib import redirect_stderr, redirect_stdout
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from run_workloads import main
from workload_report import FORMAT, add_report_parser, generate_report, render


REPORTS = Path(__file__).resolve().parents[2] / "docs/performance/reports"


class ReaderReportsTest(unittest.TestCase):
    def test_published_markdown_replays_from_the_portable_receipt_alone(self):
        receipts = sorted(REPORTS.glob("*.json"))
        self.assertTrue(receipts)
        for receipt in receipts:
            with self.subTest(receipt=receipt.name), redirect_stdout(io.StringIO()):
                self.assertEqual(main(["report", "--from", str(receipt), "--check"]), 0)
                page = receipt.with_suffix(".md").read_bytes()
                self.assertEqual(page.count(b"**How to read this table.**"), 3)
                self.assertEqual(page.count(b"**How to read these counts.**"), 1)
                self.assertEqual(page.count(b"**How to read browser rows:**"), 1)
                self.assertEqual(page.count(b"**How to read layout work:**"), 1)
                self.assertIn(receipt.with_suffix(".md").name, (REPORTS / "README.md").read_text(encoding="utf-8"))
                self.assertIn(receipt.name.encode(), page)
                raw = receipt.read_bytes()
                for local in (b"E:\\\\Rust", b"C:\\\\Users", b"/Users/", b"/home/runner/", b"DESKTOP-", b'"hostname"', b"<svg", b"/node_modules/"):
                    self.assertNotIn(local, raw)

    def test_check_detects_an_edit_and_regeneration_restores_only_markdown(self):
        with tempfile.TemporaryDirectory() as tmp, redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
            copy = Path(tmp) / "copied measurements.json"
            copy.write_bytes(next(REPORTS.glob("*.json")).read_bytes())
            original = copy.read_bytes()
            self.assertEqual(main(["report", "--from", str(copy)]), 0)
            generated = copy.with_suffix(".md").read_bytes()
            self.assertIn(b"(copied%20measurements.json)", generated)
            copy.with_suffix(".md").write_bytes(generated.replace(b"\n", b"\r\n"))
            self.assertEqual(main(["report", "--from", str(copy), "--check"]), 0)
            copy.with_suffix(".md").write_bytes(b"Edited")
            with self.assertRaises(SystemExit):
                main(["report", "--from", str(copy), "--check"])
            self.assertEqual(copy.with_suffix(".md").read_bytes(), b"Edited")
            self.assertEqual(main(["report", "--from", str(copy)]), 0)
            self.assertEqual(copy.with_suffix(".md").read_bytes(), generated)
            self.assertEqual(copy.read_bytes(), original)

    def test_regeneration_can_update_existing_report_when_atomic_replace_is_denied(self):
        with tempfile.TemporaryDirectory() as tmp, redirect_stdout(io.StringIO()):
            receipt = Path(tmp) / "measurement.json"
            receipt.write_bytes(next(REPORTS.glob("*.json")).read_bytes())
            original = receipt.with_suffix(".md")
            original.write_bytes(b"old generated report")
            with patch("workload_report.os.replace", side_effect=PermissionError("reader holds a file handle")):
                self.assertEqual(main(["report", "--from", str(receipt)]), 0)
            self.assertIn(b"**How to read these counts.**", original.read_bytes())
            self.assertEqual(main(["report", "--from", str(receipt), "--check"]), 0)

    def test_without_independent_samples_coverage_publishes_no_timing(self):
        source = json.loads((REPORTS / "2026-11-23-windows-x64.json").read_bytes())
        only = {"node_coverage": source["measurements"]["node_coverage"]}
        draft = render({**source, "measurements": only, "note": "local <preview> [abc](ref)", "raw_url": None}, "reader.json")
        self.assertIn(b"No independent Node timing rounds", draft)
        self.assertIn(b"No browser corpus was included", draft)
        self.assertIn(b"&lt;preview&gt;", draft)
        self.assertNotIn(b"[abc](ref)", draft)
        self.assertNotIn(b"Peer / Merman (range)", draft)
        self.assertIn(b"**How to read these counts.**", draft)
        self.assertNotIn(b"**How to read this table.**", draft)
        with self.assertRaisesRegex(ValueError, "HTTPS"):
            render({**source, "raw_url": "https://user:password@example.org/archive"}, "reader.json")

    def test_refuses_to_publish_over_an_existing_dated_pair(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            source = json.loads((REPORTS / "2026-11-23-windows-x64.json").read_bytes())
            data = {**source, "measurements": {"rust_work": source["measurements"]["rust_work"]}}
            parser = argparse.ArgumentParser()
            add_report_parser(parser.add_subparsers())
            args = parser.parse_args(["report", "--rust-work", "plan.json", "work.json", "--date", source["date"],
                                      "--out", str(directory / "report.md")])
            with patch("workload_report.collect", return_value=data):
                generate_report(args)
                with self.assertRaises(FileExistsError):
                    generate_report(args)
            self.assertEqual(json.loads((directory / "report.json").read_bytes())["format"], FORMAT)
