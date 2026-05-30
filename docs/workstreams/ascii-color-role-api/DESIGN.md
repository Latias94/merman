# ASCII Color Role API

Status: Active
Last updated: 2026-05-30

## Why This Lane Exists

`merman-ascii` intentionally emits plain ASCII/Unicode today. That keeps snapshots stable, but it
also blocks Mermaid style/class semantics, colored terminal output, HTML previews, and richer
XYChart series output. The next step should be a color role API, not ad hoc ANSI escape insertion in
individual renderers.

## Relevant Authority

- `docs/adr/0065-ascii-output-boundary.md`
- `docs/adr/0067-ascii-color-role-api.md`
- `docs/adr/0014-upstream-parity-policy.md`
- `crates/merman-ascii/FLOWCHART_SUPPORT.md`
- `crates/merman-ascii/SEQUENCE_SUPPORT.md`
- `docs/workstreams/ascii-reference-implementation-expansion/HANDOFF.md`
- `repo-ref/beautiful-mermaid/src/ascii/ansi.ts`
- `repo-ref/beautiful-mermaid/src/ascii/canvas.ts`
- `repo-ref/beautiful-mermaid/src/ascii/types.ts`

## Problem

Color needs to work across diagram families without weakening the text renderer's product contract:

- default output must remain byte-for-byte plain text;
- color must not affect layout width, wrapping, routing, or safety limits;
- role assignment must be semantic enough for future Mermaid `classDef`, `style`, and `linkStyle`;
- ANSI/HTML escaping must be centralized and testable;
- `AsciiRenderOptions` is already a public struct with public fields, so adding color is a public
  API change that should be decided deliberately.

## Target State

Callers can opt into colored ASCII/Unicode output through `AsciiRenderOptions`, with plain output as
the default. Renderers write characters with semantic color roles into a role-aware canvas. The final
encoder converts runs of equal roles into ANSI 16-color, ANSI 256-color, truecolor, or HTML `<span>`
output after layout is complete.

## Public API Sketch

```rust
#[non_exhaustive]
pub enum AsciiColorMode {
    Plain,
    Auto,
    Ansi16,
    Ansi256,
    TrueColor,
    Html,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsciiRgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl AsciiRgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self;
    pub const fn from_hex24(rgb: u32) -> Self;
}

#[non_exhaustive]
pub enum AsciiColorRole {
    Text,
    MutedText,
    NodeBorder,
    GroupBorder,
    EdgeLine,
    EdgeArrow,
    EdgeLabel,
    Junction,
    SequenceLifeline,
    SequenceActivation,
    SequenceFrame,
    ChartAxis,
    ChartSeries(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsciiColorTheme {
    /* private fields */
}

impl AsciiColorTheme {
    pub fn default_light() -> Self;
    pub fn default_dark() -> Self;
    pub fn color_for(&self, role: AsciiColorRole) -> AsciiRgb;
    pub fn with_role(self, role: AsciiColorRole, color: AsciiRgb) -> Self;
}

impl AsciiRenderOptions {
    pub fn with_color_mode(self, mode: AsciiColorMode) -> Self;
    pub fn with_color_theme(self, theme: AsciiColorTheme) -> Self;
}
```

Example:

```rust
let options = AsciiRenderOptions::unicode()
    .with_color_mode(AsciiColorMode::TrueColor)
    .with_color_theme(
        AsciiColorTheme::default_dark()
            .with_role(AsciiColorRole::EdgeArrow, AsciiRgb::from_hex24(0x7aa2f7)),
    );
```

## API Decisions

1. Default mode is `Plain`. It must produce the exact same output as today.
2. `Auto` is opt-in because environment detection is nondeterministic. Library tests should force a
   concrete mode.
3. The first API is foreground-color only. Background/fill color is a follow-on because it changes
   trailing-space, trimming, and terminal background expectations.
4. `AsciiColorRole` is non-exhaustive. Diagram families will need new roles over time.
5. `AsciiColorTheme` should keep private fields and builder methods. Avoid another public struct
   whose future fields become breaking changes.
6. ADR 0067 accepts one pre-1.0 `AsciiRenderOptions` migration: add `color_mode` and `color_theme`,
   keep the struct `Copy`, add builder methods, and mark the struct `#[non_exhaustive]` during the
   same change.

## Internal Architecture Direction

- Add a `color` module and re-export the public color types from `lib.rs`.
- Replace `Canvas { cells: Vec<char> }` with a role-aware representation or a parallel role buffer.
- Keep internal color tags flexible enough for future direct style colors:
  `Role(AsciiColorRole)` first, `Direct(AsciiRgb)` later.
- Add role-aware write APIs:
  - `set_role(x, y, ch, role)`
  - `write_text_role(x, y, text, role)`
  - plain `set`/`write_text` remain and write no role.
- Keep layout code role-agnostic. Renderers assign roles only at drawing time.
- Encode color in `Canvas::finish_with_options(&AsciiRenderOptions)` or an adjacent finalizer, never
  during measurement or routing.
- Group consecutive same-role spans per line before emitting ANSI or HTML.
- Escape HTML output centrally.

## Mermaid Style Mapping Direction

Do not map Mermaid styles in the first implementation slice. First ship theme roles for renderer
semantics. Then add style mapping as a follow-on:

- flowchart `classDef`, `class`, inline `style`, and `linkStyle` can resolve into direct foreground
  colors or role overrides;
- classDiagram and erDiagram already preserve style arrays in typed models and can consume the same
  style resolver later;
- unsupported style properties should remain explicit diagnostics or documented no-ops until mapped.

## In Scope

- public color mode, RGB, role, and theme API design;
- plain-by-default compatibility;
- role canvas / encoder architecture;
- first implementation plan for flowchart roles;
- future integration plan for sequence, class, ER, and XYChart.

## Out Of Scope

- implementing the API in this draft task;
- background/fill colors;
- full Mermaid style/class/linkStyle parity;
- SVG theming changes;
- parser changes;
- stateDiagram ASCII.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| `AsciiRenderOptions` public fields make color additions breaking. | External struct literals may fail to compile. | Decide via ADR before implementation; add builder methods and document preferred construction. |
| ANSI escapes corrupt width or snapshots. | Rendered diagrams become unstable. | Insert escapes only after final layout; keep `Plain` default and force modes in tests. |
| Role list is too specific too early. | Public enum churn. | Mark enum non-exhaustive and keep theme private. |
| Fill/background semantics are added too soon. | Trailing spaces and terminal backgrounds become product behavior. | Foreground-only v1; split fill/background into a later lane. |
| Style mapping duplicates Mermaid CSS parsing poorly. | Divergence from upstream style semantics. | Reuse typed model style strings, add a small sanitized style resolver, and port upstream cases. |

## Closeout Condition

This lane can close when:

- role-aware canvas and forced encoders are implemented without changing default output;
- flowchart role coverage ships as the first vertical slice;
- support docs describe the opt-in color boundary;
- and Mermaid style/class/linkStyle mapping is either implemented or split into a follow-on.
