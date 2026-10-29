# ELK endpoint measurement: fixture 8368

Scope: `flowchart/local_mermaid_12_2_elk_label_clearance_8368` on main.
No production algorithm, comparator, tolerance, or root viewport receipt changes.
The companion integration admits an exact browser-text residual for this fixture
in parity/parity-root only, after the controlled evidence below.

## Durable capture

Entry 46 in
[`flowchart-elk-browser-measurements.json`](../../fixtures/_verification/flowchart-elk-browser-measurements.json)
records a natural-browser run collected at `2026-10-28T14:23:05.753Z`:

- Browser: `Edg/156.0.4314.50`, Puppeteer `25.16.0`.
- Mermaid: `12.2.0`, source `1fb7869866dd619168d840291c9c54b4f129758b`;
  provider: actual `elkjs@0.9.3`.
- Input SHA-256: `e82ce307a88afea33da54324d9dfdfb674dfc7c35107dd162e71845ea1b8a497`.
- Upstream SVG SHA-256: `26b4dde86ef879321f362a35486a1980d13088ef5d71baa25a449000db7b2fd4`.
- Instrumented runtime SHA-256: `6b0cfa9a8bc36314e9ecc89f40bb6a08efc86ad813bfbfb3ce25b0fcce41ae19`.

Its `capture` object binds the browser, source modules/source map, provider bundle,
runtime, harness, rendered SVG, initialization and viewport. The matrix-level
browser describes the historical collection; it does not describe this new entry.
The previous 45 entries and matrix metadata retain their original values.

`provider_capture.input` and `.output` are copies immediately before and after
the real `elk.layout(elkGraph)` call. No provider output is synthesized from final
SVG paths. The root container origin is `(0,0)`; raw sections precede clipping,
terminal straightening and opposite-edge label separation. The capture preserves
the provider's zero-sized empty label; the Rust projection represents it as no
label, consistent with the existing adapter.

The natural lane returns native `getBBox()` results unchanged and injects no
fonts or dimensions. Capture follows the invocation boundary described in
[ADR0024](../adr/0024-dompurify-default-allowlists-and-generation.md).

## Observed behavior

| Measurement lane | A to B commands | B to A commands |
| --- | --- | --- |
| Natural browser | `MLLQLQLL` | `MLLQLQL` |
| Before deterministic dimensions, literal height 15.4 | `MLLQLQLL` | `MLLQLQL` |
| Current deterministic dimensions, literal height 17 | `MLLQLQLQL` | `MLQLQLQL` |

All lanes execute the complete pinned upstream layout/render pipeline. Natural
browser output matches upstream exactly for all three edge `d` strings and
decoded `data-points`, both edge-label centers and global background rectangles.
This is not a claim of byte-identical whole SVG output.

The controlled current lane fixes literal widths/heights and source-derived shape
measurement boxes, then runs real elkjs and upstream endpoint processing. Its
three-Q routes have exactly the same `data-points`, label centers and global
background rectangles as current Merman. Provider node geometry, raw routes and
label rectangles match the current layout golden at its three-decimal precision.
No node positions, routes or label centers are injected into this lane.

The raw provider point counts stay `7 / 2 / 6`. The added Q arises later:
`outsideNode` uses exact `abs(dx) >= width/2`. At the diamond's left port,
`abs(dx) - width/2` changes from `+1.4210854715202004e-14` before to
`-2.842170943040401e-14` current. The selected outside sample changes; departure-axis
attachment fails and the center-ray fallback creates a genuine diagonal.
That segment is `9.403077898807496px` long with a `33.91812257348938` degree turn.
It is neither collinear nor below the pinned marker-stub threshold.
Restoring browser shape-bbox precision while retaining literal height 17 restores
two Q. Restoring height 16 alone with deterministic widths does not.

Source: pinned Mermaid `elk/geometry.ts` (`outsideNode`, `outlineAttachPoint`) and
`elk/render.ts` (`cutter2`); corresponding Rust code is
`crates/merman-render/src/elk_edge_geometry.rs`. No production correction is
supported by this evidence.

## Provider test and limits

[`flowchart_elk_routes_with_captured_browser_dimensions`](../../crates/merman-render/src/flowchart/elk/browser_measurements_tests.rs)
now expects 46 entries. It replaces only invocation node/label dimensions and
executes the actual Rust provider. For this entry, optional `provider_geometry`
checks node center x/y and width/height; existing `points` checks every raw route;
optional `provider_labels` checks center-label count, identity and top-left
x/y/width/height. An empty list asserts absence. All use the existing `1e-8`
provider precision. Previous entries acquire no new expectations.

The coordinating agent compiled and executed the provider test successfully.
The combined provider and SVG writer owner run passed all six tests. Its terminal
capture was independently regenerated: five fixtures, 78 literal probes; the four
previous entries are unchanged. The exact browser-text receipt and root-contract
owner run also passed all 13 tests.
The upstream experiments prove same-measurement agreement for this fixture;
they do not prove that the complete Rust pipeline with a real browser text
measurement policy already passes. The existing captured-geometry writer test
replaces node placement and routes, so its earlier pass cannot supply that proof.

The coordinating main agent has separately aligned the xtask catalog/matrix
counts to 126/46, including the parity/parity-root receipt for 8368. That work
does not change the original comparison gates or tolerances and is outside this
evidence change.

The main agent owns compilation and the separate writer gate:
`svg/parity/flowchart/browser_measurement_tests.rs` iterates this matrix but obtains
SVG terminal primitives from `flowchart-svg-terminal-measurements.json`. The coordinating agent supplied those separately captured primitives through
the existing browser collector; the strict lookup remains unchanged.

Both matrix consumers and the full structure/parity/parity-root gate passed.
These results retain the boundary above: they do not assert a complete browser
text-profile-driven Rust layout run or cross-platform pixel identity.
