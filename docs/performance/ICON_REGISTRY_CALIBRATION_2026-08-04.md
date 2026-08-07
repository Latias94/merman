# Icon Registry Constructor Calibration

Date: 2026-08-04

Status: accepted. The current renderer-owned icon registry limits remain fixed and
non-caller-configurable for the native SDK service contract.

## Decision

The admitted limits support the measured complete Iconify collections, ordinary curated subsets,
and synthetic exact-boundary graphs without adding acquisition I/O or a caller-loosenable service
policy. No limit was increased after measurement.

The direct-icon entry ceiling is the narrowest observed margin: the four complete collections use
29,739 of 32,768 direct-icon entries, or about 1.10x headroom. Any proposal to admit a materially
larger complete-collection corpus must repeat this calibration and the exact/plus-one contract
tests instead of silently raising the ceiling.

This receipt is constructor and render-amplification evidence. It is not an operating-system RSS
claim, a browser-safety claim, or evidence for network/filesystem acquisition. The allocation
counter covers Rust global allocations inside the measured call; process RSS and third-party
native allocator internals remain outside that boundary.

## Provenance

- Revision: `69295f207ec952e6d5989a2ce74ce34ddea7ec70`.
- Package: `merman 0.8.0-alpha.4`.
- Cargo lock SHA-256: `d8a360a8a3ae88d70d576dba0a359208ce9e79ecc99af4fc3111fb7b6336f5b4`.
- Toolchain: Rust `1.95.0` (`59807616e1fa2540724bfbac14d7976d7e4a3860`), Cargo `1.95.0`.
- Host: macOS `26.5.1`, `aarch64`, Apple M4 Pro, Darwin `25.5.0`.
- Build: Cargo `release`, debug assertions disabled, `--no-default-features --features svg`.
- Schedule: 9 complete/curated constructor iterations, 3 synthetic constructor iterations,
  8 repeated-icon render insertions, and 256 curated icons per pack.
- Repository status: `git status --short` was empty immediately before the run. The example's
  optional `tracked_worktree_dirty` provenance probe returned `null`, so this receipt relies on the
  explicit status check and committed revision above.

Reproduction command:

```text
cargo run --release --locked -p merman --example icon_registry_calibration \
  --no-default-features --features svg -- \
  --iterations 9 \
  --render-repetitions 8 \
  --curated-icons-per-pack 256 \
  target/ffi-icon-calibration/logos.json \
  target/ffi-icon-calibration/material-symbols.json \
  target/ffi-icon-calibration/mdi.json \
  target/ffi-icon-calibration/simple-icons.json
```

## Input corpus

| Collection | SHA-256 | Encoded bytes | Direct icons | Aliases | Retained body bytes | Largest body |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `logos` | `09198ad7e85796fb49b8d70425c35051c17e54131889262eaf25dbaf06d6eab8` | 7,447,821 | 2,091 | 9 | 7,241,554 | 155,517 |
| `material-symbols` | `fe53ee335cb1627f3017980804d62888d46139625076553ab719b6b7a06fef4f` | 8,396,150 | 16,284 | 8,087 | 7,006,863 | 2,922 |
| `mdi` | `f38bcea6e945f01004384c1dbb66ddb2a538d81b7f6af3cf6b95fc603603833a` | 3,096,165 | 7,638 | 6,363 | 2,362,411 | 4,350 |
| `simple-icons` | `bb773761b2635432708369eae392039fe33412f884ed78782d5472a782565d87` | 4,765,207 | 3,726 | 12 | 4,613,403 | 54,071 |
| **Aggregate** | — | **23,705,343** | **29,739** | **14,471** | **21,224,231** | **155,517** |

The aggregate contains 44,210 resolved entries and 92,155 inspected JSON members. Its observed
maximum JSON depth is 3, maximum JSON key length is 59 bytes, maximum XML element count is 753,
maximum XML depth is 7, maximum alias depth is 1, and maximum alias fan-out is 15.

The curated corpus contains four packs, 1,024 direct icons, 1,367,370 encoded bytes, and 1,324,181
retained body bytes. It deliberately removes aliases so the common small-registry path is measured
separately from complete-collection alias resolution.

## Frozen constructor limits

All limits below are renderer-owned hard capabilities and have
`caller_configurable = false`.

| Limit ID | Value | Unit |
| --- | ---: | --- |
| `max_icon_registry_packs` | 16 | packs |
| `max_icon_pack_bytes` | 16,777,216 | bytes |
| `max_icon_registry_input_bytes` | 33,554,432 | bytes |
| `max_icon_registry_json_depth` | 32 | levels |
| `max_icon_registry_json_members` | 1,000,000 | members/items |
| `max_icon_registry_json_key_bytes` | 1,024 | bytes |
| `max_icon_registry_prefix_bytes` | 64 | bytes |
| `max_icon_registry_name_bytes` | 128 | bytes |
| `max_icon_body_bytes` | 262,144 | bytes |
| `max_icon_registry_retained_body_bytes` | 33,554,432 | bytes |
| `max_icon_registry_icon_entries` | 32,768 | entries |
| `max_icon_registry_alias_entries` | 32,768 | entries |
| `max_icon_registry_entries` | 65,536 | entries |
| `max_icon_registry_alias_edges` | 32,768 | edges |
| `max_icon_registry_alias_depth` | 64 | levels |
| `max_icon_registry_alias_fanout` | 1,024 | aliases |
| `max_icon_registry_build_work_units` | 4,000,000 | work units |
| `max_icon_xml_elements_per_body` | 4,096 | elements |
| `max_icon_xml_depth_per_body` | 32 | levels |
| `max_icon_id_rewrite_edits_per_body` | 16,384 | edits |
| `max_icon_registry_retained_xml_plan_bytes` | 16,777,216 | bytes |
| `max_icon_coordinate_magnitude` | 1,000,000 | coordinate units |

Observed complete-corpus headroom is approximately 2.00x for the largest encoded pack, 1.42x for
aggregate encoded bytes, 1.58x for retained body bytes, 1.10x for direct icons, 2.26x for aliases,
1.48x for total entries, 10.85x for inspected JSON members, 1.69x for the largest body, and 5.44x
for XML elements per body.

## Constructor measurements

| Workload | Published entries | Cold latency | Warm median | Warm range | Peak live growth | Retained growth | Total allocated |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Four complete collections | 44,210 | 148.67 ms | 138.12 ms | 136.93–139.24 ms | 35.24 MiB | 29.83 MiB | 207.60 MiB |
| Four curated subsets | 1,024 | 6.50 ms | 6.42 ms | 6.27–6.77 ms | 1.62 MiB | 1.56 MiB | 9.22 MiB |
| Exact max body/alias graph | 1,090 | 1.05 ms | 1.05 ms | 1.04–1.52 ms | 1.25 MiB | 0.42 MiB | 2.68 MiB |
| Exact max entry/edge graph | 65,536 | 74.90 ms | 71.52 ms | 69.40–71.71 ms | 22.70 MiB | 14.29 MiB | 148.86 MiB |
| Exact max XML rewrite plan | 1 | 4.30 ms | 4.49 ms | 4.24–4.49 ms | 2.56 MiB | 0.41 MiB | 8.60 MiB |

Every measured constructor released its retained allocation growth after the registry was dropped;
the allocation counter reported neither overflow nor underflow.

The synthetic fixtures exercise these combined exact boundaries:

- `synthetic/max-body-alias-graph`: 262,144-byte body, XML depth 32, alias depth 64, and
  alias fan-out 1,024.
- `synthetic/max-entry-edge-graph`: 32,768 direct icons, 32,768 aliases, 65,536 total entries, and
  32,768 alias edges.
- `synthetic/max-xml-rewrite-plan`: 4,096 XML elements and the admitted deterministic rewrite
  planning path in one body.

Exact and plus-one unit/contract tests remain the admission authority. This report demonstrates
that representative complete collections and synthetic exact-boundary inputs fit the frozen
policy; it does not replace those rejection tests.

## Render amplification

| Icon | Source body | Repetitions | Total latency | Approx. latency/insertion | SVG bytes | Output/body ratio | Peak live growth |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `logos:lerna` | 155,517 B | 8 | 20.33 ms | 2.54 ms | 1,259,162 | 1.012x | 2.35 MiB |
| `calibration:max-body` | 262,144 B | 8 | 4.34 ms | 0.54 ms | 2,112,162 | 1.007x | 3.61 MiB |

The output ratios show near-linear repeated-body growth. The operation path therefore must retain
the existing pre-charge of projected SVG bytes and deterministic scoping/sanitizer work before
each clone and assembly; constructor admission alone is not an aggregate render-output limit.

## Consequences

- Keep the current 16-pack, 16 MiB per-pack, and 32 MiB aggregate encoded-byte limits.
- Keep 32,768 direct icons, 32,768 aliases, and 65,536 total resolved entries as hard limits.
- Do not expose caller overrides for any constructor limit through Rust, C, UniFFI, JNI, Flutter,
  Apple, or Python.
- Continue to require hosts to curate collections that exceed these ceilings; Merman performs no
  package lookup, filesystem loading, network loading, or automatic slicing for SDK registries.
- Recalibrate before increasing the direct-icon limit or admitting a substantially different
  corpus. The current complete corpus leaves only about 10% direct-icon capacity.
- Preserve the effective-configuration sanitizer and post-sanitizer XML validation. Successful
  bounded construction does not make parity/readable SVG safe for direct browser DOM insertion.
