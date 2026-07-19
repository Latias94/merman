# ZenUML Geometry Port

The Rust layout is a headless port of the selected oracle's DOM-free SVG path. Constants and
algorithms must remain traceable to the oracle; they are not fixture tuning knobs.

| Rust surface | Oracle source | Port rule |
| --- | --- | --- |
| participant visual dimensions | `src/svg/svgConstants.ts`, `src/svg/buildParticipantGeometry.ts` | label measurement plus icon/emoji/stereotype overhead, bounded by the source min/max |
| participant spacing | `src/positioning/Coordinates.ts` | adjacent half-width gaps, then message-width constraints |
| lifelines | `src/svg/buildParticipantGeometry.ts` | participant bottom to computed diagram bottom, dashed stroke |
| message/creation heights | `src/positioning/Constants.ts`, statement VM classes | measured labels and source statement height classes |
| occurrence bars | `src/svg/buildStatementGeometry.ts` | target lifeline center and nested statement extent |
| fragments | `src/svg/buildFragmentGeometry.ts` | local participant bounds, frame padding, section separators |
| root frame | `src/svg/renderToSvg.ts` | 1px frame, 28px header, 10px content padding |
| SVG primitives | `src/svg/components/*.ts` | escaped text/attributes and source arrow direction |

Text widths always come from the operation-owned `TextMeasurer`. No character-count or
fixture-specific width is used for layout decisions. The current port intentionally records
browser-dependent residuals (Canvas/DOM font shaping, icon glyph metrics, and CSS baseline
rounding) instead of pretending they are exact in a resvg environment. Browser computed-behavior
tests own those residuals.

Vertical placement ports `BlockVM`'s 56px root origin and 16px statement margins, then applies the
selected statement VM formulas independently for synchronous/asynchronous self calls, creation,
empty/body occurrences, assignment returns, fragments, dividers, and comments. A final non-self
return inside an occurrence retains the oracle's collapsed-coordinate distinction while restoring
the 16px SVG return debt. Nested occurrence endpoints use the oracle's 7px bar-side offset.

When the candidate Core version is admitted, this table and the U1 delta inventory must be updated
from the candidate's corresponding `src/svg` and positioning sources before changing behavior.
