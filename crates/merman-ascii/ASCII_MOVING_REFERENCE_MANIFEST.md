# ASCII Moving Reference Manifest

Status: discovery evidence, not a byte oracle

Last updated: 2026-08-15

The machine-readable authority for moving-reference fixture dispositions is
`tests/testdata/mermaid-ascii/MOVING_REFERENCE_DISPOSITIONS.tsv`. This document intentionally does
not duplicate its 140 rows.

## Scope

The manifest records the fixture delta discovered after the immutable v1 copy. A local
`repo-ref/` checkout may inform development, but it is not a release dependency and its output is
not a Merman byte oracle.

The authority file pins:

- the moving `mermaid-ascii` reference revision;
- the immutable copied baseline;
- the `beautiful-mermaid` capability-prior-art revision;
- the Mermaid validity version;
- one classification, admission disposition, semantic feature, and path for each moving-only
  fixture.

## Dispositions

- `mermaid_valid`: pinned Mermaid accepts the input.
- `mixed_valid_private_behavior`: Mermaid accepts the input, but the moving reference assigns at
  least one construct a different private meaning.
- `reference_private`: the moving reference accepts syntax rejected by pinned Mermaid; the fixture
  is retained only to prevent accidental admission.
- `semantic_probe`: parser-backed product tests own equivalent behavior; reference output is not an
  oracle.
- `discovery_only`: useful research input that authorizes no support claim.

Tests validate the TSV schema, pinned revisions, path uniqueness, classification/admission pairs,
semantic-feature presence, and aggregate family counts. They do not parse this prose or inspect
private Rust test names.

## Update Rule

When the moving reference changes, update the TSV with only the new delta and pin the new revision.
A fixture may move from `discovery_only` to `semantic_probe` only when tracked parser-backed product
tests own its semantic feature. Do not copy or rewrite reference output merely to make the manifest
green.
