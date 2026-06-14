# Flowchart ELK Layout - Handoff

Status: Active
Last updated: 2026-06-14

## Current State

Flowchart ELK is no longer a binary "supported / unsupported" question. The codebase already has a
lightweight renderable subset, and the remaining question is how far the upstream `flowchart-elk`
fixture surface should go before a full ELK port becomes worthwhile.

## Decision So Far

- Do not start with a full ELK port.
- Admit smoke cases first.
- Use nested subgraph, direction, and ordering-heavy fixtures to decide whether subset growth is
  enough.
- Only split to a deeper ELK dependency boundary if the evidence says the subset cannot cover the
  useful cases.

## Next Recommended Action

Classify `repo-ref/mermaid/cypress/integration/rendering/flowchart/flowchart-elk.spec.js` into the
three tiers in `DESIGN.md`, then pick the first Tier A batch for targeted parity work.
