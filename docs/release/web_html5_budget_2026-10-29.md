# Maintenance HTML5 Web budget reassessment

Keep the corrected HTML5 parsing and sanitization contract. Refresh only the
`web-analysis` raw and stripped ceilings, using this branch's existing policy:
measured bytes plus 3%, rounded upward to 25,000 bytes. Both become 3,850,000 bytes.
The other 22 limits, artifact recipes, capabilities, compression settings, and
measurement implementation retain their existing values.

## Recorded artifact evidence

[Linux CI run 40738572390](https://github.com/Latias94/merman/actions/runs/40738572390)
measured the five assembled npm packages from PR #250 at branch head
`69e0fe506dded14451eec82923c0f2a6f3ca21da`. The checked-out PR merge revision was
`9879336694a14a563fe14c12b5f75490b9803f3a`. The run used Rust 1.95.0,
wasm-pack 0.15.0, and the existing final-package size recipe with wasm-tools
1.261.0. The package provenance/freshness checks, all five runtime smokes, and
the DOM safety smoke passed before the size gate.

| Profile | Raw bytes | Stripped bytes | Gzip bytes | Brotli bytes |
| --- | ---: | ---: | ---: | ---: |
| web-analysis | 3,728,846 | 3,728,611 | 1,430,488 | 1,101,186 |
| web-ascii | 5,314,990 | 5,314,755 | 1,957,152 | 1,490,788 |
| web-editor | 3,844,330 | 3,844,095 | 1,476,909 | 1,131,749 |
| web-full | 13,645,263 | 13,645,028 | 5,065,703 | 3,766,103 |
| web-render | 11,656,643 | 11,656,408 | 4,389,942 | 3,258,005 |

The gate measured every profile and reported only the analysis raw and stripped
failures, against their old 3,675,000-byte ceilings. Apply the existing rule to
each metric independently:

```text
((measured_bytes * 103 + 2_499_999) // 2_500_000) * 25_000
```

The two new limits leave 121,154 and 121,389 bytes of headroom. Analysis gzip and
Brotli already pass. ASCII and editor raw retain only 35,010 and 55,670 bytes of
headroom, so the updated PR must still pass the complete five-profile size owner.

## Why this implementation is retained

Analysis exposes parsed labels, titles, and accessibility text. Its core parsing
path therefore needs HTML5 recovery and sanitization even without SVG rendering.
The shared `sanitize/fragment.rs` implementation matches main and retains inert
document parsing, formatting reconstruction, table/template recovery, foreign
content namespaces, noscript handling, and explicit resource limits. The existing
sanitizer regressions cover these behaviors. Restoring the old rewriter or
removing tree recovery would change those contracts.

The locked html5ever dependency has no default feature bundle to remove; its
optional serialization and tracing features are already disabled. No final
binary evidence identifies an unrelated reachable XHTML branch to trim. Source
presence alone does not establish that such code survives linking into analysis.

These are whole-branch artifact measurements. The excess over the old ceiling is
not an isolated measurement of HTML5's cost, and this decision makes no latency
or memory acceptance claim. Main's separate semantic-boundary budget has different
recipes and changes; its numeric limits are not copied into this maintenance line.

The recorded run remains failed. Its size failure prevented later Playground
steps from running. The refreshed PR's existing Web owner must rebuild and verify
the final package group, measure all 20 Web metrics, and execute the remaining
browser checks. Historical local packages are not current-source evidence.
