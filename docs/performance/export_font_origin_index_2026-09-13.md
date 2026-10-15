# Export font origin index storage

Date: 2026-09-13

This structural storage change is based on
`3e21e5802ce64b4d2735f092b8cc4febc331d72d`. It removes the per-export copy of
system font origin metadata. It does not measure latency, allocator bytes, or RSS.
SystemOnly already reused the shared database before this change; the remaining
copy was a separate `HashMap` entry for every system face.

`ExportFaceOrigins` now retains the immutable system database through an `Arc` and
records only faces loaded for the current operation. Lookup checks those loaded
faces first, preserving the previous insertion precedence, then checks membership
in the retained system database. In fontdb 0.23.0, `Database::face` uses the
underlying SlotMap's generation-aware ID lookup. Unknown and removed face IDs do
not become System merely because they are absent from the loaded-face map.

Let S be the system face count, E the embedded face count loaded for this export,
G the visible glyph count, and U the distinct selected system face count. Previously,
origin-index construction visited S system faces and retained S + E entries. It now
retains E entries plus a shared database handle, with no system-face traversal to
build that index. Hash-map lookup remains expected O(1); final evidence still visits
G glyphs, and the existing selected-system-face key cache still contains at most U
entries. System discovery, default-family selection, aliases, catalog evidence,
embedded database merging, and prepared-text verification are unchanged. In
particular, total export preparation is not claimed to become O(1).

The test-only scale probe was first run against unchanged baseline production code,
then against the candidate. Both use cloned metadata from the same Excalifont test
fixture to build controlled system databases. This does not bundle a new font or
change the library's font policy.

| System faces | Baseline owned origin entries | Candidate owned origin entries |
| --- | --- | --- |
| 0 | 0 | 0 |
| 1 | 1 | 0 |
| 64 | 64 | 0 |
| 1024 | 1024 | 0 |

The mixed-policy controls assert exactly E loaded entries. Across four source
policies and four label/family controls (ASCII, escaped ampersand, missing family
with CJK, and empty text), the complete `ExportFontPlan` and rendered RGBA pixels
match an independent reconstruction of the former eager origin map. A separate
test mutates a caller's database through copy-on-write, verifies operation snapshot
membership, and renders an untracked glyph to prove it remains unclassified.

## Validation

Host: Apple M4 Pro, macOS ARM64, Rust 1.95.0. Cargo ran serially with
`CARGO_BUILD_JOBS=2` and the existing target cache.

- Release export library with PNG, JPEG, and PDF: 99/99 tests passed, including
  all 22 font environment tests.
- Private-cfg C6 runtime, native export smoke, and preset qualification: 5/5 passed.
- Clippy completed with existing warnings; its font-environment warning concerns
  the unchanged eight-argument traversal signature.
- Formatting and diff whitespace checks passed.

```text
cargo nextest run --release --locked -p merman-export --features png,jpeg,pdf \
  --lib --no-fail-fast
cargo clippy --locked -p merman-export --features png,jpeg,pdf --lib
python3 scripts/run_theme_acceptance.py nextest run --release --locked \
  -p merman-theme-acceptance --test c6_runtime --test native_export_smoke \
  --test preset_qualification --no-fail-fast
cargo fmt --all -- --check
```

The ignored experiment ledger and raw logs are under
`target/bench/experiments/export-font-origin-index-20260913/`. Baseline and candidate
scale logs record exact entry counts; their incidental test durations are not
benchmark evidence. This change does not alter support revision 83, retire legacy
routes, or close C7a/C7b and package-profile delivery gates.

## Clean-checkout confirmation

Source commit: `0fdbaafe11c5fe4e32a30a0a0ceb7c6bc0cace63`.

The detached checkout at `/tmp/merman-font-origin-0fdbaafe1` was clean before and
after verification. It reused the original target cache with `CARGO_BUILD_JOBS=2`,
compiling source and fixtures from the detached checkout. The Release export
library passed 99/99 again; private-cfg C6 runtime, native export smoke, and preset
qualification passed 5/5 again using the commands above. Logs are
`/tmp/font-origin-clean-export.log` and `/tmp/font-origin-clean-acceptance.log`.
