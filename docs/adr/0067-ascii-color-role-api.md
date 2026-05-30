# ADR 0067: ASCII Color Role API

- Status: accepted
- Date: 2026-05-30

## Context

`merman-ascii` currently emits deterministic plain ASCII/Unicode strings. That default is valuable:
snapshots are stable, output works in logs and documentation, and rendering has no dependency on
terminal capabilities. However, the renderer now has enough diagram coverage that color has become
the next enabling layer for:

- terminal themes and richer CLI previews;
- HTML text previews without SVG;
- XYChart series differentiation;
- eventual Mermaid `classDef`, inline `style`, and `linkStyle` semantics;
- parity work informed by `repo-ref/beautiful-mermaid`, which uses a role canvas plus ANSI/HTML
  finalization.

The current `AsciiRenderOptions` is a public `Copy` struct with public fields. Adding color fields is
a public API change for callers that construct the struct with literals. Because `merman-ascii` is
still pre-1.0, this is the right time to harden the options API instead of accumulating ad hoc color
switches in renderer-specific code.

## Decision

Add an opt-in foreground color role API to `merman-ascii`.

1. Keep default output plain.
   - `AsciiColorMode::Plain` is the default.
   - Existing render calls with default options must remain byte-for-byte identical.
   - ANSI/HTML escape sequences are inserted only after layout, routing, wrapping, and sizing are
     finished.

2. Add public color types in a new `color` module and re-export them from `merman-ascii`.
   - `AsciiColorMode`: `Plain`, `Auto`, `Ansi16`, `Ansi256`, `TrueColor`, and `Html`.
   - `AsciiRgb`: compact `Copy` RGB value with `new(r, g, b)` and `from_hex24(0xRRGGBB)`.
   - `AsciiColorRole`: non-exhaustive semantic roles such as text, muted text, node border, group
     border, edge line, edge arrow, edge label, junction, sequence lifeline, sequence activation,
     sequence frame, chart axis, and chart series.
   - `AsciiColorTheme`: private-field `Copy` theme with `default_light`, `default_dark`,
     `color_for`, and `with_role`.

3. Harden `AsciiRenderOptions` during this pre-1.0 API change.
   - Add `color_mode: AsciiColorMode` and `color_theme: AsciiColorTheme`.
   - Keep `AsciiRenderOptions` `Copy` by keeping all color types `Copy`.
   - Add builder-style methods such as `with_color_mode` and `with_color_theme`.
   - Mark `AsciiRenderOptions` `#[non_exhaustive]` during the same change so future option fields do
     not force another struct-literal break.
   - Continue supporting field mutation after `Default`, `ascii()`, or `unicode()` construction.

4. Use a role-aware canvas internally.
   - Store either a parallel role buffer or role-bearing cells.
   - Plain `set` and `write_text` remain available and write no role.
   - New role-aware methods write semantic color roles.
   - Layout modules stay role-agnostic; renderers assign roles only while drawing.
   - Finalization groups consecutive same-role text runs before emitting ANSI or HTML.

5. Defer background/fill colors.
   - The first API controls foreground color only.
   - Mermaid `fill` and terminal background colors affect spaces, trimming, and output semantics, so
     they need a separate decision.

6. Defer Mermaid style mapping until the role infrastructure is proven.
   - The first implementation slice should color renderer-owned roles.
   - Later slices may map flowchart `classDef`, inline `style`, and `linkStyle` from typed model
     style strings into direct colors or role overrides.
   - Unsupported CSS properties must remain explicit diagnostics or documented no-ops.

## Initial API Shape

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    pub const fn default_light() -> Self;
    pub const fn default_dark() -> Self;
    pub fn color_for(&self, role: AsciiColorRole) -> AsciiRgb;
    pub fn with_role(self, role: AsciiColorRole, color: AsciiRgb) -> Self;
}
```

Usage:

```rust
let options = AsciiRenderOptions::unicode()
    .with_color_mode(AsciiColorMode::TrueColor)
    .with_color_theme(
        AsciiColorTheme::default_dark()
            .with_role(AsciiColorRole::EdgeArrow, AsciiRgb::from_hex24(0x7aa2f7)),
    );
```

## Auto Mode

`Auto` is explicit, not the default. It may inspect environment and terminal capability, including
common signals such as `NO_COLOR`, `CLICOLOR_FORCE`, `COLORTERM`, `TERM`, and whether stdout is a
terminal. Tests should not snapshot `Auto`; they should force `Ansi16`, `Ansi256`, `TrueColor`, or
`Html`.

## Alternatives

1. Emit ANSI directly inside each renderer.
   - Pros: fastest small patch.
   - Cons: escape sequences would leak into width calculations and renderer logic; HTML output would
     duplicate escaping rules.

2. Copy `beautiful-mermaid`'s TypeScript API exactly.
   - Pros: known reference and familiar mode names.
   - Cons: Rust should preserve `Copy` options and typed builders; the reference parser/style model
     is not the `merman-ascii` boundary.

3. Make colors style-string based instead of role based.
   - Pros: closer to Mermaid CSS.
   - Cons: forces CSS parsing before the renderer has a stable terminal color substrate.

4. Add background/fill colors in v1.
   - Pros: closer to Mermaid node fill semantics.
   - Cons: terminal backgrounds turn spaces into visible output and change trimming/snapshot
     behavior. Foreground roles are the safer first layer.

5. Avoid changing `AsciiRenderOptions` by adding separate colored render functions.
   - Pros: avoids an immediate struct-field break.
   - Cons: creates parallel public APIs and makes color composition with existing options awkward.
     Pre-1.0 is the right moment to harden the options struct.

## Consequences

- Default plain output remains stable and semver-sensitive.
- The color API becomes one shared substrate for flowchart, sequence, class, ER, and XYChart.
- `AsciiRenderOptions` gets one intentional pre-1.0 breaking change and is hardened against future
  field additions.
- Renderer code will need a disciplined role assignment pass, starting with one flowchart slice.
- Mermaid style/class/linkStyle parity is unlocked but not automatically implemented by this ADR.
- Background/fill semantics remain a separate follow-on.

## Follow-up

Continue `docs/workstreams/ascii-color-role-api` with the role-aware canvas and encoder slice before
assigning color roles to individual diagram renderers.
