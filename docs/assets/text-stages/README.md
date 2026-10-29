# Text terminal comparison

Open `index.html` for a self-contained comparison. `comparison.png` is captured from that HTML
in Edge 156 at a 1160px viewport. The exact inputs and before/after SVGs are retained here.
`comparison.json` binds their SHA-256 hashes. `wasm-inputs.json` retains the original
full-WASM build manifest without rewriting it.

- Kanban assigned metadata: `<br/>` previously appeared as literal markup despite being measured
  as a break. The prepared XHTML now supplies both layout and the visible two-line terminal.
- Class note: `&nbsp;` previously remained an undefined XML entity, preventing the SVG from loading
  as an image. XHTML serialization now produces valid XML with the intended nonbreaking space.
  The before panel records the actual XML parse failure; it is not a substituted rendered diagram.

The baseline CLI was built from the merged source used by main commit `0ce21f1cc`; its source
matches that merge. The after SVGs come from this branch's freshly built full browser WASM,
using its deterministic fallback measurement without a host callback. Both sides declare the
same Arial font in the source for repeatable label geometry. No screenshots from an
older WASM artifact are presented as current-source evidence.

After capture, the Rust protocol expectation gained `(22, "length")` inside
`#[cfg(test)]`, and the separate ELK browser-measurement test module gained the
8368 regression. Those test-only differences are recorded in `comparison.json`;
the capture does not claim an exact final-tree input digest. The renderer and
host-operation implementation used by these examples match the captured build.
