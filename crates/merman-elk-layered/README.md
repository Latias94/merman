# merman-elk-layered

`merman-elk-layered` is the source-backed Eclipse ELK layered layout port used by
`merman-layout-elk`.

This crate is intentionally separate from the rest of the workspace because the
Eclipse ELK sources are licensed under EPL-2.0. Source-port work in this crate
must preserve upstream source references and keep algorithm translations inside
this EPL-2.0 boundary.

Current source baseline:

- Mermaid adapter: `repo-ref/mermaid/packages/mermaid-layout-elk/src/render.ts`
- elkjs: `repo-ref/elkjs` tag `0.9.3`
- Eclipse ELK: `repo-ref/elk` tag `v0.9.1`

The initial implementation exposes the layered graph, option model, and
processor assembly scaffold. Layout phases are ported incrementally from the
pinned Eclipse ELK sources rather than approximated from fixture output.
