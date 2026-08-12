# Changelog

The extension has not yet been published. These entries remain `Unreleased` for the planned first `0.1.0` release.

## Unreleased

### Added

- Added the initial Merman VS Code preview extension with local Mermaid diagnostics, completion, hover, symbols, references, rename, code actions, semantic tokens, preview, SVG/PNG export, and copy actions.
- Added bundled `merman-lsp` and `merman-cli` runtime binaries per platform through `bin/<platform>-<arch>/`.
- Added parser-backed Mermaid semantic-token declarations and default semantic highlighting generated from the shared editor token descriptor.

### Compatibility

- The first release targets the Mermaid 11.16.1 35-family parser-backed language stack and uses `merman.diagnostics.enabled` to control Problems without disabling language intelligence.
- Custom `merman-lsp` binaries must match the extension's editor schema, semantic-token descriptor digest, packed-token encoding, and negotiated legend projection. A mismatch stops language intelligence instead of applying stale token meanings.
- Packaged VSIX artifacts now include the project license, source-provenance notice, and exact third-party license texts; preview SVG follows the shared DOM safety policy.
