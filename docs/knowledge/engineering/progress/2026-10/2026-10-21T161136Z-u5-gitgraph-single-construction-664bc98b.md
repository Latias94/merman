---
type: Work Progress
title: GitGraph canonical construction and oracle-backed seeded identity
description: Remove obsolete semantic warmup and discarded random work while preserving commands, editor capture and source diagnostics.
timestamp: 2026-10-21T16:11:36Z
record_id: 664bc98bf2fd41d5b76c5456d1f1c353
producer_id: codex-root
run_id: semantic-capability-boundaries-u5
related_plan: docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md
git_branch: refactor/semantic-capability-boundaries
git_commit: bc7e779b8c462403c473682f6afd50f0ec714b29
---

# Implementation

GitGraph now exposes canonical commits and branches without requiring presentation.
The full typed names are presentation-gated aliases of the same graph; there is
no computed family graph to duplicate. Consume parsed commands into one DB, then
consume its commits into the ordered result. Remove the unused commit-order copy,
the second semantic warmup and the initial discarded random draw.

Ordinary semantic, full typed and compatibility requests omit editor observation.
Combined requests capture facts in the same syntax construction and retain their
failure/recovery behavior. Common-field sanitization, commit sequence ordering,
stable branch ordering, explicit configuration and cancellation remain at their
existing owners. Seven new family tests cover these demands and identity cases;
the two old seed assertions are deliberately corrected against the actual oracle.

# Mermaid Oracle

The independent worker executed
`node target/agent-patches/u5-gitgraph-oracle.mjs` against Mermaid 12.2.0,
selected source `1fb7869866dd619168d840291c9c54b4f129758b`.
Node 24.21.0, Puppeteer 25.6.0, esbuild 0.28.1 and
HeadlessChrome 151.0.7922.77 produced 19 actual observations. The reference runtime
was rebuilt in memory using the current builder inputs and matched the existing
artifact bytes exactly.

- Installed content SHA256: `428bc8e4372395ef8c7d3c35c0e9845fe016e7c4cba6f1e442d752251232dd87`.
- Bundle SHA256: `bf48f8fa5b5bff18a9b86d23af7ba9256b2fc712024682a20a48982abae50c52`.
- Decision receipt SHA256: `016fa3c402fb7b69823d17115a1e1808060f537f33dc98e19c417c6d7e49efa6`.
- Runtime artifact SHA256: `a6ba3f89fc1442ea9b66f3274e2e35d8122af1f99f723b144b6f4a374bab968b`.
- Observations JSON SHA256: `5dedbbcd6e610daab0542ee9096cfb342dd96de0aafff14c36e7beb090bec59c`.

Cold render, actual grammar/DB parsing and an observed render agree: seed 1
starts at `0-4ab40cd`. The observed request performed one parser call and one
DB clear. There were zero random draws before the request and seven draws for
the first automatic ID, producing digits `4 a b 4 0 c d`.

| Source case | Observed outcome |
| --- | --- |
| One automatic commit | `0-4ab40cd`; grammar and both render lanes agree |
| Three automatic commits | `0-4ab40cd`, `1-a5b722b`, `2-da218e7` |
| TB direction, one automatic commit | Same first ID |
| ROOT, feature automatic commit, cherry-pick `1-4ab40cd` | Accept; new commit `2-a5b722b`, parents ROOT and source |
| Same graph, cherry-pick provisional `1-ab40cda` | Reject missing source |
| Automatic root, F1, merge custom `0-5b722bd` | Accept; that custom ID does not collide |
| Same graph, merge custom `0-4ab40cd` | Reject actual generated-ID collision |
| Automatic root, second commit custom `0-4ab40cd` | Overwrite into one commit, sequence 1 and self parent |
| Authored `targetId` argument | Grammar rejection, zero random draws |

The ignored experiment is not a new production oracle framework. The source
cases and identities above distinguish actual pinned behavior from the obsolete
warmup assumption and the rejected provisional offset.

# Red And Green Evidence

Before changing production code, strengthen the existing two seeded-ID tests.
Nextest `54b39f08-144e-4850-95bd-4187a86ee576` failed both: the old code
returned `0-5b722bd` instead of the observed `0-4ab40cd`.

An additional original-source assertion in the duplicate warning test then
exposed a separate existing omission: nextest
`7ba716cc-2e5e-4b13-8e7f-a4f14f1b42a1` passed 2,144 tests and failed that
assertion because the warning span was absent. Pass the command's existing
statement span into its single DB commit call and attach it at warning creation.
Both lean and full facades reuse the shared source map; BOM, frontmatter and
CRLF offsets now point at the original duplicate statement. Messages and rule IDs
remain unchanged. Focused green: `5ae3bca6-1c9d-4131-b280-80e36fb442d0`.

| Final owner selection | Result | Nextest run ID |
| --- | --- | --- |
| GitGraph-only lean core library | 238 passed | `4953ff1b-e9ab-4c09-bedc-cab19e7f94b3` |
| Full core library, projection and feature contracts | 2,145 passed | `174e4a3c-8525-4af0-bf97-4a61070d4f9b` |
| GitGraph-only renderer library and GitGraph SVG | 1,368 passed, 1 skipped | `58141d5a-5e8c-4ee1-a7bc-0a555596f554` |
| Analysis warning configuration and candidate projection corpus | 3 passed, 249 outside filter | `a527f147-0234-4b4f-9fa9-a68d433e4b1e` |

Production full-core Clippy with `-D warnings`, `cargo fmt --all -- --check`
and whitespace checks passed. All Cargo runs were serialized with two build jobs
and two test threads in the existing F-drive checkout and target. An independent
read-only review confirmed the warning repair and found no other concrete issue
in the five-file source slice. It did not rerun Cargo or the oracle.

# Scope

There are now 33 canonical selectors; Class and actual editor compile isolation
remain in U5. The Class proposals are still unapplied at this checkpoint. U6-U9
consumer, installed package, strict parity and comparable final-byte requirements
remain active. No speed, allocation or final-size claim is made from construction
counters alone. No push, PR, publication or release was performed.
