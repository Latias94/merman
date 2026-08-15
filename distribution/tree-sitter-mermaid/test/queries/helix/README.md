# Helix Query Profile Evidence

This directory fixes the profile contract to Helix `25.07.1` at commit
`a05c151bb6e8e9c65ec390b0ae2afe7a5efd619b`.

Helix loads exactly five Tree-sitter query files from `helix-core/src/syntax.rs`:
`highlights.scm`, `injections.scm`, `locals.scm`, `indents.scm`, and
`textobjects.scm`. The package therefore does not create empty `folds.scm`,
`tags.scm`, `brackets.scm`, or `outline.scm` files. Those four surfaces are
explicit `not_applicable` cells in `applicability.json` for all 35 families.

The capture goldens exercise the five native surfaces. `verify.js` additionally
compiles every query, checks the Helix capture vocabulary, expands the complete
35-family by nine-surface applicability contract, and executes every applicable
cell against a family fixture. It intentionally loads the built language addon
directly; the canonical binding and artifact-receipt checks remain package-level
gates and must be regenerated after profile integration.

Run the local evidence check after building the Node binding:

```console
node test/queries/helix/verify.js
```

Markdown-looking labels are not injected in this profile yet. Most family nodes
still include their quote/backtick delimiters, while Helix does not implement the
Neovim offset directive. YAML frontmatter uses delimiter-free
`frontmatter_content` nodes and is safe to inject as combined ranges.
