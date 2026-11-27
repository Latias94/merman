use super::*;
use merman_core::{MermaidEngine, ParseOptions};

#[test]
fn flowchart_layout_only_preserves_geometry_and_preparation_admission() {
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};

    let source = "\
flowchart LR
  subgraph Group[Review queue]
    A[Open] -->|submit| B[Review]
  end
  B --> C[Done]
  style B width:220px,height:90px
  classDef default font-size:18px,stroke-width:3px
  classDef cluster fill:#eee,stroke:#000
";
    let policy = RenderResourcePolicy::unbounded_for_trusted_input();
    let mut engine = MermaidEngine::new();
    if !cfg!(any(feature = "layout-dagre", feature = "layout-elk")) {
        engine = engine.with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"layout": "shoal"}),
            ),
        );
    }
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let options = LayoutOptions::default();
    let render_session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .unwrap();
    let paint_before_render = crate::svg::FlowchartPreparedNodes::paint_table_entries_built();
    let render_work = Arc::clone(render_session.work_meter());
    let reference = prepare(&parsed, &options, render_session).unwrap();
    let paint_after_render = crate::svg::FlowchartPreparedNodes::paint_table_entries_built();
    assert_eq!(paint_after_render - paint_before_render, 4);
    let layout_session = || {
        crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session()
            .unwrap()
    };
    let session = layout_session();
    let layout_work = Arc::clone(session.work_meter());
    let layout = prepare_layout(&parsed, &options, session).unwrap();
    let BuiltinFamilyArtifact::Flowchart(flowchart) = &layout.0.family else {
        panic!("expected Flowchart family artifact");
    };
    assert!(flowchart.prepared_nodes.is_none());
    assert_eq!(
        crate::svg::FlowchartPreparedNodes::paint_table_entries_built(),
        paint_after_render,
    );
    assert_eq!(
        layout.layout_json().unwrap(),
        reference.layout_json().unwrap()
    );
    assert_eq!(
        layout.layout_geometry_json().unwrap(),
        reference.layout_geometry_json().unwrap(),
    );
    assert_eq!(layout_work.used(), render_work.used());
    assert_eq!(
        layout_work.prepared_text_retained_bytes_peak(),
        render_work.prepared_text_retained_bytes_peak(),
    );

    for (id, required) in [
        (ResourceLimitId::MaxLayoutWorkUnits, render_work.used()),
        (
            ResourceLimitId::MaxPreparedTextRetainedBytes,
            render_work.prepared_text_retained_bytes_peak(),
        ),
    ] {
        let limited_session = || {
            crate::environment::RenderEnvironment::deterministic()
                .with_resource_policy(policy.with_limit(id, required - 1).unwrap())
                .begin_session()
                .unwrap()
        };
        let Err(Error::ResourceLimitExceeded(reference)) =
            prepare(&parsed, &options, limited_session())
        else {
            panic!("expected render preparation to exceed {id:?}");
        };
        let Err(Error::ResourceLimitExceeded(layout)) =
            prepare_layout(&parsed, &options, limited_session())
        else {
            panic!("expected layout preparation to exceed {id:?}");
        };
        assert_eq!(layout, reference);
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_svg_preparation_defers_layout_edge_materialization() {
    let source = "\
---
config:
  layout: shoal
---
flowchart LR
subgraph One[Hidden group]
  A --> B
end
C enter@-->|enter| A
One@{ view: collapsed }
";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = || {
        crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap()
    };
    let options = LayoutOptions::default();
    let render = prepare(&parsed, &options, session()).unwrap();
    let BuiltinFamilyArtifact::Flowchart(flowchart) = &render.family else {
        panic!("expected Flowchart family artifact");
    };
    let layout = flowchart.pair().layout();
    assert!(layout.edges.is_empty());
    assert_eq!(layout.edge_owners.len(), 1);

    let public_layout = prepare_layout(&parsed, &options, session()).unwrap();
    let mut from_render = Vec::new();
    let mut from_layout = Vec::new();
    render
        .write_layout_geometry_json(&mut from_render, false)
        .unwrap();
    public_layout
        .write_layout_geometry_json(&mut from_layout, false)
        .unwrap();
    assert_eq!(from_render, from_layout);
    let snapshot: serde_json::Value = serde_json::from_slice(&from_render).unwrap();
    let edge = &snapshot["layout"]["FlowchartV2"]["edges"][0];
    assert_eq!(edge["id"], "enter");
    assert_eq!(edge["from"], "C");
    assert_eq!(edge["to"], "One");
}

#[cfg(feature = "layout-shoal")]
mod title_fixtures;

#[cfg(feature = "layout-shoal")]
fn render_struck_title(source: &str, group: &str, theme: Option<&[u8]>) -> RenderedFamilySvg {
    use crate::graph_layout::RouteContract;
    use crate::theme::{NativeTheme, ThemeCompiler};

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let environment = crate::environment::RenderEnvironment::deterministic();
    let session = if let Some(theme) = theme {
        let theme = ThemeCompiler::new().compile_json(theme, None).unwrap();
        environment.begin_session_with_native_theme(NativeTheme::new(theme))
    } else {
        environment.begin_session()
    }
    .unwrap();
    let mut artifact = prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let flowchart = match &mut artifact.family {
        BuiltinFamilyArtifact::Flowchart(flowchart) => flowchart,
        #[cfg(feature = "diagram-agentflow")]
        BuiltinFamilyArtifact::Agentflow { flow, .. } => flow,
        _ => panic!("expected a Flowchart layout"),
    };
    let RouteContract::CanonicalAxis(routes) = &mut flowchart.pair.layout.route_contract else {
        panic!("expected Shoal axes");
    };
    // Inject the solver's unavoidable-title result into a fresh artifact at the paint boundary.
    assert!(
        Arc::get_mut(routes)
            .unwrap()
            .struck_group_titles
            .insert(group.to_owned())
    );
    artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap()
}

#[cfg(feature = "layout-shoal")]
fn assert_struck_title_overlay<'a>(
    doc: &'a roxmltree::Document<'a>,
    fill: &str,
) -> (roxmltree::Node<'a, 'a>, roxmltree::Node<'a, 'a>) {
    let mut titles = doc
        .descendants()
        .filter(|node| node.attribute("data-merman-title-struck") == Some("true"));
    let title = titles.next().expect("halo overlay");
    assert!(titles.next().is_none());
    assert_eq!(title.attribute("paint-order"), Some("stroke"));
    assert_eq!(title.attribute("stroke"), Some(fill));
    assert_eq!(title.attribute("stroke-width"), Some("5"));
    let id = title.attribute("data-merman-cluster-title-for").unwrap();
    let mut groups = doc
        .descendants()
        .filter(|node| node.attribute("id") == Some(id));
    let group = groups.next().unwrap();
    assert!(groups.next().is_none());
    assert_eq!(group.attribute("data-et"), Some("cluster"));
    assert_eq!(group.attribute("data-id"), Some("G"));
    let outline = group
        .children()
        .find(|node| node.has_tag_name("rect"))
        .unwrap();
    let edge = doc
        .descendants()
        .find(|node| {
            node.attribute("data-et") == Some("edge") && node.attribute("data-id") == Some("trunk")
        })
        .unwrap();
    assert!(outline.range().start < edge.range().start && edge.range().start < title.range().start);
    assert!(title.descendants().any(|node| node.has_tag_name("text")));
    (title, outline)
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_unavoidable_title_halo_uses_the_composite_fill_in_svg_and_resvg() {
    for (keyword, group_keyword) in [
        ("flowchart", "subgraph"),
        #[cfg(feature = "diagram-agentflow")]
        ("agentflow-beta", "flow"),
    ] {
        for html in [false, true] {
            let source = title_fixtures::nested_title(keyword, group_keyword, html, true);
            let rendered = render_struck_title(&source, "G", Some(title_fixtures::PAPER_FLAT));
            let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
            let (title, _) = assert_struck_title_overlay(&doc, "#7F0080");
            let foreign = title
                .descendants()
                .find(|node| node.has_tag_name("foreignObject"));
            assert_eq!(foreign.is_some(), html);
            assert_eq!(
                title.descendants().any(|node| node.has_tag_name("switch")),
                html
            );
            if let Some(foreign) = foreign {
                let span = foreign
                    .descendants()
                    .find(|node| node.attribute("class") == Some("nodeLabel"))
                    .unwrap();
                assert!(
                    span.attribute("style")
                        .unwrap()
                        .contains("-webkit-text-stroke:5px #7F0080")
                );
            }
            let finalized = rendered.finalize_resvg(&SvgPipeline::resvg_safe()).unwrap();
            let doc = roxmltree::Document::parse(finalized.svg().as_str()).unwrap();
            let (title, _) = assert_struck_title_overlay(&doc, "#7F0080");
            assert!(
                !title
                    .descendants()
                    .any(|node| node.has_tag_name("switch") || node.has_tag_name("foreignObject"))
            );
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_unavoidable_title_halo_matches_the_compatibility_cluster_fill() {
    for (keyword, group_keyword, fill) in [
        ("flowchart", "subgraph", "#F9F9FB"),
        #[cfg(feature = "diagram-agentflow")]
        ("agentflow-beta", "flow", "#EEF2FF"),
    ] {
        let source = title_fixtures::nested_title(keyword, group_keyword, false, false);
        let rendered = render_struck_title(&source, "G", None);
        let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
        let (_, outline) = assert_struck_title_overlay(&doc, fill);
        assert!(
            outline
                .attribute("style")
                .unwrap()
                .split(';')
                .filter_map(|decl| decl.split_once(':'))
                .any(|(property, value)| property == "fill" && value == fill)
        );
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_unavoidable_dark_title_halo_preserves_authored_text_color_in_resvg() {
    let rendered = render_struck_title(
        title_fixtures::DARK_CLUSTER,
        "prod",
        Some(title_fixtures::STUDIO),
    );
    let finalized = rendered.finalize_resvg(&SvgPipeline::resvg_safe()).unwrap();
    let doc = roxmltree::Document::parse(finalized.svg().as_str()).unwrap();
    let title = doc
        .descendants()
        .find(|node| node.attribute("data-merman-title-struck") == Some("true"))
        .unwrap();
    assert!(
        !title
            .descendants()
            .any(|node| node.has_tag_name("switch") || node.has_tag_name("foreignObject"))
    );
    let text = title
        .descendants()
        .find(|node| node.has_tag_name("text"))
        .expect("native title fallback");
    assert!(
        text.attribute("style")
            .unwrap()
            .contains("fill:#ffffff !important")
    );
}
