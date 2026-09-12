# Block ClusterLabel projection retirement

## Boundary

Baseline: `86a61b8d873a130f8d74fae8dbbe8a8cb3c07003`.
Pinned Mermaid source: `mermaid@11.17.2`,
`dcb694ddb58dc5ad3502e7e903cac05fd812eac3`.

Block composite labels are node labels. Its stylesheet reads nodeTextColor for
visible labels; secondaryTextColor and tertiaryTextColor have no Block consumer.
The NodeDiagramTheme adapter and Block writer retain their existing behavior.
Marker paint remains legacy: Block emits arrowMarkerPath and styles its fill
from arrowheadColor, so it has an actual consumer.

The final change removes only Block's ClusterLabel projection and its generic Text
fallback. Class keeps its original compatibility assignments and assignment order
until its own cutover. The earlier `adae4f255` commit removed the shared assignment
from both families; the follow-up restores the Class boundary and tests both its
contribution and effective secondaryTextColor/tertiaryTextColor values.

## Historical and runtime evidence

Two Release tests ran before production edits. Twelve ClusterLabel solid/transparent
and unqualified/Default requests over Classic, Neo and HandDrawn installed a fallback
contribution and changed effective secondaryTextColor, but left native PNG pixels
unchanged. A Node.fill control changed pixels in every look. Composite and leaf
labels remained present. A strict-admission regression failed with a legacy
compatibility residual before the fix.

A second run at the same historical revision covers Default, Base, Dark, Forest and
Neutral with all three looks: 60 installed ClusterLabel projections leave native
pixels unchanged, while all 15 Node.fill controls change pixels. The final native
harness also covers these five themes and requires an independent NodeLabel.fill
control to change pixels.

The existing empty terminal domain now reconciles ClusterLabel requests as
NotApplicable; public discovery reports Unsupported. No typed ClusterLabel support
is claimed. Tests cover HTML/SVG labels, Default/Active/unqualified selectors,
ordinal mixed facets, Clear, and a sibling Node fill that must still apply.

KTD23 v8 adds two historical identities and four value probes; prior batches remain
frozen. The cumulative inventory has 78 identities and 156 value probes. Remaining
executable legacy routes drop from 82 to 78: Block 36, Class 26, XY Chart 16. KTD17
positive typed-route counts are unchanged.

## Investigation limits

Reading adapter fields alone does not establish that an assignment is unused:
Dark derives noteTextColor from secondaryTextColor, and Class uses different HTML
and native note rules. An attempted native PNG negative control did not exercise
the HTML consumer. A historical comparison then showed that the tested Class HTML
SVG was byte-identical before retirement, after the initial removal, and with the
Class assignment retained (SHA-256
`ef9ad919a0bc227d3c4e7249fab5bb515b2634c62b2e8aa46f55b450f72c888c`).
Chromium observed visible note text with color rgb(184, 182, 182). This does not
establish a new visual regression or a complete Class Text support claim. Class
compatibility is retained because it is outside this Block retirement batch.

A local shared-target check reused the previous checkout's probe binary: its SVG
output went to the previous checkout's designated output directory. A forced
rebuild of the affected crate roots corrected that provenance issue. Distinct
historical source comparisons must invalidate the affected Cargo artifacts or use
separate targets; a fast test result alone is insufficient source evidence.

## Verification

The initial retirement passed 3,844 Release renderer tests (four skipped), 138
private acceptance tests, and the full SVG structure comparison. Its clean checkout
passed 87 focused renderer tests and 138 private acceptance tests. These checks did
not guard the unrelated Class configuration assignment, which is now retained.

The corrected implementation passed 3,844 Release renderer tests (four skipped),
including the Class effective-configuration guard, and 138 private acceptance
tests including the five-theme native matrix. The complete SVG structure comparison
and formatting checks also passed.

A clean detached checkout of corrected implementation
`5e90b8940bdc14cba449d3045f420654839d4391` passed 89 focused renderer tests
(Block, support discovery, and the Class assignment guard; 2,487 tests excluded
by the filter), followed by the complete private acceptance suite: 138 passed,
none skipped. The checkout remained clean. Both runs used the pinned toolchain,
two build jobs and the shared target with affected crate roots invalidated before
the checkout build. No Web artifact rebuild was performed for this slice. This slice does
not close C7a or establish fresh Web artifact qualification; the previous five-profile
Web size-budget failure remains unclosed.
