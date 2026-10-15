---
type: Verification Evidence
title: Installed Python wheel authoring and support revision 82
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
tags: theme,python,authoring,verification
---

# Artifact and consumer

The `python-uniffi-native` artifact was rebuilt from source revision `983708e33` using the
existing wheel builder. It selected `aarch64-apple-darwin`, the `native-distribution` Cargo profile,
and exactly `analysis,ascii,layout-cytoscape,layout-elk,svg`. The builder generated bindings from
the production metadata-bearing rlib and cdylib, checked the committed Python support projections,
and verified the target-specific license report and native wheel layout.

| Item | Value |
| --- | --- |
| Wheel | `merman-0.8.0a6-py3-none-macosx_11_0_arm64.whl` |
| Wheel bytes | 9169052 |
| Wheel SHA-256 | `e0a474d109ad72182702fd2916748ee4789827e3a7d67b4030447947755b665d` |
| Bundled library | `merman/libmerman_uniffi.dylib` |
| Library bytes | 22082016 |
| Library SHA-256 | `614d663f1b0237d79f2d31ebc4b0e7024c81564e996598c8e53346e3c912602b` |
| WHEEL tag / purelib | `py3-none-macosx_11_0_arm64` / `false` |
| Consumer host | macOS ARM64, Python 3.14.6 |

A new virtual environment installed this wheel with `--no-deps`. An explicit import-path check
confirmed `merman.__file__` was inside that environment's `site-packages`. The checked-in
`examples/smoke.py` ran using `python -E` from the temporary consumer directory, so Python
configuration environment variables could not redirect imports.

The full smoke passed the native capability/resource contract, icon and text-measurer services,
SVG/ASCII/analysis operations, missing specialist capability errors, and theme authoring. The
latter executed light/dark canonical/spec vectors, three-family reuse and isolation, rule editing,
complete-spec cold start, preset export, catalog golden comparisons, and all ten support vectors
through both one-shot and reusable APIs. Support revision 82 includes Class's retired background
query. All six authoring/resource error observations preserved their shared envelopes.

# Terminal oracle repair

Before the repair, `state_fill()` accepted an SVG whose Active-state rectangle had zero width
and height and returned `#123abc`. That could falsely satisfy the authoring fill assertion.

The bounded oracle now requires the SVG namespace/root, a real state group and rectangle, and
finite positive rectangle width and height. It retains the existing terminal fill assertion.
Three ordinary script tests cover valid output, missing/invalid/nonfinite/nonpositive dimensions,
and non-SVG/non-rectangle lookalikes. They isolate the pure oracle only; they are not transport
witnesses. The independently installed wheel then passed the full smoke again with the stricter
oracle. This does not implement CSS visibility evaluation or claim raster qualification.

# Commands and results

```text
CARGO_BUILD_JOBS=2 python3 scripts/build-python-uniffi-wheel.py --wheel-dir target/python-wheel-revision82-983708e33
python3 -m venv <consumer>/venv
<consumer>/venv/bin/python -m pip install --no-deps <wheel>
<consumer>/venv/bin/python -E <repo>/platforms/python/merman/examples/smoke.py
python3 -m unittest scripts.test_python_theme_authoring scripts.test_python_wheel_licenses scripts.test_verify_platform_bindings
python3 -m py_compile platforms/python/merman/examples/theme_authoring.py scripts/test_python_theme_authoring.py
git diff --check
```

The wheel build and both installed smoke runs passed. The combined focused Python suite passed
36/36. The new oracle test is included by the ordinary CI script discovery rule
`python3 -m unittest discover -s scripts -p 'test_*.py'`.

Logs: `/tmp/theme-python-revision82-{build,installed,installed-final,area-before,oracle,contracts}.log`.
The consumer location is recorded in `/tmp/theme-python-revision82-path.txt`; the wheel remains in
the named target directory. No artifact was published.

This is a native Python SVG authoring/discovery witness. It does not add PNG/JPEG/PDF to the
Python profile, qualify public preset cells, certify Linux/Windows wheels, or close C5/C7a. The
checked-in C5 status has separately been corrected to stop presenting historical v27/seven-family
counts as current implementation state; its requirement-by-requirement closure audit remains open.
