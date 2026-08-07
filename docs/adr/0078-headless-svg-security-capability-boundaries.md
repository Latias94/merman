# ADR 0078: Headless SVG Security Capability Boundaries

- Status: accepted
- Date: 2026-08-06
- Baseline: Mermaid `11.16.0`

## Context

Mermaid's `securityLevel` combines source sanitization with behavior supplied by its browser host.
In particular, `sandbox` wraps the rendered result in an iframe. Merman is a headless renderer: it
can reproduce parsing, configuration, URL sanitization, and SVG structure, but it does not own a
browser origin, iframe, content-security policy, navigation handler, or process sandbox.

SVG references also carry different capabilities. An anchor navigation is activated by a user;
an image, filter, style, or external `<use>` reference can initiate work while the document is
rendered. Treating both as an undifferentiated "external resource" rejects valid Mermaid diagrams
such as Kanban tickets. Treating both as harmless gives an SVG consumer ambient file or network
access.

The project already has three consumers with distinct needs: Mermaid-parity SVG, sealed
resvg-compatible SVG for raster and PDF export, and SVG mounted into a browser DOM by the Web
package. One configuration switch cannot truthfully describe all three.

## Decision

1. Mermaid config and host output policy remain separate axes.
   - `securityLevel` retains Mermaid semantics. Non-loose levels sanitize labels and URLs; `loose`
     is an explicit trusted-input mode.
   - `sandbox` uses the non-loose sanitization path in Rust but does not claim iframe or origin
     isolation. Hosts that require isolation must create it.
   - Merman does not add `allowExternalResources`, `allowLinks`, or another browser-host setting to
     `MermaidConfig`.

2. Every renderer-created SVG navigation href uses one shared final admission path while preserving
   its diagram family's upstream URL stage.
   - Flowchart and Class keep their config-sensitive `formatUrl` stage; Sequence keeps its
     unconditional `sanitizeUrl` stage; Kanban and State pass their constructed or model URL
     directly to final admission, matching Mermaid 11.16.
   - The shared path distinguishes a DOM attribute value from serialized SVG source. It serializes
     the renderer-created value, applies Mermaid's global `decodeEntities` cleanup, and only then
     parses the source for the pinned DOMPurify URI policy. This prevents both placeholder leakage
     and entity double-decoding.
   - Non-loose output emits the normalized DOMPurify value or omits the href. Loose output stops
     after Mermaid cleanup, matching the upstream trusted-markup path.
   - Rejected URLs lose the href while their visible diagram text remains.
   - Loose parity output may retain active URLs by design and must be treated as trusted markup.

3. Rust SVG outputs make consumer-specific claims.
   - Parity and readable SVG preserve Mermaid navigation metadata and do not claim browser DOM
     safety or resource closure.
     Strict-like levels follow Mermaid's final DOMPurify shape; loose parity may retain family-local
     target metadata because Mermaid skips that final browser sanitizer. A browser host may rewrite
     those targets at mount time.
   - `ResvgCompatibleSvg` is a sealed raster-consumer contract. It removes active content and
     closes automatically resolved rendering resources, but retains safe anchor metadata because
     resvg does not navigate anchors. Its source scanner validates serialized attributes; terminal
     XML validation consumes parsed DOM values and never decodes character references a second time.
   - A future self-contained Rust SVG type requires a concrete non-browser consumer and a terminal
     validator. It will not be represented by a boolean on the parity renderer.

4. Browser capabilities remain owned by the Web host.
   - Closed previews use `assertSelfContainedSvgForDom()`; authoring surfaces that support Mermaid
     links use `assertNavigableSvgForDom()`.
   - The validators return distinct opaque `SelfContainedSvgDomAdmission` and
     `NavigableSvgDomAdmission` capabilities. They are branded by the package instance and retained
     in a private runtime capability registry, so object literals, structured clones, and the wrong
     capability cannot cross a typed mount boundary.
   - The matching `prepareSelfContainedSvgForDomMount()` or
     `prepareNavigableSvgForDomMount()` helper revalidates the actual parsed root in its owner
     document immediately before insertion. The navigable helper also hardens anchors. Rechecking
     the root prevents the earlier source assertion from authorizing an unchecked or subsequently
     unsafe tree, while the document check prevents an HTML `<base>` element from turning
     `#fragment` into an external resource request.
   - External navigations receive `target="_blank"` plus `noopener noreferrer`; fragment anchors
     receive `target="_self"`, so a host `<base target>` cannot redirect them into another browsing
     context.
   - CSP, iframe isolation, opener behavior, gesture arbitration, and navigation enablement remain
     host responsibilities because only the browser host can enforce them.

## Consequences

- Kanban ticket links and other sanitized Mermaid navigation remain usable without admitting
  external images, styles, filters, or scripts.
- Rust callers can select Mermaid sanitization and output compatibility independently without a
  misleading universal "safe SVG" mode.
- `securityLevel: "sandbox"` cannot be mistaken for an isolation guarantee from a headless API.
- Browser validation remains close to the browser parser and does not drift into a second partial
  DOM implementation in Rust.
- Fragment resource closure is a source-plus-host invariant rather than a false property of the SVG
  string alone.
- Hosts that deliberately request loose parity output must still validate or isolate it before DOM
  insertion.

## Rejected Alternatives

### Reject every external href

Rejected because it confuses user-activated navigation with automatic resource loading and breaks
documented Mermaid behavior.

### Add an `allowExternalResources` switch

Rejected because it grants unrelated image, CSS, filter, tracking, and navigation capabilities at
once. It would turn an internal classification error into a user-facing security footgun.

### Make Rust `securityLevel` select browser behavior

Rejected because Rust cannot create or enforce the browser origin, iframe, CSP, target, or gesture
boundary. Such an option would make a claim the headless renderer cannot satisfy.

### Validate browser DOM safety only in Rust

Rejected because XML parsing alone does not reproduce HTML parsing, namespace fixups, browsing
contexts, CSP, or browser navigation behavior. Rust owns source and output semantics; the Web host
owns DOM admission.

## Related Decisions

- ADR-0059: Raster Output Strategy
- ADR-0063: Extensible SVG Output Pipeline
- ADR-0069: WASM Package Surface Semantics
- ADR-0074: Browser Runtime And Benchmark Ownership
- ADR-0077: Presentation, Theme, Mermaid Config, And SVG Output Ownership
