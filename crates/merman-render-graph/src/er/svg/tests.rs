use super::*;
use crate::er::labels::{measure_entity_box, prepare_er_labels};
use indexmap::IndexMap;
use merman_model::{
    MermaidConfig, family::DiagramFamilyId, presentation::PresentationConfig,
    resources::InputResourcePolicy,
};
use merman_render_core::{
    LayoutNode, resources::RenderResourcePolicy, text::DeterministicTextMeasurer,
};
use roughr::core::{RoughJsSeed, RoughMathRandom};
use serde_json::json;

#[test]
fn er_unstyled_unwrapped_svg_labels_preserve_terminal_bytes() {
    let config = json!({ "htmlLabels": false, "flowchart": { "wrappingWidth": 1000 } });
    let settings = ErConfigView::new(&config).render_settings_with_resolved_typography(None, None);
    let measurer = DeterministicTextMeasurer::default();
    for source in [
        "Account",
        "**Account** *owner*",
        "A&amp;lt;br&amp;gt;B",
        "First<br/>Last",
    ] {
        let entity = ErEntity {
            id: "E".to_owned(),
            label: source.to_owned(),
            ..Default::default()
        };
        let measured = measure_entity_box(
            &entity,
            &measurer,
            &settings.label_style,
            &settings.label_style,
            settings.entity_measurement,
        );
        let mut actual = String::new();
        super::write_er_svg_box_label(&mut actual, &measured.label);
        let mut previous = String::new();
        label::write_svg_text_markdown_from_create_text_source(
            &mut previous,
            measured.label.svg_text_source(),
            true,
        );
        assert_eq!(actual, previous, "unchanged unwrapped label: {source}");
    }
}

#[test]
fn er_redux_colors_follow_entity_insertion_ids() {
    let config = json!({
        "theme": "redux-color",
        "themeVariables": {
            "borderColorArray": ["#e879f9", "#2dd4bf"],
            "bkgColorArray": ["#fdf4ff", "#f0fdfa"],
            "THEME_COLOR_LIMIT": 2
        }
    });
    let zeta = ErEntity {
        id: "entity-ZETA-0".to_string(),
        label: "ZETA".to_string(),
        shape: "erBox".to_string(),
        css_classes: "default".to_string(),
        ..ErEntity::default()
    };
    let alpha = ErEntity {
        id: "entity-ALPHA-1".to_string(),
        label: "ALPHA".to_string(),
        shape: "erBox".to_string(),
        css_classes: "default".to_string(),
        ..ErEntity::default()
    };
    let model = ErDiagramModel {
        direction: "TB".to_string(),
        entities: IndexMap::from([
            ("ALPHA".to_string(), alpha.clone()),
            ("ZETA".to_string(), zeta.clone()),
        ]),
        ..ErDiagramModel::default()
    };
    let color_indices = super::er_color_indices(&model);
    assert_eq!(color_indices.get(&zeta.id), Some(&0));
    assert_eq!(color_indices.get(&alpha.id), Some(&1));
    let measurer = DeterministicTextMeasurer::default();
    let settings = ErConfigView::new(&config).render_settings_with_resolved_typography(None, None);
    let measure = |entity: &ErEntity| {
        measure_entity_box(
            entity,
            &measurer,
            &settings.label_style,
            &settings.label_style,
            settings.entity_measurement,
        )
    };
    let zeta_measure = measure(&zeta);
    let alpha_measure = measure(&alpha);
    let nodes = [
        // Deliberately reverse the layout vector. Mermaid colorIndex belongs to the entity,
        // not to whichever order the layout backend returns nodes in.
        LayoutNode {
            id: alpha.id.clone(),
            x: 220.0,
            y: 80.0,
            width: alpha_measure.width,
            height: alpha_measure.height,
            is_cluster: false,
            label_width: None,
            label_height: None,
        },
        LayoutNode {
            id: zeta.id.clone(),
            x: 80.0,
            y: 80.0,
            width: zeta_measure.width,
            height: zeta_measure.height,
            is_cluster: false,
            label_width: None,
            label_height: None,
        },
    ];
    let prepared_labels = prepare_er_labels(&model, &config, &measurer, false);
    let work_meter = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    let effective_config = PresentationConfig::from_config_controlled(
        MermaidConfig::from_value(config),
        &InputResourcePolicy::default(),
        work_meter.control(),
    )
    .unwrap()
    .unwrap();
    let config = effective_config.as_value();
    let view = ErConfigView::new(config);
    let relationship_html_labels = view.relationship_html_labels(false);
    let layout = || ErPreparedSvgLayout {
        geometry: ErSvgLayoutView {
            clusters: &[],
            nodes: &nodes,
            edges: &[],
            render_edges: &[],
        },
        swimlane: None,
        prepared_labels: &prepared_labels,
    };
    let entity_theme = ErEntityThemePlan::resolve(
        &effective_config,
        None,
        relationship_html_labels,
        &model,
        layout(),
        &work_meter,
    )
    .expect("resolve baseline ER entity theme");
    let diagram_id = SvgId::new("er-colors").unwrap();
    let rough_randomness = RoughRandomness::new(RoughJsSeed::new(1.0), RoughMathRandom::new(1));
    let context = ErSvgContext {
        diagram_id,
        viewport: root_svg::RootViewportContext::new(DiagramFamilyId::ER, diagram_id),
        canvas_background: None,
        canvas_has_texture: false,
        elk_parity: false,
        swimlane: false,
        #[cfg(feature = "layout-shoal")]
        canonical_routes: None,
        work_meter: &work_meter,
        rough_randomness: &rough_randomness,
        include_edges: true,
        viewbox_padding: 8.0,
    };
    let svg = render_er_diagram_svg_model(
        layout(),
        &model,
        &entity_theme,
        &effective_config,
        None,
        &measurer,
        &context,
    )
    .and_then(|svg| svg.into_string_for(DiagramFamilyId::ER))
    .unwrap();

    let zeta_dom = r#"<g id="er-colors-entity-ZETA-0" class="node default" data-look="classic" data-color-id="color-0""#;
    let alpha_dom = r#"<g id="er-colors-entity-ALPHA-1" class="node default" data-look="classic" data-color-id="color-1""#;
    assert!(svg.contains(zeta_dom), "{svg}");
    assert!(svg.contains(alpha_dom), "{svg}");
    assert!(svg.find(zeta_dom).unwrap() < svg.find(alpha_dom).unwrap());
}
