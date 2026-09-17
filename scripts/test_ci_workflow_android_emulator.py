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


def step_block(text: str, step_name: str) -> str:
    start = index_of_step(text, step_name)
    lines = text[start:].splitlines()
    if not lines:
        raise AssertionError(f"ci.yml does not define step {step_name!r}")

    marker_indent = len(lines[0]) - len(lines[0].lstrip(" "))
    block: list[str] = [lines[0]]
    for line in lines[1:]:
        if line.strip() == "":
            block.append(line)
            continue

        indent = len(line) - len(line.lstrip(" "))
        if indent <= marker_indent:
            break
        block.append(line)
    return "\n".join(block)


class AndroidEmulatorWorkflowTests(unittest.TestCase):
    def test_android_sdk_omits_retired_tools_package(self) -> None:
        text = read_ci_workflow()
        setup = step_block(text, "Setup Android SDK")
        install = step_block(text, "Install Android NDK")

        self.assertIn("packages: platform-tools", setup)
        self.assertIn('"ndk;${ANDROID_NDK_VERSION}"', install)
        self.assertIn('"platforms;android-35"', install)

    def test_all_android_setup_steps_override_legacy_package_default(self) -> None:
        for workflow in (ROOT / ".github" / "workflows").glob("*.yml"):
            text = workflow.read_text(encoding="utf-8")
            for setup in text.split("uses: android-actions/setup-android@")[1:]:
                with self.subTest(workflow=workflow.name):
                    inputs = setup.split("\n      - ", 1)[0]
                    self.assertIn("packages: platform-tools\n", inputs)

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
        emulator_step = step_block(text, "Run Android instrumentation smoke")

        self.assertIn("uses: reactivecircus/android-emulator-runner@", emulator_step)
        self.assertIn("api-level: 29", emulator_step)
        self.assertIn("target: default", emulator_step)
        self.assertNotIn("target: google_apis", emulator_step)
        self.assertIn("disable-linux-hw-accel: false", emulator_step)
        self.assertIn("emulator-boot-timeout: 900", emulator_step)
        self.assertIn("-no-metrics", emulator_step)

if __name__ == "__main__":
    unittest.main()
