# Mindmap HTML-to-XHTML output boundary

## Scope and cause

This default-output correction starts at `e9c4c1341`. The full structure/parity/parity-root sweep in the preceding Sequence increment found two existing Mindmap fixtures rejected by final XML validation: `stress_mindmap_html_sanitization_013` and `stress_mindmap_html_sanitization_links_034`.

The sanitizer correctly removes `onerror` and unsafe `javascript:` links, but preserves accepted HTML attribute spelling such as `src=x`. HTML accepts that spelling; SVG's embedded XHTML requires quoted attribute values and self-closed void elements. Mindmap previously replaced only line-break tags. The final XML validator correctly rejected the resulting image markup; its checks are not relaxed.

The existing shared XHTML normalizer now uses the already-linked `quick-xml` attribute iterator to quote HTML-only attribute forms. Already valid XML tags preserve their original spelling. It retains raw entity values, escapes delimiters when moving single-quoted values to double quotes, and distinguishes an unquoted trailing slash from a self-closing delimiter. Malformed attributes remain visible to the final validator; duplicate detection is not disabled there. Output growth continues to use the existing bounded string contract.

Mindmap uses this normalizer for ordinary and math label fragments. Security decisions remain in the original sanitizer. No dependency, public API, capability, font, budget, or acceptance rule is added.

## Design checks

An initial experiment forced every accepted sanitizer attribute through `set_attribute`. Review found that the dependency performs a linear name search per setter, introducing quadratic work for many accepted attributes. That experiment was removed completely; `merman-core/src/sanitize.rs` has no final diff. Its diagnostic test pass is not treated as final validation.

The final projection performs at most two linear attribute scans and builds a result only for HTML-only attribute syntax. A local optimized probe includes the actual production `xml.rs` and scanner source. Median time per call over seven batches of 20 calls was 47,837 / 98,387 / 200,347 / 390,968 ns for 1,000 / 2,000 / 4,000 / 8,000 unquoted attributes (11,905 / 24,905 / 50,905 / 102,905 input bytes). This supports linear growth for this helper; it does not measure complete rendering, sanitizer complexity, allocator peaks, or the branch's overall performance gates. Reproduction and source hashes are in `/tmp/merman-mindmap-xhtml-2069e5d85/`.

Chromium 151 with the locally installed DOMPurify 3.4.13 confirms that accepted unquoted/single-quoted attributes become quoted and unsafe attributes/links disappear. This is a direct HTML serialization check, not a change to the pinned Mermaid baseline.

## Validation

- Final Renderer Release nextest passed **2,964 tests, three skipped**, with `--no-default-features --features layout-cytoscape,embedded-fonts`, covering library tests and the Mindmap, Flowchart and Sequence SVG integration targets. This is the final renderer projection; the earlier sanitizer experiment is excluded.
- A direct compilation of the production XML normalizer and scanner passed all ten local module tests. Regression cases cover unquoted and boolean attributes, attached versus detached slashes, entity preservation, valid single-quoted spelling, and one-byte-short output limits. The two actual Mindmap fixtures additionally check XHTML image namespace, preserved safe links and removed unsafe attributes/links.
- The completed independent code review found no retained issues across correctness, testing, maintainability, project standards, security, performance and adversarial lenses. One independent Codex context ran those lenses serially; this is not seven independent reviews. It also checked shared Flowchart/math consumers and the unchanged final XML validator. Receipt: `/tmp/compound-engineering-501/ce-code-review/20260919-094406-194bd81c/review.json`.

- The complete CI-style SVG sweep passed all 35 comparison groups using `compare-all-svgs --check-dom --dom-modes structure,parity,parity-root --dom-decimals 3 --diagnostic-browser-text-layout --report-root`. Mindmap selected and rendered 114 fixtures without skips, producing 342 DOM comparisons and no accepted residuals. The preceding Sequence increment's corrected browser-text receipts also pass in this full sweep.
- Chromium 151.0.7922.34 loaded both newly generated failing-fixture SVGs: zero XML parser errors, one XHTML image with preserved `src="x"` in each, safe links retained, and no unsafe event/JavaScript attributes. Nonempty text labels have visible nonzero geometry. Network requests were blocked, so the deliberately nonexistent image is not a successful-image-paint test. Screenshots were inspected; the `docs` link shows browser text clipping, so these checks do not establish complete text fitting or pixel-level visual qualification.
- Scoped Renderer Release Clippy completed successfully with 160 warnings, matching the preceding increment's count; no diagnostic names either changed production file. This is not a warning-free build. Workspace formatting and `git diff --check` passed.

Logs: `/tmp/mindmap-xhtml-{renderer-final,parity-final,clippy-final,fmt-final}.log`. Browser observations, script and screenshots: `/tmp/merman-mindmap-xhtml-2069e5d85/`. The final three-file source diff is unchanged from the completed review; validation documentation was added afterward. Local checks for this correction are complete; overall branch readiness remains governed by the open product plan.

## Scope limits

This increment repairs default output. It does not close U7/U8/U9, qualify the complete Cyberpunk scenes, rebuild installed platform packages, or freeze C7a. No performance or artifact-size budget is relaxed.
