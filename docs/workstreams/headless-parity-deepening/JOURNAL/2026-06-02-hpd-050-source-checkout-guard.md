# HPD-050 Source Checkout Guard

Date: 2026-06-02

## Finding

The local `repo-ref/mermaid` checkout is not currently at the pinned baseline:

- `git -C repo-ref/mermaid rev-parse HEAD` => `9bae92cd3214f9ec99369ab314ef41ffb283f6b6`
- `git -C repo-ref/mermaid status --short --branch` => `develop...origin/develop`
- `tools/upstreams/REPOS.lock.json` pins Mermaid to
  `41646dfd43ac83f001b03c70605feb036afae46d` (`mermaid@11.15.0`)

This explains the earlier false lead around a later Architecture `withSeededRandom` source path:
that path exists in the current checkout but not in the locked baseline source or the installed
`mermaid@11.15.0` dist used by upstream SVG generation.

## Rule

For source-backed parity claims, use one of:

- `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:<path>`
- `tools/mermaid-cli/node_modules/mermaid/dist/mermaid.js`
- fresh `xtask check-upstream-svgs` output

Do not treat `repo-ref/mermaid/<path>` as baseline truth until the checkout has been verified
against `tools/upstreams/REPOS.lock.json`.

## Evidence

`git show` on the locked Architecture renderer confirmed the current baseline has
`gap: 1.5 * db.getConfigField('iconSize')`, reads the shipped FCoSE config fields, and does not
contain `withSeededRandom`.
