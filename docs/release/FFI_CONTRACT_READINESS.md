# FFI Contract Readiness

This is the final pre-release readiness view for the FFI contract alignment work. It is a
candidate-branch engineering record, not a publication or release approval. The public-native
and private-Node lanes are reported separately because the Node candidate remains private even
when its transport checks are green.

## Readiness lanes

| Lane | Status | Contract boundary | Evidence |
| --- | --- | --- | --- |
| `public-native` | green | C ABI 3, Android JNI transport API 1, UniFFI API 3, and the one full native SDK SKU | [dependency closures](../../abi/ffi-contract-baseline/dependency-closures.json), [native artifact sizes](../../abi/ffi-contract-baseline/native-artifact-sizes.json), and the platform verification script |
| `private-node` | green, private | deterministic, static-SVG, text-only candidate; not an admitted or publishable native SDK surface | [dependency closures](../../abi/ffi-contract-baseline/dependency-closures.json) and the Node package contract tests |

The public-native lane does not claim that Android uses C ABI 3: Android consumes its direct JNI
transport API 1. C ABI 3 keeps its published six-slot prefix; appended service records and slots
remain invisible to old consumers. UniFFI remains API 3. These wire/version decisions are
intentional breaks at the source SDK layer, not compatibility aliases.

## Immutable attribution baseline

Every comparison is anchored to the exact baseline commit
`5117c0ae12da2c0346b47061642286174cea3f5f`. The baseline reports are immutable and must not be
regenerated from the candidate branch. Dependency closures cover eight public-native probes and
two private-Node probes; the final verifier reports each lane independently.

## Native artifact weight

The same-recipe stripped artifacts remain within the R26 budgets:

| Profile / artifact | Delta | Budget | Result |
| --- | ---: | ---: | --- |
| `ffi-full-native` cdylib | +248,720 bytes | 524,288 bytes | green |
| `ffi-full-native` staticlib | +1,866,568 bytes | 3,384,636 bytes | green |
| `ffi-semantic` cdylib | -16,528 bytes | 65,536 bytes | green |
| `ffi-semantic` staticlib | +19,112 bytes | 442,062 bytes | green |

These are stripped Apple native artifacts from the descriptor-owned recipes. They are weight
signals, not universal performance claims. The SVG icon registry stays inside the existing `svg`
closure: no `icons` feature, acquisition I/O, async runtime, or second native SKU was added.

## Clean-build timing

The checked-in [timing evidence](evidence/ffi-contract-native-build-timing.json) is the source of
truth for clean Cargo build-and-link wall time. It records Rust/Cargo/Xcode and machine
provenance, uses three or more odd-numbered alternating baseline/candidate pairs, and allocates a
fresh target directory for every sample. The gate requires explicit review only when the median
candidate regression is above both 10% and the measured relative-MAD noise floor. It does not
hide a timing regression behind an unchanged dependency closure.

The current matched capture compares baseline `5117c0ae12da2c0346b47061642286174cea3f5f`
against candidate `b9e16e8d32d4186ff80e5e1ab5f89cdf6c4c3a74`: the medians are 115.012 seconds
and 103.086 seconds respectively, a 10.37% decrease against a 2.28% noise floor. The configured
10% regression review threshold is not crossed, so no timing review exception is required.

Timing is measured on the recorded local Apple host and is not a cross-machine, universal, or
runtime-rendering performance claim. `measure_ffi_contract_native_build_timing.py verify` is part
of the platform-binding verification path; a missing, malformed, recipe-drifted, or unreviewed
report fails closed. The report remains evidence for its exact recorded candidate tree: later
commits are not guessed to be timing inputs from a repository-wide path allowlist and do not
silently become part of the measurement.

## Verification entry points

Run the following from the repository root after regenerating owned projections:

```text
python3 scripts/ffi_contract_docs.py
python3 scripts/verify_artifact_dependency_closures.py \
  --baseline abi/ffi-contract-baseline/dependency-closures.json
python3 scripts/verify_native_artifact_sizes.py \
  --baseline abi/ffi-contract-baseline/native-artifact-sizes.json
python3 scripts/measure_ffi_contract_native_build_timing.py verify
python3 scripts/verify-platform-bindings.py
```

The reports and commands above describe readiness; they do not publish packages, move a tag, or
change the Mermaid baseline.
