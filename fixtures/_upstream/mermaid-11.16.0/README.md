# Mermaid 11.16 Added MMD Corpus

This directory preserves the `.mmd` files added between Mermaid `11.15.0` and `11.16.0`:

- source ref: `mermaid@11.15.0..mermaid@11.16.0`
- source commit: `7c0cafcf42e76bfaf79d0cbbd12edb986612f014`
- selection: files added by the range with the `*.mmd` pathspec

`_manifest.json` records every upstream source path and its SHA-256 digest. Every source file is
copied verbatim under `sources/` with its upstream path preserved, including duplicate contents.
Content hashes are still used to report the number of unique inputs, but never to omit a source
path from the versioned corpus.

This directory is evidence, not automatic parity admission. Its leading underscore keeps the raw
corpus out of fixture sweeps that assume every input is already supported. Family-specific tests
and SVG baselines promote source-backed cases into the normal fixture directories.

Synchronize or verify the corpus with:

```sh
cargo run -p xtask -- sync-upstream-mmd-corpus \
  --from mermaid@11.15.0 \
  --to mermaid@11.16.0

cargo run -p xtask -- sync-upstream-mmd-corpus \
  --from mermaid@11.15.0 \
  --to mermaid@11.16.0 \
  --check
```
