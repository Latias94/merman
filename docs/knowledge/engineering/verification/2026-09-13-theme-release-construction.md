# Theme patch construction in optimized tests — 2026-09-13

## Finding

The Release GitGraph winning-property test aborts on clean `f9228bb1d`, including
an independent target directory. A crash in `recipe_fingerprint` is a downstream
symptom: removing compilation and rendering still reproduces corrupted font
strings while reading three freshly constructed patches.

A standalone safe Rust program with no Merman dependencies reproduces the same
empty-string assertion and abort. On this ARM64 macOS host:

| Compiler / options | Result |
| --- | --- |
| Rust 1.95.0, `-O` | Assertion failure, abort |
| Rust 1.95.0, `-O -Zmir-enable-passes=-GVN` | Passed |
| Rust 1.97.1, `-O` | Passed |

The diagnostic `-Z` option required `RUSTC_BOOTSTRAP=1`; neither setting is added
to the build configuration. This comparison implicates optimization of repeated
moves into a mutating closure, not theme fingerprint encoding. It does not identify
a specific upstream fix or prove the absence of other compiler defects.

The optimized LLVM IR reuses the default-patch argument at repeated closure call
sites without restoring its initial fields, while the callee modifies its font
field. That is consistent with the observed stale ownership and corrupted strings.

## Scoped workaround

Use a named test helper for the existing font-patch transformation. Preserve every
case and every applied, not-applicable, residual, SVG and strict-admission assertion.
The original focused Release matrix passes with this change. Merely changing the
case array to a vector did not fix it.

A separate font-only compilation test checks that the font contributes to the
recipe fingerprint. That test passes even before the workaround and is isolation
coverage, not a regression test for the optimizer failure. The original matrix
is the failure-bearing test.

There is no production theme-model change or toolchain upgrade in this workaround.
A later toolchain upgrade should rerun the original closure form before removing
it. The full renderer Release suite passed: 3,839 tests, with 4 skipped. This run
included the pending family-dispatch refactor. Its private acceptance, Web and
structure gates remain separate and are not closed by that result.

## Local evidence

Ignored experiment directory:
`target/bench/experiments/family-evidence-dispatch-f9228bb1d/`.
It contains `font-stack-repro.rs`, `compiler-isolation-results.json`, the original
matrix source, reduced construction tests, and the experiment ledger.

Focused passing command:

```text
CARGO_BUILD_JOBS=2 cargo nextest run --locked --release -p merman-render --test gitgraph_svg_test -E 'test(gitgraph_commit_background_accounts_for_each_winning_property)' --cargo-quiet --test-threads 1
```

Full-suite log: `/tmp/family-evidence-render-tests-2.log`.
