# Third-Party Notices

This file records source translated, copied, generated, or embedded in `crates/merman-markup`.
It is generated from `docs/release/THIRD_PARTY_COMPONENTS.json`.

## DOMPurify

Merman selects DOMPurify's Apache-2.0 option for generated sanitizer defaults; the exact upstream Apache-2.0 license file is preserved.

- Version: `3.4.17`
- Source: https://github.com/cure53/DOMPurify.git @ `a77a3e78b59f72fa4d1c83f22e121ad9cc7d3575`
- Relationship: `generated`, `translated`
- License: `(Apache-2.0 OR MPL-2.0)`
- Legal file: `THIRD_PARTY_LICENSES/dompurify/LICENSE`

## Mermaid

Merman independently implements Mermaid-compatible behavior while translating selected algorithms, generating defaults, copying architecture icon data, and retaining upstream fixtures and snapshots.

- Version: `12.3.0`
- Source: https://github.com/mermaid-js/mermaid.git @ `2086904e76c692331e34a45d7cc3bc30860d040e`
- Relationship: `behavior-reference`, `copied`, `fixtures`, `generated`, `modified`, `translated`
- License: `MIT`
- Legal file: `THIRD_PARTY_LICENSES/mermaid/LICENSE`

## sanitize-url

Merman's URL sanitization behavior is a source-backed Rust translation of sanitize-url.

- Version: `7.1.2`
- Source: https://github.com/braintree/sanitize-url.git @ `206cb8b942b10dad28dd3d31db3bf39e3fcdd7aa`
- Relationship: `modified`, `translated`
- License: `MIT`
- Legal file: `THIRD_PARTY_LICENSES/sanitize-url/LICENSE`
