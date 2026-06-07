# HPD-050 - Shared Config And Directive Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After the Dugong/Graphlib cycle traversal slice, the next shared public-input candidate was config
handling rather than another diagram family. Mermaid config reaches core through:

- host `site_config`;
- YAML frontmatter `config`;
- `%%{init: ...}%%` / `%%{initialize: ...}%%` directives.

Those paths all flowed through `MermaidConfig::deep_merge(...)`, and init directives also flowed
through `sanitize_directive(...)`.

## Red Signal

Focused small-stack regressions reproduced stack overflow before the fixes:

- deep host `site_config` merging through public metadata parsing;
- deep frontmatter config before YAML parsing / merge cleanup;
- deep init directive config before JSON5 parsing / sanitizer cleanup.

Diagnosis narrowed the text-input overflows to third-party YAML / JSON5 parser depth before local
merge code could run, while the lower-level sanitizer and config clone/drop paths still needed
explicit-stack coverage.

## Changes

- Replaced `MermaidConfig` recursive clone-on-write and merge/drop behavior with explicit
  heap-backed helpers:
  - non-recursive `serde_json::Value` clone;
  - non-recursive `serde_json::Value` drop;
  - non-recursive replacement for overwritten config values;
  - iterative `deep_merge(...)` path traversal.
- Kept legacy root `fontFamily` mirroring behavior but made its replacement/drop path
  non-recursive.
- Changed frontmatter processing to:
  - strip frontmatter by line scanning instead of a broad regex;
  - reject config nesting beyond `MAX_DIAGRAM_NESTING_DEPTH` before YAML parsing, covering flow
    collections, YAML indentation, and inline YAML sequence indicators;
  - preserve the legacy `serde_yaml::Value` to `serde_json::Value` conversion behavior, including
    non-string YAML key compatibility;
  - consume converted values with non-recursive merge/drop paths.
- Changed init directive processing to:
  - reject config nesting beyond `MAX_DIAGRAM_NESTING_DEPTH` before JSON5 parsing;
  - clone directive args non-recursively before sanitization;
  - sanitize through an explicit object/array path stack;
  - remove blocked keys with non-recursive drops.
- Changed `DetectorRegistry::detect_type(...)` frontmatter stripping to the same line-scanning
  model so direct detector callers do not depend on the old regex over deep input.

## Verification

- `cargo +1.95 nextest run -p merman-core clone_on_write_handles_deep_config_with_small_stack sanitize_directive_handles_deep_values_with_small_stack detector_registry_strips_deep_frontmatter_with_small_stack site_config_deep_merge_handles_deep_public_config_with_small_stack init_directive_config_sanitizes_deep_values_with_small_stack frontmatter_config_deep_merge_handles_deep_values_with_small_stack init_directive_rejects_excessive_config_nesting_with_small_stack frontmatter_rejects_excessive_config_nesting_with_small_stack frontmatter_rejects_excessive_inline_yaml_sequence_nesting_with_small_stack frontmatter_non_string_yaml_keys_are_ignored_like_legacy_conversion config_nesting_counts_inline_yaml_sequence_indicators` -
  passed, `11` tests run.
- `cargo +1.95 nextest run -p merman-core` - passed, `609` tests run.
- `cargo +1.95 fmt` - passed.
- `git diff --check` - passed.

## Boundary

No SVG baseline, root override, Mermaid parity fixture, rendered output formula, Architecture
root-bounds behavior, or theme derivation formula changed. This slice is shared parser/config
stack-safety hardening. The repo's default `1.95.0` cargo shim reported an unusable cargo component
during verification, so commands used the installed `1.95-x86_64-pc-windows-msvc` toolchain
explicitly.
