# Installed Python authoring budget matrix — 2026-09-14

Source: `c04a6a0dd07ab31cd56ac67f665c66aac9dfaab4`.

## Change and evidence

The installed Python theme-authoring smoke now checks encoded-input budgets for materialization,
support discovery, and preset export through both `Merman` and `MermanEngine`. Previously only
materialization had a resource rejection vector. All three operations now reject the shared
one-byte ceiling before semantic input validation; error codes and resource details must match
the shared fixture. Materialization retains its authoring diagnostic envelope, while support and
export retain their existing resource-only error shape.

Each operation also executes with a valid constrained-profile byte budget and must return the
same JSON as its unconstrained baseline. Materialization and support use the exact source UTF-8
length. Export's valid budget covers the returned expanded recipe as well as its preset input;
this is a sufficient-budget check, not an exact minimum claim for export.

A test-process mutation dropped `options_json` for one operation and one consumer at a time,
while still calling the installed native library. The old smoke passed all four support/export
mutations; the new smoke rejected all six operation/consumer mutations, including materialization.
Mutation code and logs stay in the ignored experiment directory; production wrappers were not
changed. The existing smoke remains the only permanent integration surface for this addition.
Ordinary Python CI already runs `build-python-uniffi-wheel.py --run-smoke`, and Release Python
runs the checked-in smoke against the final installed wheel.

## Final installed artifact

The canonical builder selected `python-uniffi-native`, `aarch64-apple-darwin`, and the
`native-distribution` profile with exactly `analysis,ascii,layout-cytoscape,layout-elk,svg`.
It regenerated UniFFI bindings from the production metadata rlib and cdylib, verified committed
support projections, target license material, and native wheel layout. Cargo work ran serially
with two build jobs and reused the existing target.

| Item | Value |
| --- | --- |
| Wheel | `merman-0.8.0a6-py3-none-macosx_11_0_arm64.whl` |
| Wheel bytes | 9,222,643 |
| Wheel SHA-256 | `70c8e9aae741c16c9185c795e0f15e31e98770d098dd67e4818314820aa9b3ca` |
| Library | `merman/libmerman_uniffi.dylib` |
| Library bytes | 22,231,184 |
| Library SHA-256 | `331d4e61df21f4f379c3ea101981ee4fce79a852618e670d9fb99408734204d1` |
| WHEEL tag / purelib | `py3-none-macosx_11_0_arm64` / `false` |
| Host tools | macOS ARM64, Python 3.14.6, Rust 1.95.0 |

A new virtual environment installed only the local wheel using `--no-index --no-deps`.
An explicit import-path assertion confirmed the installed `site-packages` location, and `python -E`
ran the checked-in full smoke from the consumer directory. It passed the native capability/resource
contract, services, SVG/ASCII/analysis, absent specialist capabilities, and the authoring workflow.
Authoring includes light/dark canonical/spec vectors, three-family isolation, a family rule edit,
complete-spec cold start, preset export, the shared catalog, all 22 revision-90 support vectors
through both consumers, and the new three-operation budget matrix.

The final wheel was rebuilt after committing the smoke change. The earlier exploratory wheel at
`4d983f362` has a separate hash in the ledger; it is not this artifact. No runtime, dependency,
build recipe, or budget changed, and these measurements do not establish a speedup, memory bound,
or minimum possible size.

## Checks and boundary

```text
CARGO_BUILD_JOBS=2 python3 scripts/build-python-uniffi-wheel.py --wheel-dir <final-wheels>
python3 -m venv <consumer>/venv
<venv-python> -m pip install --no-index --no-deps <wheel>
<venv-python> -E <repo>/platforms/python/merman/examples/smoke.py
python3 -m unittest scripts.test_python_theme_authoring scripts.test_python_wheel_licenses scripts.test_verify_platform_bindings
python3 -m py_compile platforms/python/merman/examples/theme_authoring.py
git diff --check
```

The final installed smoke and **36/36** focused Python tests passed. The only tracked change
before this record was the verified smoke update. The maintainer's unrelated untracked knowledge
directories were preserved; this is not a clean-checkout build claim. Raw build/install logs,
mutation comparisons, source status, and artifact identities are retained under
`target/bench/experiments/theme-python-revision90/`.

This closes the current macOS Python revision-90 installation and budget-verification slice.
It does not qualify Linux/Windows wheels, add raster/PDF capabilities to Python, populate shared
preset qualification cells, or freeze C7a. The clean CLI/C6a and provider-retirement runs retain
their separately recorded sources and scopes. Web/Typst and the cross-host/public rollout gates
still require their own final evidence.
