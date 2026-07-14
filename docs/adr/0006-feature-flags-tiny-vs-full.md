# ADR-0006: Feature Flags (Tiny vs Full)

## Status

Accepted

## Context

Upstream Mermaid has a “tiny” build that excludes some large features/diagrams. We want to support
both without forking the codebase.

## Decision

- Introduce a feature split to distinguish “full” vs “tiny” behavior.
- Default is “full” to match `mermaid` package behavior.
- Current implementation:
  - `merman-core` default features include `full` and `host`.
  - `full` remains the compatibility profile and enables `full-registry`, `full-config`, and
    `full-sanitization`.
  - `full-registry` selects the complete detector/parser registry. Without it, Architecture,
    Mindmap, and `flowchart-elk` are not registered.
  - `full-config` enables full YAML frontmatter parsing and JSON5 directive parsing.
  - `full-sanitization` enables DOMPurify-like HTML sanitization and URL canonicalization.
  - `host` enables `host-clock`, `host-random`, and `host-timing`.
  - Build pure/tiny parser profiles via `--no-default-features` (disables `full` and `host`). The
    current registry split is runtime gating and does not yet remove every family module or parser
    grammar from compilation.
- “tiny” primarily affects:
  - which diagrams are registered/detectable,
  - optional dependencies (e.g. KaTeX),
  - layout engines.
- Host capability features affect whether parsing can use ambient system/browser capabilities.
  `host-clock` owns system local time, `host-random` owns UUID-backed generated IDs, and
  `host-timing` owns parse timing instrumentation.
- Pure/tiny parser profiles keep common Mermaid inline metadata through a built-in parser, but do
  not apply YAML frontmatter title/config without `full-config`.
- Pure/tiny parser profiles use conservative HTML escaping without `full-sanitization`; they do not
  claim DOMPurify parity or URL canonicalization parity.

Feature surfaces and host profile expectations are documented in `docs/FEATURES.md`.

Related: `dugong` also exposes an optional parity-oriented pipeline (`layout_dagreish`) behind the
`dugong/dagreish` feature (enabled by default).

## Consequences

- Detector order and diagram registry become feature-dependent and must be documented and tested.
- Analysis, editor-core, and LSP crates expose granular forwarding features while retaining
  `core-full` and `core-host` as compatibility aliases.
- The LSP stdio executable is gated by its orthogonal `stdio` feature; the library can be built
  without a transport binary.
