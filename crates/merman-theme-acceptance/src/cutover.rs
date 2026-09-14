use std::collections::{BTreeMap, BTreeSet};

use crate::cutover_manifest::{EXPECTED_AUTHORIZED_MANIFEST_DIGEST, authorize_cutover_routes};
use crate::observation::{RouteCutoverRuntimeError, append_len_prefixed, sha256};
use crate::runner::{
    C6ProofError, C6ProofResult, FamilyEvidenceRequirements, portable_svg_request,
    prove_portable_family_evidence,
};
use merman::svg::{
    CanvasPaint, CanvasSpec, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontAssetSpec,
    FontCatalogSpec, FontEmbeddingRequirement, FontSource, GenericFontFamily, ThemeAssets,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use merman::{
    DiagramFamilyId, Engine, MermaidConfig, OperationControl, RenderArtifactKind, RenderOutput,
    RenderRequest, Renderer, TargetAdmissionReason, TargetAdmissionReceipt, TargetAdmissionStatus,
    TargetFontSource,
};
use merman_export::RasterOptions;
use merman_render::__private::{
    ThemeRouteCutoverDescriptor, ThemeRouteCutoverFacet, ThemeRouteCutoverProjection,
    ThemeRouteCutoverProjectionSet, ThemeRouteCutoverSelector, ThemeRouteCutoverValue,
    legacy_replacing_typed_theme_routes,
};

const FLOWCHART_TEXT_SOURCE: &str = "---\ntitle: Diagram title\n---\nflowchart TD\nsubgraph Group[Group title]\nA[Alpha] -->|Advance| B[Beta]\nend\n";
const SWIMLANE_TEXT_SOURCE: &str = "---\ntitle: Diagram title\nconfig:\n  layout: swimlane\n---\nflowchart TD\nsubgraph Group[Group title]\nA[Alpha] -->|Advance| B[Beta]\nend\n";
const FLOWCHART_NODE_SOURCE: &str = "flowchart LR\nA[Alpha]\nB[Beta]\n";
const FLOWCHART_NODE_LABEL_SOURCE: &str = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A[Alpha]
B[Beta]
"#;
const FLOWCHART_EDGE_SOURCE: &str =
    "flowchart LR\nA[Alpha] o--o B[Beta]\nB x--x C[Gamma]\nC <--> D[Delta]\n";
const FLOWCHART_ANIMATED_EDGE_SOURCE: &str = "flowchart LR\nA[Alpha] circles@o--o B[Beta]\nB crosses@x--x C[Gamma]\nC points@<--> D[Delta]\ncircles@{ animate: true }\ncrosses@{ animate: true }\npoints@{ animate: true }\n";
const FLOWCHART_CLUSTER_SOURCE: &str = "flowchart TD\nsubgraph Group[Group]\nA[Alpha]\nend\n";
const SWIMLANE_NODE_SOURCE: &str = r#"---
config:
  layout: swimlane
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart TD
A[Alpha]
B[Beta]
"#;
const SWIMLANE_EDGE_SOURCE: &str = r#"---
config:
  layout: swimlane
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart TD
A[Alpha] o--o B[Beta]
B x--x C[Gamma]
C <--> D[Delta]
"#;
const SWIMLANE_ANIMATED_EDGE_SOURCE: &str = r#"---
config:
  layout: swimlane
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart TD
A[Alpha] circles@o--o B[Beta]
B crosses@x--x C[Gamma]
C points@<--> D[Delta]
circles@{ animate: true }
crosses@{ animate: true }
points@{ animate: true }
"#;
const SWIMLANE_CLUSTER_SOURCE: &str = r#"---
config:
  layout: swimlane
---
flowchart TD
subgraph Lane[Lane]
A[Alpha]
B[Beta]
end
"#;
const CLASS_NODE_SOURCE: &str = "classDiagram\nclass Alpha\nclass Beta\n";
const CLASS_TEXT_SOURCE: &str = r#"---
title: Class text terminals
---
classDiagram
class Alpha {
  +String id
  +render() String
}
class Beta
namespace Internal {
  class Gamma
}
Alpha "1" --> "many" Beta : relates
note for Alpha "annotated"
"#;
const ER_TEXT_SOURCE: &str =
    "---\ntitle: Entity relationship title\n---\nerDiagram\n  CUSTOMER ||--o{ ORDER : places\n";
const ER_TABLE_SOURCE: &str = "erDiagram\n  CUSTOMER {\n    string id\n    string name\n  }\n";
const INFO_TEXT_SOURCE: &str = "info\n";
const ARCHITECTURE_TEXT_SOURCE: &str = r#"architecture-beta
  group core(cloud)[Core]
  service api(server)[API] in core
  service worker(server)[Worker] in core
  service db(database)[Database] in core
  api:R -[calls]-> L:worker
"#;
const SEQUENCE_FILL_SOURCE: &str = r#"sequenceDiagram
participant Plain
participant Stick@{"type":"actor"}
participant Boundary@{"type":"boundary"}
participant Entity@{"type":"entity"}
participant Collection@{"type":"collections"}
participant Queue@{"type":"queue"}
participant Database@{"type":"database"}
Plain->>Database: Hello
"#;
const SEQUENCE_STROKE_SOURCE: &str = r#"sequenceDiagram
participant Plain
participant Stick@{"type":"actor"}
participant Boundary@{"type":"boundary"}
participant Entity@{"type":"entity"}
participant Collection@{"type":"collections"}
participant Queue@{"type":"queue"}
Plain->>Queue: Hello
"#;
const SEQUENCE_LIFELINE_SOURCE: &str = r#"sequenceDiagram
actor Alice
participant Bob
Alice->>Bob: Hello
"#;
const SEQUENCE_ROLE_LABEL_SOURCE: &str = r#"sequenceDiagram
box rgb(241,245,249) Team
participant Alice
participant Bob
end
Alice->>Bob: Hello
"#;
const SEQUENCE_NOTE_SOURCE: &str = r#"sequenceDiagram
participant Alice
participant Bob
Note left of Alice: Left note
Note over Alice,Bob: Shared note
Note right of Bob: Right note
Alice->>Bob: Hello
"#;
const SEQUENCE_ACTIVATION_SOURCE: &str = r#"sequenceDiagram
participant Alice
participant Bob
Alice->>Bob: Outer request
activate Bob
Alice->>Bob: Middle request
activate Bob
Alice->>Bob: Inner request
activate Bob
Bob-->>Alice: Inner response
deactivate Bob
Bob-->>Alice: Middle response
deactivate Bob
Bob-->>Alice: Outer response
deactivate Bob
"#;
const SEQUENCE_MESSAGE_SOURCE: &str = r#"sequenceDiagram
autonumber
participant A
participant B
A<<->>B: bidirectional arrow
B--xA: dotted cross
A--)B: dotted filled
B-|\A: solid top
A-|/B: solid bottom
B-\\A: stick top
A-//B: stick bottom
"#;
const SEQUENCE_LOOP_SOURCE: &str = r#"sequenceDiagram
participant Alice
participant Bob
alt Primary path
Alice->>Bob: Ping
else Secondary path
Bob-->>Alice: Pong
end
"#;
const XYCHART_TITLE_SOURCE: &str = r#"xychart-beta
 title Cutover xychart title
 x-axis [A, B]
 y-axis 0 --> 10
 bar [3, 7]
"#;
const XYCHART_PAINT_SOURCE: &str = r#"---
config:
  xyChart:
    showDataLabel: true
    showDataLabelOutsideBar: true
---
xychart-beta
 title Cutover xychart paint
 x-axis Categories [A, B]
 y-axis Values 0 --> 10
 bar [3, 7]
"#;
#[cfg(test)]
const XYCHART_HORIZONTAL_PAINT_SOURCE: &str = r#"---
config:
  xyChart:
    showDataLabel: true
    showDataLabelOutsideBar: true
---
xychart-beta horizontal
 title Cutover xychart paint
 x-axis Categories [A, B]
 y-axis Values 0 --> 10
 bar [3, 7]
"#;
const TREEMAP_TITLE_SOURCE: &str = r#"treemap
title Cutover treemap title
"Section"
  "Leaf": 1
"#;
const TREEMAP_TEXT_SOURCE: &str = r#"treemap
"Section"
  "Leaf": 1
"#;
const RAILROAD_TITLE_SOURCE: &str = r#"railroad-beta
expr = sequence(nonterminal("term"), terminal("+"), special("guard")) ;
"#;
const RADAR_TITLE_SOURCE: &str = r#"radar-beta
title Cutover radar title
axis A,B,C
curve Current{3,4,2}
"#;
// Isolate the axis stroke from later graticule/curve paint. Four cardinal axes provide
// horizontal and vertical native raster terminals for both Axis.fill fallback and Axis.stroke.
const RADAR_AXIS_SOURCE: &str = r#"---
config:
  radar:
    graticuleColor: transparent
    curveOpacity: 0
    curveStrokeWidth: 0
---
radar-beta
axis North,East,South,West
curve Current{3,4,2,3}
"#;
// Exercise all inherited text terminals and the independently owned title fallback.
// Keep unrelated chart paint transparent so native text proofs observe glyph ink.
const RADAR_TEXT_SOURCE: &str = r#"---
config:
  radar:
    axisColor: transparent
    graticuleColor: transparent
    curveOpacity: 0
    curveStrokeWidth: 0
---
radar-beta
title Cutover radar text
axis North,East,South,West
curve Current{3,4,2,3}
"#;
// Links are painted after labels in Mermaid's Sankey renderer. Keep this witness's links
// transparent so the route-local PNG proof observes the intended text terminal rather than a
// later, opaque link stroke covering the glyphs.
const SANKEY_TEXT_SOURCE: &str =
    "---\nconfig:\n  sankey:\n    linkColor: transparent\n---\nsankey-beta\nA,B,10\n";
const REQUIREMENT_PAINT_SOURCE: &str = r#"requirementDiagram
requirement req1 {
  id: 1
  text: Cutover requirement
  risk: high
  verifymethod: analysis
}
"#;
const REQUIREMENT_RELATION_SOURCE: &str = r#"requirementDiagram
requirement req1 {
  id: 1
  text: Required behavior
  risk: high
  verifymethod: analysis
}
element impl {
  type: simulation
}
req1 - contains -> impl
impl - satisfies -> req1
"#;
const PIE_SLICE_SOURCE: &str = r#"pie
  "Alpha" : 3
  "Beta" : 2
"#;
const PIE_TITLE_SOURCE: &str = r#"pie title Cutover pie title
  "Alpha" : 3
  "Beta" : 2
"#;
const PIE_TEXT_SOURCE: &str = r#"pie
  "Alpha" : 3
  "Beta" : 2
"#;
const BLOCK_NODE_SOURCE: &str = r#"block-beta
  columns 5
  rect["Rect"] circle(("Circle")) double((("Double"))) cylinder[("Cylinder")] polygon{{"Polygon"}}
"#;
// Empty cells keep the line terminals visible outside node and marker geometry.
const BLOCK_EDGE_SOURCE: &str = "block-beta\n columns 3\n A[\"Alpha\"] space B[\"Beta\"]\n C[\"Gamma\"] space D[\"Delta\"]\n A --> B\n C -- \"connects\" --> D\n";
const BLOCK_EDGE_LABEL_SOURCE: &str = "block-beta\n  A[\"Alpha\"] -- \"relates\" --> B[\"Beta\"]\n";
const ZENUML_TITLE_SOURCE: &str = "zenuml\ntitle Cutover ZenUML title\nClient->Service: request\n";
const VENN_THEME_SOURCE: &str = r#"venn-beta
title Cutover Venn title
set A["Alpha"]:20
set B["Beta"]:12
union A,B["Shared"]:3
"#;
const ISHIKAWA_TEXT_SOURCE: &str = "ishikawa-beta\n Root cause\n  Process\n   Slow step\n   Missing spec\n  People\n   Missing owner\n";
const EVENT_MODELING_TEXT_SOURCE: &str =
    "eventmodeling\ntf 01 ui View\ntf 02 cmd Run ->> 01\ntf 03 evt Done ->> 02\n";
const CYNEFIN_TEXT_SOURCE: &str = r#"---
config:
  cynefin:
    showDomainDescriptions: false
---
cynefin-beta
clear
"Runbook"
complex
"Retrospective"
clear --> complex : "Probe"
"#;
const CLASS_CLUSTER_SOURCE: &str = r#"classDiagram
namespace Internal {
  class A
  class B
}
namespace External {
  class C
  class D
}
A --> C : relation
note for A "attached note"
"#;
const CLASS_EDGE_SOURCE: &str = r#"classDiagram
  A *-- B
  C <|-- D
  E ..> F
  G o-- H
  I ()-- J
  note for A "attached paint terminal"
"#;
const ER_RELATION_SOURCE: &str = r#"erDiagram
  A ||--o{ B : owns
  SELF ||--o{ SELF : refers
"#;
const MINDMAP_PAINT_SOURCE: &str = r#"mindmap
  Root
    First
      First child
    Second
"#;
const GITGRAPH_TEXT_SOURCE: &str = r#"---
title: GitGraph inherited text
---
gitGraph
  commit id: "visible commit" tag: "visible tag"
"#;
const GITGRAPH_NODE_PAINT_SOURCE: &str = r#"gitGraph
  commit id: "state" type: REVERSE tag: "visible tag"
"#;
const GITGRAPH_EDGE_SOURCE: &str = r#"gitGraph
  commit id: "1"
  branch develop
  checkout develop
  commit id: "2"
  checkout main
  commit id: "3"
"#;
const GANTT_TASK_FILL_SOURCE: &str = r#"gantt
dateFormat YYYY-MM-DD
todayMarker off
section Delivery
Default: default-task, 2024-01-01, 1d
Active: active, active-task, 2024-01-02, 1d
Done: done, done-task, 2024-01-03, 1d
Critical: crit, crit-task, 2024-01-04, 1d
Active critical: crit, active, active-crit-task, 2024-01-05, 1d
Done critical: crit, done, done-crit-task, 2024-01-06, 1d
"#;
const GANTT_TITLE_SOURCE: &str = r#"gantt
title Cutover Gantt title
dateFormat YYYY-MM-DD
todayMarker off
section Delivery
Task: task, 2024-01-01, 1d
"#;
const GANTT_WARNING_TODAY_SOURCE: &str = r#"gantt
dateFormat YYYY-MM-DD
section Delivery
Task: regular-task, 2024-01-01, 4d
"#;
const GANTT_WARNING_VERTICAL_SOURCE: &str = r#"gantt
dateFormat YYYY-MM-DD
todayMarker off
section Delivery
Vertical marker: vert, vertical-marker, 2024-01-02, 0d
Task: regular-task, 2024-01-01, 4d
"#;
const JOURNEY_TASK_PAINT_SOURCE: &str = r#"journey
  section Delivery
    Ship release: 5: Maintainer
"#;
const JOURNEY_TEXT_PAINT_SOURCE: &str = r#"journey
  title Cutover Journey text
  section Delivery
    Ship release: 5: Maintainer
"#;
const TIMELINE_EVENT_FILL_SOURCE: &str = r#"timeline
  title Cutover Timeline event
  section Delivery
    Ship release : Completed
"#;
// This witness proves only the shared Redux activity-axis paint. Node and label fanout are
// covered by family-owned receipts and SVG integration tests, not by this native PNG pair.
const TIMELINE_EVENT_STROKE_AXIS_SOURCE: &str =
    "timeline\n  title Cutover Timeline activity axis\n";
const TREE_VIEW_PAINT_SOURCE: &str = "treeView-beta\nRoot/\n    Child\n";
const TREE_VIEW_MARKER_PAINT_SOURCE: &str = "treeView-beta\nRoot icon(folder)\n";
const KANBAN_TEXT_FILL_SOURCE: &str = r#"kanban
  todo[Delivery]
    task[Ship release]@{ ticket: MC-2038, assigned: 'Maintainer' }
"#;
const KANBAN_TASK_STROKE_SOURCE: &str = r#"kanban
  todo[Todo]
    first[First]
    second[Second]
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum CutoverWitnessProfile {
    ClassicStatic,
    ClassicToday,
    ClassicVertical,
    HandDrawnStatic,
    NeoStatic,
    NeoAnimated,
    ReduxStatic,
}

impl CutoverWitnessProfile {
    const CLASSIC: [Self; 1] = [Self::ClassicStatic];
    const TIMELINE_EVENT_STROKE: [Self; 1] = [Self::ReduxStatic];
    const TEXT_LOOKS: [Self; 3] = [Self::ClassicStatic, Self::NeoStatic, Self::HandDrawnStatic];
    const GANTT_WARNING: [Self; 2] = [Self::ClassicToday, Self::ClassicVertical];
    const CLUSTER: [Self; 2] = [Self::ClassicStatic, Self::HandDrawnStatic];
    const EDGE: [Self; 3] = [Self::ClassicStatic, Self::NeoStatic, Self::NeoAnimated];
    const CLASS_EDGE: [Self; 2] = [Self::ClassicStatic, Self::HandDrawnStatic];
    const ISHIKAWA_TEXT: [Self; 2] = [Self::ClassicStatic, Self::HandDrawnStatic];
    // Mindmap Node.fill historically projected `mainBkg`, which reaches a branch shape only in
    // the Redux Neo writer path. The native Node.stroke witness stays on Neo so shape/edge stroke
    // terminals, rather than the Classic root-label fanout, prove the route.
    const MINDMAP_NODE: [Self; 1] = [Self::NeoStatic];

    const fn id(self) -> &'static str {
        match self {
            Self::ClassicStatic => "classic-static",
            Self::ClassicToday => "classic-today",
            Self::ClassicVertical => "classic-vertical",
            Self::HandDrawnStatic => "hand-drawn-static",
            Self::NeoStatic => "neo-static",
            Self::NeoAnimated => "neo-animated",
            Self::ReduxStatic => "redux-static",
        }
    }

    const fn look(self) -> &'static str {
        match self {
            Self::ClassicStatic
            | Self::ClassicToday
            | Self::ClassicVertical
            | Self::ReduxStatic => "classic",
            Self::HandDrawnStatic => "handDrawn",
            Self::NeoStatic | Self::NeoAnimated => "neo",
        }
    }

    fn for_route(route: ThemeRouteCutoverDescriptor) -> &'static [Self] {
        if route.family_id() == DiagramFamilyId::CLASS
            && matches!(route.target(), ThemeTarget::Title | ThemeTarget::Text)
        {
            return &Self::TEXT_LOOKS;
        }
        if route.family_id() == DiagramFamilyId::BLOCK
            && matches!(
                route.target(),
                ThemeTarget::EdgeLabelBackground | ThemeTarget::NodeLabel | ThemeTarget::Edge
            )
        {
            return &Self::TEXT_LOOKS;
        }
        if matches!(
            route.family_id(),
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
        ) && matches!(
            route.target(),
            ThemeTarget::Title
                | ThemeTarget::ClusterLabel
                | ThemeTarget::Text
                | ThemeTarget::EdgeLabelBackground
        ) {
            return &Self::TEXT_LOOKS;
        }
        if (matches!(
            route.family_id(),
            DiagramFamilyId::C4 | DiagramFamilyId::KANBAN | DiagramFamilyId::TIMELINE
        ) && route.target() == ThemeTarget::Text)
            || (route.family_id() == DiagramFamilyId::GIT_GRAPH
                && matches!(
                    route.target(),
                    ThemeTarget::Text
                        | ThemeTarget::NodeLabel
                        | ThemeTarget::EdgeLabel
                        | ThemeTarget::Node
                ))
            || (route.family_id() == DiagramFamilyId::GIT_GRAPH
                && route.target() == ThemeTarget::Edge
                && route.facet() == ThemeRouteCutoverFacet::Fill)
        {
            return &Self::TEXT_LOOKS;
        }
        if route.family_id() == DiagramFamilyId::TIMELINE
            && route.target() == ThemeTarget::TimelineEvent
            && route.facet() == ThemeRouteCutoverFacet::Stroke
        {
            &Self::TIMELINE_EVENT_STROKE
        } else if route.family_id() == DiagramFamilyId::REQUIREMENT
            && matches!(route.target(), ThemeTarget::Relation | ThemeTarget::Text)
        {
            &Self::TEXT_LOOKS
        } else if route.family_id() == DiagramFamilyId::GANTT
            && route.target() == ThemeTarget::Task
            && route.facet() == ThemeRouteCutoverFacet::Stroke
            && route.selector() == ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Warning)
        {
            &Self::GANTT_WARNING
        } else if matches!(
            route.family_id(),
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
        ) && route.target() == ThemeTarget::Edge
        {
            &Self::EDGE
        } else if route.family_id() == DiagramFamilyId::CLASS && route.target() == ThemeTarget::Edge
        {
            &Self::CLASS_EDGE
        } else if route.family_id() == DiagramFamilyId::ISHIKAWA
            && route.target() == ThemeTarget::Text
        {
            &Self::ISHIKAWA_TEXT
        } else if route.family_id() == DiagramFamilyId::MINDMAP
            && route.target() == ThemeTarget::Node
        {
            &Self::MINDMAP_NODE
        } else if matches!(
            route.family_id(),
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::CLASS
        ) && route.target() == ThemeTarget::Cluster
        {
            &Self::CLUSTER
        } else {
            &Self::CLASSIC
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CutoverWitnessId {
    route: ThemeRouteCutoverDescriptor,
    profile: CutoverWitnessProfile,
}

impl CutoverWitnessId {
    const fn new(route: ThemeRouteCutoverDescriptor, profile: CutoverWitnessProfile) -> Self {
        Self { route, profile }
    }

    const fn route(self) -> ThemeRouteCutoverDescriptor {
        self.route
    }

    const fn profile(self) -> CutoverWitnessProfile {
        self.profile
    }
}

fn expected_cutover_witnesses(routes: &[ThemeRouteCutoverDescriptor]) -> Vec<CutoverWitnessId> {
    let mut witnesses = routes
        .iter()
        .copied()
        .flat_map(|route| {
            CutoverWitnessProfile::for_route(route)
                .iter()
                .copied()
                .map(move |profile| CutoverWitnessId::new(route, profile))
        })
        .collect::<Vec<_>>();
    witnesses.sort_unstable();
    witnesses
}

fn source_for_route(route: ThemeRouteCutoverDescriptor) -> C6ProofResult<&'static str> {
    match (route.family_id(), route.target(), route.facet()) {
        (
            DiagramFamilyId::QUADRANT_CHART,
            ThemeTarget::Text | ThemeTarget::Title | ThemeTarget::Axis,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(
            "quadrantChart\ntitle Priorities\nx-axis Low --> High\ny-axis Low --> High\nAlpha: [0.3, 0.4]\nBeta: [0.7, 0.8]\n",
        ),
        (
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::EdgeLabelBackground,
            ThemeRouteCutoverFacet::Fill,
        ) => Ok("flowchart TD\nA[Alpha] -->|Advance| B[Beta]\n"),
        (
            DiagramFamilyId::SWIMLANE,
            ThemeTarget::EdgeLabelBackground,
            ThemeRouteCutoverFacet::Fill,
        ) => Ok(
            "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA[Alpha] -->|Advance| B[Beta]\n",
        ),

        (DiagramFamilyId::FLOWCHART, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(FLOWCHART_TEXT_SOURCE)
        }
        (DiagramFamilyId::SWIMLANE, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(SWIMLANE_TEXT_SOURCE)
        }
        (
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::Title | ThemeTarget::ClusterLabel,
            ThemeRouteCutoverFacet::Fill,
        ) => Ok(FLOWCHART_CLUSTER_SOURCE),
        (
            DiagramFamilyId::SWIMLANE,
            ThemeTarget::Title | ThemeTarget::ClusterLabel,
            ThemeRouteCutoverFacet::Fill,
        ) => Ok(SWIMLANE_CLUSTER_SOURCE),

        (DiagramFamilyId::C4, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok("---\ntitle: C4 inherited paint\n---\nC4Context\nSystem(service, \"Service\")\n")
        }
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Node, _) => Ok(FLOWCHART_NODE_SOURCE),
        (DiagramFamilyId::FLOWCHART, ThemeTarget::NodeLabel, _) => Ok(FLOWCHART_NODE_LABEL_SOURCE),
        (
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::Edge,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(FLOWCHART_EDGE_SOURCE),
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Cluster, _) => Ok(FLOWCHART_CLUSTER_SOURCE),
        (DiagramFamilyId::SWIMLANE, ThemeTarget::Node | ThemeTarget::NodeLabel, _) => {
            Ok(SWIMLANE_NODE_SOURCE)
        }
        (
            DiagramFamilyId::SWIMLANE,
            ThemeTarget::Edge,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(SWIMLANE_EDGE_SOURCE),
        (
            DiagramFamilyId::SWIMLANE,
            ThemeTarget::Cluster,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(SWIMLANE_CLUSTER_SOURCE),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Actor, ThemeRouteCutoverFacet::Fill) => {
            Ok(SEQUENCE_FILL_SOURCE)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Actor, ThemeRouteCutoverFacet::Stroke) => {
            Ok(SEQUENCE_STROKE_SOURCE)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::ActorLabel, ThemeRouteCutoverFacet::Fill) => {
            Ok(SEQUENCE_ROLE_LABEL_SOURCE)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Lifeline, _) => Ok(SEQUENCE_LIFELINE_SOURCE),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Note, _) => Ok(SEQUENCE_NOTE_SOURCE),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::NoteLabel, ThemeRouteCutoverFacet::Fill) => {
            Ok(SEQUENCE_NOTE_SOURCE)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Activation, _) => Ok(SEQUENCE_ACTIVATION_SOURCE),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Message,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(SEQUENCE_MESSAGE_SOURCE),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::MessageLabel, ThemeRouteCutoverFacet::Fill) => {
            Ok(SEQUENCE_ROLE_LABEL_SOURCE)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Loop | ThemeTarget::LoopLabel, _) => {
            Ok(SEQUENCE_LOOP_SOURCE)
        }
        (
            DiagramFamilyId::XY_CHART,
            ThemeTarget::Text | ThemeTarget::Axis,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(XYCHART_PAINT_SOURCE),
        (DiagramFamilyId::XY_CHART, ThemeTarget::Title, ThemeRouteCutoverFacet::Fill) => {
            Ok(XYCHART_TITLE_SOURCE)
        }
        (DiagramFamilyId::TREEMAP, ThemeTarget::Title, ThemeRouteCutoverFacet::Fill) => {
            Ok(TREEMAP_TITLE_SOURCE)
        }
        (DiagramFamilyId::TREEMAP, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(TREEMAP_TEXT_SOURCE)
        }
        (DiagramFamilyId::RAILROAD, ThemeTarget::Title, ThemeRouteCutoverFacet::Fill) => {
            Ok(RAILROAD_TITLE_SOURCE)
        }
        (DiagramFamilyId::RAILROAD, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(RAILROAD_TITLE_SOURCE)
        }
        (
            DiagramFamilyId::RADAR,
            ThemeTarget::Axis,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(RADAR_AXIS_SOURCE),
        (DiagramFamilyId::RADAR, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(RADAR_TEXT_SOURCE)
        }
        (DiagramFamilyId::RADAR, ThemeTarget::Title, ThemeRouteCutoverFacet::Fill) => {
            Ok(RADAR_TITLE_SOURCE)
        }
        (DiagramFamilyId::SANKEY, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(SANKEY_TEXT_SOURCE)
        }
        (DiagramFamilyId::REQUIREMENT, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(REQUIREMENT_RELATION_SOURCE)
        }
        (
            DiagramFamilyId::REQUIREMENT,
            ThemeTarget::Requirement,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(REQUIREMENT_PAINT_SOURCE),
        (
            DiagramFamilyId::REQUIREMENT,
            ThemeTarget::Relation,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(REQUIREMENT_RELATION_SOURCE),
        (
            DiagramFamilyId::PIE,
            ThemeTarget::PieSlice,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(PIE_SLICE_SOURCE),
        (DiagramFamilyId::PIE, ThemeTarget::Title, ThemeRouteCutoverFacet::Fill) => {
            Ok(PIE_TITLE_SOURCE)
        }
        (DiagramFamilyId::PIE, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(PIE_TEXT_SOURCE)
        }
        (
            DiagramFamilyId::BLOCK,
            ThemeTarget::Node,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(BLOCK_NODE_SOURCE),
        (
            DiagramFamilyId::BLOCK,
            ThemeTarget::Edge,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(BLOCK_EDGE_SOURCE),
        (DiagramFamilyId::BLOCK, ThemeTarget::NodeLabel, ThemeRouteCutoverFacet::Fill) => {
            Ok(BLOCK_NODE_SOURCE)
        }
        (
            DiagramFamilyId::BLOCK,
            ThemeTarget::EdgeLabelBackground,
            ThemeRouteCutoverFacet::Fill,
        ) => Ok(BLOCK_EDGE_LABEL_SOURCE),
        (DiagramFamilyId::ZENUML, ThemeTarget::Title, ThemeRouteCutoverFacet::Fill) => {
            Ok(ZENUML_TITLE_SOURCE)
        }
        (
            DiagramFamilyId::VENN,
            ThemeTarget::Title | ThemeTarget::Text,
            ThemeRouteCutoverFacet::Fill,
        ) => Ok(VENN_THEME_SOURCE),
        (DiagramFamilyId::ISHIKAWA, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(ISHIKAWA_TEXT_SOURCE)
        }
        (DiagramFamilyId::EVENT_MODELING, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(EVENT_MODELING_TEXT_SOURCE)
        }
        (DiagramFamilyId::CYNEFIN, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(CYNEFIN_TEXT_SOURCE)
        }
        (DiagramFamilyId::CLASS, ThemeTarget::Title, ThemeRouteCutoverFacet::Fill) => {
            Ok(CLASS_CLUSTER_SOURCE)
        }
        (DiagramFamilyId::CLASS, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(CLASS_TEXT_SOURCE)
        }
        (
            DiagramFamilyId::CLASS,
            ThemeTarget::Cluster,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(CLASS_CLUSTER_SOURCE),
        (
            DiagramFamilyId::CLASS,
            ThemeTarget::Edge,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(CLASS_EDGE_SOURCE),
        (
            DiagramFamilyId::CLASS,
            ThemeTarget::Node,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(CLASS_NODE_SOURCE),
        (DiagramFamilyId::CLASS, ThemeTarget::NodeLabel, ThemeRouteCutoverFacet::Fill) => {
            Ok(CLASS_NODE_SOURCE)
        }
        (
            DiagramFamilyId::ER,
            ThemeTarget::Relation,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(ER_RELATION_SOURCE),
        (DiagramFamilyId::ER, ThemeTarget::Table, ThemeRouteCutoverFacet::Fill) => {
            Ok(ER_TABLE_SOURCE)
        }
        (DiagramFamilyId::ER, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(ER_TEXT_SOURCE)
        }
        (DiagramFamilyId::INFO, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(INFO_TEXT_SOURCE)
        }
        (DiagramFamilyId::ARCHITECTURE, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(ARCHITECTURE_TEXT_SOURCE)
        }
        (
            DiagramFamilyId::MINDMAP,
            ThemeTarget::Node,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        )
        | (DiagramFamilyId::MINDMAP, ThemeTarget::Edge, ThemeRouteCutoverFacet::Stroke) => {
            Ok(MINDMAP_PAINT_SOURCE)
        }
        (
            DiagramFamilyId::GIT_GRAPH,
            ThemeTarget::Node,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(GITGRAPH_NODE_PAINT_SOURCE),
        (
            DiagramFamilyId::GIT_GRAPH,
            ThemeTarget::Text | ThemeTarget::NodeLabel | ThemeTarget::EdgeLabel,
            ThemeRouteCutoverFacet::Fill,
        ) => Ok(GITGRAPH_TEXT_SOURCE),
        (
            DiagramFamilyId::GIT_GRAPH,
            ThemeTarget::Edge,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        )
        | (
            DiagramFamilyId::GIT_GRAPH,
            ThemeTarget::EdgeLabelBackground,
            ThemeRouteCutoverFacet::Fill,
        ) => Ok(GITGRAPH_EDGE_SOURCE),
        (DiagramFamilyId::GANTT, ThemeTarget::Title, ThemeRouteCutoverFacet::Fill) => {
            Ok(GANTT_TITLE_SOURCE)
        }
        (DiagramFamilyId::GANTT, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(GANTT_TASK_FILL_SOURCE)
        }
        (DiagramFamilyId::GANTT, ThemeTarget::Task, ThemeRouteCutoverFacet::Stroke)
            if route.selector()
                == ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Warning) =>
        {
            Ok(GANTT_WARNING_TODAY_SOURCE)
        }
        (
            DiagramFamilyId::GANTT,
            ThemeTarget::Task,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(GANTT_TASK_FILL_SOURCE),
        (
            DiagramFamilyId::JOURNEY,
            ThemeTarget::JourneyTask,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(JOURNEY_TASK_PAINT_SOURCE),
        (DiagramFamilyId::JOURNEY, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(JOURNEY_TEXT_PAINT_SOURCE)
        }
        (DiagramFamilyId::TIMELINE, ThemeTarget::TimelineEvent, ThemeRouteCutoverFacet::Fill) => {
            Ok(TIMELINE_EVENT_FILL_SOURCE)
        }
        (DiagramFamilyId::TIMELINE, ThemeTarget::TimelineEvent, ThemeRouteCutoverFacet::Stroke) => {
            Ok(TIMELINE_EVENT_STROKE_AXIS_SOURCE)
        }
        (DiagramFamilyId::TIMELINE, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(TIMELINE_EVENT_FILL_SOURCE)
        }
        (
            DiagramFamilyId::TREE_VIEW,
            ThemeTarget::Edge | ThemeTarget::NodeLabel | ThemeTarget::Text,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(TREE_VIEW_PAINT_SOURCE),
        (DiagramFamilyId::TREE_VIEW, ThemeTarget::Marker, _) => Ok(TREE_VIEW_MARKER_PAINT_SOURCE),
        (DiagramFamilyId::KANBAN, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Ok(KANBAN_TEXT_FILL_SOURCE)
        }
        (DiagramFamilyId::KANBAN, ThemeTarget::Task, ThemeRouteCutoverFacet::Stroke) => {
            Ok(KANBAN_TASK_STROKE_SOURCE)
        }
        _ => Err(C6ProofError::new(
            "route-source",
            format!("no route-cutover witness source for {}", route_label(route)),
        )),
    }
}

fn source_for_witness(witness: CutoverWitnessId) -> C6ProofResult<&'static str> {
    if witness.profile() == CutoverWitnessProfile::ClassicVertical {
        return Ok(GANTT_WARNING_VERTICAL_SOURCE);
    }
    if witness.profile() != CutoverWitnessProfile::NeoAnimated {
        return source_for_route(witness.route());
    }
    match (
        witness.route().family_id(),
        witness.route().target(),
        witness.route().facet(),
    ) {
        (
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::Edge,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(FLOWCHART_ANIMATED_EDGE_SOURCE),
        (
            DiagramFamilyId::SWIMLANE,
            ThemeTarget::Edge,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Ok(SWIMLANE_ANIMATED_EDGE_SOURCE),
        _ => Err(C6ProofError::new(
            "route-source",
            format!(
                "no animated route-cutover witness source for {}",
                witness_label(witness)
            ),
        )),
    }
}

#[derive(Clone, Copy)]
struct CutoverCase {
    id: CutoverWitnessId,
    source: &'static str,
}

struct RenderedCutoverCase {
    document: merman::RenderedDocument,
    svg_target_receipt_digest: [u8; 32],
    svg_route_receipt_digest: [u8; 32],
}

struct RenderedPngCutoverFacts {
    target_receipt_digest: [u8; 32],
    semantic_receipt_digest: [u8; 32],
}

/// Coarse evidence that the exact manifest-declared bridge routes passed their cutover gate.
///
/// The sealed route receipts and aggregate route-report digest remain private to this crate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteCutoverAuthorizationReport {
    manifest_digest: [u8; 32],
    authorization_digest: [u8; 32],
}

impl RouteCutoverAuthorizationReport {
    /// Returns the digest of the exact route-cutover manifest authorized by this run.
    pub const fn manifest_digest(&self) -> &[u8; 32] {
        &self.manifest_digest
    }

    /// Returns the sealed authorization digest for this run.
    pub const fn authorization_digest(&self) -> &[u8; 32] {
        &self.authorization_digest
    }
}

#[derive(Debug)]
struct RouteCutoverAuthorizationReceipt {
    manifest_digest: [u8; 32],
    route_report_digest: [u8; 32],
    digest: [u8; 32],
}

impl RouteCutoverAuthorizationReceipt {
    fn seal(manifest_digest: [u8; 32], route_report_digest: [u8; 32]) -> C6ProofResult<Self> {
        c6_ensure!(
            "route-authorization",
            manifest_digest != [0; 32],
            "cutover manifest digest is zero"
        );
        c6_ensure!(
            "route-authorization",
            route_report_digest != [0; 32],
            "aggregate route report digest is zero"
        );
        let digest = Self::canonical_digest(manifest_digest, route_report_digest);
        c6_ensure!(
            "route-authorization",
            digest != [0; 32],
            "route authorization digest is zero"
        );
        Ok(Self {
            manifest_digest,
            route_report_digest,
            digest,
        })
    }

    fn canonical_digest(manifest_digest: [u8; 32], route_report_digest: [u8; 32]) -> [u8; 32] {
        let mut value = b"merman.c6-route-cutover-authorization.v1\0".to_vec();
        append_len_prefixed(&mut value, b"manifest");
        value.extend_from_slice(&manifest_digest);
        append_len_prefixed(&mut value, b"aggregate-route-report");
        value.extend_from_slice(&route_report_digest);
        sha256(value)
    }

    #[cfg(test)]
    pub(crate) fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    fn into_report(self) -> RouteCutoverAuthorizationReport {
        debug_assert_eq!(
            self.digest,
            Self::canonical_digest(self.manifest_digest, self.route_report_digest)
        );
        RouteCutoverAuthorizationReport {
            manifest_digest: self.manifest_digest,
            authorization_digest: self.digest,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(manifest_digest: [u8; 32], route_report_digest: [u8; 32]) -> Self {
        Self::seal(manifest_digest, route_report_digest).expect("seal test route authorization")
    }
}

struct RouteCutoverReceipt {
    digest: [u8; 32],
}

impl RouteCutoverReceipt {
    fn seal(
        witness: CutoverWitnessId,
        rendered: &RenderedCutoverCase,
        png: &RenderedPngCutoverFacts,
    ) -> C6ProofResult<Self> {
        Self::seal_digests(
            witness,
            rendered.svg_target_receipt_digest,
            png.target_receipt_digest,
            png.semantic_receipt_digest,
            rendered.svg_route_receipt_digest,
        )
    }

    fn seal_digests(
        witness: CutoverWitnessId,
        svg_target_receipt_digest: [u8; 32],
        png_target_receipt_digest: [u8; 32],
        png_semantic_receipt_digest: [u8; 32],
        svg_route_receipt_digest: [u8; 32],
    ) -> C6ProofResult<Self> {
        c6_ensure!(
            "route-admission-receipt",
            svg_target_receipt_digest != [0; 32]
                && png_target_receipt_digest != [0; 32]
                && png_semantic_receipt_digest != [0; 32],
            "target-owned route admission digest is zero for {}",
            witness_label(witness)
        );
        c6_ensure!(
            "route-assertion-receipt",
            svg_route_receipt_digest != [0; 32],
            "renderer-owned route receipt digest is zero for {}",
            witness_label(witness)
        );
        let mut value = b"merman.c6-route-cutover-receipt.v6\0".to_vec();
        append_witness(&mut value, witness);
        value.extend_from_slice(&svg_target_receipt_digest);
        value.extend_from_slice(&png_target_receipt_digest);
        value.extend_from_slice(&png_semantic_receipt_digest);
        value.extend_from_slice(&svg_route_receipt_digest);
        Ok(Self {
            digest: sha256(value),
        })
    }
}

pub(crate) fn run_route_cutover_witnesses()
-> Result<RouteCutoverAuthorizationReport, RouteCutoverRuntimeError> {
    let inventory = legacy_replacing_typed_theme_routes().map_err(|error| {
        C6ProofError::new("route-inventory", error.to_string()).into_route_runtime("inventory")
    })?;
    let authorized_manifest = prove_route("manifest", authorize_cutover_routes(inventory))?;
    if authorized_manifest.routes().is_empty() {
        return Err(C6ProofError::new(
            "route-inventory",
            "typed legacy-replacing route inventory is empty",
        )
        .into_route_runtime("inventory"));
    }
    let manifest_digest = authorized_manifest_digest(
        authorized_manifest.manifest_version(),
        authorized_manifest.routes(),
    );
    if manifest_digest != EXPECTED_AUTHORIZED_MANIFEST_DIGEST {
        return Err(C6ProofError::new(
            "route-manifest",
            format!(
                "frozen cutover manifest digest mismatch: expected {}, observed {}",
                hex_digest(&EXPECTED_AUTHORIZED_MANIFEST_DIGEST),
                hex_digest(&manifest_digest)
            ),
        )
        .into_route_runtime("manifest"));
    }
    let authorized = authorized_manifest.into_routes();

    let mut rendered = BTreeMap::new();
    for witness_id in expected_cutover_witnesses(&authorized) {
        let route = witness_id.route();
        let case = CutoverCase {
            id: witness_id,
            source: source_for_witness(witness_id)
                .map_err(|error| error.into_route_runtime(witness_label(witness_id)))?,
        };
        let witness = case_label(case);
        let completed = prove_route(&witness, render_cutover_case(case, vec![route]))?;
        if rendered.insert(witness_id, completed).is_some() {
            return Err(RouteCutoverRuntimeError::DuplicateReceipt { route: witness });
        }
    }

    let raster_options = RasterOptions::default().with_scale(2.0);
    let mut png_facts = BTreeMap::new();
    let mut receipts = BTreeMap::new();
    let shapes = rendered
        .keys()
        .copied()
        .map(cutover_witness_shape)
        .collect::<BTreeSet<_>>();
    for shape in shapes {
        let pair_label = cutover_witness_shape_label(shape);
        let solid_witness = rendered
            .keys()
            .copied()
            .find(|witness| {
                cutover_witness_shape(*witness) == shape
                    && witness.route().value() == ThemeRouteCutoverValue::Solid
            })
            .ok_or_else(|| RouteCutoverRuntimeError::CoverageMismatch {
                missing: vec![format!("{pair_label}/solid")],
                unexpected: Vec::new(),
            })?;
        let transparent_witness = rendered
            .keys()
            .copied()
            .find(|witness| {
                cutover_witness_shape(*witness) == shape
                    && witness.route().value() == ThemeRouteCutoverValue::Transparent
            })
            .ok_or_else(|| RouteCutoverRuntimeError::CoverageMismatch {
                missing: vec![format!("{pair_label}/transparent")],
                unexpected: Vec::new(),
            })?;
        let solid = rendered.get(&solid_witness).ok_or_else(|| {
            RouteCutoverRuntimeError::CoverageMismatch {
                missing: vec![witness_label(solid_witness)],
                unexpected: Vec::new(),
            }
        })?;
        let transparent = rendered.get(&transparent_witness).ok_or_else(|| {
            RouteCutoverRuntimeError::CoverageMismatch {
                missing: vec![witness_label(transparent_witness)],
                unexpected: Vec::new(),
            }
        })?;
        let png_pair = prove_route(
            &pair_label,
            merman::__theme_acceptance::export_theme_route_cutover_png_pair(
                &solid.document,
                &transparent.document,
                solid_witness.route(),
                transparent_witness.route(),
                &raster_options,
                OperationControl::new(),
            )
            .map_err(|error| C6ProofError::new("route-png-encode", error.to_string())),
        )?;
        let solid_png = png_pair.solid();
        let transparent_png = png_pair.transparent();
        if solid_png.bytes().is_empty() || transparent_png.bytes().is_empty() {
            return Err(C6ProofError::new(
                "route-png-artifact",
                format!("PNG route projection produced an empty artifact for {pair_label}"),
            )
            .into_route_runtime(pair_label));
        }
        let solid_png_target_receipt_digest = prove_route(
            &format!("{pair_label}/solid"),
            validate_cutover_target_receipt(
                CutoverTargetAdmissionContract::PortableNativePngV1,
                solid_png.receipt(),
            ),
        )?;
        let transparent_png_target_receipt_digest = prove_route(
            &format!("{pair_label}/transparent"),
            validate_cutover_target_receipt(
                CutoverTargetAdmissionContract::PortableNativePngV1,
                transparent_png.receipt(),
            ),
        )?;
        let semantic_receipt_digest = png_pair.receipt_digest();
        if semantic_receipt_digest == [0; 32] {
            return Err(C6ProofError::new(
                "route-png-semantic",
                format!(
                    "renderer-owned route PNG semantic receipt is zero for {}",
                    pair_label
                ),
            )
            .into_route_runtime(pair_label));
        }
        if png_facts
            .insert(
                solid_witness,
                RenderedPngCutoverFacts {
                    target_receipt_digest: solid_png_target_receipt_digest,
                    semantic_receipt_digest,
                },
            )
            .is_some()
        {
            return Err(RouteCutoverRuntimeError::DuplicateReceipt {
                route: witness_label(solid_witness),
            });
        }
        if png_facts
            .insert(
                transparent_witness,
                RenderedPngCutoverFacts {
                    target_receipt_digest: transparent_png_target_receipt_digest,
                    semantic_receipt_digest,
                },
            )
            .is_some()
        {
            return Err(RouteCutoverRuntimeError::DuplicateReceipt {
                route: witness_label(transparent_witness),
            });
        }
        for (witness_id, rendered_case) in
            [(solid_witness, solid), (transparent_witness, transparent)]
        {
            let png = png_facts.get(&witness_id).ok_or_else(|| {
                RouteCutoverRuntimeError::CoverageMismatch {
                    missing: vec![format!("{}/png-facts", witness_label(witness_id))],
                    unexpected: Vec::new(),
                }
            })?;
            let receipt = prove_route(
                &witness_label(witness_id),
                RouteCutoverReceipt::seal(witness_id, rendered_case, png),
            )?;
            if receipts.insert(witness_id, receipt).is_some() {
                return Err(RouteCutoverRuntimeError::DuplicateReceipt {
                    route: witness_label(witness_id),
                });
            }
        }
    }

    let route_report_digest = evaluate_route_receipts(authorized, receipts)?;
    prove_route(
        "authorization",
        RouteCutoverAuthorizationReceipt::seal(manifest_digest, route_report_digest),
    )
    .map(RouteCutoverAuthorizationReceipt::into_report)
}

fn render_cutover_case(
    case: CutoverCase,
    mut routes: Vec<ThemeRouteCutoverDescriptor>,
) -> C6ProofResult<RenderedCutoverCase> {
    let witness_id = case.id;
    let expected_route = witness_id.route();
    let profile = witness_id.profile();
    routes.sort_unstable();
    c6_ensure!(
        "route-inventory",
        !routes.is_empty(),
        "{} has no matching typed bridge-replacing routes",
        case_label(case)
    );
    c6_ensure!(
        "route-inventory",
        routes.iter().all(|route| { *route == expected_route })
            && CutoverWitnessProfile::for_route(expected_route).contains(&profile),
        "{} contains a route outside its canonical selector/family/value",
        case_label(case)
    );

    let theme = compile_cutover_theme(case)?;
    let renderer = cutover_renderer(case.id);
    let document = render_cutover_document(&renderer, case, theme.clone())?;
    let render_evidence = document.evidence();
    prove_portable_family_evidence(
        render_evidence,
        &theme,
        expected_route.family_id(),
        FamilyEvidenceRequirements::ROUTE_CUTOVER,
    )?;
    let document_portability = document.portability();
    c6_ensure!(
        "route-document-portability",
        document_portability.is_evidence_valid()
            && !document_portability.is_host_dependent()
            && document_portability.reasons().is_empty(),
        "route document proof is not target-independent and verified: verified={} host_dependent={} reasons={:?}",
        document_portability.is_evidence_valid(),
        document_portability.is_host_dependent(),
        document_portability.reasons()
    );
    let acceptance_evidence =
        merman::__theme_acceptance::theme_acceptance_evidence(render_evidence);
    let family_evidence = acceptance_evidence.family();
    c6_ensure!(
        "route-family-disposition",
        family_evidence.required_count() == 1
            && family_evidence.applied_count() == 1
            && family_evidence.not_applicable_count() == 0
            && family_evidence.residual_count() == 0,
        "route witness did not prove one exact Applied family mechanism: required={} applied={} not_applicable={} residual={}",
        family_evidence.required_count(),
        family_evidence.applied_count(),
        family_evidence.not_applicable_count(),
        family_evidence.residual_count()
    );

    let svg_receipt = document.standalone_svg_admission();
    let route_receipt = merman::__theme_acceptance::theme_route_cutover_receipts(&document)
        .iter()
        .find(|receipt| receipt.descriptor().id() == expected_route.id())
        .copied()
        .ok_or_else(|| {
            C6ProofError::new(
                "route-svg-receipt",
                format!(
                    "renderer did not seal a route receipt for {}",
                    route_label(expected_route)
                ),
            )
        })?;
    c6_ensure!(
        "route-svg-receipt",
        route_receipt.proves_artifact(svg_receipt.artifact_digest()),
        "renderer route receipt is not bound to the finalized Standalone SVG artifact"
    );

    let svg_target_receipt_digest = validate_cutover_target_receipt(
        CutoverTargetAdmissionContract::PaintStandaloneSvgV1,
        svg_receipt,
    )?;

    Ok(RenderedCutoverCase {
        document,
        svg_target_receipt_digest,
        svg_route_receipt_digest: route_receipt.digest(),
    })
}

fn render_cutover_document(
    renderer: &Renderer,
    case: CutoverCase,
    theme: DiagramTheme,
) -> C6ProofResult<merman::RenderedDocument> {
    let document_output = renderer
        .render(
            RenderRequest::document(case.source, OperationControl::new(), portable_svg_request())
                .with_theme(theme),
        )
        .map_err(|error| C6ProofError::new("route-svg-render", error.to_string()))?;
    let RenderOutput::Document(Some(document)) = document_output else {
        return Err(C6ProofError::new(
            "route-svg-render",
            "route witness did not produce a completed document",
        ));
    };
    Ok(document)
}

fn compile_cutover_theme(case: CutoverCase) -> C6ProofResult<DiagramTheme> {
    compile_cutover_theme_with_value(case, case.id.route().value())
}

fn compile_cutover_theme_with_value(
    case: CutoverCase,
    route_value: ThemeRouteCutoverValue,
) -> C6ProofResult<DiagramTheme> {
    let mut styles = ThemeRuleSet::default();
    let route = case.id.route();
    let control_css = route.raster_control_css();
    let style = match route.facet() {
        ThemeRouteCutoverFacet::Fill => {
            ThemeStylePatch::default().with_fill(cutover_paint(route_value, &control_css)?)
        }
        ThemeRouteCutoverFacet::Stroke => {
            ThemeStylePatch::default().with_stroke(cutover_paint(route_value, &control_css)?)
        }
    };
    styles = styles.with_rule(cutover_rule(case, route.target(), style));

    let font_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    let font_catalog =
        FontCatalogSpec::new([FontAssetSpec::new("route-cutover-excalifont", font_bytes)])
            .with_alias("trebuchet ms", "Excalifont")
            .with_alias("verdana", "Excalifont")
            .with_alias("arial", "Excalifont")
            .with_generic_family(GenericFontFamily::SansSerif, "Excalifont")
            .with_available_sources([FontSource::Embedded])
            .with_embedding_requirement(FontEmbeddingRequirement::FullFont);

    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_assets(ThemeAssets::default().with_font_catalog(font_catalog))
                .with_canvas(CanvasSpec::transparent())
                .with_styles(styles),
        )
        .map_err(|error| C6ProofError::new("route-theme", error.to_string()))
}

fn cutover_rule(case: CutoverCase, target: ThemeTarget, style: ThemeStylePatch) -> ThemeRule {
    let route = case.id.route();
    let rule = ThemeRule::new(target, style).for_family(route.family_id());
    match route.selector() {
        ThemeRouteCutoverSelector::StaticUnqualified => rule,
        ThemeRouteCutoverSelector::StaticVariant(variant) => rule.with_variant(variant),
    }
}

fn cutover_paint(value: ThemeRouteCutoverValue, solid: &str) -> C6ProofResult<CanvasPaint> {
    match value {
        ThemeRouteCutoverValue::Transparent => Ok(CanvasPaint::Transparent),
        ThemeRouteCutoverValue::Solid => CanvasPaint::solid(solid)
            .map_err(|error| C6ProofError::new("route-theme-paint", error.to_string())),
    }
}

fn cutover_renderer(witness: CutoverWitnessId) -> Renderer {
    let profile = witness.profile();
    let theme_variables = match (witness.route().family_id(), witness.route().target()) {
        (_, ThemeTarget::Activation) => serde_json::json!({
            "actorLineColor": "transparent",
            "signalColor": "transparent"
        }),
        (DiagramFamilyId::GANTT, ThemeTarget::Task) => serde_json::json!({
            "sectionBkgColor": "transparent",
            "sectionBkgColor2": "transparent",
            "altSectionBkgColor": "transparent",
            "gridColor": "transparent"
        }),
        (DiagramFamilyId::SWIMLANE, ThemeTarget::EdgeLabelBackground) => serde_json::json!({
            // Isolate the translucent label background from the automatic lane's fill. The
            // native proof then observes its actual paint without modeling alpha compositing.
            "clusterBkg": "transparent"
        }),
        (DiagramFamilyId::ER, ThemeTarget::Text) => serde_json::json!({
            // Keep the control surface opaque and identical to the entity surface so the
            // route proof observes the Text terminal instead of reimplementing rgba
            // compositing for Mermaid's default edge-label background.
            "edgeLabelBackground": "#ECECFF"
        }),
        (DiagramFamilyId::CYNEFIN, ThemeTarget::Text)
        | (DiagramFamilyId::XY_CHART, ThemeTarget::Text | ThemeTarget::Title | ThemeTarget::Axis)
        | (
            DiagramFamilyId::QUADRANT_CHART,
            ThemeTarget::Text | ThemeTarget::Title | ThemeTarget::Axis,
        ) => {
            serde_json::json!({
                // These families emit plain SVG text without prepared-label tokens. Pin the deterministic
                // route witness to the embedded catalog face so native raster admission can classify
                // every visible text glyph without a host fallback.
                "fontFamily": "Excalifont"
            })
        }
        (DiagramFamilyId::MINDMAP, ThemeTarget::Node) => serde_json::json!({
            // The route-local raster proof isolates paint and intentionally rejects SVG group
            // effects. Keep the Redux Neo writer path that consumes `mainBkg`/`nodeBorder`, but
            // disable its orthogonal shadow and gradient effects for this witness.
            "dropShadow": "none",
            "useGradient": false
        }),
        _ => serde_json::json!({}),
    };
    let mut site_config = serde_json::json!({
        "htmlLabels": false,
        "look": profile.look(),
        "flowchart": { "htmlLabels": false },
        "themeVariables": theme_variables
    });
    if (witness.route().family_id() == DiagramFamilyId::MINDMAP
        && witness.route().target() == ThemeTarget::Node)
        || profile == CutoverWitnessProfile::ReduxStatic
    {
        site_config["theme"] = serde_json::Value::String("redux".to_string());
    }
    let renderer = Renderer::new()
        .with_engine(Engine::new().with_site_config(MermaidConfig::from_value(site_config)));
    if matches!(
        profile,
        CutoverWitnessProfile::ClassicToday | CutoverWitnessProfile::ClassicVertical
    ) {
        // todayMarker configures CSS, not time. Keep the actual clock inside the task interval.
        renderer.with_runtime_policy(
            merman::runtime::RuntimePolicy::deterministic()
                .with_fixed_unix_millis(1_704_240_000_000),
        )
    } else {
        renderer
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CutoverTargetAdmissionContract {
    PaintStandaloneSvgV1,
    PortableNativePngV1,
}

impl CutoverTargetAdmissionContract {
    const fn id(self) -> &'static str {
        match self {
            Self::PaintStandaloneSvgV1 => "paint-standalone-svg-v1",
            Self::PortableNativePngV1 => "portable-native-png-v1",
        }
    }

    const fn artifact_kind(self) -> RenderArtifactKind {
        match self {
            Self::PaintStandaloneSvgV1 => RenderArtifactKind::Svg,
            Self::PortableNativePngV1 => RenderArtifactKind::Png,
        }
    }

    fn accepts(
        self,
        status: TargetAdmissionStatus,
        reasons: &[TargetAdmissionReason],
        font_source: TargetFontSource,
    ) -> bool {
        match self {
            Self::PaintStandaloneSvgV1 => {
                (status == TargetAdmissionStatus::Portable
                    && reasons.is_empty()
                    && font_source == TargetFontSource::Embedded)
                    || (status == TargetAdmissionStatus::HostDependent
                        && reasons == [TargetAdmissionReason::SvgFontsNotSelfContained]
                        && matches!(
                            font_source,
                            TargetFontSource::None | TargetFontSource::Embedded
                        ))
            }
            Self::PortableNativePngV1 => {
                status == TargetAdmissionStatus::Portable
                    && reasons.is_empty()
                    && font_source == TargetFontSource::Embedded
            }
        }
    }
}

fn validate_cutover_target_receipt(
    contract: CutoverTargetAdmissionContract,
    receipt: &TargetAdmissionReceipt,
) -> C6ProofResult<[u8; 32]> {
    let expected_kind = contract.artifact_kind();
    c6_ensure!(
        "route-target-admission",
        receipt.artifact_kind() == expected_kind,
        "expected {} target receipt, got {}",
        expected_kind.id(),
        receipt.artifact_kind().id()
    );
    c6_ensure!(
        "route-target-admission",
        contract.accepts(receipt.status(), receipt.reasons(), receipt.font_source(),),
        "{} target does not satisfy {}: status={} reasons={:?} font_source={}",
        expected_kind.id(),
        contract.id(),
        receipt.status().id(),
        receipt.reasons(),
        receipt.font_source().id()
    );
    c6_ensure!(
        "route-target-admission",
        receipt.receipt_digest() != [0; 32],
        "{} target receipt retained a zero canonical digest",
        expected_kind.id()
    );
    Ok(receipt.receipt_digest())
}

fn evaluate_route_receipts(
    inventory: Vec<ThemeRouteCutoverDescriptor>,
    receipts: BTreeMap<CutoverWitnessId, RouteCutoverReceipt>,
) -> Result<[u8; 32], RouteCutoverRuntimeError> {
    let expected = expected_cutover_witnesses(&inventory)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let actual = receipts.keys().copied().collect::<BTreeSet<_>>();
    if expected != actual {
        return Err(RouteCutoverRuntimeError::CoverageMismatch {
            missing: expected
                .difference(&actual)
                .copied()
                .map(witness_label)
                .collect(),
            unexpected: actual
                .difference(&expected)
                .copied()
                .map(witness_label)
                .collect(),
        });
    }

    let mut value = b"merman.c6-route-cutover-report.v2\0".to_vec();
    value.extend_from_slice(&usize_to_u64(receipts.len()).to_be_bytes());
    for (&witness, receipt) in &receipts {
        append_witness(&mut value, witness);
        value.extend_from_slice(&receipt.digest);
    }
    Ok(sha256(value))
}

fn authorized_manifest_digest(
    manifest_version: u16,
    routes: &[ThemeRouteCutoverDescriptor],
) -> [u8; 32] {
    let mut value = b"merman.c6-route-cutover-manifest.v1\0".to_vec();
    value.extend_from_slice(&manifest_version.to_be_bytes());
    value.extend_from_slice(&usize_to_u64(routes.len()).to_be_bytes());
    for &route in routes {
        append_route(&mut value, route);
    }
    sha256(value)
}

fn hex_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

type RouteShape = (
    DiagramFamilyId,
    ThemeTarget,
    ThemeRouteCutoverSelector,
    ThemeRouteCutoverFacet,
    ThemeRouteCutoverProjectionSet,
);

fn route_shape(route: ThemeRouteCutoverDescriptor) -> RouteShape {
    (
        route.family_id(),
        route.target(),
        route.selector(),
        route.facet(),
        route.projections(),
    )
}

fn route_shape_label(shape: RouteShape) -> String {
    format!(
        "{}/{}/{}/{}/{}",
        shape.0.as_str(),
        shape.1.id(),
        shape.2.id(),
        facet_id(shape.3),
        projection_set_label(shape.4)
    )
}

type CutoverWitnessShape = (RouteShape, CutoverWitnessProfile);

fn cutover_witness_shape(witness: CutoverWitnessId) -> CutoverWitnessShape {
    (route_shape(witness.route()), witness.profile())
}

fn cutover_witness_shape_label(shape: CutoverWitnessShape) -> String {
    format!("{}/{}", route_shape_label(shape.0), shape.1.id())
}

fn route_label(route: ThemeRouteCutoverDescriptor) -> String {
    format!(
        "{}/{}/{}/{}/{}/{}",
        route.family_id().as_str(),
        route.target().id(),
        route.selector().id(),
        facet_id(route.facet()),
        value_id(route.value()),
        projection_set_label(route.projections())
    )
}

fn witness_label(witness: CutoverWitnessId) -> String {
    format!(
        "{}/{}",
        route_label(witness.route()),
        witness.profile().id()
    )
}

fn case_label(case: CutoverCase) -> String {
    witness_label(case.id)
}

fn projection_set_label(projections: ThemeRouteCutoverProjectionSet) -> String {
    projections
        .iter()
        .map(ThemeRouteCutoverProjection::contribution_id)
        .collect::<Vec<_>>()
        .join("+")
}

fn facet_id(facet: ThemeRouteCutoverFacet) -> &'static str {
    match facet {
        ThemeRouteCutoverFacet::Fill => "fill",
        ThemeRouteCutoverFacet::Stroke => "stroke",
    }
}

fn value_id(value: ThemeRouteCutoverValue) -> &'static str {
    match value {
        ThemeRouteCutoverValue::Transparent => "transparent",
        ThemeRouteCutoverValue::Solid => "solid",
    }
}

fn append_route(output: &mut Vec<u8>, route: ThemeRouteCutoverDescriptor) {
    append_len_prefixed(output, route.family_id().as_str().as_bytes());
    append_len_prefixed(output, route.target().id().as_bytes());
    append_len_prefixed(output, route.selector().id().as_bytes());
    append_len_prefixed(output, facet_id(route.facet()).as_bytes());
    append_len_prefixed(output, value_id(route.value()).as_bytes());
    output.extend_from_slice(&usize_to_u64(route.projections().len()).to_be_bytes());
    for projection in route.projections().iter() {
        append_len_prefixed(output, projection.contribution_id().as_bytes());
        append_len_prefixed(output, projection.action().id().as_bytes());
    }
}

fn append_witness(output: &mut Vec<u8>, witness: CutoverWitnessId) {
    append_route(output, witness.route());
    append_len_prefixed(output, witness.profile().id().as_bytes());
}

pub(crate) fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn prove_route<T>(witness: &str, result: C6ProofResult<T>) -> Result<T, RouteCutoverRuntimeError> {
    result.map_err(|error| error.into_route_runtime(witness))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use merman::svg::{ThemeTarget, ThemeVariant};
    use merman::{DiagramFamilyId, TargetAdmissionReason, TargetAdmissionStatus, TargetFontSource};
    use merman_render::__private::{
        ThemeRouteCutoverFacet, ThemeRouteCutoverSelector, ThemeRouteCutoverValue,
    };

    use super::{
        CutoverTargetAdmissionContract, CutoverWitnessId, CutoverWitnessProfile,
        RouteCutoverAuthorizationReceipt, RouteCutoverReceipt, evaluate_route_receipts,
        expected_cutover_witnesses, legacy_replacing_typed_theme_routes,
        run_route_cutover_witnesses,
    };
    use crate::cutover_manifest::EXPECTED_AUTHORIZED_MANIFEST_DIGEST;

    #[test]
    fn gantt_warning_today_and_vertical_have_independent_visible_png_evidence() {
        let inventory = legacy_replacing_typed_theme_routes().expect("route inventory");
        for profile in [
            CutoverWitnessProfile::ClassicToday,
            CutoverWitnessProfile::ClassicVertical,
        ] {
            let render = |value| {
                let route = inventory
                    .iter()
                    .copied()
                    .find(|route| {
                        route.family_id() == DiagramFamilyId::GANTT
                            && route.target() == ThemeTarget::Task
                            && route.facet() == ThemeRouteCutoverFacet::Stroke
                            && route.selector()
                                == ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Warning)
                            && route.value() == value
                    })
                    .expect("Gantt Warning stroke route");
                assert_eq!(
                    CutoverWitnessProfile::for_route(route),
                    &[
                        CutoverWitnessProfile::ClassicToday,
                        CutoverWitnessProfile::ClassicVertical
                    ]
                );
                let id = CutoverWitnessId::new(route, profile);
                let source = super::source_for_witness(id).expect("source");
                match profile {
                    CutoverWitnessProfile::ClassicToday => assert!(!source.contains("vert,")),
                    CutoverWitnessProfile::ClassicVertical => {
                        assert!(source.contains("todayMarker off"))
                    }
                    _ => unreachable!(),
                }
                let case = super::CutoverCase { id, source };
                (
                    route,
                    super::render_cutover_case(case, vec![route]).expect("SVG witness"),
                )
            };
            let (solid_route, solid) = render(ThemeRouteCutoverValue::Solid);
            let (transparent_route, transparent) = render(ThemeRouteCutoverValue::Transparent);
            merman::__theme_acceptance::export_theme_route_cutover_png_pair(
                &solid.document,
                &transparent.document,
                solid_route,
                transparent_route,
                &merman_export::RasterOptions::default().with_scale(2.0),
                merman::OperationControl::new(),
            )
            .expect("each Warning terminal must independently produce route-local pixels");
        }
    }

    #[test]
    fn requirement_text_node_and_relation_colors_have_independent_png_evidence() {
        let inventory = legacy_replacing_typed_theme_routes().expect("route inventory");
        for owner in ["nodeTextColor", "relationLabelColor"] {
            for profile in CutoverWitnessProfile::TEXT_LOOKS {
                for selector in [
                    ThemeRouteCutoverSelector::StaticUnqualified,
                    ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default),
                ] {
                    let render = |value| {
                        let route = inventory
                            .iter()
                            .copied()
                            .find(|route| {
                                route.family_id() == DiagramFamilyId::REQUIREMENT
                                    && route.target() == ThemeTarget::Text
                                    && route.facet() == ThemeRouteCutoverFacet::Fill
                                    && route.selector() == selector
                                    && route.value() == value
                            })
                            .expect("Requirement text route");
                        let id = CutoverWitnessId::new(route, profile);
                        let case = super::CutoverCase {
                            id,
                            source: super::REQUIREMENT_RELATION_SOURCE,
                        };
                        let theme = super::compile_cutover_theme(case).expect("typed text theme");
                        let renderer = super::cutover_renderer(id).with_engine(
                            super::Engine::new().with_site_config(
                                super::MermaidConfig::from_value(serde_json::json!({
                                    "htmlLabels": false, "look": profile.look(),
                                    "themeVariables": { (owner): "#112233" }
                                })),
                            ),
                        );
                        (
                            route,
                            super::render_cutover_document(&renderer, case, theme)
                                .expect("independent text consumer SVG"),
                        )
                    };
                    let (solid_route, solid) = render(ThemeRouteCutoverValue::Solid);
                    let (transparent_route, transparent) =
                        render(ThemeRouteCutoverValue::Transparent);
                    let pair = merman::__theme_acceptance::export_theme_route_cutover_png_pair(
                        &solid,
                        &transparent,
                        solid_route,
                        transparent_route,
                        &merman_export::RasterOptions::default().with_scale(2.0),
                        merman::OperationControl::new(),
                    )
                    .expect("each unowned text color must produce independent native pixels");
                    for receipt in [pair.solid().receipt(), pair.transparent().receipt()] {
                        super::validate_cutover_target_receipt(
                            CutoverTargetAdmissionContract::PortableNativePngV1,
                            receipt,
                        )
                        .expect("unchanged native PNG admission");
                    }
                    assert_ne!(pair.receipt_digest(), [0; 32]);
                }
            }
        }
    }

    #[test]
    fn timeline_event_stroke_has_visible_redux_axis_png_evidence() {
        let inventory = legacy_replacing_typed_theme_routes().expect("route inventory");
        for selector in [
            ThemeRouteCutoverSelector::StaticUnqualified,
            ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default),
        ] {
            let render = |value| {
                let route = inventory
                    .iter()
                    .copied()
                    .find(|route| {
                        route.family_id() == DiagramFamilyId::TIMELINE
                            && route.target() == ThemeTarget::TimelineEvent
                            && route.facet() == ThemeRouteCutoverFacet::Stroke
                            && route.selector() == selector
                            && route.value() == value
                    })
                    .expect("Timeline event stroke route");
                assert_eq!(
                    CutoverWitnessProfile::for_route(route),
                    &[CutoverWitnessProfile::ReduxStatic]
                );
                let id = CutoverWitnessId::new(route, CutoverWitnessProfile::ReduxStatic);
                let source = super::source_for_witness(id).expect("axis source");
                assert_eq!(source, super::TIMELINE_EVENT_STROKE_AXIS_SOURCE);
                let rendered =
                    super::render_cutover_case(super::CutoverCase { id, source }, vec![route])
                        .expect("Redux axis SVG witness");
                let svg = rendered.document.sealed_svg().as_str();
                assert!(svg.contains(r#"class="lineWrapper"><line"#));
                assert!(
                    !svg.contains(r#"class="timeline-node "#),
                    "axis proof must not claim shadow-bearing nodes"
                );
                (route, rendered)
            };
            let (solid_route, solid) = render(ThemeRouteCutoverValue::Solid);
            let (transparent_route, transparent) = render(ThemeRouteCutoverValue::Transparent);
            let pair = merman::__theme_acceptance::export_theme_route_cutover_png_pair(
                &solid.document,
                &transparent.document,
                solid_route,
                transparent_route,
                &merman_export::RasterOptions::default().with_scale(2.0),
                merman::OperationControl::new(),
            )
            .expect("shared Redux axis paint must produce route-local native pixels");
            for receipt in [pair.solid().receipt(), pair.transparent().receipt()] {
                super::validate_cutover_target_receipt(
                    CutoverTargetAdmissionContract::PortableNativePngV1,
                    receipt,
                )
                .expect("axis PNG must satisfy unchanged native target admission");
            }
            assert_ne!(pair.receipt_digest(), [0; 32]);
        }
    }

    #[test]
    fn radar_axis_fill_fallback_and_stroke_have_native_axis_png_evidence() {
        let inventory = legacy_replacing_typed_theme_routes().expect("route inventory");
        for facet in [ThemeRouteCutoverFacet::Fill, ThemeRouteCutoverFacet::Stroke] {
            for selector in [
                ThemeRouteCutoverSelector::StaticUnqualified,
                ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default),
            ] {
                let render = |value| {
                    let route = inventory
                        .iter()
                        .copied()
                        .find(|route| {
                            route.family_id() == DiagramFamilyId::RADAR
                                && route.target() == ThemeTarget::Axis
                                && route.facet() == facet
                                && route.selector() == selector
                                && route.value() == value
                        })
                        .expect("Radar axis paint route");
                    assert_eq!(
                        route.projections(),
                        super::ThemeRouteCutoverProjectionSet::REPLACE_RADAR_AXIS_PAINT
                    );
                    let id = CutoverWitnessId::new(route, CutoverWitnessProfile::ClassicStatic);
                    let source = super::source_for_witness(id).expect("axis source");
                    assert_eq!(source, super::RADAR_AXIS_SOURCE);
                    let rendered =
                        super::render_cutover_case(super::CutoverCase { id, source }, vec![route])
                            .expect("Radar axis SVG witness");
                    assert_eq!(
                        rendered
                            .document
                            .sealed_svg()
                            .as_str()
                            .matches("class=\"radarAxisLine\"")
                            .count(),
                        4
                    );
                    (route, rendered)
                };
                let (solid_route, solid) = render(ThemeRouteCutoverValue::Solid);
                let (transparent_route, transparent) = render(ThemeRouteCutoverValue::Transparent);
                let pair = merman::__theme_acceptance::export_theme_route_cutover_png_pair(
                    &solid.document,
                    &transparent.document,
                    solid_route,
                    transparent_route,
                    &merman_export::RasterOptions::default().with_scale(2.0),
                    merman::OperationControl::new(),
                )
                .expect("Axis.fill fallback and Axis.stroke must each change native axis pixels");
                for receipt in [pair.solid().receipt(), pair.transparent().receipt()] {
                    super::validate_cutover_target_receipt(
                        CutoverTargetAdmissionContract::PortableNativePngV1,
                        receipt,
                    )
                    .expect("axis PNG must satisfy unchanged native target admission");
                }
                assert_ne!(pair.receipt_digest(), [0; 32]);
            }
        }
    }

    #[test]
    fn xychart_text_and_axis_channels_have_independent_native_png_evidence() {
        let inventory = legacy_replacing_typed_theme_routes().expect("route inventory");
        let all_channels = [
            "titleColor",
            "dataLabelColor",
            "xAxisTitleColor",
            "xAxisLabelColor",
            "yAxisTitleColor",
            "yAxisLabelColor",
            "xAxisLineColor",
            "xAxisTickColor",
            "yAxisLineColor",
            "yAxisTickColor",
        ];
        // Pin every sibling channel so each raster pair must observe the one remaining
        // consumer. The source has a title, both logical axis titles/labels/lines/ticks,
        // and outside-bar data labels, which remain visible in either orientation.
        for (target, facet, channels) in [
            (
                ThemeTarget::Text,
                ThemeRouteCutoverFacet::Fill,
                &all_channels[..6],
            ),
            (
                ThemeTarget::Axis,
                ThemeRouteCutoverFacet::Fill,
                &all_channels[2..],
            ),
            (
                ThemeTarget::Axis,
                ThemeRouteCutoverFacet::Stroke,
                &all_channels[6..],
            ),
        ] {
            for source in [
                super::XYCHART_PAINT_SOURCE,
                super::XYCHART_HORIZONTAL_PAINT_SOURCE,
            ] {
                for selector in [
                    ThemeRouteCutoverSelector::StaticUnqualified,
                    ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default),
                ] {
                    for &channel in channels {
                        let render = |value| {
                            let route = inventory
                                .iter()
                                .copied()
                                .find(|route| {
                                    route.family_id() == DiagramFamilyId::XY_CHART
                                        && route.target() == target
                                        && route.facet() == facet
                                        && route.selector() == selector
                                        && route.value() == value
                                })
                                .expect("XY Chart scalar paint route");
                            let id =
                                CutoverWitnessId::new(route, CutoverWitnessProfile::ClassicStatic);
                            let case = super::CutoverCase { id, source };
                            let theme =
                                super::compile_cutover_theme(case).expect("typed chart theme");
                            let pinned = all_channels
                                .iter()
                                .filter(|&&key| key != channel)
                                .map(|&key| (key.to_owned(), serde_json::json!("#112233")))
                                .collect::<serde_json::Map<_, _>>();
                            let renderer = super::cutover_renderer(id).with_engine(
                                super::Engine::new().with_site_config(
                                    super::MermaidConfig::from_value(serde_json::json!({
                                        "htmlLabels": false,
                                        "themeVariables": {
                                            "fontFamily": "Excalifont",
                                            "xyChart": pinned
                                        }
                                    })),
                                ),
                            );
                            let document = super::render_cutover_document(&renderer, case, theme)
                                .unwrap_or_else(|error| {
                                    panic!("{target:?}/{facet:?}/{selector:?}/{channel}: {error}")
                                });
                            super::validate_cutover_target_receipt(
                                CutoverTargetAdmissionContract::PaintStandaloneSvgV1,
                                document.standalone_svg_admission(),
                            )
                            .expect("isolated chart channel SVG admission");
                            (route, document)
                        };
                        let (solid_route, solid) = render(ThemeRouteCutoverValue::Solid);
                        let (transparent_route, transparent) =
                            render(ThemeRouteCutoverValue::Transparent);
                        let pair = merman::__theme_acceptance::export_theme_route_cutover_png_pair(
                            &solid,
                            &transparent,
                            solid_route,
                            transparent_route,
                            &merman_export::RasterOptions::default().with_scale(2.0),
                            merman::OperationControl::new(),
                        )
                        .unwrap_or_else(|error| {
                            panic!("{target:?}/{facet:?}/{selector:?}/{channel}: {error}")
                        });
                        for receipt in [pair.solid().receipt(), pair.transparent().receipt()] {
                            super::validate_cutover_target_receipt(
                                CutoverTargetAdmissionContract::PortableNativePngV1,
                                receipt,
                            )
                            .expect("isolated chart channel PNG admission");
                        }
                        assert_ne!(pair.receipt_digest(), [0; 32]);
                    }
                }
            }
        }
    }

    #[test]
    fn radar_text_base_and_title_fallback_have_independent_native_png_evidence() {
        let inventory = legacy_replacing_typed_theme_routes().expect("route inventory");
        // Base derives textColor from primaryTextColor and titleColor from tertiaryTextColor,
        // so these consumers can be owned independently. Default instead propagates an explicit
        // textColor override to titleColor; using it here would legitimately own both channels.
        // Pin each Base consumer in turn and prove the remaining native glyph pixels.
        for owner in ["titleColor", "textColor"] {
            for selector in [
                ThemeRouteCutoverSelector::StaticUnqualified,
                ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default),
            ] {
                let render = |value| {
                    let route = inventory
                        .iter()
                        .copied()
                        .find(|route| {
                            route.family_id() == DiagramFamilyId::RADAR
                                && route.target() == ThemeTarget::Text
                                && route.facet() == ThemeRouteCutoverFacet::Fill
                                && route.selector() == selector
                                && route.value() == value
                        })
                        .expect("Radar text fill route");
                    assert_eq!(
                        route.projections(),
                        super::ThemeRouteCutoverProjectionSet::REPLACE_RADAR_TEXT_PAINT
                    );
                    let id = CutoverWitnessId::new(route, CutoverWitnessProfile::ClassicStatic);
                    let source = super::source_for_witness(id).expect("text source");
                    assert_eq!(source, super::RADAR_TEXT_SOURCE);
                    let case = super::CutoverCase { id, source };
                    let theme = super::compile_cutover_theme(case).expect("typed text theme");
                    let renderer = super::cutover_renderer(id).with_engine(
                        super::Engine::new().with_site_config(super::MermaidConfig::from_value(
                            serde_json::json!({
                                "theme": "base",
                                "htmlLabels": false,
                                "themeVariables": { (owner): "#112233" }
                            }),
                        )),
                    );
                    let document = super::render_cutover_document(&renderer, case, theme)
                        .expect("independent Radar text consumer SVG");
                    let svg = document.sealed_svg().as_str();
                    assert_eq!(svg.matches("class=\"radarAxisLabel\"").count(), 4);
                    assert_eq!(svg.matches("class=\"radarLegendText\"").count(), 1);
                    assert_eq!(svg.matches("class=\"radarTitle\"").count(), 1);
                    (route, document)
                };
                let (solid_route, solid) = render(ThemeRouteCutoverValue::Solid);
                let (transparent_route, transparent) = render(ThemeRouteCutoverValue::Transparent);
                let pair = merman::__theme_acceptance::export_theme_route_cutover_png_pair(
                    &solid,
                    &transparent,
                    solid_route,
                    transparent_route,
                    &merman_export::RasterOptions::default().with_scale(2.0),
                    merman::OperationControl::new(),
                )
                .expect("each independently unowned Radar text consumer must change native glyph pixels");
                for receipt in [pair.solid().receipt(), pair.transparent().receipt()] {
                    super::validate_cutover_target_receipt(
                        CutoverTargetAdmissionContract::PortableNativePngV1,
                        receipt,
                    )
                    .expect("Radar text PNG must satisfy unchanged native target admission");
                }
                assert_ne!(pair.receipt_digest(), [0; 32]);
            }
        }
    }

    #[test]
    fn flowchart_text_channels_have_independent_native_png_evidence() {
        let inventory = legacy_replacing_typed_theme_routes().expect("route inventory");
        for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
            for (channel, flowchart_source, swimlane_source) in [
                (
                    "node",
                    "---\n---\nflowchart TD\nA[Alpha]\n",
                    "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA[Alpha]\n",
                ),
                (
                    "edge",
                    "---\n---\nflowchart TD\nA[Alpha] -->|Advance| B[Beta]\nstyle A color:#112233\nstyle B color:#112233\n",
                    "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA[Alpha] -->|Advance| B[Beta]\nstyle A color:#112233\nstyle B color:#112233\n",
                ),
                (
                    "cluster",
                    "---\n---\nflowchart TD\nsubgraph Group[Group title]\nA[Alpha]\nend\nstyle A color:#112233\n",
                    "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nsubgraph Group[Group title]\nA[Alpha]\nend\nstyle A color:#112233\n",
                ),
                (
                    "diagram-title",
                    "---\ntitle: Diagram title\n---\nflowchart TD\nA[Alpha]\nstyle A color:#112233\n",
                    "---\ntitle: Diagram title\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA[Alpha]\nstyle A color:#112233\n",
                ),
            ] {
                let source = if family == DiagramFamilyId::SWIMLANE {
                    swimlane_source
                } else {
                    flowchart_source
                };
                for profile in CutoverWitnessProfile::TEXT_LOOKS {
                    for selector in [
                        ThemeRouteCutoverSelector::StaticUnqualified,
                        ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default),
                    ] {
                        let render = |value| {
                            let route = inventory
                                .iter()
                                .copied()
                                .find(|route| {
                                    route.family_id() == family
                                        && route.target() == ThemeTarget::Text
                                        && route.facet() == ThemeRouteCutoverFacet::Fill
                                        && route.selector() == selector
                                        && route.value() == value
                                })
                                .expect("Flowchart text fill route");
                            let id = CutoverWitnessId::new(route, profile);
                            let case = super::CutoverCase { id, source };
                            let rendered = super::render_cutover_case(case, vec![route])
                                .unwrap_or_else(|error| {
                                    panic!("{family}/{channel}/{profile:?}: {error}")
                                });
                            (route, rendered.document)
                        };
                        let (solid_route, solid) = render(ThemeRouteCutoverValue::Solid);
                        let (transparent_route, transparent) =
                            render(ThemeRouteCutoverValue::Transparent);
                        let pair = merman::__theme_acceptance::export_theme_route_cutover_png_pair(
                            &solid,
                            &transparent,
                            solid_route,
                            transparent_route,
                            &merman_export::RasterOptions::default().with_scale(2.0),
                            merman::OperationControl::new(),
                        )
                        .unwrap_or_else(|error| panic!("{family}/{channel}/{profile:?}: {error}"));
                        for receipt in [pair.solid().receipt(), pair.transparent().receipt()] {
                            super::validate_cutover_target_receipt(
                                CutoverTargetAdmissionContract::PortableNativePngV1,
                                receipt,
                            )
                            .expect("isolated text channel must satisfy native target admission");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn class_text_channels_have_independent_native_png_evidence() {
        let inventory = legacy_replacing_typed_theme_routes().expect("route inventory");
        // Source-owned node text keeps unrelated glyphs identical in each pair. A passing PNG
        // comparison must therefore come from the named, independently unowned text channel.
        for (channel, source) in [
            (
                "node",
                "classDiagram\nclass Alpha {\n+String id\n+render() String\n}\n",
            ),
            (
                "namespace",
                "classDiagram\nnamespace Internal {\nclass Alpha\n}\nstyle Alpha color:#112233\n",
            ),
            (
                "edge",
                "classDiagram\nAlpha --> Beta : relates\nstyle Alpha color:#112233\nstyle Beta color:#112233\n",
            ),
            (
                "start-cardinality",
                "classDiagram\nAlpha \"1\" --> Beta\nstyle Alpha color:#112233\nstyle Beta color:#112233\n",
            ),
            (
                "end-cardinality",
                "classDiagram\nAlpha --> \"many\" Beta\nstyle Alpha color:#112233\nstyle Beta color:#112233\n",
            ),
            (
                "note",
                "classDiagram\nclass Alpha\nnote for Alpha \"annotated\"\nstyle Alpha color:#112233\n",
            ),
            (
                "diagram-title",
                "---\ntitle: Class text terminals\n---\nclassDiagram\nclass Alpha\nstyle Alpha color:#112233\n",
            ),
        ] {
            for profile in CutoverWitnessProfile::TEXT_LOOKS {
                for selector in [
                    ThemeRouteCutoverSelector::StaticUnqualified,
                    ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Default),
                ] {
                    let render = |value| {
                        let route = inventory
                            .iter()
                            .copied()
                            .find(|route| {
                                route.family_id() == DiagramFamilyId::CLASS
                                    && route.target() == ThemeTarget::Text
                                    && route.facet() == ThemeRouteCutoverFacet::Fill
                                    && route.selector() == selector
                                    && route.value() == value
                            })
                            .expect("Class text fill route");
                        let case = super::CutoverCase {
                            id: CutoverWitnessId::new(route, profile),
                            source,
                        };
                        let rendered = super::render_cutover_case(case, vec![route])
                            .unwrap_or_else(|error| {
                                panic!("{channel}/{profile:?}/{selector:?}: {error}")
                            });
                        (route, rendered.document)
                    };
                    let (solid_route, solid) = render(ThemeRouteCutoverValue::Solid);
                    let (transparent_route, transparent) =
                        render(ThemeRouteCutoverValue::Transparent);
                    let pair = merman::__theme_acceptance::export_theme_route_cutover_png_pair(
                        &solid,
                        &transparent,
                        solid_route,
                        transparent_route,
                        &merman_export::RasterOptions::default().with_scale(2.0),
                        merman::OperationControl::new(),
                    )
                    .unwrap_or_else(|error| panic!("{channel}/{profile:?}/{selector:?}: {error}"));
                    for receipt in [pair.solid().receipt(), pair.transparent().receipt()] {
                        super::validate_cutover_target_receipt(
                            CutoverTargetAdmissionContract::PortableNativePngV1,
                            receipt,
                        )
                        .expect("isolated Class text channel must satisfy native target admission");
                    }
                }
            }
        }
    }

    #[test]
    fn edge_label_background_cutover_has_native_terminal_proof() {
        let inventory = legacy_replacing_typed_theme_routes().expect("route inventory");
        for family in [
            DiagramFamilyId::BLOCK,
            DiagramFamilyId::FLOWCHART,
            DiagramFamilyId::SWIMLANE,
        ] {
            for profile in CutoverWitnessProfile::TEXT_LOOKS {
                let render = |value| {
                    let route = inventory
                        .iter()
                        .copied()
                        .find(|route| {
                            route.family_id() == family
                                && route.target() == ThemeTarget::EdgeLabelBackground
                                && route.selector() == ThemeRouteCutoverSelector::StaticUnqualified
                                && route.value() == value
                        })
                        .expect("background route");
                    let id = CutoverWitnessId::new(route, profile);
                    let case = super::CutoverCase {
                        id,
                        source: super::source_for_witness(id).unwrap(),
                    };
                    let rendered = super::render_cutover_case(case, vec![route])
                        .unwrap_or_else(|error| panic!("{family}/{profile:?}: {error}"));
                    (route, rendered.document)
                };
                let (solid_route, solid) = render(ThemeRouteCutoverValue::Solid);
                let (transparent_route, transparent) = render(ThemeRouteCutoverValue::Transparent);
                merman::__theme_acceptance::export_theme_route_cutover_png_pair(
                    &solid,
                    &transparent,
                    solid_route,
                    transparent_route,
                    &merman_export::RasterOptions::default().with_scale(2.0),
                    merman::OperationControl::new(),
                )
                .unwrap_or_else(|error| panic!("{family}/{profile:?}: {error}"));
            }
        }
    }

    #[test]
    fn flowchart_cluster_label_cutover_has_native_terminal_proof() {
        let inventory = legacy_replacing_typed_theme_routes().expect("route inventory");
        for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
            for selector in [
                ThemeRouteCutoverSelector::StaticUnqualified,
                ThemeRouteCutoverSelector::StaticVariant(
                    merman_render::diagram_theme::ThemeVariant::Default,
                ),
            ] {
                for profile in CutoverWitnessProfile::TEXT_LOOKS {
                    let render = |value| {
                        let route = inventory
                            .iter()
                            .copied()
                            .find(|route| {
                                route.family_id() == family
                                    && route.target() == ThemeTarget::ClusterLabel
                                    && route.selector() == selector
                                    && route.value() == value
                            })
                            .expect("cluster label route");
                        let id = CutoverWitnessId::new(route, profile);
                        let case = super::CutoverCase {
                            id,
                            source: super::source_for_witness(id).unwrap(),
                        };
                        let rendered = super::render_cutover_case(case, vec![route])
                            .unwrap_or_else(|error| panic!("{family}/{profile:?}: {error}"));
                        (route, rendered.document)
                    };
                    let (solid_route, solid) = render(ThemeRouteCutoverValue::Solid);
                    let (transparent_route, transparent) =
                        render(ThemeRouteCutoverValue::Transparent);
                    merman::__theme_acceptance::export_theme_route_cutover_png_pair(
                        &solid,
                        &transparent,
                        solid_route,
                        transparent_route,
                        &merman_export::RasterOptions::default().with_scale(2.0),
                        merman::OperationControl::new(),
                    )
                    .unwrap_or_else(|error| panic!("{family}/{profile:?}: {error}"));
                }
            }
        }
    }

    #[test]
    fn edge_fill_cutover_has_native_stroke_proof() {
        let inventory = legacy_replacing_typed_theme_routes().expect("route inventory");
        for family in [
            DiagramFamilyId::FLOWCHART,
            DiagramFamilyId::SWIMLANE,
            DiagramFamilyId::BLOCK,
        ] {
            for selector in [
                ThemeRouteCutoverSelector::StaticUnqualified,
                ThemeRouteCutoverSelector::StaticVariant(
                    merman_render::diagram_theme::ThemeVariant::Default,
                ),
            ] {
                for profile in if family == DiagramFamilyId::BLOCK {
                    CutoverWitnessProfile::TEXT_LOOKS
                } else {
                    CutoverWitnessProfile::EDGE
                } {
                    let render = |value| {
                        let route = inventory
                            .iter()
                            .copied()
                            .find(|route| {
                                route.family_id() == family
                                    && route.target() == ThemeTarget::Edge
                                    && route.facet() == ThemeRouteCutoverFacet::Fill
                                    && route.selector() == selector
                                    && route.value() == value
                            })
                            .expect("edge fill route");
                        assert_eq!(route.raster_paint_facet(), ThemeRouteCutoverFacet::Stroke);
                        let id = CutoverWitnessId::new(route, profile);
                        let case = super::CutoverCase {
                            id,
                            source: super::source_for_witness(id).unwrap(),
                        };
                        let rendered = super::render_cutover_case(case, vec![route])
                            .unwrap_or_else(|error| panic!("{family}/{profile:?}: {error}"));
                        (route, rendered.document)
                    };
                    let (solid_route, solid) = render(ThemeRouteCutoverValue::Solid);
                    let (transparent_route, transparent) =
                        render(ThemeRouteCutoverValue::Transparent);
                    merman::__theme_acceptance::export_theme_route_cutover_png_pair(
                        &solid,
                        &transparent,
                        solid_route,
                        transparent_route,
                        &merman_export::RasterOptions::default().with_scale(2.0),
                        merman::OperationControl::new(),
                    )
                    .unwrap_or_else(|error| panic!("{family}/{profile:?}: {error}"));
                }
            }
        }
    }

    #[test]
    fn every_legacy_replacing_typed_route_has_terminal_svg_and_png_proof() {
        let authorization = run_route_cutover_witnesses().expect("prove typed bridge cutovers");

        let inventory = legacy_replacing_typed_theme_routes().expect("derive route inventory");
        assert!(!inventory.is_empty());
        assert_ne!(authorization.manifest_digest(), &[0; 32]);
        assert_ne!(authorization.authorization_digest(), &[0; 32]);
    }

    #[test]
    fn route_authorization_binds_manifest_and_aggregate_report_digests() {
        let baseline = RouteCutoverAuthorizationReceipt::for_test([1; 32], [2; 32]);
        let changed_manifest = RouteCutoverAuthorizationReceipt::for_test([5; 32], [2; 32]);
        let changed_report = RouteCutoverAuthorizationReceipt::for_test([1; 32], [6; 32]);

        assert_ne!(baseline.digest(), changed_manifest.digest());
        assert_ne!(baseline.digest(), changed_report.digest());
    }

    #[test]
    fn route_inventory_retains_486_routes_and_754_artifact_witnesses() {
        let inventory = legacy_replacing_typed_theme_routes().expect("derive route inventory");

        assert_eq!(inventory.len(), 486);
        assert_eq!(expected_cutover_witnesses(&inventory).len(), 754);
    }

    #[test]
    fn route_manifest_digest_matches_frozen_authority() {
        let manifest = super::authorize_cutover_routes(
            legacy_replacing_typed_theme_routes().expect("derive current route inventory"),
        )
        .expect("authorize current route inventory");
        let digest =
            super::authorized_manifest_digest(manifest.manifest_version(), manifest.routes());

        assert_eq!(digest, EXPECTED_AUTHORIZED_MANIFEST_DIGEST);
    }

    #[test]
    fn route_receipt_binds_witness_target_receipts_and_renderer_receipt() {
        let inventory = legacy_replacing_typed_theme_routes().expect("derive route inventory");
        let edge_route = inventory
            .iter()
            .copied()
            .find(|route| {
                route.family_id() == DiagramFamilyId::FLOWCHART
                    && route.target() == ThemeTarget::Edge
            })
            .expect("Flowchart Edge route");
        let classic = CutoverWitnessId::new(edge_route, CutoverWitnessProfile::ClassicStatic);
        let animated = CutoverWitnessId::new(edge_route, CutoverWitnessProfile::NeoAnimated);
        let seal = |witness, svg_receipt, png_receipt, png_semantic_receipt, svg_route_receipt| {
            RouteCutoverReceipt::seal_digests(
                witness,
                svg_receipt,
                png_receipt,
                png_semantic_receipt,
                svg_route_receipt,
            )
            .expect("seal route receipt")
            .digest
        };
        let baseline = seal(classic, [1; 32], [2; 32], [4; 32], [3; 32]);

        assert_eq!(baseline, seal(classic, [1; 32], [2; 32], [4; 32], [3; 32]));
        assert_ne!(baseline, seal(animated, [1; 32], [2; 32], [4; 32], [3; 32]));
        assert_ne!(baseline, seal(classic, [5; 32], [2; 32], [4; 32], [3; 32]));
        assert_ne!(baseline, seal(classic, [1; 32], [5; 32], [4; 32], [3; 32]));
        assert_ne!(baseline, seal(classic, [1; 32], [2; 32], [5; 32], [3; 32]));
        assert_ne!(baseline, seal(classic, [1; 32], [2; 32], [4; 32], [5; 32]));
        assert!(
            RouteCutoverReceipt::seal_digests(classic, [0; 32], [2; 32], [4; 32], [3; 32]).is_err()
        );
        assert!(
            RouteCutoverReceipt::seal_digests(classic, [1; 32], [2; 32], [0; 32], [3; 32]).is_err()
        );
    }

    #[test]
    fn paint_route_svg_admission_allows_only_the_exact_font_seal_residual() {
        let contract = CutoverTargetAdmissionContract::PaintStandaloneSvgV1;

        assert!(contract.accepts(
            TargetAdmissionStatus::Portable,
            &[],
            TargetFontSource::Embedded,
        ));
        assert!(contract.accepts(
            TargetAdmissionStatus::HostDependent,
            &[TargetAdmissionReason::SvgFontsNotSelfContained],
            TargetFontSource::Embedded,
        ));
        assert!(contract.accepts(
            TargetAdmissionStatus::HostDependent,
            &[TargetAdmissionReason::SvgFontsNotSelfContained],
            TargetFontSource::None,
        ));
        assert!(!contract.accepts(
            TargetAdmissionStatus::HostDependent,
            &[TargetAdmissionReason::HostDependentTextLayout],
            TargetFontSource::Embedded,
        ));
        assert!(!contract.accepts(
            TargetAdmissionStatus::HostDependent,
            &[
                TargetAdmissionReason::SvgFontsNotSelfContained,
                TargetAdmissionReason::HostDependentTextLayout,
            ],
            TargetFontSource::Embedded,
        ));
        assert!(!contract.accepts(
            TargetAdmissionStatus::Rejected,
            &[TargetAdmissionReason::SvgFontsNotSelfContained],
            TargetFontSource::Embedded,
        ));
        assert!(!contract.accepts(
            TargetAdmissionStatus::HostDependent,
            &[TargetAdmissionReason::SvgFontsNotSelfContained],
            TargetFontSource::System,
        ));
    }

    #[test]
    fn native_png_route_admission_remains_strictly_portable() {
        let contract = CutoverTargetAdmissionContract::PortableNativePngV1;

        assert!(contract.accepts(
            TargetAdmissionStatus::Portable,
            &[],
            TargetFontSource::Embedded,
        ));
        assert!(!contract.accepts(
            TargetAdmissionStatus::HostDependent,
            &[TargetAdmissionReason::SvgFontsNotSelfContained],
            TargetFontSource::Embedded,
        ));
        assert!(!contract.accepts(
            TargetAdmissionStatus::Portable,
            &[],
            TargetFontSource::Mixed,
        ));
    }

    #[test]
    fn text_and_gitgraph_edge_fill_routes_require_each_look_specific_witness() {
        let inventory = legacy_replacing_typed_theme_routes().expect("derive route inventory");
        for (family, target) in [
            (DiagramFamilyId::C4, ThemeTarget::Text),
            (DiagramFamilyId::KANBAN, ThemeTarget::Text),
            (DiagramFamilyId::GIT_GRAPH, ThemeTarget::Text),
            (DiagramFamilyId::GIT_GRAPH, ThemeTarget::Node),
            (DiagramFamilyId::GIT_GRAPH, ThemeTarget::NodeLabel),
            (DiagramFamilyId::GIT_GRAPH, ThemeTarget::EdgeLabel),
            (DiagramFamilyId::GIT_GRAPH, ThemeTarget::Edge),
        ] {
            let routes = inventory
                .iter()
                .copied()
                .filter(|route| {
                    route.family_id() == family
                        && route.target() == target
                        && (route.facet() == ThemeRouteCutoverFacet::Fill
                            || (family == DiagramFamilyId::GIT_GRAPH
                                && target == ThemeTarget::Node))
                })
                .collect::<Vec<_>>();
            assert_eq!(
                routes.len(),
                if family == DiagramFamilyId::GIT_GRAPH && target == ThemeTarget::Node {
                    8
                } else {
                    4
                }
            );
            for route in routes {
                for profile in [
                    CutoverWitnessProfile::ClassicStatic,
                    CutoverWitnessProfile::NeoStatic,
                    CutoverWitnessProfile::HandDrawnStatic,
                ] {
                    let mut receipts = expected_cutover_witnesses(&inventory)
                        .into_iter()
                        .map(|witness| (witness, RouteCutoverReceipt { digest: [0x5a; 32] }))
                        .collect::<BTreeMap<_, _>>();
                    assert!(
                        receipts
                            .remove(&CutoverWitnessId::new(route, profile))
                            .is_some()
                    );
                    assert!(
                        evaluate_route_receipts(inventory.clone(), receipts).is_err(),
                        "missing {profile:?} {family} witness must reject authorization"
                    );
                }
            }
        }
    }

    #[test]
    fn edge_route_receipts_require_classic_and_both_neo_profiles() {
        let inventory = legacy_replacing_typed_theme_routes().expect("derive route inventory");
        let edge_route = inventory
            .iter()
            .copied()
            .find(|route| {
                route.family_id() == DiagramFamilyId::FLOWCHART
                    && route.target() == ThemeTarget::Edge
            })
            .expect("Flowchart Edge route");
        let complete_receipts = || {
            expected_cutover_witnesses(&inventory)
                .into_iter()
                .map(|witness| (witness, RouteCutoverReceipt { digest: [0x5a; 32] }))
                .collect::<BTreeMap<_, _>>()
        };

        assert!(evaluate_route_receipts(inventory.clone(), complete_receipts()).is_ok());
        for profile in [
            CutoverWitnessProfile::ClassicStatic,
            CutoverWitnessProfile::NeoStatic,
            CutoverWitnessProfile::NeoAnimated,
        ] {
            let mut incomplete = complete_receipts();
            incomplete.remove(&CutoverWitnessId::new(edge_route, profile));
            assert!(
                evaluate_route_receipts(inventory.clone(), incomplete).is_err(),
                "missing {profile:?} Edge witness must reject cutover authorization"
            );
        }
    }
}
