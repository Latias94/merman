# Changelog

The extension has not yet been published. These entries remain `Unreleased` for the planned first `0.1.0` release.

## Unreleased

### Added

- Added the initial Merman VS Code preview extension with local Mermaid diagnostics, completion, hover, symbols, references, rename, code actions, Tree-sitter-backed syntax highlighting, preview, SVG/PNG export, and copy actions.
- Added bundled `merman-lsp` and `merman-cli` runtime binaries per platform through `bin/<platform>-<arch>/`.
- Added standard LSP semantic-token highlighting produced from the canonical Tree-sitter Mermaid query.

### Compatibility

- The first release targets the Mermaid 12.1.0 parser-backed language stack, including Agentflow and Usecase and uses `merman.diagnostics.enabled` to control Problems without disabling language intelligence.
- Prepared runtime binaries follow Merman `0.9.0`; the extension keeps its independent `0.1.0` version. Native CLI preview/export uses deterministic text measurement by default, transparent SVG backgrounds, and the corrected Unicode wrapping. Set an explicit background when opaque SVG is required.
- Custom `merman-lsp` binaries must provide the standard LSP capabilities used by the extension and the compatible Merman analysis-config contract.
- Packaged VSIX artifacts now include the project license, source-provenance notice, and exact third-party license texts; preview SVG follows the shared DOM safety policy.
