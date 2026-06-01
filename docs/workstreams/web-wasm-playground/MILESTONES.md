# Web WASM Playground - Milestones

Status: Active
Last updated: 2026-06-01

## M0 - Scope And Evidence Freeze

Exit criteria:

- Problem and target state are explicit.
- Non-goals are explicit.
- Relevant ADRs/docs/reference repos are linked.
- First proof target is chosen.

Primary evidence:

- `docs/workstreams/web-wasm-playground/DESIGN.md`
- `docs/workstreams/web-wasm-playground/TODO.md`

Status: complete. First implementation slice is `WWP-020`.

## M1 - Formal WASM Crate

Exit criteria:

- `crates/merman-wasm` exists as a workspace member.
- It exposes transport-only WASM functions over `merman-bindings-core`.
- `wasm32-unknown-unknown` compile blockers are resolved or precisely documented.
- `wasm-pack build` produces a web package.

Primary gates:

- `cargo check -p merman-wasm --target wasm32-unknown-unknown`
- `wasm-pack build crates/merman-wasm --target web --out-dir ../../target/merman-wasm-pkg`

Status: complete. `crates/merman-wasm` builds for host and wasm targets, and wasm-pack emits a web
package.

## M2 - TypeScript Web Package

Exit criteria:

- `platforms/web` builds generated WASM and TypeScript declarations.
- Public TS helpers initialize WASM once and serialize typed options to the shared JSON contract.
- Generated artifacts are handled intentionally.

Primary gate:

- `npm run build --prefix platforms/web`

Status: complete. `platforms/web` builds the generated WASM package and TypeScript declarations,
and `npm pack --dry-run` includes the expected static WASM artifacts.

## M3 - Playground Integration

Exit criteria:

- `playground` builds as a static Vite app.
- The live editor renders through real WASM in the normal path.
- Mock or fallback behavior is explicit and not mistaken for production rendering.

Primary gates:

- `npm run build --prefix playground`
- Browser smoke or screenshot evidence after local preview.

Status: complete. `playground` builds as a Vite static app, imports `@merman/web`, and the browser
smoke confirms the default editor flowchart renders through the generated WASM package.

## M4 - GitHub Pages Build

Exit criteria:

- A Pages workflow builds Rust WASM and the static app.
- A verifier fails when `.wasm` or generated JS shim is absent from dist.
- Deployment configuration assumptions are documented.

Primary evidence:

- `.github/workflows/pages.yml`
- dist verifier output

Status: complete. The Pages workflow builds `platforms/web`, builds `playground`, verifies the
static dist contains the generated WASM binary and JS shim, and uploads `playground/dist` for
GitHub Pages deployment.

## M5 - Closeout

Exit criteria:

- Gate set is recorded.
- Remaining work is either completed, deferred, or split into follow-ons.
- `WORKSTREAM.json` status is updated.
