# ADR-0087: Shared Rustdoc Markdown And Deferred Macro Rendering

- Status: accepted
- Date: 2026-09-10
- Related: [ADR-0082](0082-rustdoc-dual-path-integrations.md), [ADR-0077](0077-presentation-theme-and-output-ownership.md)

## Context

The CLI rustdoc generator and the `merman-rustdoc` attribute macro have independent product and
packaging contracts, but duplicate Markdown scanning, diagram HTML wrapping, and parts of SVG
validation. Both scanners can mistake code examples or comments for executable diagram input and
lose Markdown block boundaries around inserted HTML. The macro also renders descendants before
Rust removes conditionally disabled syntax, and content-derived SVG identities collide between
separately annotated items on one page.

The root SVG background is another ownership mismatch: rustdoc embeds diagrams into a page whose
background changes with its theme, while the renderer's default root background is white. Mermaid
theme configuration does not own this output decision under ADR-0077.

## Decision

### Share document transformations below the product integrations

`merman-doc` owns Markdown diagram discovery, source replacement ranges, and HTML wrapping for
rustdoc. It uses the Markdown parser's structural events and source offsets, replaces only diagram
ranges, and preserves surrounding source for rustdoc to interpret. It does not render Mermaid,
read files, inspect Rust syntax, evaluate configuration conditions, or manage generated bundles.

The shared transformation supports Mermaid fences and standalone `include_mmd!` directives at the
top level and inside ordered or unordered lists, blockquotes, footnotes, and their nested
combinations. Replacement metadata retains the original container's opening and continuation
prefixes so generated HTML stays within that container. It preserves HTML comments, inline code,
indented examples, other language fences, and neighboring Markdown. Includes on lazy continuation
lines without explicit container indentation or blockquote markers report a prefix error;
`fail = "keep-source"` preserves the original source. This is source-range replacement within
parser-recognized containers, not serialization of a complete Markdown document.

The macro retains Rust attribute adaptation, option parsing, diagnostics, and include resolution.
The CLI retains configuration, local file ownership, bundle receipts, transactional publication,
and read-only freshness checks. Both integrations use renderer-owned SVG validation and output
policies. Neither integration depends on or invokes the other: sharing lower-level libraries does
not merge the two independently packaged products established by ADR-0082. CLI consumers can still
publish static fragments without a Merman dependency in the documented crate.

### Defer rendering to compiler-selected documentation

The public `merman` attribute keeps its one-line authoring form. It collects literal documentation
and produces hidden function-like macro expressions in `doc` attributes. Actual diagram rendering
and include reads occur when those expressions expand, after Rust processes the enclosing syntax's
conditional compilation. Disabled functions, fields, and variants therefore do not trigger work.
The implementation does not evaluate `cfg` or expand arbitrary Rust macros itself.

A parent `scope = "tree"` applies rendering options to visible descendants. An enabled child
`merman` attribute recognizes deferred documentation and inherits parent rendering options,
overriding only explicitly supplied fields. `scope` is local and defaults to `item`; it is never
inherited. The child's default scope changes only that item's effective rendering options, while
`scope = "tree"` propagates them to its visible descendants. `inherit = "off"` starts from
defaults, including clearing the inherited `id_prefix`, before applying local options. A disabled
child `cfg_attr` leaves the parent configuration intact. Internal helper paths support Cargo
dependency renaming.

The macro reads existing deferred documentation within its selected scope to recover the preceding
configuration, using the same syntax-tree visitor as rewriting. It does not resolve child macro
names, so imports and aliases behave identically. An undocumented item receives no synthetic doc
attribute, preserving `missing_docs` diagnostics. A `scope = "item"` attribute on such an item is
a no-op; it does not save options for a later tree attribute. Otherwise, stacked attributes apply
to the existing documentation in expansion order.

Literal doc attributes are combined across ordinary attributes, and line, block, and multiline
literal documentation is normalized without converting indented examples into diagrams. Dynamic
`doc` expressions remain unchanged and delimit literal groups. Evaluating `include_str!` document
expressions and rewriting crate-level inner documentation remain outside the macro's scope.
The attribute adapter compensates for rustdoc trimming the outer newline of a multiline doc value
that begins with a newline, preserving the separator before a following dynamic doc expression.
Complete CLI documents instead preserve their original trailing line endings without adding
generated blank lines at EOF.

### Keep theme, background, safety, and identity independent

The macro preserves `scope`, `pipeline`, `fail`, `source`, `sanitize`, and `theme`, and adds
`inherit = "on" | "off"` with `on` as the default. Options remain string literals; unknown and
duplicate options are rejected.

- `background` defaults to `transparent` and sets the renderer's root SVG output background.
  It is independent of Mermaid `themeVariables.background` and of light/dark theme selection.
- `id_prefix` is an optional nonempty namespace containing ASCII letters, digits, hyphens, or
  underscores. It augments automatic occurrence identity rather than replacing instance suffixes.
  Ordinary users need not configure it; callers generating overlapping source locations can give
  their invocations distinct prefixes.
- Automatic SVG identity includes the documented occurrence, diagram index, and theme variant.
  Built-in `line!()` and `column!()` are evaluated in the final doc expression to distinguish
  separate outer declarative-macro calls. Identical methods generated by one call, or calls
  at identical positions in different files, require distinct `id_prefix` values or attributes
  on the complete impl with `scope = "tree"`.
  Machine-specific absolute paths must not become part of generated identity. ID rewriting updates
  references together with their targets and is applied even when `sanitize = "off"`.
- `sanitize = "strict"` reuses renderer-owned checks for SVG, embedded HTML resources, and CSS URLs,
  including escaped URL syntax. Browser output retains safe `foreignObject` labels. Label fallback
  conversion remains a separate pipeline choice.
- Mermaid source configuration continues to override the macro's theme default. The existing
  `parity`, `readable`, and `resvg-safe` pipeline meanings remain unchanged.

### Integrate include tracking with the consumer's build script

`include_mmd!` paths remain relative to the consuming crate's `CARGO_MANIFEST_DIR`. The macro's
file reads do not automatically establish Cargo dependencies, and `proc_macro::tracked::path`
requires nightly. The supported stable integration is a consumer-owned `build.rs` with no new
build dependency:

```rust
fn main() {
    println!("cargo::rerun-if-changed=docs/diagrams");
}
```

The corresponding include is `include_mmd!("docs/diagrams/architecture.mmd")`. Watching an existing
dedicated directory covers modifications, additions, deletions, and missing-file restoration for
`fail = "keep-source"`. Consumers with an existing build script add the instruction to it; adding
the first explicit rerun instruction requires retaining any other inputs previously covered by
Cargo's default package-wide tracking.

Paths are relative to the consumer's manifest, including cross-package or package-external inputs.
A local include under `../shared/docs/diagrams` needs that directory watched by the consumer.
Published crates must contain their required diagrams, accounting for package include allowlists;
workspace-relative external inputs are not portable publication resources. CLI generation and
freshness checks remain an alternative for committed documentation fragments.

Do not append source data to generated HTML or inject unrelated items to simulate file dependency
tracking. The build-script contract keeps file invalidation with Cargo and rendering with the macro.

## Compatibility And Validation

Transparent root backgrounds and occurrence-based SVG IDs intentionally change generated output.
Consumers should not rely on historical IDs. Strict resource checks can reject previously accepted
unsafe output. Repeated options now produce diagnostics rather than silently selecting the last
value. Inputs previously misinterpreted inside Markdown examples or comments remain source text.
These changes are ahead of published alpha.6 and are intended for the next workspace release;
installation snippets for alpha.6 must not imply it already supports the new options or behavior.

Regression coverage must exercise actual rustdoc output for conditionally disabled children,
nested macro overrides, dependency renaming, literal documentation forms, Markdown block
boundaries, and multiple diagrams on the same page. Renderer tests own resource validation and
reference rewriting; shared document tests own source-range and container behavior. The CLI keeps
its independent generation, transaction, and freshness validation.
Consumer-level Cargo tests additionally verify that unchanged inputs stay cached and that diagram
modification, deletion, and restoration regenerate docs without changing Rust source.

The CLI fragment rule from ADR-0082 remains: one pre-generated fragment may appear at most once on
a rendered page. Build-time instance isolation cannot distinguish repeated insertion of identical
already-generated bytes.

## Rejected Alternatives

1. Keep patching two scanners and two security implementations.
   This preserves duplicated interpretation and allows the same defects to recur in each product.
2. Build a general Markdown rewriting framework or Rust condition evaluator.
   Parser-backed source ranges and container prefixes handle the supported diagrams; Rust owns
   conditional compilation.
3. Force a rendering pipeline or theme preset to change the background.
   This violates output ownership in ADR-0077 and couples independent user choices.
4. Use global counters or machine-specific paths for SVG identity.
   Expansion order and checkout location must not determine generated documentation.
5. Merge CLI generation into the macro or make either product discover the other.
   This breaks the independent deployment and dependency contracts of ADR-0082.

## Renderer Integration Corrections

Strict browser admission exposed existing output defects. Eventmodeling, Ishikawa, TreeView,
and ZenUML now scope built-in CSS to the SVG root. Error diagrams emit SVG-namespace styles.
Mindmap retains the upstream duplicate background path IDs in raw parity output. Before ID
isolation, the embedding pipeline removes only background path IDs that duplicate their direct
node group in renderer-certified Mindmap output. Other duplicate IDs still fail admission, and
arbitrary input cannot opt into the repair by declaring its diagram type. Safe XHTML navigation
links use the existing URL policy. Upstream baselines and comparator rules remain unchanged.
