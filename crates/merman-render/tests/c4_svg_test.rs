use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, GradientStop,
    LinearGradient, OrdinalPalette, OrdinalSelector, PatternKind, PatternSpec, RadialGradient,
    Specified, ThemeColorValue, ThemeGeometryPatch, ThemeLength, ThemePortabilityRequirement,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant,
    TypographySpec,
};
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
use std::sync::{Arc, Mutex};

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

fn c4_cluster_rules_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    let styles = rules
        .into_iter()
        .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule);
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile C4 cluster theme")
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

fn render_c4_with_preset(
    source: &str,
    preset: merman_render::diagram_theme::ThemePreset,
    engine: MermaidEngine,
) -> String {
    let theme = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
    let parsed = common::install_parse_compatibility(&theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_theme(&theme)
        .unwrap();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned()
}

#[test]
fn c4_host_presets_pair_node_surfaces_with_their_label_foregrounds() {
    use merman_render::diagram_theme::ThemePreset;
    let source = "C4Context\nPerson(user, \"User\")\nSystem(app, \"App\")\nSystemDb(db, \"Database\")\nSystem_Ext(ext, \"External\")\n";
    for (preset, fill, stroke, text) in [
        (ThemePreset::EditorLight, "#f8fafc", "#94a3b8", "#0f172a"),
        (ThemePreset::EditorDark, "#111827", "#475569", "#e5e7eb"),
    ] {
        let svg = render_c4_with_preset(source, preset, MermaidEngine::new());
        let document = roxmltree::Document::parse(&svg).unwrap();
        for label in ["User", "App", "Database", "External"] {
            let text_node = document
                .descendants()
                .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
                .unwrap();
            assert_eq!(
                c4_effective_property(text_node, "fill").as_deref(),
                Some(text)
            );
            assert_eq!(
                c4_node_body_property(text_node, "fill").as_deref(),
                Some(fill)
            );
            assert_eq!(
                c4_node_body_property(text_node, "stroke").as_deref(),
                Some(stroke)
            );
            let label_group = text_node
                .ancestors()
                .find(|node| node.attribute("class") == Some("label"))
                .unwrap();
            assert_eq!(
                c4_effective_property(label_group, "color").as_deref(),
                Some(text)
            );
        }
    }
    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).unwrap();
    for (label, fill) in [
        ("User", "#08427B"),
        ("App", "#1168BD"),
        ("Database", "#1168BD"),
        ("External", "#999999"),
    ] {
        let text_node = document
            .descendants()
            .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
            .unwrap();
        assert_eq!(
            c4_effective_property(text_node, "fill").as_deref(),
            Some("#FFFFFF")
        );
        assert_eq!(
            c4_node_body_property(text_node, "fill").as_deref(),
            Some(fill)
        );
    }
}

#[test]
fn c4_node_source_and_site_paints_override_only_their_properties() {
    use merman_render::diagram_theme::ThemePreset;
    let source = "C4Context\nSystem(a, \"A\")\nSystem(b, \"B\")\nUpdateElementStyle(a, $fontColor=\"red\", $bgColor=\"yellow\")\n";
    for prefix in ["", "---\nconfig: {theme: base}\n---\n"] {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "c4": {"system_font_color": "green", "system_border_color": "blue"}
            })),
        );
        let svg = render_c4_with_preset(
            &format!("{prefix}{source}"),
            ThemePreset::EditorLight,
            engine,
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        for (label, text, fill) in [("A", "red", "yellow"), ("B", "green", "#f8fafc")] {
            let node = document
                .descendants()
                .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
                .unwrap();
            assert_eq!(c4_effective_property(node, "fill").as_deref(), Some(text));
            assert_eq!(c4_node_body_property(node, "fill").as_deref(), Some(fill));
            assert_eq!(
                c4_node_body_property(node, "stroke").as_deref(),
                Some("blue")
            );
        }
    }
}

#[test]
fn c4_node_and_label_rules_prove_actual_terminals_and_preserve_unsupported_facets() {
    let styles = [
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#112233").unwrap())
                .with_stroke(CanvasPaint::solid("#223344").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::NodeLabel,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ddeeff").unwrap()),
        ),
    ];
    let theme = c4_cluster_rules_theme(styles);
    let rendered =
        try_render_c4_rendered_with_theme("C4Context\nSystem(a, \"A\")\n", &theme).unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let label = document
        .descendants()
        .find(|node| node.has_tag_name("text") && svg_text_content(*node) == "A")
        .unwrap();
    assert_eq!(
        c4_effective_property(label, "fill").as_deref(),
        Some("#ddeeff")
    );
    assert_eq!(
        c4_node_body_property(label, "fill").as_deref(),
        Some("#112233")
    );
    assert_eq!(
        c4_node_body_property(label, "stroke").as_deref(),
        Some("#223344")
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
    for target in [ThemeTarget::Node, ThemeTarget::NodeLabel] {
        let mixed = c4_cluster_rules_theme([ThemeRule::new(
            target,
            ThemeStylePatch {
                paint: merman_render::diagram_theme::ThemePaintPatch {
                    fill: Specified::Value(CanvasPaint::solid("#112233").unwrap()),
                    opacity: Specified::Value(0.5),
                    ..Default::default()
                },
                ..ThemeStylePatch::default()
            },
        )]);
        assert!(
            try_render_c4_rendered_with_theme("C4Context\nSystem(a, \"A\")\n", &mixed).is_err()
        );
    }
}

fn c4_node_body_property(node: roxmltree::Node<'_, '_>, property: &str) -> Option<String> {
    node.ancestors()
        .find(|node| {
            node.attribute("class")
                .is_some_and(|classes| classes.split_whitespace().any(|class| class == "c4-shape"))
        })?
        .descendants()
        .filter(|node| {
            ["rect", "path", "polygon", "circle", "ellipse"].contains(&node.tag_name().name())
        })
        .find_map(|node| c4_effective_property(node, property))
}

#[test]
fn c4_node_receipts_track_layout_occurrences_after_alias_updates() {
    let source =
        "C4Context\nSystem(a, \"A\")\nSystem(b, \"B\")\nUpdateElementStyle(a, $alias=\"b\")\n";
    for target in [ThemeTarget::Node, ThemeTarget::NodeLabel] {
        let theme = c4_cluster_rules_theme([ThemeRule::new(
            target,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        )]);
        let rendered = try_render_c4_rendered_with_theme(source, &theme)
            .expect("each layout occurrence remains renderable after an alias collision");
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            document
                .descendants()
                .filter(|node| { node.has_tag_name("text") && svg_text_content(*node) == "B" })
                .count(),
            2
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn c4_text_receipts_track_each_relationship_label_occurrence() {
    let source = "C4Context\nSystem(a, \"A\")\nSystem(b, \"B\")\nSystem(c, \"C\")\nUpdateElementStyle(a, $fontColor=\"red\")\nUpdateElementStyle(b, $fontColor=\"blue\")\nUpdateElementStyle(c, $fontColor=\"green\")\nRel(a, c, \"\")\nRel(a, b, \"Calls\")\nUpdateRelStyle(a, c, $to=\"b\")\n";
    let theme = c4_cluster_rules_theme([ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
    )]);
    let rendered = try_render_c4_rendered_with_theme(source, &theme).unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let label = document
        .descendants()
        .find(|node| node.has_tag_name("text") && svg_text_content(*node) == "Calls")
        .unwrap();
    assert_eq!(label.attribute("fill"), Some("#123456"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn c4_boundary_text_receipts_track_layout_occurrences_after_alias_updates() {
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
    for target in [ThemeTarget::Text, ThemeTarget::ClusterLabel] {
        let theme = c4_cluster_rules_theme([ThemeRule::new(
            target,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        )]);
        let rendered = try_render_c4_rendered_with_theme(source, &theme)
            .expect("boundary text proof follows layout occurrences after an alias collision");
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
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

#[test]
fn c4_empty_and_typography_themes_keep_source_relationship_markers_shared() {
    let source = "C4Context\nSystem(a, \"A\")\nSystem(b, \"B\")\nBiRel(a, b, \"Calls\", \"HTTPS\")\nUpdateRelStyle(a, b, $textColor=\"red\", $lineColor=\"blue\")\n";
    let typography = TypographySpec::default().with_family_style(
        merman_render::DiagramFamilyId::C4,
        ThemeTextStyle::default().with_font_stack(FontStack::single("Inter").unwrap()),
    );
    let text_only =
        DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        )));
    for spec in [
        DiagramThemeSpec::new(),
        DiagramThemeSpec::new().with_typography(typography),
        text_only,
    ] {
        let theme = DiagramThemeCompiler::new().compile(spec).unwrap();
        let rendered = try_render_c4_rendered_with_theme(source, &theme).unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_c4_shared_relationship_markers(&document);
    }
}

#[test]
fn c4_requested_edge_and_marker_paints_keep_source_color_precedence() {
    let source = "C4Context\nSystem(a, \"A\")\nSystem(b, \"B\")\nBiRel(a, b, \"Calls\", \"HTTPS\")\nUpdateRelStyle(a, b, $textColor=\"red\", $lineColor=\"blue\")\n";
    for target in [ThemeTarget::Edge, ThemeTarget::Marker] {
        let theme = c4_cluster_rules_theme([ThemeRule::new(
            target,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#123456").unwrap()),
        )]);
        let rendered = try_render_c4_rendered_with_theme(source, &theme).unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let line = document
            .descendants()
            .find(|node| node.has_tag_name("line"))
            .unwrap();
        assert_eq!(line.attribute("stroke"), Some("blue"));
        for attribute in ["marker-start", "marker-end"] {
            let id = line
                .attribute(attribute)
                .unwrap()
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let path = document
                .descendants()
                .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(id))
                .unwrap()
                .children()
                .find(|node| node.has_tag_name("path"))
                .unwrap();
            assert_eq!(path.attribute("fill"), Some("blue"), "{target:?}");
            assert_eq!(path.attribute("stroke"), Some("blue"), "{target:?}");
        }
    }
}

#[test]
fn c4_host_presets_reach_boundary_relation_and_marker_paint() {
    use merman_render::diagram_theme::ThemePreset;
    let source = "C4Context\nBoundary(boundary, \"Boundary\") {\nSystem(a, \"A\")\n}\nSystem(b, \"B\")\nBiRel(a, b, \"Calls\", \"HTTPS\")\n";
    for (preset, text, line) in [
        (ThemePreset::EditorLight, "#0f172a", "#64748b"),
        (ThemePreset::EditorDark, "#e5e7eb", "#94a3b8"),
    ] {
        let svg = render_c4_with_preset(source, preset, MermaidEngine::new());
        let document = roxmltree::Document::parse(&svg).unwrap();
        for label in ["Boundary", "Calls", "[HTTPS]"] {
            let node = document
                .descendants()
                .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
                .unwrap();
            assert_eq!(node.attribute("fill"), Some(text), "{preset:?} {label}");
        }
        let relation = document
            .descendants()
            .find(|node| node.has_tag_name("line") && node.attribute("marker-end").is_some())
            .unwrap();
        assert_eq!(relation.attribute("stroke"), Some(line), "{preset:?}");
        for marker in ["marker-start", "marker-end"] {
            let id = relation
                .attribute(marker)
                .unwrap()
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let marker = document
                .descendants()
                .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(id))
                .unwrap();
            let path = marker
                .children()
                .find(|node| node.has_tag_name("path"))
                .unwrap();
            assert_eq!(path.attribute("fill"), Some(line), "{preset:?} marker fill");
            assert_eq!(
                path.attribute("stroke"),
                Some(line),
                "{preset:?} marker stroke"
            );
        }
    }
}

#[test]
fn c4_host_relation_paint_preserves_explicit_source_and_site_owners() {
    use merman_render::diagram_theme::ThemePreset;
    let body = "---\nconfig: {theme: base}\n---\nC4Context\nBoundary(boundary, \"Boundary\") {\nSystem(a, \"A\")\n}\nSystem(b, \"B\")\nBiRel(a, b, \"Calls\", \"HTTPS\")\n";
    for source_owned in [false, true] {
        let source = if source_owned {
            format!("{body}UpdateRelStyle(a, b, $textColor=\"red\", $lineColor=\"blue\")\n")
        } else {
            body.to_owned()
        };
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "c4": {
                    "boundary_bg_color": "#334455", "boundary_border_color": "#667788",
                    "boundary_font_color": "green", "relationship_text_color": "orange",
                    "relationship_line_color": "purple"
                }
            })),
        );
        let svg = render_c4_with_preset(&source, ThemePreset::EditorDark, engine);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let text = if source_owned { "red" } else { "orange" };
        let line = if source_owned { "blue" } else { "purple" };
        for (label, fill) in [("Boundary", "green"), ("Calls", text), ("[HTTPS]", text)] {
            let node = document
                .descendants()
                .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
                .unwrap();
            assert_eq!(node.attribute("fill"), Some(fill));
        }
        assert_eq!(
            c4_boundary_fill(&document, "Boundary").as_deref(),
            Some("#334455")
        );
        assert_eq!(
            c4_effective_property(c4_rect_labeled(&document, "Boundary"), "stroke").as_deref(),
            Some("#667788")
        );
        let relation = document
            .descendants()
            .find(|node| node.has_tag_name("line") && node.attribute("marker-end").is_some())
            .unwrap();
        assert_eq!(relation.attribute("stroke"), Some(line));
        for attribute in ["marker-start", "marker-end"] {
            let marker_id = relation
                .attribute(attribute)
                .unwrap()
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let marker = document
                .descendants()
                .find(|node| node.attribute("id") == Some(marker_id))
                .unwrap();
            let path = marker
                .children()
                .find(|node| node.has_tag_name("path"))
                .unwrap();
            assert_eq!(path.attribute("fill"), Some(line));
            assert_eq!(path.attribute("stroke"), Some(line));
        }
    }
}

#[test]
fn c4_semantic_role_paints_are_proved_at_their_terminals() {
    let theme = c4_cluster_rules_theme([
        ThemeRule::new(
            ThemeTarget::ClusterLabel,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#112233").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::EdgeLabel,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#223344").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#334455").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::Marker,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#445566").unwrap())
                .with_stroke(CanvasPaint::solid("#556677").unwrap()),
        ),
    ]);
    let body = "C4Context\nBoundary(boundary, \"Boundary\", \"\") {\nSystem(a, \"A\")\n}\nSystem(b, \"B\")\nBiRel(a, b, \"Calls\", \"HTTPS\")\n";
    for source_owned in [false, true] {
        let source = if source_owned {
            format!(
                "{body}UpdateElementStyle(boundary, $fontColor=\"green\")\nUpdateRelStyle(a, b, $textColor=\"red\", $lineColor=\"blue\")\n"
            )
        } else {
            body.to_owned()
        };
        let rendered = try_render_c4_rendered_with_theme(&source, &theme).unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for (label, fill) in [
            ("Boundary", if source_owned { "green" } else { "#112233" }),
            ("Calls", if source_owned { "red" } else { "#223344" }),
            ("[HTTPS]", if source_owned { "red" } else { "#223344" }),
        ] {
            let text = document
                .descendants()
                .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
                .unwrap();
            assert_eq!(text.attribute("fill"), Some(fill));
        }
        let relation = document
            .descendants()
            .find(|node| node.has_tag_name("line") && node.attribute("marker-end").is_some())
            .unwrap();
        assert_eq!(
            relation.attribute("stroke"),
            Some(if source_owned { "blue" } else { "#334455" })
        );
        for marker in ["marker-start", "marker-end"] {
            let id = relation
                .attribute(marker)
                .unwrap()
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let path = document
                .descendants()
                .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(id))
                .unwrap()
                .children()
                .find(|node| node.has_tag_name("path"))
                .unwrap();
            assert_eq!(
                path.attribute("fill"),
                Some(if source_owned { "blue" } else { "#445566" })
            );
            assert_eq!(
                path.attribute("stroke"),
                Some(if source_owned { "blue" } else { "#556677" })
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 4);
        assert_eq!(evidence.accounted_count(), 4);
        assert_eq!(evidence.applied_count(), if source_owned { 0 } else { 4 });
        assert_eq!(
            evidence.not_applicable_count(),
            if source_owned { 4 } else { 0 }
        );
        assert_eq!(evidence.theme_residual_count(), 0);
    }
    let rendered =
        try_render_c4_rendered_with_theme("C4Context\nSystem(a, \"A\")\n", &theme).unwrap();
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.not_applicable_count(), 4);
    assert_eq!(evidence.applied_count(), 0);
}

#[test]
fn c4_unconsumed_palette_variables_do_not_own_native_role_defaults() {
    let body = "C4Context\nBoundary(boundary, \"Boundary\", \"\") {\nSystem(a, \"A\")\n}\nSystem(b, \"B\")\nBiRel(a, b, \"Calls\")\n";
    let empty_theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    for (prefix, site_config) in [
        ("---\nconfig: {theme: base}\n---\n", serde_json::json!({})),
        ("", serde_json::json!({"theme": "base"})),
        (
            "",
            serde_json::json!({"themeVariables": {
                "textColor": "#abcdef", "lineColor": "#fedcba", "clusterBkg": "#123456",
                "clusterBorder": "#654321", "personBkg": "#224466", "personBorder": "#446688"
            }}),
        ),
    ] {
        let source = format!("{prefix}{body}");
        let engine = || {
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    site_config.clone(),
                ),
            )
        };
        let svg = render_c4_with_preset(
            &source,
            merman_render::diagram_theme::ThemePreset::EditorDark,
            engine(),
        );
        let baseline = try_render_c4_rendered_with_engine(&source, &empty_theme, engine()).unwrap();
        for (svg, typed) in [(svg.as_str(), true), (baseline.svg(), false)] {
            let document = roxmltree::Document::parse(svg).unwrap();
            for label in ["Boundary", "Calls"] {
                let text = document
                    .descendants()
                    .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
                    .unwrap();
                assert_eq!(
                    text.attribute("fill"),
                    Some(if typed { "#e5e7eb" } else { "#444444" })
                );
            }
            for label in ["A", "B"] {
                let text = document
                    .descendants()
                    .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
                    .unwrap();
                assert_eq!(
                    c4_effective_property(text, "fill").as_deref(),
                    Some(if typed { "#e5e7eb" } else { "#FFFFFF" })
                );
                assert_eq!(
                    c4_node_body_property(text, "fill").as_deref(),
                    Some(if typed { "#111827" } else { "#1168BD" })
                );
                assert_eq!(
                    c4_node_body_property(text, "stroke").as_deref(),
                    Some(if typed { "#475569" } else { "#3C7FC0" })
                );
            }
            assert_eq!(
                c4_boundary_fill(&document, "Boundary").as_deref(),
                Some(if typed { "#1e293b" } else { "none" })
            );
            assert_eq!(
                c4_effective_property(c4_rect_labeled(&document, "Boundary"), "stroke").as_deref(),
                Some(if typed { "#475569" } else { "#444444" })
            );
            let line = document
                .descendants()
                .find(|node| node.has_tag_name("line") && node.attribute("marker-end").is_some())
                .unwrap();
            assert_eq!(
                line.attribute("stroke"),
                Some(if typed { "#94a3b8" } else { "#444444" })
            );
            if typed {
                for attribute in ["marker-start", "marker-end"] {
                    let marker_id = line
                        .attribute(attribute)
                        .unwrap()
                        .strip_prefix("url(#")
                        .unwrap()
                        .strip_suffix(')')
                        .unwrap();
                    let path = document
                        .descendants()
                        .find(|node| node.attribute("id") == Some(marker_id))
                        .unwrap()
                        .children()
                        .find(|node| node.has_tag_name("path"))
                        .unwrap();
                    assert_eq!(path.attribute("fill"), Some("#94a3b8"));
                    assert_eq!(path.attribute("stroke"), Some("#94a3b8"));
                }
            }
            if site_config.get("themeVariables").is_some() {
                assert!(svg.contains("stroke:#446688;fill:#224466;"));
            }
        }
    }
}

#[test]
fn c4_unconsumed_palette_variables_do_not_certify_unsupported_role_facets() {
    let source = "C4Context\nBoundary(boundary, \"Boundary\") {\nSystem(a, \"A\")\n}\nSystem(b, \"B\")\nBiRel(a, b, \"Calls\")\n";
    for target in [
        ThemeTarget::Edge,
        ThemeTarget::Marker,
        ThemeTarget::EdgeLabel,
        ThemeTarget::ClusterLabel,
        ThemeTarget::Cluster,
    ] {
        let mut style = if target == ThemeTarget::Edge {
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#112233").unwrap())
        } else {
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#112233").unwrap())
        };
        style.paint.opacity = Specified::Value(0.5);
        let theme = c4_cluster_rules_theme([ThemeRule::new(target, style)]);
        let engine = MermaidEngine::new().with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": {"textColor": "#abcdef", "lineColor": "#fedcba", "clusterBkg": "#123456", "clusterBorder": "#654321"}
        })));
        let error = try_render_c4_rendered_with_engine(source, &theme, engine)
            .err()
            .expect("unconsumed palette variables must not hide an unsupported winning facet");
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::C4, 1)),
            "{target:?}"
        );
    }
}

fn c4_cluster_radius_theme(radius: Specified<f32>) -> DiagramTheme {
    let style = ThemeStylePatch {
        geometry: ThemeGeometryPatch { radius },
        ..ThemeStylePatch::default()
    };
    c4_cluster_rules_theme([
        ThemeRule::new(ThemeTarget::Cluster, style).for_family(merman_render::DiagramFamilyId::C4)
    ])
}

fn c4_cluster_paint_theme(fill: CanvasPaint, stroke: CanvasPaint) -> DiagramTheme {
    c4_cluster_rules_theme([ThemeRule::new(
        ThemeTarget::Cluster,
        ThemeStylePatch::default()
            .with_fill(fill)
            .with_stroke(stroke),
    )
    .for_family(merman_render::DiagramFamilyId::C4)])
}

fn c4_cluster_fill_with_ordinal_palette_theme(fill: CanvasPaint) -> DiagramTheme {
    let styles = ThemeRuleSet::default()
        .with_rule(
            ThemeRule::new(
                ThemeTarget::Cluster,
                ThemeStylePatch::default().with_fill(fill),
            )
            .for_family(merman_render::DiagramFamilyId::C4),
        )
        .with_ordinal_palette(
            ThemeTarget::Cluster,
            OrdinalPalette::new([
                ThemeColorValue::parse("#123456").expect("valid C4 Cluster palette color")
            ])
            .expect("non-empty C4 Cluster palette"),
        );
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile C4 Cluster fill and ordinal palette theme")
}

fn c4_typography_theme(font_stack: FontStack, font_size_px: f32) -> DiagramTheme {
    let typography = ThemeTextStyle::default()
        .with_font_stack(font_stack)
        .with_font_size_px(font_size_px)
        .expect("valid C4 typography");
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default()
                    .with_family_style(merman_render::DiagramFamilyId::C4, typography),
            ),
        )
        .expect("compile C4 typography theme")
}

fn try_render_c4_rendered_with_theme(
    source: &str,
    theme: &DiagramTheme,
) -> merman_render::Result<family::RenderedFamilySvg> {
    try_render_c4_rendered_with_engine(source, theme, MermaidEngine::new())
}

fn try_render_c4_rendered_with_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
) -> merman_render::Result<family::RenderedFamilySvg> {
    try_render_c4_rendered_with_engine_requirement(
        source,
        theme,
        engine,
        ThemePortabilityRequirement::RequirePortable,
    )
}

fn try_render_c4_rendered_with_engine_requirement(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = common::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed C4 diagram")
        .expect("detect themed C4 diagram")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed C4 session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?;

    artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
}

fn try_render_c4_svg_with_theme(
    source: &str,
    theme: &DiagramTheme,
) -> merman_render::Result<String> {
    Ok(try_render_c4_rendered_with_theme(source, theme)?
        .svg()
        .to_owned())
}

fn render_c4_svg_with_theme(source: &str, theme: &DiagramTheme) -> String {
    try_render_c4_svg_with_theme(source, theme).expect("render themed C4 SVG")
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

fn c4_boundary_fill(document: &roxmltree::Document<'_>, label: &str) -> Option<String> {
    let outline = c4_rect_labeled(document, label);
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class") == Some("c4-boundary-background")
                && ["x", "y", "width", "height"]
                    .into_iter()
                    .all(|attribute| node.attribute(attribute) == outline.attribute(attribute))
        })
        .unwrap_or(outline)
        .attribute("fill")
        .map(str::to_owned)
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

#[test]
fn c4_cluster_solid_paint_reaches_explicit_solid_and_dashed_boundaries_only() {
    let source = r#"C4Context
Node(solid, "Solid Boundary") {
  System(service, "Service")
}
Boundary(dashed, "Dashed Boundary") {
  System(worker, "Worker")
}
"#;
    let theme = c4_cluster_paint_theme(
        CanvasPaint::solid("#ef4444").expect("valid C4 Cluster fill"),
        CanvasPaint::solid("#2563eb").expect("valid C4 Cluster stroke"),
    );
    let svg = render_c4_svg_with_theme(source, &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid themed C4 SVG XML");

    for label in ["Solid Boundary", "Dashed Boundary"] {
        let boundary = c4_rect_labeled(&document, label);
        assert_eq!(
            c4_boundary_fill(&document, label).as_deref(),
            Some("#ef4444"),
            "{label}"
        );
        assert_eq!(boundary.attribute("stroke"), Some("#2563eb"), "{label}");
    }

    let ordinary_element = c4_rect_labeled(&document, "Service");
    assert_eq!(
        c4_effective_property(ordinary_element, "fill"),
        Some("#1168BD".to_owned())
    );
    assert_eq!(
        c4_effective_property(ordinary_element, "stroke"),
        Some("#3C7FC0".to_owned())
    );
}

#[test]
fn c4_cluster_transparent_paint_reaches_terminal_boundary_attributes() {
    let source = r#"C4Context
Boundary(boundary, "Boundary") {
  System(service, "Service")
}
"#;
    let theme = c4_cluster_paint_theme(CanvasPaint::Transparent, CanvasPaint::Transparent);
    let svg = render_c4_svg_with_theme(source, &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid themed C4 SVG XML");

    let boundary = c4_rect_labeled(&document, "Boundary");
    assert_eq!(boundary.attribute("fill"), Some("transparent"));
    assert_eq!(boundary.attribute("stroke"), Some("transparent"));
    let ordinary_element = c4_rect_labeled(&document, "Service");
    assert_eq!(
        c4_effective_property(ordinary_element, "fill"),
        Some("#1168BD".to_owned())
    );
    assert_eq!(
        c4_effective_property(ordinary_element, "stroke"),
        Some("#3C7FC0".to_owned())
    );
}

#[test]
fn c4_cluster_source_paint_outranks_typed_values_per_facet() {
    let source = r##"C4Context
Boundary(fill_owned, "Source Fill") {
  System(fill_child, "Fill Child")
}
Boundary(stroke_owned, "Source Stroke") {
  System(stroke_child, "Stroke Child")
}
UpdateElementStyle(fill_owned, $bgColor="#22c55e")
UpdateElementStyle(stroke_owned, $borderColor="#0f766e")
"##;
    let theme = c4_cluster_paint_theme(
        CanvasPaint::solid("#ef4444").expect("valid C4 Cluster fill"),
        CanvasPaint::solid("#2563eb").expect("valid C4 Cluster stroke"),
    );
    let svg = render_c4_svg_with_theme(source, &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid themed C4 SVG XML");

    let source_fill = c4_rect_labeled(&document, "Source Fill");
    assert_eq!(
        c4_boundary_fill(&document, "Source Fill").as_deref(),
        Some("#22c55e")
    );
    assert_eq!(source_fill.attribute("stroke"), Some("#2563eb"));

    let source_stroke = c4_rect_labeled(&document, "Source Stroke");
    assert_eq!(
        c4_boundary_fill(&document, "Source Stroke").as_deref(),
        Some("#ef4444")
    );
    assert_eq!(source_stroke.attribute("stroke"), Some("#0f766e"));
}

#[test]
fn c4_cluster_paint_is_not_applicable_when_source_owns_both_facets() {
    let source = r##"C4Context
Boundary(boundary, "Source Owned") {
  System(service, "Service")
}
UpdateElementStyle(boundary, $bgColor="#22c55e", $borderColor="#0f766e")
"##;
    let theme = c4_cluster_paint_theme(
        CanvasPaint::solid("#ef4444").expect("valid suppressed C4 Cluster fill"),
        CanvasPaint::solid("#2563eb").expect("valid suppressed C4 Cluster stroke"),
    );
    let svg = render_c4_svg_with_theme(source, &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid source-owned C4 SVG XML");
    let boundary = c4_rect_labeled(&document, "Source Owned");

    assert_eq!(
        c4_boundary_fill(&document, "Source Owned").as_deref(),
        Some("#22c55e")
    );
    assert_eq!(boundary.attribute("stroke"), Some("#0f766e"));
    assert!(!svg.contains("#ef4444"));
    assert!(!svg.contains("#2563eb"));
}

#[test]
fn c4_cluster_ordinal_palette_is_not_applicable_when_typed_fill_wins() {
    let source = r#"C4Context
Boundary(boundary, "Boundary") {
  System(service, "Service")
}
"#;
    let theme = c4_cluster_fill_with_ordinal_palette_theme(
        CanvasPaint::solid("#ef4444").expect("valid C4 Cluster fill"),
    );
    let svg = render_c4_svg_with_theme(source, &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid themed C4 SVG XML");
    assert_eq!(
        c4_boundary_fill(&document, "Boundary").as_deref(),
        Some("#ef4444")
    );
}

#[test]
fn c4_text_fill_reaches_unowned_labels_and_preserves_authored_colors() {
    let source = r##"---
title: C4 paint
---
C4Context
Boundary(boundary, "Boundary", "") {
  System(a, "First")
  System(b, "Second")
}
Rel(a, b, "Connects")
UpdateElementStyle(a, $fontColor="#135790")
UpdateElementStyle(b, $fontColor="#246801")
UpdateRelStyle(a, b, $textColor="#975310")
"##;
    for variant in [None, Some(ThemeVariant::Default)] {
        for (paint, expected) in [
            (
                CanvasPaint::solid("#123456").expect("valid paint"),
                "#123456",
            ),
            (CanvasPaint::Transparent, "transparent"),
        ] {
            let mut rule = ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default().with_fill(paint),
            )
            .for_family(merman_render::DiagramFamilyId::C4);
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            let theme = c4_cluster_rules_theme([rule]);
            let rendered = try_render_c4_rendered_with_theme(source, &theme)
                .expect("C4 Text.fill must have portable terminal evidence");
            let document = roxmltree::Document::parse(rendered.svg()).expect("valid SVG");
            let title = document
                .descendants()
                .find(|node| node.has_tag_name("text") && svg_text_content(*node) == "C4 paint")
                .expect("visible title");
            assert_eq!(title.attribute("fill"), Some(expected));
            for (label, color) in [
                ("First", "#135790"),
                ("Second", "#246801"),
                ("Boundary", expected),
                ("Connects", "#975310"),
            ] {
                let text = document
                    .descendants()
                    .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
                    .unwrap_or_else(|| panic!("missing independently colored C4 label {label}"));
                assert_eq!(c4_effective_property(text, "fill").as_deref(), Some(color));
            }
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.accounted_count(), 1);
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.not_applicable_count(), 0);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn c4_text_fill_reaches_untitled_nodes_but_preserves_explicit_text_color() {
    let spec = DiagramThemeSpec::new().with_styles(
        ThemeRuleSet::default().with_rule(
            ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#123456").expect("valid paint")),
            )
            .for_family(merman_render::DiagramFamilyId::C4),
        ),
    );
    let theme = DiagramThemeCompiler::new()
        .compile(spec.clone())
        .expect("typed text theme");
    let owned_engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(
            serde_json::json!({"themeVariables": {"textColor": "#abcdef"}}),
        ),
    );
    for (source, engine, config_owned) in [
        (
            "C4Context\nSystem(a, \"Service\")\n",
            MermaidEngine::new(),
            false,
        ),
        (
            "---\ntitle: Owned title\n---\nC4Context\nSystem(a, \"Service\")\n",
            owned_engine,
            true,
        ),
    ] {
        let rendered = try_render_c4_rendered_with_engine(source, &theme, engine)
            .expect("node labels and source-owned title are portable");
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid SVG");
        let service = document
            .descendants()
            .find(|node| node.has_tag_name("text") && svg_text_content(*node) == "Service")
            .expect("visible node label");
        assert_eq!(
            c4_effective_property(service, "fill").as_deref(),
            Some(if config_owned { "#FFFFFF" } else { "#123456" })
        );
        let title = document
            .descendants()
            .find(|node| node.has_tag_name("text") && svg_text_content(*node) == "Owned title");
        assert_eq!(title.is_some(), config_owned);
        if let Some(title) = title {
            assert_eq!(c4_effective_property(title, "fill"), None);
            let root = document.root_element();
            let root_selector = format!("#{}", root.attribute("id").expect("SVG root id"));
            let style = root
                .children()
                .find(|node| node.has_tag_name("style"))
                .expect("root stylesheet");
            let css = svg_text_content(style);
            let root_fills = css
                .split('}')
                .filter_map(|rule| rule.split_once('{'))
                .filter(|(selector, _)| selector.trim() == root_selector)
                .flat_map(|(_, declarations)| declarations.split(';'))
                .filter_map(|declaration| declaration.split_once(':'))
                .filter_map(|(property, value)| (property.trim() == "fill").then_some(value.trim()))
                .collect::<Vec<_>>();
            assert_eq!(root_fills, ["#abcdef"]);
        }
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.not_applicable_count(), usize::from(config_owned));
        assert_eq!(evidence.applied_count(), usize::from(!config_owned));
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn c4_boundary_only_text_keeps_stroke_and_effect_applicable() {
    use merman_render::diagram_theme::{
        DiagramEffectSet, EffectBinding, EffectGraph, EffectInput, EffectPrimitive,
    };

    let paint = CanvasPaint::solid("#123456").expect("valid text paint");
    let fill = c4_cluster_rules_theme([ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(paint.clone()),
    )
    .for_family(merman_render::DiagramFamilyId::C4)]);
    let stroke = c4_cluster_rules_theme([ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_stroke(paint),
    )
    .for_family(merman_render::DiagramFamilyId::C4)]);
    let effects = DiagramEffectSet::default()
        .with_graph(
            EffectGraph::new(
                "blur",
                [EffectPrimitive::GaussianBlur {
                    input: EffectInput::SourceGraphic,
                    std_deviation: 1.0,
                }],
            )
            .expect("valid text effect graph"),
        )
        .expect("insert text effect graph")
        .with_binding(EffectBinding::new(ThemeTarget::Text, "blur").expect("valid text binding"))
        .expect("insert text binding");
    let effect = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_effects(effects))
        .expect("compile text effect theme");

    for label in ["Visible", ""] {
        let source = format!(
            "C4Context\nBoundary(b, \"{label}\", \"\") {{\n  UpdateLayoutConfig(3, 2)\n}}\n"
        );
        for (theme, inherited_fill_only) in [(&fill, true), (&stroke, false), (&effect, false)] {
            let result = try_render_c4_rendered_with_theme(&source, theme);
            if !label.is_empty() && !inherited_fill_only {
                let error = match result {
                    Ok(_) => panic!("visible boundary text retains unsupported stroke or effect"),
                    Err(error) => error,
                };
                assert_eq!(
                    error.unverified_family_theme(),
                    Some((merman_render::DiagramFamilyId::C4, 1))
                );
                continue;
            }
            let rendered = result.expect("owned fill or an empty text domain is portable");
            let document = roxmltree::Document::parse(rendered.svg()).expect("valid boundary SVG");
            let texts = document
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .filter(|node| !svg_text_content(*node).trim().is_empty())
                .collect::<Vec<_>>();
            assert_eq!(texts.len(), usize::from(!label.is_empty()));
            if let Some(text) = texts.first() {
                assert_eq!(svg_text_content(*text), "Visible");
                assert_eq!(
                    c4_effective_property(*text, "fill").as_deref(),
                    Some("#123456")
                );
            }
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.accounted_count(), 1);
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(label.is_empty())
            );
            assert_eq!(evidence.applied_count(), usize::from(!label.is_empty()));
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn c4_legacy_break_only_labels_do_not_certify_visible_role_paint() {
    for wrap in [false, true] {
        for br in [
            "<br/>",
            "</br>",
            "<BR />",
            "<br\u{00a0}/>",
            "</BR\u{feff}/>",
        ] {
            for target in [ThemeTarget::EdgeLabel, ThemeTarget::ClusterLabel] {
                let body = if target == ThemeTarget::EdgeLabel {
                    format!("System(a, \"Alpha\")\nSystem(b, \"Beta\")\nRel(a, b, \"{br}\")\n")
                } else {
                    format!("Boundary(group, \"{br}\", \"\") {{\nSystem(a, \"Alpha\")\n}}\n")
                };
                let source = format!(
                    "---\nconfig: {{wrap: {wrap}, c4: {{wrap: {wrap}}}}}\n---\nC4Context\n{body}"
                );
                let theme = c4_cluster_rules_theme([ThemeRule::new(
                    target,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                )]);
                let rendered = try_render_c4_rendered_with_theme(&source, &theme)
                    .expect("an empty legacy role remains portable");
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                assert!(
                    document
                        .descendants()
                        .filter(|node| {
                            node.has_tag_name("text")
                                && !node.ancestors().any(|ancestor| {
                                    ancestor.attribute("class").is_some_and(|classes| {
                                        classes
                                            .split_ascii_whitespace()
                                            .any(|class| class == "c4-shape")
                                    })
                                })
                        })
                        .all(|node| svg_text_content(node).trim().is_empty()),
                    "wrap={wrap}, {target:?}, {br:?}"
                );
                let completion = rendered.into_completion();
                let evidence = merman_render::__private::family_evidence(completion.report());
                assert_eq!(evidence.required_count(), 1);
                assert_eq!(evidence.accounted_count(), 1);
                assert_eq!(evidence.applied_count(), 0, "{target:?}: {br:?}");
                assert_eq!(evidence.not_applicable_count(), 1);
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn c4_role_paint_receipts_separate_zero_size_glyphs_from_dom_occurrences() {
    for (role, overrides, root_size, body, visible, has_shapes) in [
        (
            ThemeTarget::ClusterLabel,
            serde_json::json!({"boundaryFontSize":0}),
            29,
            "Boundary(b, \"\", \"Type\") {\nSystem(a, \"A\")\n}\n",
            false,
            true,
        ),
        (
            ThemeTarget::ClusterLabel,
            serde_json::json!({"boundaryFontSize":14}),
            29,
            "Boundary(b, \"\", \"Type\") {\nSystem(a, \"A\")\n}\n",
            true,
            true,
        ),
        (
            ThemeTarget::ClusterLabel,
            serde_json::json!({"boundaryFontSize":0}),
            29,
            "Boundary(b, \"Name\", \"Type\") {\nSystem(a, \"A\")\n}\n",
            true,
            true,
        ),
        (
            ThemeTarget::ClusterLabel,
            serde_json::json!({"boundaryFontSize":0}),
            29,
            "Deployment_Node(b, \"\", \"Type\", \"Description\") {\nSystem(a, \"A\")\n}\n",
            true,
            true,
        ),
        (
            ThemeTarget::ClusterLabel,
            serde_json::json!({"boundaryFontSize":2}),
            29,
            "Deployment_Node(b, \"\", \"\", \"Description\") {\nSystem(a, \"A\")\n}\n",
            false,
            true,
        ),
        (
            ThemeTarget::EdgeLabel,
            serde_json::json!({"messageFontSize":0}),
            29,
            "System(a, \"A\")\nSystem(b, \"B\")\nRel(a, b, \"Calls\", \"HTTPS\")\n",
            false,
            true,
        ),
        (
            ThemeTarget::EdgeLabel,
            serde_json::json!({"messageFontSize":12}),
            29,
            "System(a, \"A\")\nSystem(b, \"B\")\nRel(a, b, \"Calls\", \"HTTPS\")\n",
            true,
            true,
        ),
        (
            ThemeTarget::EdgeLabel,
            serde_json::json!({"messageFontSize":-1}),
            29,
            "System(a, \"A\")\nSystem(b, \"B\")\nRel(a, b, \"Calls\", \"HTTPS\")\n",
            true,
            true,
        ),
        (
            ThemeTarget::NodeLabel,
            serde_json::json!({"systemFontSize":"0px"}),
            29,
            "System(a, \"Name\", \"Description\")\n",
            false,
            true,
        ),
        (
            ThemeTarget::NodeLabel,
            serde_json::json!({"systemFontSize":14, "systemFontFamily":"var(--host-font)"}),
            29,
            "System(a, \"Name\", \"Description\")\n",
            true,
            true,
        ),
        (
            ThemeTarget::NodeLabel,
            serde_json::json!({"systemFontSize":-1}),
            29,
            "System(a, \"Name\", \"Description\")\n",
            true,
            true,
        ),
        (
            ThemeTarget::Title,
            serde_json::json!({}),
            0,
            "title Title\nUpdateLayoutConfig(3)\n",
            false,
            false,
        ),
        (
            ThemeTarget::Title,
            serde_json::json!({}),
            29,
            "title Title\nUpdateLayoutConfig(3)\n",
            true,
            false,
        ),
        (
            ThemeTarget::Title,
            serde_json::json!({}),
            -2,
            "title Title\nUpdateLayoutConfig(3)\n",
            true,
            false,
        ),
    ] {
        for target in [ThemeTarget::Text, role] {
            let theme = c4_cluster_rules_theme([
                ThemeRule::new(
                    target,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                ),
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#aabbcc").unwrap()),
                ),
            ]);
            let mut config = serde_json::json!({"themeVariables":{"fontSize":root_size}, "c4": {
                "wrap":false, "systemFontSize":"0px"
            }});
            config["c4"]
                .as_object_mut()
                .unwrap()
                .extend(overrides.as_object().unwrap().clone());
            let rendered = try_render_c4_rendered_with_engine(
                &format!("C4Context\n{body}"),
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(config.clone()),
                ),
            )
            .expect("every emitted occurrence completes even when its glyphs are zero-sized");
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert!(
                document.descendants().any(|node| {
                    node.has_tag_name("text")
                        && !svg_text_content(node).trim().is_empty()
                        && (c4_effective_property(node, "fill").as_deref() == Some("#123456")
                            || node
                                .attribute("style")
                                .is_some_and(|style| style.contains("#123456")))
                }),
                "zero-sized text still retains its DOM and paint declarations: {config}"
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            let applied = usize::from(visible) + usize::from(has_shapes);
            assert_eq!(evidence.required_count(), 2);
            assert_eq!(evidence.accounted_count(), 2);
            assert_eq!(evidence.applied_count(), applied, "{target:?}, {config}");
            assert_eq!(evidence.not_applicable_count(), 2 - applied);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn c4_visible_brackets_and_unified_literal_break_words_keep_role_paint() {
    for wrap in [false, true] {
        for br in ["<br/>", "</br>", "<br\u{00a0}/>", "</BR\u{feff}/>"] {
            for (target, body) in [
                (
                    ThemeTarget::EdgeLabel,
                    format!(
                        "System(a, \"Alpha\")\nSystem(b, \"Beta\")\nRel(a, b, \"\", \"{br}\")\n"
                    ),
                ),
                (
                    ThemeTarget::ClusterLabel,
                    format!("Boundary(group, \"\", \"{br}\") {{\nSystem(a, \"Alpha\")\n}}\n"),
                ),
            ] {
                let source = format!(
                    "---\nconfig: {{wrap: {wrap}, c4: {{wrap: {wrap}}}}}\n---\nC4Context\n{body}"
                );
                let theme = c4_cluster_rules_theme([ThemeRule::new(
                    target,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                )]);
                let rendered = try_render_c4_rendered_with_theme(&source, &theme)
                    .expect("generated brackets remain visible role consumers");
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let colored_rows: Vec<_> = document
                    .descendants()
                    .filter(|node| node.has_tag_name("text"))
                    .filter(|node| {
                        c4_effective_property(*node, "fill").as_deref() == Some("#123456")
                    })
                    .map(svg_text_content)
                    .filter(|text| !text.trim().is_empty())
                    .collect();
                assert_eq!(colored_rows, ["[", "]"], "wrap={wrap}, {target:?}: {br:?}");
                let completion = rendered.into_completion();
                let evidence = merman_render::__private::family_evidence(completion.report());
                assert_eq!(evidence.required_count(), 1);
                assert_eq!(evidence.accounted_count(), 1);
                assert_eq!(evidence.applied_count(), 1);
                assert_eq!(evidence.not_applicable_count(), 0);
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
        for (source_word, visible_word) in [
            ("</br>", "</br>"),
            ("</BR\u{feff}/>", "</BR\u{feff}/>"),
            ("&lt;br/&gt;", "<br/>"),
        ] {
            let source = format!(
                "---\nconfig: {{c4: {{wrap: {wrap}}}}}\n---\nC4Context\nSystem(a, \"{source_word}\")\n"
            );
            let theme = c4_cluster_rules_theme([ThemeRule::new(
                ThemeTarget::NodeLabel,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            )]);
            let rendered = try_render_c4_rendered_with_theme(&source, &theme)
                .expect("unified labels preserve literal closing and decoded break words");
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let name = document
                .descendants()
                .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("c4-name"))
                .expect("visible unified name section");
            let name_text = name
                .descendants()
                .find(|node| node.has_tag_name("text"))
                .unwrap();
            assert_eq!(svg_text_content(name_text), visible_word);
            assert_eq!(
                c4_effective_property(name_text, "fill").as_deref(),
                Some("#123456")
            );
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.not_applicable_count(), 0);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn c4_text_fill_does_not_certify_unsupported_siblings_or_source_inheritance() {
    let fill = CanvasPaint::solid("#123456").expect("valid paint");
    let mixed = c4_cluster_rules_theme([ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default()
            .with_fill(fill.clone())
            .with_stroke(fill.clone()),
    )
    .for_family(merman_render::DiagramFamilyId::C4)]);
    let title_source = "---\ntitle: C4 paint\n---\nC4Context\nSystem(a, \"Service\")\n";
    let error = try_render_c4_svg_with_theme(title_source, &mixed)
        .expect_err("a fill receipt cannot certify an unsupported stroke");
    assert_eq!(
        error.unverified_family_theme(),
        Some((merman_render::DiagramFamilyId::C4, 1))
    );

    let theme = c4_cluster_rules_theme([ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(fill),
    )
    .for_family(merman_render::DiagramFamilyId::C4)]);
    let source = "C4Context\nSystem(a, \"First\")\nSystem(b, \"Second\")\nRel(a, b, \"Connects\")\nUpdateRelStyle(a, b, $textColor=\"inherit\")\n";
    let error = try_render_c4_svg_with_theme(source, &mixed)
        .expect_err("mixed rules must not hide source-directed root inheritance");
    assert_eq!(
        error.unverified_family_theme(),
        Some((merman_render::DiagramFamilyId::C4, 1))
    );
    let error = try_render_c4_svg_with_theme(source, &theme)
        .expect_err("source-directed root inheritance is not an absent text terminal");
    assert_eq!(
        error.unverified_family_theme(),
        Some((merman_render::DiagramFamilyId::C4, 1))
    );
}

#[test]
fn c4_text_fallbacks_do_not_treat_source_inheritance_as_an_empty_domain() {
    use merman_render::diagram_theme::{
        DiagramEffectSet, EffectBinding, EffectGraph, EffectInput, EffectPrimitive,
    };
    let palette =
        DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_ordinal_palette(
            ThemeTarget::Text,
            OrdinalPalette::new([ThemeColorValue::parse("#123456").unwrap()]).unwrap(),
        ));
    let effects = DiagramEffectSet::default()
        .with_graph(
            EffectGraph::new(
                "blur",
                [EffectPrimitive::GaussianBlur {
                    input: EffectInput::SourceGraphic,
                    std_deviation: 1.0,
                }],
            )
            .unwrap(),
        )
        .unwrap()
        .with_binding(EffectBinding::new(ThemeTarget::Text, "blur").unwrap())
        .unwrap();
    let effect_theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_effects(effects.clone()))
        .unwrap();
    let mut clear = ThemeStylePatch::default();
    clear.effects.effect = Specified::Clear;
    let partially_cleared = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_effects(effects).with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(ThemeTarget::Text, clear)
                        .for_family(merman_render::DiagramFamilyId::C4)
                        .with_ordinal(OrdinalSelector::Exact(1)),
                ),
            ),
        )
        .unwrap();
    assert_eq!(
        try_render_c4_svg_with_theme("C4Context\nSystem(a, \"Visible\")\n", &partially_cleared)
            .expect_err("an unproved ordinal clear cannot mask the whole binding")
            .unverified_family_theme(),
        Some((merman_render::DiagramFamilyId::C4, 2))
    );
    {
        let source = "C4Context\nSystem(a, \"Visible\")\n";
        assert_eq!(
            try_render_c4_svg_with_theme(source, &effect_theme)
                .expect_err("fill ownership does not suppress an active effect")
                .unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::C4, 1))
        );
    }
    try_render_c4_svg_with_theme(
        "C4Context\nUpdateLayoutConfig($c4ShapeInRow=\"3\")\n",
        &effect_theme,
    )
    .expect("no text terminal has an active effect");
    {
        let theme = DiagramThemeCompiler::new().compile(palette).unwrap();
        let owned =
            "C4Context\nSystem(a, \"First\")\nSystem(b, \"Second\")\nRel(a, b, \"Connects\")\n";
        try_render_c4_svg_with_theme(owned, &theme)
            .expect("independently colored labels have no inherited text paint");
        let source = format!("{owned}UpdateRelStyle(a, b, $textColor=\"inherit\")\n");
        for label in ["", "<br/>"] {
            let empty_label = source.replace("\"Connects\"", &format!("\"{label}\""));
            try_render_c4_svg_with_theme(&empty_label, &theme)
                .expect("an empty relationship label does not inherit visible text paint");
        }
        let technology_only =
            source.replace("Rel(a, b, \"Connects\")", "Rel(a, b, \"\", \"HTTPS\")");
        assert_eq!(
            try_render_c4_svg_with_theme(&technology_only, &theme)
                .unwrap_err()
                .unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::C4, 1))
        );
        let error = try_render_c4_svg_with_theme(&source, &theme)
            .expect_err("inherited text cannot certify an empty fallback domain");
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::C4, 1))
        );
    }
}

#[test]
fn c4_owned_fill_does_not_hide_unsupported_text_stroke() {
    let paint = CanvasPaint::solid("#123456").unwrap();
    for patch in [
        ThemeStylePatch::default().with_stroke(paint.clone()),
        ThemeStylePatch::default()
            .with_fill(paint.clone())
            .with_stroke(paint),
    ] {
        let theme =
            c4_cluster_rules_theme([ThemeRule::new(ThemeTarget::Text, patch)
                .for_family(merman_render::DiagramFamilyId::C4)]);
        let error = try_render_c4_svg_with_theme("C4Context\nSystem(a, \"Visible\")\n", &theme)
            .expect_err("visible text retains an unsupported stroke even when fill is owned");
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::C4, 1))
        );
    }
}

#[test]
fn c4_text_palette_respects_the_fill_winner_and_host_color_owner() {
    let styles = ThemeRuleSet::default()
        .with_ordinal_palette(
            ThemeTarget::Text,
            OrdinalPalette::new([ThemeColorValue::parse("#654321").unwrap()]).unwrap(),
        )
        .with_rule(
            ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            )
            .for_family(merman_render::DiagramFamilyId::C4),
        );
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .unwrap();
    let source = "---\ntitle: Inherited paint\n---\nC4Context\nSystem(a, \"Service\")\n";
    for config_owned in [false, true] {
        let engine = if config_owned {
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"themeVariables": {"textColor": "#abcdef"}}),
                ),
            )
        } else {
            MermaidEngine::new()
        };
        let rendered = try_render_c4_rendered_with_engine(source, &theme, engine).unwrap();
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.applied_count(), usize::from(!config_owned));
        assert_eq!(
            evidence.not_applicable_count(),
            1 + usize::from(config_owned)
        );
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn c4_unproved_text_ordinals_do_not_certify_absence() {
    for ordinal in [
        OrdinalSelector::Exact(2),
        OrdinalSelector::Cycle {
            period: 2,
            offset: 1,
        },
    ] {
        let theme = c4_cluster_rules_theme([ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#123456").unwrap()),
        )
        .for_family(merman_render::DiagramFamilyId::C4)
        .with_ordinal(ordinal)]);
        let error = try_render_c4_svg_with_theme(
            "C4Context\nSystem(a, \"First\")\nSystem(b, \"Second\")\n",
            &theme,
        )
        .expect_err("unsupported ordinal domains cannot be proved empty from ordinal one");
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::C4, 1))
        );
        try_render_c4_svg_with_theme(
            "C4Context\nUpdateLayoutConfig($c4ShapeInRow=\"3\")\n",
            &theme,
        )
        .expect("a genuinely empty text domain is not applicable");
    }
}

#[test]
fn c4_text_ordinals_preserve_provable_global_winners() {
    let ordinal = ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ff0000").unwrap()),
    )
    .for_family(merman_render::DiagramFamilyId::C4)
    .with_ordinal(OrdinalSelector::Exact(2));
    let global = ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#0000ff").unwrap()),
    )
    .for_family(merman_render::DiagramFamilyId::C4);
    let source = "---\ntitle: Global winner\n---\nC4Context\nSystem(a, \"Visible\")\n";
    let covered = c4_cluster_rules_theme([ordinal.clone(), global.clone()]);
    let rendered = try_render_c4_rendered_with_theme(source, &covered)
        .expect("a later global winner covers any ordinal");
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    let unproved = c4_cluster_rules_theme([global, ordinal.clone()]);
    assert_eq!(
        try_render_c4_svg_with_theme(source, &unproved)
            .expect_err("a later ordinal remains unverified")
            .unverified_family_theme(),
        Some((merman_render::DiagramFamilyId::C4, 1))
    );

    let palette = OrdinalPalette::new([ThemeColorValue::parse("#123456").unwrap()]).unwrap();
    let partial = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(ordinal.with_ordinal(OrdinalSelector::Exact(1)))
                    .with_ordinal_palette(ThemeTarget::Text, palette),
            ),
        )
        .unwrap();
    assert_eq!(
        try_render_c4_svg_with_theme(source, &partial)
            .expect_err("an ordinal does not suppress the whole palette")
            .unverified_family_theme(),
        Some((merman_render::DiagramFamilyId::C4, 2))
    );
}

#[test]
fn c4_ignored_base_size_leaves_typed_title_size_available() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(
                merman_render::DiagramFamilyId::C4,
                ThemeTextStyle::default().with_font_size_px(29.0).unwrap(),
            ),
        ))
        .unwrap();
    let body = "C4Context\ntitle Size audit\nSystem(api, \"API\")\n";
    for (value, expected) in [
        (serde_json::json!(false), "29"),
        (serde_json::json!("bad"), "29"),
        (serde_json::json!("calc(24px);fill:red"), "29"),
        (serde_json::Value::Null, "29"),
        (serde_json::json!({}), "29"),
        (serde_json::json!([]), "29"),
        (serde_json::json!(-2), "29"),
        (serde_json::json!("-2px"), "29"),
        (serde_json::json!(23), "23"),
        (serde_json::json!("23px"), "23"),
        (serde_json::json!(0), "0"),
        (serde_json::json!("0px"), "0"),
        (serde_json::json!("0em"), "0"),
        (serde_json::json!("0%"), "0"),
        (serde_json::json!("0rem"), "0"),
        (serde_json::json!(0.5), "0.5"),
        (serde_json::json!("0.5px"), "0.5"),
        (serde_json::json!(1e-7), "0.0000001"),
    ] {
        let config = serde_json::json!({"fontSize":77, "themeVariables":{"fontSize":value}});
        for from_source in [false, true] {
            let (source, site) = if from_source {
                (
                    format!("---\nconfig: {config}\n---\n{body}"),
                    serde_json::json!({}),
                )
            } else {
                (body.to_owned(), config.clone())
            };
            let rendered = try_render_c4_rendered_with_engine(
                &source,
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(site),
                ),
            )
            .unwrap();
            assert!(
                rendered.svg().contains(&format!("font-size:{expected}px;")),
                "source={from_source}: {value}"
            );
            assert!(rendered.svg().contains(">Size audit</text>"));
            assert!(
                rendered.svg().contains(".c4-shape.c4-system .label{font-family:\"Open Sans\",sans-serif;font-size:14px;"),
                "element text keeps its own size"
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), usize::from(expected == "29"));
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(expected != "29")
            );
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn c4_typed_base_typography_reaches_root_css_and_title_evidence() {
    let source = "---\ntitle: C4 typography\n---\nC4Context\nSystem(service, \"Service\")\n";
    for size in [1e-7, 0.5, 18.0] {
        let theme = c4_typography_theme(FontStack::single("Inter").unwrap(), size);
        let rendered =
            try_render_c4_rendered_with_theme(source, &theme).expect("render typed C4 title");
        assert!(rendered.svg().contains("font-family:Inter;"));
        assert!(
            rendered
                .svg()
                .contains(&format!("font-size:{}px;", f64::from(size)))
        );
        assert!(rendered.svg().contains(">C4 typography</text>"));

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.accounted_count(), 2);
        assert_eq!(evidence.applied_count(), 2);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn c4_zero_root_size_only_proves_visible_inherited_roles() {
    let theme = c4_typography_theme(FontStack::single("Cousine").unwrap(), 29.0);
    for (family, applied) in [("Independent", 0), ("inherit", 1)] {
        let config = serde_json::json!({
            "themeVariables":{"fontSize":0},
            "c4":{"systemFontFamily":family}
        });
        let rendered = try_render_c4_rendered_with_engine(
            "C4Context\ntitle Invisible title\nSystem(a, \"Visible node\")\n",
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config),
            ),
        )
        .unwrap();
        assert!(rendered.svg().contains("font-size:0px;"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.applied_count(), applied);
        assert_eq!(evidence.not_applicable_count(), 2 - applied);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
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
    theme: &DiagramTheme,
) -> (
    family::RenderedFamilySvg,
    Vec<(String, TextMeasurementOperation, TextStyle)>,
    C4DiagramLayout,
) {
    let parsed = common::install_parse_compatibility(
        theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(config),
        ),
    )
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
        .begin_session_with_theme(theme)
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
    let theme = c4_typography_theme(FontStack::new(["Cousine", "serif"]).unwrap(), 29.0);
    for family in ["", "inherit", "unset", r"i\6eherit", r"u\6eset"] {
        for has_title in [false, true] {
            let config = serde_json::json!({"c4": {
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
            let (rendered, probes, _) = render_c4_with_font_host(&source, config, &theme);
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
                            && style.font_family.as_deref() == Some("Cousine, serif")
                            && (style.font_size - size).abs() < 1e-9
                            && style.font_weight.as_deref() == Some(weight)
                    }),
                    "{family}, title={has_title}, missing resolved probe for {text}: {probes:?}"
                );
            }
            assert!(
                probes.iter().all(|(_, _, style)| {
                    style.font_family.as_deref() == Some("Cousine, serif")
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
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 2);
            assert_eq!(evidence.applied_count(), 1 + usize::from(has_title));
            assert_eq!(evidence.not_applicable_count(), usize::from(!has_title));
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn c4_font_stack_evidence_follows_actual_inherited_roles() {
    let theme = c4_typography_theme(FontStack::single("Cousine").unwrap(), 29.0);
    let nodes = "System(a, \"A\")\nSystem(b, \"B\")\n";
    for (config, body, inherits) in [
        (
            serde_json::json!({}),
            "System(a, \"A\")\n".to_owned(),
            false,
        ),
        (
            serde_json::json!({"systemFontFamily":"inherit"}),
            "System(a, \"\")\n".to_owned(),
            true,
        ),
        (
            serde_json::json!({"personFontFamily":"inherit"}),
            nodes.to_owned(),
            false,
        ),
        (
            serde_json::json!({"personFontFamily":"var(--host-font)"}),
            nodes.to_owned(),
            false,
        ),
        (
            serde_json::json!({"systemFontFamily":"inherit", "systemFontSize":"0px"}),
            nodes.to_owned(),
            false,
        ),
        (
            serde_json::json!({"boundaryFontFamily":"unset"}),
            "Boundary(b, \"\", \"\") {\nSystem(a, \"A\")\n}\n".to_owned(),
            false,
        ),
        (
            serde_json::json!({"boundaryFontFamily":"inherit"}),
            "Boundary(b, \"Boundary\", \"\") {\nSystem(a, \"A\")\n}\n".to_owned(),
            true,
        ),
        (
            serde_json::json!({"boundaryFontFamily":"inherit"}),
            "Boundary(b, \"\", \"Type\") {\nSystem(a, \"A\")\n}\n".to_owned(),
            true,
        ),
        (
            serde_json::json!({"boundaryFontFamily":"inherit"}),
            "Deployment_Node(b, \"\", \"\", \"Description\") {\nSystem(a, \"A\")\n}\n".to_owned(),
            true,
        ),
        (
            serde_json::json!({"messageFontFamily":"inherit"}),
            nodes.to_owned(),
            false,
        ),
        (
            serde_json::json!({"messageFontFamily":"inherit"}),
            format!("{nodes}Rel(a, b, \"Calls\")\n"),
            true,
        ),
        (
            serde_json::json!({"messageFontFamily":"unset"}),
            format!("{nodes}Rel(a, b, \"\", \"HTTPS\")\n"),
            true,
        ),
        (
            serde_json::json!({"messageFontFamily":"inherit"}),
            format!("{nodes}Rel(a, b, \" \", \" \", \"Hidden description\")\n"),
            false,
        ),
        (
            serde_json::json!({"systemFontFamily":"inherit", "boundaryFontFamily":"inherit", "messageFontFamily":"inherit"}),
            "UpdateLayoutConfig($c4ShapeInRow=\"3\")\n".to_owned(),
            false,
        ),
        (
            serde_json::json!({"systemFontFamily":"\"inherit\""}),
            nodes.to_owned(),
            false,
        ),
        (
            serde_json::json!({"systemFontFamily":"\"unset\""}),
            nodes.to_owned(),
            false,
        ),
    ] {
        for has_title in [false, true] {
            let source = format!(
                "C4Context\n{}{body}",
                if has_title { "title Context\n" } else { "" }
            );
            let rendered = try_render_c4_rendered_with_engine(
                &source,
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(
                        serde_json::json!({"c4":config}),
                    ),
                ),
            )
            .unwrap();
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            let applied = usize::from(has_title || inherits) + usize::from(has_title);
            assert_eq!(evidence.required_count(), 2);
            assert_eq!(evidence.applied_count(), applied, "{config}, {source}");
            assert_eq!(
                evidence.not_applicable_count(),
                2 - applied,
                "{config}, {source}"
            );
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn c4_zero_and_fractional_legacy_font_sizes_follow_visible_lines() {
    let theme = c4_typography_theme(FontStack::single("Cousine").unwrap(), 29.0);
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
            let mut config = serde_json::json!({"c4": {
                "wrap":false, "boundaryFontFamily":family, "messageFontFamily":family
            }});
            config["c4"][key] = serde_json::json!(base);
            let (rendered, probes, _) = render_c4_with_font_host(&source, config.clone(), &theme);
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
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            let applied = usize::from(used > 0.0 && family == "inherit")
                + usize::from(used > 0.0 && raw < 0.0);
            assert_eq!(evidence.required_count(), 2);
            assert_eq!(evidence.applied_count(), applied, "{config}, {source}");
            assert_eq!(evidence.not_applicable_count(), 2 - applied);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn c4_invalid_element_sizes_inherit_root_before_section_scaling() {
    let theme = c4_typography_theme(FontStack::single("Cousine").unwrap(), 29.0);
    let source = "C4Context\nSystem(a, \"NodeName\", \"NodeDescription\")\n";
    for (raw, used) in [(-3.0, 29.0), (-1.0, 29.0), (0.0, 29.0), (1e-7, 1e-7)] {
        for family in ["inherit", "Independent"] {
            let config = serde_json::json!({"c4": {
                "wrap":false, "systemFontSize":raw, "systemFontFamily":family,
                "systemFontWeight":600
            }});
            let (rendered, probes, _) = render_c4_with_font_host(source, config.clone(), &theme);
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
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            let applied = usize::from(raw <= 0.0) + usize::from(family == "inherit");
            assert_eq!(evidence.required_count(), 2);
            assert_eq!(evidence.applied_count(), applied, "{config}");
            assert_eq!(evidence.not_applicable_count(), 2 - applied);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn c4_unknown_font_owners_remain_property_local_residuals() {
    let theme = c4_typography_theme(FontStack::single("Cousine").unwrap(), 29.0);
    for family in ["initial", r"i\6eitial", "revert", "var(--host-font)"] {
        for has_title in [false, true] {
            let source = format!(
                "C4Context\n{}System(a, \"A\")\n",
                if has_title { "title Context\n" } else { "" }
            );
            let engine = MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"c4":{"systemFontFamily":family}}),
                ),
            );
            let rendered = try_render_c4_rendered_with_engine_requirement(
                &source,
                &theme,
                engine,
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            assert!(rendered.svg().contains(&format!(
                ".c4-shape.c4-system .label{{font-family:{family};"
            )));
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), usize::from(has_title));
            assert_eq!(evidence.not_applicable_count(), usize::from(!has_title));
            assert_eq!(evidence.theme_residual_count(), 1, "{family}, {source}");
        }
    }
}

#[test]
fn c4_element_css_sizes_keep_source_units_inheritance_and_evidence() {
    let theme = c4_typography_theme(FontStack::single("Cousine").unwrap(), 29.0);
    let body = "C4Context\nSystem(a, \"NodeName\", \"NodeDescription\")\n";
    for (raw, used, css, inherits) in [
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
            let config = serde_json::json!({"c4": {
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
            let (rendered, probes, _) = render_c4_with_font_host(&source, site, &theme);
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
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            let applied = usize::from(used > 0.0) * (1 + usize::from(inherits));
            assert_eq!(
                evidence.applied_count(),
                applied,
                "{raw}: source={from_source}"
            );
            assert_eq!(evidence.not_applicable_count(), 2 - applied);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn c4_element_cssom_rejects_literal_priority_before_normalizing_fonts() {
    let theme = c4_typography_theme(FontStack::single("Cousine").unwrap(), 29.0);
    let source = "C4Context\nSystem(a, \"NodeName\", \"NodeDescription\")\n";
    for (family, size, family_css, size_css, used, inherits_family, inherits_size) in [
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
        let config = serde_json::json!({"c4": {"wrap":false,
            "systemFontFamily":family,"systemFontSize":size,"systemFontWeight":600}});
        let (rendered, probes, _) = render_c4_with_font_host(source, config, &theme);
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
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        let applied = usize::from(inherits_family) + usize::from(inherits_size);
        assert_eq!(evidence.applied_count(), applied, "{family}/{size}");
        assert_eq!(evidence.not_applicable_count(), 2 - applied);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn c4_unknown_element_sizes_do_not_certify_typography_or_label_paint() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_default(
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("Cousine").unwrap())
                            .with_font_size_px(29.0)
                            .unwrap(),
                    ),
                )
                .with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::NodeLabel,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                ))),
        )
        .unwrap();
    for size in ["1rem", "calc(24px)", "var(--audit-size, 24px)"] {
        for zero_root in [false, true] {
            let mut config = serde_json::json!({"c4": {
                "wrap":false, "systemFontFamily":"inherit", "systemFontSize":size
            }});
            if zero_root {
                config["themeVariables"] = serde_json::json!({"fontSize":"0px"});
            }
            let rendered = try_render_c4_rendered_with_engine_requirement(
                "C4Context\nSystem(a, \"API\")\n",
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(config),
                ),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            assert!(rendered.svg().contains(&format!("font-size:{size};")));
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 0, "{size}: zero_root={zero_root}");
            assert_eq!(evidence.not_applicable_count(), usize::from(zero_root));
            assert_eq!(evidence.theme_residual_count(), 3 - usize::from(zero_root));
        }
    }
}

#[test]
fn c4_unknown_root_sizes_preserve_css_and_keep_independent_element_sizes() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_default(
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("Cousine").unwrap())
                            .with_font_size_px(29.0)
                            .unwrap(),
                    ),
                )
                .with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::Title,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                ))),
        )
        .unwrap();
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
            let rendered = try_render_c4_rendered_with_engine_requirement(
                &source,
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(site),
                ),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            assert!(rendered.svg().contains(&format!("font-size:{size};")));
            assert!(rendered.svg().contains(
                ".c4-shape.c4-system .label{font-family:\"Open Sans\",sans-serif;font-size:14px;"
            ));
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 0, "{size}: source={from_source}");
            assert_eq!(evidence.not_applicable_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 2);
        }
    }
}

#[test]
fn c4_subpixel_root_fallback_multiline_sections_use_actual_em_steps() {
    let theme = c4_typography_theme(FontStack::single("Cousine").unwrap(), 0.5);
    for configured_root in [false, true] {
        let mut config = serde_json::json!({"c4": {
            "wrap":false, "systemFontSize":-1, "systemFontFamily":"inherit"
        }});
        if configured_root {
            config["themeVariables"] = serde_json::json!({"fontSize":"0.5px"});
        }
        let (_, _, single) = render_c4_with_font_host(
            "C4Context\nSystem(a, \"A\", \"First\")\n",
            config.clone(),
            &theme,
        );
        let (rendered, probes, multi) = render_c4_with_font_host(
            "C4Context\nSystem(a, \"A<br/>B\", \"First<br/>Second\")\n",
            config,
            &theme,
        );
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
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 2 - usize::from(configured_root));
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(configured_root)
        );
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn c4_cluster_paint_is_not_applicable_without_explicit_boundaries() {
    let theme = c4_cluster_paint_theme(
        CanvasPaint::solid("#ef4444").expect("valid empty-domain C4 Cluster fill"),
        CanvasPaint::solid("#2563eb").expect("valid empty-domain C4 Cluster stroke"),
    );
    let svg = render_c4_svg_with_theme("C4Context\nSystem(service, \"Service\")\n", &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid empty-domain C4 SVG XML");
    let service = c4_rect_labeled(&document, "Service");

    assert_eq!(
        c4_effective_property(service, "fill"),
        Some("#1168BD".to_owned())
    );
    assert_eq!(
        c4_effective_property(service, "stroke"),
        Some("#3C7FC0".to_owned())
    );
    assert!(!svg.contains("#ef4444"));
    assert!(!svg.contains("#2563eb"));
}

#[test]
fn c4_cluster_paint_rejects_variant_ordinal_clear_gradient_and_pattern_routes() {
    let stops = || {
        [
            GradientStop::new(
                0.0,
                ThemeColorValue::parse("#123456").expect("valid C4 gradient start"),
            )
            .expect("valid C4 gradient stop"),
            GradientStop::new(
                1.0,
                ThemeColorValue::parse("#abcdef").expect("valid C4 gradient end"),
            )
            .expect("valid C4 gradient stop"),
        ]
    };
    let linear = LinearGradient::new(90.0, stops()).expect("valid C4 linear gradient");
    let radial = RadialGradient::new(
        ThemeLength::percent(50.0),
        ThemeLength::percent(50.0),
        ThemeLength::percent(50.0),
        stops(),
    )
    .expect("valid C4 radial gradient");
    let pattern = PatternSpec::new(
        PatternKind::Grid,
        8.0,
        8.0,
        ThemeColorValue::parse("#123456").expect("valid C4 pattern color"),
    )
    .expect("valid C4 pattern");
    let mut clear = ThemeStylePatch::default();
    clear.paint.fill = Specified::Clear;
    clear.stroke.paint = Specified::Clear;
    let solid = || {
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").expect("valid C4 fill"))
            .with_stroke(CanvasPaint::solid("#abcdef").expect("valid C4 stroke"))
    };
    let cases = [
        ThemeRule::new(ThemeTarget::Cluster, solid()).with_variant(ThemeVariant::Default),
        ThemeRule::new(ThemeTarget::Cluster, solid())
            .with_ordinal(OrdinalSelector::exact(1).expect("valid C4 boundary ordinal")),
        ThemeRule::new(ThemeTarget::Cluster, clear),
        ThemeRule::new(
            ThemeTarget::Cluster,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::LinearGradient(linear.clone()))
                .with_stroke(CanvasPaint::LinearGradient(linear)),
        ),
        ThemeRule::new(
            ThemeTarget::Cluster,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::RadialGradient(radial.clone()))
                .with_stroke(CanvasPaint::RadialGradient(radial)),
        ),
        ThemeRule::new(
            ThemeTarget::Cluster,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::Pattern(pattern.clone()))
                .with_stroke(CanvasPaint::Pattern(pattern)),
        ),
    ];
    let source =
        "C4Context\nBoundary(boundary, \"Boundary\") {\n  System(service, \"Service\")\n}\n";

    for rule in cases {
        let theme = c4_cluster_rules_theme([rule.for_family(merman_render::DiagramFamilyId::C4)]);
        let error = try_render_c4_svg_with_theme(source, &theme)
            .expect_err("unsupported C4 Cluster paint routes must fail closed");
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::C4, 1))
        );
    }
}

#[test]
fn c4_cluster_radius_applies_to_explicit_solid_and_dashed_boundaries_only() {
    let source = r#"C4Context
Node(solid, "Solid Boundary") {
  System(service, "Service")
}
Boundary(dashed, "Dashed Boundary") {
  System(worker, "Worker")
}
"#;
    let theme = c4_cluster_radius_theme(Specified::Value(9.0));
    let svg = render_c4_svg_with_theme(source, &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid themed C4 SVG XML");

    let solid_boundary = c4_rect_labeled(&document, "Solid Boundary");
    assert_eq!(solid_boundary.attribute("rx"), Some("9"));
    assert_eq!(solid_boundary.attribute("ry"), Some("9"));
    assert_eq!(solid_boundary.attribute("stroke-dasharray"), None);

    let dashed_boundary = c4_rect_labeled(&document, "Dashed Boundary");
    assert_eq!(dashed_boundary.attribute("rx"), Some("9"));
    assert_eq!(dashed_boundary.attribute("ry"), Some("9"));
    assert_eq!(
        dashed_boundary.attribute("stroke-dasharray"),
        Some("7.0,7.0")
    );

    let ordinary_element = c4_rect_labeled(&document, "Service");
    assert_eq!(
        c4_effective_property(ordinary_element, "rx"),
        Some("12px".to_owned())
    );
    assert_eq!(
        c4_effective_property(ordinary_element, "ry"),
        Some("12px".to_owned())
    );

    let cleared_svg = render_c4_svg_with_theme(source, &c4_cluster_radius_theme(Specified::Clear));
    let cleared_document =
        roxmltree::Document::parse(&cleared_svg).expect("valid cleared C4 SVG XML");
    for label in ["Solid Boundary", "Dashed Boundary", "Service"] {
        let rect = c4_rect_labeled(&cleared_document, label);
        let expected_radius = if label == "Service" { "12px" } else { "2.5" };
        assert_eq!(
            c4_effective_property(rect, "rx"),
            Some(expected_radius.to_owned())
        );
        assert_eq!(
            c4_effective_property(rect, "ry"),
            Some(expected_radius.to_owned())
        );
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

#[test]
fn c4_title_fill_accounts_visibility_and_property_local_source_ownership() {
    let text_color = r##"{"textColor":"#abcdef"}"##;
    let title_color = r##"{"titleColor":"#abcdef"}"##;
    for (title, variables, site, stroke, applied, residual) in [
        (false, "{}", false, false, false, false),
        (true, "{}", false, false, true, false),
        (true, text_color, false, false, false, false),
        (true, text_color, false, true, false, true),
        (true, title_color, false, false, true, false),
        (true, "{}", false, true, false, true),
        (true, text_color, true, false, false, false),
        (true, text_color, true, true, false, true),
    ] {
        let source_variables = if site { "{}" } else { variables };
        let source = format!(
            "---\ntitle: \"{}\"\nconfig:\n  themeVariables: {source_variables}\n---\nC4Context\nSystem(a, \"Service\")\n",
            if title { "Coverage title" } else { "" },
        );
        let mut patch =
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
        if stroke {
            patch = patch.with_stroke(CanvasPaint::solid("#654321").unwrap());
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Title, patch)),
            ))
            .unwrap();
        let baseline = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new())
            .unwrap();
        let render = |theme, portability| {
            let engine = if site {
                let variables: serde_json::Value = serde_json::from_str(variables).unwrap();
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(
                        serde_json::json!({
                            "look": "classic", "layout": "dagre", "themeVariables": variables,
                        }),
                    ),
                )
            } else {
                MermaidEngine::new()
            };
            try_render_c4_rendered_with_engine_requirement(&source, theme, engine, portability)
        };
        let rendered = render(&theme, ThemePortabilityRequirement::BestEffort).unwrap();
        let baseline = render(&baseline, ThemePortabilityRequirement::BestEffort).unwrap();
        let paints_title = title && !variables.contains("textColor");
        assert_eq!(rendered.svg() == baseline.svg(), !paints_title);
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let title_node = document
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some("Coverage title"));
        assert_eq!(title_node.is_some(), title);
        if let Some(title_node) = title_node {
            assert_eq!(
                title_node.attribute("fill"),
                paints_title.then_some("#123456")
            );
        }
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), usize::from(applied));
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(!applied && !residual)
        );
        assert_eq!(evidence.theme_residual_count(), usize::from(residual));
        assert_eq!(
            completion
                .report()
                .theme_diagnostics()
                .iter()
                .any(|diagnostic| {
                    diagnostic.target() == Some(ThemeTarget::Title)
                        && diagnostic.code() == "unsupported-paint"
                }),
            residual,
        );
        match render(&theme, ThemePortabilityRequirement::RequirePortable) {
            Ok(_) => assert!(
                !residual,
                "winning unsupported Title stroke must fail strict admission"
            ),
            Err(error) => {
                assert!(residual);
                assert_eq!(
                    error.unverified_family_theme(),
                    Some((merman_render::DiagramFamilyId::C4, 1)),
                );
            }
        }
    }
}

#[test]
fn c4_title_accounting_resolves_shadowed_rules_and_absent_ordinals() {
    let fill = |color| {
        ThemeRule::new(
            ThemeTarget::Title,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid(color).unwrap()),
        )
    };
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(fill("#112233"))
                    .with_rule(fill("#223344").with_ordinal(OrdinalSelector::Exact(1)))
                    .with_rule(fill("#334455").with_ordinal(OrdinalSelector::Exact(2)))
                    .with_rule(fill("#445566")),
            ),
        )
        .unwrap();
    let rendered = try_render_c4_rendered_with_engine_requirement(
        "---\ntitle: Coverage title\n---\nC4Context\nSystem(a, \"Service\")\n",
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let title = document
        .descendants()
        .find(|node| node.has_tag_name("text") && node.text() == Some("Coverage title"))
        .unwrap();
    assert_eq!(title.attribute("fill"), Some("#445566"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 4);
    assert_eq!(evidence.accounted_count(), 4);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 3);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn c4_title_static_fill_and_clear_override_text_fallback() {
    use merman_render::diagram_theme::Specified;
    for variant in [None, Some(ThemeVariant::Default)] {
        for fill in [
            None,
            Some(Specified::Value(CanvasPaint::solid("#123456").unwrap())),
            Some(Specified::Value(CanvasPaint::Transparent)),
            Some(Specified::Clear),
        ] {
            let title_has_own_fill = fill.is_some();
            let expected = match &fill {
                None => Some("#224466"),
                Some(Specified::Value(CanvasPaint::Solid(_))) => Some("#123456"),
                Some(Specified::Value(CanvasPaint::Transparent)) => Some("transparent"),
                Some(Specified::Clear) => Some("#333"),
                _ => unreachable!(),
            };
            let mut styles = ThemeRuleSet::default();
            if let Some(fill) = fill {
                let mut patch = ThemeStylePatch::default();
                patch.paint.fill = fill;
                let mut rule = ThemeRule::new(ThemeTarget::Title, patch);
                if let Some(variant) = variant {
                    rule = rule.with_variant(variant);
                }
                styles = styles.with_rule(rule);
            }
            // A later Text fallback must not take ownership from an explicit title role.
            styles = styles.with_rule(ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#224466").unwrap()),
            ));
            let theme = DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new().with_styles(styles))
                .unwrap();
            let rendered = try_render_c4_rendered_with_engine_requirement(
                "---\ntitle: Coverage title\n---\nC4Context\nSystem(a, \"Service\")\nUpdateElementStyle(a, $fontColor=\"#abcdef\")\n",
                &theme,
                MermaidEngine::new(),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let title = document
                .descendants()
                .find(|node| node.has_tag_name("text") && node.text() == Some("Coverage title"))
                .unwrap();
            assert_eq!(title.attribute("fill"), expected);
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .and_then(|node| node.text())
                .unwrap();
            assert!(css.contains("fill:#224466;"));
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.accounted_count(), evidence.required_count());
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(title_has_own_fill)
            );
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn c4_title_selectors_keep_only_real_unsupported_winners_residual() {
    use merman_render::diagram_theme::OrdinalSelector;
    for (variant, ordinal, residual) in [
        (None, Some(OrdinalSelector::Exact(1)), true),
        (None, Some(OrdinalSelector::Exact(2)), false),
        (Some(ThemeVariant::Error), None, false),
        (
            Some(ThemeVariant::Error),
            Some(OrdinalSelector::Exact(1)),
            false,
        ),
    ] {
        let mut rule = ThemeRule::new(
            ThemeTarget::Title,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        );
        if let Some(variant) = variant {
            rule = rule.with_variant(variant);
        }
        if let Some(ordinal) = ordinal {
            rule = rule.with_ordinal(ordinal);
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
            .unwrap();
        let rendered = try_render_c4_rendered_with_engine_requirement(
            "---\ntitle: Coverage title\n---\nC4Context\nSystem(a, \"Service\")\n",
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let title = document
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some("Coverage title"))
            .unwrap();
        assert_eq!(title.attribute("fill"), None);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), usize::from(!residual));
        assert_eq!(evidence.theme_residual_count(), usize::from(residual));
    }
}

#[test]
fn c4_all_presets_consume_title_fill_without_unsupported_title_diagnostics() {
    let presets = merman_render::diagram_theme::theme_preset_descriptors();
    assert_eq!(presets.len(), 11);
    for preset in presets {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(preset.preset())
            .unwrap();
        let rendered = try_render_c4_rendered_with_engine_requirement(
            "---\ntitle: Coverage title\n---\nC4Context\nSystem(a, \"Service\")\n",
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let title = document
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some("Coverage title"))
            .unwrap();
        assert!(title.attribute("fill").is_some(), "{}", preset.id());
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert!(evidence.applied_count() > 0, "{}", preset.id());
        assert_eq!(
            evidence.accounted_count(),
            evidence.required_count(),
            "{}",
            preset.id()
        );
        assert!(
            completion
                .report()
                .theme_diagnostics()
                .iter()
                .all(|diagnostic| {
                    diagnostic.target() != Some(ThemeTarget::Title)
                        && diagnostic.code() != "evidence-incomplete"
                }),
            "{}",
            preset.id()
        );
    }
}

#[allow(dead_code)]
mod common;
