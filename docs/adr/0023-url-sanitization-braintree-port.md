# ADR-0023: URL Sanitization Parity (`@braintree/sanitize-url@7.1.1`)

## Status

Accepted

## Context

Mermaid `utils.formatUrl()` delegates URL sanitization to `@braintree/sanitize-url` when
`securityLevel !== 'loose'`.

For a 1:1 clone pinned to `mermaid@11.12.2`, the effective URL sanitization behavior is therefore
the combination of:

- Mermaid's `formatUrl` contract (trim + conditional sanitization), and
- `@braintree/sanitize-url` exact behavior (decoding/normalization + unsafe-scheme blocking).

Our initial implementation only blocked a few obvious schemes (`javascript:`, `vbscript:`, `data:`),
which was not sufficient for full parity (e.g. encoded/escaped attack vectors and normalization
rules).

## Decision

- Implement `merman-core::utils::sanitize_url` as a Rust port of
  `@braintree/sanitize-url@7.1.1`.
- Drive parity with a direct test-vector port of the upstream `sanitize-url` suite (v7.1.1),
  ensuring we match decoding and invalid-protocol handling.

## Consequences

- `utils::format_url` now matches Mermaid's dependency behavior more closely (including some URL
  normalization, e.g. `https://example.com` -> `https://example.com/`).
- URL sanitization parity is no longer "best effort"; it is regression-tested against the
  dependency's own vectors.

## References

- Mermaid implementation: `repo-ref/mermaid/packages/mermaid/src/utils.ts`
- Mermaid formatting tests: `repo-ref/mermaid/packages/mermaid/src/utils.spec.ts`
- Dependency reference (pinned by Mermaid lockfile): `repo-ref/mermaid/pnpm-lock.yaml`
- Dependency source and tests:
  - `repo-ref/sanitize-url/src/index.ts`
  - `repo-ref/sanitize-url/src/__tests__/index.test.ts`
