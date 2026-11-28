//! Supplies the selected canonical route and facade environment to the graph writer.

use super::super::SvgExecution;
use crate::{DiagramFamilyId, Result};
use merman_model::families::requirement::RequirementDiagramModel;
#[cfg(feature = "layout-shoal")]
use merman_render_core::{
    Bounds, rough_svg::constrained_rough_edge_path, svg_bounds::svg_path_bounds_from_d,
};
use merman_render_core::{root_svg, text::TextMeasurer};
use merman_render_graph::requirement as graph;

#[cfg(feature = "layout-shoal")]
fn paint_canonical_edge(
    routes: &crate::graph_layout::canonical::CanonicalRoutes,
    edge: &crate::model::LayoutEdge,
    stroke_outset: f64,
    edge_style: Option<merman_theme_contract::v2::EdgeStyle>,
    hand_drawn: bool,
    work_meter: &crate::resources::OperationWorkMeter,
    randomness: &roughr::core::RoughRandomness,
) -> Result<(String, graph::RequirementCanonicalEdge)> {
    use crate::graph_layout::route_paint::RouteStyle;
    use kurbo::{Point, Rect, Vec2};

    work_meter.charge(edge.points.len())?;
    use merman_theme_contract::v2::EdgeStyle;
    let style = edge_style.map_or(RouteStyle::Curved, RouteStyle::from_theme);
    let geometry = routes.paint(edge, Vec2::ZERO, stroke_outset, style, work_meter)?;
    let (d, shaft) = if hand_drawn || edge_style == Some(EdgeStyle::Sketch) {
        let d = constrained_rough_edge_path(
            &geometry.path.d,
            geometry.markers,
            work_meter,
            randomness,
        )?;
        work_meter.charge(d.len())?;
        let bounds = svg_path_bounds_from_d(&d).ok_or_else(|| crate::Error::InvalidModel {
            message: format!(
                "Requirement edge `{}` has no hand-drawn paint bounds",
                edge.id
            ),
        })?;
        (
            d,
            Rect::new(bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y),
        )
    } else {
        let bounds = geometry.path.bounds;
        (
            geometry.path.d,
            Rect::new(bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y),
        )
    };
    let outset = stroke_outset * std::f64::consts::SQRT_2;
    let mut bounds = shaft.inflate(outset, outset);
    let [start_ref, start_lead, end_lead, end_ref] = geometry.marker_points;
    for (marker, reference, lead) in [
        (geometry.markers.0, start_ref, start_lead),
        (geometry.markers.1, end_ref, end_lead),
    ] {
        if marker.depth > 0.0 {
            let reference = Point::new(reference.x, reference.y);
            let normal = (Point::new(lead.x, lead.y) - reference).normalize();
            let across = Vec2::new(-normal.y, normal.x) * (marker.width / 2.0);
            for axial_distance in [-marker.trim, marker.depth] {
                let center = reference + normal * axial_distance;
                bounds = bounds.union_pt(center + across).union_pt(center - across);
            }
        }
    }
    Ok((
        d,
        graph::RequirementCanonicalEdge {
            points: geometry.data_points,
            bounds: Bounds {
                min_x: bounds.x0,
                min_y: bounds.y0,
                max_x: bounds.x1,
                max_y: bounds.y1,
            },
        },
    ))
}

pub(crate) fn render_requirement_diagram_svg_model(
    prepared: &crate::requirement::RequirementPreparedArtifact,
    model: &RequirementDiagramModel,
    paint_theme: &crate::requirement::RequirementPaintThemePlan,
    sanitize_config: &merman_model::presentation::PresentationConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("requirement");
    let work_meter = options.work_meter();
    let configured_seed =
        graph::RequirementConfigView::new(sanitize_config.as_value()).hand_drawn_seed();
    let randomness = options.rough_randomness(configured_seed, "render.requirement.roughjs");
    #[cfg(feature = "layout-shoal")]
    let canonical_routes = prepared.canonical_routes().map(|routes| {
        let randomness = &randomness;
        let hand_drawn = merman_render_core::config::mermaid_config_diagram_look(sanitize_config)
            .is_hand_drawn();
        move |edge: &merman_render_core::LayoutEdge, stroke_outset, edge_style| {
            paint_canonical_edge(
                routes,
                edge,
                stroke_outset,
                edge_style,
                hand_drawn,
                work_meter,
                randomness,
            )
        }
    });
    let context = graph::RequirementSvgContext {
        diagram_id,
        viewport: options.root_viewport(DiagramFamilyId::REQUIREMENT, diagram_id),
        work_meter,
        rough_randomness: &randomness,
        #[cfg(feature = "layout-shoal")]
        canonical_routes: canonical_routes
            .as_ref()
            .map(|paint| paint as &graph::RequirementCanonicalEdgePainter<'_>),
    };
    graph::render_requirement_diagram_svg_model(
        &prepared.prepared,
        model,
        paint_theme,
        sanitize_config,
        diagram_title,
        measurer,
        &context,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "layout-dagre")]
    use crate::environment::{RenderEnvironment, TextMeasurementPhase};
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};
    #[cfg(feature = "layout-dagre")]
    use crate::resources::{ResourceLimitCause, ResourceLimitPhase};
    #[cfg(feature = "layout-dagre")]
    use crate::svg::{SvgDebugOptions, SvgExecution};
    #[cfg(any(feature = "layout-dagre", feature = "layout-shoal"))]
    use crate::svg::{SvgRenderOptions, with_test_svg_execution};
    #[cfg(any(feature = "layout-dagre", feature = "layout-shoal"))]
    use crate::text::{DeterministicTextMeasurer, TextMeasurer};
    #[cfg(feature = "layout-dagre")]
    use crate::text::{TextMetrics, TextStyle, WrapMode};
    #[cfg(feature = "layout-dagre")]
    use merman_model::families::requirement::RequirementElement;
    #[cfg(any(feature = "layout-dagre", feature = "layout-shoal"))]
    use merman_model::families::requirement::{
        RequirementDiagramModel, RequirementNode, RequirementRelationship,
    };
    #[cfg(feature = "layout-dagre")]
    use merman_render_graph::requirement::RequirementEdgeIdentity;
    #[cfg(feature = "layout-dagre")]
    use std::cell::Cell;
    #[cfg(any(feature = "layout-dagre", feature = "layout-shoal"))]
    use std::collections::BTreeMap;

    #[cfg(feature = "layout-dagre")]
    fn report_call_count(session: &crate::environment::RenderSession) -> u64 {
        session
            .text_measurement_report()
            .entries()
            .iter()
            .map(crate::environment::TextMeasurementSummary::count)
            .sum()
    }

    #[cfg(feature = "layout-dagre")]
    #[derive(Default)]
    struct CountingRequirementMeasurer {
        inner: DeterministicTextMeasurer,
        mermaid_dimensions: Cell<usize>,
        wrapped: Cell<usize>,
    }

    #[cfg(feature = "layout-dagre")]
    impl TextMeasurer for CountingRequirementMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.inner.measure(text, style)
        }

        fn measure_mermaid_calculate_text_dimensions(
            &self,
            text: &str,
            style: &TextStyle,
        ) -> TextMetrics {
            self.mermaid_dimensions
                .set(self.mermaid_dimensions.get() + 1);
            self.inner
                .measure_mermaid_calculate_text_dimensions(text, style)
        }

        fn measure_wrapped(
            &self,
            text: &str,
            style: &TextStyle,
            max_width: Option<f64>,
            wrap_mode: WrapMode,
        ) -> TextMetrics {
            self.wrapped.set(self.wrapped.get() + 1);
            self.inner
                .measure_wrapped(text, style, max_width, wrap_mode)
        }
    }

    #[cfg(any(feature = "layout-dagre", feature = "layout-shoal"))]
    fn empty_requirement_model() -> RequirementDiagramModel {
        RequirementDiagramModel {
            acc_title: None,
            acc_descr: None,
            direction: String::new(),
            requirements: Vec::new(),
            elements: Vec::new(),
            relationships: Vec::new(),
            classes: BTreeMap::new(),
        }
    }

    #[cfg(feature = "layout-dagre")]
    fn prepared_requirement_model() -> RequirementDiagramModel {
        RequirementDiagramModel {
            requirements: vec![RequirementNode {
                name: "requirement-a".to_string(),
                node_type: "Requirement".to_string(),
                requirement_id: "REQ-1".to_string(),
                text: "The **prepared** requirement".to_string(),
                risk: "Low".to_string(),
                verify_method: "Test".to_string(),
                css_styles: Vec::new(),
                classes: Vec::new(),
            }],
            elements: vec![RequirementElement {
                name: "element-b".to_string(),
                element_type: "System".to_string(),
                doc_ref: "DOC-1".to_string(),
                css_styles: Vec::new(),
                classes: Vec::new(),
            }],
            relationships: vec![RequirementRelationship {
                rel_type: "satisfies".to_string(),
                src: "element-b".to_string(),
                dst: "requirement-a".to_string(),
            }],
            ..empty_requirement_model()
        }
    }

    #[cfg(any(feature = "layout-dagre", feature = "layout-shoal"))]
    fn requirement_node(name: &str) -> RequirementNode {
        RequirementNode {
            name: name.to_string(),
            node_type: "Requirement".to_string(),
            requirement_id: String::new(),
            text: String::new(),
            risk: String::new(),
            verify_method: String::new(),
            css_styles: Vec::new(),
            classes: Vec::new(),
        }
    }

    #[cfg(feature = "layout-dagre")]
    #[test]
    fn requirement_redux_colors_follow_requirement_then_element_order() {
        let model = RequirementDiagramModel {
            requirements: vec![
                RequirementNode {
                    name: "requirement-a".to_string(),
                    node_type: "Requirement".to_string(),
                    requirement_id: String::new(),
                    text: String::new(),
                    risk: String::new(),
                    verify_method: String::new(),
                    css_styles: Vec::new(),
                    classes: Vec::new(),
                },
                RequirementNode {
                    name: "requirement-b".to_string(),
                    node_type: "Requirement".to_string(),
                    requirement_id: String::new(),
                    text: String::new(),
                    risk: String::new(),
                    verify_method: String::new(),
                    css_styles: Vec::new(),
                    classes: Vec::new(),
                },
            ],
            elements: vec![RequirementElement {
                name: "element-c".to_string(),
                element_type: "Element".to_string(),
                doc_ref: String::new(),
                css_styles: Vec::new(),
                classes: Vec::new(),
            }],
            ..empty_requirement_model()
        };
        let config = serde_json::json!({
            "theme": "redux-color",
            "themeVariables": {
                "borderColorArray": ["#e879f9", "#2dd4bf", "#fb923c"],
                "bkgColorArray": ["#fdf4ff", "#f0fdfa", "#fff7ed"],
                "THEME_COLOR_LIMIT": 3
            }
        });
        let options = SvgRenderOptions {
            diagram_id: Some("requirement-colors".to_string()),
            ..SvgRenderOptions::default()
        };
        let svg = render_requirement_for_test(
            &model,
            &config,
            None,
            &crate::text::DeterministicTextMeasurer::default(),
            &options,
        )
        .unwrap();

        assert!(svg.contains(
            r#"id="requirement-colors-requirement-a" data-look="classic" data-color-id="color-0""#
        ));
        assert!(svg.contains(
            r#"id="requirement-colors-requirement-b" data-look="classic" data-color-id="color-1""#
        ));
        assert!(svg.contains(
            r#"id="requirement-colors-element-c" data-look="classic" data-color-id="color-2""#
        ));
    }

    #[cfg(feature = "layout-dagre")]
    fn root_view_box(svg: &str) -> Vec<f64> {
        let root_open = svg.split_once('>').expect("root svg open tag").0;
        let value = root_open
            .split_once("viewBox=\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(value, _)| value)
            .expect("root viewBox");
        value
            .split_whitespace()
            .map(|part| part.parse::<f64>().expect("numeric viewBox part"))
            .collect()
    }

    #[cfg(feature = "layout-dagre")]
    fn render_requirement_for_test(
        model: &RequirementDiagramModel,
        effective_config: &serde_json::Value,
        diagram_title: Option<&str>,
        measurer: &dyn TextMeasurer,
        request: &SvgRenderOptions,
    ) -> crate::Result<String> {
        let effective_config = crate::test_support::presentation_config(effective_config.clone());
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            model,
            effective_config.as_value(),
            measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )?;
        render_prepared_requirement_for_test(
            &prepared,
            model,
            &effective_config,
            diagram_title,
            measurer,
            request,
        )
    }

    #[cfg(any(feature = "layout-dagre", feature = "layout-shoal"))]
    fn render_prepared_requirement_for_test(
        prepared: &crate::requirement::RequirementPreparedArtifact,
        model: &RequirementDiagramModel,
        effective_config: &merman_model::presentation::PresentationConfig,
        diagram_title: Option<&str>,
        measurer: &dyn TextMeasurer,
        request: &SvgRenderOptions,
    ) -> crate::Result<String> {
        with_test_svg_execution(DiagramFamilyId::REQUIREMENT, request, |options| {
            let paint_theme = crate::requirement::RequirementPaintThemePlan::resolve(
                None,
                effective_config,
                model,
                options.work_meter(),
            )?;
            render_requirement_diagram_svg_model(
                prepared,
                model,
                &paint_theme,
                effective_config,
                diagram_title,
                measurer,
                options,
            )
        })
        .and_then(|svg| svg.into_string_for(DiagramFamilyId::REQUIREMENT))
    }

    #[cfg(feature = "layout-dagre")]
    fn render_requirement_with_policy(policy: RenderResourcePolicy) -> crate::Result<String> {
        let model = prepared_requirement_model();
        let effective_config = crate::test_support::presentation_config(serde_json::json!({}));
        let measurer = DeterministicTextMeasurer::default();
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            effective_config.as_value(),
            &measurer,
            policy,
        )?;
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session()
            .expect("render session");
        let request = SvgRenderOptions {
            diagram_id: Some("requirement-bounded".to_string()),
            ..SvgRenderOptions::default()
        };
        let debug = SvgDebugOptions::default();
        let execution = SvgExecution::unthemed_for_test(
            &request,
            &debug,
            &session,
            DiagramFamilyId::REQUIREMENT,
        )
        .expect("SVG execution");
        let paint_theme = crate::requirement::RequirementPaintThemePlan::resolve(
            None,
            &effective_config,
            &model,
            execution.work_meter(),
        )?;

        render_requirement_diagram_svg_model(
            &prepared,
            &model,
            &paint_theme,
            &effective_config,
            None,
            &measurer,
            &execution,
        )?
        .into_string_for(DiagramFamilyId::REQUIREMENT)
    }

    #[cfg(feature = "layout-dagre")]
    #[test]
    fn requirement_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
        let baseline =
            render_requirement_with_policy(RenderResourcePolicy::unbounded_for_trusted_input())
                .expect("render unbounded Requirement SVG");
        let exact_bytes = baseline.len();
        assert!(exact_bytes > 1);

        let exact = render_requirement_with_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
                .expect("valid exact Requirement SVG ceiling"),
        )
        .expect("exact Requirement SVG ceiling must succeed");
        assert_eq!(exact.as_bytes(), baseline.as_bytes());

        let below_exact = exact_bytes - 1;
        let error = render_requirement_with_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
                .expect("valid below-exact Requirement SVG ceiling"),
        )
        .expect_err("one byte below the Requirement SVG size must fail");
        let crate::Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected Requirement MaxSvgBytes rejection, got {error}");
        };
        assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
        assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
        assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
        assert_eq!(limit.max, below_exact);
        assert!(limit.actual > limit.max);
    }

    #[cfg(feature = "layout-dagre")]
    #[test]
    fn requirement_opaque_measurer_preserves_render_stage_label_measurements() {
        let model = prepared_requirement_model();
        let config = crate::test_support::presentation_config(serde_json::json!({}));
        let measurer = CountingRequirementMeasurer::default();
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            config.as_value(),
            &measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        let dimensions_after_prepare = measurer.mermaid_dimensions.get();
        let wrapped_after_prepare = measurer.wrapped.get();

        assert!(dimensions_after_prepare > 0);
        assert!(wrapped_after_prepare > 0);

        render_prepared_requirement_for_test(
            &prepared,
            &model,
            &config,
            None,
            &measurer,
            &SvgRenderOptions::default(),
        )
        .unwrap();

        assert_eq!(
            measurer.mermaid_dimensions.get() - dimensions_after_prepare,
            dimensions_after_prepare
        );
        assert_eq!(
            measurer.wrapped.get() - wrapped_after_prepare,
            wrapped_after_prepare
        );
    }

    #[cfg(feature = "layout-dagre")]
    #[test]
    fn requirement_routed_builtin_measurements_are_reused_during_svg_emission() {
        let model = prepared_requirement_model();
        let config = crate::test_support::presentation_config(serde_json::json!({}));
        let environment = RenderEnvironment::deterministic().with_resource_policy(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let session = environment.begin_session().expect("deterministic session");
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            config.as_value(),
            &measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        let calls_after_prepare = report_call_count(&session);

        assert!(calls_after_prepare > 0);

        render_prepared_requirement_for_test(
            &prepared,
            &model,
            &config,
            None,
            &measurer,
            &SvgRenderOptions::default(),
        )
        .unwrap();

        assert_eq!(report_call_count(&session), calls_after_prepare);
    }

    #[cfg(feature = "layout-dagre")]
    #[test]
    fn requirement_prepared_measurements_are_not_reused_across_style_changes() {
        let model = prepared_requirement_model();
        let prepare_config = crate::test_support::presentation_config(serde_json::json!({}));
        let render_config = crate::test_support::presentation_config(serde_json::json!({
            "themeVariables": {
                "fontFamily": "Courier New",
                "fontSize": "24px"
            }
        }));
        let environment = RenderEnvironment::deterministic().with_resource_policy(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let session = environment.begin_session().expect("deterministic session");
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            prepare_config.as_value(),
            &measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        let calls_after_prepare = report_call_count(&session);

        render_prepared_requirement_for_test(
            &prepared,
            &model,
            &render_config,
            None,
            &measurer,
            &SvgRenderOptions::default(),
        )
        .unwrap();

        assert!(report_call_count(&session) > calls_after_prepare);
    }

    #[cfg(feature = "layout-dagre")]
    #[test]
    fn requirement_prepared_measurements_are_not_reused_across_label_modes() {
        let model = prepared_requirement_model();
        let prepare_config =
            crate::test_support::presentation_config(serde_json::json!({"htmlLabels":true}));
        let render_config =
            crate::test_support::presentation_config(serde_json::json!({"htmlLabels":false}));
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            )
            .begin_session()
            .unwrap();
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            prepare_config.as_value(),
            &measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        let calls_after_prepare = report_call_count(&session);
        let svg = render_prepared_requirement_for_test(
            &prepared,
            &model,
            &render_config,
            None,
            &measurer,
            &SvgRenderOptions::default(),
        )
        .unwrap();
        assert!(report_call_count(&session) > calls_after_prepare);
        assert!(!svg.contains("foreignObject"));
    }

    #[cfg(feature = "layout-dagre")]
    #[test]
    fn requirement_prepared_labels_keep_markdown_and_strict_sanitization_at_render_time() {
        let mut model = prepared_requirement_model();
        model.requirements[0].text = concat!(
            "**safe** ",
            r#"<a href="jav&#x61;script:alert(1)" onclick="alert(2)">bad</a>"#,
            "<script>alert(3)</script>"
        )
        .to_string();
        let measurer = DeterministicTextMeasurer::default();
        for html_labels in [true, false] {
            let config = crate::test_support::presentation_config(serde_json::json!({
                "securityLevel": "strict", "htmlLabels": html_labels
            }));
            let prepared =
                crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
                    &model,
                    config.as_value(),
                    &measurer,
                    crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
                )
                .unwrap();

            let svg = render_prepared_requirement_for_test(
                &prepared,
                &model,
                &config,
                None,
                &measurer,
                &SvgRenderOptions::default(),
            )
            .unwrap();

            if html_labels {
                assert!(svg.contains("<strong>safe</strong>"), "{svg}");
            } else {
                let document = crate::xml_dom::Document::parse(&svg).expect("valid SVG labels");
                assert!(
                    !document
                        .descendants()
                        .any(|node| node.has_tag_name("foreignObject"))
                );
                assert!(document.descendants().any(|node| node.has_tag_name("tspan")
                    && node.attribute("font-weight") == Some("bold")
                    && node.text().is_some_and(|text| text.trim() == "safe")));
            }
            assert!(!svg.contains("<script"), "{svg}");
            assert!(!svg.contains("onclick="), "{svg}");
            assert!(!svg.contains("<a href="), "{svg}");
        }
    }

    #[cfg(feature = "layout-dagre")]
    #[test]
    fn requirement_prepared_edge_plan_keeps_parallel_relationships() {
        let mut model = prepared_requirement_model();
        model.relationships = vec![
            RequirementRelationship {
                rel_type: "satisfies".to_string(),
                src: "element-b".to_string(),
                dst: "requirement-a".to_string(),
            },
            RequirementRelationship {
                rel_type: "contains".to_string(),
                src: "element-b".to_string(),
                dst: "requirement-a".to_string(),
            },
        ];
        let config = crate::test_support::presentation_config(serde_json::json!({}));
        let measurer = DeterministicTextMeasurer::default();
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            config.as_value(),
            &measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();

        let svg = render_prepared_requirement_for_test(
            &prepared,
            &model,
            &config,
            None,
            &measurer,
            &SvgRenderOptions::default(),
        )
        .unwrap();

        assert!(svg.contains("&lt;&lt;contains&gt;&gt;"), "{svg}");
        assert!(svg.contains("&lt;&lt;satisfies&gt;&gt;"), "{svg}");
        assert!(svg.contains("edge-pattern-solid relationshipLine"), "{svg}");
        assert!(
            svg.contains("edge-pattern-dashed relationshipLine"),
            "{svg}"
        );
        assert!(
            svg.contains(r#"data-id="element-b-requirement-a-0""#),
            "{svg}"
        );
        assert!(
            svg.contains(r#"data-id="element-b-requirement-a-1""#),
            "{svg}"
        );
    }

    #[cfg(feature = "layout-dagre")]
    #[test]
    fn requirement_prepared_edge_plan_disambiguates_colliding_endpoint_text() {
        let model = RequirementDiagramModel {
            requirements: vec![
                requirement_node("a-b"),
                requirement_node("c"),
                requirement_node("a"),
                requirement_node("b-c"),
            ],
            relationships: vec![
                RequirementRelationship {
                    rel_type: "contains".to_string(),
                    src: "a-b".to_string(),
                    dst: "c".to_string(),
                },
                RequirementRelationship {
                    rel_type: "satisfies".to_string(),
                    src: "a".to_string(),
                    dst: "b-c".to_string(),
                },
            ],
            ..empty_requirement_model()
        };
        let config = crate::test_support::presentation_config(serde_json::json!({}));
        let measurer = DeterministicTextMeasurer::default();
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            config.as_value(),
            &measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();

        let (layout, _, edge_plans) = prepared.prepared.render_parts();
        assert_eq!(layout.edges.len(), 2);
        for edge in &layout.edges {
            let identity = RequirementEdgeIdentity::from(edge);
            let label_plan = edge_plans.get(&identity).expect("prepared edge identity");
            match (edge.from.as_str(), edge.to.as_str()) {
                ("a-b", "c") => {
                    assert_eq!(edge.id, "a-b-c-0");
                    assert_eq!(label_plan.relationship_type, "contains");
                }
                ("a", "b-c") => {
                    assert_eq!(edge.id, "a-b-c-1");
                    assert_eq!(label_plan.relationship_type, "satisfies");
                }
                endpoints => panic!("unexpected Requirement edge: {endpoints:?}"),
            }
        }

        let svg = render_prepared_requirement_for_test(
            &prepared,
            &model,
            &config,
            None,
            &measurer,
            &SvgRenderOptions::default(),
        )
        .unwrap();

        assert_eq!(svg.matches("&lt;&lt;contains&gt;&gt;").count(), 1, "{svg}");
        assert_eq!(svg.matches("&lt;&lt;satisfies&gt;&gt;").count(), 1, "{svg}");
        assert_eq!(
            svg.matches("edge-pattern-solid relationshipLine").count(),
            1,
            "{svg}"
        );
        assert_eq!(
            svg.matches("edge-pattern-dashed relationshipLine").count(),
            1,
            "{svg}"
        );
        assert_eq!(svg.matches(r#"marker-start="url(#"#).count(), 1, "{svg}");
        assert_eq!(svg.matches(r#"marker-end="url(#"#).count(), 1, "{svg}");

        let document = crate::xml_dom::Document::parse(&svg).expect("valid Requirement SVG");
        let edge_label_max_widths = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("div")
                    && node
                        .attribute("class")
                        .unwrap_or_default()
                        .split_whitespace()
                        .any(|class| class == "labelBkg")
            })
            .map(|node| node.attribute("style").unwrap_or_default())
            .collect::<Vec<_>>();
        assert_eq!(edge_label_max_widths.len(), 2, "{svg}");
        assert!(
            edge_label_max_widths
                .iter()
                .all(|style| style.contains("max-width: 200px;")),
            "Requirement edge labels keep Mermaid's fixed 200px wrap cap: {svg}"
        );
    }

    #[cfg(feature = "layout-dagre")]
    #[test]
    fn requirement_prepared_source_paints_reach_actual_paths_in_both_label_modes() {
        let mut node = requirement_node("source-node");
        node.css_styles = vec![
            "fill:#112233 !important".to_owned(),
            "stroke:#445566 !important".to_owned(),
            "stroke-width:2px".to_owned(),
            "color:#778899".to_owned(),
        ];
        let model = RequirementDiagramModel {
            requirements: vec![node],
            ..empty_requirement_model()
        };
        for html in [false, true] {
            let svg = render_requirement_for_test(
                &model,
                &serde_json::json!({"htmlLabels": html, "layout": "dagre"}),
                None,
                &crate::text::DeterministicTextMeasurer::default(),
                &SvgRenderOptions::default(),
            )
            .expect("render prepared source paints");
            let document = crate::xml_dom::Document::parse(&svg).expect("valid SVG");
            assert!(
                document
                    .descendants()
                    .any(|element| element.has_tag_name("path")
                        && element.attribute("fill") == Some("#112233")),
                "{svg}"
            );
            assert!(
                document
                    .descendants()
                    .any(|element| element.has_tag_name("path")
                        && element.attribute("stroke") == Some("#445566")
                        && element.attribute("stroke-width") == Some("2")),
                "{svg}"
            );
            assert!(!svg.contains("!important !important"), "{svg}");
        }
    }

    #[cfg(feature = "layout-dagre")]
    #[test]
    fn requirement_self_loop_preserves_pinned_segment_identity_and_dom() {
        let model = RequirementDiagramModel {
            requirements: vec![requirement_node("req1")],
            relationships: vec![RequirementRelationship {
                rel_type: "satisfies".to_string(),
                src: "req1".to_string(),
                dst: "req1".to_string(),
            }],
            ..empty_requirement_model()
        };
        let config = crate::test_support::presentation_config(serde_json::json!({}));
        let measurer = DeterministicTextMeasurer::default();
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            config.as_value(),
            &measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();

        let (layout, node_plans, edge_plans) = prepared.prepared.render_parts();
        assert_eq!(layout.nodes.len(), 3);
        assert!(matches!(
            node_plans.get("req1---req1---1"),
            Some(merman_render_graph::requirement::RequirementNodeRenderPlan::EdgeLabelAnchor)
        ));
        assert!(matches!(
            node_plans.get("req1---req1---2"),
            Some(merman_render_graph::requirement::RequirementNodeRenderPlan::EdgeLabelAnchor)
        ));
        assert_eq!(
            layout
                .edges
                .iter()
                .map(|edge| edge.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "req1-cyclic-special-0",
                "req1-cyclic-special-1",
                "req1-cyclic-special-2",
            ]
        );
        let rendered_segments = layout
            .edges
            .iter()
            .map(|edge| {
                let identity = RequirementEdgeIdentity::from(edge);
                let plan = edge_plans.get(&identity).expect("self-loop edge plan");
                (
                    plan.rendered_id.as_str(),
                    plan.has_label,
                    plan.marker_start,
                    plan.marker_end,
                    edge.label.is_some(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rendered_segments,
            vec![
                ("req1-cyclic-special-1", false, false, false, false),
                ("req1-cyclic-special-mid", true, false, false, true),
                ("req1-cyclic-special-2", false, false, true, false),
            ]
        );

        let options = SvgRenderOptions {
            diagram_id: Some("requirement-self-loop".to_string()),
            ..SvgRenderOptions::default()
        };
        let svg = render_prepared_requirement_for_test(
            &prepared, &model, &config, None, &measurer, &options,
        )
        .unwrap();

        for rendered_id in [
            "req1-cyclic-special-1",
            "req1-cyclic-special-mid",
            "req1-cyclic-special-2",
        ] {
            assert!(
                svg.contains(&format!(r#"data-id="{rendered_id}""#)),
                "{svg}"
            );
        }
        assert!(!svg.contains("req1-req1-0"), "{svg}");
        assert_eq!(svg.matches("&lt;&lt;satisfies&gt;&gt;").count(), 1, "{svg}");
        assert_eq!(svg.matches(r#"marker-start="url(#"#).count(), 0, "{svg}");
        assert_eq!(svg.matches(r#"marker-end="url(#"#).count(), 1, "{svg}");
        assert!(svg.contains(
            r#"<g class="edgeLabel"><g class="label" data-id="req1-cyclic-special-1" transform="translate(0, 0)">"#
        ));
        assert!(
            svg.contains(
                r#"<g class="label edgeLabel" id="req1---req1---1" transform="translate("#
            )
        );
        assert!(svg.contains(r#"<rect width="0.1" height="0.1"/>"#));

        let middle_edge = layout
            .edges
            .iter()
            .find(|edge| edge.id == "req1-cyclic-special-1")
            .expect("middle self-loop segment");
        let middle_label = middle_edge.label.as_ref().expect("middle label geometry");
        let middle_path =
            merman_render_core::svg_edge_parity::curve::curve_basis_path_d(&middle_edge.points);
        let final_position =
            merman_render_core::svg_edge_parity::edge_label_geometry::position_edge_label(
                crate::model::LayoutPoint {
                    x: middle_label.x,
                    y: middle_label.y,
                },
                Some(&middle_edge.points),
                &middle_edge.points,
                &middle_path,
                false,
            );
        let view_box = root_view_box(&svg);
        assert!(
            final_position.x - middle_label.width / 2.0 >= view_box[0]
                && final_position.x + middle_label.width / 2.0 <= view_box[0] + view_box[2]
                && final_position.y - middle_label.height / 2.0 >= view_box[1]
                && final_position.y + middle_label.height / 2.0 <= view_box[1] + view_box[3],
            "final label bounds must be inside the root viewport: {view_box:?}"
        );
    }

    #[cfg(feature = "layout-dagre")]
    #[test]
    fn requirement_contains_self_loop_keeps_only_the_first_segment_start_marker() {
        let model = RequirementDiagramModel {
            requirements: vec![requirement_node("req1")],
            relationships: vec![RequirementRelationship {
                rel_type: "contains".to_string(),
                src: "req1".to_string(),
                dst: "req1".to_string(),
            }],
            ..empty_requirement_model()
        };
        let config = crate::test_support::presentation_config(serde_json::json!({}));
        let measurer = DeterministicTextMeasurer::default();
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            config.as_value(),
            &measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        let svg = render_prepared_requirement_for_test(
            &prepared,
            &model,
            &config,
            None,
            &measurer,
            &SvgRenderOptions::default(),
        )
        .unwrap();

        assert_eq!(svg.matches(r#"marker-start="url(#"#).count(), 1, "{svg}");
        assert_eq!(svg.matches(r#"marker-end="url(#"#).count(), 0, "{svg}");
        let document = crate::xml_dom::Document::parse(&svg).expect("valid Requirement SVG");
        let path_for = |id: &str| {
            document
                .descendants()
                .find(|node| node.has_tag_name("path") && node.attribute("data-id") == Some(id))
                .unwrap_or_else(|| panic!("missing self-loop path {id}"))
        };
        assert!(
            path_for("req1-cyclic-special-1")
                .attribute("marker-start")
                .is_some_and(|value| value.ends_with("requirement_containsStart)"))
        );
        assert_eq!(
            path_for("req1-cyclic-special-mid").attribute("marker-start"),
            None
        );
        assert_eq!(
            path_for("req1-cyclic-special-2").attribute("marker-end"),
            None
        );
    }

    #[cfg(feature = "layout-shoal")]
    #[test]
    fn requirement_shoal_keeps_both_native_loop_markers_and_svg_label_anchors() {
        use kurbo::{BezPath, ParamCurve, Point};
        use merman_render_core::number_format::canonical_number as fmt;

        let model = RequirementDiagramModel {
            requirements: vec![requirement_node("req")],
            relationships: ["contains", "refines"]
                .map(|kind| RequirementRelationship {
                    rel_type: kind.into(),
                    src: "req".into(),
                    dst: "req".into(),
                })
                .into(),
            ..empty_requirement_model()
        };
        let config = crate::test_support::presentation_config(serde_json::json!({
            "layout": "shoal",
            "look": "neo",
            "htmlLabels": false,
            "themeVariables": { "strokeWidth": 2.5 }
        }));
        let measurer = DeterministicTextMeasurer::default();
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            config.as_value(),
            &measurer,
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        let svg = render_prepared_requirement_for_test(
            &prepared,
            &model,
            &config,
            None,
            &measurer,
            &SvgRenderOptions::default(),
        )
        .unwrap();
        let document = crate::xml_dom::Document::parse(&svg).unwrap();
        let layout = prepared.layout();
        assert_eq!(layout.nodes.len(), 1);
        assert_eq!(layout.edges.len(), 2);
        for (edge, marker_attr, opposite) in [
            (&layout.edges[0], "marker-start", "marker-end"),
            (&layout.edges[1], "marker-end", "marker-start"),
        ] {
            let path = document
                .descendants()
                .find(|node| {
                    node.attribute("data-et") == Some("edge")
                        && node.attribute("data-id") == Some(edge.id.as_str())
                })
                .unwrap();
            assert!(path.attribute(marker_attr).is_some());
            assert!(path.attribute(opposite).is_none());
            let label = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g") && node.attribute("data-id") == Some(edge.id.as_str())
                })
                .unwrap();
            assert!(label.descendants().any(|node| node.has_tag_name("text")));
            let anchor = edge.label.as_ref().unwrap();
            assert_eq!(
                label.parent().unwrap().attribute("transform"),
                Some(format!("translate({}, {})", fmt(anchor.x), fmt(anchor.y)).as_str())
            );
            let painted = BezPath::from_svg(path.attribute("d").unwrap()).unwrap();
            let (reference, contact, expected_setback) = if marker_attr == "marker-start" {
                (
                    painted.segments().next().unwrap().start(),
                    &edge.points[0],
                    0.0,
                )
            } else {
                let setback = (2.5 / 2.0) * (1.0 / 5.0_f64.sqrt()) * (20.0 / 25.0);
                (
                    painted.segments().last().unwrap().end(),
                    edge.points.last().unwrap(),
                    setback,
                )
            };
            let actual_setback = (reference - Point::new(contact.x, contact.y)).hypot();
            assert!((actual_setback - expected_setback).abs() < 0.002);
        }
    }

    #[cfg(feature = "layout-dagre")]
    #[test]
    fn requirement_root_honors_disabled_max_width() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let options = SvgRenderOptions {
            diagram_id: Some("requirementFixed".to_string()),
            ..SvgRenderOptions::default()
        };

        let svg = render_requirement_for_test(
            &empty_requirement_model(),
            &serde_json::json!({"state": {"useMaxWidth": false}}),
            None,
            &measurer,
            &options,
        )
        .unwrap();
        let root_open = svg.split_once('>').expect("root svg open tag").0;

        assert!(root_open.contains(r#"width="16""#), "{root_open}");
        assert!(root_open.contains(r#"height="16""#), "{root_open}");
        assert!(
            root_open.contains(r#"viewBox="-8 -8 16 16""#),
            "{root_open}"
        );
        assert!(!root_open.contains("style="), "{root_open}");
        assert!(!root_open.contains("max-width"), "{root_open}");
    }

    #[cfg(feature = "layout-dagre")]
    #[test]
    fn requirement_root_is_derived_for_formerly_pinned_fixture_ids() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let pinned_options = SvgRenderOptions {
            diagram_id: Some(
                "upstream_cypress_requirementdiagram_unified_spec_example_025".to_string(),
            ),
            ..SvgRenderOptions::default()
        };
        let control_options = SvgRenderOptions {
            diagram_id: Some("requirement-control".to_string()),
            ..SvgRenderOptions::default()
        };
        let model = prepared_requirement_model();
        let config = serde_json::json!({});

        let pinned_svg =
            render_requirement_for_test(&model, &config, None, &measurer, &pinned_options).unwrap();
        let control_svg =
            render_requirement_for_test(&model, &config, None, &measurer, &control_options)
                .unwrap();
        let root_open = pinned_svg.split_once('>').expect("root svg open tag").0;

        assert_eq!(root_view_box(&pinned_svg), root_view_box(&control_svg));
        assert!(root_open.contains("max-width:"), "{root_open}");
    }

    #[cfg(feature = "layout-dagre")]
    #[test]
    fn requirement_title_uses_pre_title_bounds_and_state_margin() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let options = SvgRenderOptions {
            diagram_id: Some("requirementTitle".to_string()),
            ..SvgRenderOptions::default()
        };

        let svg = render_requirement_for_test(
            &empty_requirement_model(),
            &serde_json::json!({"state": {"titleTopMargin": 33}}),
            Some("simple Requirement diagram"),
            &measurer,
            &options,
        )
        .unwrap();
        let view_box = root_view_box(&svg);

        assert!(svg.contains(r#"x="0" y="-33" class="requirementDiagramTitleText""#));
        assert!(view_box[0] < 0.0, "{view_box:?}");
        assert!(view_box[1] < -33.0, "{view_box:?}");
        assert!(view_box[2] > 16.0, "{view_box:?}");
        assert!(view_box[3] > 41.0, "{view_box:?}");
    }

    #[cfg(feature = "layout-dagre")]
    #[test]
    fn requirement_html_labels_use_xhtml_and_deterministic_wrap_styles() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let config = serde_json::json!({
            "fontFamily": "trebuchet ms, verdana, arial, sans-serif",
            "fontSize": 10,
            "themeVariables": {"fontSize": "24px"}
        });
        let model = RequirementDiagramModel {
            requirements: vec![RequirementNode {
                name: "req_font_size".to_string(),
                node_type: "Requirement".to_string(),
                requirement_id: "req_font_size".to_string(),
                text: "font size precedence should be deterministic".to_string(),
                risk: "`Low`".to_string(),
                verify_method: "Test".to_string(),
                css_styles: Vec::new(),
                classes: Vec::new(),
            }],
            ..empty_requirement_model()
        };
        let options = SvgRenderOptions {
            diagram_id: Some("requirementLabels".to_string()),
            ..SvgRenderOptions::default()
        };

        let svg = render_requirement_for_test(&model, &config, None, &measurer, &options).unwrap();
        let calculation_style = crate::text::TextStyle {
            font_family: Some("trebuchet ms, verdana, arial, sans-serif".to_owned()),
            font_size: 10.0,
            ..crate::text::TextStyle::default()
        };
        let expected_max_width =
            merman_render_graph::requirement::calculate_text_width_like_mermaid_px(
                &measurer,
                &calculation_style,
                "Text: font size precedence should be deterministic",
            ) + 50;

        assert!(svg.contains(r#"<div xmlns="http://www.w3.org/1999/xhtml""#));
        assert!(svg.contains("display: table; white-space: break-spaces;"));
        assert!(svg.contains(&format!("max-width: {expected_max_width}px;")));
        assert!(svg.contains(&format!("width: {expected_max_width}px;")));
        assert!(svg.contains(r#"class="nodeLabel markdown-node-label""#));
        assert!(svg.contains("<p>Risk: `Low`</p>"), "{svg}");
    }
}
