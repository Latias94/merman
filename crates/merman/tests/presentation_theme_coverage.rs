#![cfg(feature = "svg")]

use merman::svg::{
    CssOverridePolicy, HostTheme, HostThemePreset, Presentation, PresentationProfile,
    SvgOutputPolicy, SvgPipeline, SvgPipelinePreset, TextMeasurementPolicy, ThemeRole,
};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

const USER_GITGRAPH_THEME_REGRESSION: &str = r#"gitGraph
    commit
    commit
    branch develop
    checkout develop
    commit
    commit
    checkout main
    merge develop
    commit
    branch feature
    checkout feature
    commit
    checkout main
    merge feature
"#;

const USER_GITGRAPH_CHERRYPICK_TAG_THEME_REGRESSION: &str = r#"gitGraph
    commit id: "base"
    branch feature
    checkout feature
    commit id: "parser-fix"
    checkout main
    commit id: "release" tag: "v1.0"
    cherry-pick id: "parser-fix" tag: "backport"
"#;

const USER_ER_GRUVBOX_LABEL_REGRESSION: &str = r#"erDiagram
    USER ||--o{ ORDER : places
    USER {
        int id PK
        string name
        string email
    }
    ORDER ||--|{ ORDER_ITEM : contains
    ORDER {
        int id PK
        date created_at
        string status
    }
    ORDER_ITEM {
        int id PK
        int quantity
        float price
    }
    PRODUCT ||--o{ ORDER_ITEM : "ordered in"
    PRODUCT {
        int id PK
        string name
        float price
    }
"#;

#[derive(Debug, Clone)]
struct TypedSvgRenderer {
    renderer: Renderer,
    request: SvgRequest,
}

impl TypedSvgRenderer {
    fn new() -> Self {
        Self {
            renderer: Renderer::new(),
            request: SvgRequest::default(),
        }
    }

    fn with_presentation(mut self, presentation: Presentation) -> Self {
        let resolved = presentation.resolve();
        let engine = resolved.materialize_engine(self.renderer.engine().clone());
        self.renderer = self.renderer.with_engine(engine);
        self.request.presentation = resolved.render_policy();
        self
    }

    fn with_svg_pipeline(mut self, pipeline: SvgPipeline) -> Self {
        self.request.pipeline = Some(pipeline);
        self
    }

    fn with_vendored_text_measurer(mut self) -> Self {
        self.request.environment = self
            .request
            .environment
            .with_text_measurement_policy(TextMeasurementPolicy::parity());
        self
    }

    fn with_diagram_id(mut self, name: &str) -> Self {
        self.request.options.diagram_id = Some(name.to_string());
        self
    }

    fn render_svg(&self, source: &str) -> Result<Option<String>, merman::RenderError> {
        let output = self.renderer.render(RenderRequest::svg(
            source,
            OperationControl::new(),
            self.request.clone(),
        ))?;
        let RenderOutput::Svg(svg) = output else {
            unreachable!("SVG request must return SVG output")
        };
        Ok(svg.map(|svg| svg.into_parts().0))
    }
}

fn themed_renderer(preset: HostThemePreset, name: &str) -> TypedSvgRenderer {
    let theme = HostTheme::from_preset(preset);
    let background = theme
        .role(ThemeRole::Canvas)
        .expect("bundled themes must define a canvas color")
        .to_string();
    let pipeline = SvgOutputPolicy {
        preset: SvgPipelinePreset::ResvgSafe,
        css_override_policy: CssOverridePolicy::StripExistingImportant,
        root_background_color: Some(background),
        drop_native_duplicate_fallbacks: false,
        scoped_css: None,
    }
    .pipeline();

    TypedSvgRenderer::new()
        .with_presentation(Presentation::new().with_theme(theme))
        .with_svg_pipeline(pipeline)
        .with_vendored_text_measurer()
        .with_diagram_id(name)
}

fn render_with_editor_dark_theme(name: &str, source: &str) -> String {
    themed_renderer(HostThemePreset::EditorDark, name)
        .render_svg(source)
        .unwrap_or_else(|err| panic!("{name}: render failed: {err}"))
        .unwrap_or_else(|| panic!("{name}: no diagram detected"))
}

#[test]
fn empty_presentation_preserves_default_svg_bytes() {
    let source = "sequenceDiagram\nAlice->>Bob: Hello";
    let default_svg = TypedSvgRenderer::new()
        .with_diagram_id("empty-profile-parity")
        .render_svg(source)
        .expect("default render should succeed")
        .expect("sequence diagram should be detected");
    let empty_presentation_svg = TypedSvgRenderer::new()
        .with_presentation(Presentation::new())
        .with_diagram_id("empty-profile-parity")
        .render_svg(source)
        .expect("empty presentation render should succeed")
        .expect("sequence diagram should be detected");

    assert_eq!(empty_presentation_svg, default_svg);
}

#[test]
fn merman_modern_profile_renders_non_flowchart_without_elk() {
    let source = "sequenceDiagram\nAlice->>Bob: Hello";
    let svg = TypedSvgRenderer::new()
        .with_presentation(Presentation::new().with_profile(PresentationProfile::MermanModern))
        .with_diagram_id("modern-non-flowchart")
        .render_svg(source)
        .expect("modern profile should not require ELK for sequence diagrams")
        .expect("sequence diagram should be detected");

    assert!(svg.contains(r#"aria-roledescription="sequence""#), "{svg}");
}

fn assert_contains_all(name: &str, svg: &str, expected: &[&str]) {
    assert!(svg.starts_with("<svg"), "{name}: expected SVG");
    assert!(!svg.contains("NaN"), "{name}: leaked NaN");
    assert!(
        !svg.contains("<foreignObject"),
        "{name}: explicit output policy should use resvg-safe output"
    );
    for needle in expected {
        assert!(
            svg.contains(needle),
            "{name}: expected {needle:?} in SVG: {svg}"
        );
    }
}

#[cfg(feature = "layout-cytoscape")]
fn assert_current_dom_consumes(name: &str, svg: &str, expected: &[&str]) {
    let normalized_svg = svg
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect::<String>();
    for needle in expected {
        let found = if needle.contains('{') {
            let normalized_needle = needle
                .chars()
                .filter(|ch| !ch.is_ascii_whitespace())
                .collect::<String>();
            normalized_svg.contains(&normalized_needle)
        } else {
            svg.contains(needle)
        };
        assert!(
            found,
            "{name}: expected current DOM/CSS surface {needle:?} in SVG: {svg}"
        );
    }
}

fn assert_gitgraph_branch_label_baselines_centered(name: &str, svg: &str, expected: &[&str]) {
    let document =
        roxmltree::Document::parse(svg).unwrap_or_else(|err| panic!("{name}: invalid SVG: {err}"));
    let mut seen = Vec::new();

    for label_group in document.descendants().filter(|node| {
        node.is_element()
            && node.tag_name().name() == "g"
            && node.attribute("class").is_some_and(|classes| {
                classes.split_whitespace().any(|class| class == "label")
                    && classes
                        .split_whitespace()
                        .any(|class| class.starts_with("branch-label"))
            })
    }) {
        let Some(text) = label_group
            .children()
            .find(|node| node.is_element() && node.tag_name().name() == "text")
        else {
            panic!("{name}: branch label group is missing text: {svg}");
        };
        assert_eq!(
            text.attribute("dominant-baseline"),
            Some("central"),
            "{name}: branch label text should use a stable central baseline: {svg}"
        );
        assert_eq!(
            text.attribute("alignment-baseline"),
            Some("central"),
            "{name}: branch label text should use a stable central alignment: {svg}"
        );
        assert!(
            text.attribute("y").is_some(),
            "{name}: branch label text should carry explicit centered y: {svg}"
        );

        let Some(tspan) = text
            .children()
            .find(|node| node.is_element() && node.tag_name().name() == "tspan")
        else {
            panic!("{name}: branch label text is missing tspan: {svg}");
        };
        assert_eq!(
            tspan.attribute("dy"),
            Some("0"),
            "{name}: branch label tspan should not rely on font-sensitive dy=1em: {svg}"
        );
        if let Some(label) = tspan.text() {
            seen.push(label.to_string());
        }
    }

    for expected_label in expected {
        assert!(
            seen.iter().any(|label| label == expected_label),
            "{name}: expected branch label {expected_label:?}, saw {seen:?}: {svg}"
        );
    }
}

fn assert_gitgraph_branch_labels_keep_mermaid_parity_baseline(name: &str, svg: &str) {
    let document =
        roxmltree::Document::parse(svg).unwrap_or_else(|err| panic!("{name}: invalid SVG: {err}"));
    let mut checked = 0usize;

    for label_group in document.descendants().filter(|node| {
        node.is_element()
            && node.tag_name().name() == "g"
            && node.attribute("class").is_some_and(|classes| {
                classes.split_whitespace().any(|class| class == "label")
                    && classes
                        .split_whitespace()
                        .any(|class| class.starts_with("branch-label"))
            })
    }) {
        let text = label_group
            .children()
            .find(|node| node.is_element() && node.tag_name().name() == "text")
            .unwrap_or_else(|| panic!("{name}: branch label group is missing text: {svg}"));
        assert_eq!(
            text.attribute("dominant-baseline"),
            None,
            "{name}: parity SVG should preserve Mermaid's raw branch label text shape: {svg}"
        );
        let tspan = text
            .children()
            .find(|node| node.is_element() && node.tag_name().name() == "tspan")
            .unwrap_or_else(|| panic!("{name}: branch label text is missing tspan: {svg}"));
        assert_eq!(
            tspan.attribute("dy"),
            Some("1em"),
            "{name}: parity SVG should keep Mermaid's dy=1em branch label baseline: {svg}"
        );
        checked += 1;
    }

    assert!(
        checked >= 3,
        "{name}: expected main/develop/feature branch labels, checked {checked}: {svg}"
    );
}

fn assert_er_edge_label_fallbacks_are_readable(name: &str, svg: &str, labels: &[&str]) {
    fn find_usvg_text_fill(group: &usvg::Group, label: &str) -> Option<(u8, u8, u8)> {
        for node in group.children() {
            match node {
                usvg::Node::Group(group) => {
                    if let Some(fill) = find_usvg_text_fill(group, label) {
                        return Some(fill);
                    }
                }
                usvg::Node::Text(text) => {
                    for chunk in text.chunks() {
                        if chunk.text().trim() != label {
                            continue;
                        }
                        let span = chunk.spans().first()?;
                        let fill = span.fill()?;
                        if let usvg::Paint::Color(color) = fill.paint() {
                            return Some((color.red, color.green, color.blue));
                        }
                    }
                }
                usvg::Node::Path(_) | usvg::Node::Image(_) => {}
            }
        }
        None
    }

    let document =
        roxmltree::Document::parse(svg).unwrap_or_else(|err| panic!("{name}: invalid SVG: {err}"));
    let mut options = usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let usvg_tree = usvg::Tree::from_str(svg, &options)
        .unwrap_or_else(|err| panic!("{name}: usvg should parse final fallback output: {err}"));

    for label in labels {
        let Some(text) = document.descendants().find(|node| {
            node.is_element()
                && node.tag_name().name() == "text"
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|class| class == "merman-foreignobject-fallback-text")
                })
                && node.text() == Some(*label)
        }) else {
            panic!("{name}: missing fallback text for ER label {label:?}: {svg}");
        };

        assert_eq!(
            text.attribute("fill"),
            Some("#ebdbb2"),
            "{name}: gruvbox ER fallback label should use readable text color: {svg}"
        );
        assert!(
            text.attribute("style").is_some_and(
                |style| style.contains("font-size:14px") || style.contains("font-size: 14px")
            ),
            "{name}: ER fallback label should inherit presentation theme font size: {svg}"
        );
        assert!(
            text.attribute("class")
                .is_some_and(|classes| !classes.split_whitespace().any(|class| class == "label")),
            "{name}: fallback text should not carry structural Mermaid label class: {svg}"
        );
        assert_eq!(
            find_usvg_text_fill(usvg_tree.root(), label),
            Some((0xeb, 0xdb, 0xb2)),
            "{name}: usvg must paint ER fallback text with XHTML color semantics: {svg}"
        );
    }
}

#[test]
fn presentation_theme_covers_core_diagram_roles() {
    let cases: &[(&str, &str, &[&str])] = &[
        (
            "presentation-theme-flowchart",
            "flowchart TD\n  A[Host] -->|Edge| B[Theme]",
            &["#111827", "#e5e7eb", "#475569", "#94a3b8"],
        ),
        (
            "presentation-theme-sequence",
            "sequenceDiagram\n  participant A as Alpha\n  participant B as Beta\n  A->>B: Hello\n  Note over A,B: Profile note",
            &["#1f2937", "#e5e7eb", "#94a3b8", "#422006", "#f59e0b"],
        ),
        (
            "presentation-theme-class",
            "classDiagram\n  Animal <|-- Dog\n  class Animal {\n    +bark()\n  }\n  note for Animal \"Profile note\"",
            &["#111827", "#e5e7eb", "#475569", "#422006", "#f59e0b"],
        ),
        (
            "presentation-theme-state",
            "stateDiagram-v2\n  [*] --> Idle: start\n  Idle --> Done: finish",
            &["#111827", "#e5e7eb", "#94a3b8"],
        ),
        (
            "presentation-theme-xychart",
            "xychart-beta\n  title Profile\n  x-axis [\"A\", \"B\"]\n  y-axis \"Value\" 0 --> 10\n  bar [4, 7]",
            &["#60a5fa", "#e5e7eb"],
        ),
        (
            "presentation-theme-pie",
            "pie title Profile Pie\n  \"A\" : 4\n  \"B\" : 7",
            &["#60a5fa", "#34d399", "#e5e7eb"],
        ),
        (
            "presentation-theme-quadrant",
            "quadrantChart\n  title Profile Matrix\n  x-axis Low --> High\n  y-axis Low --> High\n  quadrant-1 Invest\n  A: [0.7, 0.8]",
            &["#111827", "#1f2937", "#e5e7eb", "#94a3b8"],
        ),
    ];

    for (name, source, expected) in cases {
        let svg = render_with_editor_dark_theme(name, source);
        assert_contains_all(name, &svg, expected);
    }
}

#[test]
#[cfg(feature = "layout-cytoscape")]
fn presentation_theme_series_palette_reaches_ordinal_diagrams() {
    let cases: &[(&str, &str, &[&str])] = &[
        (
            "presentation-theme-mindmap",
            "mindmap\n  Root\n    Child",
            &["#60a5fa", "#34d399"],
        ),
        (
            "presentation-theme-gitgraph",
            "gitGraph\n  commit id: \"A\"\n  branch dev\n  checkout dev\n  commit id: \"B\"",
            &["#60a5fa", "#34d399"],
        ),
        (
            "presentation-theme-journey",
            "journey\n  title Profile Journey\n  section Checkout\n    Sign Up: 5: Alice\n    Pay: 3: Bob",
            &["#60a5fa", "#34d399"],
        ),
        (
            "presentation-theme-timeline",
            "timeline\n  title Profile Timeline\n  section 2026\n    Alpha : Start\n    Beta : Ship",
            &["#60a5fa", "#34d399"],
        ),
        (
            "presentation-theme-venn",
            "venn-beta\n  set A[\"Core\"]:10\n  set B[\"Editor\"]:8\n  union A,B[\"Shared\"]:3",
            &["#60a5fa", "#34d399"],
        ),
    ];

    for (name, source, expected) in cases {
        let svg = render_with_editor_dark_theme(name, source);
        assert_contains_all(name, &svg, expected);
    }
}

#[test]
fn gruvbox_presentation_theme_keeps_er_relationship_label_fallbacks_readable() {
    let svg = themed_renderer(HostThemePreset::GruvboxDark, "gruvbox-er-labels")
        .render_svg(USER_ER_GRUVBOX_LABEL_REGRESSION)
        .unwrap_or_else(|err| panic!("gruvbox ER render failed: {err}"))
        .unwrap_or_else(|| panic!("gruvbox ER render produced no diagram"));

    assert_contains_all(
        "gruvbox-er-labels",
        &svg,
        &[
            "#ebdbb2",
            "font-size:14px",
            "places",
            "contains",
            "ordered in",
        ],
    );
    assert_er_edge_label_fallbacks_are_readable(
        "gruvbox-er-labels",
        &svg,
        &["places", "contains", "ordered in"],
    );
}

#[test]
fn presentation_theme_centers_gitgraph_branch_labels_with_editor_fonts() {
    let plain = TypedSvgRenderer::new()
        .with_vendored_text_measurer()
        .with_diagram_id("gitgraph-plain-baseline")
        .render_svg(USER_GITGRAPH_THEME_REGRESSION)
        .unwrap_or_else(|err| panic!("plain gitGraph render failed: {err}"))
        .unwrap_or_else(|| panic!("plain gitGraph render produced no diagram"));
    assert_gitgraph_branch_labels_keep_mermaid_parity_baseline("plain-gitgraph", &plain);

    let themed = themed_renderer(HostThemePreset::OneDark, "gitgraph-one-dark-baseline")
        .render_svg(USER_GITGRAPH_THEME_REGRESSION)
        .unwrap_or_else(|err| panic!("one-dark gitGraph render failed: {err}"))
        .unwrap_or_else(|| panic!("one-dark gitGraph render produced no diagram"));
    assert_gitgraph_branch_label_baselines_centered(
        "one-dark-gitgraph",
        &themed,
        &["main", "develop", "feature"],
    );

    let cherry_pick = themed_renderer(HostThemePreset::OneDark, "gitgraph-one-dark-cherry-pick")
        .render_svg(USER_GITGRAPH_CHERRYPICK_TAG_THEME_REGRESSION)
        .unwrap_or_else(|err| panic!("one-dark cherry-pick gitGraph render failed: {err}"))
        .unwrap_or_else(|| panic!("one-dark cherry-pick gitGraph render produced no diagram"));
    assert_gitgraph_branch_label_baselines_centered(
        "one-dark-cherry-pick-gitgraph",
        &cherry_pick,
        &["main", "feature"],
    );
}

#[test]
#[cfg(feature = "layout-cytoscape")]
fn presentation_theme_covers_additional_current_diagram_surfaces() {
    let cases: &[(&str, &str, &[&str], &[&str])] = &[
        (
            "presentation-theme-er",
            "erDiagram\n  CUSTOMER ||--o{ ORDER : places\n  CUSTOMER {\n    string name\n  }",
            &["#111827", "#e5e7eb", "#94a3b8", "#475569"],
            &[
                ".entityBox{fill:#111827;stroke:#475569;}",
                ".relationshipLine{stroke:#94a3b8;",
            ],
        ),
        (
            "presentation-theme-requirement",
            "requirementDiagram\n  requirement req1 {\n    id: 1\n    text: Host requirement\n    risk: high\n    verifymethod: analysis\n  }\n  element sys {\n    type: system\n  }\n  sys - satisfies -> req1",
            &["#111827", "#e5e7eb", "#475569", "#94a3b8"],
            &[
                "fill=\"#111827\"",
                "stroke=\"#475569\" stroke-width=\"1.3\"",
                ".relationshipLine{stroke:#94a3b8;",
            ],
        ),
        (
            "presentation-theme-gantt",
            "gantt\n  title Profile Plan\n  dateFormat YYYY-MM-DD\n  section Core\n  Build : 2026-01-01, 15d\n  Critical :crit, 2026-01-16, 2d\n  Ship :done, 2026-01-18, 3d",
            &[
                "#e5e7eb", "#1f2937", "#475569", "#34d399", "#f87171", "#fbbf24",
            ],
            &[
                ".grid .tick{stroke:#475569;",
                ".done0,#presentation-theme-gantt .done1",
                "{stroke:#34d399;fill:#1f2937;",
                ".crit0,#presentation-theme-gantt .crit1",
                "{stroke:#f87171;fill:#1f2937;",
            ],
        ),
        (
            "presentation-theme-architecture",
            "architecture-beta\n  group core(cloud)[Core]\n  service api(server)[API] in core\n  service db(database)[DB] in core\n  api:R --> L:db",
            &["#94a3b8", "#475569", "#e5e7eb"],
            &[
                ".edge{stroke-width:3;stroke:#94a3b8;fill:none;}",
                ".node-bkg{fill:none;stroke:#475569;",
            ],
        ),
        (
            "presentation-theme-block",
            "block\n  block:Core\n    A[\"Alpha\"]\n    B[\"Beta\"]\n  end\n  A --> B",
            &["#e5e7eb", "rgba(30, 41, 59, 0.5)", "#475569", "#94a3b8"],
            &[
                "class=\"edge-thickness-normal edge-pattern-solid",
                ".node .cluster{fill:rgba(30, 41, 59, 0.5);stroke:rgba(71, 85, 105, 0.2);",
            ],
        ),
        (
            "presentation-theme-kanban",
            "kanban\n  todo[Todo]\n    card[Dark Card]@{ assigned: \"Core\", priority: \"High\" }",
            &["#60a5fa", "#34d399", "#e5e7eb", "#475569"],
            &[
                ".section-root rect,#presentation-theme-kanban .section-root path",
                ".node rect,#presentation-theme-kanban .node circle",
            ],
        ),
        (
            "presentation-theme-packet",
            "packet\ntitle Profile Packet\n+8: \"Byte\"\n+16: \"Word\"",
            &["#94a3b8", "#475569", "#e5e7eb", "#111827"],
            &[
                ".packetByte.start{fill:#94a3b8;}",
                ".packetBlock{stroke:#475569;stroke-width:1;fill:#111827;}",
            ],
        ),
        (
            "presentation-theme-sankey",
            "sankey\nSource,Target,10\nTarget,Done,2",
            &["#e5e7eb", "#111827"],
            &[
                ".sankey-label-bg{stroke:#111827;stroke-width:4px;",
                ".sankey-label-fg{fill:#e5e7eb;}",
            ],
        ),
        (
            "presentation-theme-radar",
            "radar-beta\n  title Profile Radar\n  axis Speed, Quality, Cost\n  curve Team{8, 7, 4}",
            &["#e5e7eb", "#60a5fa", "#94a3b8", "#475569"],
            &[
                ".radarAxisLine{stroke:#94a3b8;stroke-width:2;}",
                ".radarGraticule{fill:#475569;fill-opacity:0.3;stroke:#475569;",
                ".radarCurve-0{color:#60a5fa;fill:#60a5fa;",
            ],
        ),
        (
            "presentation-theme-treemap",
            "treemap-beta\n  \"Profile Section\"\n    \"Profile Leaf\": 42",
            &["#e5e7eb", "#cbd5e1", "#475569", "#1f2937", "#111827"],
            &[
                ".treemapNode.section{stroke:#475569;stroke-width:1;fill:#1f2937;}",
                ".treemapNode.leaf{stroke:#475569;stroke-width:1;fill:#111827;}",
            ],
        ),
        (
            "presentation-theme-c4",
            "C4Component\nComponentDb(db, \"Database\", \"Postgres\", \"Stores data\")\nComponentQueue(queue, \"Queue\", \"NATS\", \"Events\")",
            &["#111827", "#475569"],
            &[
                "fill=\"#111827\" stroke-width=\"0.5\" stroke=\"#475569\"",
                "stroke-width=\"0.5\" stroke=\"#475569\"",
            ],
        ),
        (
            "presentation-theme-tree-view",
            include_str!("../../../fixtures/treeView/upstream_docs_treeview_basic.mmd"),
            &["#e5e7eb", "#94a3b8"],
            &[
                ".treeView-node-label { font-size: 16px; fill: #e5e7eb; white-space: pre; }",
                ".treeView-node-line { stroke: #94a3b8; }",
            ],
        ),
        (
            "presentation-theme-ishikawa",
            include_str!(
                "../../../fixtures/ishikawa/upstream_cypress_ishikawa_spec_1_should_render_a_simple_ishikawa_diagram_001.mmd"
            ),
            &["#e5e7eb", "#111827", "#94a3b8"],
            &[
                ".ishikawa .ishikawa-spine,.ishikawa .ishikawa-branch,.ishikawa .ishikawa-sub-branch { stroke: #94a3b8;",
                ".ishikawa .ishikawa-head { fill: #111827; stroke: #94a3b8;",
            ],
        ),
        (
            "presentation-theme-eventmodeling",
            include_str!("../../../fixtures/eventmodeling/upstream_docs_eventmodeling_minimum.mmd"),
            &[
                "#e5e7eb", "#111827", "#1e293b", "#475569", "#34d399", "#60a5fa", "#f59e0b",
            ],
            &[
                "class=\"em-swimlane\"><rect",
                "fill=\"#1e293b\" stroke=\"#475569\"",
                "stroke=\"#475569\" fill=\"#111827\"",
                "stroke=\"#34d399\" fill=\"#34d399\"",
                "stroke=\"#94a3b8\" fill=\"#60a5fa\"",
                "stroke=\"#fbbf24\" fill=\"#f59e0b\"",
            ],
        ),
    ];

    for (name, source, expected, dom_expected) in cases {
        let svg = render_with_editor_dark_theme(name, source);
        assert_contains_all(name, &svg, expected);
        assert_current_dom_consumes(name, &svg, dom_expected);
    }
}
