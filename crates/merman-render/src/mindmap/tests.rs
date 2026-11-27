fn independent_tidy_tree_presentation(
    edges: &[(usize, usize)],
) -> merman_model::presentation::PresentationDiagram {
    use merman_model::presentation::{
        AuthoredPresentationMetadata, PresentationContext, PresentationDiagram, RenderSemanticModel,
    };
    use merman_model::resources::InputResourcePolicy;
    use merman_model::{MermaidConfig, OperationControl};
    use serde_json::json;

    let nodes = [0, 1, 1, 2]
        .into_iter()
        .enumerate()
        .map(|(index, level)| {
            json!({
                "id": index.to_string(),
                "domId": format!("node_{index}"),
                "label": "Repeated",
                "shape": "rect",
                "cssClasses": if index == 0 {
                    "mindmap-node section-root section--1"
                } else {
                    "mindmap-node section-0"
                },
                "nodeId": "repeated-authored-name",
                "level": level
            })
        })
        .collect::<Vec<_>>();
    let edges = edges
        .iter()
        .enumerate()
        .map(|(index, (start, end))| {
            json!({
                "id": format!("edge_{index}"),
                "start": start.to_string(),
                "end": end.to_string()
            })
        })
        .collect::<Vec<_>>();
    let model = serde_json::from_value(json!({ "nodes": nodes, "edges": edges }))
        .expect("independent Mindmap full data");
    let config = MermaidConfig::from_value(json!({
        "layout": "tidy-tree",
        "htmlLabels": false
    }));
    PresentationDiagram::try_new(
        AuthoredPresentationMetadata {
            diagram_type: "mindmap".to_string(),
            config: config.clone(),
            effective_config: config,
            title: None,
        },
        RenderSemanticModel::Mindmap(model),
        PresentationContext::None,
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .expect("presentation construction should not cancel")
    .expect("construction leaves layout-specific graph admission to the renderer")
}

#[test]
fn mindmap_model_admission_rejects_multiple_incoming_edges() {
    for (edges, target) in [
        ([(0, 1), (0, 2), (1, 3), (2, 3)].as_slice(), "3"),
        ([(0, 1), (0, 1)].as_slice(), "1"),
    ] {
        let parsed = independent_tidy_tree_presentation(edges);
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin independent render session");
        let result = crate::family::prepare(&parsed, &crate::LayoutOptions::default(), session);
        let Err(crate::Error::InvalidModel { message }) = result else {
            panic!("tidy-tree must reject multiple incoming edges before subtree expansion");
        };
        assert_eq!(
            message,
            format!("tidy-tree node {target} has more than one parent edge")
        );
    }
}

#[test]
fn mindmap_model_admission_accepts_tree_with_repeated_authored_names_and_labels() {
    let parsed = independent_tidy_tree_presentation(&[(0, 1), (0, 2), (1, 3)]);
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin independent render session");
    let artifact = crate::family::prepare(&parsed, &crate::LayoutOptions::default(), session)
        .expect("distinct internal IDs form a valid tree despite repeated authored data");
    let rendered = artifact
        .render_svg(
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("independent-mindmap".to_string()),
                ..Default::default()
            },
            &crate::svg::SvgDebugOptions::default(),
        )
        .expect("render independent Mindmap without a compatibility JSON projection");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Mindmap SVG");
    let labels = document
        .descendants()
        .filter(|node| node.is_text() && node.text() == Some("Repeated"))
        .count();
    assert_eq!(
        labels, 4,
        "every distinct node must retain its authored label"
    );
}

#[test]
fn mindmap_declined_math_measurement_keeps_markdown_fallback() {
    let parsed = merman_core::MermaidEngine::new().parse_diagram_for_render_model_sync(
        "---\nconfig:\n  layout: tidy-tree\n---\nmindmap\n  root[Root]\n    child[\"**Bold** $$x$$\"]",
        merman_core::ParseOptions::strict(),
    ).unwrap().unwrap().into_presentation();
    let merman_model::presentation::RenderSemanticModel::Mindmap(model) = parsed.model() else {
        panic!("Mindmap render model");
    };
    let padding = model.nodes[1].padding;
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_math_renderer(std::sync::Arc::new(crate::math::NoopMathRenderer))
        .begin_session()
        .unwrap();
    let artifact =
        crate::family::prepare(&parsed, &crate::LayoutOptions::default(), session).unwrap();
    let layout = artifact.layout_json().unwrap();
    let nodes = layout["layout"]["MindmapDiagram"]["nodes"]
        .as_array()
        .unwrap();
    assert_eq!(nodes.len(), 2);
    let child = nodes
        .iter()
        .find(|node| node["id"] == "1")
        .expect("Markdown child node");
    let config = serde_json::json!({});
    let style = crate::text::TextStyle {
        font_family: Some(crate::config::config_font_family_css(&config)),
        font_size: crate::config::config_theme_font_size_css_or_root_number_px(&config, 16.0)
            .max(1.0),
        font_weight: None,
        font_style: None,
    };
    let measurer = crate::text::DeterministicTextMeasurer::default();
    let html = crate::text::mermaid_markdown_to_html_label_fragment("**Bold** $$x$$", true);
    let expected = crate::text::measure_html_with_inline_styles(
        &measurer,
        &html,
        &style,
        Some(120.0),
        crate::text::WrapMode::HtmlLike,
    );
    assert_eq!(
        child["width"].as_f64(),
        Some(expected.width + 2.0 * padding)
    );
    assert_eq!(child["height"].as_f64(), Some(expected.height + padding));
}
