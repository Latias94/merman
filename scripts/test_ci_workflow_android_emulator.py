#!/usr/bin/env python3
"""Contract tests for the Android emulator CI smoke gate."""

from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CI_WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"


def read_ci_workflow() -> str:
    return CI_WORKFLOW.read_text(encoding="utf-8")


def index_of_step(text: str, step_name: str) -> int:
    marker = f"- name: {step_name}"
    try:
        return text.index(marker)
    except ValueError as exc:
        raise AssertionError(f"ci.yml does not define step {step_name!r}") from exc


class AndroidEmulatorWorkflowTests(unittest.TestCase):
    def test_android_emulator_enables_kvm_before_launch(self) -> None:
        text = read_ci_workflow()

        kvm_step = index_of_step(text, "Enable KVM access")
        emulator_step = index_of_step(text, "Run Android instrumentation smoke")

        self.assertLess(kvm_step, emulator_step)
        self.assertIn('/dev/kvm is unavailable', text)
        self.assertIn('MODE="0666"', text)
        self.assertIn("sudo udevadm trigger --name-match=kvm", text)

    def test_android_emulator_requires_hardware_acceleration(self) -> None:
        text = read_ci_workflow()

        self.assertIn("disable-linux-hw-accel: false", text)
        self.assertIn("emulator-boot-timeout: 900", text)
        self.assertIn("-no-metrics", text)


if __name__ == "__main__":
    unittest.main()
