#!/usr/bin/env python3
"""Run the exact process-level feature matrix for merman-cli."""

from __future__ import annotations

import argparse
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
import os
from pathlib import Path
import signal
import subprocess
import sys
from typing import TypeAlias


REPO_ROOT = Path(__file__).resolve().parents[1]
PROFILE_CASE_ENV = "MERMAN_CLI_PROFILE_CASE"
CASE_TIMEOUT_SECONDS = 20 * 60
TERMINATION_GRACE_SECONDS = 5.0

CommandRunner: TypeAlias = Callable[..., subprocess.CompletedProcess[object]]


@dataclass(frozen=True)
class ProfileCase:
    """One exact Cargo feature selection and its required runtime workflow."""

    case_id: str
    name: str
    features: tuple[str, ...]
    workflow: str
    use_all_features: bool = False
    use_default_features: bool = False

    def __post_init__(self) -> None:
        if self.use_all_features and self.use_default_features:
            raise ValueError(f"{self.name} cannot select defaults and --all-features")
        if (self.use_all_features or self.use_default_features) and self.features:
            raise ValueError(f"{self.name} cannot list explicit features in aggregate mode")
        if not self.case_id or not self.name or not self.workflow:
            raise ValueError("profile cases require an id, name, and workflow")
        if any(not feature for feature in self.features):
            raise ValueError(f"{self.name} contains an empty Cargo feature")


PROFILE_CASES = (
    ProfileCase(
        "base",
        "Base",
        (),
        "Detect and parse stdin; unavailable commands are absent.",
    ),
    ProfileCase(
        "analysis",
        "Analysis",
        ("analysis",),
        "Lint and fix stdin without render dependencies.",
    ),
    ProfileCase(
        "svg",
        "SVG",
        ("svg",),
        "Render one SVG to stdout and to an atomic file target.",
    ),
    ProfileCase(
        "ascii",
        "ASCII",
        ("ascii",),
        "Render ASCII and Unicode through the shared executor without SVG.",
    ),
    ProfileCase(
        "local-icons",
        "Local icons",
        ("icons",),
        "Load a bounded local icon pack and render it.",
    ),
    ProfileCase(
        "markdown",
        "Markdown",
        ("markdown",),
        "Run sequential native batch publication and recovery smoke.",
    ),
    ProfileCase(
        "parallel-markdown",
        "Parallel Markdown",
        ("parallel-markdown",),
        "Render a multi-chart SVG Markdown batch with bounded parallel scheduling.",
    ),
    ProfileCase(
        "network-icons",
        "Network icons",
        ("network-icons",),
        "Reject unauthorized loopback, then load an authorized bounded fixture.",
    ),
    ProfileCase(
        "png",
        "PNG",
        ("png",),
        "Render and validate one PNG.",
    ),
    ProfileCase(
        "jpeg",
        "JPEG",
        ("jpeg",),
        "Render and validate one JPEG.",
    ),
    ProfileCase(
        "pdf",
        "PDF",
        ("pdf",),
        "Render and validate one PDF.",
    ),
    ProfileCase(
        "parallel-pdf",
        "Parallel PDF",
        ("parallel-markdown", "pdf"),
        "Render a multi-chart Markdown batch with bounded parallel scheduling.",
    ),
    ProfileCase(
        "cytoscape-layout",
        "Cytoscape layout",
        ("layout-cytoscape",),
        "Render a family that calls the compiled Cytoscape layout.",
    ),
    ProfileCase(
        "elk-layout",
        "ELK layout",
        ("layout-elk",),
        "Render a family that calls the compiled ELK layout.",
    ),
    ProfileCase(
        "math",
        "Math",
        ("math",),
        "Render one RaTeX expression.",
    ),
    ProfileCase(
        "completions",
        "Completions",
        ("shell-completions",),
        "Generate one completion script with only compiled commands.",
    ),
    ProfileCase(
        "svg-completions",
        "SVG completions",
        ("shell-completions", "svg"),
        "Generate completion values for a representative slim render surface.",
    ),
    ProfileCase(
        "system-clock",
        "System clock",
        ("system-clock",),
        "Invoke the clock adapter flag without the native runtime shortcut.",
    ),
    ProfileCase(
        "system-timezone",
        "System timezone",
        ("system-timezone",),
        "Invoke the timezone adapter flag without the native runtime shortcut.",
    ),
    ProfileCase(
        "system-random",
        "System random",
        ("system-random",),
        "Invoke the random adapter flag without the native runtime shortcut.",
    ),
    ProfileCase(
        "system-timing",
        "System timing",
        ("system-timing",),
        "Invoke the timing adapter flag without the native runtime shortcut.",
    ),
    ProfileCase(
        "default",
        "Default",
        (),
        "Exercise the distributed defaults and prove the release descriptor is exact.",
        use_default_features=True,
    ),
    ProfileCase(
        "release",
        "Release",
        (),
        "Exercise every cfg branch and the native runtime shortcut.",
        use_all_features=True,
    ),
)


class ProcessMatrixError(RuntimeError):
    """A process-matrix case could not be started or did not pass."""

    def __init__(
        self,
        profile: ProfileCase,
        message: str,
        *,
        returncode: int | None = None,
    ) -> None:
        super().__init__(f"CLI profile matrix case {profile.name!r} failed: {message}")
        self.profile = profile
        self.returncode = returncode


def _terminate_process_tree(process: subprocess.Popen[object]) -> None:
    """Stop a timed-out case and every descendant it owns where possible."""

    if os.name == "posix":
        process_group_id = process.pid
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            process.wait()
            return

        try:
            process.wait(timeout=TERMINATION_GRACE_SECONDS)
        except subprocess.TimeoutExpired:
            pass
        try:
            os.killpg(process_group_id, signal.SIGKILL)
        except ProcessLookupError:
            pass
        if process.poll() is None:
            process.kill()
            process.wait()
        return


    _terminate_windows_process_tree(process)


def _terminate_windows_process_tree(process: subprocess.Popen[object]) -> None:
    """Use the Windows tree-aware process terminator, with a local fallback."""

    try:
        subprocess.run(
            ["taskkill", "/PID", str(process.pid), "/T", "/F"],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=TERMINATION_GRACE_SECONDS,
        )
    except (OSError, subprocess.TimeoutExpired):
        if process.poll() is None:
            process.kill()
    finally:
        if process.poll() is None:
            process.wait()


def run_case_subprocess(
    command: Sequence[str],
    *,
    cwd: Path,
    env: Mapping[str, str],
    timeout: float,
) -> subprocess.CompletedProcess[object]:
    """Run one matrix row with a deadline and isolated process group."""

    process_group_kwargs: dict[str, object]
    if os.name == "posix":
        process_group_kwargs = {"start_new_session": True}
    else:
        process_group_kwargs = {
            "creationflags": subprocess.CREATE_NEW_PROCESS_GROUP,
        }

    process = subprocess.Popen(
        command,
        cwd=cwd,
        env=dict(env),
        **process_group_kwargs,
    )
    try:
        returncode = process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        _terminate_process_tree(process)
        raise
    return subprocess.CompletedProcess(command, returncode)


def cargo_nextest_args(
    profile: ProfileCase,
    *,
    locked: bool = False,
) -> list[str]:
    """Project one profile row into its exact nextest invocation."""

    args = ["cargo", "nextest", "run"]
    if locked:
        args.append("--locked")
    args.extend(["-p", "merman-cli"])

    if profile.use_all_features:
        args.append("--all-features")
    elif not profile.use_default_features:
        args.append("--no-default-features")
        if profile.features:
            args.extend(["--features", ",".join(profile.features)])

    args.extend(["--test", "profile_contract"])
    return args


def run_process_matrix(
    *,
    locked: bool = False,
    repo_root: Path = REPO_ROOT,
    environment: Mapping[str, str] | None = None,
    runner: CommandRunner = run_case_subprocess,
    profiles: Sequence[ProfileCase] = PROFILE_CASES,
) -> None:
    """Run each exact profile sequentially and stop at its first failure."""

    base_environment = dict(os.environ if environment is None else environment)
    for index, profile in enumerate(profiles, start=1):
        command = cargo_nextest_args(profile, locked=locked)
        case_environment = dict(base_environment)
        case_environment[PROFILE_CASE_ENV] = profile.case_id
        print(
            f"[{index}/{len(profiles)}] {profile.name}: {profile.workflow}",
            flush=True,
        )
        try:
            completed = runner(
                command,
                cwd=repo_root,
                env=case_environment,
                timeout=CASE_TIMEOUT_SECONDS,
            )
        except subprocess.TimeoutExpired as error:
            raise ProcessMatrixError(
                profile,
                f"nextest timed out after {CASE_TIMEOUT_SECONDS} seconds; terminated its process group",
            ) from error
        except OSError as error:
            raise ProcessMatrixError(profile, f"could not start Cargo: {error}") from error
        if completed.returncode != 0:
            raise ProcessMatrixError(
                profile,
                f"nextest exited with status {completed.returncode}",
                returncode=completed.returncode,
            )


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Run merman-cli's process contract once for every supported exact "
            "feature profile."
        )
    )
    parser.add_argument(
        "--locked",
        action="store_true",
        help="Pass --locked to every Cargo invocation.",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        run_process_matrix(locked=args.locked)
    except ProcessMatrixError as error:
        print(f"error: {error}", file=sys.stderr)
        if error.returncode is not None and 1 <= error.returncode <= 255:
            return error.returncode
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
