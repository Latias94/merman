# ADR 0068: Ordered Theme Resolution And Render Presentation Views

- Status: accepted
- Date: 2026-06-03
- Last amended: 2026-07-20

## Context

Mermaid theme compatibility has two related ownership problems:

- theme construction is an ordered, mutable upstream process, not a flat JSON merge;
- family renderers need semantic presentation roles rather than repeated raw-path fallback chains.

The earlier render-side theme view solved the second problem but described core theme access as a
single expansion step. That model was incomplete. In Mermaid, user variables are applied around
derived-color calculation and explicit values are replayed afterward. A font-only override exposed
the distinction: a flat recalculation changed `cScale*` values for Radar, Kanban, Mindmap, and
Timeline even though font choice is not a color input.

Theme evidence also has three different artifact contracts. Raw SVG may preserve an exact upstream
token that a browser resolves through CSS inheritance; computed browser presentation is the visible
contract; resvg-safe export must replace invalid browser-only tokens with explicit valid values.
Collapsing those lanes either breaks raw parity or hides a visible defect.

Color handling has a related but distinct boundary problem. Mermaid theme construction uses Khroma
semantics, while browser CSSOM serialization, grammar-level color whitelists, and RoughJS adapters
have different accepted inputs and outputs. Sharing parsing and math is useful only where those
protocols agree; treating every color-shaped string as one abstraction would silently widen security
boundaries or change emitted SVG.

## Decision

1. `merman-core` owns an explicit ordered theme resolution pipeline:
   - `DefaultSnapshot`: the generated result for the selected public theme;
   - `OverridesApplied`: site and source values overlaid with their provenance;
   - `Calculated`: Mermaid-compatible derived-color calculation;
   - `ExplicitReplay`: user values replayed over the calculated snapshot.

   A `ThemeProgram` descriptor selects the pure-Rust evaluator and declares evaluated color inputs
   for each public theme. Cross-field operations such as scale peers, surfaces, ER rows, and Git
   palettes execute as one dependency graph between `Calculated` completion and `ExplicitReplay`;
   they are not distributed across family renderers or repaired after rendering.

2. The final immutable `effective_config.themeVariables` is the only palette input to family
   renderers. A family may not independently derive shared scale, peer, inverse, label, or surface
   colors.

3. `merman-core::theme_color` is the single implementation of the pinned Khroma parse, channel,
   transform, and serialization semantics used during theme construction. Invalid color inputs are
   typed errors; theme calculation may not replace them with guessed colors or unchanged strings.

4. Browser CSSOM serialization, diagram grammar validation, and RoughJS conversion remain explicit
   adapters with their own contracts. They may reuse the shared parser where their accepted language
   is identical, but they may not silently broaden or narrow their protocol. In particular,
   Railroad's CSS whitelist and the hex-only RoughJS boundary are not Khroma theme operations.

5. `merman-render` exposes `PresentationTheme` and focused family views. They convert resolved
   tokens into typography, surfaces, borders, lines, notes, labels, and diagram-specific roles.
   Direct raw JSON access is reserved for exact Mermaid tokens that cannot be represented by an
   existing role; repeated fallback logic should deepen the shared view instead. Raw value accessors
   use `theme_token` terminology so they cannot be confused with color evaluation.

6. Theme changes are verified in separate lanes:
   - a generated artifact records the pinned Mermaid package hash, source tag, source commit,
     complete default snapshots, complete `darkMode=true` snapshots, and a compact override-value
     oracle;
   - raw theme snapshots and raw SVG assert exact selected-release tokens;
   - Chromium tests inspect computed styles for visible behavior;
   - resvg-safe tests assert XML-safe, rasterizable, explicit values.

   `cargo run -p xtask -- gen-theme-snapshot` is the only supported refresh path. It executes the
   content-pinned Mermaid runtime, and `verify-theme-snapshot` plus the umbrella
   `verify-generated` command reject provenance or behavior drift.

7. Host or product styling remains outside parity rendering. Host theme profiles and postprocessors
   may map product roles, but they do not mutate Mermaid's resolved theme state or redefine family
   semantics.

## Consequences

- Override order and value provenance are testable instead of implicit in mutation order.
- Release snapshots and executable semantics have separate ownership: generated JSON supplies
  exact constants and oracle evidence, while Rust owns runtime evaluation without JavaScript.
- Font-only and partial overrides preserve upstream palettes across all consumers.
- Duplicate RGB/HSL engines and family-local palette heuristics are deleted rather than kept as
  compatibility fallbacks.
- Browser inheritance and export fallbacks can differ intentionally without comparator
  normalization or family-specific patches.
- Renderer views remain deep only when they remove shared policy or meaningful duplication; layout
  constants and truly family-local semantics stay with the family.

## Rejected Alternatives

1. Merge JSON once and let every family derive colors.
   This cannot reproduce Mermaid's update-and-replay ordering and causes repeated switches.
2. Patch the four affected families.
   The defect belongs to shared theme construction and would recur in new scale consumers.
3. Normalize raw invalid colors to browser RGB in the comparator.
   That changes semantic evidence and hides the difference between source and presentation.
4. Solve parity through `themeCSS` or host postprocessing.
   Those stages are too late for layout and renderer-owned presentation semantics.
5. Copy an unrelated role-based theme system wholesale.
   Merman must retain Mermaid-compatible config and token behavior while deepening its render view.
6. Route every color-shaped value through the Khroma implementation.
   CSSOM, grammar security, and RoughJS have different contracts; sharing a name would hide rather
   than remove those differences.
