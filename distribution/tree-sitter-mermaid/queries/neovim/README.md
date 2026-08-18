# Neovim query profile

This profile is a pre-1.0 adoption asset for Neovim and nvim-treesitter. It
targets the language ABI exposed by the committed Mermaid parser, but the
downstream integration remains responsible for pinning a released repository
revision and validating it against the Neovim version it supports.

`highlights.scm` is an adapter loaded after `queries/portable/highlights.scm`.
It adds only Neovim's `@spell` capture, so portable syntax captures remain the
single semantic vocabulary. The other files are complete Neovim query groups.
Merman's package tests compile the shipped queries against the generated
language; real Neovim loader behavior is validated when preparing a downstream
adoption change rather than through a second package-owned capture matrix.

The profile follows established consumer conventions:

- core/nvim-treesitter: `@fold`, `@indent.*`, `@injection.*`, and
  `@local.*`;
- Tree-sitter tags: `@definition.*` and `@name`;
- rainbow-delimiters.nvim: `@container` and `@delimiter`;
- aerial.nvim: `@symbol`, `@name`, and a `kind` property;
- nvim-treesitter-textobjects: `@class.*`, `@block.*`, `@function.*`,
  `@conditional.*`, `@loop.*`, `@assignment.*`, `@parameter.*`, and
  `@comment.*`.

Query and capture changes are pre-1.0 API changes. Keep them source-reviewed and
compile-clean, then exercise the affected behavior in the downstream editor
repository that owns its runtime contract.
