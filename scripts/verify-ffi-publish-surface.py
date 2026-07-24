#!/usr/bin/env python3
"""Verify FFI ABI, package metadata, and canonical Python example contracts."""

from __future__ import annotations

import ast
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TEXT_MEASUREMENT_PROTOCOL_ID = "merman-text-measurement"
TEXT_MEASUREMENT_PROTOCOL_VERSION = 1


class CheckFailure(Exception):
    pass


def read_text(rel_path: str) -> str:
    return (ROOT / rel_path).read_text(encoding="utf-8")


def require_file(rel_path: str, label: str) -> None:
    path = ROOT / rel_path
    if not path.is_file():
        raise CheckFailure(f"{rel_path}: missing {label}")


def require_match(rel_path: str, pattern: str, label: str) -> str:
    text = read_text(rel_path)
    match = re.search(pattern, text, flags=re.MULTILINE)
    if not match:
        raise CheckFailure(f"{rel_path}: missing {label}")
    return match.group(1)


def require_contains(rel_path: str, needle: str, label: str) -> None:
    if needle not in read_text(rel_path):
        raise CheckFailure(f"{rel_path}: missing {label}")


def dotted_name(node: ast.expr) -> str | None:
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        parent = dotted_name(node.value)
        if parent is not None:
            return f"{parent}.{node.attr}"
    return None


def python_fences(rel_path: str) -> list[tuple[int, str]]:
    blocks: list[tuple[int, str]] = []
    lines = read_text(rel_path).splitlines()
    start_line: int | None = None
    current: list[str] = []
    for line_number, line in enumerate(lines, start=1):
        if start_line is None:
            if line.strip().lower() == "```python":
                start_line = line_number + 1
                current = []
            continue
        if line.strip() == "```":
            blocks.append((start_line, "\n".join(current) + "\n"))
            start_line = None
            current = []
            continue
        current.append(line)
    if start_line is not None:
        raise CheckFailure(f"{rel_path}:{start_line - 1}: unclosed Python code fence")
    return blocks


def call_argument_count(call: ast.Call, label: str) -> int:
    if any(isinstance(argument, ast.Starred) for argument in call.args):
        raise CheckFailure(f"{label}:{call.lineno}: starred arguments hide the UniFFI call shape")
    if any(keyword.arg is None for keyword in call.keywords):
        raise CheckFailure(f"{label}:{call.lineno}: expanded keywords hide the UniFFI call shape")
    return len(call.args) + len(call.keywords)


def validate_python_uniffi_usage(
    source: str,
    label: str,
    *,
    require_text_measurer: bool = False,
    allowed_calls: set[tuple[str, str, int]] | None = None,
) -> set[tuple[str, str, int]]:
    try:
        tree = ast.parse(source, filename=label)
    except SyntaxError as exc:
        raise CheckFailure(f"{label}:{exc.lineno}: invalid Python: {exc.msg}") from exc

    engine_names: set[str] = set()
    reusable_names: set[str] = set()
    measurer_classes = 0

    for node in ast.walk(tree):
        if isinstance(node, ast.ClassDef):
            is_measurer = any(
                (dotted_name(base) or "").endswith(".MermanTextMeasurer")
                or dotted_name(base) == "MermanTextMeasurer"
                for base in node.bases
            )
            if not is_measurer:
                continue
            measurer_classes += 1
            methods = {
                item.name: item
                for item in node.body
                if isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef))
            }
            if "measure_text" in methods:
                raise CheckFailure(
                    f"{label}:{methods['measure_text'].lineno}: UniFFI callback is measure(), not measure_text()"
                )
            measure = methods.get("measure")
            if measure is None:
                raise CheckFailure(
                    f"{label}:{node.lineno}: MermanTextMeasurer subclass must implement measure(self, request)"
                )
            if isinstance(measure, ast.AsyncFunctionDef):
                raise CheckFailure(
                    f"{label}:{measure.lineno}: measure callback must be synchronous"
                )
            positional = [*measure.args.posonlyargs, *measure.args.args]
            if [argument.arg for argument in positional] != ["self", "request"]:
                raise CheckFailure(
                    f"{label}:{measure.lineno}: measure callback must have signature measure(self, request)"
                )
            if (
                measure.args.vararg is not None
                or measure.args.kwarg is not None
                or measure.args.kwonlyargs
                or measure.args.defaults
                or measure.args.kw_defaults
            ):
                raise CheckFailure(
                    f"{label}:{measure.lineno}: measure callback must have no optional or variadic arguments"
                )

        if not isinstance(node, (ast.Assign, ast.AnnAssign)):
            continue
        value = node.value
        if not isinstance(value, ast.Call):
            continue
        if isinstance(node, ast.Assign):
            targets = [target for target in node.targets if isinstance(target, ast.Name)]
        else:
            targets = [node.target] if isinstance(node.target, ast.Name) else []
        if not targets:
            continue
        call_name = dotted_name(value.func)
        if call_name in {"merman.MermanEngine", "MermanEngine"}:
            engine_names.update(target.id for target in targets)
            continue
        if isinstance(value.func, ast.Attribute) and value.func.attr in {
            "reusable_engine",
            "reusable_engine_with_text_measurer",
        }:
            reusable_names.update(target.id for target in targets)

    if require_text_measurer and measurer_classes == 0:
        raise CheckFailure(f"{label}: missing executable MermanTextMeasurer example")

    observed_calls: set[tuple[str, str, int]] = set()
    known_engine_methods = (
        {method for _, method, _ in allowed_calls} if allowed_calls is not None else set()
    )
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call) or not isinstance(node.func, ast.Attribute):
            continue
        receiver = dotted_name(node.func.value)
        method = node.func.attr
        if receiver == "merman" and method in known_engine_methods:
            raise CheckFailure(
                f"{label}:{node.lineno}: {method} is an engine method, not a merman module function"
            )
        owner: str | None = None
        if receiver in engine_names:
            owner = "MermanEngine"
        elif receiver in reusable_names:
            owner = "MermanReusableEngine"
        if owner is None:
            continue
        actual = call_argument_count(node, label)
        shape = (owner, method, actual)
        observed_calls.add(shape)
        if allowed_calls is not None and shape not in allowed_calls:
            expected_arities = sorted(
                arity
                for allowed_owner, allowed_method, arity in allowed_calls
                if allowed_owner == owner and allowed_method == method
            )
            if not expected_arities:
                raise CheckFailure(
                    f"{label}:{node.lineno}: {owner}.{method} is absent from the bindgen-executed example"
                )
            expected = " or ".join(str(arity) for arity in expected_arities)
            raise CheckFailure(
                f"{label}:{node.lineno}: {owner}.{method} expects {expected} argument(s); got {actual}"
            )
    return observed_calls


def check_python_examples() -> None:
    smoke_path = "platforms/python/merman/examples/smoke.py"
    canonical_calls = validate_python_uniffi_usage(
        read_text(smoke_path), smoke_path, require_text_measurer=True
    )
    for rel_path in [
        "docs/bindings/HOST_TEXT_MEASUREMENT.md",
        "docs/bindings/PYTHON_UNIFFI.md",
        "platforms/python/merman/README.md",
    ]:
        blocks = python_fences(rel_path)
        if not blocks:
            raise CheckFailure(f"{rel_path}: missing Python example")
        combined = "\n".join(source for _, source in blocks)
        validate_python_uniffi_usage(
            combined,
            rel_path,
            require_text_measurer=rel_path != "platforms/python/merman/README.md",
            allowed_calls=canonical_calls,
        )
    print("Python UniFFI examples: AST contract valid; canonical smoke is bindgen-executed")


def check_python_package_exports() -> None:
    rel_path = "platforms/python/merman/src/merman/__init__.py"
    try:
        tree = ast.parse(read_text(rel_path), filename=rel_path)
    except SyntaxError as exc:
        raise CheckFailure(f"{rel_path}:{exc.lineno}: invalid Python: {exc.msg}") from exc

    imported: set[str] = set()
    exported: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.ImportFrom) and node.module in {
            "merman_uniffi",
            "_resource_options",
        }:
            imported.update(alias.asname or alias.name for alias in node.names)
    for node in tree.body:
        if not isinstance(node, ast.Assign):
            continue
        if not any(isinstance(target, ast.Name) and target.id == "__all__" for target in node.targets):
            continue
        if isinstance(node.value, (ast.List, ast.Tuple)):
            exported.update(
                item.value
                for item in node.value.elts
                if isinstance(item, ast.Constant) and isinstance(item.value, str)
            )

    required = {
        "MermanEngine",
        "MermanReusableEngine",
        "MermanTextMeasurer",
        "ResourceLimitId",
        "ResourceOptions",
        "ResourceOptionsBuilder",
        "ResourceProfile",
    }
    missing_imports = sorted(required - imported)
    missing_exports = sorted(required - exported)
    if missing_imports:
        raise CheckFailure(f"{rel_path}: missing generated imports {', '.join(missing_imports)}")
    if missing_exports:
        raise CheckFailure(f"{rel_path}: missing __all__ exports {', '.join(missing_exports)}")


def check_text_measurement_protocol() -> None:
    descriptor_path = ROOT / "abi" / "text-measurement-v1.json"
    descriptor = json.loads(descriptor_path.read_text(encoding="utf-8"))
    if descriptor.get("schema_version") != 1:
        raise CheckFailure(f"{descriptor_path}: schema_version must be 1")
    if descriptor.get("protocol_id") != TEXT_MEASUREMENT_PROTOCOL_ID:
        raise CheckFailure(
            f"{descriptor_path}: protocol_id must be {TEXT_MEASUREMENT_PROTOCOL_ID!r}"
        )
    if descriptor.get("protocol_version") != TEXT_MEASUREMENT_PROTOCOL_VERSION:
        raise CheckFailure(
            f"{descriptor_path}: protocol_version must be {TEXT_MEASUREMENT_PROTOCOL_VERSION}"
        )

    operations = descriptor.get("operations")
    result_kinds = descriptor.get("result_kinds")
    if not isinstance(operations, list) or not isinstance(result_kinds, list):
        raise CheckFailure(f"{descriptor_path}: operations and result_kinds must be arrays")
    if not all(isinstance(entry, dict) for entry in operations):
        raise CheckFailure(f"{descriptor_path}: operations entries must be objects")
    if not all(isinstance(entry, dict) for entry in result_kinds):
        raise CheckFailure(f"{descriptor_path}: result_kinds entries must be objects")
    if [entry.get("code") for entry in operations] != list(range(19)):
        raise CheckFailure(f"{descriptor_path}: operation codes must be the contiguous range 0..18")
    if [entry.get("code") for entry in result_kinds] != list(range(4)):
        raise CheckFailure(f"{descriptor_path}: result-kind codes must be the contiguous range 0..3")

    known_result_kinds = {entry.get("id") for entry in result_kinds}
    unknown = sorted(
        {
            str(entry.get("result_kind"))
            for entry in operations
            if entry.get("result_kind") not in known_result_kinds
        }
    )
    if unknown:
        raise CheckFailure(
            f"{descriptor_path}: operations reference unknown result kinds {unknown}"
        )
    print(
        "Text-measurement protocol descriptor: "
        f"{TEXT_MEASUREMENT_PROTOCOL_ID} v{TEXT_MEASUREMENT_PROTOCOL_VERSION}"
    )


def check_python_package_metadata() -> None:
    rel_path = "platforms/python/merman/pyproject.toml"
    require_contains(rel_path, 'readme = "README.md"', "PyPI README metadata")
    for label in ["Homepage", "Repository", "Documentation", "Issues", "Changelog"]:
        require_match(rel_path, rf"^{label}\s*=\s*\"([^\"]+)\"", f"project.urls {label}")

    require_file("platforms/python/merman/README.md", "package README")
    require_file("platforms/python/merman/CHANGELOG.md", "package changelog")
    if "does not expose host text-measurement callbacks yet" in read_text("README.md"):
        raise CheckFailure("README.md: stale Python host text-measurement limitation")
    check_python_package_exports()
    check_python_examples()
    print("Python package surface: metadata, files, exports, and examples valid")


def check_flutter_package_metadata() -> None:
    rel_path = "platforms/flutter/pubspec.yaml"
    text = read_text(rel_path)
    for field in ["homepage", "repository", "issue_tracker", "documentation"]:
        if not re.search(rf"^{field}:\s+\S+", text, flags=re.MULTILINE):
            raise CheckFailure(f"{rel_path}: missing {field}")

    topics_match = re.search(r"^topics:\s*\n((?:\s+-\s+\S+\s*\n)+)", text, flags=re.MULTILINE)
    if not topics_match:
        raise CheckFailure(f"{rel_path}: missing topics list")
    topics = {
        line.split("-", 1)[1].strip()
        for line in topics_match.group(1).splitlines()
        if "-" in line
    }
    required_topics = {"mermaid", "ffi", "flutter", "svg", "diagrams"}
    missing_topics = sorted(required_topics - topics)
    if missing_topics:
        raise CheckFailure(f"{rel_path}: missing topics {', '.join(missing_topics)}")

    require_file("platforms/flutter/README.md", "package README")
    require_file("platforms/flutter/CHANGELOG.md", "package changelog")
    print("Flutter package surface: metadata and package files valid")


def main() -> int:
    try:
        check_text_measurement_protocol()
        check_python_package_metadata()
        check_flutter_package_metadata()
    except CheckFailure as exc:
        print(f"::error::{exc}", file=sys.stderr)
        return 1

    print("FFI publish surface verification completed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
