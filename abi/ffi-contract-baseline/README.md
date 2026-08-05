# FFI contract baseline

This directory contains reviewed evidence anchored to commit
`5117c0ae12da2c0346b47061642286174cea3f5f`:

- `dependency-closures.json` freezes ten public-native and private-Node dependency probes.
- `native-artifact-sizes.json` freezes full and semantic C ABI artifacts on the recorded Apple
  toolchain.
- `docs/release/evidence/ffi-contract-native-build-timing.json` records a separate matched
  clean-build/link timing comparison. It is not part of the immutable baseline because timing is
  machine-sensitive; its baseline source revision is still fixed to the commit above and its
  verifier enforces toolchain, target, recipe, order, noise-floor, and review provenance.
- `baseline-lock.json` binds both complete report files, their source inputs, the fixed Git tree,
  and the probe registry.

Do not edit these files by hand. Capture a candidate bundle under ignored `target/`:

```text
python3 scripts/capture_ffi_contract_baseline.py \
  --source-root <clean-worktree-at-5117> \
  --output-root target/ffi-contract-baseline-candidate
```

Review both reports and the proposed lock together, verify them with the current loaders, then
promote all three files in one commit. Native size comparison requires the exact recorded Rust and
Apple toolchain identity; dependency comparison permits installation-path changes but not tool
byte or version drift.

Capture timing only after the candidate implementation is committed:

```text
python3 scripts/measure_ffi_contract_native_build_timing.py capture \
  --candidate-revision HEAD \
  --runs 3 \
  --output docs/release/evidence/ffi-contract-native-build-timing.json
```

If the matched median regression is above both the 10% threshold and the measured noise floor,
review the result and rerun with one trimmed single-line `--review-reason`. A green dependency
closure never suppresses that timing review.
