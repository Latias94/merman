#!/usr/bin/env python3
"""Aggregate the focused performance contract test owners."""

import sys
import unittest

TEST_MODULES = (
    "test_perf_corpus_contracts",
    "test_perf_runner_contracts",
    "test_perf_recipe_contracts",
    "test_perf_statistics_contracts",
    "test_perf_report_contracts",
    "test_perf_workflow_contracts",
)


def main() -> int:
    loader = unittest.defaultTestLoader
    suite = unittest.TestSuite(
        loader.loadTestsFromName(module_name) for module_name in TEST_MODULES
    )
    result = unittest.TextTestRunner().run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main())
