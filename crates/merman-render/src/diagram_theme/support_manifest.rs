//! Versioned renderer-owned theme support claims.
//!
//! Runtime discovery depends only on this manifest. The private C5 family mechanism matrix is
//! referenced below only by the C6 drift test and is never a production dependency.

use crate::DiagramFamilyId;
use merman_theme_contract::{ThemeRuleFacetV1, ThemeSupportBaseTypographyPropertyV1};

/// Renderer-owned support-claim manifest revision.
///
/// Version against the last published claim manifest. Unreleased claim changes share revision 1;
/// source identities distinguish development builds.
pub(super) const SUPPORT_CLAIM_MANIFEST_REVISION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SupportClaimKind {
    TypedSurface,
    TypedPartial,
    Unsupported,
    Missing,
}

#[derive(Clone, Copy)]
struct RuleClaim {
    family: &'static str,
    target: &'static str,
    kind: SupportClaimKind,
    facets: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct BaseClaim {
    family: &'static str,
    kind: SupportClaimKind,
    properties: &'static [&'static str],
}

const MANIFEST_TARGET_IDS: &[&str] = &[
    "canvas",
    "node",
    "node-label",
    "edge",
    "edge-label",
    "edge-label-background",
    "cluster",
    "cluster-label",
    "marker",
    "title",
    "text",
    "axis",
    "axis-title",
    "axis-label",
    "axis-tick",
    "legend",
    "table",
    "task",
    "task-label",
    "packet-byte-label",
    "packet-field-label",
    "state",
    "state-label",
    "transition",
    "transition-marker",
    "transition-label",
    "transition-label-background",
    "composite",
    "composite-header",
    "composite-label",
    "special-state",
    "special-state-inner",
    "actor",
    "actor-label",
    "lifeline",
    "message",
    "message-label",
    "sequence-number",
    "loop",
    "loop-label-background",
    "loop-label",
    "note",
    "note-label",
    "activation",
    "requirement",
    "entity",
    "relation",
    "pie-slice",
    "chart-series",
    "timeline-event",
    "journey-task",
];

const MANIFEST_RULE_FACET_IDS: &[&str] = &[
    "fill",
    "opacity",
    "fill-opacity",
    "stroke-paint",
    "stroke-width",
    "stroke-dasharray",
    "stroke-line-cap",
    "stroke-line-join",
    "stroke-opacity",
    "radius",
    "padding",
    "font-stack",
    "font-size",
    "font-weight",
    "font-style",
    "line-height",
    "letter-spacing",
    "word-spacing",
    "text-transform",
    "text-decoration",
    "text-align",
    "white-space",
    "wrap",
    "effect",
];

const MANIFEST_BASE_PROPERTY_IDS: &[&str] = &[
    "font-stack",
    "font-size",
    "font-weight",
    "font-style",
    "line-height",
    "letter-spacing",
    "word-spacing",
    "transform",
    "decoration",
    "text-align",
    "white-space",
    "wrap",
];

// The rows are expressed in stable contract identifiers rather than private matrix types.
const RULE_CLAIMS: &[RuleClaim] = &[
    RuleClaim {
        family: "architecture",
        target: "cluster",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "architecture",
        target: "edge",
        kind: SupportClaimKind::TypedPartial,
        facets: &["stroke-paint"],
    },
    RuleClaim {
        family: "architecture",
        target: "marker",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "architecture",
        target: "node",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "architecture",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "architecture",
        target: "title",
        kind: SupportClaimKind::Unsupported,
        facets: &["fill"],
    },
    RuleClaim {
        family: "block",
        target: "node",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "c4",
        target: "cluster",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "radius", "stroke-paint"],
    },
    RuleClaim {
        family: "class",
        target: "edge",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint", "stroke-width"],
    },
    RuleClaim {
        family: "class",
        target: "node-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "class",
        target: "node",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "er",
        target: "entity",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "er",
        target: "relation",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "er",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "eventmodeling",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "flowchart",
        target: "cluster",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint", "stroke-width"],
    },
    RuleClaim {
        family: "flowchart",
        target: "edge-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "effect",
            "font-size",
            "font-stack",
            "font-weight",
            "padding",
        ],
    },
    RuleClaim {
        family: "flowchart",
        target: "edge",
        kind: SupportClaimKind::TypedPartial,
        facets: &["effect", "stroke-dasharray", "stroke-paint", "stroke-width"],
    },
    RuleClaim {
        family: "flowchart",
        target: "node-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &["effect", "fill", "font-size", "font-stack", "font-weight"],
    },
    RuleClaim {
        family: "flowchart",
        target: "node",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "effect",
            "fill",
            "radius",
            "stroke-dasharray",
            "stroke-paint",
            "stroke-width",
        ],
    },
    RuleClaim {
        family: "gantt",
        target: "task",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "radius", "stroke-paint"],
    },
    RuleClaim {
        family: "gantt",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "gantt",
        target: "title",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "gitGraph",
        target: "edge",
        kind: SupportClaimKind::TypedPartial,
        facets: &["stroke-paint"],
    },
    RuleClaim {
        family: "gitGraph",
        target: "edge-label-background",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "info",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "ishikawa",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "journey",
        target: "journey-task",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "radius", "stroke-paint"],
    },
    RuleClaim {
        family: "kanban",
        target: "task-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "kanban",
        target: "task",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "radius", "stroke-paint"],
    },
    RuleClaim {
        family: "mindmap",
        target: "edge",
        kind: SupportClaimKind::TypedPartial,
        facets: &["stroke-paint"],
    },
    RuleClaim {
        family: "packet",
        target: "packet-byte-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "packet",
        target: "packet-field-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "packet",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "packet",
        target: "title",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "pie",
        target: "pie-slice",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "pie",
        target: "title",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "quadrantChart",
        target: "chart-series",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "radius"],
    },
    RuleClaim {
        family: "requirement",
        target: "requirement",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "sequence",
        target: "activation",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint", "stroke-width", "radius", "effect"],
    },
    RuleClaim {
        family: "sequence",
        target: "actor-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "effect",
            "font-size",
            "font-stack",
            "font-style",
            "font-weight",
        ],
    },
    RuleClaim {
        family: "sequence",
        target: "actor",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint", "stroke-width", "radius", "effect"],
    },
    RuleClaim {
        family: "sequence",
        target: "lifeline",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint", "stroke-width", "effect"],
    },
    RuleClaim {
        family: "sequence",
        target: "loop-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "font-size",
            "font-stack",
            "font-style",
            "font-weight",
            "effect",
        ],
    },
    RuleClaim {
        family: "sequence",
        target: "loop-label-background",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint", "stroke-width", "effect"],
    },
    RuleClaim {
        family: "sequence",
        target: "loop",
        kind: SupportClaimKind::TypedPartial,
        facets: &["stroke-paint", "stroke-width", "effect"],
    },
    RuleClaim {
        family: "sequence",
        target: "message-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "font-size",
            "font-stack",
            "font-style",
            "font-weight",
        ],
    },
    RuleClaim {
        family: "sequence",
        target: "message",
        kind: SupportClaimKind::TypedPartial,
        facets: &["stroke-paint", "stroke-width", "effect"],
    },
    RuleClaim {
        family: "sequence",
        target: "note-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "font-size",
            "font-stack",
            "font-style",
            "font-weight",
            "effect",
        ],
    },
    RuleClaim {
        family: "sequence",
        target: "note",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint", "stroke-width", "radius", "effect"],
    },
    RuleClaim {
        family: "sequence",
        target: "sequence-number",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "state",
        target: "composite-header",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "fill-opacity",
            "opacity",
            "stroke-dasharray",
            "stroke-line-cap",
            "stroke-line-join",
            "stroke-opacity",
            "stroke-paint",
            "stroke-width",
        ],
    },
    RuleClaim {
        family: "state",
        target: "composite-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "font-size",
            "font-stack",
            "font-style",
            "font-weight",
            "letter-spacing",
            "text-transform",
            "word-spacing",
        ],
    },
    RuleClaim {
        family: "state",
        target: "composite",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "fill-opacity",
            "opacity",
            "padding",
            "radius",
            "stroke-dasharray",
            "stroke-line-cap",
            "stroke-line-join",
            "stroke-opacity",
            "stroke-paint",
            "stroke-width",
        ],
    },
    RuleClaim {
        family: "state",
        target: "note-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "font-size",
            "font-stack",
            "font-style",
            "font-weight",
            "letter-spacing",
            "text-transform",
            "word-spacing",
        ],
    },
    RuleClaim {
        family: "state",
        target: "note",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "fill-opacity",
            "opacity",
            "padding",
            "radius",
            "stroke-dasharray",
            "stroke-line-cap",
            "stroke-line-join",
            "stroke-opacity",
            "stroke-paint",
            "stroke-width",
        ],
    },
    RuleClaim {
        family: "state",
        target: "special-state-inner",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "fill-opacity",
            "opacity",
            "stroke-dasharray",
            "stroke-line-cap",
            "stroke-line-join",
            "stroke-opacity",
            "stroke-paint",
            "stroke-width",
        ],
    },
    RuleClaim {
        family: "state",
        target: "special-state",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "fill-opacity",
            "opacity",
            "stroke-dasharray",
            "stroke-line-cap",
            "stroke-line-join",
            "stroke-opacity",
            "stroke-paint",
            "stroke-width",
        ],
    },
    RuleClaim {
        family: "state",
        target: "state-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "font-size",
            "font-stack",
            "font-style",
            "font-weight",
            "letter-spacing",
            "text-transform",
            "word-spacing",
        ],
    },
    RuleClaim {
        family: "state",
        target: "state",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "effect",
            "fill",
            "fill-opacity",
            "opacity",
            "padding",
            "radius",
            "stroke-dasharray",
            "stroke-line-cap",
            "stroke-line-join",
            "stroke-opacity",
            "stroke-paint",
            "stroke-width",
        ],
    },
    RuleClaim {
        family: "state",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "font-size",
            "font-stack",
            "font-style",
            "font-weight",
            "letter-spacing",
            "text-transform",
            "word-spacing",
        ],
    },
    RuleClaim {
        family: "state",
        target: "title",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "font-size",
            "font-stack",
            "font-style",
            "font-weight",
            "letter-spacing",
            "text-transform",
            "word-spacing",
        ],
    },
    RuleClaim {
        family: "state",
        target: "transition-label-background",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "fill-opacity",
            "opacity",
            "stroke-dasharray",
            "stroke-line-cap",
            "stroke-line-join",
            "stroke-opacity",
            "stroke-paint",
            "stroke-width",
        ],
    },
    RuleClaim {
        family: "state",
        target: "transition-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "font-size",
            "font-stack",
            "font-style",
            "font-weight",
            "letter-spacing",
            "text-transform",
            "word-spacing",
        ],
    },
    RuleClaim {
        family: "state",
        target: "transition-marker",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "fill-opacity",
            "opacity",
            "stroke-dasharray",
            "stroke-line-cap",
            "stroke-line-join",
            "stroke-opacity",
            "stroke-paint",
            "stroke-width",
        ],
    },
    RuleClaim {
        family: "state",
        target: "transition",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "fill-opacity",
            "opacity",
            "stroke-dasharray",
            "stroke-line-cap",
            "stroke-line-join",
            "stroke-opacity",
            "stroke-paint",
            "stroke-width",
        ],
    },
    RuleClaim {
        family: "swimlane",
        target: "cluster",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint", "stroke-width"],
    },
    RuleClaim {
        family: "swimlane",
        target: "edge-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &["font-size", "font-stack", "font-weight", "padding"],
    },
    RuleClaim {
        family: "swimlane",
        target: "edge",
        kind: SupportClaimKind::TypedPartial,
        facets: &["effect", "stroke-dasharray", "stroke-paint", "stroke-width"],
    },
    RuleClaim {
        family: "swimlane",
        target: "node-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "font-size", "font-stack", "font-weight"],
    },
    RuleClaim {
        family: "swimlane",
        target: "node",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "effect",
            "fill",
            "radius",
            "stroke-dasharray",
            "stroke-paint",
            "stroke-width",
        ],
    },
    RuleClaim {
        family: "timeline",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "timeline",
        target: "timeline-event",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "opacity", "radius", "stroke-paint"],
    },
    RuleClaim {
        family: "treemap",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "treemap",
        target: "title",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "treeView",
        target: "edge",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint", "stroke-width"],
    },
    RuleClaim {
        family: "treeView",
        target: "marker",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "treeView",
        target: "node-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "treeView",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "venn",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "venn",
        target: "title",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "zenuml",
        target: "title",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "flowchart",
        target: "node",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "gitGraph",
        target: "node",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "kanban",
        target: "task",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "mindmap",
        target: "node",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "pie",
        target: "pie-slice",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "radar",
        target: "chart-series",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "sankey",
        target: "node",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "state",
        target: "composite-label",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "state",
        target: "composite",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "state",
        target: "note-label",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "state",
        target: "note",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "state",
        target: "special-state",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "state",
        target: "state-label",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "state",
        target: "state",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "state",
        target: "transition-label",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "state",
        target: "transition",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "timeline",
        target: "timeline-event",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "journey",
        target: "journey-task",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "swimlane",
        target: "node",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "xychart",
        target: "chart-series",
        kind: SupportClaimKind::TypedSurface,
        facets: &["ordinal-palette"],
    },
    RuleClaim {
        family: "block",
        target: "cluster-label",
        kind: SupportClaimKind::Unsupported,
        facets: &["fill"],
    },
    RuleClaim {
        family: "block",
        target: "cluster",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "block",
        target: "edge-label-background",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "block",
        target: "edge",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "block",
        target: "marker",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "block",
        target: "node-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "block",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "block",
        target: "title",
        kind: SupportClaimKind::Unsupported,
        facets: &["fill"],
    },
    RuleClaim {
        family: "c4",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "class",
        target: "cluster",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "class",
        target: "edge-label-background",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "class",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "class",
        target: "title",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "cynefin",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "er",
        target: "table",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "er",
        target: "title",
        kind: SupportClaimKind::Unsupported,
        facets: &["fill"],
    },
    RuleClaim {
        family: "flowchart",
        target: "cluster-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "flowchart",
        target: "edge-label-background",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "flowchart",
        target: "edge",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "flowchart",
        target: "marker",
        kind: SupportClaimKind::Unsupported,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "flowchart",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "flowchart",
        target: "title",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "gitGraph",
        target: "edge-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "gitGraph",
        target: "edge",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "gitGraph",
        target: "node-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "gitGraph",
        target: "node",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "gitGraph",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "journey",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "journey",
        target: "title",
        kind: SupportClaimKind::Unsupported,
        facets: &["fill"],
    },
    RuleClaim {
        family: "kanban",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "kanban",
        target: "title",
        kind: SupportClaimKind::Unsupported,
        facets: &["fill"],
    },
    RuleClaim {
        family: "mindmap",
        target: "node",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "pie",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "quadrantChart",
        target: "axis",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "quadrantChart",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "quadrantChart",
        target: "title",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "radar",
        target: "axis",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "radar",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "radar",
        target: "title",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "railroad",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "railroad",
        target: "title",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "requirement",
        target: "relation",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "requirement",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "sankey",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "sequence",
        target: "message",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "sequence",
        target: "text",
        kind: SupportClaimKind::Unsupported,
        facets: &["fill"],
    },
    RuleClaim {
        family: "sequence",
        target: "title",
        kind: SupportClaimKind::Unsupported,
        facets: &["fill"],
    },
    RuleClaim {
        family: "swimlane",
        target: "cluster-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "swimlane",
        target: "edge-label-background",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "swimlane",
        target: "edge",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "swimlane",
        target: "marker",
        kind: SupportClaimKind::Unsupported,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "swimlane",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "swimlane",
        target: "title",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "timeline",
        target: "title",
        kind: SupportClaimKind::Unsupported,
        facets: &["fill"],
    },
    RuleClaim {
        family: "xychart",
        target: "chart-series",
        kind: SupportClaimKind::TypedPartial,
        facets: &[
            "fill",
            "stroke-paint",
            "stroke-width",
            "opacity",
            "fill-opacity",
            "stroke-opacity",
            "effect",
        ],
    },
    RuleClaim {
        family: "xychart",
        target: "axis",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint"],
    },
    RuleClaim {
        family: "xychart",
        target: "text",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill"],
    },
    RuleClaim {
        family: "xychart",
        target: "title",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "font-size", "font-weight", "effect"],
    },
    RuleClaim {
        family: "xychart",
        target: "axis-title",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "font-size", "font-weight", "effect"],
    },
    RuleClaim {
        family: "xychart",
        target: "axis-label",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "font-size", "font-weight", "effect"],
    },
    RuleClaim {
        family: "xychart",
        target: "axis-tick",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "stroke-paint", "opacity"],
    },
    RuleClaim {
        family: "xychart",
        target: "legend",
        kind: SupportClaimKind::TypedPartial,
        facets: &["fill", "font-size", "font-weight", "effect"],
    },
];

const BASE_CLAIMS: &[BaseClaim] = &[
    BaseClaim {
        family: "architecture",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-size", "font-stack"],
    },
    BaseClaim {
        family: "cynefin",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-stack"],
    },
    BaseClaim {
        family: "error",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-stack"],
    },
    BaseClaim {
        family: "flowchart",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-size", "font-stack"],
    },
    BaseClaim {
        family: "info",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-stack"],
    },
    BaseClaim {
        family: "packet",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-stack"],
    },
    BaseClaim {
        family: "railroad",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-size", "font-stack"],
    },
    BaseClaim {
        family: "sequence",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-size", "font-stack"],
    },
    BaseClaim {
        family: "state",
        kind: SupportClaimKind::TypedSurface,
        properties: &[
            "font-size",
            "font-stack",
            "font-style",
            "font-weight",
            "letter-spacing",
            "transform",
            "word-spacing",
        ],
    },
    BaseClaim {
        family: "swimlane",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-size", "font-stack"],
    },
    BaseClaim {
        family: "wardley",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-stack"],
    },
    BaseClaim {
        family: "block",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-size", "font-stack"],
    },
    BaseClaim {
        family: "c4",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-size", "font-stack"],
    },
    BaseClaim {
        family: "class",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-size", "font-stack"],
    },
    BaseClaim {
        family: "er",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-size"],
    },
    BaseClaim {
        family: "er",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-stack"],
    },
    BaseClaim {
        family: "eventmodeling",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-size", "font-stack"],
    },
    BaseClaim {
        family: "gantt",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-stack"],
    },
    BaseClaim {
        family: "gitGraph",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-size", "font-stack"],
    },
    BaseClaim {
        family: "ishikawa",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-size", "font-stack"],
    },
    BaseClaim {
        family: "journey",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-size", "font-stack"],
    },
    BaseClaim {
        family: "kanban",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-size", "font-stack"],
    },
    BaseClaim {
        family: "mindmap",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-stack"],
    },
    BaseClaim {
        family: "pie",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-stack"],
    },
    BaseClaim {
        family: "pie",
        kind: SupportClaimKind::Unsupported,
        properties: &["font-size"],
    },
    BaseClaim {
        family: "quadrantChart",
        kind: SupportClaimKind::Unsupported,
        properties: &["font-size"],
    },
    BaseClaim {
        family: "quadrantChart",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-stack"],
    },
    BaseClaim {
        family: "radar",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-size", "font-stack"],
    },
    BaseClaim {
        family: "requirement",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-size"],
    },
    BaseClaim {
        family: "requirement",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-stack"],
    },
    BaseClaim {
        family: "sankey",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-stack"],
    },
    BaseClaim {
        family: "timeline",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-size"],
    },
    BaseClaim {
        family: "timeline",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-stack"],
    },
    BaseClaim {
        family: "treemap",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-stack"],
    },
    BaseClaim {
        family: "treemap",
        kind: SupportClaimKind::Unsupported,
        properties: &["font-size"],
    },
    BaseClaim {
        family: "treeView",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-stack"],
    },
    BaseClaim {
        family: "venn",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-stack"],
    },
    BaseClaim {
        family: "xychart",
        kind: SupportClaimKind::TypedSurface,
        properties: &["font-stack"],
    },
    BaseClaim {
        family: "xychart",
        kind: SupportClaimKind::Unsupported,
        properties: &["font-size"],
    },
];

/// Returns the manifest claim for one semantic rule facet.
pub(super) fn rule_claim(
    family: DiagramFamilyId,
    target: super::semantic::ThemeTarget,
    facet: ThemeRuleFacetV1,
) -> SupportClaimKind {
    rule_claim_for_ids(family.as_str(), target.id(), facet.id())
}

fn rule_claim_for_ids(family: &str, target: &str, facet: &str) -> SupportClaimKind {
    RULE_CLAIMS
        .iter()
        .find(|claim| {
            claim.family == family && claim.target == target && claim.facets.contains(&facet)
        })
        .map_or_else(
            || {
                if is_catalog_family_id(family)
                    && MANIFEST_TARGET_IDS.contains(&target)
                    && MANIFEST_RULE_FACET_IDS.contains(&facet)
                {
                    SupportClaimKind::Unsupported
                } else {
                    SupportClaimKind::Missing
                }
            },
            |claim| claim.kind,
        )
}

/// Returns the manifest claim for one semantic target's ordinal palette.
pub(super) fn ordinal_claim(
    family: DiagramFamilyId,
    target: super::semantic::ThemeTarget,
) -> SupportClaimKind {
    ordinal_claim_for_ids(family.as_str(), target.id())
}

fn ordinal_claim_for_ids(family: &str, target: &str) -> SupportClaimKind {
    RULE_CLAIMS
        .iter()
        .find(|claim| {
            claim.family == family
                && claim.target == target
                && claim.facets.contains(&"ordinal-palette")
        })
        .map_or_else(
            || {
                if is_catalog_family_id(family) && MANIFEST_TARGET_IDS.contains(&target) {
                    SupportClaimKind::Unsupported
                } else {
                    SupportClaimKind::Missing
                }
            },
            |claim| claim.kind,
        )
}

/// Returns the manifest claim for one family-wide base typography property.
pub(super) fn base_typography_claim(
    family: DiagramFamilyId,
    property: ThemeSupportBaseTypographyPropertyV1,
) -> SupportClaimKind {
    base_typography_claim_for_ids(family.as_str(), property.id())
}

fn base_typography_claim_for_ids(family: &str, property: &str) -> SupportClaimKind {
    BASE_CLAIMS
        .iter()
        .find(|claim| claim.family == family && claim.properties.contains(&property))
        .map_or_else(
            || {
                if is_catalog_family_id(family) && MANIFEST_BASE_PROPERTY_IDS.contains(&property) {
                    SupportClaimKind::Unsupported
                } else {
                    SupportClaimKind::Missing
                }
            },
            |claim| claim.kind,
        )
}

fn is_catalog_family_id(family: &str) -> bool {
    DiagramFamilyId::from_id(family).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::family_mechanism_matrix::{
        FamilyThemeSupportSummary, summarize_base_typography_support, summarize_theme_support,
    };
    use crate::diagram_theme::semantic::ThemeTarget;
    use merman_theme_contract::{ThemeSupportBaseTypographyPropertyV1, ThemeSupportFacetV1};

    #[test]
    fn manifest_surface_covers_current_contract_catalogs() {
        for &family in DiagramFamilyId::all() {
            assert!(is_catalog_family_id(family.as_str()));
        }
        for &target in ThemeTarget::ALL {
            assert!(
                MANIFEST_TARGET_IDS.contains(&target.id()),
                "target missing from support manifest: {}",
                target.id()
            );
        }
        for &facet in ThemeRuleFacetV1::ALL {
            assert!(
                MANIFEST_RULE_FACET_IDS.contains(&facet.id()),
                "rule facet missing from support manifest: {}",
                facet.id()
            );
        }
        for &property in ThemeSupportBaseTypographyPropertyV1::ALL {
            assert!(
                MANIFEST_BASE_PROPERTY_IDS.contains(&property.id()),
                "base property missing from support manifest: {}",
                property.id()
            );
        }
    }

    #[test]
    fn manifest_claim_rows_reference_only_catalog_families() {
        for claim in RULE_CLAIMS {
            assert!(
                is_catalog_family_id(claim.family),
                "rule claim references unknown family: {}",
                claim.family
            );
        }
        for claim in BASE_CLAIMS {
            assert!(
                is_catalog_family_id(claim.family),
                "base claim references unknown family: {}",
                claim.family
            );
        }
    }

    #[test]
    fn manifest_claim_keys_are_unique() {
        use std::collections::BTreeSet;

        let mut rule_keys = BTreeSet::new();
        for claim in RULE_CLAIMS {
            for &facet in claim.facets {
                assert!(
                    rule_keys.insert((claim.family, claim.target, facet)),
                    "duplicate rule claim key: {}|{}|{}",
                    claim.family,
                    claim.target,
                    facet,
                );
            }
        }

        let mut base_keys = BTreeSet::new();
        for claim in BASE_CLAIMS {
            for &property in claim.properties {
                assert!(
                    base_keys.insert((claim.family, property)),
                    "duplicate base typography claim key: {}|{}",
                    claim.family,
                    property,
                );
            }
        }
    }

    #[test]
    fn unknown_family_ids_fail_closed_to_missing() {
        assert_eq!(
            rule_claim_for_ids("future-family", "node", "fill"),
            SupportClaimKind::Missing
        );
        assert_eq!(
            ordinal_claim_for_ids("future-family", "node"),
            SupportClaimKind::Missing
        );
        assert_eq!(
            base_typography_claim_for_ids("future-family", "font-size"),
            SupportClaimKind::Missing
        );
    }

    #[test]
    fn venn_base_typography_claims_match_the_family_boundary() {
        assert_eq!(
            base_typography_claim_for_ids("venn", "font-stack"),
            SupportClaimKind::TypedSurface
        );
        assert_eq!(
            base_typography_claim_for_ids("venn", "font-size"),
            SupportClaimKind::Unsupported
        );
    }

    #[test]
    fn manifest_reconciles_private_matrix_exactly() {
        for &family in DiagramFamilyId::all() {
            for &target in ThemeTarget::ALL {
                if target == ThemeTarget::Canvas || !target.valid_for(family) {
                    continue;
                }
                for &facet in ThemeRuleFacetV1::ALL {
                    let expected =
                        summarize_theme_support(family, target, ThemeSupportFacetV1::Rule(facet));
                    assert_eq!(
                        rule_claim(family, target, facet),
                        claim_kind(expected),
                        "rule claim drift for {}|{}|{}",
                        family.as_str(),
                        target.id(),
                        facet.id(),
                    );
                }
                let expected =
                    summarize_theme_support(family, target, ThemeSupportFacetV1::OrdinalPalette);
                assert_eq!(
                    ordinal_claim(family, target),
                    claim_kind(expected),
                    "ordinal claim drift for {}|{}",
                    family.as_str(),
                    target.id(),
                );
            }

            for &property in ThemeSupportBaseTypographyPropertyV1::ALL {
                let expected = summarize_base_typography_support(family, property);
                assert_eq!(
                    base_typography_claim(family, property),
                    claim_kind(expected),
                    "base typography claim drift for {}|{}",
                    family.as_str(),
                    property.id(),
                );
            }
        }
    }

    fn claim_kind(summary: FamilyThemeSupportSummary) -> SupportClaimKind {
        assert!(
            !summary.has_legacy(),
            "retired family compatibility must not supply a production support claim"
        );
        if summary.has_typed() {
            if summary.has_unsupported() {
                SupportClaimKind::TypedPartial
            } else {
                SupportClaimKind::TypedSurface
            }
        } else {
            assert!(
                summary.has_unsupported(),
                "matrix returned an empty claim summary"
            );
            SupportClaimKind::Unsupported
        }
    }
}
