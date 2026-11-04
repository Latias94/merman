# v0.9.2 maintenance compatibility decisions

## Release boundary

Reviewed through 2026-10-22 against development `3119c614bf8c43d49cfe17ded0a53ec2e7fa983c` and the maintenance delta from immutable `v0.9.1` (`f6202f702dbde0a75049b1a62b3124feae709b3d`). Merged [#235](https://github.com/Latias94/merman/pull/235), source `ae3299f9a9db517e91c808e63a67cb8d30a1ce37`, contains the initial maintenance repairs. Follow-up [#237](https://github.com/Latias94/merman/pull/237) adds the independently applicable fixes identified below. Its base is `release/0.9.x`; the published tag, channel receipts, and final source are recorded in [PUBLISH_ORDER.md](PUBLISH_ORDER.md). The published root and SDK 0.9.2 changelogs record the release date 2026-10-22.

The selected repairs still fit a 0.9.2 patch: Presentation, Options JSON schema 2, Mermaid 12.1.0, and public API signatures remain unchanged. Main's compiled theme API, schema 3, and Mermaid 12.2.0 are not backported.

The maintenance release comparison is `v0.9.1..#237`. Main development is inspected through first-parent integrations from `v0.9.0`, with fixes already published in 0.9.1 excluded from new outcomes. The 0.9.1 maintenance tag is not an ancestor of main; a raw missing-commit list would confuse adapted equivalents and old branch history with missing behavior. Decisions below compare final source and owning tests, not commit dates or subjects alone.

## Selected repairs

| Surface and audience | Published baseline -> candidate | Evidence and disposition | Changelog home |
| --- | --- | --- | --- |
| Native font selection | Implicit generic and missing-glyph fallback can select symbolic faces -> symbolic faces excluded, explicit named fonts retained | `208cbc5f9`, `741a2e231`; #235, issue #228; included | Root 0.9.2 Fixed; custom-native SDK projections; Node omitted |
| C4 rendering | Filled boundaries can cover descendants and host relationship paints are incomplete -> background-first layering and actual host paint ownership | `282497485`, `4b4927959`, `31a8add27`; #235, issue #229; folded into one outcome | Root and coupled SDK 0.9.2 Fixed |
| Wardley rendering | Host anchor and structural colors incomplete -> explicit/evolved colors preserved while host fallbacks reach terminals | `3b38d4178`; #235, issue #230; included | Root and coupled SDK 0.9.2 Fixed |
| ZenUML rendering | Host colors/typography incomplete -> matching measurement/paint typography and explicit namespace precedence | `3b38d4178`; #235, issue #231; included | Root and coupled SDK 0.9.2 Fixed |
| Authored Mermaid palettes | Host-derived colors leak into selected themes -> full explicit site/source palette retained, independent fonts available | `0576442c2`; #235, issue #232; included; main intentionally has a different source-only palette policy | Root and coupled SDK 0.9.2 Fixed |
| Look selectors | Arbitrary look strings can reach selectors -> supported-look whitelist and normal fallback | Main `3fe754e8b` -> `9f007b96c`; #235; included | Root 0.9.2 Fixed and SDK CSS item |
| Block source CSS | Rejected declarations can be restored into SVG -> rejected declarations dropped, valid siblings retained | Main `070e9889b` -> `1fc89439c`; #235; included | Root 0.9.2 Fixed and SDK CSS item |
| Requirement source CSS | Shared validation bypassed and quoted priorities truncated -> safe parsing with declaration order and quoted values preserved | Main `916d3c529` -> `c6939dba5`; #235; included | Root 0.9.2 Fixed and SDK CSS item |
| Flowchart style declarations | Repeated `classDef` overwrites the prior declaration -> source-ordered accumulation | Main #227 `ef5fa90dd`; maintenance `semantic.rs` retained the overwrite and receives the same fix/tests in #237 | Root 0.9.2 Fixed and SDK parser item |
| Flowchart error recovery | Unsupported multibyte characters advance one byte -> whole-character diagnostic spans and safe continuation | Main #178 `a5e3cd2d6`; only the remaining lexer/recovery delta is backported in #237; Unicode identifier ranges are already present in the legacy parser | Root 0.9.2 Fixed and SDK parser item |
| Pie/Info strict and lenient parsing | Invalid known-type headers can succeed as error-shaped JSON -> normal parse failure, recovered editor facts, and lenient error diagram | Main #227 `ef5fa90dd`, characterization `d5a4e8643`; maintenance uses the same family parsers; adapted inline characterization matrix in #237 | Root 0.9.2 Fixed and SDK parser item |
| XYChart empty input | Semantic/render/editor paths construct different empty models -> one configured default model | Main #227 `ef5fa90dd`; same state constructor exists on maintenance; included in #237 | Root 0.9.2 parser outcome and SDK parser item |
| SVG CSS cleanup | EOF-terminated strings/comments/URLs can survive one cleanup pass -> reject incomplete tokens and retain safe neighboring CSS | Main #227 `ef5fa90dd`; shared sanitizer exists on maintenance; same token and idempotence controls included in #237 | Root 0.9.2 Fixed and SDK CSS item |
| Python wheel construction | Hardcoded repository `target` ignores Cargo configuration -> locate both cdylib and metadata under Cargo's reported target directory | Main #178 `c7f661394`; reproduced by source/path contract and included with owner tests in #237 | Root and Python 0.9.2 Fixed; other SDKs omitted |
| Workspace crates publisher | Preflight runs independently versioned Typst wrapper tests against a new workspace plugin -> current-plugin examples and separate wrapper owner | Main #210; maintenance still lacked `--examples-only`; workflow and its contract test included in #237 | Root 0.9.2 Fixed; SDK runtime projections omitted |
| Tree-sitter edit fuzzing | Fresh/incremental recovery ranges are incorrectly required to match -> per-tree geometry plus inverse-edit restoration, following Tree-sitter 0.27.1 | Main #227; identical legacy fuzz target receives the standalone repair, regression tests and exact LF/binary seeds in #237; no parser API/grammar import | Root 0.9.2 developer-workflow item; SDK runtime projections omitted |
| Certificate verification dependencies | Distrusted emSign paths admitted by affected locked versions -> `rustls-webpki 0.103.16` and `webpki-root-certs 1.0.10` | Main `d5097b8d0` / maintenance `7d13eff7d`; already included by #235; RustSec links verified against the advisory database | Root 0.9.2 Security; SDKs without this TLS dependency closure omitted |
| Compatible dependency refresh | Re-resolve the three release-owned Cargo locks against the current index | #237 updates the workspace, fuzz and standalone Node locks; fixed math/Tree-sitter/NAPI versions and the shared export type boundary remain pinned | Root 0.9.2 Changed; generated legal reports and exception paths refreshed |


## Issue repair mapping

| Behavior | Main | Maintenance equivalent |
| --- | --- | --- |
| Authored palette and derived ER/Quadrant colors (#232) | `4c70b8909`, `633b45ff0` | `0576442c2`: separate raw initialize/source layers from Presentation defaults; retain typography, actual terminal fallback chains and detector mutation semantics. Includes Timeline scale regression. |
| Symbolic implicit font selection (#228) | `9a5bf169a` | `208cbc5f9`: same admitted-byte classification across closed, bounded, legacy system and optional platform discovery; explicit named fonts remain selectable. |
| Filled C4 boundaries hide descendants (#229) | `0903b2c94`: family painter background before descendants, foreground after | `282497485`: adapt the iterative old painter; transparent baseline preserved. |
| C4 relationships, labels and arrowheads (#229) | `0903b2c94`, `1f01c31b7`: actual typed terminal roles and receipts | `282497485`, `4b4927959`, `31a8add27`: existing native C4 fields and HostTheme projection; explicit site/source owners win; exact materialized host line provenance retains native shared markers without an actual host paint request. |
| Gantt state and label paints (#230) | `0903b2c94`: state-qualified presets and terminal foregrounds | `cc7491584`: existing old surface/muted/alt states already pass new regression coverage; no typed-theme feature port. |
| Wardley anchors (#230) | `0903b2c94`: text/node-label adapter with independent annotation and arrow pairs | `3b38d4178`: legacy final palette fallback, explicit anchor/axis values and evolution retained. |
| ZenUML host colors and typography (#231) | `0903b2c94`: unified family preparation and honest terminal receipts | `3b38d4178`: namespace-first legacy adapter; matching measurement/paint fonts; plain companion baseline retained. |

## Compatibility exclusions

The maintenance line preserves Presentation, Options JSON schema 2, Mermaid 12.1 and its
published Rust/binding signatures. Main's compiled-theme architecture, schema changes, new
prepared-rendering APIs, HTML parser replacement and Mermaid upgrades are separate development
contracts. Port an independently reproduced correctness repair through maintenance's existing
owners; matching commit subjects or patch IDs do not prove equivalent behavior.

Explicit site/source palette choices on maintenance retain their historical ownership. Main's
source-only palette and typed terminal receipts must not be transplanted as a maintenance API
change. Gantt's legacy state colors already passed the new regression controls, so the release
does not announce a new Gantt implementation repair.

The reported older docgen input used alpha.7's former interactive/trusted-native layout ceilings.
Both selected branches had already adopted the calibrated 14,100,000/15,000,000 work-unit ceilings.
The failing graph was unavailable, so no claim is made that it fits those limits. The maintenance
repairs neither increase those budgets nor select an unbounded policy.

## Verification boundary

This page records release-specific compatibility decisions. Published tag, source, channel and
immutable preflight identities belong to [the publication record](PUBLISH_ORDER.md#092-publication-snapshot).
Historical dependency review results do not establish the current advisory state; use the
[current dependency policy](../DEPENDENCY_POLICY.md) and its Security Audit owner.
