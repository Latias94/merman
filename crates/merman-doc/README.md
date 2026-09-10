# merman-doc

Shared Markdown scanning and static HTML wrapping for `merman-rustdoc` and the
`merman rustdoc` command. This crate has no renderer, filesystem, or Rust syntax dependency.

`scan` returns source ranges for Mermaid fences, standalone `include_mmd!("path")`
directives, and individual syntax errors. Callers retain control of file resolution,
rendering, diagnostics, and whether to preserve a failed block. `visit_blocks` additionally
accepts cancellation and admission callbacks so command-line callers can stop before
collecting or rendering diagrams above their resource limits.

Markdown structure comes from `pulldown-cmark`. Backtick and tilde Mermaid fences
are supported; colon fences are ordinary Markdown text. HTML comments, indented
code, other fenced languages, and inline examples do not execute includes.
Includes accept one double-quoted path on its own source line. Ordinary escapes
such as `\\` and `\"` are decoded with `serde_json`; raw Rust string literals and
Rust-only escapes such as `\u{2026}` are not supported. Use forward slashes or
JSON-compatible escapes when migrating existing macro includes. Unclosed directives
produce per-block diagnostics. Existing `{mermaid}` and comma-separated Mermaid
fence info aliases remain supported.

Diagram blocks inside ordered and unordered lists, block quotes, and footnotes retain
their container structure, including nested combinations and task list checkboxes.
Use explicit indentation and quote markers on standalone include lines. Lazy paragraph
continuations without those prefixes produce a diagnostic asking for the missing prefix.
The scanner returns an `Embedding` with each block; use its `write_html` method to insert
generated HTML with the correct first-line and continuation prefixes. `html_len` counts
the exact embedded byte length for callers that enforce output limits.
At the end of a complete document, embedding preserves the original trailing line endings without
adding blank lines. Use `with_trailing_boundary` when the scanned source is only a fragment with
more documentation supplied separately, such as a group of Rust doc attributes.

`write_diagram_html` accepts either one SVG or light/dark SVG variants and writes
an optional escaped source disclosure. It inserts blank lines around the HTML so
adjacent Markdown paragraphs retain their formatting. SVG input must already have
been validated by the renderer. Wrapper IDs and source text are HTML-escaped.
Multiline SVG is serialized with `quick-xml`: XML text values and normalized attribute
values are retained, while literal newlines become character references. CDATA becomes
escaped text and comments are omitted. XML declarations, processing instructions, and
DTDs are rejected in multiline inline SVG. This prevents blank lines inside labels or
attributes from terminating the enclosing Markdown HTML block.

The Markdown parser itself is synchronous. Cancellation is checked before parsing,
between parser events, and while indexing source lines; a single parser operation
cannot be interrupted.

## License

Licensed under either the MIT license or the Apache License, Version 2.0, at your option.
