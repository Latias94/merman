import hashlib
from argparse import Namespace
from pathlib import Path
import sys
import unittest


BENCH = Path(__file__).resolve().parent
if str(BENCH) not in sys.path:
    sys.path.insert(0, str(BENCH))

import run_workloads as bench
from workload_svg import validate_svg


DOCUMENTS = BENCH / "documents"
GENERATED = "mermaid-examples-generated"
# Opening fence line, trailing LFs omitted by the fixture, SHA-256 of the upstream fence body.
# The full UTF-8 bodies come from the Mermaid commit linked in each manifest's origin.
AUTHOR_FENCES = {
    "mermaid-examples-author": [
        (11, 0, "28b468acdb67b7088180448fc08db4a6fe7a84a7cd3c5bf29e251274e1bae9b3"),
        (17, 0, "52c8e87d218519c91de4d0e57da6728f39330624d7bc2f014d0ddfa4eb025928"),
        (26, 0, "b8e52ce7d792978000b1909b6aa84cf3ba5943bdaf7769d8cfef92c09aa390a4"),
        (40, 0, "d8c6a25c7cfac0a026c1d7e304a2f7c0b52636878ac64f0de3a797c0cfee5583"),
        (50, 0, "22b7b9e26643b7efeba14a4a6ad048fac3a33832d8a2c7b2a965facd788614ca"),
        (76, 0, "c6681aebec310400ed7c61ecc1869b7957a6eb8e8526bdb09b765fcf0a30e21d"),
        (94, 0, "e31e84a14d63985055ce1cdd577df24ef5b9f713079ade3701277b0015f86ce0"),
        (110, 1, "48067e3330977a84023ce4439d809df7bdb2c569f3f1fc9121039c6a433c8b57"),
        (144, 0, "ea62bfe1c68c4205bdd33b6353dcdd17320b4bfb5d737611e1dcd73d7aa41ead"),
    ],
    "mermaid-c4-author": [
        (7, 2, "f96d24037e03cbf93c7027ba4a5dc84f351183e98b68b854a78d223aec22823f"),
        (175, 1, "bc80e69a491d586884cdcf98e50a1dae32190551b919554098eb25f914c301c3"),
        (225, 1, "96b1a834b23451a341a0991d5327a833e705801ea1bb53c90afb98165035cf2e"),
        (267, 1, "e233a72687411e6bf3ff6c128a3211bfa260621c90e5c7f49f6b935d0b0844ee"),
        (309, 1, "5f697bdc17bc182b87987d6e376658eac959975e6a62cffcda427fbbf1682e58"),
        (331, 1, "90f106b92d6089986447c7595f2d9a875ea6458535a81f2ca9a62b5925121100"),
    ],
}


class WorkloadDocumentsTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        names = [*AUTHOR_FENCES, GENERATED]
        manifests = [DOCUMENTS / f"{name}.json" for name in names]
        plan = bench.materialize(
            Namespace(suite="quick", scenario=[], document=manifests, pele_fixtures=None)
        )
        cls.requests = {
            case["id"]: case["requests"] for case in plan["scenarios"] if case["id"] in names
        }

    def test_author_fixtures_match_pinned_fences_in_reading_order(self):
        for name, fences in AUTHOR_FENCES.items():
            requests = self.requests[name]
            self.assertEqual(len(requests), len(fences), name)
            for request, (line, omitted_lfs, upstream_hash) in zip(requests, fences, strict=True):
                with self.subTest(document=name, line=line):
                    filename = Path(request["fixture"])
                    self.assertFalse(filename.is_absolute())
                    fixture = (DOCUMENTS / filename).resolve()
                    self.assertTrue(fixture.is_relative_to(bench.ROOT / "fixtures"))
                    self.assertEqual(request["source"], fixture.read_text(encoding="utf-8"))
                    upstream = request["source"].encode("utf-8") + b"\n" * omitted_lfs
                    self.assertEqual(hashlib.sha256(upstream).hexdigest(), upstream_hash)

        first, second = self.requests["mermaid-c4-author"][:2]
        self.assertEqual(first["fixture"], second["fixture"])
        self.assertNotEqual(first["diagram_id"], second["diagram_id"])

    def test_generated_site_pairs_each_author_occurrence_with_its_own_copy(self):
        author = self.requests["mermaid-examples-author"]
        generated = self.requests[GENERATED]
        for field in ("fixture", "expected_labels"):
            self.assertEqual(
                [r[field] for r in generated], [r[field] for r in author for _ in range(2)], field
            )
        self.assertEqual(len({r["diagram_id"] for r in generated}), len(generated))

    def test_generated_site_weight_requires_an_explicit_manifest(self):
        full = bench.materialize(Namespace(suite="full", scenario=[], document=[], pele_fixtures=None))
        self.assertNotIn(GENERATED, {case["id"] for case in full["scenarios"]})

    def test_reviewed_labels_match_the_upstream_svgs(self):
        fixtures = bench.ROOT / "fixtures"
        for name in AUTHOR_FENCES:
            for request in self.requests[name]:
                with self.subTest(document=name, fixture=request["fixture"]):
                    relative = (DOCUMENTS / request["fixture"]).resolve().relative_to(fixtures)
                    svg = fixtures / "upstream-svgs" / relative.with_suffix(".svg")
                    labels = request["expected_labels"]
                    self.assertTrue(labels)
                    self.assertEqual(validate_svg(svg.read_bytes(), labels), {"status": "ok"})

    def test_sequence_section_labels_use_the_displayed_brackets(self):
        sections = self.requests["mermaid-examples-author"][5:8]
        displayed = ("[Daily query]", "[HealthCheck]", "[Credentials not found]")
        for request, text in zip(sections, displayed, strict=True):
            self.assertIn(text, request["expected_labels"], request["fixture"])


if __name__ == "__main__":
    unittest.main()
