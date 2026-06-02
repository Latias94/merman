#![cfg(feature = "render")]

use merman::render::HeadlessRenderer;

fn render_svg(name: &str, source: &str) -> String {
    HeadlessRenderer::new()
        .with_vendored_text_measurer()
        .with_diagram_id(name)
        .render_svg_sync(source)
        .unwrap_or_else(|err| panic!("{name}: render failed: {err}"))
        .unwrap_or_else(|| panic!("{name}: no diagram detected"))
}

fn assert_renderable_theme_signals(
    name: &str,
    svg: &str,
    expected_labels: &[&str],
    expected_colors: &[&str],
) {
    assert!(svg.starts_with("<svg"), "{name}: expected SVG output");
    assert!(!svg.contains("NaN"), "{name}: SVG leaked NaN geometry");

    let svg_without_upstream_placeholder_classes = svg
        .replace(r#"class="cluster undefined "#, r#"class="cluster "#)
        .replace(r#"class="node undefined"#, r#"class="node"#)
        .replace(r#"class="node-bkg node-undefined""#, r#"class="node-bkg""#);
    assert!(
        !svg_without_upstream_placeholder_classes.contains("undefined"),
        "{name}: SVG leaked undefined tokens"
    );

    for label in expected_labels {
        assert!(
            svg.contains(label),
            "{name}: expected rendered label {label:?}"
        );
    }

    for color in expected_colors {
        assert!(
            svg.contains(color),
            "{name}: expected visible theme color {color:?}"
        );
    }
}

#[test]
fn representative_dark_theme_diagrams_keep_visible_theme_signals() {
    let cases: &[(&str, &str, &[&str], &[&str])] = &[
        (
            "theme-flowchart",
            r##"%%{init: {"themeVariables": {"mainBkg": "#111827", "primaryTextColor": "#f8fafc", "nodeBorder": "#38bdf8", "lineColor": "#f59e0b", "edgeLabelBackground": "#0f172a", "nodeTextColor": "#f8fafc"}}}%%
flowchart TD
  A[Dark Node] -->|Readable Edge| B[Other]
"##,
            &["Dark Node", "Readable Edge", "Other"],
            &["#111827", "#f8fafc", "#38bdf8", "#f59e0b"],
        ),
        (
            "theme-sequence",
            r##"%%{init: {"themeVariables": {"actorBkg": "#111827", "actorBorder": "#38bdf8", "actorTextColor": "#f8fafc", "signalColor": "#22c55e", "signalTextColor": "#facc15", "noteBkgColor": "#1f2937", "noteBorderColor": "#f97316", "noteTextColor": "#fef3c7"}}}%%
sequenceDiagram
  participant A as Alpha
  participant B as Beta
  A->>B: Request
  Note over A,B: Dark note
"##,
            &["Alpha", "Beta", "Request", "Dark note"],
            &[
                "#111827", "#38bdf8", "#f8fafc", "#22c55e", "#facc15", "#1f2937", "#f97316",
            ],
        ),
        (
            "theme-class",
            r##"%%{init: {"themeVariables": {"classText": "#f8fafc", "mainBkg": "#111827", "nodeBorder": "#38bdf8", "lineColor": "#f59e0b", "noteBkgColor": "#1f2937", "noteBorderColor": "#f97316", "noteTextColor": "#fde68a", "strokeWidth": 4}}}%%
classDiagram
  Animal <|-- Dog
  class Animal {
    +bark()
  }
  note for Animal "Dark note"
"##,
            &["Animal", "Dog", "bark()", "Dark note"],
            &[
                "#f8fafc", "#111827", "#38bdf8", "#f59e0b", "#1f2937", "#f97316", "#fde68a",
            ],
        ),
        (
            "theme-state",
            r##"%%{init: {"themeVariables": {"transitionColor": "#22c55e", "lineColor": "#38bdf8", "stateLabelColor": "#f8fafc", "transitionLabelColor": "#facc15", "stateBkg": "#111827", "stateBorder": "#38bdf8", "specialStateColor": "#f97316", "strokeWidth": 4}}}%%
stateDiagram-v2
  [*] --> Idle: start
  Idle --> Done: finish
"##,
            &["Idle", "Done", "start", "finish"],
            &[
                "#22c55e", "#38bdf8", "#f8fafc", "#facc15", "#111827", "#f97316",
            ],
        ),
        (
            "theme-er",
            r##"%%{init: {"look": "neo", "themeVariables": {"textColor": "#f8fafc", "lineColor": "#22c55e", "mainBkg": "#111827", "nodeBorder": "#38bdf8", "nodeTextColor": "#fde68a", "tertiaryColor": "#172554", "edgeLabelBackground": "#334155", "strokeWidth": 3}}}%%
erDiagram
  CUSTOMER ||--o{ ORDER : places
  CUSTOMER {
    string name
  }
"##,
            &["CUSTOMER", "ORDER", "places", "name"],
            &[
                "#f8fafc", "#22c55e", "#111827", "#38bdf8", "#fde68a", "#172554", "#334155",
            ],
        ),
        (
            "theme-kanban",
            r##"%%{init: {"themeVariables": {"background": "#0f172a", "nodeBorder": "#38bdf8", "textColor": "#f8fafc", "git0": "#22c55e", "gitBranchLabel0": "#020617", "cScale0": "hsl(160, 80%, 40%)", "cScaleLabel0": "#f8fafc", "cScaleInv0": "#111827"}}}%%
kanban
  todo[Todo]
    card[Dark Card]@{ assigned: "Core", priority: "High" }
"##,
            &["Todo", "Dark Card", "Core"],
            &["#0f172a", "#38bdf8", "#f8fafc", "#22c55e", "#020617"],
        ),
        (
            "theme-mindmap",
            r##"%%{init: {"theme": "redux", "themeVariables": {"THEME_COLOR_LIMIT": 2, "git0": "#22c55e", "gitBranchLabel0": "#020617", "nodeBorder": "#facc15", "cScale0": "#172554", "cScaleLabel0": "#f8fafc", "cScaleInv0": "#334155"}}}%%
mindmap
  Root
    Child
"##,
            &["Root", "Child"],
            &[
                "#22c55e", "#020617", "#facc15", "#172554", "#f8fafc", "#334155",
            ],
        ),
        (
            "theme-gitgraph",
            r##"%%{init: {"themeVariables": {"git0": "#22c55e", "gitBranchLabel0": "#020617", "git1": "#38bdf8", "gitBranchLabel1": "#0f172a", "commitLabelColor": "#f8fafc", "commitLabelBackground": "#111827", "commitLineColor": "#f59e0b"}}}%%
gitGraph
  commit id: "A"
  branch dev
  checkout dev
  commit id: "B"
"##,
            &["main", "dev"],
            &[
                "#22c55e", "#020617", "#38bdf8", "#0f172a", "#f8fafc", "#111827", "#f59e0b",
            ],
        ),
        (
            "theme-c4",
            r##"%%{init: {"c4": {"person_bg_color": "#172554", "person_border_color": "#38bdf8", "system_bg_color": "#111827", "system_border_color": "#facc15"}}}%%
C4Context
title Theme C4
Person(customer, "Customer", "Uses the system")
System(system, "Internet Banking", "Core system")
Rel(customer, system, "Uses", "HTTPS")
"##,
            &[
                "Theme C4",
                "Customer",
                "Internet Banking",
                "Core system",
                "Uses",
            ],
            &["#172554", "#38bdf8", "#111827", "#facc15"],
        ),
        (
            "theme-architecture",
            r##"%%{init: {"themeVariables": {"archEdgeColor": "#22c55e", "archEdgeArrowColor": "#facc15", "archEdgeWidth": 5, "archGroupBorderColor": "#38bdf8", "archGroupBorderWidth": "4px"}}}%%
architecture-beta
  group core(cloud)[Core]
  service api(server)[API] in core
  service db(database)[DB] in core
  api:R --> L:db
"##,
            &["Core", "API", "DB"],
            &[
                "#22c55e",
                "#facc15",
                "#38bdf8",
                "stroke-width:5",
                "stroke-width:4px",
            ],
        ),
        (
            "theme-block",
            r##"%%{init: {"themeVariables": {"nodeTextColor": "#f8fafc", "clusterBkg": "#172554", "clusterBorder": "#38bdf8"}}}%%
block
  block:Core
    A["Alpha"]
  end
"##,
            &["Alpha"],
            &[
                "#f8fafc",
                "rgba(23, 37, 84, 0.5)",
                "rgba(56, 189, 248, 0.2)",
            ],
        ),
        (
            "theme-journey",
            r##"%%{init: {"themeVariables": {"textColor": "#f8fafc", "lineColor": "#22c55e", "faceColor": "#111827", "mainBkg": "#1f2937", "nodeBorder": "#38bdf8", "arrowheadColor": "#facc15", "edgeLabelBackground": "#0f172a", "titleColor": "#fde68a", "fillType0": "#172554", "actor0": "#f97316"}}}%%
journey
  title Theme Journey
  section Checkout
    Sign Up: 5: Alice
    Pay: 3: Alice
"##,
            &["Theme Journey", "Checkout", "Sign Up", "Pay", "Alice"],
            &[
                "#f8fafc", "#22c55e", "#111827", "#1f2937", "#38bdf8", "#facc15", "#fde68a",
                "#172554", "#f97316",
            ],
        ),
        (
            "theme-quadrantchart",
            r##"%%{init: {"themeVariables": {"quadrant1Fill": "#172554", "quadrant2Fill": "#1e3a8a", "quadrant3Fill": "#0f172a", "quadrant4Fill": "#111827", "quadrant1TextFill": "#f8fafc", "quadrant2TextFill": "#f8fafc", "quadrant3TextFill": "#f8fafc", "quadrant4TextFill": "#f8fafc", "quadrantPointFill": "#facc15", "quadrantPointTextFill": "#111827", "quadrantTitleFill": "#f8fafc", "quadrantXAxisTextFill": "#bfdbfe", "quadrantYAxisTextFill": "#fde68a"}}}%%
quadrantChart
  title Priority Matrix
  x-axis Low Effort --> High Effort
  y-axis Low Impact --> High Impact
  quadrant-1 Invest
  quadrant-2 Watch
  quadrant-3 Park
  quadrant-4 Delegate
  Feature: [0.7, 0.8]
"##,
            &[
                "Priority Matrix",
                "Low Effort",
                "High Impact",
                "Invest",
                "Feature",
            ],
            &[
                "#172554", "#1e3a8a", "#0f172a", "#111827", "#f8fafc", "#facc15", "#bfdbfe",
                "#fde68a",
            ],
        ),
        (
            "theme-packet",
            r##"%%{init: {"packet": {"startByteColor": "#22c55e", "endByteColor": "#f97316", "labelColor": "#f8fafc", "titleColor": "#fde68a", "blockStrokeColor": "#38bdf8", "blockStrokeWidth": 2, "blockFillColor": "#111827"}}}%%
packet
title Theme Packet
+8: "Byte"
+16: "Word"
"##,
            &["Theme Packet", "Byte", "Word"],
            &[
                "#22c55e", "#f97316", "#f8fafc", "#fde68a", "#38bdf8", "#111827",
            ],
        ),
        (
            "theme-sankey",
            r##"%%{init: {"themeVariables": {"textColor": "#f8fafc", "mainBkg": "#111827"}, "sankey": {"labelStyle": "outlined", "nodeColors": {"Source": "#22c55e", "Target": "#38bdf8"}, "showValues": true, "prefix": "$", "suffix": " units"}}}%%
sankey
Source,Target,10
Target,Done,2
"##,
            &["Source", "Target", "Done", "$10 units"],
            &["#f8fafc", "#111827", "#22c55e", "#38bdf8"],
        ),
        (
            "theme-radar",
            r##"%%{init: {"themeVariables": {"titleColor": "#f8fafc", "cScale0": "#22c55e", "radar": {"axisColor": "#38bdf8", "graticuleColor": "#1f2937"}}, "radar": {"axisColor": "#facc15", "axisStrokeWidth": 4, "axisLabelFontSize": 14, "graticuleColor": "#334155", "graticuleOpacity": 0.8, "curveOpacity": 0.9, "curveStrokeWidth": 5}}}%%
radar-beta
  title Theme Radar
  axis Speed, Quality, Cost
  curve Team{8, 7, 4}
"##,
            &["Theme Radar", "Speed", "Quality", "Cost", "Team"],
            &[
                "#f8fafc",
                "#22c55e",
                "#facc15",
                "#334155",
                "stroke-width:4",
                "stroke-width:5",
            ],
        ),
        (
            "theme-requirement",
            r##"%%{init: {"look": "neo", "themeVariables": {"relationColor": "#22c55e", "lineColor": "#38bdf8", "requirementBackground": "#111827", "requirementBorderColor": "#facc15", "requirementTextColor": "#f8fafc", "relationLabelBackground": "#1f2937", "relationLabelColor": "#fde68a", "edgeLabelBackground": "#0f172a", "requirementEdgeLabelBackground": "#334155", "nodeBorder": "#f97316", "strokeWidth": 3}}}%%
requirementDiagram
  requirement req1 {
    id: 1
    text: Dark requirement
    risk: high
    verifymethod: analysis
  }
"##,
            &[
                "req1",
                "Dark requirement",
                "Risk: High",
                "Verification: Analysis",
            ],
            &[
                "#22c55e", "#38bdf8", "#111827", "#facc15", "#f8fafc", "#1f2937", "#fde68a",
                "#0f172a", "#334155",
            ],
        ),
        (
            "theme-timeline",
            r##"%%{init: {"themeVariables": {"tertiaryColor": "#172554", "clusterBorder": "#f8fafc"}}}%%
timeline
  title Theme Timeline
  section Release
    2026 : Ship
"##,
            &["Theme Timeline", "Release", "2026", "Ship"],
            &["#172554", "#f8fafc"],
        ),
        (
            "theme-gantt",
            r##"%%{init: {"themeVariables": {"textColor": "#f8fafc", "sectionBkgColor": "#172554", "sectionBkgColor2": "#1e3a8a", "titleColor": "#fde68a", "gridColor": "#38bdf8", "taskTextColor": "#f8fafc", "taskBkgColor": "#111827", "taskBorderColor": "#facc15", "taskTextOutsideColor": "#fb923c", "doneTaskBkgColor": "#22c55e", "doneTaskBorderColor": "#16a34a"}}}%%
gantt
  title Theme Plan
  dateFormat YYYY-MM-DD
  section Core
  Ship :done, 2026-01-01, 1d
"##,
            &["Theme Plan", "Core", "Ship"],
            &[
                "#f8fafc", "#172554", "#1e3a8a", "#fde68a", "#38bdf8", "#111827", "#facc15",
                "#fb923c", "#22c55e",
            ],
        ),
        (
            "theme-treemap",
            r##"%%{init: {"themeVariables": {"textColor": "#f8fafc", "titleColor": "#fde68a"}, "treemap": {"sectionStrokeColor": "#38bdf8", "sectionFillColor": "#172554", "leafStrokeColor": "#facc15", "leafFillColor": "#111827", "labelColor": "#f8fafc", "valueColor": "#fb923c", "titleColor": "#fde68a"}}}%%
treemap-beta
  "Theme Section"
    "Theme Leaf": 42
"##,
            &["Theme Section", "Theme Leaf", "42"],
            &[
                "#f8fafc", "#fde68a", "#38bdf8", "#172554", "#facc15", "#111827", "#fb923c",
            ],
        ),
        (
            "theme-pie",
            r##"%%{init: {"themeVariables": {"pieTitleTextColor": "#f8fafc", "pieSectionTextColor": "#111827", "pieLegendTextColor": "#fde68a", "pieStrokeColor": "#38bdf8", "pieStrokeWidth": "3px"}}}%%
pie title Theme Pie
  "Alpha": 40
  "Beta": 60
"##,
            &["Theme Pie", "Alpha", "Beta"],
            &["#f8fafc", "#111827", "#fde68a", "#38bdf8", "3px"],
        ),
        (
            "theme-xychart",
            r##"%%{init: {"themeVariables": {"xyChart": {"backgroundColor": "#0f172a", "titleColor": "#f8fafc", "xAxisLabelColor": "#93c5fd", "xAxisTitleColor": "#bfdbfe", "xAxisTickColor": "#38bdf8", "xAxisLineColor": "#60a5fa", "yAxisLabelColor": "#fde68a", "yAxisTitleColor": "#facc15", "yAxisTickColor": "#f97316", "yAxisLineColor": "#fb923c", "plotColorPalette": "#22c55e,#e879f9"}}}}%%
xychart
  title "Theme Chart"
  x-axis Months [Jan, Feb]
  y-axis Revenue 0 --> 10
  bar [3, 7]
  line [3, 7]
"##,
            &["Theme Chart", "Months", "Revenue", "Jan"],
            &[
                "#0f172a", "#f8fafc", "#93c5fd", "#bfdbfe", "#38bdf8", "#60a5fa", "#fde68a",
                "#facc15", "#f97316", "#fb923c", "#22c55e", "#e879f9",
            ],
        ),
    ];

    for (name, source, expected_labels, expected_colors) in cases {
        let svg = render_svg(name, source);
        assert_renderable_theme_signals(name, &svg, expected_labels, expected_colors);
    }
}
