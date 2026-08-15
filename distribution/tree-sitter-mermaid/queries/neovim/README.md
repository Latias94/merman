# Neovim query profile

This profile targets Neovim `v0.12.4` and the language ABI exposed by the
committed Mermaid parser. The fixed version is intentionally blocking; probes
against newer Neovim releases are compatibility signals, not a reason to
silently change the query contract.

`highlights.scm` is an adapter loaded after `queries/portable/highlights.scm`.
It adds only Neovim's `@spell` capture, so portable syntax captures remain the
single semantic vocabulary. The other files are complete Neovim query groups.
The fixed-editor smoke compiles the raw profile, including Neovim's `#offset!`
directive. The Node structural verifier removes only those host-owned offset
lines because the pinned Node runtime rejects predicates it does not execute.

The profile follows established consumer conventions:

- core/nvim-treesitter: `@fold`, `@indent.*`, `@injection.*`, and
  `@local.*`;
- Tree-sitter tags: `@definition.*` and `@name`;
- rainbow-delimiters.nvim: `@container` and `@delimiter`;
- aerial.nvim: `@symbol`, `@name`, and a `kind` property;
- nvim-treesitter-textobjects: `@class.*`, `@block.*`, `@function.*`,
  `@conditional.*`, `@loop.*`, `@assignment.*`, `@parameter.*`, and
  `@comment.*`.

`test/queries/neovim/applicability.json` is the executable, explicit 35-by-9
matrix. Every `applicable` cell names its query and the captures that must occur
on its representative source. A `not_applicable` cell must provide a non-empty
rationale; an empty query file is never evidence of non-applicability.
