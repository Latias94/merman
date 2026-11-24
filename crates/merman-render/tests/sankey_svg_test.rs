use merman_core::{MermaidEngine, ParseOptions};
use merman_model::OperationControl;
use merman_model::presentation::{PresentationDiagram, RenderSemanticModel};
use merman_model::resources::InputResourcePolicy;
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn amend_sankey_node_ids(parsed: PresentationDiagram, ids: [&str; 2]) -> PresentationDiagram {
    let (metadata, mut model, context) = parsed.into_parts();
    let RenderSemanticModel::Sankey(sankey) = &mut model else {
        panic!("expected Sankey model");
    };
    let [source, target] = sankey.graph.nodes.as_mut_slice() else {
        panic!("Sankey label fixture must contain exactly two nodes");
    };
    let [link] = sankey.graph.links.as_mut_slice() else {
        panic!("Sankey label fixture must contain exactly one link");
    };
    assert_eq!(link.source, source.id);
    assert_eq!(link.target, target.id);
    assert_ne!(ids[0], ids[1]);
    source.id = ids[0].to_string();
    target.id = ids[1].to_string();
    link.source = source.id.clone();
    link.target = target.id.clone();
    PresentationDiagram::try_from_frozen(
        metadata,
        model,
        context,
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .expect("validate caller-authored Sankey presentation")
    .expect("admit caller-authored Sankey presentation")
}

fn sankey_node_fills(svg: &str) -> Vec<String> {
    let document = roxmltree::Document::parse(svg).expect("valid Sankey SVG");
    document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("node"))
        .map(|node| {
            node.children()
                .find(|child| child.has_tag_name("rect"))
                .and_then(|rect| rect.attribute("fill"))
                .unwrap_or_else(|| panic!("Sankey node group must contain a filled rect: {svg}"))
                .to_string()
        })
        .collect()
}

fn sankey_style_text(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid Sankey SVG");
    document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Sankey SVG stylesheet")
        .to_owned()
}

fn render_sankey(source: &str, options: &SvgRenderOptions) -> String {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("layout ok")
        .render_svg(options, &SvgDebugOptions::default())
        .expect("render SVG")
        .svg()
        .to_owned()
}

#[test]
fn sankey_visible_values_use_ecmascript_number_text() {
    for (value, expected) in [
        ("1e30", "1e+30"),
        ("1e21", "999999999999999900000"),
        ("10", "10"),
    ] {
        let svg = render_sankey(
            &format!("sankey\nA,B,{value}\n"),
            &SvgRenderOptions::default(),
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        let labels = document
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .map(|node| node.text().unwrap_or_default().to_string())
            .collect::<Vec<_>>();
        assert_eq!(labels, [format!("A\n{expected}"), format!("B\n{expected}")]);
    }
}

fn try_render_sankey_with_resource_policy(
    source: &str,
    options: &SvgRenderOptions,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Sankey resource-bound fixture")
        .expect("detect Sankey resource-bound fixture")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Sankey resource-bound session");
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)?
        .render_svg(options, &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn sankey_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = "sankey-beta\nInput,Transform,10\nTransform,Output,8\n";
    let options = SvgRenderOptions::default();
    let baseline = try_render_sankey_with_resource_policy(
        source,
        &options,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Sankey baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Sankey fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Sankey SVG byte ceiling");
    let exact = try_render_sankey_with_resource_policy(source, &options, exact_policy)
        .expect("the exact Sankey family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Sankey SVG byte ceiling");
    let error = try_render_sankey_with_resource_policy(source, &options, below_policy)
        .expect_err("one byte below the Sankey family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Sankey MaxSvgBytes rejection, got {error}");
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
fn sankey_prepared_labels_enforce_the_reported_retained_byte_boundary_without_a_theme() {
    let source = "---\nconfig:\n  sankey:\n    showValues: false\n---\nsankey\nAB,CD,1\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let baseline = family::prepare(
        &parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic()
            .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
            .begin_session()
            .unwrap(),
    )
    .unwrap()
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .unwrap();
    let expected_svg = baseline.svg().to_owned();
    let completion = baseline.into_completion();
    let exact = completion
        .report()
        .session_report()
        .prepared_text_retained_bytes_peak();
    assert!(
        exact >= 4,
        "both literal labels must own their payload bytes"
    );
    drop(completion);

    for maximum in [1, exact - 1, exact] {
        let result = try_render_sankey_with_resource_policy(
            source,
            &SvgRenderOptions::default(),
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, maximum)
                .unwrap(),
        );
        if maximum == exact {
            assert_eq!(result.unwrap(), expected_svg);
            continue;
        }
        let merman_render::Error::ResourceLimitExceeded(limit) = result.unwrap_err() else {
            panic!("the prepared-label byte ceiling must reject before SVG output");
        };
        assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(
            limit.limit,
            ResourceLimitId::MaxPreparedTextRetainedBytes.as_str()
        );
        assert_eq!(limit.max, maximum);
        assert!(limit.actual > maximum);
        assert!(limit.explicit_overrides.iter().any(|resource_override| {
            resource_override.id == ResourceLimitId::MaxPreparedTextRetainedBytes
                && resource_override.value == maximum
        }));
    }
}

#[test]
fn sankey_prepared_labels_bound_prefix_and_suffix_amplification_before_svg_output() {
    let prefix = "p".repeat(64 * 1024);
    let suffix = "s".repeat(8 * 1024);
    let links = (0..31)
        .map(|index| format!("N{index},N{},1\n", index + 1))
        .collect::<String>();
    let source = format!(
        "---\nconfig: {}\n---\nsankey\n{links}",
        serde_json::json!({"sankey": {"showValues": true, "prefix": prefix, "suffix": suffix}})
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    for maximum in [4 * 1024, 4 * 1024 * 1024] {
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, maximum)
                    .unwrap()
                    .with_limit(ResourceLimitId::MaxSvgBytes, 1)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();
        match family::prepare(&parsed, &LayoutOptions::default(), session) {
            Ok(artifact) => {
                assert_eq!(maximum, 4 * 1024 * 1024);
                drop(artifact);
            }
            Err(merman_render::Error::ResourceLimitExceeded(limit)) => {
                assert_eq!(maximum, 4 * 1024);
                assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
                assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
                assert_eq!(
                    limit.limit,
                    ResourceLimitId::MaxPreparedTextRetainedBytes.as_str()
                );
                assert_eq!(limit.max, maximum);
                assert!(limit.actual >= 32 * (64 + 8) * 1024);
                assert!(limit.actual < 4 * 1024 * 1024);
            }
            Err(error) => panic!("unexpected Sankey preparation error: {error}"),
        }
    }
}

#[test]
fn sankey_svg_uses_configured_node_colors_and_outlined_labels() {
    let svg = render_sankey(
        r##"---
config:
  sankey:
    nodeColors:
      A: "#112233"
      B: rebeccapurple
    labelStyle: outlined
---
sankey-beta
A,B,10
"##,
        &SvgRenderOptions::default(),
    );

    assert!(
        svg.contains(r##"fill="#112233""##),
        "expected node A to use configured fill: {svg}"
    );
    assert!(
        svg.contains(r#"fill="rebeccapurple""#),
        "expected node B to use configured fill: {svg}"
    );
    assert!(
        svg.contains(r##"stop-color="#112233""##),
        "expected source gradient stop to use configured color: {svg}"
    );
    assert!(
        svg.contains(r#"stop-color="rebeccapurple""#),
        "expected target gradient stop to use configured color: {svg}"
    );
    assert!(
        svg.contains(r#"class="sankey-label-bg""#),
        "expected outlined label background text: {svg}"
    );
    assert!(
        svg.contains(r#"class="sankey-label-fg""#),
        "expected outlined label foreground text: {svg}"
    );
    assert!(
        svg.contains(".sankey-label-bg"),
        "expected outlined label CSS: {svg}"
    );
}

#[test]
fn sankey_viewport_contains_all_raw_labels_with_their_emitted_anchors() {
    use merman_render::text::{DeterministicTextMeasurer, TextMeasurer, TextStyle};

    let long_records = format!("{},Output,1\n", "WideLabel".repeat(20));
    let measurer = DeterministicTextMeasurer::default();
    let text_style = TextStyle {
        font_family: Some("monospace".to_string()),
        font_size: 14.0,
        ..TextStyle::default()
    };
    for (label_style, records, show_values) in [
        ("outlined", "A,B,1\n", false),
        ("legacy", "A,B,1\n", false),
        ("outlined", "Input,Hub,1\nOther,Hub,2\nHub,Output,3\n", true),
        ("legacy", long_records.as_str(), true),
    ] {
        for use_max_width in [false, true] {
            let source = format!(
                "---\nconfig:\n  fontFamily: monospace\n  sankey:\n    labelStyle: {label_style}\n    showValues: {show_values}\n    useMaxWidth: {use_max_width}\n    prefix: '<br>value: '\n    suffix: ' units'\n---\nsankey\n{records}"
            );
            let svg = render_sankey(&source, &SvgRenderOptions::default());
            let document = roxmltree::Document::parse(&svg).unwrap();
            let root = document.root_element();
            let view_box = root
                .attribute("viewBox")
                .unwrap()
                .split_whitespace()
                .map(|number| number.parse::<f64>().unwrap())
                .collect::<Vec<_>>();
            let left = view_box[0];
            let right = left + view_box[2];
            let labels = document
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .collect::<Vec<_>>();
            assert!(!labels.is_empty());
            for label in labels {
                let text = label.text().unwrap_or_default();
                let width = measurer.measure_svg_raw_text_bbox_width_px(text, &text_style);
                let x = label.attribute("x").unwrap().parse::<f64>().unwrap();
                let label_left = match label.attribute("text-anchor") {
                    Some("start") => x,
                    Some("end") => x - width,
                    anchor => panic!("unexpected Sankey label anchor {anchor:?}"),
                };
                assert!(
                    left <= label_left + 1e-6 && right + 1e-6 >= label_left + width,
                    "viewport [{left}, {right}] must contain {text:?} at [{label_left}, {}]: {source}",
                    label_left + width
                );
            }
            if use_max_width {
                assert_eq!(root.attribute("width"), Some("100%"));
                assert!(
                    root.attribute("style")
                        .unwrap()
                        .contains(&format!("max-width: {}px", view_box[2]))
                );
            } else {
                assert_eq!(
                    root.attribute("width").unwrap().parse::<f64>().unwrap(),
                    view_box[2]
                );
            }
            if records == "A,B,1\n" {
                if label_style == "legacy" {
                    assert_eq!((left, right), (0.0, 600.0));
                } else {
                    assert!(right > 606.0);
                }
            } else if label_style == "outlined" {
                assert!(left < 0.0 && right > 600.0);
            } else {
                assert!(right > 600.0);
            }
        }
    }
}

#[test]
fn sankey_label_bounds_use_the_raw_text_profile_and_emitted_font() {
    use merman_render::environment::{
        MeasurementProfileId, TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity,
    };
    use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RawLabelMeasurer {
        labels: Mutex<Vec<String>>,
    }

    impl TextMeasurer for RawLabelMeasurer {
        fn measure(&self, _text: &str, style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 0.0,
                height: style.font_size,
                line_count: 1,
            }
        }

        fn measure_svg_raw_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
            assert_eq!(style.font_family.as_deref(), Some("monospace"));
            assert_eq!(style.font_size, 14.0);
            assert!(style.font_weight.is_none());
            assert!(style.font_style.is_none());
            self.labels.lock().unwrap().push(text.to_owned());
            80.0
        }
    }

    let backend = Arc::new(RawLabelMeasurer::default());
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.sankey-raw-labels").unwrap(),
            "1",
        )
        .unwrap(),
        backend.clone(),
    );
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::deterministic()
            .with_profile_for_phase(TextMeasurementPhase::SvgBBox, profile),
    );
    let source = concat!(
        "---\nconfig:\n  fontFamily: monospace\n  sankey:\n",
        "    labelStyle: outlined\n    prefix: 'total '\n    suffix: ' units'\n---\n",
        "sankey\nSource,Target,1\n"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    // Exercise the renderer's preprocessor-placeholder decoding with checked typed IDs;
    // the CSV grammar itself admits only ASCII field content.
    let parsed = amend_sankey_node_ids(parsed, ["ﬂ°°35¶ß32;", "ﬂ°°32¶ß"]);
    let session = environment.begin_session().unwrap();
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let view_box = document
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .split_whitespace()
        .map(|number| number.parse::<f64>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(view_box[0] + view_box[2], 686.0);
    assert!(sankey_style_text(rendered.svg()).contains(".node-labels{font-family:monospace;}"));
    let measured = backend.labels.lock().unwrap();
    assert_eq!(*measured, ["#32;\ntotal 1 units", " \ntotal 1 units"]);
    let emitted = document
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(|node| node.text().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(
        emitted,
        ["#32;\ntotal 1 units", " \ntotal 1 units"].repeat(2),
        "outlined passes must serialize the measured text without decoding it again"
    );
}

#[test]
fn sankey_generated_ids_are_prefixed_when_diagram_id_is_provided() {
    let svg = render_sankey(
        "sankey-beta\nA,B,10\n",
        &SvgRenderOptions {
            diagram_id: Some("sankey-inline".to_string()),
            ..SvgRenderOptions::default()
        },
    );

    assert!(
        svg.contains(r#"id="sankey-inline-node-1""#),
        "expected scoped Sankey node id: {svg}"
    );
    assert!(
        svg.contains(r#"id="sankey-inline-linearGradient-3""#),
        "expected scoped Sankey gradient id: {svg}"
    );
    assert!(
        svg.contains(r#"stroke="url(#sankey-inline-linearGradient-3)""#),
        "expected scoped Sankey gradient reference: {svg}"
    );
    assert!(
        !svg.contains(r#"id="node-1""#),
        "expected no bare Sankey node id: {svg}"
    );
    assert!(
        !svg.contains(r#"id="linearGradient-3""#),
        "expected no bare Sankey gradient id: {svg}"
    );
    assert!(
        !svg.contains(r#"stroke="url(#linearGradient-3)""#),
        "expected no bare Sankey gradient reference: {svg}"
    );
}

#[test]
fn sankey_generated_ids_keep_mermaid_style_without_diagram_id() {
    let svg = render_sankey("sankey-beta\nA,B,10\n", &SvgRenderOptions::default());

    assert!(
        svg.contains(r#"id="node-1""#),
        "expected Mermaid-style Sankey node id without explicit diagram_id: {svg}"
    );
    assert!(
        svg.contains(r#"id="linearGradient-3""#),
        "expected Mermaid-style Sankey gradient id without explicit diagram_id: {svg}"
    );
    assert!(
        svg.contains(r#"stroke="url(#linearGradient-3)""#),
        "expected Mermaid-style Sankey gradient reference without explicit diagram_id: {svg}"
    );
    assert!(
        !svg.contains(r#"id="sankey-node-1""#),
        "expected default rendering to avoid implicit node id scoping: {svg}"
    );
    assert!(
        !svg.contains(r#"id="sankey-linearGradient-3""#),
        "expected default rendering to avoid implicit resource id scoping: {svg}"
    );
}

#[test]
fn sankey_explicit_node_color_does_not_consume_the_default_palette_domain() {
    for link_color in ["gradient", "target"] {
        let source = format!(
            "---\nconfig:\n  sankey:\n    nodeColors:\n      A: '#112233'\n    linkColor: {link_color}\n---\nsankey-beta\nA,B,10\n"
        );
        let svg = render_sankey(&source, &SvgRenderOptions::default());
        assert_eq!(sankey_node_fills(&svg), ["#112233", "#4e79a7"]);
        let document = roxmltree::Document::parse(&svg).unwrap();
        if link_color == "gradient" {
            let colors = document
                .descendants()
                .filter(|node| node.has_tag_name("stop"))
                .map(|node| node.attribute("stop-color").unwrap())
                .collect::<Vec<_>>();
            assert_eq!(colors, ["#112233", "#4e79a7"]);
        } else {
            assert!(document.descendants().any(
                |node| node.has_tag_name("path") && node.attribute("stroke") == Some("#4e79a7")
            ));
        }
    }
}

#[test]
fn sankey_decoded_empty_labels_preserve_separate_node_identities() {
    for label_style in ["legacy", "outlined"] {
        // Explicit renderer-only IDs preserve placeholder decoding and XML filtering.
        for (first, second, decoded) in [
            ("ﬂ°°32¶ß", "ﬂ°°9¶ß", [" ", "\t"]),
            ("ﬂ°°1¶ß", "ﬂ°°2¶ß", ["", ""]),
        ] {
            for show_values in [false, true] {
                let source = format!(
                    "---\nconfig:\n  sankey:\n    labelStyle: {label_style}\n    showValues: {show_values}\n---\nsankey\nSource,Target,1\n"
                );
                let parsed = MermaidEngine::new()
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .unwrap()
                    .unwrap()
                    .into_presentation();
                let parsed = amend_sankey_node_ids(parsed, [first, second]);
                let merman_model::presentation::RenderSemanticModel::Sankey(model) = parsed.model()
                else {
                    panic!("expected Sankey model");
                };
                let ids = model
                    .graph
                    .nodes
                    .iter()
                    .map(|node| node.id.clone())
                    .collect::<Vec<_>>();
                assert_eq!(ids.len(), 2);
                assert_ne!(ids[0], ids[1]);
                let session = RenderEnvironment::deterministic().begin_session().unwrap();
                let artifact =
                    family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
                let projection = artifact.layout_json().unwrap();
                let layout = &projection["layout"]["SankeyDiagram"];
                for (index, id) in ids.iter().enumerate() {
                    assert_eq!(layout["nodes"][index]["id"].as_str(), Some(id.as_str()));
                }
                assert_eq!(layout["links"][0]["source"].as_str(), Some(ids[0].as_str()));
                assert_eq!(layout["links"][0]["target"].as_str(), Some(ids[1].as_str()));
                let rendered = artifact
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .unwrap();
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let labels = document
                    .descendants()
                    .filter(|node| node.has_tag_name("text"))
                    .map(|node| node.text().unwrap_or_default().to_owned())
                    .collect::<Vec<_>>();
                let expected = decoded.map(|text| {
                    if show_values {
                        format!("{text}\n1")
                    } else {
                        text.to_owned()
                    }
                });
                assert_eq!(labels.len(), if label_style == "outlined" { 4 } else { 2 });
                for (index, label) in labels.iter().enumerate() {
                    assert_eq!(label, &expected[index % 2]);
                }
                assert_eq!(sankey_node_fills(rendered.svg()), ["#4e79a7", "#f28e2c"]);
                assert_eq!(
                    document
                        .descendants()
                        .filter(|node| node.has_tag_name("path"))
                        .count(),
                    1,
                    "distinct raw IDs retain their connecting edge"
                );
            }
        }
    }
}

fn assert_sankey_compositing(svg: &str, blend_mode: &str, opacity: &str, layers: [&str; 3]) {
    let document = roxmltree::Document::parse(svg).expect("valid Sankey SVG");
    let actual_layers = document
        .descendants()
        .filter_map(|node| node.attribute("class"))
        .filter(|class| matches!(*class, "links" | "nodes" | "node-labels"))
        .collect::<Vec<_>>();
    assert_eq!(actual_layers, layers);
    let link_groups = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("link"))
        .collect::<Vec<_>>();
    assert!(!link_groups.is_empty());
    let expected_inline = format!("mix-blend-mode: {blend_mode};");
    for link in link_groups {
        assert_eq!(link.attribute("style"), Some(expected_inline.as_str()));
    }
    assert!(sankey_style_text(svg).contains(&format!(
        ".link{{fill:none;stroke-opacity:{opacity};mix-blend-mode:{blend_mode};}}"
    )));
}

#[test]
fn sankey_without_native_theme_keeps_mermaid_compositing_and_order() {
    for mermaid_theme in ["default", "dark"] {
        for label_style in ["legacy", "outlined"] {
            let source = format!(
                "---\nconfig:\n  theme: {mermaid_theme}\n  sankey:\n    labelStyle: {label_style}\n---\nsankey-beta\nA,B,10\nB,C,7\n"
            );
            let svg = render_sankey(&source, &SvgRenderOptions::default());
            assert_sankey_compositing(&svg, "multiply", "0.5", ["nodes", "node-labels", "links"]);
            for id in [
                "node-1",
                "node-2",
                "node-3",
                "linearGradient-4",
                "linearGradient-5",
            ] {
                assert!(svg.contains(&format!("id=\"{id}\"")));
            }
        }
    }
}

#[test]
fn native_theme_v2_sankey_keeps_flows_beneath_nodes_and_uses_source_colors() {
    use merman_render::theme::{BuiltinPreset, Color, NativeTheme, ThemeCompiler};

    let source = concat!(
        "---\nconfig:\n  sankey:\n    nodeColors:\n      A: '#112233'\n",
        "    labelStyle: outlined\n---\nsankey\nA,B,10\n",
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Blueprint, None)
        .unwrap();
    let palette = &theme.family("sankey").data;
    let node_ink = palette.ink(1).to_string();
    let link_fill = palette.fill(1).to_string();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(
            NativeTheme::new(theme).with_background(Color::rgb(240, 230, 220)),
        )
        .unwrap();
    let rendered = family::prepare(parsed.presentation(), &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let svg = rendered.svg();
    assert_sankey_compositing(svg, "normal", "0.35", ["links", "nodes", "node-labels"]);
    assert_eq!(sankey_node_fills(svg), ["#112233", node_ink.as_str()]);
    let document = roxmltree::Document::parse(svg).unwrap();
    let gradient = document
        .descendants()
        .find(|node| {
            node.has_tag_name("linearGradient")
                && node.attribute("gradientUnits") == Some("userSpaceOnUse")
        })
        .unwrap();
    let stops = gradient
        .children()
        .filter_map(|node| node.attribute("stop-color"))
        .collect::<Vec<_>>();
    assert_eq!(stops, ["#112233", link_fill.as_str()]);
    assert!(sankey_style_text(svg).contains(".sankey-label-bg{stroke:#F0E6DC;"));
}
