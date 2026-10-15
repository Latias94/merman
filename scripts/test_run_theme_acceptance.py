"""Contracts for the explicit workspace acceptance runner."""

import unittest

from scripts.run_theme_acceptance import ACCEPTANCE_CFG, acceptance_environment


class AcceptanceEnvironmentTests(unittest.TestCase):
    def test_preserves_encoded_flag_boundaries_and_cargo_precedence(self):
        source = {
            "CARGO_ENCODED_RUSTFLAGS": "--remap-path-prefix\x1fa path=another path",
            "RUSTFLAGS": "--ignored",
            "RUSTDOCFLAGS": "--deny rustdoc::broken_intra_doc_links",
        }
        actual = acceptance_environment(source)
        self.assertEqual(
            actual["CARGO_ENCODED_RUSTFLAGS"].split("\x1f"),
            ["--remap-path-prefix", "a path=another path", "--cfg", ACCEPTANCE_CFG],
        )
        self.assertEqual(
            actual["RUSTDOCFLAGS"],
            f"--deny rustdoc::broken_intra_doc_links --cfg {ACCEPTANCE_CFG}",
        )
        self.assertNotIn(ACCEPTANCE_CFG, source["CARGO_ENCODED_RUSTFLAGS"])

    def test_ordinary_flags_and_empty_encoded_override(self):
        for source, expected in [
            ({"RUSTFLAGS": "-C debuginfo=1"}, ["-C", "debuginfo=1"]),
            ({"RUSTFLAGS": "--ignored", "CARGO_ENCODED_RUSTFLAGS": ""}, []),
            ({}, []),
        ]:
            with self.subTest(source=source):
                actual = acceptance_environment(source)
                self.assertEqual(
                    actual["CARGO_ENCODED_RUSTFLAGS"].split("\x1f"),
                    [*expected, "--cfg", ACCEPTANCE_CFG],
                )


if __name__ == "__main__":
    unittest.main()
