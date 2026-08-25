#![cfg(feature = "svg")]

use merman::svg::{
    CssOverridePolicy, DiagramTheme, DiagramThemeCompiler, SvgOutputPolicy, SvgPipeline,
    SvgPipelinePreset, TextMeasurementPolicy, ThemePreset,
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
    theme: Option<DiagramTheme>,
}

impl TypedSvgRenderer {
    fn new() -> Self {
        Self {
            renderer: Renderer::new(),
            request: SvgRequest::default(),
            theme: None,
        }
    }

    fn with_theme(mut self, theme: DiagramTheme) -> Self {
        self.theme = Some(theme);
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
        let request = RenderRequest::svg(source, OperationControl::new(), self.request.clone());
        let request = match self.theme.as_ref() {
            Some(theme) => request.with_theme(theme.clone()),
            None => request,
        };
        let output = self.renderer.render(request)?;
        let RenderOutput::Svg(svg) = output else {
            unreachable!("SVG request must return SVG output")
        };
        Ok(svg.map(|svg| svg.into_parts().0))
    }
}

fn themed_renderer(preset: ThemePreset, name: &str) -> TypedSvgRenderer {
    let theme = DiagramThemeCompiler::new()
        .compile_preset(preset)
        .expect("built-in theme preset should compile");
    let pipeline = SvgOutputPolicy {
        preset: SvgPipelinePreset::ResvgSafe,
        css_override_policy: CssOverridePolicy::StripExistingImportant,
        root_background_color: None,
        drop_native_duplicate_fallbacks: false,
        scoped_css: None,
    }
    .pipeline();

    TypedSvgRenderer::new()
        .with_theme(theme)
        .with_svg_pipeline(pipeline)
        .with_vendored_text_measurer()
        .with_diagram_id(name)
}

fn render_with_editor_dark_theme(name: &str, source: &str) -> String {
    themed_renderer(ThemePreset::EditorDark, name)
        .render_svg(source)
        .unwrap_or_else(|err| panic!("{name}: render failed: {err}"))
        .unwrap_or_else(|| panic!("{name}: no diagram detected"))
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
    fn usvg_options_with_system_sans_serif() -> usvg::Options<'static> {
        const PREFERRED_FAMILIES: &[&str] = &[
            "DejaVu Sans",
            "Liberation Sans",
            "Noto Sans",
            "Arial",
            "Helvetica",
        ];

        let mut options = usvg::Options::default();
        let fallback_family = {
            let fontdb = options.fontdb_mut();
            fontdb.load_system_fonts();
            PREFERRED_FAMILIES
                .iter()
                .find_map(|preferred| {
                    fontdb
                        .faces()
                        .flat_map(|face| face.families.iter())
                        .find(|(family, _)| family.eq_ignore_ascii_case(preferred))
                        .map(|(family, _)| family.clone())
                })
                .or_else(|| {
                    fontdb
                        .faces()
                        .find_map(|face| face.families.first().map(|(family, _)| family.clone()))
                })
        }
        .expect("usvg color assertions require at least one system font");
        options.font_family = fallback_family.clone();
        options.fontdb_mut().set_sans_serif_family(fallback_family);
        options
    }

    fn find_flattened_fill(group: &usvg::Group) -> Option<(u8, u8, u8)> {
        for node in group.children() {
            match node {
                usvg::Node::Group(group) => {
                    if let Some(fill) = find_flattened_fill(group) {
                        return Some(fill);
                    }
                }
                usvg::Node::Path(path) => {
                    let Some(fill) = path.fill() else {
                        continue;
                    };
                    if let usvg::Paint::Color(color) = fill.paint() {
                        return Some((color.red, color.green, color.blue));
                    }
                }
                usvg::Node::Text(text) => {
                    if let Some(fill) = find_flattened_fill(text.flattened()) {
                        return Some(fill);
                    }
                }
                usvg::Node::Image(_) => {}
            }
        }
        None
    }

    fn find_usvg_text_fill(group: &usvg::Group, label: &str) -> Option<(u8, u8, u8)> {
        for node in group.children() {
            match node {
                usvg::Node::Group(group) => {
                    if let Some(fill) = find_usvg_text_fill(group, label) {
                        return Some(fill);
                    }
                }
                usvg::Node::Text(text) => {
                    let text_content = text
                        .chunks()
                        .iter()
                        .map(|chunk| chunk.text())
                        .collect::<String>();
                    if text_content.trim() == label {
                        return find_flattened_fill(text.flattened());
                    }
                }
                usvg::Node::Path(_) | usvg::Node::Image(_) => {}
            }
        }
        None
    }

    let document =
        roxmltree::Document::parse(svg).unwrap_or_else(|err| panic!("{name}: invalid SVG: {err}"));
    let options = usvg_options_with_system_sans_serif();
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
            "{name}: ER fallback label should inherit diagram theme font size: {svg}"
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
fn diagram_theme_covers_core_diagram_roles() {
    let cases: &[(&str, &str, &[&str])] = &[
        (
            "diagram-theme-flowchart",
            "flowchart TD\n  A[Host] -->|Edge| B[Theme]",
            &["#111827", "#e5e7eb", "#475569", "#94a3b8"],
        ),
        (
            "diagram-theme-sequence",
            "sequenceDiagram\n  participant A as Alpha\n  participant B as Beta\n  A->>B: Hello\n  Note over A,B: Profile note",
            &["#1f2937", "#e5e7eb", "#94a3b8", "#422006", "#f59e0b"],
        ),
        (
            "diagram-theme-class",
            "classDiagram\n  Animal <|-- Dog\n  class Animal {\n    +bark()\n  }",
            &["#111827", "#e5e7eb", "#475569"],
        ),
        (
            "diagram-theme-state",
            "stateDiagram-v2\n  [*] --> Idle: start\n  Idle --> Done: finish",
            &["#111827", "#e5e7eb", "#94a3b8"],
        ),
        (
            "diagram-theme-xychart",
            "xychart-beta\n  title Profile\n  x-axis [\"A\", \"B\"]\n  y-axis \"Value\" 0 --> 10\n  bar [4, 7]",
            &["#60a5fa", "#e5e7eb"],
        ),
        (
            "diagram-theme-pie",
            "pie title Profile Pie\n  \"A\" : 4\n  \"B\" : 7",
            &["#60a5fa", "#34d399", "#e5e7eb"],
        ),
        (
            "diagram-theme-quadrant",
            "quadrantChart\n  title Profile Matrix\n  x-axis Low --> High\n  y-axis Low --> High\n  quadrant-1 Invest\n  A: [0.7, 0.8]",
            &["#0f172a", "#e5e7eb", "#94a3b8"],
        ),
    ];

    for (name, source, expected) in cases {
        let svg = render_with_editor_dark_theme(name, source);
        assert_contains_all(name, &svg, expected);
    }
}

#[test]
#[cfg(feature = "layout-cytoscape")]
fn diagram_theme_series_palette_reaches_supported_ordinal_diagrams() {
    let cases: &[(&str, &str, &[&str])] = &[
        (
            "diagram-theme-mindmap",
            "mindmap\n  Root\n    First\n    Second",
            &["#60a5fa", "#34d399"],
        ),
        (
            "diagram-theme-gitgraph",
            "gitGraph\n  commit id: \"A\"\n  branch dev\n  checkout dev\n  commit id: \"B\"",
            &["#60a5fa", "#34d399"],
        ),
        (
            "diagram-theme-journey",
            "journey\n  title Profile Journey\n  section Checkout\n    Sign Up: 5: Alice\n    Pay: 3: Bob",
            &["#60a5fa", "#34d399"],
        ),
        (
            "diagram-theme-timeline",
            "timeline\n  title Profile Timeline\n  section 2026\n    Alpha : Start\n    Beta : Ship",
            &["#60a5fa", "#34d399"],
        ),
    ];

    for (name, source, expected) in cases {
        let svg = render_with_editor_dark_theme(name, source);
        assert_contains_all(name, &svg, expected);
    }
}

#[test]
fn gruvbox_diagram_theme_keeps_er_relationship_label_fallbacks_readable() {
    let svg = themed_renderer(ThemePreset::GruvboxDark, "gruvbox-er-labels")
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
fn diagram_theme_centers_gitgraph_branch_labels_with_editor_fonts() {
    let plain = TypedSvgRenderer::new()
        .with_vendored_text_measurer()
        .with_diagram_id("gitgraph-plain-baseline")
        .render_svg(USER_GITGRAPH_THEME_REGRESSION)
        .unwrap_or_else(|err| panic!("plain gitGraph render failed: {err}"))
        .unwrap_or_else(|| panic!("plain gitGraph render produced no diagram"));
    assert_gitgraph_branch_labels_keep_mermaid_parity_baseline("plain-gitgraph", &plain);

    let themed = themed_renderer(ThemePreset::OneDark, "gitgraph-one-dark-baseline")
        .render_svg(USER_GITGRAPH_THEME_REGRESSION)
        .unwrap_or_else(|err| panic!("one-dark gitGraph render failed: {err}"))
        .unwrap_or_else(|| panic!("one-dark gitGraph render produced no diagram"));
    assert_gitgraph_branch_label_baselines_centered(
        "one-dark-gitgraph",
        &themed,
        &["main", "develop", "feature"],
    );

    let cherry_pick = themed_renderer(ThemePreset::OneDark, "gitgraph-one-dark-cherry-pick")
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
fn diagram_theme_covers_additional_current_diagram_surfaces() {
    let cases: &[(&str, &str, &[&str], &[&str])] = &[
        (
            "diagram-theme-er",
            "erDiagram\n  CUSTOMER ||--o{ ORDER : places\n  CUSTOMER {\n    string name\n  }",
            &["#111827", "#e5e7eb", "#94a3b8", "#475569"],
            &[
                "class=\"basic label-container\" style=\"fill:#111827;stroke:#475569\"",
                "class=\"edge-thickness-normal edge-pattern-solid relationshipLine\" style=\"stroke:#94a3b8\"",
            ],
        ),
        (
            "diagram-theme-requirement",
            "requirementDiagram\n  requirement req1 {\n    id: 1\n    text: Host requirement\n    risk: high\n    verifymethod: analysis\n  }\n  element sys {\n    type: system\n  }\n  sys - satisfies -> req1",
            &["#111827", "#e5e7eb"],
            &["fill=\"#111827\""],
        ),
        (
            "diagram-theme-gantt",
            "gantt\n  title Profile Plan\n  dateFormat YYYY-MM-DD\n  section Core\n  Build : 2026-01-01, 15d\n  Critical :crit, 2026-01-16, 2d\n  Ship :done, 2026-01-18, 3d",
            &["#e5e7eb", "#111827"],
            &[
                "id=\"diagram-theme-gantt-task1\" style=\"fill:#111827\"",
                "id=\"diagram-theme-gantt-task2\" style=\"fill:#111827\"",
                "id=\"diagram-theme-gantt-task3\" style=\"fill:#111827\"",
            ],
        ),
        (
            "diagram-theme-block",
            "block\n  block:Core\n    A[\"Alpha\"]\n    B[\"Beta\"]\n  end\n  A --> B",
            &["#e5e7eb", "rgba(30, 41, 59, 0.5)", "#475569", "#94a3b8"],
            &[
                "class=\"edge-thickness-normal edge-pattern-solid",
                ".node .cluster{fill:rgba(30, 41, 59, 0.5);stroke:rgba(71, 85, 105, 0.2);",
            ],
        ),
        (
            "diagram-theme-kanban",
            "kanban\n  todo[Todo]\n    card[Dark Card]@{ assigned: \"Core\", priority: \"High\" }",
            &[
                "#e5e7eb",
                "hsl(213.1168831169, 93.9024390244%, 57.8431372549%)",
            ],
            &[
                "class=\"basic label-container __APA__\" style=\"fill:hsl(213.1168831169, 93.9024390244%, 57.8431372549%)\"",
                "class=\"label\" style=\"color:#000000;fill:#000000;text-align:left\"",
            ],
        ),
        (
            "diagram-theme-radar",
            "radar-beta\n  title Profile Radar\n  axis Speed, Quality, Cost\n  curve Team{8, 7, 4}",
            &["#e5e7eb", "#60a5fa", "#94a3b8"],
            &[
                ".radarAxisLine{stroke:#94a3b8;stroke-width:2;}",
                ".radarCurve-0{color:#60a5fa;fill:#60a5fa;",
            ],
        ),
        (
            "diagram-theme-tree-view",
            include_str!("../../../fixtures/treeView/upstream_docs_treeview_basic.mmd"),
            &["#e5e7eb", "#94a3b8"],
            &[
                ".treeView-node-label { font-size: 16px; fill: #e5e7eb; white-space: pre; }",
                ".treeView-node-line { stroke: #94a3b8; }",
            ],
        ),
    ];

    for (name, source, expected, dom_expected) in cases {
        let svg = render_with_editor_dark_theme(name, source);
        assert_contains_all(name, &svg, expected);
        assert_current_dom_consumes(name, &svg, dom_expected);
    }
}

#[test]
#[cfg(feature = "layout-cytoscape")]
fn diagram_theme_keeps_shared_canvas_and_typography_on_unadapted_family_roles() {
    let cases: &[(&str, &str)] = &[
        (
            "diagram-theme-venn",
            "venn-beta\n  set A[\"Core\"]:10\n  set B[\"Editor\"]:8\n  union A,B[\"Shared\"]:3",
        ),
        (
            "diagram-theme-architecture",
            "architecture-beta\n  group core(cloud)[Core]\n  service api(server)[API] in core\n  service db(database)[DB] in core\n  api:R --> L:db",
        ),
        (
            "diagram-theme-sankey",
            "sankey\nSource,Target,10\nTarget,Done,2",
        ),
        (
            "diagram-theme-treemap",
            "treemap-beta\n  \"Profile Section\"\n    \"Profile Leaf\": 42",
        ),
        (
            "diagram-theme-c4",
            "C4Component\nComponentDb(db, \"Database\", \"Postgres\", \"Stores data\")\nComponentQueue(queue, \"Queue\", \"NATS\", \"Events\")",
        ),
        (
            "diagram-theme-ishikawa",
            include_str!(
                "../../../fixtures/ishikawa/upstream_cypress_ishikawa_spec_1_should_render_a_simple_ishikawa_diagram_001.mmd"
            ),
        ),
        (
            "diagram-theme-eventmodeling",
            include_str!("../../../fixtures/eventmodeling/upstream_docs_eventmodeling_minimum.mmd"),
        ),
    ];

    for (name, source) in cases {
        let svg = render_with_editor_dark_theme(name, source);
        assert_contains_all(
            name,
            &svg,
            &["data-merman-theme-canvas=\"base\"", "fill=\"#0f172a\""],
        );
        let compact_svg = svg
            .chars()
            .filter(|ch| !ch.is_ascii_whitespace())
            .collect::<String>();
        assert!(
            compact_svg.contains("font-family:Inter,ui-sans-serif,system-ui,sans-serif"),
            "{name}: expected shared preset typography in SVG: {svg}"
        );
    }

    let packet = render_with_editor_dark_theme(
        "diagram-theme-packet",
        "packet\ntitle Profile Packet\n+8: \"Byte\"\n+16: \"Word\"",
    );
    assert_contains_all(
        "diagram-theme-packet",
        &packet,
        &[
            "data-merman-theme-canvas=\"base\"",
            "fill=\"#0f172a\"",
            "Profile Packet",
            "Byte",
        ],
    );
}
