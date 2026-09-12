# Block ClusterLabel projection retirement

## Boundary

Baseline: `86a61b8d873a130f8d74fae8dbbe8a8cb3c07003`.
Pinned Mermaid source: `mermaid@11.17.2`,
`dcb694ddb58dc5ad3502e7e903cac05fd812eac3`.

Block composite labels are node labels. The Block stylesheet reads nodeTextColor
for visible labels; secondaryTextColor and tertiaryTextColor have no consumer.
The native NodeDiagramTheme and ClassDiagramTheme adapters also omit these fields
(`crates/merman-render/src/svg/parity/theme/families.rs`). Class had already
retired its direct ClusterLabel projection. Removing the shared unused assignment
also prevents a generic Text rule from recreating it in either family, while
preserving the real node-label projection.

Marker paint is different: Block emits arrowMarkerPath and styles its fill from
arrowheadColor. Marker routes remain legacy and are not included in this retirement.

## Evidence and change

Before editing production code, two Release tests exercised the baseline bridge:
twelve ClusterLabel solid/transparent and unqualified/Default requests over
Classic, Neo and HandDrawn installed a fallback contribution and changed effective
secondaryTextColor. They left native PNG pixels unchanged, while a Node.fill
control changed the pixels in each look. Composite and leaf labels remained
present. The strict-admission regression failed with one legacy compatibility
residual before the fix.

The writer and its label semantics are unchanged. The existing empty terminal
domain now reconciles ClusterLabel requests as NotApplicable; public discovery
reports Unsupported. This does not claim typed ClusterLabel support. Tests cover
HTML/SVG labels, Default/Active/unqualified selectors, ordinal mixed facets, Clear,
and a sibling Node fill that must still apply. Native comparisons and effective
configuration checks cover both retired text targets without duplicating their
rendering harness. The final native harness also requires NodeLabel.fill to change
the pixels independently of the Node.fill control.

KTD23 v8 adds two historical identities and four value probes; prior historical
batches remain frozen. The cumulative inventory has 78 identities and 156 value
probes. Remaining executable legacy routes drop from 82 to 78: Block 36, Class 26,
XY Chart 16. KTD17 positive typed-route counts are unchanged.

## Verification

The focused Release Block rendering and support-discovery suite passed 87 tests.
The full Release renderer suite passed 3,844 tests (four skipped), and the private
acceptance suite passed 138 tests. After adding the independent NodeLabel paint
control, all three native retirement tests passed. The complete SVG structure
comparison and formatting check passed. Clean-checkout results remain to be
recorded against the implementation commit. This
slice does not close C7a or establish fresh Web artifact qualification; the existing
five-profile Web size-budget failure remains open.
