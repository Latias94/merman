#![cfg(feature = "svg")]
#![recursion_limit = "256"]

use merman::svg::{
    CssOverridePolicy, CssOverridePostprocessor, ScopedCssPostprocessor, SvgPipeline,
    SvgRenderOptions,
};
use merman::{
    Engine, MermaidConfig, OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest,
};

fn zed_like_engine() -> Engine {
    let theme_config = MermaidConfig::from_value(serde_json::json!({
        "theme": "base",
        "darkMode": true,
        "fontFamily": "system-ui",
        "flowchart": {
            "padding": 16
        },
        "themeVariables": {
            "primaryColor": "#2f343e",
            "primaryTextColor": "#dce0e5",
            "primaryBorderColor": "#464b57",
            "lineColor": "#464b57",
            "secondaryColor": "#2e343e",
            "secondaryTextColor": "#dce0e5",
            "tertiaryColor": "#363c46",
            "tertiaryTextColor": "#dce0e5",
            "background": "#282c33",
            "mainBkg": "#2f343e",
            "nodeBorder": "#464b57",
            "nodeTextColor": "#dce0e5",
            "clusterBkg": "#2f343e",
            "clusterBorder": "#363c46",
            "titleColor": "#dce0e5",
            "edgeLabelBackground": "#282c33",
            "textColor": "#dce0e5",
            "fontFamily": "system-ui",
            "noteBkgColor": "#2f343e",
            "noteBorderColor": "#363c46",
            "noteTextColor": "#dce0e5",
            "actorBkg": "#2e343e",
            "actorBorder": "#464b57",
            "actorTextColor": "#dce0e5",
            "labelTextColor": "#dce0e5",
            "loopTextColor": "#dce0e5",
            "signalColor": "#dce0e5",
            "signalTextColor": "#dce0e5",
            "activationBkgColor": "#363c46",
            "activationBorderColor": "#464b57",
            "classText": "#dce0e5",
            "labelColor": "#dce0e5",
            "attributeBackgroundColorOdd": "#2f343e",
            "attributeBackgroundColorEven": "#2e343e",
            "pieTitleTextColor": "#dce0e5",
            "pieSectionTextColor": "#dce0e5",
            "pieLegendTextColor": "#dce0e5",
            "pieStrokeColor": "#464b57",
            "pieOuterStrokeColor": "#464b57",
            "quadrant1Fill": "#2f343e",
            "quadrant2Fill": "#2f343e",
            "quadrant3Fill": "#2f343e",
            "quadrant4Fill": "#2f343e",
            "quadrant1TextFill": "#dce0e5",
            "quadrant2TextFill": "#dce0e5",
            "quadrant3TextFill": "#dce0e5",
            "quadrant4TextFill": "#dce0e5",
            "quadrantPointFill": "#464b57",
            "quadrantPointTextFill": "#dce0e5",
            "quadrantTitleFill": "#dce0e5",
            "quadrantXAxisTextFill": "#dce0e5",
            "quadrantYAxisTextFill": "#dce0e5",
            "quadrantExternalBorderStrokeFill": "#464b57",
            "quadrantInternalBorderStrokeFill": "#464b57",
            "cScale0": "#74ade8",
            "cScaleLabel0": "#000000",
            "pie1": "#74ade8",
            "cScale1": "#be5046",
            "cScaleLabel1": "#ffffff",
            "pie2": "#be5046",
            "cScale2": "#bf956a",
            "cScaleLabel2": "#000000",
            "pie3": "#bf956a",
            "cScale3": "#b477cf",
            "cScaleLabel3": "#000000",
            "pie4": "#b477cf",
            "cScale4": "#6eb4bf",
            "cScaleLabel4": "#000000",
            "pie5": "#6eb4bf",
            "cScale5": "#d07277",
            "cScaleLabel5": "#000000",
            "pie6": "#d07277",
            "cScale6": "#dec184",
            "cScaleLabel6": "#000000",
            "pie7": "#dec184",
            "cScale7": "#a1c181",
            "cScaleLabel7": "#000000",
            "pie8": "#a1c181"
        }
    }));

    Engine::new().with_site_config(theme_config)
}

fn render_zed_safe(name: &str, source: &str) -> String {
    let pipeline = SvgPipeline::resvg_safe()
        .with_postprocessor(CssOverridePostprocessor::strip_existing_important());

    render_zed_with_pipeline(name, source, &pipeline)
}

fn render_zed_with_host_css(name: &str, source: &str, host_css: &str) -> String {
    let pipeline = SvgPipeline::resvg_safe()
        .with_postprocessor(CssOverridePostprocessor::strip_existing_important())
        .with_postprocessor(
            ScopedCssPostprocessor::new(host_css)
                .with_override_policy(CssOverridePolicy::StripExistingImportant),
        );

    render_zed_with_pipeline(name, source, &pipeline)
}

fn render_zed_with_pipeline(name: &str, source: &str, pipeline: &SvgPipeline) -> String {
    let output = Renderer::new()
        .with_engine(zed_like_engine())
        .render(RenderRequest::svg(
            source,
            OperationControl::new(),
            SvgRequest {
                options: SvgRenderOptions {
                    diagram_id: Some(name.to_string()),
                    ..Default::default()
                },
                pipeline: Some(pipeline.clone()),
                ..Default::default()
            },
        ))
        .unwrap_or_else(|err| panic!("{name}: render failed: {err}"));
    let RenderOutput::Svg(Some(svg)) = output else {
        panic!("{name}: no diagram detected");
    };
    svg.into_parts().0
}

fn assert_zed_safe_svg(name: &str, svg: &str) {
    assert!(svg.starts_with("<svg"), "{name}: expected SVG output");
    roxmltree::Document::parse(svg)
        .unwrap_or_else(|err| panic!("{name}: output should be XML-parseable: {err}"));
    assert!(
        svg.contains(&format!(r#"id="{name}""#)),
        "{name}: configured SVG id should reach the root SVG"
    );

    for bad in [
        "<foreignObject",
        "</foreignObject>",
        "@keyframes",
        "@-webkit-keyframes",
        ":root",
        "animation:",
        "animation-name:",
        "!important",
        "NaN",
        "Infinity",
        r#"fill="undefined""#,
        r#"stroke="undefined""#,
        r#"width="undefined""#,
        r#"height="undefined""#,
        r#"transform="undefined""#,
        r#"d="undefined""#,
        "fill:undefined",
        "stroke:undefined",
        "width:undefined",
        "height:undefined",
        "transform:undefined",
        r#"fill="""#,
        r#"stroke="""#,
        r#"width="""#,
        r#"height="""#,
        "fill: ;",
        "fill:;",
        "stroke: ;",
        "stroke:;",
    ] {
        assert!(
            !svg.contains(bad),
            "{name}: leaked unsafe SVG token {bad:?}"
        );
    }
}

#[test]
#[cfg(feature = "layout-cytoscape")]
fn zed_like_editor_pipeline_keeps_resvg_safe_themeable_svg_contract() {
    let cases = [
        (
            "zed-contract-flowchart",
            "flowchart TD\n    A[Hello] --> B[World]\n    B --> C{Decision}\n    C -->|Yes| D[OK]\n    C -->|No| E[Fail]",
            &["Hello", "World", "Decision", "Yes", "No"][..],
        ),
        (
            "zed-contract-class",
            "classDiagram\n    class Shelter {\n        -List~Animal~ animals\n        +adopt(Animal a) bool\n    }",
            &["Shelter", "List&lt;Animal", "adopt"][..],
        ),
        (
            "zed-contract-sequence",
            "sequenceDiagram\n    Alice->>Bob: Hello\n    Bob-->>Alice: Hi\n    Note over Alice,Bob: A note",
            &["Alice", "Bob", "Hello", "A note"][..],
        ),
        (
            "zed-contract-er",
            "erDiagram\n    A { int id PK }\n    B { int id PK }\n    A ||--o{ B : has",
            &["A", "B", "id", "has"][..],
        ),
        (
            "zed-contract-gantt",
            "gantt\n    title Test\n    dateFormat YYYY-MM-DD\n    section S\n        Task :a1, 2025-01-01, 7d",
            &["Test", "Task"][..],
        ),
        (
            "zed-contract-mindmap",
            "mindmap\n  root((Root))\n    Child1\n    Child2",
            &["Root", "Child1", "Child2"][..],
        ),
        (
            "zed-contract-journey",
            "journey\n    title Test\n    section S\n        Task: 5: Actor",
            &["Test", "Task", "Actor"][..],
        ),
        (
            "zed-contract-gitgraph",
            "gitGraph\n    commit id: \"init\"\n    branch dev\n    commit id: \"feat\"\n    checkout main\n    merge dev",
            &["main", "dev"][..],
        ),
        (
            "zed-contract-quadrant",
            "quadrantChart\n    title Test\n    x-axis Low --> High\n    y-axis Low --> High\n    A: [0.3, 0.8]\n    B: [0.7, 0.4]",
            &["Test", "Low", "High", "A", "B"][..],
        ),
        (
            "zed-contract-timeline",
            "timeline\n    title Test\n    section 2020s\n        2020 : Event A\n        2022 : Event B",
            &["Test", "2020s", "2020", "Event", "A", "B"][..],
        ),
        (
            "zed-contract-xychart",
            "xychart-beta\n    title Test\n    x-axis [\"A\", \"B\", \"C\"]\n    y-axis \"Val\" 0 --> 10\n    bar [3, 7, 5]",
            &["Test", "Val", "A"][..],
        ),
    ];

    for (name, source, labels) in cases {
        let svg = render_zed_safe(name, source);
        assert_zed_safe_svg(name, &svg);
        for label in labels {
            assert!(
                svg.contains(label),
                "{name}: expected visible label {label:?}"
            );
        }
    }
}

#[test]
fn zed_like_css_override_pipeline_leaves_host_css_in_control() {
    let svg = render_zed_with_host_css(
        "zed-contract-css",
        r##"%%{init: {"themeCSS": ".node rect { fill: #123456 !important; } @keyframes pulse { to { opacity: 1; } }"}}%%
flowchart TD
  A[Styled] --> B[Host]
"##,
        r#".node rect { stroke: #0ea5e9; }"#,
    );

    assert_zed_safe_svg("zed-contract-css", &svg);
    assert!(
        !svg.contains("fill: #123456"),
        "diagram-level themeCSS should stay filtered out: {svg}"
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Zed SVG XML");
    let host_css = document
        .descendants()
        .find(|node| {
            node.has_tag_name("style")
                && node.attribute("data-merman-postprocess") == Some("scoped-css")
        })
        .and_then(|node| node.text())
        .expect("scoped host CSS");
    assert!(
        host_css.contains("#zed-contract-css .node rect") && host_css.contains("stroke: #0ea5e9"),
        "expected scoped host CSS to remain in control: {svg}"
    );
    assert!(
        !svg.contains("@keyframes"),
        "resvg-safe output should remove animation rules: {svg}"
    );
}

#[test]
fn zed_like_host_css_is_sanitized_before_the_resvg_contract_is_sealed() {
    let svg = render_zed_with_host_css(
        "zed-contract-host-css-safety",
        "flowchart TD\n  A[Styled] --> B[Host]\n",
        r#"
@import url("https://example.invalid/theme.css");
.node rect {
  stroke: #0ea5e9;
  fill: url("javascript:alert(1)");
  animation: pulse 1s infinite;
}
@keyframes pulse { to { opacity: 0.5; } }
"#,
    );

    assert_zed_safe_svg("zed-contract-host-css-safety", &svg);
    let lower = svg.to_ascii_lowercase();
    assert!(!lower.contains("@import"), "{svg}");
    assert!(!lower.contains("javascript:"), "{svg}");
    assert!(!lower.contains("example.invalid"), "{svg}");
    assert!(
        svg.contains("stroke: #0ea5e9") || svg.contains("stroke:#0ea5e9"),
        "safe host declarations should survive terminal sanitization: {svg}"
    );
}

#[test]
fn zed_like_pipeline_keeps_class_generics_readable_without_double_escaping() {
    let svg = render_zed_safe(
        "zed-contract-generics",
        "classDiagram\n    class Shelter {\n        -List~Animal~ animals\n    }",
    );

    assert_zed_safe_svg("zed-contract-generics", &svg);
    assert!(
        svg.contains("List&lt;Animal"),
        "class generic fallback text should stay readable: {svg}"
    );
    assert!(
        !svg.contains("&amp;lt;") && !svg.contains("&amp;gt;"),
        "class generic fallback text should not be double-escaped: {svg}"
    );
}

#[test]
fn zed_like_pipeline_decodes_double_escaped_entities_in_fallback_text() {
    let svg = render_zed_safe(
        "zed-contract-entities",
        r#"flowchart TD
    A["Tom &amp; Jerry"]
    B["List&amp;lt;Animal&amp;gt; &amp;amp; friends"]
    A --> B"#,
    );

    assert_zed_safe_svg("zed-contract-entities", &svg);
    assert!(
        !svg.contains("&amp;amp;") && !svg.contains("&amp;lt;") && !svg.contains("&amp;gt;"),
        "fallback text should not double-escape XML entities: {svg}"
    );

    let doc = roxmltree::Document::parse(&svg).expect("valid SVG");
    let fallback_text = doc
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .filter(|node| {
            node.ancestors()
                .any(|ancestor| ancestor.attribute("data-merman-foreignobject") == Some("fallback"))
        })
        .filter_map(|node| node.text())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        fallback_text.contains("Tom & Jerry") && fallback_text.contains("List<Animal> & friends"),
        "expected decoded fallback text for XML predefined entities: {fallback_text:?}"
    );
}
