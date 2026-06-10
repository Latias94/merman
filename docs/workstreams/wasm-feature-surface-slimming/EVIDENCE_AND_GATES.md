# WASM Feature Surface Slimming -- Evidence And Gates

Status: Open
Last updated: 2026-06-09

## Current Evidence

### Typst Plugin Protocol

Source: `repo-ref/typst/crates/typst-library/src/foundations/plugin.rs`

Observed contract:

- Typst loads raw WebAssembly bytes through `wasmi`.
- The module must export `memory`.
- Callable plugin exports accept only `i32` parameters and return one `i32`.
- The host links only:
  - `typst_env::wasm_minimal_protocol_write_args_to_buffer`
  - `typst_env::wasm_minimal_protocol_send_result_to_host`
- The documentation explicitly says WASI does not directly work and plugins cannot print, read
  files, or access similar system capabilities.

### Parser-Only Probe

Probe location: `/tmp/merman-typst-probe`

Probe shape:

- `cdylib`
- `wasm32-unknown-unknown`
- `merman = { path = ".../crates/merman", default-features = false }`
- one exported function calling `Engine::parse_metadata_with_type_sync` for a flowchart with fixed
  date.

Command:

```bash
cargo build --release --target wasm32-unknown-unknown
wasm-tools print /tmp/merman-typst-probe/target/wasm32-unknown-unknown/release/merman_typst_probe.wasm
```

Observed result:

- Unstripped wasm: about 4.3 MB.
- Import count: 12.
- Import table included `__wbindgen_placeholder__` and `__wbindgen_externref_xform__`.
- JS capability imports included WebCrypto randomness and `js_sys::Date` timezone/time functions.

Conclusion: parser-only `merman` is not currently Typst-plugin-compatible.

### Browser WASM Package

Default command:

```bash
cargo build --profile wasm-size -p merman-wasm --target wasm32-unknown-unknown
```

Observed result:

- `target/wasm32-unknown-unknown/wasm-size/merman_wasm.wasm`: about 8.5 MB.
- Import count: 49.
- Expected browser imports include wasm-bindgen glue, console panic hook, WebCrypto, JS Date, JS
  performance, and serde-wasm-bindgen helpers.

No-default command:

```bash
cargo build --profile wasm-size -p merman-wasm --target wasm32-unknown-unknown --no-default-features
```

Observed result:

- `target/wasm32-unknown-unknown/wasm-size/merman_wasm.wasm`: about 2.7 MB.
- Import count: 25.
- Still includes wasm-bindgen glue, WebCrypto, and JS Date imports.

Conclusion: disabling render/ascii lowers size but does not produce a pure wasm surface.

### Dependency Roots

Commands:

```bash
cargo tree -p merman-core --target wasm32-unknown-unknown -e normal --depth 2
cargo tree -p merman-core --target wasm32-unknown-unknown -e normal -i wasm-bindgen --depth 6
cargo tree -p merman-render --target wasm32-unknown-unknown -e normal --depth 2
```

Important roots:

- `chrono` pulls `js-sys`/`wasm-bindgen` on `wasm32-unknown-unknown` through default clock/timezone
  behavior.
- `uuid` pulls `wasm-bindgen` through the `js` feature and WebCrypto-backed UUID generation.
- `web-time` pulls `js-sys`/`wasm-bindgen` on wasm targets.
- `lol_html`, `url`, `serde_yaml`, and `json5` are direct `merman-core` dependencies and are major
  core slimming candidates.
- `roughr` pulls `rand`/`getrandom` in render paths and needs deterministic/pure-wasm review before
  any Typst SVG subset claims hand-drawn parity.

### WFS-030 Host Clock Split

Change:

- workspace `chrono` now disables default features and enables only `std`;
- `merman-core` default features include `host-clock`, which owns `chrono/clock`;
- `merman-core --no-default-features` uses deterministic UTC fallback behavior for implicit local
  time instead of `chrono::Local`;
- native/render/xtask crates that need system time opt into `chrono/clock` explicitly.

Validation:

```bash
cargo fmt --check -p merman-core -p merman-render -p xtask
cargo check -p merman-core --no-default-features --target wasm32-unknown-unknown
cargo nextest run -p merman-core gantt
cargo nextest run -p merman-core --no-default-features gantt
cargo nextest run -p merman-core --no-default-features runtime
cargo nextest run -p xtask profile_budget
```

Post-change dependency evidence:

```bash
cargo tree -p merman-core --no-default-features --target wasm32-unknown-unknown -e normal -i js-sys --depth 8
cargo tree -p merman-core --no-default-features --target wasm32-unknown-unknown -e normal -i wasm-bindgen --depth 8
cargo tree -p merman-core --no-default-features --target wasm32-unknown-unknown -e features -i chrono --depth 8
```

Observed result:

- `chrono` now appears only through its `std`/`alloc` feature path for `merman-core
  --no-default-features`;
- remaining `js-sys` root is `web-time`;
- remaining `wasm-bindgen` roots are `uuid` and `web-time`.

### WFS-040 Host Random Split

Change:

- workspace `uuid` now disables default features and enables only `std`;
- `merman-core` default features include `host-random`, which owns `uuid/js` and `uuid/v4`;
- block and gitGraph generated IDs use a core runtime helper rather than direct UUID calls;
- `merman-core --no-default-features` uses deterministic generated hex IDs.

Validation:

```bash
cargo fmt --check -p merman-core -p merman-render -p xtask
cargo check -p merman-core
cargo check -p merman-core --no-default-features --target wasm32-unknown-unknown
cargo check -p merman-wasm --target wasm32-unknown-unknown
cargo nextest run -p merman-core block
cargo nextest run -p merman-core git_graph
cargo nextest run -p merman-core --no-default-features block
cargo nextest run -p merman-core --no-default-features git_graph
cargo nextest run -p merman-core --no-default-features runtime
```

Post-change dependency evidence:

```bash
cargo tree -p merman-core --no-default-features --target wasm32-unknown-unknown -e normal --depth 8 | rg "uuid|getrandom|wasm-bindgen|js-sys|web-time"
cargo tree -p merman-core --no-default-features --target wasm32-unknown-unknown -e normal -i wasm-bindgen --depth 8
cargo run -p xtask -- profile-budget check-deps --profile pure-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 3
```

Observed result:

- `uuid` and `getrandom` no longer appear in the `merman-core --no-default-features` wasm tree;
- `profile-budget` failures dropped to `web-time`, `js-sys`, and `wasm-bindgen`;
- remaining `wasm-bindgen`/`js-sys` root is `web-time`, which is WFS-050.

### WFS-050 Host Timing Split

Change:

- `merman-core` default features include `host-timing`, which owns `web-time`;
- core parse timing now goes through runtime timing helpers;
- `merman-core --no-default-features` disables parse timing instrumentation and does not link
  `web-time`.

Validation:

```bash
cargo fmt --check -p merman-core -p merman-render -p xtask
cargo check -p merman-core
cargo check -p merman-core --no-default-features --target wasm32-unknown-unknown
cargo check -p merman-wasm --target wasm32-unknown-unknown
cargo nextest run -p merman-core --no-default-features runtime
cargo nextest run -p merman-core --no-default-features gantt
cargo run -p xtask -- profile-budget check-deps --profile pure-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 3
cargo run -p xtask -- profile-budget check-deps --profile typst-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 3
```

Post-change dependency evidence:

```bash
cargo tree -p merman-core --no-default-features --target wasm32-unknown-unknown -e normal --depth 8 | rg "uuid|getrandom|wasm-bindgen|js-sys|web-time"
```

Observed result:

- pure-wasm and typst-wasm dependency gates pass for `merman-core --no-default-features`;
- the dependency tree filter has no matches for `uuid`, `getrandom`, `wasm-bindgen`, `js-sys`, or
  `web-time`;
- render/layout crates still use timing internally and remain browser/full-surface work until later
  profile or crate-splitting tasks.

### WFS-060 Full Config Split

Change:

- `merman-core` default `full` now enables `full-config`;
- `full-config` owns `serde_yaml`, full YAML frontmatter parsing, and the `json5` directive parser;
- `merman-core --no-default-features` strips closed YAML frontmatter before detection but does not
  apply frontmatter title/config;
- no-default core uses a small built-in inline config parser for common Mermaid metadata and
  directive objects, including flowchart `@{ shape: rounded }`, sequence participant metadata, and
  kanban item metadata;
- the built-in inline parser also covers existing flowchart metadata tests for multiline double
  quoted strings and YAML-style `|` literal block labels.

Validation:

```bash
cargo check -p merman-core
cargo check -p merman-core --no-default-features --target wasm32-unknown-unknown
cargo nextest run -p merman-core inline_config
cargo nextest run -p merman-core --no-default-features inline_config
cargo nextest run -p merman-core parse_merges_frontmatter_and_directive_config parse_metadata_with_type_sync_moves_init_config_without_detection parse_diagram_sequence_extended_participant_syntax_parses_type_override parse_diagram_flowchart_node_data_multiple_properties_same_line knbn_37_ticket_metadata
cargo nextest run -p merman-core --no-default-features frontmatter_is_stripped_without_full_config_but_config_is_not_applied parse_returns_malformed_frontmatter_error_for_unclosed_frontmatter parse_metadata_with_type_sync_moves_init_config_without_detection parse_merges_init_directive_numeric_values_like_upstream parse_diagram_sequence_extended_participant_syntax_parses_type_override parse_diagram_flowchart_node_data_multiple_properties_same_line knbn_37_ticket_metadata
cargo nextest run -p merman-core --no-default-features parse_diagram_flowchart_node_data
cargo nextest run -p merman-core --no-default-features parse_diagram_sequence_extended_participant_syntax
cargo nextest run -p merman-core --no-default-features metadata
cargo run -p xtask -- profile-budget check-deps --profile pure-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 3
cargo run -p xtask -- profile-budget check-deps --profile typst-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 3
```

Post-change dependency evidence:

```bash
cargo tree -p merman-core --no-default-features --target wasm32-unknown-unknown -e normal --depth 8 | rg "serde_yaml|unsafe-libyaml|json5|pest"
cargo tree -p merman-core --target wasm32-unknown-unknown -e features -i serde_yaml --depth 6
cargo tree -p merman-core --target wasm32-unknown-unknown -e features -i json5 --depth 6
```

Observed result:

- the no-default wasm dependency tree filter has no matches for `serde_yaml`, `unsafe-libyaml`,
  `json5`, or `pest`;
- default/full dependency roots remain explicit through `full -> full-config`;
- pure-wasm and typst-wasm dependency budget gates still pass for
  `merman-core --no-default-features`.

### WFS-070 Full Sanitization Split

Change:

- `merman-core` default `full` now enables `full-sanitization`;
- `full-sanitization` owns DOMPurify-like HTML rewriting through `lol_html`;
- `full-sanitization` owns URL canonicalization through the `url` crate;
- `merman-core --no-default-features` keeps dangerous URL protocol filtering but returns the
  cleaned URL without `url` crate canonicalization;
- no-default core uses conservative HTML escaping while preserving Mermaid line break tags such as
  `<br/>`;
- public facade defaults remain full/host, while `merman --no-default-features` now forwards to a
  minimal core dependency graph.

Validation:

```bash
cargo fmt --check
cargo check -p merman-core
cargo check -p merman-core --no-default-features --target wasm32-unknown-unknown
cargo check -p merman --no-default-features --target wasm32-unknown-unknown
cargo check -p merman-wasm --target wasm32-unknown-unknown
cargo nextest run -p xtask profile_budget
cargo nextest run -p merman-core sanitize_text sanitize_url remove_script format_url
cargo nextest run -p merman-core --no-default-features sanitize_text sanitize_url remove_script format_url inline_config parse_diagram_flowchart_node_data parse_diagram_sequence_extended_participant_syntax_parses_type_override knbn_37_ticket_metadata
cargo run -p xtask -- profile-budget check-deps --profile pure-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 8
cargo run -p xtask -- profile-budget check-deps --profile typst-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 8
```

Post-change dependency evidence:

```bash
cargo tree -p merman --no-default-features --target wasm32-unknown-unknown -e normal --depth 4
cargo tree -p merman-core --no-default-features --target wasm32-unknown-unknown -e normal --depth 8 | rg "serde_yaml|unsafe-libyaml|json5|pest|lol_html|url|idna|icu_|cssparser|selectors|encoding_rs|uuid|getrandom|wasm-bindgen|js-sys|web-time"
```

Observed result:

- `profile-budget` reports zero failures for pure-wasm and typst-wasm core profiles;
- the no-default wasm dependency tree filter has no matches for full config, full sanitizer,
  host-random, host-timing, or wasm-bindgen/browser crates;
- default `merman-wasm` still builds for `wasm32-unknown-unknown`, preserving the browser
  wasm-bindgen package surface.

## Gates

### Always-Preserve Gates

These protect existing full/native/browser behavior:

```bash
cargo fmt --all --check
cargo nextest run -p merman-core
cargo nextest run -p merman-render
cargo nextest run -p merman-bindings-core
cargo run -p xtask -- check-alignment
```

Use narrower task-local gates first, but closeout must explain any skipped full gate.

### Dependency Gates

Before and after each dependency-slimming task, capture:

```bash
cargo tree -p merman-core --target wasm32-unknown-unknown -e normal --depth 3
cargo tree -p merman --no-default-features --target wasm32-unknown-unknown -e normal --depth 3
cargo tree -p merman-wasm --target wasm32-unknown-unknown -e features --depth 4
```

Pure-wasm target acceptance:

- no `wasm-bindgen`;
- no `js-sys`;
- no WebCrypto/random JS imports;
- no JS Date/timezone imports;
- no browser panic hook imports.

The repository now has an initial dependency allowlist gate:

```bash
cargo run -p xtask -- profile-budget check-deps --profile pure-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 3
cargo run -p xtask -- profile-budget check-deps --profile typst-wasm --package merman-core --target wasm32-unknown-unknown --no-default-features --depth 3
```

Both profiles currently fail on `wasm-bindgen`, `js-sys`, `web-time`, `getrandom`,
`serde-wasm-bindgen`, `wasm-bindgen-futures`, or `console_error_panic_hook` when those crates are
present in the checked tree. Additional blocked crates can be supplied with repeated `--forbid`
flags.

### Import Allowlist Gate

For a Typst plugin or probe wasm:

```bash
wasm-tools print <plugin.wasm> | awk '/^  \(import/{print}'
```

Allowed imports:

```text
(import "typst_env" "wasm_minimal_protocol_write_args_to_buffer" ...)
(import "typst_env" "wasm_minimal_protocol_send_result_to_host" ...)
```

Everything else must be justified as a blocker or removed. In particular, these are failures:

- `__wbindgen_placeholder__`
- `__wbindgen_externref_xform__`
- `js`
- `wasi_snapshot_preview1`
- WebCrypto/getRandomValues
- JS Date/timezone/performance

The repository now has an initial checked gate for this:

```bash
cargo run -p xtask -- profile-budget check-wasm --profile typst-wasm --wasm <plugin.wasm>
cargo run -p xtask -- profile-budget check-imports --profile pure-wasm --wat-file <wasm-tools-print.wat>
```

`typst-wasm` allows only the two wasm-minimal-protocol `typst_env` imports and requires exported
`memory` when using `check-wasm` or `check-exports`. `pure-wasm` currently allows no imports.

### Export Gate

For a Typst plugin or probe wasm:

```bash
wasm-tools print <plugin.wasm> | awk '/^  \(export/{print}'
```

Required:

- exported `memory`;
- one or more user-facing plugin functions with Typst-compatible signatures.

The `xtask profile-budget check-wasm --profile typst-wasm --wasm <plugin.wasm>` gate enforces the
exported `memory` requirement. Function signature checking remains a follow-up.

### Size Gate

Do not set a hard budget until WFS-020 produces repeatable measurements. Initial observed
baselines:

| Profile | Size | Imports | Notes |
| --- | ---: | ---: | --- |
| parser-only probe | ~4.3 MB | 12 | Not Typst-compatible. |
| `merman-wasm` default `wasm-size` | ~8.5 MB | 49 | Browser package, render + ascii. |
| `merman-wasm --no-default-features` `wasm-size` | ~2.7 MB | 25 | Browser transport only, still JS-bound. |

Proposed acceptance direction:

- pure parser probe should be materially smaller than the current 4.3 MB and have no JS imports;
- browser slim/core package should be documented separately from full render package;
- Typst SVG subset size should be budgeted after admitted families and rough/hand-drawn support are
  decided.
