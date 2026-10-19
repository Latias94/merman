# Native theme regression: macOS causal attribution, 2026-10-16

## Finding

The theme merge adds substantial work to ordinary source-to-SVG rendering even without a selected
Merman theme. The strongest shared owner is mandatory standalone finalization: a full XML and
reference-resource walk after SVG generation. Family emission also acquires a whole-output
reserved-token search and bounded serialization work. Current default theme calculation is not the
primary remaining owner for these fixtures.

This is a diagnosis checkpoint. No performance implementation, admitted speedup, or calibrated
non-regression result is included. Existing XML, resource, cancellation, and exact-artifact contracts
remain authoritative.

## Direct theme-merge comparison

Compare clean `768370e08` (the first parent, already on Mermaid 12.2) with clean theme merge
`b9844db7b`. This excludes the subsequent HTML5 parser replacement and later repair commits.
The checked-in pipeline benchmark and both fixture byte strings match. Both SVG identities match
before and after every timed invocation. The complete two-row diagnostic exits zero.

| Fixture | Before merge, us | Theme merge, us | Increase |
| --- | ---: | ---: | ---: |
| class_medium | 634.785 | 1101.450 | 73.52% |
| journey_medium | 26.467 | 104.770 | 295.86% |

Each row has two independent AB/BA pairs. Values are arithmetic means of Criterion console point
estimates, not per-operation percentiles. These observations isolate a regression in the merge
range; they are diagnostic rather than A/A-calibrated statistical confirmation.

## Remaining gap at current head

Historical control `3a4b8c393` and current `3d8a738d4` plus the build repair below produce identical
SVGs for these four rows:

| Fixture | Historical control, us | Current, us | Increase |
| --- | ---: | ---: | ---: |
| class_medium | 658.180 | 1066.950 | 62.11% |
| journey_medium | 27.503 | 98.479 | 258.06% |
| kanban_medium | 44.474 | 130.730 | 193.95% |
| class_tiny | 127.200 | 220.645 | 73.46% |

These are retained matched rows from a five-row diagnostic whose overall exit is **2**: Info has a
different SVG identity and is not timed. Its displayed pinned Mermaid version changes across this
release range. Do not describe the entire run as a successful suite. The first attempt has no
timings at all because current main fails compilation before sampling.

The historical-to-current range also contains Mermaid and HTML parser updates. It describes the
remaining product gap; the clean direct-merge comparison above provides the narrower attribution.

### Stage evidence

| Stage | Fixture | Historical control, us | Current, us |
| --- | --- | ---: | ---: |
| parse | class_medium | 146.795 | 148.860 |
| parse | journey_medium | 5.419 | 5.996 |
| family SVG emission | class_medium | 232.990 | 313.980 |
| family SVG emission | journey_medium | 16.337 | 25.237 |
| family SVG emission | kanban_medium | 26.135 | 50.429 |

Parse's complete receipt exits 2 because Kanban's model projection differs; that row is not timed.
The three emission rows form a successful diagnostic receipt. Layout projections all differ, so
the comparison rejects them before timing: there is no measured old/new layout conclusion.
Stage setup, ownership, and destruction boundaries differ. Do not add these values or subtract
them from end-to-end measurements.

## Causal paths

### Ordinary standalone finalization

Historical `render_svg_target` returns the generated string directly when `pipeline` is `None`.
The theme branch introduces exact standalone sealing in `e1f93a837`. Current code follows:

1. `crates/merman/src/render.rs::render_svg_target` unconditionally finalizes standalone output.
2. `crates/merman-render/src/family.rs::finalize_standalone_with_portability` also finalizes the
   `None` path through a parity pipeline.
3. `svg/pipeline/standalone.rs::finalize_exact_buffer` hashes SVG resources, then admits the complete
   XML/reference resource budget even for best-effort output without a compatibility request.
4. `svg/pipeline/final_validation.rs::validate_svg_xml_facts_with_controls` walks the XML, validates
   attributes and characters, gathers reference facts, and then analyzes expanded resource work.
5. `crates/merman/src/render/document.rs::standalone_svg_evidence` hashes the exact public SVG and
   seals target-admission evidence. Resource and artifact hashes bind different contracts.

An independent current-head probe uses the actual optimized production rlibs. It prepares 32 family
outputs outside each timer, then consumes them through `finalize_standalone(None)`. Sixteen batches
follow public-facade/output preflight and one explicit standalone warmup; every output remains
byte-identical to both the generated and public-facade SVG.

| Fixture | SVG bytes | Mean per finalize batch operation, us | Batch range, us |
| --- | ---: | ---: | ---: |
| class_medium | 77419 | 282.763 | 267.122–366.698 |
| journey_medium | 12224 | 48.376 | 48.016–48.975 |
| kanban_medium | 21557 | 39.260 | 38.249–41.608 |
| class_tiny | 16936 | 47.765 | 47.461–48.716 |
| info_medium | 2518 | 3.175 | 3.165–3.186 |

The timer includes iterator/collect/container management and consumed-input lifecycle work; it is
not a pure callee CPU measurement. Returned-artifact verification and destruction happen afterward.
The Class range includes warmer later batches and a slower initial batch. The probe does not
independently assert terminal status or resource fingerprint. Its values cannot establish an
end-to-end percentage, an exactly removable delta, or a historical finalization ratio.

Five-second, 1 ms macOS `sample` captures of the same normal Criterion executables independently
locate the owner. Current Class has a maximum inclusive node of 1011 samples for standalone
finalization and 915 for its XML/resource-budget child; Journey has 1923 and 1846. Attribute
iteration, duplicate checking, QName validation, XML character checking, and normalization appear
under this path. These are overlapping inclusive observations, not exclusive times or a breakdown
of the regression. The summary takes each function's maximum node rather than summing stack offsets.

Each ordinary nonempty XML attribute of at most 256 bytes executes at least six source-level
checkpoint callbacks, in addition to event and element checks. Class medium has 1488 non-namespace
attributes; Journey has 352. Default operation checkpoints normally take an atomic fast path, not a
clock read.
High callback frequency is a plausible constant-factor owner; the exact instruction-level cost
has not been isolated.

### Whole-SVG reserved-token search during family emission

Current `family.rs::render_svg` unconditionally invokes
`svg/pipeline/prepared_text.rs::reject_reserved_prepared_text_tokens`. Its miss path uses the generic
byte-by-byte KMP implementation in `svg/pipeline/builtin/util.rs`. The historical control has no such
stage; `1907caa2e` introduces the prepared-text partition, and `509275592` retains observation after
removing the unreachable ledger.

The actual Class, Journey and Kanban outputs contain no `merman-prepared-`, but still scan their full
77419/12224/21557 bytes. Source-level checkpoint counts are respectively 1212/193/339:
`ceil(svg_bytes / 64) + 2`. This is linear work, not a quadratic defect.

The current end-to-end profiles place this search separately from standalone finalization, under
family preparation/emission. Its maximum inclusive call-tree nodes have 405 samples for Journey and
251 for Class. These counts locate another substantial owner but do not give exact microseconds or
an admitted optimization benefit. The earlier
[Windows search candidate](native_reserved_token_search_windows_2026-10-16.md) remains unadmitted;
its noisy controls do not prove that the existing search is cheap.

### Bounded serialization and family evidence

The old emitters write to `String`. Current `svg/parity/output.rs` uses a bounded sink whose writes
observe cancellation even after capacity is admitted. CSS `XmlTextOutput` additionally observes
operation control and scans logical text before forwarding to that sink. Kanban's stylesheet is
14459 of its 21557 output bytes. This makes fragment count and stylesheet handling useful secondary
hypotheses, without establishing an independently measured contribution yet.

Class also creates operation-local style/relationship receipts. Several earlier costs are already
repaired, including configuration COW transactions, staged provenance scans, shared-default paint
copies, repeated edge ordering, and complete compatibility snapshots used only for predicates.
Do not report those as still-existing root causes. Default Class already recalculated its scoped
theme in the historical control; current nearly unchanged parse timing supports deprioritizing
another broad theme cache.

## Build and measurement contract

- Apple M4 Pro, 14 cores, 48 GiB; macOS 27.0 ARM64; Rust 1.95.0.
- Both sides use locked `merman/pipeline` bench builds, default features plus `svg`: all diagrams,
  ELK, Cytoscape and math. Native/system fonts and raster codecs are not selected.
- Cargo artifact JSON confirms `opt_level=3`, no debug info or debug assertions. Standard bench
  settings have no cross-crate LTO; this is not the distinct `native-sdk` thin-LTO release profile.
- Builds are serial and finish before sampling. Executables run directly and retain hashes.
  Criterion pre/postflight XML parsing and identity hashing are outside the timed operation;
  production finalization validation/hashing are inside it.
- Each comparison uses 30 samples, a two-second warmup, three-second measurement, and two balanced
  AB/BA pairs. No calibrated confirmation or production optimization admission is claimed.
- Public end-to-end rendering reuses Engine/Renderer, uses strict parsing, deterministic text
  measurement, default source appearance, no Merman preset, and trusted-native resource policy.

Current main initially cannot build because Architecture and Mindmap still call the removed
`OperationWorkMeter::max_svg_bytes`. The investigation branch changes those two sites to the exact
existing `policy().value(ResourceLimitId::MaxSvgBytes)` query. The current-head receipts explicitly
mark this dirty build repair. Neither repaired family is timed, but a code-generation effect has not
been independently excluded. The direct theme-merge comparison uses clean, unmodified commits.

The build repair passes 63 Architecture/Mindmap SVG integration tests with one existing skip:

```console
CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo nextest run --locked --cargo-profile release \
  -p merman-render --features all-diagrams,layout-cytoscape,layout-elk,math \
  --test architecture_svg_test --test mindmap_svg_test
```

`cargo fmt --all -- --check` and `git diff --check` pass. Arithmetic, probe boundaries, optimized
artifact selection, excluded rows, and source/profile attribution receive an independent read-only
review. No full-workspace semantic or DOM parity admission is claimed for this diagnostic checkpoint.

## Follow-up priority and retained evidence

Preserve resource, XML, cancellation, reserved-token, and artifact-admission semantics. The next
bounded experiments should isolate the reserved-prefix observer and XML attribute processing.
A deeper writer/facts boundary can avoid interpreting generated XML again only if it carries a
complete generation-bound proof and invalidates it after every opaque mutation. Removing validation
or weakening cancellation is not an equivalent optimization. Separate the two hash contracts before
considering any hashing change.

Ignored raw evidence is under `target/bench/experiments/theme-root-cause-macos-20261016/`:
`experiment.yaml`, `summary.json`, the comparison JSON/Markdown/logs, `finalize_probe.rs`, its exact
build command and raw batches, production Cargo artifact JSON, SVGs and shape statistics, normal
binary CPU captures, profile metadata, and frozen historical/current executables. The direct-merge
runner originally writes its reports below the head worktree's corresponding `target/bench` path;
identical copies are retained in the main evidence directory.

The clean direct comparison command is:

```console
python3 tools/bench/compare_self.py \
  --base-dir .worktrees/bench-theme-merge-parent-20261016 \
  --head-dir .worktrees/theme-ownership-performance \
  --base-label before-theme-merge-768370e08 --head-label theme-merge-b9844db7b \
  --base-target-dir target/bench/experiments/theme-root-cause-macos-20261016/build-base \
  --head-target-dir target --preset long --pairs 2 \
  --filter 'end_to_end/(class_medium|journey_medium)' \
  --out target/bench/experiments/theme-root-cause-macos-20261016/theme-merge.md \
  --json-out target/bench/experiments/theme-root-cause-macos-20261016/theme-merge.json
```

Replaying creates new evidence; preserve existing raw output and freeze any executable before
reusing its target directory for a different revision.
