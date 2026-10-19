# Fixture Versions and Provenance

Fixture input history and the current behavior baseline are separate facts.

- The selected Mermaid version and exact source commit are owned by
  [`MERMAID_REFERENCE_BUNDLE.json`](../tools/upstreams/MERMAID_REFERENCE_BUNDLE.json).
  Materialized sources under `repo-ref/` must match
  [`REPOS.lock.json`](../tools/upstreams/REPOS.lock.json).
- Each `upstream-svgs/<family>/_baseline-manifest.json` records the runtime that generated the
  reference SVGs, along with the input and output hashes. This is the reference output's version
  authority; a fixture comment cannot establish that an output was rendered or verified.
- Comments such as `Authored stress fixture (Mermaid@11.12.2)` record the version used when the
  input was originally authored. `Normalized to Mermaid@11.12.2 ... grammar` records an earlier
  input adaptation. Both remain historical when the selected runtime advances. Copied source
  URLs, tags, commits, and historical corpus manifests likewise retain their original identities.

For new or revised historical comments, use explicit wording such as
`Originally authored against Mermaid 11.12.2` or
`Originally normalized for the Mermaid 11.12.2 grammar`. Do not add a second current-version field
to every input. Read the selected bundle and the family manifest for the current reference version.
The independently versioned Tree-sitter package retains its own baseline in
[`metadata/provenance.json`](../distribution/tree-sitter-mermaid/metadata/provenance.json).

Keep release bookkeeping out of diagram content such as `accTitle`, `accDescr`, and labels unless
that text is itself the behavior under test. A historical input can exercise current Mermaid
behavior without including a version in its visible or accessible content.

Changing even a comment changes the input hash. Avoid bulk version replacements in historical
inputs. When an input does need editing, regenerate its semantic/layout snapshots as applicable
and its reference SVG through the existing `xtask` commands, then check alignment. Do not manually
replace manifest hashes or relabel existing SVGs as evidence of a different runtime.
