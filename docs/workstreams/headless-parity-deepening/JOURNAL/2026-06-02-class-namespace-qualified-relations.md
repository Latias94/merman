# HPD-080 Class Namespace-Qualified Relations

Date: 2026-06-02

## Source Evidence

- Pinned Mermaid 11.15 commit:
  `41646dfd43ac83f001b03c70605feb036afae46d`
- Audited source:
  `packages/mermaid/src/diagrams/class/classDb.ts`
- Mermaid 11.15 `addRelation(...)` calls `addClass(...)` for relation endpoints, then assigns
  `classRelation.id1` and `classRelation.id2` from `splitClassNameAndType(...).className`.
- Mermaid does not resolve `Outer.Foo` relation endpoints back to an existing namespace member
  `Foo`; fully-qualified endpoints can therefore create implicit top-level facade class nodes.

## Change

- Removed the Class core shortcut that resolved namespace-qualified relation endpoints to existing
  namespace member classes.
- Updated the focused Class core regression to document Mermaid-like facade classes and
  fully-qualified relation endpoints.
- Kept ASCII output readable by folding only empty namespace facade classes back to their declared
  namespace member at render time. This is an ASCII view concern, not core semantic truth.
- Updated the Class SVG HTML-cap guard to the current deterministic headless output. The structural
  authority remains the pinned upstream Class SVG comparison.

## Verification

- `cargo fmt -p merman-core -p merman-render -p merman-ascii`
- `cargo fmt --check -p merman-core -p merman-render -p merman-ascii`
- `cargo test -p merman-core class --lib`
- `cargo test -p merman-render --test class_svg_test`
- `cargo test -p merman-ascii class --test class_model`
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Outcome

- The previously exposed Class structural mismatches are closed:
  - `stress_class_comments_inside_namespaces_024`
  - `stress_class_nested_namespaces_many_levels_021`
  - `stress_class_unicode_namespace_mix_017`
- ASCII keeps the previous no-duplicate-box behavior without requiring core to pretend namespace
  facade classes do not exist.

