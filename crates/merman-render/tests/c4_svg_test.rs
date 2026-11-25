use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::C4DiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMetrics, TextStyle};
use merman_render::theme::{NativeTheme, ThemeCompiler};
use std::sync::{Arc, Mutex};

#[test]
fn native_theme_v2_c4_uses_categorical_fills_and_preserves_authored_paint() {
    let source = r##"C4Component
Person(user, "User")
System(system, "System")
System_Ext(external, "External")
Container_Boundary(boundary, "Group") {
    Container(container, "Container", "Rust")
    Component(component, "Component", "Rust")
}
UpdateElementStyle(system, $bgColor="#AABBCC")
Rel(user, system, "Uses")
"##;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version":2,"extends":"paper",
                "roles":{"surface.node":"#DDEEFF","surface.container":"#E0E8F0",
                         "stroke.connector":"#405060"},
                "typography":{"sans":["C4 Body","sans-serif"],"base_size_px":19},
                "form":{"radius":7,"container_radius":11,"connector_width":3,
                        "edge_style":"orthogonal","marker_style":"open"}
            }"##,
            None,
        )
        .unwrap();
    let data = &theme
        .family(merman_render::DiagramFamilyId::C4.as_str())
        .data;
    let fills = [
        data.fill(0).to_string(),
        data.fill(2).to_string(),
        data.fill(3).to_string(),
    ];
    let person_ink = data.fill_ink(0).to_string();
    let prepared = family::prepare(
        parsed.presentation(),
        &LayoutOptions::headless_svg_defaults(),
        RenderEnvironment::deterministic()
            .begin_session_with_native_theme(NativeTheme::new(theme))
            .unwrap(),
    )
    .unwrap();
    let rendered = prepared
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let shape = |kind| {
        document
            .descendants()
            .find(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_ascii_whitespace().any(|class| class == kind))
            })
            .unwrap()
    };
    for (kind, color) in [
        ("c4-person", fills[0].as_str()),
        ("c4-system", "#AABBCC"),
        ("c4-external_system", "#DDEEFF"),
        ("c4-container", fills[1].as_str()),
        ("c4-component", fills[2].as_str()),
    ] {
        assert!(
            shape(kind).descendants().any(|node| node
                .attribute("style")
                .is_some_and(|style| style.contains(&format!("fill:{color} !important")))),
            "{kind}"
        );
    }
    assert!(shape("c4-person").descendants().any(|node| {
        node.has_tag_name("text")
            && node
                .attribute("style")
                .is_some_and(|style| style.contains(&person_ink))
    }));
    let boundary = shape("c4-boundary-background");
    assert_eq!(boundary.attribute("fill"), Some("#E0E8F0"));
    assert_eq!(boundary.attribute("rx"), Some("11"));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("path")
            && node.attribute("stroke-width") == Some("3")
            && node.attribute("stroke") == Some("#405060")
            && node
                .attribute("d")
                .is_some_and(|path| path.contains('H') && path.contains('V'))
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("path")
            && node.attribute("fill") == Some("none")
            && node.attribute("stroke") == Some("#405060")
            && node
                .parent_element()
                .is_some_and(|parent| parent.has_tag_name("marker"))
    }));
}

fn render_c4_svg_with_environment(source: &str, environment: &RenderEnvironment) -> String {
    try_render_c4_svg_with_environment(source, environment).expect("render C4 SVG")
}

fn try_render_c4_svg_with_environment(
    source: &str,
    environment: &RenderEnvironment,
) -> merman_render::Result<String> {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = environment.begin_session().expect("begin render session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?;

    Ok(artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?
        .svg()
        .to_owned())
}

#[test]
fn c4_filled_nested_boundaries_paint_backgrounds_before_their_children() {
    let source = r##"C4Context
Boundary(outer, "Outer") {
  System(sibling, "Sibling")
  Boundary(inner, "Inner") {
    System(leaf, "Leaf")
  }
}
UpdateElementStyle(outer, $bgColor="#112233")
UpdateElementStyle(inner, $bgColor="#445566")
"##;
    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).unwrap();
    let filled_boundary = |fill| {
        document
            .descendants()
            .find(|node| node.has_tag_name("rect") && node.attribute("fill") == Some(fill))
            .expect("filled boundary background")
    };
    let outer = filled_boundary("#112233");
    let inner = filled_boundary("#445566");
    let sibling = c4_rect_labeled(&document, "Sibling");
    let leaf = c4_rect_labeled(&document, "Leaf");
    assert!(
        outer.range().start < sibling.range().start,
        "outer fill must not cover its child"
    );
    assert!(outer.range().start < inner.range().start);
    assert!(
        inner.range().start < leaf.range().start,
        "inner fill must not cover its child"
    );
    assert!(
        leaf.range().start < c4_rect_labeled(&document, "Inner").range().start,
        "the inner outline and label remain above the child"
    );
}

#[test]
fn c4_nodes_render_each_layout_occurrence_after_alias_updates() {
    let source =
        "C4Context\nSystem(a, \"A\")\nSystem(b, \"B\")\nUpdateElementStyle(a, $alias=\"b\")\n";
    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| { node.has_tag_name("text") && svg_text_content(*node) == "B" })
            .count(),
        2
    );
}

#[test]
fn c4_boundary_labels_render_each_layout_occurrence_after_alias_updates() {
    let source = "C4Context\nBoundary(a, \"A\") {\nUpdateLayoutConfig(4)\n}\nBoundary(b, \"B\") {\nUpdateLayoutConfig(4)\n}\nSystem(outside, \"Outside\")\nUpdateElementStyle(a, $alias=\"b\")\n";
    let baseline = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&baseline).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| { node.has_tag_name("text") && svg_text_content(*node) == "B" })
            .count(),
        2
    );
}

#[test]
fn c4_source_relationship_paints_keep_shared_markers_without_host_paint() {
    let body = "C4Context\nSystem(a, \"A\")\nSystem(b, \"B\")\nBiRel(a, b, \"Calls\", \"HTTPS\")\n";
    for source_owned in [false, true] {
        let source = if source_owned {
            format!("{body}UpdateRelStyle(a, b, $textColor=\"red\", $lineColor=\"blue\")\n")
        } else {
            body.to_owned()
        };
        let config = if source_owned {
            serde_json::json!({})
        } else {
            serde_json::json!({"c4": {
                "relationship_text_color": "red", "relationship_line_color": "blue"
            }})
        };
        let parsed = MermaidEngine::new()
            .with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config),
            )
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_c4_shared_relationship_markers(&document);
        let line = document
            .descendants()
            .find(|node| node.has_tag_name("line"))
            .unwrap();
        assert_eq!(line.attribute("stroke"), Some("blue"));
        for label in ["Calls", "[HTTPS]"] {
            let text = document
                .descendants()
                .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
                .unwrap();
            assert_eq!(text.attribute("fill"), Some("red"));
            if label == "[HTTPS]" {
                assert_eq!(text.attribute("font-style"), Some("italic"));
            }
        }
    }
}

fn assert_c4_shared_relationship_markers(document: &roxmltree::Document<'_>) {
    let line = document
        .descendants()
        .find(|node| node.has_tag_name("line") && node.attribute("marker-end").is_some())
        .unwrap();
    assert!(
        line.parent()
            .unwrap()
            .children()
            .all(|node| !node.has_tag_name("defs")),
        "source relationship colors must preserve Mermaid's relationship group"
    );
    for (attribute, suffix) in [
        ("marker-start", "-arrowend)"),
        ("marker-end", "-arrowhead)"),
    ] {
        assert!(line.attribute(attribute).unwrap().ends_with(suffix));
    }
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.has_tag_name("marker"))
            .count(),
        4
    );
}

fn try_render_c4_svg_with_resource_policy(
    source: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse C4 resource-bound fixture")
        .expect("detect C4 resource-bound fixture")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin C4 resource-bound session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

fn layout_c4_with_options(source: &str, options: &LayoutOptions) -> C4DiagramLayout {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, options, session).expect("layout ok");
    let projection = artifact.layout_json().expect("serialize C4 layout");
    serde_json::from_value(projection["layout"]["C4Diagram"].clone()).expect("C4 layout projection")
}

fn svg_text_content(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|descendant| descendant.is_text())
        .filter_map(|descendant| descendant.text())
        .collect()
}

fn c4_rect_labeled<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    label: &str,
) -> roxmltree::Node<'a, 'input> {
    let label_node = document
        .descendants()
        .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
        .unwrap_or_else(|| panic!("missing C4 label {label}"));

    let shape_group = label_node.ancestors().find(|node| {
        node.has_tag_name("g")
            && node
                .attribute("class")
                .is_some_and(|classes| classes.split_whitespace().any(|class| class == "c4-shape"))
    });

    if let Some(group) = shape_group
        && let Some(shape) = group.descendants().find(|node| {
            ["rect", "path", "polygon", "circle", "ellipse"].contains(&node.tag_name().name())
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|class| class == "label-container")
                })
        })
    {
        return shape;
    }

    label_node
        .ancestors()
        .filter(|node| node.has_tag_name("g"))
        .find_map(|group| {
            group.children().find(|child| {
                child.has_tag_name("rect")
                    && (child.attribute("fill").is_some() || child.attribute("style").is_some())
            })
        })
        .unwrap_or_else(|| panic!("missing C4 shape for label {label}"))
}

fn c4_effective_property(node: roxmltree::Node<'_, '_>, property: &str) -> Option<String> {
    let from_style = node.attribute("style").and_then(|style| {
        style.split(';').find_map(|declaration| {
            let (key, value) = declaration.split_once(':')?;
            (key.trim() == property).then_some(
                value
                    .trim()
                    .trim_end_matches("!important")
                    .trim()
                    .to_owned(),
            )
        })
    });
    from_style.or_else(|| node.attribute(property).map(str::to_owned))
}

fn deep_c4_boundary_chain(depth: usize) -> String {
    let mut input = String::from("C4Context\n");
    for level in 0..depth {
        input.push_str(&format!("Boundary(b{level}, \"B{level}\") {{\n"));
    }
    input.push_str("System(leaf, \"Leaf\")\n");
    for _ in 0..depth {
        input.push_str("}\n");
    }
    input
}

#[derive(Default)]
struct C4FontHost {
    probes: Mutex<Vec<(String, TextMeasurementOperation, TextStyle)>>,
}

impl HostTextMeasurer for C4FontHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        self.probes.lock().unwrap().push((
            request.text.to_owned(),
            request.operation,
            request.style.clone(),
        ));
        let width = request.text.chars().count() as f64 * request.style.font_size * 0.5;
        Ok(Some(match request.operation {
            TextMeasurementOperation::Measure
            | TextMeasurementOperation::Wrapped
            | TextMeasurementOperation::MermaidCalculateTextDimensions => {
                HostTextMeasurement::Metrics(TextMetrics {
                    width,
                    height: request.style.font_size,
                    line_count: 1,
                })
            }
            TextMeasurementOperation::BBoxX => HostTextMeasurement::HorizontalExtents {
                left: 0.0,
                right: width,
            },
            TextMeasurementOperation::ComputedLength => HostTextMeasurement::Length(width),
            TextMeasurementOperation::RawBBoxHeight => {
                HostTextMeasurement::Length(request.style.font_size)
            }
            TextMeasurementOperation::CreateTextBBoxYOffset => {
                HostTextMeasurement::Length(-request.style.font_size * 0.8)
            }
            _ => return Ok(None),
        }))
    }
}

fn render_c4_with_font_host(
    source: &str,
    config: serde_json::Value,
) -> (
    family::RenderedFamilySvg,
    Vec<(String, TextMeasurementOperation, TextStyle)>,
    C4DiagramLayout,
) {
    let parsed = MermaidEngine::new()
        .with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let host = Arc::new(C4FontHost::default());
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.c4-font-host").unwrap(),
        "1",
    )
    .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::host_display(
            identity,
            host.clone(),
            TextMeasurementPhase::ALL,
        ))
        .begin_session()
        .unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare C4 through the production host measurement route");
    // Snapshot preparation before the writer can contribute any measurement probes.
    let probes = host.probes.lock().unwrap().clone();
    let projection = artifact
        .layout_json()
        .expect("serialize host-measured C4 layout");
    let layout = serde_json::from_value(projection["layout"]["C4Diagram"].clone()).unwrap();
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    (rendered, probes, layout)
}

#[test]
fn c4_host_measurement_resolves_inherited_fonts_without_rewriting_css() {
    for family in ["", "inherit", "unset", r"i\6eherit", r"u\6eset"] {
        for has_title in [false, true] {
            let config = serde_json::json!({"themeVariables":{"fontFamily":"Cousine,serif","fontSize":"29px"}, "c4": {
                "wrap": false,
                "systemFontFamily": family, "systemFontSize": 20, "systemFontWeight": 600,
                "boundaryFontFamily": family, "boundaryFontSize": 22, "boundaryFontWeight": 500,
                "messageFontFamily": family, "messageFontSize": 24, "messageFontWeight": 300
            }});
            let source = format!(
                "{}C4Context\nDeployment_Node(boundary, \"BoundaryName\", \"BoundaryType\", \"BoundaryDescription\") {{\nSystem(a, \"NodeName\", \"NodeDescription\")\n}}\nSystem(b, \"Peer\")\nRel(a, b, \"MessageLabel\", \"MessageTechnology\")\n",
                if has_title {
                    "---\ntitle: Context\n---\n"
                } else {
                    ""
                }
            );
            let (rendered, probes, _) = render_c4_with_font_host(&source, config);
            for (text, operation, size, weight) in [
                ("NodeName", TextMeasurementOperation::BBoxX, 20.0, "bold"),
                (
                    "[Software System]",
                    TextMeasurementOperation::BBoxX,
                    15.0,
                    "600",
                ),
                (
                    "NodeDescription",
                    TextMeasurementOperation::BBoxX,
                    16.4,
                    "600",
                ),
                (
                    "BoundaryName",
                    TextMeasurementOperation::MermaidCalculateTextDimensions,
                    24.0,
                    "bold",
                ),
                (
                    "[BoundaryType]",
                    TextMeasurementOperation::MermaidCalculateTextDimensions,
                    22.0,
                    "500",
                ),
                (
                    "BoundaryDescription",
                    TextMeasurementOperation::MermaidCalculateTextDimensions,
                    20.0,
                    "500",
                ),
                (
                    "MessageLabel",
                    TextMeasurementOperation::MermaidCalculateTextDimensions,
                    24.0,
                    "300",
                ),
                (
                    "MessageTechnology",
                    TextMeasurementOperation::MermaidCalculateTextDimensions,
                    24.0,
                    "300",
                ),
            ] {
                assert!(
                    probes.iter().any(|(actual_text, actual_operation, style)| {
                        actual_text == text
                            && *actual_operation == operation
                            && style.font_family.as_deref() == Some("Cousine,serif")
                            && (style.font_size - size).abs() < 1e-9
                            && style.font_weight.as_deref() == Some(weight)
                    }),
                    "{family}, title={has_title}, missing resolved probe for {text}: {probes:?}"
                );
            }
            assert!(
                probes.iter().all(|(_, _, style)| {
                    style.font_family.as_deref() == Some("Cousine,serif")
                        || style.font_family.as_deref() == Some("sans-serif")
                }),
                "legacy dual-font probes may use sans-serif, never unresolved inheritance"
            );
            let family_css = if family.is_empty() {
                String::new()
            } else {
                format!("font-family:{family};")
            };
            assert!(rendered.svg().contains(&format!(
                ".c4-shape.c4-system .label{{{family_css}font-size:20px;font-weight:600;}}"
            )));
            assert!(rendered.svg().contains(&format!("font-family: {family};")));
        }
    }
}

#[test]
fn c4_zero_and_fractional_legacy_font_sizes_follow_visible_lines() {
    let nodes = "System(a, \"A\")\nSystem(b, \"B\")\n";
    for (key, base, text, raw, used) in [
        ("messageFontSize", 0.0, "Calls", 0.0, 0.0),
        ("messageFontSize", 0.5, "Calls", 0.5, 0.5),
        ("messageFontSize", -1.0, "Calls", -1.0, 29.0),
        ("messageFontSize", -3.0, "Calls", -3.0, 29.0),
        ("messageFontSize", -5e-7, "Calls", -5e-7, 29.0),
        ("messageFontSize", 5e-7, "Calls", 5e-7, 5e-7),
        ("boundaryFontSize", 0.0, "BoundaryName", 2.0, 2.0),
        ("boundaryFontSize", 0.0, "[Type]", 0.0, 0.0),
        ("boundaryFontSize", 0.0, "Description", -2.0, 29.0),
        ("boundaryFontSize", 0.5, "Description", -1.5, 29.0),
        ("boundaryFontSize", 2.0, "Description", 0.0, 0.0),
        (
            "boundaryFontSize",
            1.9999995,
            "Description",
            1.9999995 - 2.0,
            29.0,
        ),
        (
            "boundaryFontSize",
            2.0000005,
            "Description",
            2.0000005 - 2.0,
            2.0000005 - 2.0,
        ),
        ("boundaryFontSize", 2.5, "Description", 0.5, 0.5),
        ("boundaryFontSize", -1.0, "BoundaryName", 1.0, 1.0),
        ("boundaryFontSize", -1.0, "[Type]", -1.0, 29.0),
        ("boundaryFontSize", -1.0, "Description", -3.0, 29.0),
        ("boundaryFontSize", -3.0, "BoundaryName", -1.0, 29.0),
        ("boundaryFontSize", -3.0, "[Type]", -3.0, 29.0),
        ("boundaryFontSize", -3.0, "Description", -5.0, 29.0),
    ] {
        let body = match text {
            "Calls" => format!("{nodes}Rel(a, b, \"Calls\", \"HTTPS\")\n"),
            "BoundaryName" => {
                "Boundary(b, \"BoundaryName\", \"\") {\nSystem(a, \"A\")\n}\n".to_owned()
            }
            "[Type]" => "Boundary(b, \"\", \"Type\") {\nSystem(a, \"A\")\n}\n".to_owned(),
            _ => "Deployment_Node(b, \"\", \"\", \"Description\") {\nSystem(a, \"A\")\n}\n"
                .to_owned(),
        };
        let source = format!("C4Context\n{body}");
        for family in ["inherit", "Independent"] {
            let mut config = serde_json::json!({"themeVariables":{"fontFamily":"Cousine","fontSize":"29px"}, "c4": {
                "wrap":false, "boundaryFontFamily":family, "messageFontFamily":family
            }});
            config["c4"][key] = serde_json::json!(base);
            let (rendered, probes, _) = render_c4_with_font_host(&source, config.clone());
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let label = document
                .descendants()
                .find(|node| node.has_tag_name("text") && svg_text_content(*node) == text)
                .expect("the zero-sized row is still emitted");
            let css_size = c4_effective_property(label, "font-size").unwrap();
            assert_eq!(css_size, format!("{raw}px"), "{config}, {source}");
            // The legacy writer emits only numeric px declarations. Invalid negatives are ignored
            // by CSS and inherit the root, while zero remains a valid invisible declaration.
            let computed = css_size.strip_suffix("px").unwrap().parse::<f64>().unwrap();
            let computed = if computed < 0.0 { 29.0 } else { computed };
            assert_eq!(computed, used, "{config}, {source}");
            let measured_family = if family == "inherit" {
                "Cousine"
            } else {
                family
            };
            let configured_probes = probes
                .iter()
                .filter(|(actual, operation, style)| {
                    actual == text
                        && *operation == TextMeasurementOperation::MermaidCalculateTextDimensions
                        && style.font_family.as_deref() == Some(measured_family)
                })
                .collect::<Vec<_>>();
            assert!(
                !configured_probes.is_empty(),
                "missing host probes: {config}, {source}"
            );
            assert!(
                configured_probes
                    .iter()
                    .all(|(_, _, style)| style.font_size == computed),
                "{configured_probes:?}"
            );
            assert!(probes.iter().all(|(_, _, style)| style.font_size >= 0.0));
        }
    }
}

#[test]
fn c4_invalid_element_sizes_inherit_root_before_section_scaling() {
    let source = "C4Context\nSystem(a, \"NodeName\", \"NodeDescription\")\n";
    for (raw, used) in [(-3.0, 29.0), (-1.0, 29.0), (0.0, 29.0), (1e-7, 1e-7)] {
        for family in ["inherit", "Independent"] {
            let config = serde_json::json!({"themeVariables":{"fontFamily":"Cousine","fontSize":"29px"}, "c4": {
                "wrap":false, "systemFontSize":raw, "systemFontFamily":family,
                "systemFontWeight":600
            }});
            let (rendered, probes, _) = render_c4_with_font_host(source, config.clone());
            assert!(rendered.svg().contains("font-size:29px;"));
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let stylesheet = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .unwrap()
                .text()
                .unwrap();
            let element_rule = stylesheet
                .split(".c4-shape.c4-system .label{")
                .nth(1)
                .unwrap()
                .split('}')
                .next()
                .unwrap();
            if raw <= 0.0 {
                assert!(
                    !element_rule.contains("font-size:"),
                    "CSSOM drops invalid or falsy sizes: {element_rule}"
                );
            } else {
                assert!(
                    element_rule.contains(&format!("font-size:{raw}px;")),
                    "tiny positive CSS sizes remain positive: {element_rule}"
                );
            }
            assert!(
                stylesheet.contains(".c4-type{font-size:0.75em;}")
                    && stylesheet.contains(".c4-descr{font-size:0.82em;}")
            );
            let measured_family = if family == "inherit" {
                "Cousine"
            } else {
                family
            };
            for (text, computed, weight) in [
                ("NodeName", used, "bold"),
                ("[Software System]", used * 0.75, "600"),
                ("NodeDescription", used * 0.82, "600"),
            ] {
                let measured = probes
                    .iter()
                    .filter(|(actual, operation, _)| {
                        actual == text && *operation == TextMeasurementOperation::BBoxX
                    })
                    .collect::<Vec<_>>();
                assert!(!measured.is_empty(), "missing section probe for {text}");
                assert!(
                    measured.iter().all(|(_, _, style)| {
                        style.font_family.as_deref() == Some(measured_family)
                            && (style.font_size - computed).abs() < 1e-9
                            && style.font_weight.as_deref() == Some(weight)
                    }),
                    "{config}, {text}: {measured:?}"
                );
            }
        }
    }
}

#[test]
fn c4_element_css_sizes_keep_source_units_inheritance_before_measurement() {
    let body = "C4Context\nSystem(a, \"NodeName\", \"NodeDescription\")\n";
    for (raw, used, css, _inherits) in [
        (serde_json::json!(""), 29.0, None, true),
        (serde_json::json!(false), 29.0, None, true),
        (serde_json::json!("bad"), 29.0, None, true),
        (serde_json::json!("24"), 29.0, None, true),
        (serde_json::json!("24pt"), 32.0, Some("24pt"), false),
        (serde_json::json!("150%"), 43.5, Some("150%"), false),
        (serde_json::json!("0em"), 0.0, Some("0em"), false),
        (serde_json::json!("0rem"), 0.0, Some("0rem"), false),
        (serde_json::json!("inherit"), 29.0, Some("inherit"), true),
        (serde_json::json!("unset"), 29.0, Some("unset"), true),
    ] {
        for from_source in [false, true] {
            let config = serde_json::json!({"themeVariables":{"fontFamily":"Cousine","fontSize":"29px"}, "c4": {
                "wrap":false, "systemFontFamily":"inherit", "systemFontSize":raw,
                "systemFontWeight":600
            }});
            let (source, site) = if from_source {
                (
                    format!("---\nconfig: {config}\n---\n{body}"),
                    serde_json::json!({}),
                )
            } else {
                (body.to_owned(), config)
            };
            let (rendered, probes, _) = render_c4_with_font_host(&source, site);
            let stylesheet = roxmltree::Document::parse(rendered.svg())
                .unwrap()
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .unwrap()
                .text()
                .unwrap()
                .to_owned();
            let rule = stylesheet
                .split(".c4-shape.c4-system .label{")
                .nth(1)
                .unwrap()
                .split('}')
                .next()
                .unwrap();
            match css {
                Some(css) => assert!(rule.contains(&format!("font-size:{css};")), "{raw}: {rule}"),
                None => assert!(!rule.contains("font-size:"), "{raw}: {rule}"),
            }
            for (text, size, weight) in [
                ("NodeName", used, "bold"),
                ("[Software System]", used * 0.75, "600"),
                ("NodeDescription", used * 0.82, "600"),
            ] {
                if used > 0.0 {
                    assert!(
                        probes.iter().any(|(actual, operation, style)| {
                            actual == text
                                && *operation == TextMeasurementOperation::BBoxX
                                && (style.font_size - size).abs() < 1e-9
                                && style.font_family.as_deref() == Some("Cousine")
                                && style.font_weight.as_deref() == Some(weight)
                        }),
                        "{raw}, {text}: {probes:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn c4_element_cssom_rejects_literal_priority_before_normalizing_fonts() {
    let source = "C4Context\nSystem(a, \"NodeName\", \"NodeDescription\")\n";
    for (family, size, family_css, size_css, used, inherits_family, _inherits_size) in [
        (
            "serif !important",
            "24px !important",
            None,
            None,
            29.0,
            true,
            true,
        ),
        (
            r#""A: B", serif !important"#,
            "0em !important",
            None,
            None,
            29.0,
            true,
            true,
        ),
        ("serif;", "24pt !IMPORTANT", None, None, 29.0, true, true),
        ("inherit", "24px;", Some("inherit"), None, 29.0, true, true),
        (
            "inherit",
            "calc(24px);",
            Some("inherit"),
            None,
            29.0,
            true,
            true,
        ),
        (
            r#""A: !important", serif"#,
            "24pt",
            Some(r#""A: !important",serif"#),
            Some("24pt"),
            32.0,
            false,
            false,
        ),
        (
            "inherit !IMPORTANT",
            "unset !important",
            Some("inherit !important"),
            Some("unset !important"),
            29.0,
            true,
            true,
        ),
    ] {
        // Source presentation filtering rejects unquoted priority before the family adapter.
        // Site input exercises the actual raw CSSOM boundary retained by Mermaid's C4 stylesheet.
        let config = serde_json::json!({"themeVariables":{"fontFamily":"Cousine","fontSize":"29px"}, "c4": {"wrap":false,
            "systemFontFamily":family,"systemFontSize":size,"systemFontWeight":600}});
        let (rendered, probes, _) = render_c4_with_font_host(source, config);
        let stylesheet = roxmltree::Document::parse(rendered.svg())
            .unwrap()
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap()
            .to_owned();
        let rule = stylesheet
            .split(".c4-shape.c4-system .label{")
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        for (property, value) in [("font-family", family_css), ("font-size", size_css)] {
            match value {
                Some(value) => assert!(
                    rule.contains(&format!("{property}:{value};")),
                    "{family}/{size}: {rule}"
                ),
                None => assert!(
                    !rule.contains(&format!("{property}:")),
                    "{family}/{size}: {rule}"
                ),
            }
        }
        let expected_family = if inherits_family {
            "Cousine"
        } else {
            r#""A: !important",serif"#
        };
        assert!(
            probes.iter().any(|(text, operation, style)| {
                text == "NodeName"
                    && *operation == TextMeasurementOperation::BBoxX
                    && style.font_size == used
                    && style.font_family.as_deref() == Some(expected_family)
            }),
            "{family}/{size}: {probes:?}"
        );
    }
}

#[test]
fn c4_contextual_root_sizes_preserve_css_and_keep_independent_element_sizes() {
    let body = "C4Context\ntitle Root Size\nSystem(a, \"API\")\n";
    for size in [
        "1em",
        "100%",
        "1rem",
        "calc(24px)",
        "var(--audit-size, 24px)",
        "calc(24px);",
        "1em;",
        "100%;",
    ] {
        for from_source in [false, true] {
            if from_source && (size.starts_with("var(") || size.ends_with(';')) {
                continue;
            }
            let config = serde_json::json!({"themeVariables":{"fontSize":size}});
            let (source, site) = if from_source {
                (
                    format!("---\nconfig: {config}\n---\n{body}"),
                    serde_json::json!({}),
                )
            } else {
                (body.to_owned(), config)
            };
            let (rendered, _, _) = render_c4_with_font_host(&source, site);
            assert!(rendered.svg().contains(&format!("font-size:{size};")));
            assert!(rendered.svg().contains(
                ".c4-shape.c4-system .label{font-family:\"Open Sans\",sans-serif;font-size:14px;"
            ));
        }
    }
}

#[test]
fn c4_subpixel_root_fallback_multiline_sections_use_actual_em_steps() {
    let config = serde_json::json!({
        "themeVariables": {"fontFamily":"Cousine", "fontSize":"0.5px"},
        "c4": {"wrap":false, "systemFontSize":-1, "systemFontFamily":"inherit"}
    });
    for from_source in [false, true] {
        let render = |body: &str| {
            if from_source {
                let source = format!("---\nconfig: {config}\n---\n{body}");
                render_c4_with_font_host(&source, serde_json::json!({}))
            } else {
                render_c4_with_font_host(body, config.clone())
            }
        };
        let (_, _, single) = render("C4Context\nSystem(a, \"A\", \"First\")\n");
        let (rendered, probes, multi) =
            render("C4Context\nSystem(a, \"A<br/>B\", \"First<br/>Second\")\n");
        assert!(rendered.svg().contains("font-size:0.5px;"));
        let single = &single.shapes[0];
        let multi = &multi.shapes[0];
        assert!((multi.label.height - single.label.height - 0.55).abs() < 1e-9);
        assert!((multi.type_block.height - single.type_block.height).abs() < 1e-9);
        assert!(
            (multi.descr.as_ref().unwrap().height - single.descr.as_ref().unwrap().height - 0.451)
                .abs()
                < 1e-9
        );
        assert!((multi.height - single.height - 1.001).abs() < 1e-9);
        for (text, size) in [("B", 0.5), ("Second", 0.41)] {
            assert!(probes.iter().any(|(actual, operation, style)| {
                actual == text
                    && *operation == TextMeasurementOperation::BBoxX
                    && (style.font_size - size).abs() < 1e-9
                    && style.font_family.as_deref() == Some("Cousine")
            }));
        }
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for class in ["c4-name", "c4-descr"] {
            let section = document
                .descendants()
                .find(|node| node.attribute("class") == Some(class))
                .unwrap();
            let rows = section
                .descendants()
                .filter(|node| node.attribute("class") == Some("text-outer-tspan row"))
                .collect::<Vec<_>>();
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].attribute("y"), Some("-0.1em"));
            assert_eq!(rows[1].attribute("y"), Some("1em"));
            assert!(rows.iter().all(|row| row.attribute("dy") == Some("1.1em")));
        }
    }
}

#[test]
fn c4_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"C4Context
Person(user, "User")
System_Boundary(system, "Bounded system") {
  Container(api, "API", "Rust")
  ContainerDb(database, "Database", "PostgreSQL")
}
Rel(user, api, "Calls", "HTTPS")
Rel(api, database, "Reads", "SQL")
"#;
    let baseline = try_render_c4_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded C4 baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "C4 fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact C4 SVG byte ceiling");
    let exact = try_render_c4_svg_with_resource_policy(source, exact_policy)
        .expect("the exact C4 family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact C4 SVG byte ceiling");
    let error = try_render_c4_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the C4 family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected C4 MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
    assert!(limit.explicit_overrides.iter().any(|resource_override| {
        resource_override.id == ResourceLimitId::MaxSvgBytes
            && resource_override.value == below_exact
    }));
}

#[test]
fn c4_public_layout_and_svg_render_handle_deep_boundary_chain() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    const DEPTH: usize = 1500;
    let source = deep_c4_boundary_chain(DEPTH);

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    assert_eq!(parsed.metadata().diagram_type, "c4");

    let options = LayoutOptions::default();
    let artifact = family::prepare(&parsed, &options, session)
        .expect("layout should not depend on recursive boundary traversal");
    let projection = artifact.layout_json().expect("serialize C4 layout");
    let c4: C4DiagramLayout = serde_json::from_value(projection["layout"]["C4Diagram"].clone())
        .expect("C4 layout projection");

    assert_eq!(c4.boundaries.len(), DEPTH + 1);
    assert_eq!(c4.shapes.len(), 1);
    assert_eq!(c4.shapes[0].alias, "leaf");

    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("SVG painting should use an iterative boundary traversal")
        .svg()
        .to_owned();
    assert!(svg.contains(">Leaf</tspan>"));
    assert!(svg.contains(">B0</tspan>"));
    assert!(svg.contains(&format!(">B{}</tspan>", DEPTH - 1)));
}

#[test]
fn c4_unified_shapes_render_canonical_labels() {
    let svg = render_c4_svg_with_environment(
        r#"C4Context
Person(person, "Person")
Container(system, "System", "Rust", "A short description")
Person_Ext(external, "External")
System(framed, "Framed", "A component-shaped system", $shape="component")
"#,
        &RenderEnvironment::deterministic(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");

    for class in ["c4-name", "c4-type", "c4-descr"] {
        assert!(
            document
                .descendants()
                .any(|node| { node.has_tag_name("g") && node.attribute("class") == Some(class) }),
            "missing unified C4 label section {class}: {svg}"
        );
    }
    let type_texts = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("c4-type"))
        .map(svg_text_content)
        .collect::<Vec<_>>();
    assert!(type_texts.iter().any(|text| text == "[Person]"));
    assert!(type_texts.iter().any(|text| text == "[Container: Rust]"));
    assert!(
        document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("c4-descr"))
            .map(svg_text_content)
            .any(|text| text == "A short description")
    );
    assert_eq!(svg.matches("<circle ").count(), 2);
    assert_eq!(svg.matches("<polygon ").count(), 1);
    assert!(!svg.contains("<<person>>"));
    assert!(!svg.contains("<<system>>"));
    assert!(!svg.contains("<<external_person>>"));
    assert!(
        document
            .descendants()
            .all(|node| node.attribute("data-look").is_none()),
        "Classic C4 must retain the upstream DOM without an unused look attribute"
    );
}

#[test]
fn c4_neo_look_reaches_layout_and_svg_terminals() {
    let classic_source = r#"C4Context
ContainerDb(database, "Database", "PostgreSQL")
"#;
    let neo_source = r#"%%{init: {"look": "neo"}}%%
C4Context
ContainerDb(database, "Database", "PostgreSQL")
"#;

    let classic = layout_c4_with_options(classic_source, &LayoutOptions::default());
    let neo = layout_c4_with_options(neo_source, &LayoutOptions::default());
    let classic_shape = classic.shapes.first().expect("classic C4 shape");
    let neo_shape = neo.shapes.first().expect("neo C4 shape");

    // Mermaid's 11.17 cylinder handler increases the vertical label padding from 20px to
    // 24px for Neo. Keep the assertion relational so host text metrics do not become a pixel
    // oracle for this semantic difference.
    assert!(
        neo_shape.height > classic_shape.height + 3.0,
        "Neo cylinder padding must affect layout height: classic={:?}, neo={:?}",
        classic_shape,
        neo_shape
    );

    let svg = render_c4_svg_with_environment(neo_source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).expect("valid Neo C4 SVG");
    let shape_group = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|c| c == "c4-shape"))
        })
        .expect("Neo C4 shape group");
    assert_eq!(shape_group.attribute("data-look"), Some("neo"));
    let style = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("C4 stylesheet");
    assert!(
        style.contains(r#"[data-look="neo"].node path"#),
        "the common Neo selector must be reachable from C4 terminals"
    );
}

#[test]
fn c4_neo_component_uses_framed_rectangle_geometry() {
    let source = r#"%%{init: {"look": "neo", "c4": {"width": 1}}}%%
C4Context
System(framed, "Framed", "Component description", $shape="component")
"#;
    let layout = layout_c4_with_options(source, &LayoutOptions::default());
    let shape = layout.shapes.first().expect("Neo component shape");

    let mut content_width = shape.label.width.max(shape.type_block.width);
    let mut content_height = shape.label.height + shape.type_block.height;
    let mut section_count = 2_usize;
    if let Some(description) = shape.descr.as_ref() {
        content_width = content_width.max(description.width);
        content_height += description.height;
        section_count += 1;
    }
    content_height += 3.0 * section_count.saturating_sub(1) as f64;

    // Mermaid maps C4 `component` to `fr-rect`/subroutine. Its Neo geometry reserves two
    // eight-pixel frames plus 28px horizontal label padding and 12px total vertical padding.
    assert!((shape.width - (content_width + 44.0)).abs() < 1e-6);
    assert!((shape.height - (content_height + 12.0)).abs() < 1e-6);

    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).expect("valid Neo component SVG");
    let shape_group = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-framed"))
        .expect("Neo component group");
    let label_group = shape_group
        .children()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("label"))
        .expect("Neo component label group");
    let transform = label_group
        .attribute("transform")
        .and_then(|value| value.strip_prefix("translate("))
        .and_then(|value| value.strip_suffix(')'))
        .expect("translate transform");
    let mut values = transform.split(',').map(|value| {
        value
            .trim()
            .parse::<f64>()
            .expect("numeric translate component")
    });
    let translate_x = values.next().expect("translate x");
    let translate_y = values.next().expect("translate y");
    assert!(values.next().is_none());
    assert!((translate_x + content_width / 2.0).abs() < 1e-6);
    assert!((translate_y + content_height / 2.0).abs() < 1e-6);
}

#[test]
fn c4_hand_drawn_look_emits_rough_shape_geometry() {
    let source = r#"%%{init: {"look": "handDrawn"}}%%
C4Context
Person(person, "Person")
System(service, "Service")
ContainerDb(database, "Database", "PostgreSQL")
System(component, "Component", "Rust", $shape="component")
System(queue, "Queue", "SQS", $shape="queue")
"#;

    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).expect("valid hand-drawn C4 SVG");
    let shape_groups = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|c| c == "c4-shape"))
        })
        .collect::<Vec<_>>();
    assert_eq!(shape_groups.len(), 5);
    assert!(shape_groups.iter().all(|group| {
        group.attribute("data-look") == Some("handDrawn")
            && group.descendants().any(|node| node.has_tag_name("path"))
    }));
}

#[test]
fn c4_hand_drawn_look_honors_configured_seed() {
    fn rough_stroke_paths(svg: &str) -> Vec<String> {
        let document = roxmltree::Document::parse(svg).expect("valid hand-drawn C4 SVG");
        document
            .descendants()
            .filter(|node| {
                node.has_tag_name("path")
                    && node.attribute("class").is_some_and(|classes| {
                        classes
                            .split_whitespace()
                            .any(|class| class == "rough-stroke")
                    })
            })
            .map(|node| {
                node.attribute("d")
                    .expect("rough stroke path data")
                    .to_owned()
            })
            .collect()
    }

    let source_for_seed = |seed| {
        format!(
            r#"%%{{init: {{"look": "handDrawn", "handDrawnSeed": {seed}}}}}%%
C4Context
System(service, "Service")
"#
        )
    };
    let environment = RenderEnvironment::deterministic();
    let seeded_once = render_c4_svg_with_environment(&source_for_seed(7), &environment);
    let seeded_again = render_c4_svg_with_environment(&source_for_seed(7), &environment);
    let differently_seeded = render_c4_svg_with_environment(&source_for_seed(8), &environment);

    let paths = rough_stroke_paths(&seeded_once);
    assert!(!paths.is_empty());
    assert_eq!(paths, rough_stroke_paths(&seeded_again));
    assert_ne!(paths, rough_stroke_paths(&differently_seeded));
}

#[test]
fn c4_shape_named_type_does_not_override_the_rendered_stereotype() {
    let svg = render_c4_svg_with_environment(
        r#"C4Context
Container(app, "Application", $type="Domain Service")
"#,
        &RenderEnvironment::deterministic(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let type_texts = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("c4-type"))
        .map(svg_text_content)
        .collect::<Vec<_>>();

    assert_eq!(type_texts, ["[Container]"]);
}

#[test]
fn c4_shape_technology_reaches_layout_and_svg_while_named_type_is_ignored() {
    let source = r#"C4Context
Container(app, "Application", "Rust", $type="Domain Service")
"#;
    let layout = layout_c4_with_options(source, &LayoutOptions::default());
    let shape = layout
        .shapes
        .iter()
        .find(|shape| shape.alias == "app")
        .expect("C4 application shape");

    assert_eq!(shape.type_block.text, "[Container: Rust]");

    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let type_texts = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("c4-type"))
        .map(svg_text_content)
        .collect::<Vec<_>>();

    assert_eq!(type_texts, ["[Container: Rust]"]);
    assert!(!svg.contains("Domain Service"));
}

#[test]
fn c4_uses_explicit_screen_available_width_without_changing_container_geometry() {
    let source = include_str!(
        "../../../fixtures/c4/upstream_docs_c4_c4_container_diagram_c4container_006.mmd"
    );
    let default = layout_c4_with_options(source, &LayoutOptions::default());
    let wide_screen = layout_c4_with_options(
        source,
        &LayoutOptions::default().with_screen_available_width(1280.0),
    );

    assert_eq!(default.container_width, 800.0);
    assert_eq!(default.screen_available_width, None);
    assert_eq!(wide_screen.container_width, 800.0);
    assert_eq!(wide_screen.screen_available_width, Some(1280.0));
    assert!(wide_screen.width > default.width);
    assert!(wide_screen.height < default.height);
}

#[test]
fn c4_svg_paints_each_boundary_subtree_in_mermaid_order() {
    let svg = render_c4_svg_with_environment(
        r#"C4Context
System(root_before, "Root Before")
Boundary(outer, "Outer Boundary") {
  System(outer_before, "Outer Before")
  Boundary(inner, "Inner Boundary") {
    System(inner_shape, "Inner Shape")
  }
  System(outer_after, "Outer After")
}
Boundary(sibling, "Sibling Boundary") {
  System(sibling_shape, "Sibling Shape")
}
System(root_after, "Root After")
Rel(inner_shape, root_before, "Leaves subtree")
"#,
        &RenderEnvironment::deterministic(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let labels = document
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(svg_text_content)
        .filter(|text| {
            matches!(
                text.as_str(),
                "Root Before"
                    | "Root After"
                    | "Outer Before"
                    | "Outer After"
                    | "Inner Shape"
                    | "Inner Boundary"
                    | "Outer Boundary"
                    | "Sibling Shape"
                    | "Sibling Boundary"
                    | "Leaves subtree"
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        labels,
        [
            "Root Before",
            "Root After",
            "Outer Before",
            "Outer After",
            "Inner Shape",
            "Inner Boundary",
            "Outer Boundary",
            "Sibling Shape",
            "Sibling Boundary",
            "Leaves subtree",
        ]
    );
}

#[test]
fn c4_relation_keeps_explicit_line_and_text_styles() {
    let svg = render_c4_svg_with_environment(
        r#"C4Context
System(a, "A")
System(b, "B")
Rel(a, b, "Calls", "HTTPS")
UpdateRelStyle(a, b, $textColor="red", $lineColor="blue", $offsetX="10", $offsetY="20")
"#,
        &RenderEnvironment::deterministic(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let calls = document
        .descendants()
        .find(|node| node.has_tag_name("text") && svg_text_content(*node) == "Calls")
        .expect("relationship label");
    let technology = document
        .descendants()
        .find(|node| node.has_tag_name("text") && svg_text_content(*node) == "[HTTPS]")
        .expect("relationship technology label");
    let line = document
        .descendants()
        .find(|node| {
            matches!(node.tag_name().name(), "line" | "path")
                && node.attribute("stroke") == Some("blue")
                && node.attribute("marker-end").is_some()
        })
        .expect("relationship line");

    assert_eq!(calls.attribute("fill"), Some("red"));
    assert!(calls.attribute("style").is_some_and(|style| {
        style.contains("text-anchor: middle") && style.contains("font-family:")
    }));
    assert_eq!(technology.attribute("fill"), Some("red"));
    assert_eq!(technology.attribute("font-style"), Some("italic"));
    assert_eq!(line.attribute("stroke-width"), Some("1"));
    assert_eq!(line.attribute("style"), Some("fill: none;"));
}

#[test]
fn c4_relationships_resolve_boundary_endpoints_and_prefer_shapes() {
    let source = r#"C4Context
System(outside, "Outside")
Boundary(left, "Left") {
  System(inside, "Inside")
}
Boundary(right, "Right") {
  System(other, "Other")
}
Rel(outside, left, "Enters")
Rel(left, outside, "Leaves")
Rel(left, right, "Connects")
"#;
    let layout = layout_c4_with_options(source, &LayoutOptions::headless_svg_defaults());
    assert_eq!(layout.rels.len(), 3);
    for (alias, point) in [
        ("left", &layout.rels[0].end_point),
        ("left", &layout.rels[1].start_point),
        ("left", &layout.rels[2].start_point),
        ("right", &layout.rels[2].end_point),
    ] {
        let boundary = layout.boundaries.iter().find(|b| b.alias == alias).unwrap();
        let on_vertical = (point.x - boundary.x).abs() < 1e-8
            || (point.x - boundary.x - boundary.width).abs() < 1e-8;
        let on_horizontal = (point.y - boundary.y).abs() < 1e-8
            || (point.y - boundary.y - boundary.height).abs() < 1e-8;
        assert!(
            on_vertical || on_horizontal,
            "endpoint must meet {alias}'s perimeter"
        );
        assert!(point.x >= boundary.x - 1e-8 && point.x <= boundary.x + boundary.width + 1e-8);
        assert!(point.y >= boundary.y - 1e-8 && point.y <= boundary.y + boundary.height + 1e-8);
    }
    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    assert!(svg.contains("Enters") && svg.contains("Leaves") && svg.contains("Connects"));

    let shared_alias = source
        .replace("System(outside,", "System(left,")
        .replace("outside, left", "left, inside")
        .replace("left, outside", "inside, left");
    let layout = layout_c4_with_options(&shared_alias, &LayoutOptions::headless_svg_defaults());
    let shape = layout
        .shapes
        .iter()
        .find(|shape| shape.alias == "left")
        .unwrap();
    let endpoint = &layout.rels[0].start_point;
    assert!(endpoint.x >= shape.x - 1e-8 && endpoint.x <= shape.x + shape.width + 1e-8);
    assert!(endpoint.y >= shape.y - 1e-8 && endpoint.y <= shape.y + shape.height + 1e-8);
}

#[test]
fn c4_hand_drawn_person_body_shares_the_classic_body_coordinate_domain() {
    let source = "%%{init: {\"look\": \"handDrawn\"}}%%\nC4Context\nPerson(person, \"Person\")\n";
    let layout = layout_c4_with_options(source, &LayoutOptions::default());
    let shape = &layout.shapes[0];
    let radius = (shape.width * 0.23).clamp(16.0, 56.0);
    let overlap = radius * 0.27;
    let body_height = shape.height - 2.0 * radius + overlap;
    let body_top = -shape.height / 2.0 + 2.0 * radius - overlap;
    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).unwrap();
    let body = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g") && node.attribute("class") == Some("basic label-container")
        })
        .unwrap();
    let transforms = body
        .children()
        .filter(|node| node.has_tag_name("g"))
        .map(|node| node.attribute("transform").unwrap())
        .collect::<Vec<_>>();
    assert_eq!(transforms.len(), 2);
    let translate_y = |value: &str| {
        value
            .strip_prefix("translate(0,")
            .unwrap()
            .strip_suffix(')')
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    assert!((translate_y(transforms[0]) - (body_top + body_height / 2.0)).abs() < 1e-6);
    assert!((translate_y(transforms[1]) - (-shape.height / 2.0 + radius)).abs() < 1e-6);
    assert!(translate_y(transforms[0]) > translate_y(transforms[1]));
}

const PRESENTATION_C4: &str = r#"C4Context
title Theme coverage
Boundary(boundary, "Boundary title", "Boundary type") {
  Person(person, "Person", "Person description")
  System(system, "System", "System description")
  Deployment_Node(node, "Node title", "Node type", "Boundary description") {
    System(child, "Child")
  }
}
System_Ext(external, "External", "External description")
BiRel(person, system, "Uses", "HTTPS")
Rel(system, external, "Calls")
"#;
#[test]
fn c4_dynamic_text_colors_remain_single_xml_attributes_for_site_and_source_values() {
    let site_color = r#"red" data-probe="site&raw"#;
    let authored_color = "blue' data-probe='source&raw";
    let source = format!(
        r#"{PRESENTATION_C4}
UpdateRelStyle(system, external, $textColor="{authored_color}")
"#
    );
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({"c4": {
            "boundary_font_color": site_color,
            "relationship_text_color": site_color
        }})),
    );
    let parsed = engine
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let svg = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned();
    let document =
        roxmltree::Document::parse(&svg).expect("dynamic color values must not break XML");
    for (content, expected) in [
        ("Boundary title", site_color),
        ("Boundary description", site_color),
        ("Uses", site_color),
        ("[HTTPS]", site_color),
        ("Calls", authored_color),
    ] {
        let text = document
            .descendants()
            .find(|node| node.has_tag_name("text") && svg_text_content(*node) == content)
            .unwrap();
        assert_eq!(text.attribute("fill"), Some(expected), "{content}");
    }
    assert!(
        document
            .descendants()
            .all(|node| node.attribute("data-probe").is_none())
    );
}

mod common;

#[test]
fn c4_svg_preserves_db_accessibility_text() {
    for (description, expected) in [
        ("First\n<!--a11y-->", Some("First\n")),
        ("<!--a11y-->\u{0085}<!--a11y-->", Some("\u{0085}")),
        ("<!--a11y-->", None),
    ] {
        let source = format!("C4Context\naccDescr {{\n{description}\n}}\nSystem(app, \"App\")\n");
        let svg = render_c4_svg_with_environment(&source, &RenderEnvironment::deterministic());
        common::assert_svg_accessibility(&svg, None, expected);
    }
}
