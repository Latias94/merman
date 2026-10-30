#![cfg(feature = "svg")]
#![recursion_limit = "256"]

use merman::svg::{
    CssOverridePolicy, CssOverridePostprocessor, ScopedCssPostprocessor, SvgPipeline,
    SvgRenderOptions,
};
use merman::{
    MermaidEngine, OperationControl, RenderOutput, RenderRequest, Renderer,
    SourcePresentationConfig, SvgRequest,
};

fn zed_like_engine() -> MermaidEngine {
    let theme_config = SourcePresentationConfig::from_value(serde_json::json!({
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

    MermaidEngine::new().with_site_config(theme_config)
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
        .with_presentation_engine(zed_like_engine())
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

fn numeric_value_is_finite(value: &str) -> bool {
    value
        .split(|character: char| {
            !character.is_ascii_alphanumeric() && !matches!(character, '.' | '+' | '-')
        })
        .filter(|token| !token.is_empty())
        .all(|token| {
            let number = [
                "px", "rem", "em", "pt", "pc", "cm", "mm", "in", "ex", "deg", "rad",
            ]
            .into_iter()
            .find_map(|unit| token.strip_suffix(unit))
            .unwrap_or(token);
            number.parse::<f64>().map_or(true, f64::is_finite)
        })
}

fn visual_attribute_is_finite(name: &str, value: &str) -> bool {
    match name {
        "d" | "transform" => {
            let lower = value.to_ascii_lowercase();
            !lower.contains("nan") && !lower.contains("inf")
        }
        "fill" | "stroke" => {
            !value.contains('(')
                || value.trim_start().starts_with("url(")
                || numeric_value_is_finite(value)
        }
        "style" => value
            .split(';')
            .filter_map(|declaration| declaration.split_once(':'))
            .all(|(property, value)| match property.trim() {
                "transform" => visual_attribute_is_finite("transform", value),
                "fill" | "stroke" => visual_attribute_is_finite(property.trim(), value),
                "font-size" | "line-height" | "font-weight" | "letter-spacing" | "word-spacing"
                | "opacity" | "fill-opacity" | "stroke-opacity" | "stroke-width"
                | "stroke-dasharray" | "stroke-dashoffset" | "width" | "height" | "x" | "y" => {
                    numeric_value_is_finite(value)
                }
                _ => true,
            }),
        _ => numeric_value_is_finite(value),
    }
}

#[test]
fn zed_numeric_guard_checks_values_without_treating_identifiers_as_numbers() {
    assert!(visual_attribute_is_finite(
        "style",
        "dominant-baseline:central;alignment-baseline:central;font-family:Banana,Infinity;fill:url(#infinity);stroke:banana;font-size:16px"
    ));
    assert!(visual_attribute_is_finite("fill", "url(#nan)"));
    assert!(visual_attribute_is_finite("stroke", "banana"));
    for (attribute, value) in [
        ("d", "MNaN 0L1 2"),
        ("d", "M0 0LInf 2"),
        ("transform", "matrix(1 0 0 1 Infinity 0)"),
        ("style", "font-size:NaNpx"),
        ("style", "line-height:NaNrem"),
        ("style", "opacity:Infinity"),
        ("style", "stroke-width:Inf"),
        ("style", "fill:rgb(0,NaN,0)"),
        ("width", "Inf"),
        ("height", "1e999"),
        ("height", "1e+999"),
        ("x", "NaN"),
    ] {
        assert!(
            !visual_attribute_is_finite(attribute, value),
            "{attribute}={value}"
        );
    }
}

fn assert_zed_safe_svg(name: &str, svg: &str) {
    assert!(svg.starts_with("<svg"), "{name}: expected SVG output");
    let document = roxmltree::Document::parse(svg)
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

    for node in document.descendants() {
        for attribute in node.attributes() {
            let is_visual_attribute = matches!(
                attribute.name(),
                "d" | "fill"
                    | "height"
                    | "opacity"
                    | "stroke"
                    | "stroke-width"
                    | "transform"
                    | "width"
                    | "x"
                    | "x1"
                    | "x2"
                    | "y"
                    | "y1"
                    | "y2"
                    | "style"
            );
            if !is_visual_attribute {
                continue;
            }
            assert!(
                visual_attribute_is_finite(attribute.name(), attribute.value()),
                "{name}: non-finite visual attribute {}={:?}",
                attribute.name(),
                attribute.value()
            );
        }
    }
}

fn inject_css_after_merman(svg: &str, css: &str) -> String {
    let closing_tag = "</svg>";
    let insertion = svg
        .rfind(closing_tag)
        .unwrap_or_else(|| panic!("expected an SVG closing tag: {svg}"));
    let mut composed = String::with_capacity(svg.len() + css.len() + 64);
    composed.push_str(&svg[..insertion]);
    composed.push_str(r#"<style data-zed-postprocess="inject-css">"#);
    composed.push_str(css);
    composed.push_str("</style>");
    composed.push_str(&svg[insertion..]);
    composed
}

fn fallback_content<'a, 'input>(group: roxmltree::Node<'a, 'input>) -> roxmltree::Node<'a, 'input> {
    let instance = group
        .children()
        .find(|node| node.has_tag_name("use"))
        .expect("fallback instance must reference its isolated content");
    let id = instance
        .attribute("href")
        .and_then(|href| href.strip_prefix('#'))
        .expect("fallback use must have a local href");
    let mut definitions = group
        .document()
        .descendants()
        .filter(|node| node.attribute("id") == Some(id));
    let content = definitions.next().expect("fallback href must resolve");
    assert!(
        definitions.next().is_none(),
        "fallback content id must be unique"
    );
    assert!(
        content.ancestors().any(|node| node.has_tag_name("defs")
            && node.parent() == Some(group.document().root_element())),
        "fallback content must live in root definitions"
    );
    content
}

fn fallback_text_style(svg: &str, label: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid Zed SVG XML");
    document
        .descendants()
        .filter(|group| group.attribute("data-merman-foreignobject") == Some("fallback"))
        .flat_map(|group| fallback_content(group).descendants())
        .find(|node| {
            node.has_tag_name("text") && node.text().is_some_and(|text| text.trim() == label)
        })
        .and_then(|node| node.attribute("style"))
        .map(str::to_owned)
        .unwrap_or_else(|| panic!("expected fallback text {label:?}: {svg}"))
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
fn zed_like_pipeline_separates_pre_injection_metrics_from_host_hooks() {
    let source = r#"classDiagram
    class User {
        +String name
    }"#;
    let merman_svg = render_zed_safe("zed-issue-89-pre-injection", source);
    let merman_style = fallback_text_style(&merman_svg, "User");
    assert!(
        merman_style.contains("font-size:16px") || merman_style.contains("font-size: 16px"),
        "Merman must resolve issue #89 before Zed injects product CSS: {merman_style}"
    );
    assert!(
        !merman_style.contains("font-size:10px") && !merman_style.contains("font-size: 10px"),
        "the SVG-only class text selector must not leak into the source XHTML label: {merman_style}"
    );

    let composed = render_zed_with_host_css(
        "zed-issue-89-host-hooks",
        source,
        ".merman-foreignobject-fallback-text { font-size: 18px; }",
    );
    assert_zed_safe_svg("zed-issue-89-host-hooks", &composed);
    assert!(
        composed.contains("data-merman-foreignobject=\"fallback\""),
        "stable fallback marker must remain available to Zed postprocessing: {composed}"
    );
    assert!(
        composed.contains("font-size: 18px"),
        "the scoped host stylesheet should remain discoverable after Merman's terminal pass: {composed}"
    );
    let composed_style = fallback_text_style(&composed, "User");
    assert!(
        composed_style.contains("font-size:16px") || composed_style.contains("font-size: 16px"),
        "Merman's inline metric remains the measurement/emission contract; post-fallback host metric changes are a documented limitation: {composed_style}"
    );
}

#[test]
fn zed_like_edge_label_fallback_uses_theme_background() {
    let svg = render_zed_safe(
        "zed-edge-label-background",
        "flowchart TD\n  A[Start] -->|Yes| B[Done]",
    );
    assert_zed_safe_svg("zed-edge-label-background", &svg);

    let document = roxmltree::Document::parse(&svg).expect("valid Zed SVG XML");
    let fallback_group = document.descendants().find(|node| {
        node.has_tag_name("g")
            && node.attribute("data-merman-foreignobject") == Some("fallback")
            && fallback_content(*node).descendants().any(|descendant| {
                descendant.has_tag_name("text")
                    && descendant.text().is_some_and(|text| text.trim() == "Yes")
            })
    });
    let rect_fill = fallback_group
        .and_then(|group| {
            fallback_content(group)
                .descendants()
                .find(|child| child.has_tag_name("rect") && child.attribute("fill") != Some("none"))
        })
        .and_then(|rect| rect.attribute("fill"));
    assert_eq!(
        rect_fill,
        Some("#282c33"),
        "edge-label fallback should use the configured Zed theme background: {svg}"
    );
}

#[test]
fn zed_like_after_render_css_is_a_paint_only_host_composition() {
    let source = r#"classDiagram
    class User {
        +String name
    }"#;
    let sealed = render_zed_safe("zed-issue-89-after-render", source);
    let composed = inject_css_after_merman(
        &sealed,
        ".merman-foreignobject-fallback-text { font-size: 18px !important; }",
    );

    assert_zed_safe_svg("zed-issue-89-after-render", &sealed);
    let document = roxmltree::Document::parse(&composed).expect("postprocessed SVG should be XML");
    let injected_css = document
        .descendants()
        .find(|node| node.attribute("data-zed-postprocess") == Some("inject-css"))
        .and_then(|node| node.text())
        .expect("Zed-style CSS should be injected after Merman returns");
    assert!(injected_css.contains("font-size: 18px !important"));
    assert!(
        composed.contains("data-merman-foreignobject=\"fallback\""),
        "the generated fallback marker must survive host composition: {composed}"
    );

    let inline_style = fallback_text_style(&composed, "User");
    assert!(
        inline_style.contains("font-size:16px") || inline_style.contains("font-size: 16px"),
        "post-return CSS must not rewrite Merman's sealed inline metric or imply remeasurement: {inline_style}"
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
    let fallback_labels: Vec<_> = doc
        .descendants()
        .filter(|node| node.attribute("data-merman-foreignobject") == Some("fallback"))
        .map(|group| {
            let rows: Vec<_> = fallback_content(group)
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .map(|node| {
                    node.descendants()
                        .filter(|node| node.is_text())
                        .filter_map(|node| node.text())
                        .collect::<String>()
                })
                .collect();
            let label = rows
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            (label, rows)
        })
        .collect();
    assert_eq!(fallback_labels.len(), 2, "{fallback_labels:?}");
    let expected_labels: &[(&str, &[&str])] = &[
        ("Tom & Jerry", &["Tom & Jerry"]),
        ("List<Animal> & friends", &["List<Animal> & ", "friends"]),
    ];
    for (expected, expected_rows) in expected_labels {
        let rows = fallback_labels
            .iter()
            .find_map(|(label, rows)| (label == expected).then_some(rows))
            .unwrap_or_else(|| {
                panic!("expected complete decoded fallback label {expected:?}: {fallback_labels:?}")
            });
        assert_eq!(
            rows.iter().map(String::as_str).collect::<Vec<_>>(),
            *expected_rows,
            "{expected:?}"
        );
    }
}
