# ASCII Semantic-Depth Closeout Scorecard

Status: current index; [ASCII_U25_CLOSEOUT_RECEIPT.md](ASCII_U25_CLOSEOUT_RECEIPT.md) is the sole authority for revision, verification, performance, and maintainer disposition.

| Item | Current value |
| --- | --- |
| Product source | `861fc0ba33ac6f0a724263b3a6f303a3f26eee15` |
| Product tree | `fc552d5c52f9555c619a67294d523baaf2c89203` |
| Merge-base | `16b84122615b2dd60c67577bd8708ef5f226f755` |
| Mermaid baseline | `11.16.1` |
| Public ASCII entry point | `Renderer::render(RenderRequest::ascii(...))` |
| Verification status | Rust, bindings, generated artifacts, web, playground, and CLI representative gates recorded as passed in the receipt |
| Performance status | Medium relative results inconclusive but below the lane-specific materiality boundary; large Flowchart/Sequence/Class A/A inconclusive; ER/XYChart non-regressions |
| Support boundary | Diagrammatic: Flowchart, Sequence, State, Class, ER, XYChart; structured text remains explicitly non-diagrammatic; unsupported families remain unsupported |

## Maintainer decision

The refactor is ready for review as a Partial-support implementation. The receipt accepts measured residuals and statistical insufficiency explicitly; it does not claim zero relative slowdown, universal visual superiority, or Full support for every Mermaid family.

The 50 microsecond figure is a lane-specific materiality boundary, not a universal repository rule. Any future performance claim must register its own profile, evidence, and decision rule.

## Residual follow-ups

The receipt records accepted P2 follow-ups: the doc-hidden terminal helper surface, the legacy `OperationLedgerError` name, shadow test orchestration, overlapping performance owners, and fine-grained cancellation gaps in grapheme trimming/layered sorting. These are follow-up work, not unresolved closeout rows.

Do not duplicate command tables or historical benchmark output here. Update the canonical receipt first, then keep this file as an index.
