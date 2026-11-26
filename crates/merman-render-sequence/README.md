# merman-render-sequence

Sequence preparation and layout for Mermaid diagrams, including typography, text and math
measurement, participant drawing, control frames, and activation rendering. The facade supplies
the original operation context and owns the complete SVG and terminal output checks. Preparation
and emission use the same operation budget. Select `math` to include the shared RaTeX backend.

Use `merman` for source-to-output rendering.
