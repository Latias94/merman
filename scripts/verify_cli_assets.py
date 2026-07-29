#!/usr/bin/env python3
"""Validate generated merman-cli completion scripts and manual pages."""

from __future__ import annotations

import argparse
from collections.abc import Callable, Iterable, Sequence
import os
from pathlib import Path
import shutil
import subprocess
import sys
from typing import TypeAlias


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_ASSET_ROOT = REPO_ROOT / "crates" / "merman-cli" / "assets"
CHECK_IDS = ("bash", "zsh", "fish", "powershell", "elvish", "mandoc")
COMPLETION_PATHS = {
    "bash": Path("completions/merman-cli.bash"),
    "zsh": Path("completions/_merman-cli"),
    "fish": Path("completions/merman-cli.fish"),
    "powershell": Path("completions/merman-cli.ps1"),
    "elvish": Path("completions/merman-cli.elv"),
}

CommandRunner: TypeAlias = Callable[..., subprocess.CompletedProcess[str]]
ExecutableFinder: TypeAlias = Callable[[str], str | None]


class AssetValidationError(RuntimeError):
    """A generated CLI support asset is missing or invalid."""


def parse_required_checks(value: str) -> frozenset[str]:
    required = frozenset(item.strip() for item in value.split(",") if item.strip())
    unknown = sorted(required - set(CHECK_IDS))
    if unknown:
        raise argparse.ArgumentTypeError(
            "unknown required check(s): " + ", ".join(unknown)
        )
    return required


def find_executable(check_id: str, finder: ExecutableFinder) -> str | None:
    candidates = ("pwsh", "powershell") if check_id == "powershell" else (check_id,)
    return next((path for name in candidates if (path := finder(name)) is not None), None)


def syntax_command(check_id: str, executable: str, path: Path) -> tuple[list[str], dict[str, str]]:
    environment = os.environ.copy()
    if check_id == "bash":
        return [executable, "-n", str(path)], environment
    if check_id == "zsh":
        return [executable, "-n", str(path)], environment
    if check_id == "fish":
        return [executable, "--no-execute", str(path)], environment
    if check_id == "elvish":
        return [executable, "-compileonly", str(path)], environment
    if check_id == "powershell":
        environment["MERMAN_COMPLETION_PATH"] = str(path)
        parser = (
            "$tokens = $null; $errors = $null; "
            "[System.Management.Automation.Language.Parser]::ParseFile("
            "$env:MERMAN_COMPLETION_PATH, [ref]$tokens, [ref]$errors) | Out-Null; "
            "if ($errors.Count -ne 0) { "
            "$errors | ForEach-Object { [Console]::Error.WriteLine($_.Message) }; exit 1 }"
        )
        return [
            executable,
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            parser,
        ], environment
    if check_id == "mandoc":
        return [
            executable,
            "-T",
            "lint",
            "-W",
            "warning",
            str(path),
        ], environment
    raise AssetValidationError(f"unsupported CLI asset check {check_id!r}")


def run_checked(
    check_id: str,
    path: Path,
    command: Sequence[str],
    environment: dict[str, str],
    runner: CommandRunner,
) -> subprocess.CompletedProcess[str]:
    result = runner(
        list(command),
        cwd=REPO_ROOT,
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        details = "\n".join(
            part
            for part in (
                result.stdout.strip(),
                result.stderr.strip(),
            )
            if part
        )
        suffix = f":\n{details}" if details else ""
        raise AssetValidationError(
            f"{check_id} rejected generated asset {path}{suffix}"
        )
    return result


def verify_manpage(
    executable: str,
    path: Path,
    runner: CommandRunner,
) -> None:
    lint_command, environment = syntax_command("mandoc", executable, path)
    lint = run_checked("mandoc-lint", path, lint_command, environment, runner)
    if lint.stdout or lint.stderr:
        raise AssetValidationError(
            f"mandoc lint emitted diagnostics for generated asset {path}:\n"
            f"{lint.stdout}{lint.stderr}".rstrip()
        )

    rendered = run_checked(
        "mandoc-render",
        path,
        [executable, "-T", "utf8", str(path)],
        environment,
        runner,
    )
    if not rendered.stdout.strip():
        raise AssetValidationError(f"mandoc rendered an empty manual page for {path}")


def verify_bash_routing(
    executable: str,
    path: Path,
    runner: CommandRunner,
) -> None:
    environment = os.environ.copy()
    environment["MERMAN_COMPLETION_PATH"] = str(path)
    probe = r'''
set -eo pipefail
source "$MERMAN_COMPLETION_PATH"
probe_completion() {
    local subcommand="$1"
    local current="$2"
    COMP_WORDS=(merman-cli "$subcommand" "$current")
    COMP_CWORD=2
    _merman-cli merman-cli "$current" "$subcommand"
    printf '%s\n' "${COMPREPLY[@]}"
}
probe_completion render --f
printf '%s\n' __MMDC__
probe_completion mmdc -e
'''
    result = run_checked(
        "bash-routing",
        path,
        [executable, "--noprofile", "--norc", "-c", probe],
        environment,
        runner,
    )
    native, separator, mmdc = result.stdout.partition("__MMDC__\n")
    if not separator or "--format" not in native.splitlines() or "-e" not in mmdc.splitlines():
        raise AssetValidationError(
            "Bash completion does not route render -f and mmdc -e through their "
            f"generated subcommand states:\n{result.stdout.rstrip()}"
        )


def manpage_paths(asset_root: Path) -> list[Path]:
    paths = sorted((asset_root / "man").glob("*.1"))
    if not paths:
        raise AssetValidationError(f"no generated manpages under {asset_root / 'man'}")
    return paths


def verify_assets(
    asset_root: Path,
    *,
    required: Iterable[str] = (),
    finder: ExecutableFinder | None = None,
    runner: CommandRunner | None = None,
) -> tuple[frozenset[str], frozenset[str]]:
    required_set = frozenset(required)
    unknown = sorted(required_set - set(CHECK_IDS))
    if unknown:
        raise AssetValidationError("unknown required check(s): " + ", ".join(unknown))

    finder = finder or shutil.which
    runner = runner or subprocess.run
    checked: set[str] = set()
    skipped: set[str] = set()

    for check_id, relative_path in COMPLETION_PATHS.items():
        path = asset_root / relative_path
        if not path.is_file():
            raise AssetValidationError(f"missing generated completion asset {path}")
        executable = find_executable(check_id, finder)
        if executable is None:
            if check_id in required_set:
                raise AssetValidationError(
                    f"required {check_id} parser is not available on PATH"
                )
            skipped.add(check_id)
            continue
        command, environment = syntax_command(check_id, executable, path)
        run_checked(check_id, path, command, environment, runner)
        if check_id == "bash":
            verify_bash_routing(executable, path, runner)
        checked.add(check_id)

    mandoc = find_executable("mandoc", finder)
    if mandoc is None:
        if "mandoc" in required_set:
            raise AssetValidationError("required mandoc parser is not available on PATH")
        skipped.add("mandoc")
    else:
        for path in manpage_paths(asset_root):
            verify_manpage(mandoc, path, runner)
        checked.add("mandoc")

    return frozenset(checked), frozenset(skipped)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--asset-root",
        type=Path,
        default=DEFAULT_ASSET_ROOT,
        help="generated merman-cli asset root",
    )
    parser.add_argument(
        "--require",
        type=parse_required_checks,
        default=frozenset(),
        metavar="ID,...",
        help="fail unless these native checkers run",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    checked, skipped = verify_assets(args.asset_root, required=args.require)
    print("validated CLI assets with: " + ", ".join(sorted(checked)))
    if skipped:
        print("skipped unavailable optional checkers: " + ", ".join(sorted(skipped)))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AssetValidationError, OSError) as error:
        print(f"verify_cli_assets.py: {error}", file=sys.stderr)
        raise SystemExit(1) from error
