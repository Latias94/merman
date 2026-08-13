# Pinned Cypress collector

This upgrade-only collector executes selected Mermaid Cypress spec modules through the `esbuild`
version installed by Mermaid's pinned `pnpm-lock.yaml`. It replaces only the imported render-helper
module and provides a strict test-registration host. It does not parse JavaScript syntax or emulate
JavaScript expressions.

Run it from the repository root with Mermaid's exact Node and pnpm versions after installing the
pinned checkout with lifecycle scripts disabled:

```bash
cd repo-ref/mermaid
npx --yes --package=node@22.14.0 --package=pnpm@10.30.3 -- \
  pnpm install --frozen-lockfile --ignore-scripts
cd ../..

npx --yes --package=node@22.14.0 --package=pnpm@10.30.3 -- \
  node tools/upstreams/cypress-collector/collect.mjs \
  --scope new-family \
  --output target/upstream-cypress-new-family.json

npx --yes --package=node@22.14.0 --package=pnpm@10.30.3 -- \
  node tools/upstreams/cypress-collector/collect.mjs \
  --scope flowchart-elk \
  --output target/upstream-cypress-flowchart-elk.json
```

After reviewing the observations, project them into the committed manifests. The migration
equivalence check has been completed; standing checks read committed evidence and do not execute
upstream JavaScript.

```bash
cargo run -p xtask -- project-upstream-cypress-collection \
  --scope new-family \
  --input target/upstream-cypress-new-family.json \
  --refresh

cargo run -p xtask -- project-upstream-cypress-collection \
  --scope flowchart-elk \
  --input target/upstream-cypress-flowchart-elk.json \
  --refresh
```

The collector fails on an unpinned toolchain, a dirty or wrong source checkout, a changed Cypress
test pattern, missing scope specs, unsupported imports or helpers, unreviewed runtime effects,
timeouts, skipped-registration drift, or a changed call count. The generated observations are
temporary inputs to `xtask`; committed scope manifests contain only reviewed identities and
digests.
