# merman-render-sequence

Layout types, participant drawing, prepared control frames, and activation rendering for Mermaid
Sequence diagrams. Activation state, bounds, palette slots, and complete SVG groups belong to this
crate. `merman-render` orchestrates layout, text and math measurement, and the final SVG document
using the same operation budget across both crates.

Use `merman` for source-to-output rendering.
