import unittest
from copy import deepcopy

if __package__:
    from .workload_reference import compare_reference
else:
    from workload_reference import compare_reference


def report(surface, engine, times=((1, 100), (2, 2), (100, 1)), fmt="svg", *, wasm=False):
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


class ReferenceTests(unittest.TestCase):
    def test_comparisons_distinguish_same_version_rebuilds_and_read_historical_reports(self):
        for surface, peer in (("node", "pele"), ("cli", "mmdc")):
            with self.subTest(surface=surface):
                left, right = report(surface, "merman"), report(surface, peer)
                if surface == "cli":
                    right["command_prefix"].append("/mmdc.js")
                    right["artifact_identity"]["files"].append(dict(role="script", path="/mmdc.js", bytes=7, sha256="a" * 64))
                original = compare_reference(deepcopy(left), right)
                left["artifact_identity"]["files"][-1] = {**left["artifact_identity"]["files"][-1], "sha256": "0" * 64}
                rebuilt = compare_reference(deepcopy(left), right)
                self.assertEqual(rebuilt["left"]["version"], original["left"]["version"])
                self.assertNotEqual(rebuilt["left"], original["left"])
                self.assertEqual(rebuilt["right"]["artifact_identity"], right["artifact_identity"])
                self.assertEqual(rebuilt["left"]["artifact_identity"], left["artifact_identity"])
                self.assertEqual(rebuilt["scenarios"][0]["complete_batch_ms"]["right_over_left"], 2)
                del left["artifact_identity"]
                if surface == "node":
                    left["package"]["entry_sha256"] = "a" * 64
                historical = compare_reference(left, right)
                self.assertEqual(historical["left"]["artifact_identity"]["status"], "unknown")
                self.assertIn("merman", historical["ratio_withheld_reason"])
                row, = historical["scenarios"]
                self.assertEqual((row["common_stable"], row["complete_batch_ms"]["left"]), (2, 111))
                self.assertIsNone(row["complete_batch_ms"]["right_over_left"])

    def test_partial_or_malformed_evidence_cannot_identify_a_transport(self):
        left, right = report("node", "merman"), report("node", "pele")
        identity = left["artifact_identity"]
        identity["files"].pop()
        with self.assertRaisesRegex(ValueError, "incomplete product artifact"):
            compare_reference(left, right)
        identity.update(status="unknown", reason="No loaded addon")
        self.assertIsNone(compare_reference(left, right)["scenarios"][0]["common_render_ms"]["right_over_left"])
        identity["files"][0]["sha256"] = "version-1.2.3"
        with self.assertRaisesRegex(ValueError, "artifact file evidence"):
            compare_reference(left, right)

    def test_recorded_file_evidence_must_match_the_reported_entry_or_command(self):
        left, right = report("node", "merman"), report("node", "pele")
        left["package"]["entry_sha256"] = "b" * 64
        with self.assertRaisesRegex(ValueError, "differs from the Node entry"):
            compare_reference(left, right)
        left, right = report("cli", "merman"), report("cli", "mmdc")
        left["command_prefix"] = ["/path/to/old-binary"]
        with self.assertRaisesRegex(ValueError, "differs from the CLI command"):
            compare_reference(left, right)

    def test_png_content_ratio_requires_matching_decoder_evidence(self):
        left, right = (report("cli", engine, fmt="png") for engine in ("merman", "mmdc"))
        for decoder in (None, {"name": "Pillow", "version": "different"}):
            right["png_decoder"] = decoder
            compared = compare_reference(left, right)
            self.assertIn("PNG decoder", compared["ratio_withheld_reason"])
            self.assertIsNone(compared["scenarios"][0]["complete_batch_ms"]["right_over_left"])

    def test_historical_or_changed_collector_preserves_coverage_and_withholds_ratios(self):
        left, right = report("cli", "merman"), report("cli", "mmdc")
        for fingerprint in (None, "f" * 64):
            with self.subTest(fingerprint=fingerprint):
                result = compare_reference(left, dict(right, collector_sha256=fingerprint))
                self.assertEqual(result["render_settings"], "renderer defaults")
                self.assertIn("collector", result["ratio_withheld_reason"])
                row = result["scenarios"][0]
                self.assertEqual(row["common_stable"], 2)
                self.assertIsNone(row["complete_batch_ms"]["right_over_left"])

    def test_mermaid_v13_requires_its_node_api_and_reports_discovery_context(self):
        merman, pele = report("node", "merman"), report("node", "pele")
        project = {"cwd": "/bench", "digests": {"package.json": "a" * 64, "package-lock.json": "b" * 64}}
        v13 = dict(pele, engine="mermaid-v13", package={"name": "mermaid", "version": "13.0.0-alpha.3"},
                   render_api="mermaid.render", node_project=project)
        for peer in (merman, pele):
            with self.subTest(engine=peer["engine"]):
                result = compare_reference(dict(peer, node_project=project), v13)
                self.assertIsNotNone(result["scenarios"][0]["complete_batch_ms"]["right_over_left"])
                self.assertEqual(result["right"]["node_project"], project)
                result = compare_reference(peer, v13)
                self.assertEqual(result["scenarios"][0]["common_stable"], 2)
                self.assertIsNone(result["scenarios"][0]["complete_batch_ms"]["right_over_left"])
                self.assertTrue(result["ratio_withheld_reason"])
        for change in ({"render_api": "renderSync"}, {"package": {"name": "mermaid", "version": "12.2.0"}}):
            with self.assertRaisesRegex(ValueError, "unrecognized Node"):
                compare_reference(merman, {**v13, **change})

    def test_node_uses_median_of_warm_batch_sums_and_allows_different_output(self):
        left, right = report("node", "merman"), report("node", "pele")
        (row,) = compare_reference(left, right)["scenarios"]
        self.assertEqual((row["common_completed"], row["planned_requests"]), (2, 2))
        self.assertEqual(row["common_render_ms"], dict(left=101, right=202, right_over_left=2))
        self.assertEqual(row["complete_batch_ms"], dict(left=111, right=222, right_over_left=2))
        first = row["requests"][0]
        self.assertEqual(
            (first["left"]["output_bytes"], first["right"]["output_bytes"]), ([100], [200])
        )
        self.assertEqual(
            compare_reference(right, left)["scenarios"][0]["complete_batch_ms"]["right_over_left"], 0.5
        )

    def test_mixed_success_uses_one_fixed_intersection_even_when_a_failure_is_cold(self):
        times = ((3, 100, 200, 300), (6, 190, 210, 310))
        left, right = report("node", "merman", times), report("node", "pele", times)
        a, b = left["scenarios"][0], right["scenarios"][0]
        for batch in [b["cold_batch"], *b["warm_samples"]]:
            batch["results"][0]["status"] = "missing-labels"
        a["warm_samples"][0]["results"][2]["output_sha256"] = "e" * 64  # Same byte count.
        b["cold_batch"]["results"][3]["status"] = "render-error"
        (row,) = compare_reference(left, right)["scenarios"]
        self.assertEqual(
            (row["common_completed"], row["common_stable"], row["planned_requests"]), (2, 1, 4)
        )
        first, _, third, fourth = row["requests"]
        self.assertEqual(
            (first["right"]["state"], first["right"]["statuses"]), ("missing", ["missing-labels"])
        )
        self.assertEqual(
            (third["left"]["state"], fourth["right"]["state"]), ("unstable-output", "intermittent")
        )
        self.assertEqual(row["common_render_ms"], dict(left=145, right=290, right_over_left=2))
        self.assertIsNone(row["complete_batch_ms"])

    def test_disjoint_cli_success_never_becomes_a_latency(self):
        left, right = report("cli", "merman"), report("cli", "mmdc")
        for i, data in enumerate((left, right)):
            data["status"] = "failed"
            for batch in data["scenarios"][0]["batches"]:
                batch["results"][i] = dict(status="timeout", elapsed_ms=0.001)
        (row,) = compare_reference(left, right)["scenarios"]
        self.assertEqual(row["common_completed"], 0)
        self.assertIsNone(row["common_render_ms"])
        self.assertIsNone(row["complete_batch_ms"])

    def test_only_matching_reviewed_output_classes_get_warm_request_times(self):
        outcomes = ["draw", "allow-empty", "allow-empty", "allow-empty", "unreviewed", "reject", "reject", "draw"]
        times = [(1, 2, 3, 4, 5, 6, 7, 8)] * 2
        left, right = report("node", "merman", times), report("node", "pele", times)
        for data in (left, right):
            row = data["scenarios"][0]
            for req, outcome in zip(row["requests"], outcomes):
                req["expected_outcome"] = outcome
            for batch in [row["cold_batch"], *row["warm_samples"]]:
                results = batch["results"]
                results[1]["output_class"] = "empty"
                results[2]["output_class"] = "empty" if data is left else "draw"
                results[3]["output_class"] = "draw"
                results[5] = dict(status="expected-rejection", elapsed_ms=9000)
                results[6] = dict(status="expected-rejection" if data is left else "render-error", elapsed_ms=9000)
        (row,) = compare_reference(left, right)["scenarios"]
        self.assertEqual((row["common_stable"], row["common_completed"], row["common_expected_rejections"]), (6, 6, 1))
        self.assertIsNone(row["complete_batch_ms"])
        self.assertIsNone(row["common_render_ms"])
        groups = [(g["expected_outcome"], g["output_class"], g["request_indices"], g["render_ms"]) for g in row["render_timing_groups"]]
        self.assertEqual(groups, [(outcome, kind, indices, dict(left=ms, right=ms * 2, right_over_left=2))
                                  for outcome, kind, indices, ms in [("draw", "draw", [0, 7], 9), ("allow-empty", "empty", [1], 2),
                                                                    ("allow-empty", "draw", [3], 4)]])
        self.assertEqual([r["expected_outcome"] for r in row["requests"]], outcomes)
        self.assertEqual(row["requests"][5]["left"]["state"], "expected-rejection")
        self.assertEqual(row["requests"][6]["right"]["statuses"], ["render-error"])

    def test_unreviewed_and_unclassified_allowed_outputs_retain_observations_without_timings(self):
        left, right = report("node", "merman", [(2,)]), report("node", "pele", [(3,)])
        for outcome in ("allow-empty", "unreviewed"):
            for data in (left, right):
                row = data["scenarios"][0]
                row["requests"][0]["expected_outcome"] = outcome
                if outcome == "unreviewed":
                    for batch in [row["cold_batch"], *row["warm_samples"]]:
                        batch["results"][0]["output_class"] = "draw"
            with self.subTest(outcome=outcome):
                row, = compare_reference(left, right)["scenarios"]
                self.assertEqual(row["common_stable"], 1)
                self.assertEqual(row["render_timing_groups"], [])
                self.assertIsNone(row["common_render_ms"])
                self.assertIsNone(row["complete_batch_ms"])

    def test_allow_empty_cli_wall_clock_requires_the_same_observed_class(self):
        left, right = (report("cli", engine, [(2,), (3,)], fmt="png") for engine in ("merman", "mmdc"))
        for kind in ("empty", "draw"):
            for data in (left, right):
                row = data["scenarios"][0]
                row["requests"][0].update(expected_outcome="allow-empty", expected_labels=[])
                for batch in row["batches"]:
                    batch["results"][0].update(expected_outcome="allow-empty", output_class="empty" if data is left else kind)
            row, = compare_reference(left, right)["scenarios"]
            self.assertEqual(row["common_stable"], 1)
            if kind == "empty":
                self.assertEqual(row["complete_batch_ms"]["right_over_left"], 2)
            else:
                self.assertIsNone(row["complete_batch_ms"])
                self.assertIsNone(row["common_render_ms"])

    def test_failed_workers_need_a_complete_ordered_inventory_without_partial_measurements(self):
        left, right = report("node", "merman"), report("node", "pele")
        for raw in (left, right):
            raw["planned_scenarios"] = 1
            raw["scenarios"][0]["index"] = 0
        left["status"] = "failed"
        failed = left["scenarios"][0]
        failed.update(status="timeout", error="scenario timed out", unmeasured_requests=2,
                      init_ms=None, first_render_ms=None, cold_batch=None, warm_samples=[])
        for a, b in [(left, right), (right, left)]:
            row, = compare_reference(a, b)["scenarios"]
            missing, measured = ("left", "right") if a is left else ("right", "left")
            self.assertEqual(row["worker_failures"], {missing: {"status": "timeout", "error": "scenario timed out"}})
            self.assertEqual([r[missing]["state"] for r in row["requests"]], ["unmeasured"] * 2)
            self.assertEqual([r[measured]["state"] for r in row["requests"]], ["stable"] * 2)
            self.assertEqual((row["common_completed"], row["common_stable"], row["common_expected_rejections"]), (0, 0, 0))
            self.assertIsNone(row["complete_batch_ms"])
            self.assertIsNone(row["common_render_ms"])
        for attrs in ({"index": 1}, {"warm_samples": [{}]}, {"cold_batch": {}}, {"first_render_ms": 1}, {"status": "unavailable"},
                      {"unmeasured_requests": 1}, {"requests": list(reversed(failed["requests"]))}):
            with self.subTest(attrs=attrs), self.assertRaises(ValueError):
                compare_reference({**left, "scenarios": [{**failed, **attrs}]}, right)
        for change in ({"planned_scenarios": 2}, {"planned_scenarios": None}, {"status": "ok"}, {"failed_scenario": {}}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                compare_reference({**left, **change}, right)
        with self.assertRaises(ValueError):
            compare_reference({**right, "status": "failed"}, left)

    def test_default_png_and_explicit_options_keep_dimensions_and_coverage(self):
        left, right = report("cli", "merman", fmt="png"), report("cli", "mmdc", fmt="png")
        left["render_environment"] = right["render_environment"] = {
            "PUPPETEER_EXECUTABLE_PATH": "chrome"
        }
        (row,) = compare_reference(left, right)["scenarios"]
        self.assertEqual(row["complete_batch_ms"]["right_over_left"], 2)
        first = row["requests"][0]
        self.assertEqual(
            (first["left"]["dimensions_px"], first["right"]["dimensions_px"]),
            ([[300, 200]], [[600, 200]]),
        )
        for key, override in (
            ("font_files", [{"path": "A.ttf", "sha256": "c" * 64}]),
            ("extra_args", ["-c", "font.json"]),
            ("render_environment", {"FONTCONFIG_FILE": "fonts.conf"}),
        ):
            for both in (False, True):
                with self.subTest(key=key, both=both):
                    result = compare_reference(
                        {**left, key: override}, {**right, key: override} if both else right
                    )
                    self.assertTrue(result["ratio_withheld_reason"])
                    (explicit,) = result["scenarios"]
                    self.assertEqual(explicit["common_completed"], 2)
                    for metric in ("common_render_ms", "complete_batch_ms"):
                        self.assertIsNone(explicit[metric]["right_over_left"])

    def test_rejects_other_contracts_and_changed_input_under_same_plan_digest(self):
        left, right = report("node", "merman"), report("node", "pele")
        for key, value in (
            ("plan_sha256", "d" * 64),
            ("surface", "cli"),
            ("engine", "merman"),
            ("host", {**right["host"], "hostname": "other"}),
            ("output_format", "png"),
            ("package", left["package"]),
            ("samples", 2),
        ):
            with self.subTest(key=key), self.assertRaises(ValueError):
                compare_reference(left, {**right, key: value})
        with self.assertRaisesRegex(ValueError, "host"):
            compare_reference({**left, "host": "Windows"}, {**right, "host": "Windows"})
        with self.assertRaisesRegex(ValueError, "timeout_seconds"):
            compare_reference({**left, "timeout_seconds": 10}, {**right, "timeout_seconds": 20})
        with self.assertRaisesRegex(ValueError, "incomplete Node plan"):
            compare_reference({**left, "failed_scenario": {"id": "next"}}, right)
        for key, value in (
            ("source_sha256", "d" * 64),
            ("diagram_id", "other"),
            ("expected_labels", []),
            ("expected_outcome", "reject"),
        ):
            other = deepcopy(right)
            other["scenarios"][0]["requests"][0][key] = value
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "request identity"):
                compare_reference(left, other)

    def test_unrepeated_cli_or_missing_output_identity_withhold_time(self):
        for samples in (1, 2):
            times = [(4,)] * samples
            left, right = report("cli", "merman", times), report("cli", "mmdc", times)
            if samples == 2:
                del right["scenarios"][0]["batches"][1]["results"][0]["sha256"]
            with self.subTest(samples=samples):
                (row,) = compare_reference(left, right)["scenarios"]
                self.assertEqual(row["requests"][0]["right"]["state"], "unverified-output")
                self.assertEqual((row["common_completed"], row["common_stable"]), (1, 0))
                self.assertIsNone(row["complete_batch_ms"])


if __name__ == "__main__":
    unittest.main()
