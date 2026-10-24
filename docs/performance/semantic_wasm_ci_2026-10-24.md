# Semantic boundary WASM CI investigation

Status: the source candidate was rejected; existing WASM budgets remain a CI blocker.

## Scope and frozen acceptance

This checkpoint investigates PR #241's existing WASM size gates. It is not an R14 timing
confirmation, does not resume the paused refactoring objective, and does not change
`docs/release/WASM_SIZE_BUDGETS.json` or any artifact capability recipe.

The failed source is `b6161e13aaaee3b12532682a0d6a8d2a819156f7`, CI run
`39984962125`. The same-run Web owner measured the following stripped artifacts:

| Profile | Failed bytes | Existing ceiling | Excess |
| --- | ---: | ---: | ---: |
| `web-analysis` | 4,033,974 | 3,938,000 | 95,974 |
| `web-ascii` | 5,689,883 | 5,517,000 | 172,883 |
| `web-editor` | 4,146,707 | 4,054,000 | 92,707 |
| `typst-wasm` | 12,293,379 | 12,293,000 | 379 |

The ASCII gzip and Brotli gates also failed. Acceptance requires every existing
raw, stripped, gzip, and Brotli ceiling to pass, not just a smaller raw artifact.

Main `83e3ea74bf4da30089a8012ec2d3418d40638e52`, run `39747740429`, passed
the Web owner with stripped analysis/ASCII/editor sizes of 3,936,967 / 5,488,152 /
4,049,958 bytes. Those three feature recipes are unchanged. Full/render are not
equivalent controls because this branch makes math an explicit capability.
The old budget notes name Binaryen 131 whereas the current Typst recipe uses 133;
the Typst excess alone is not a controlled attribution to this branch.

## Rejected candidate and correctness

The candidate shared the heavy AST-to-source and source-to-model constructors
between Flowchart's canonical and presentation metadata types. It kept metadata
selection at the existing entry points and passed borrowed diagram type/config
references into non-generic construction functions. The edge projection used
the same borrowed inputs. This changed neither configuration precedence nor the decision
to capture render context; sanitization, cancellation, and resource checkpoints
remained in their existing order. The change was not retained because it did not
meet the frozen size bounds.

Verified locally before size admission:

- All-features Flowchart/Swimlane library tests: **253 passed**.
- Lean Flowchart/Swimlane plus sanitizer library tests: **113 passed**.
- The Unix Gantt timezone migration is a separate test-only repair; its DST test
  passed under WSL with ASCII and native time features enabled.

The local size toolchain is Rust 1.95.0, wasm-pack 0.15.0, Binaryen 133, and
wasm-tools 1.261.0 on Windows 11. Final Linux CI remains the platform owner for
SVG parity and the published package-size gate. No latency measurements or
speedup claims are made here.

## Final assembled measurement

The analysis build and the TypeScript contract check succeeded. The latter checked
42 WASM exports, 55 runtime bindings, and five package entries. After the normal
package assembly step, the assembled analysis WASM was checked byte-for-byte by
SHA-256 against the fresh build before invoking the budget owner:

```text
SHA-256: d6a05e8369f1a3d84a404cce972382ad6e7ed6b47cbf6f9b0c910f83f8605cc5
raw:      4,027,155 / 3,938,000 (89,155 over)
stripped: 4,026,878 / 3,938,000 (88,878 over)
gzip:     1,535,863 / 1,555,000
Brotli:   1,173,666 / 1,192,000
```

The matrix exited with failure for the two byte ceilings. An earlier local matrix
read a stale assembled package before the assembly refresh; that result is
excluded, not treated as candidate evidence. No measurements are claimed for the
other candidate profiles because analysis already failed the admission gate.
The reviewed source patch was archived in the ignored experiment directory and
only that owned patch was reversed. No capability was removed and no budget was
increased to disguise this failure. Further size repair or an explicitly reviewed
budget decision remains necessary before claiming the CI issue is resolved.

Raw logs and the exploratory ledger remain outside committed evidence, under
`F:/codex-builds/merman-pr241-ci-b6161e13a-20261024/` and
`target/bench/experiments/pr241-wasm-metadata/experiment.yaml`.
