# HPD-050 - ClassDB Member/accDescr Regex Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

ClassDB still compiled two local regex helpers on public class parsing paths:

```rust
Regex::new(r"^([#+~-])?(.+)\((.*)\)([\s$*])?(.*)([$*])?$")
Regex::new(r"\n\s+")
```

Pinned Mermaid 11.15.0 defines method member parsing in
`repo-ref/mermaid/packages/mermaid/src/diagrams/class/classTypes.ts`:

```ts
const methodRegEx = /([#+~-])?(.+)\((.*)\)([\s$*])?(.*)([$*])?/;
```

That regex is greedy: earlier parentheses stay in the method id when a later `(...)` parameter
list exists.

## Changes

- Removed `regex::Regex`, `OnceLock`, `METHOD_RE`, and `ACC_DESCR_RE` from
  `crates/merman-core/src/diagrams/class/db.rs`.
- Replaced method parsing with a source-shaped scanner that uses the last `(` before the last `)`
  as the parameter-list boundary.
- Preserved Mermaid classifier handling for `$` / `*` immediately after `)` or at the end of the
  return type payload.
- Replaced multiline class `accDescr` `\n\s+` cleanup with a direct scanner that collapses
  whitespace after newlines.
- Added public parser regressions for a method id containing earlier parentheses and for multiline
  accessibility description normalization.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core parse_diagram_class_method_parser_matches_upstream_greedy_regex_boundary parse_diagram_class_acc_descr_multiline_collapses_newline_whitespace_without_regex` -
  passed, `2` tests run.
- `cargo +1.95 nextest run -p merman-core class` - passed, `49` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF/CRLF conversion warning.
- `rg -n 'Regex|regex::|OnceLock|METHOD_RE|ACC_DESCR_RE|class method regex|class acc descr regex' crates/merman-core/src/diagrams/class/db.rs` -
  no ClassDB regex dependency or helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed, `835`
  lines parsed.

## Boundary

This is a source-backed Class parser panic-surface cleanup and method parser boundary convergence.
It does not change Class layout, renderer SVG output, namespace semantics, link/callback behavior,
common sanitizer policy, Gantt date parsing, retained config projection, SVG baselines, root
viewport formulas, or Architecture residual classification.
