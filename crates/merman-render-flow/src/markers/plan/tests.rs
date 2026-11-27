use super::*;
use merman_render_core::{
    resources::{RenderResourcePolicy, ResourceLimitId},
    svg_output::BoundedSvgOutput,
};
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};

fn classic_plan(work: &OperationWorkMeter) -> FlowchartMarkerEmissionPlan<'_> {
    FlowchartMarkerEmissionPlan::new("diagram", "flowchart-v2", false, MarkerStyle::Classic, work)
}

fn attribute(tag: &BytesStart<'_>, name: &str) -> Option<String> {
    let value = tag.try_get_attribute(name).unwrap()?.value;
    Some(quick_xml::escape::unescape(&value).unwrap().into_owned())
}

fn marker_and_shape<'a>(svg: &'a str, id: Option<&str>) -> (BytesStart<'a>, BytesStart<'a>) {
    let mut reader = Reader::from_str(svg);
    let marker = loop {
        match reader.read_event().unwrap() {
            Event::Start(tag)
                if tag.name().as_ref() == "marker"
                    && id.is_none_or(|id| attribute(&tag, "id").as_deref() == Some(id)) =>
            {
                break tag;
            }
            Event::Eof => panic!("missing marker {id:?}"),
            _ => {}
        }
    };
    let shape = loop {
        match reader.read_event().unwrap() {
            Event::Start(tag) | Event::Empty(tag) => break tag,
            Event::End(_) | Event::Eof => panic!("missing shape in marker {id:?}"),
            _ => {}
        }
    };
    (marker, shape)
}

#[test]
fn bpmn_markers_keep_their_source_viewports_no_fill_and_no_neo_margin_variant() {
    let work = meter();
    let color =
        PreparedFlowchartMarkerColor::prepare(FlowchartMarkerColor::Source("#2468ac"), &work)
            .unwrap();
    let mut plan = classic_plan(&work);
    for base in [
        FlowchartMarkerBase::OpenArrowEnd,
        FlowchartMarkerBase::OpenArrowStart,
        FlowchartMarkerBase::HollowCircleStart,
        FlowchartMarkerBase::HollowCircleEnd,
    ] {
        let reference = plan
            .register_reference(base, Some(&color), true, &work)
            .unwrap();
        assert!(!reference.margin);
        let (key, raw_color) = plan.variant(reference.variant_index.unwrap());
        let mut svg = String::from("<svg>");
        push_extra_marker(
            &mut svg,
            "diagram",
            "flowchart-v2",
            key,
            raw_color,
            false,
            MarkerStyle::Classic,
        );
        svg.push_str("</svg>");
        let (marker, shape) = marker_and_shape(&svg, None);
        assert_eq!(
            attribute(&marker, "markerUnits").as_deref(),
            Some("userSpaceOnUse")
        );
        assert!(
            !attribute(&marker, "id")
                .as_deref()
                .unwrap()
                .contains("-margin")
        );
        assert_eq!(attribute(&shape, "stroke").as_deref(), Some("#2468ac"));
        assert_eq!(attribute(&shape, "fill").as_deref(), None);
        assert!(
            attribute(&shape, "style")
                .as_deref()
                .unwrap()
                .contains("fill: none")
        );
        if shape.name().as_ref() == "circle" {
            assert_eq!(attribute(&shape, "r").as_deref(), Some("3.6"));
            assert_eq!(attribute(&marker, "markerWidth").as_deref(), Some("10"));
        } else {
            assert_eq!(attribute(&marker, "markerWidth").as_deref(), Some("12"));
            assert!(
                attribute(&shape, "style")
                    .as_deref()
                    .unwrap()
                    .contains("stroke-linejoin: round")
            );
        }
    }
}

#[test]
fn bpmn_marker_references_defs_and_budget_share_one_prepared_plan() {
    let work = meter();
    let mut plan = classic_plan(&work);
    for (id, edge_type) in [
        ("open", "arrow_open_head"),
        ("hollow", "arrow_hollow_circle"),
    ] {
        plan.register_edge(&edge(id, edge_type), Some("#2468ac"))
            .unwrap();
    }
    plan.finalize_svg_budget().unwrap();
    let mut output = String::new();
    plan.push_base_markers(&mut output).unwrap();
    plan.push_extra_markers(&mut output).unwrap();
    assert_eq!(
        output.len(),
        plan.base_marker_bytes + plan.extra_marker_bytes
    );
    assert!(output.contains("diagram_flowchart-v2-openArrowStart"));
    assert!(output.contains("diagram_flowchart-v2-hollowCircleEnd"));
    let mut attributes = String::new();
    plan.push_edge_marker_attributes_for(&mut attributes, FlowchartEdgeKey::new(0))
        .unwrap();
    assert!(attributes.contains("openArrowEnd__2468ac"));
    attributes.clear();
    plan.push_edge_marker_attributes_for(&mut attributes, FlowchartEdgeKey::new(1))
        .unwrap();
    assert!(attributes.contains("hollowCircleEnd__2468ac"));
    let limited = meter_with_limits(
        plan.base_marker_bytes + plan.extra_marker_bytes + plan.edge_marker_attribute_bytes - 1,
        usize::MAX,
    );
    let mut rejected = classic_plan(&limited);
    for (id, edge_type) in [
        ("open", "arrow_open_head"),
        ("hollow", "arrow_hollow_circle"),
    ] {
        rejected
            .register_edge(&edge(id, edge_type), Some("#2468ac"))
            .unwrap();
    }
    assert!(rejected.finalize_svg_budget().is_err());
}

fn meter() -> OperationWorkMeter {
    OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input())
}

fn meter_with_limits(svg_bytes: usize, work_units: usize) -> OperationWorkMeter {
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, svg_bytes)
        .unwrap()
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, work_units)
        .unwrap();
    OperationWorkMeter::new(policy)
}

fn edge(id: &str, edge_type: &str) -> FlowEdge {
    FlowEdge {
        id: id.to_string(),
        from: "A".to_string(),
        to: "B".to_string(),
        label: None,
        label_type: None,
        edge_type: Some(edge_type.to_string()),
        arrow: String::new(),
        start_marker: Default::default(),
        end_marker: Default::default(),
        is_user_defined_id: true,
        stroke: None,
        stroke_kind: Default::default(),
        visibility: Default::default(),
        interpolate: None,
        classes: Vec::new(),
        style: Vec::new(),
        animate: None,
        animation: None,
        step: None,
        length: 1,
    }
}

fn marker_chunk<'a>(svg: &'a str, marker_id: &str) -> &'a str {
    let needle = format!(r#"id="{marker_id}""#);
    let start = svg.find(&needle).expect("marker id");
    let end = svg[start..].find("</marker>").expect("marker end") + start + 9;
    &svg[start..end]
}

#[test]
fn marker_paint_viewports_match_emitted_marker_geometry() {
    let mut defs = String::new();
    push_base_markers(
        &mut defs,
        "diagram",
        "flowchart-v2",
        MarkerStyle::Classic,
        false,
    );
    let svg = format!("<svg>{defs}</svg>");
    for (base, margin) in [
        FlowchartMarkerBase::PointEnd,
        FlowchartMarkerBase::PointStart,
        FlowchartMarkerBase::CircleEnd,
        FlowchartMarkerBase::CircleStart,
        FlowchartMarkerBase::CrossEnd,
        FlowchartMarkerBase::CrossStart,
    ]
    .into_iter()
    .flat_map(|base| [false, true].map(|margin| (base, margin)))
    {
        let spec = flowchart_marker_shape_spec(base, margin);
        let id = format!(
            "diagram_flowchart-v2-{}{}",
            base.id_suffix(),
            if margin { "-margin" } else { "" },
        );
        let (marker, _) = marker_and_shape(&svg, Some(&id));
        let number = |name| {
            attribute(&marker, name)
                .as_deref()
                .unwrap()
                .parse::<f64>()
                .unwrap()
        };
        let view_box = attribute(&marker, "viewBox")
            .as_deref()
            .unwrap()
            .split_whitespace()
            .map(|value| value.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            view_box,
            [
                0.0,
                0.0,
                spec.viewport.view_box[0],
                spec.viewport.view_box[1]
            ]
        );
        assert_eq!(spec.viewport.reference, [number("refX"), number("refY")]);
        assert_eq!(
            spec.viewport.size,
            [number("markerWidth"), number("markerHeight")]
        );
        assert_eq!(
            attribute(&marker, "markerUnits").as_deref(),
            Some("userSpaceOnUse")
        );
        assert_eq!(attribute(&marker, "orient").as_deref(), Some("auto"));
        assert_eq!(attribute(&marker, "preserveAspectRatio").as_deref(), None);
        assert_eq!(attribute(&marker, "overflow").as_deref(), None);
        assert_eq!(attribute(&marker, "style").as_deref(), None);
    }
}

#[test]
fn base_marker_defs_match_mermaid_11_16_1_bytes() {
    let mut defs = String::new();
    push_base_markers(
        &mut defs,
        "diagram",
        "flowchart-v2",
        MarkerStyle::Classic,
        false,
    );

    assert_eq!(
        defs,
        concat!(
            r#"<marker id="diagram_flowchart-v2-pointEnd" class="marker flowchart-v2" viewBox="0 0 10 10" refX="5" refY="5" markerUnits="userSpaceOnUse" markerWidth="8" markerHeight="8" orient="auto"><path d="M 0 0 L 10 5 L 0 10 z" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
            r#"<marker id="diagram_flowchart-v2-pointStart" class="marker flowchart-v2" viewBox="0 0 10 10" refX="4.5" refY="5" markerUnits="userSpaceOnUse" markerWidth="8" markerHeight="8" orient="auto"><path d="M 0 5 L 10 10 L 10 0 z" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
            r#"<marker id="diagram_flowchart-v2-pointEnd-margin" class="marker flowchart-v2" viewBox="0 0 11.5 14" refX="11.5" refY="7" markerUnits="userSpaceOnUse" markerWidth="10.5" markerHeight="14" orient="auto"><path d="M 0 0 L 11.5 7 L 0 14 z" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;"/></marker>"#,
            r#"<marker id="diagram_flowchart-v2-pointStart-margin" class="marker flowchart-v2" viewBox="0 0 11.5 14" refX="1" refY="7" markerUnits="userSpaceOnUse" markerWidth="11.5" markerHeight="14" orient="auto"><polygon points="0,7 11.5,14 11.5,0" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;"/></marker>"#,
            r#"<marker id="diagram_flowchart-v2-circleEnd" class="marker flowchart-v2" viewBox="0 0 10 10" refX="11" refY="5" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
            r#"<marker id="diagram_flowchart-v2-circleStart" class="marker flowchart-v2" viewBox="0 0 10 10" refX="-1" refY="5" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
            r#"<marker id="diagram_flowchart-v2-circleEnd-margin" class="marker flowchart-v2" viewBox="0 0 10 10" refY="5" refX="12.25" markerUnits="userSpaceOnUse" markerWidth="14" markerHeight="14" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;"/></marker>"#,
            r#"<marker id="diagram_flowchart-v2-circleStart-margin" class="marker flowchart-v2" viewBox="0 0 10 10" refX="-2" refY="5" markerUnits="userSpaceOnUse" markerWidth="14" markerHeight="14" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;"/></marker>"#,
            r#"<marker id="diagram_flowchart-v2-crossEnd" class="marker cross flowchart-v2" viewBox="0 0 11 11" refX="12" refY="5.2" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><path d="M 1,1 l 9,9 M 10,1 l -9,9" class="arrowMarkerPath" style="stroke-width: 2; stroke-dasharray: 1, 0;"/></marker>"#,
            r#"<marker id="diagram_flowchart-v2-crossStart" class="marker cross flowchart-v2" viewBox="0 0 11 11" refX="-1" refY="5.2" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><path d="M 1,1 l 9,9 M 10,1 l -9,9" class="arrowMarkerPath" style="stroke-width: 2; stroke-dasharray: 1, 0;"/></marker>"#,
            r#"<marker id="diagram_flowchart-v2-crossEnd-margin" class="marker cross flowchart-v2" viewBox="0 0 15 15" refX="17.7" refY="7.5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 1,1 L 14,14 M 1,14 L 14,1" class="arrowMarkerPath" style="stroke-width: 2.5;"/></marker>"#,
            r#"<marker id="diagram_flowchart-v2-crossStart-margin" class="marker cross flowchart-v2" viewBox="0 0 15 15" refX="-3.5" refY="7.5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 1,1 L 14,14 M 1,14 L 14,1" class="arrowMarkerPath" style="stroke-width: 2.5; stroke-dasharray: 1, 0;"/></marker>"#,
        )
    );
}

#[test]
fn colored_marker_shapes_only_add_color_identity_and_paint_to_base_bytes() {
    let cases = [
        (FlowchartMarkerBase::PointEnd, false, true, true),
        (FlowchartMarkerBase::PointStart, false, true, true),
        (FlowchartMarkerBase::PointEnd, true, true, true),
        (FlowchartMarkerBase::PointStart, true, false, false),
        (FlowchartMarkerBase::CircleEnd, false, true, false),
        (FlowchartMarkerBase::CircleStart, false, true, false),
        (FlowchartMarkerBase::CircleEnd, true, true, false),
        (FlowchartMarkerBase::CircleStart, true, true, false),
        (FlowchartMarkerBase::CrossEnd, false, true, false),
        (FlowchartMarkerBase::CrossStart, false, true, false),
        (FlowchartMarkerBase::CrossEnd, true, true, false),
        (FlowchartMarkerBase::CrossStart, true, true, false),
    ];
    let mut base_defs = String::new();
    push_base_markers(
        &mut base_defs,
        "diagram",
        "flowchart-v2",
        MarkerStyle::Classic,
        false,
    );

    for (base, margin, paints_surface, paints_fill) in cases {
        let base_id = format!(
            "diagram_flowchart-v2-{}{}",
            base.id_suffix(),
            if margin { "-margin" } else { "" }
        );
        let colored_id = format!("{base_id}__2468ac");
        let mut expected = marker_chunk(&base_defs, &base_id).replacen(
            &format!(r#"id="{base_id}""#),
            &format!(r#"id="{colored_id}""#),
            1,
        );
        if paints_surface {
            let paint = if paints_fill {
                r##" stroke="#2468ac" fill="#2468ac""##
            } else {
                r##" stroke="#2468ac""##
            };
            expected = expected.replacen("/></marker>", &format!("{paint}/></marker>"), 1);
        }

        let key = FlowchartMarkerVariantKey {
            base,
            margin,
            color_id: "_2468ac".to_string(),
            mode: FlowchartMarkerVariantMode::Source,
        };
        let mut actual = String::new();
        push_extra_marker(
            &mut actual,
            "diagram",
            "flowchart-v2",
            &key,
            "#2468ac",
            false,
            MarkerStyle::Classic,
        );

        assert_eq!(
            marker_chunk(&actual, &colored_id),
            expected,
            "base={base:?}, margin={margin}"
        );
    }
}

#[test]
fn colored_marker_plan_defines_every_referenced_base_in_first_use_order() {
    let circle = edge("circle", "double_arrow_circle");
    let cross = edge("cross", "double_arrow_cross");
    let point = edge("point", "double_arrow_point");
    let meter = meter();
    let mut plan = classic_plan(&meter);
    plan.register_edge(&circle, Some("#ef4444")).unwrap();
    plan.register_edge(&cross, Some("#22c55e")).unwrap();
    plan.register_edge(&point, Some("#2563eb")).unwrap();

    assert_eq!(
        plan.variants
            .iter()
            .map(|(key, _)| key.base)
            .collect::<Vec<_>>(),
        vec![
            FlowchartMarkerBase::CircleStart,
            FlowchartMarkerBase::CircleEnd,
            FlowchartMarkerBase::CrossStart,
            FlowchartMarkerBase::CrossEnd,
            FlowchartMarkerBase::PointStart,
            FlowchartMarkerBase::PointEnd,
        ]
    );

    let mut attributes = String::new();
    for semantic_index in 0..3 {
        plan.push_edge_marker_attributes_for(
            &mut attributes,
            FlowchartEdgeKey::new(semantic_index),
        )
        .expect("prepared marker references");
    }
    for suffix in [
        "circleStart__ef4444",
        "circleEnd__ef4444",
        "crossStart__22c55e",
        "crossEnd__22c55e",
        "pointStart__2563eb",
        "pointEnd__2563eb",
    ] {
        assert!(
            attributes.contains(&format!("url(#diagram_flowchart-v2-{suffix})")),
            "missing reference {suffix}: {attributes}"
        );
    }

    let mut defs = String::new();
    plan.write_extra_markers(&mut defs);
    for suffix in [
        "circleStart__ef4444",
        "circleEnd__ef4444",
        "crossStart__22c55e",
        "crossEnd__22c55e",
        "pointStart__2563eb",
        "pointEnd__2563eb",
    ] {
        assert_eq!(
            defs.matches(&format!(r#"id="diagram_flowchart-v2-{suffix}""#))
                .count(),
            1,
            "definition {suffix}: {defs}"
        );
    }

    for suffix in ["pointStart__2563eb", "pointEnd__2563eb"] {
        let marker = marker_chunk(&defs, &format!("diagram_flowchart-v2-{suffix}"));
        assert!(marker.contains(r##" stroke="#2563eb" fill="#2563eb""##));
    }
    for (suffix, color) in [
        ("circleStart__ef4444", "#ef4444"),
        ("circleEnd__ef4444", "#ef4444"),
        ("crossStart__22c55e", "#22c55e"),
        ("crossEnd__22c55e", "#22c55e"),
    ] {
        let marker = marker_chunk(&defs, &format!("diagram_flowchart-v2-{suffix}"));
        assert!(marker.contains(&format!(r#" stroke="{color}""#)));
        assert!(!marker.contains(" fill="), "stroke-only marker: {marker}");
    }
}

#[test]
fn duplicate_mermaid_edge_ids_keep_occurrence_scoped_marker_references() {
    let first = edge("duplicate", "double_arrow_circle");
    let second = edge("duplicate", "double_arrow_cross");
    let meter = meter();
    let mut plan = classic_plan(&meter);
    plan.register_edge(&first, Some("#ef4444")).unwrap();
    plan.register_edge(&second, Some("#22c55e")).unwrap();

    let mut first_attributes = String::new();
    plan.push_edge_marker_attributes_for(&mut first_attributes, FlowchartEdgeKey::new(0))
        .unwrap();
    assert!(first_attributes.contains("circleStart__ef4444"));
    assert!(first_attributes.contains("circleEnd__ef4444"));

    let mut second_attributes = String::new();
    plan.push_edge_marker_attributes_for(&mut second_attributes, FlowchartEdgeKey::new(1))
        .unwrap();
    assert!(second_attributes.contains("crossStart__22c55e"));
    assert!(second_attributes.contains("crossEnd__22c55e"));
}

#[test]
fn colored_marker_plan_deduplicates_by_base_and_color_id_with_first_raw_color() {
    let first = edge("first", "arrow_point");
    let second = edge("second", "arrow_point");
    let meter = meter();
    let mut plan = classic_plan(&meter);
    plan.register_edge(&first, Some("#abc")).unwrap();
    plan.register_edge(&second, Some("$abc")).unwrap();

    assert_eq!(marker_color_id("#abc"), marker_color_id("$abc"));
    assert_eq!(plan.variants.len(), 1);
    assert_eq!(
        plan.variants
            .get_index(0)
            .map(|(_, raw_color)| raw_color.as_str()),
        Some("#abc")
    );

    let mut attributes = String::new();
    plan.push_edge_marker_attributes_for(&mut attributes, FlowchartEdgeKey::new(0))
        .expect("first marker");
    plan.push_edge_marker_attributes_for(&mut attributes, FlowchartEdgeKey::new(1))
        .expect("second marker");
    assert_eq!(
        attributes
            .matches("url(#diagram_flowchart-v2-pointEnd__abc)")
            .count(),
        2
    );

    let mut defs = String::new();
    plan.write_extra_markers(&mut defs);
    assert_eq!(
        defs.matches(r#"id="diagram_flowchart-v2-pointEnd__abc""#)
            .count(),
        1
    );
    assert!(defs.contains(r##" stroke="#abc" fill="#abc""##));
    assert!(!defs.contains(r#" stroke="$abc""#));
}

#[test]
fn marker_color_id_matches_javascript_utf16_replacement_semantics() {
    assert_eq!(marker_color_id("var(--é)"), "var_____");
    assert_eq!(marker_color_id("var(--😀)"), "var______");
    assert_ne!(marker_color_id("var(--é)"), marker_color_id("var(--😀)"));
}

#[test]
fn same_colored_bidirectional_base_is_deduplicated_without_merging_start_and_end() {
    let first = edge("first", "double_arrow_point");
    let second = edge("second", "double_arrow_point");
    let meter = meter();
    let mut plan = classic_plan(&meter);
    plan.register_edge(&first, Some("#ef4444")).unwrap();
    assert_eq!(meter.used(), 7);
    plan.register_edge(&second, Some("#ef4444")).unwrap();
    assert_eq!(meter.used(), 12);

    assert_eq!(plan.variants.len(), 2);
    assert_eq!(
        plan.variants.get_index(0).map(|(key, _)| key.base),
        Some(FlowchartMarkerBase::PointStart)
    );
    assert_eq!(
        plan.variants.get_index(1).map(|(key, _)| key.base),
        Some(FlowchartMarkerBase::PointEnd)
    );
    assert!(plan.variants.keys().all(|key| key.color_id == "_ef4444"));

    let mut attributes = String::new();
    plan.push_edge_marker_attributes_for(&mut attributes, FlowchartEdgeKey::new(0))
        .unwrap();
    assert_eq!(
        attributes,
        concat!(
            r#" marker-start="url(#diagram_flowchart-v2-pointStart__ef4444)""#,
            r#" marker-end="url(#diagram_flowchart-v2-pointEnd__ef4444)""#,
        )
    );
}

#[test]
fn uncolored_edges_keep_base_marker_references_and_emit_no_extra_defs() {
    let edge = edge("edge", "arrow_point");
    let meter = meter();
    let mut plan = classic_plan(&meter);
    plan.register_edge(&edge, None).unwrap();

    let mut attributes = String::new();
    plan.push_edge_marker_attributes_for(&mut attributes, FlowchartEdgeKey::new(0))
        .expect("base marker reference");
    assert_eq!(
        attributes,
        r#" marker-end="url(#diagram_flowchart-v2-pointEnd)""#
    );

    let mut defs = String::new();
    plan.write_extra_markers(&mut defs);
    assert!(defs.is_empty());
}

#[test]
fn marker_color_normalization_work_is_admitted_atomically_before_scan() {
    let edge = edge("edge", "arrow_point");
    let color = format!("#{}", "a".repeat(64));
    let normalization_work = 1 + color.len().div_ceil(64);
    assert_eq!(normalization_work, 3);

    let short_meter = meter_with_limits(usize::MAX, normalization_work);
    let mut plan = classic_plan(&short_meter);
    let error = plan
        .register_edge(&edge, Some(&color))
        .expect_err("normalization must be rejected before scanning the color");

    assert!(matches!(error, Error::ResourceLimitExceeded(_)));
    assert_eq!(
        short_meter.used(),
        1,
        "only the edge admission may be charged"
    );
    assert!(plan.variants.is_empty());
    assert!(plan.edge_occurrences.is_empty());

    let exact_work = 1 + normalization_work + 1 + 1;
    let exact_meter = meter();
    let mut exact_plan = classic_plan(&exact_meter);
    exact_plan
        .register_edge(&edge, Some(&color))
        .expect("normalization, reference, and variant work must be charged once each");
    assert_eq!(exact_meter.used(), exact_work);
}

#[test]
fn agentflow_hierarchy_definitions_are_included_in_the_svg_budget() {
    let diagram_id = "diagram<&";
    let mut emitted = String::new();
    push_base_markers(
        &mut emitted,
        diagram_id,
        "agentflow",
        MarkerStyle::Classic,
        false,
    );
    let exact_meter = meter_with_limits(emitted.len(), usize::MAX);
    let mut exact_plan = FlowchartMarkerEmissionPlan::new(
        diagram_id,
        "agentflow",
        false,
        MarkerStyle::Classic,
        &exact_meter,
    );
    exact_plan.finalize_svg_budget().unwrap();
    assert_eq!(exact_plan.base_marker_bytes, emitted.len());
    assert_eq!(exact_meter.projected_svg_bytes(), emitted.len());
    let short_meter = meter_with_limits(emitted.len() - 1, usize::MAX);
    let mut short_plan = FlowchartMarkerEmissionPlan::new(
        diagram_id,
        "agentflow",
        false,
        MarkerStyle::Classic,
        &short_meter,
    );
    let error = short_plan
        .finalize_svg_budget()
        .expect_err("hierarchy definitions cannot escape the precharged SVG ceiling");
    assert!(matches!(error, Error::ResourceLimitExceeded(_)));
    assert_eq!(short_meter.projected_svg_bytes(), 0);
}

#[test]
fn marker_plan_precharges_exact_serialized_bytes_and_work() {
    let edge = edge("edge", "double_arrow_circle");
    let probe_meter = meter();
    let mut probe = classic_plan(&probe_meter);
    probe.register_edge(&edge, Some("#ef4444")).unwrap();
    probe.finalize_svg_budget().unwrap();
    let projected =
        probe.base_marker_bytes + probe.extra_marker_bytes + probe.edge_marker_attribute_bytes;
    let work = probe_meter.used();
    assert!(projected > 0);
    assert!(work > 0);
    assert_eq!(probe_meter.projected_svg_bytes(), projected);

    let exact_meter = meter_with_limits(projected, work);
    let mut exact = classic_plan(&exact_meter);
    exact.register_edge(&edge, Some("#ef4444")).unwrap();
    exact.finalize_svg_budget().unwrap();
    assert_eq!(exact_meter.used(), work);
    assert_eq!(exact_meter.projected_svg_bytes(), projected);

    let short_svg_meter = meter_with_limits(projected - 1, usize::MAX);
    let mut short_svg = classic_plan(&short_svg_meter);
    short_svg.register_edge(&edge, Some("#ef4444")).unwrap();
    let error = short_svg
        .finalize_svg_budget()
        .expect_err("marker bytes above the ceiling must fail before emission");
    assert!(matches!(error, Error::ResourceLimitExceeded(_)));
    assert_eq!(short_svg_meter.projected_svg_bytes(), 0);

    let short_work_meter = meter_with_limits(usize::MAX, work - 1);
    let mut short_work = classic_plan(&short_work_meter);
    let result = short_work
        .register_edge(&edge, Some("#ef4444"))
        .and_then(|_| short_work.finalize_svg_budget());
    assert!(matches!(
        result.expect_err("marker work above the ceiling must fail"),
        Error::ResourceLimitExceeded(_)
    ));
    assert_eq!(short_work_meter.projected_svg_bytes(), 0);
}

#[test]
fn marker_defs_check_the_whole_svg_ceiling_before_each_append() {
    let edge = edge("edge", "double_arrow_cross");
    let probe_meter = meter();
    let mut probe = classic_plan(&probe_meter);
    probe.register_edge(&edge, Some("#22c55e")).unwrap();
    probe.finalize_svg_budget().unwrap();
    let projected = probe.base_marker_bytes + probe.extra_marker_bytes;
    let prefix = "x".repeat(probe.edge_marker_attribute_bytes + 1);

    let exact_meter = meter_with_limits(prefix.len() + projected, usize::MAX);
    let mut exact_plan = classic_plan(&exact_meter);
    exact_plan.register_edge(&edge, Some("#22c55e")).unwrap();
    exact_plan.finalize_svg_budget().unwrap();
    let mut exact_out = prefix.clone();
    exact_plan.push_base_markers(&mut exact_out).unwrap();
    exact_plan.push_extra_markers(&mut exact_out).unwrap();
    assert_eq!(exact_out.len(), prefix.len() + projected);

    let short_meter = meter_with_limits(prefix.len() + projected - 1, usize::MAX);
    let mut short_plan = classic_plan(&short_meter);
    short_plan.register_edge(&edge, Some("#22c55e")).unwrap();
    short_plan.finalize_svg_budget().unwrap();
    let mut short_out = prefix;
    short_plan.push_base_markers(&mut short_out).unwrap();
    let before_extra = short_out.clone();
    assert!(short_plan.push_extra_markers(&mut short_out).is_err());
    assert_eq!(short_out, before_extra);
}

#[test]
fn marker_defs_propagate_cancellation_during_append() {
    let work = meter();
    let mut plan = classic_plan(&work);
    plan.finalize_svg_budget().unwrap();
    let mut out = BoundedSvgOutput::new(&work);
    // Let marker preflight succeed, then cancel at the sink's first append.
    work.control().cancel_after_checkpoints(1);

    assert!(matches!(
        plan.push_base_markers(&mut out),
        Err(Error::Cancelled(_))
    ));
    assert_eq!(out.len(), 0);
    assert!(matches!(out.finish(), Err(Error::Cancelled(_))));
}

#[test]
fn marker_attributes_check_the_whole_svg_ceiling_before_append() {
    let edge = edge("edge", "double_arrow_point");
    let probe_meter = meter();
    let mut probe = classic_plan(&probe_meter);
    probe.register_edge(&edge, Some("#2563eb")).unwrap();
    probe.finalize_svg_budget().unwrap();
    let definitions = probe.base_marker_bytes + probe.extra_marker_bytes;
    let attributes = probe.edge_marker_attribute_bytes;
    let prefix = "x".repeat(definitions + 1);

    let exact_meter = meter_with_limits(prefix.len() + attributes, usize::MAX);
    let mut exact_plan = classic_plan(&exact_meter);
    exact_plan.register_edge(&edge, Some("#2563eb")).unwrap();
    exact_plan.finalize_svg_budget().unwrap();
    let mut exact_out = prefix.clone();
    exact_plan
        .push_edge_marker_attributes_for(&mut exact_out, FlowchartEdgeKey::new(0))
        .unwrap();
    assert_eq!(exact_out.len(), prefix.len() + attributes);

    let short_meter = meter_with_limits(prefix.len() + attributes - 1, usize::MAX);
    let mut short_plan = classic_plan(&short_meter);
    short_plan.register_edge(&edge, Some("#2563eb")).unwrap();
    short_plan.finalize_svg_budget().unwrap();
    let mut short_out = prefix;
    let before = short_out.clone();
    assert!(
        short_plan
            .push_edge_marker_attributes_for(&mut short_out, FlowchartEdgeKey::new(0))
            .is_err()
    );
    assert_eq!(short_out, before);
}

#[test]
fn strict_mode_drops_raw_stroke_tokens_but_loose_mode_preserves_them() {
    for (raw, loose, expected) in [
        ("stroke:DarkGray", false, None),
        (" stroke:DarkGray", false, None),
        ("#333", false, Some("#333")),
        ("  #333  ", false, Some("#333")),
        ("stroke:#123456", false, None),
        ("st\nroke:#333", false, None),
        ("javascript:alert(1)", false, None),
        ("var(--edge)", false, Some("var(--edge)")),
        ("rgb(12,34,56)", false, Some("rgb(12,34,56)")),
        ("stroke:DarkGray", true, Some("stroke:DarkGray")),
        ("stroke:#123456", true, Some("stroke:#123456")),
    ] {
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let edge = edge("edge", "arrow_point");
        let mut plan = FlowchartMarkerEmissionPlan::new(
            "diagram",
            "flowchart-v2",
            loose,
            MarkerStyle::Classic,
            &work,
        );
        plan.register_edge(&edge, Some(raw)).unwrap();
        plan.finalize_svg_budget().unwrap();
        let mut out = String::from("<svg><defs>");
        plan.push_extra_markers(&mut out).unwrap();
        out.push_str("</defs></svg>");
        let (marker, shape) = marker_and_shape(&out, None);
        assert_eq!(
            attribute(&marker, "id").as_deref(),
            Some(format!("diagram_flowchart-v2-pointEnd_{}", marker_color_id(raw)).as_str()),
        );
        for name in ["stroke", "fill"] {
            assert_eq!(
                attribute(&shape, name).as_deref(),
                expected,
                "raw={raw:?}, loose={loose}, attribute={name}: {out}",
            );
        }
    }
}

#[test]
fn marker_admission_stops_before_resolving_the_next_occurrence() {
    use merman_render_core::svg_css::SvgId;

    let scope = FlowchartDocumentScope::new(SvgId::new("marker-admission").unwrap());
    let meter = meter_with_limits(usize::MAX, 1);
    let edge = edge("edge", "arrow_point");
    let mut requested = 0;
    let inputs = (0..2).map(|index| {
        requested += 1;
        if index == 1 {
            return Err(Error::InvalidModel {
                message: "later edge style was unavailable".to_owned(),
            });
        }
        Ok(FlowchartEdgeMarkers {
            edge: FlowchartRenderEdgeRef {
                key: FlowchartEdgeKey::new(index),
                edge: &edge,
            },
            source_color: Some("#2468ac"),
            adaptive_ink: None,
            casing_color: None,
            margin: false,
        })
    });
    let error = FlowchartMarkerEmissionPlan::prepare(
        &scope,
        "flowchart-v2",
        false,
        MarkerStyle::Classic,
        &meter,
        inputs,
    )
    .unwrap_err();
    assert!(matches!(error, Error::ResourceLimitExceeded(_)));
    assert_eq!(requested, 1);
    assert_eq!(meter.used(), 1);
    assert_eq!(meter.projected_svg_bytes(), 0);
}
