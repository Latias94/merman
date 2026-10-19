# Theme design skill verification

Date: 2026-10-19. Scope: `design-merman-theme`, its thin Node command, and the theme guide entry.
The visual workbench and Rust theme semantics were not changed in this iteration.

## Runtime and checks

- Node v24.21.0, current source Node engine, Options JSON 3, deterministic policy, SVG parity.
- The standard Node-WASM build completed Rust compilation but Binaryen's Precompute pass crashed.
  A separate local `wasm-pack --no-opt` build was used through the existing workspace engine and
  Node-WASM loader, including their normal transport identity and contract validation. It is not
  an assembled/release artifact. The skill's runtime reference documents this local alternative.
- `quick_validate.py` accepted the skill. Node syntax checking and all seven command regressions
  passed. Tests protect raw theme text, same-execution metadata, input/output aliases, fresh-file
  publication, structured error propagation, disposal, and malformed standalone JSON.
- Real Rust operations rejected duplicate theme keys, a missing reference, and an explicit null
  Options JSON version. A malformed standalone theme cannot append fields to the options envelope.
- Independent adversarial review found two input/publication defects; both were fixed, covered by
  regressions, and rechecked with no remaining findings.

## Independent forward trials

Three agents received realistic briefs, the skill, the matching engine, and separate temporary
output directories. They created local artifacts without modifying the implementation.

| Brief | Observed result |
| --- | --- |
| New warm paper-and-ink theme, restrained orange, hard shadows | Editable recipe with explicit shared values; Flowchart and Sequence previews. Supported-shape examples report `theme_status: verified`, no diagnostics, and `target_status: unverified`. Mixed-shape examples retain their residual/rejected evidence. |
| Muted teal Cyberpunk with half-strength glows | Real preset export; only the shared cyan and 17 shadow opacities changed. All 49 rules, three canvas layers, and reference relationships were preserved. Flowchart/Sequence preview evidence remains residual/rejected and is disclosed. |
| Consistent cool blue across Flowchart, Sequence, XY Chart while keeping Neo | Three previews preserve Neo. Requested typed hard shadow remains unapplied and is distinguished from Neo's built-in soft shadow. Residual/rejected execution evidence is retained. |

The parent independently compared the Cyberpunk recipe against the exact expected changes,
parsed all seven primary SVGs, checked Neo configuration, and rerendered those seven examples.
Each replay matched both the original SVG and its same-execution metadata exactly.

The trials exposed one instruction gap: `design.mermaid` supports only theme/dark-mode/scalar
variables, not arbitrary diagram configuration. The authoring and runtime references now put
`flowchart.look` in separate options/frontmatter and require those settings in the handoff.

## Limits and next decision

Visual inspection was unavailable: there were no usable browsers and the image tool rejected
image input. The design notes explicitly mark visual review pending. SVG/metadata checks establish
execution and reproducibility, not contrast, clipping, aesthetic quality, or PNG/PDF portability.

Trial JSON, sources, options, previews, metadata, and notes are local under
`.build/theme-skill/evals/{new,preset,family}/`; they are not committed preset additions.
These trials establish a usable authoring and validation workflow. They do not yet establish
that the skill can replace all visual editing needs. Evaluate the previews with users before
removing workbench controls or changing renderer support.
