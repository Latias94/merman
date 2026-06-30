# merman Typst Gallery

This gallery is the maintained index for Typst package smoke artifacts. It is not a pixel-perfect visual parity suite; it is a quick way to inspect whether the package can render representative Mermaid families through the built `@preview/merman:<version>` package.

Run locally:

```sh
cargo run -p xtask -- typst-package-smoke --skip-wasm-build --typst /path/to/typst --keep-artifacts
```

Artifacts are written to `target/typst-package-smoke/out`.

## Examples

- `examples/basic.pdf`: minimal image rendering with `mermaid`.
- `examples/document-context.pdf`: `document-context: true` for direct calls and raw blocks.
- `examples/profile.pdf`: profile reuse across direct calls and raw Mermaid fences.
- `examples/figure.pdf`: Typst `figure` wrapping with profile figure defaults.
- `examples/raw-block.pdf`: document-wide Mermaid raw block show rule.
- `examples/options.pdf`: structured result, SVG export, theme options, and placeholder errors.
- `examples/print.pdf`: print-friendly white-background profile.
- `examples/presentation.pdf`: dark slide profile.
- `examples/svg-export.pdf`: raw SVG/result inspection.

## Fixture Families

- `tests/api/test.pdf`: canonical API surface, profiles, options escape hatch, capabilities, figures, and raw block show rules.
- `tests/context/test.pdf`: explicit default behavior, context opt-in, direct override, and layout width precedence.
- `tests/errors/test.pdf`: structured invalid-source errors plus text and placeholder in-document modes.
- `tests/figure/test.pdf`: caption, placement/scope defaults, profile figure defaults, direct overrides, and document context.
- `tests/raw-blocks/test.pdf`: explicit and document-context raw block handlers.
- `tests/issues/test.pdf`: high-risk cases such as duplicate raw block ids and invalid Mermaid placeholders.
- `tests/readme-examples/test.pdf`: README migration snippets for removed context wrappers.
- `tests/visual/test.pdf`: representative Flowchart, Sequence, Class, ER, State, and Git Graph outputs.

## Current Boundaries

- Typst output embeds SVG images; it is not Typst-native vector drawing.
- Font family and size are forwarded as renderer style intent. Exact Typst font glyph measurement is not part of the current package contract.
- Warnings about SVG `foreignObject` can still appear for readable/parity-style SVG output. The `resvg-safe` pipeline is the intended embedded-image default.
