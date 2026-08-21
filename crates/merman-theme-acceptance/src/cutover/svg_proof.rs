use super::*;

mod deep_family_paint;
mod sequence_role_paint_proof;
mod terminal_paint;

fn visible_text_content(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(roxmltree::Node::is_text)
        .filter_map(|text| text.text())
        .collect()
}

fn has_visible_text_content(node: roxmltree::Node<'_, '_>) -> bool {
    node.descendants()
        .filter(roxmltree::Node::is_text)
        .filter_map(|text| text.text())
        .any(|text| !text.trim().is_empty())
}

pub(super) fn prove_svg_routes(
    case: CutoverCase,
    routes: &[ThemeRouteCutoverDescriptor],
    svg: &str,
    markers: &[FlowchartMarkerObservation],
) -> C6ProofResult<SvgCutoverProof> {
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| C6ProofError::new("route-svg-parse", error.to_string()))?;
    let view_box = match document.root_element().attribute("viewBox") {
        Some(raw) => Some(parse_svg_view_box(raw)?),
        None if is_info_text_fill_route(case.id.route()) => None,
        None => {
            return Err(C6ProofError::new(
                "route-svg-proof",
                "SVG root lacks a viewBox",
            ));
        }
    };
    let mut assertions = BTreeMap::new();
    let mut proof_regions = None;
    for &route in routes {
        let (actual, terminal_digest, target_regions) = match case.id.route().family_id() {
            DiagramFamilyId::FLOWCHART => {
                let observation = flowchart_route_observation(&document, route)?;
                (
                    observation.value,
                    observation.terminal_digest,
                    observation.target_regions,
                )
            }
            DiagramFamilyId::SWIMLANE if route.target() != ThemeTarget::Cluster => {
                let observation = flowchart_route_observation(&document, route)?;
                (
                    observation.value,
                    observation.terminal_digest,
                    observation.target_regions,
                )
            }
            DiagramFamilyId::SEQUENCE => {
                let observation =
                    sequence_role_paint_proof::sequence_route_observation(&document, route)?;
                (
                    observation.value,
                    observation.terminal_digest,
                    observation.target_regions,
                )
            }
            DiagramFamilyId::TREEMAP => {
                let observation = treemap_route_observation(&document, route)?;
                (
                    observation.value,
                    observation.terminal_digest,
                    observation.target_regions,
                )
            }
            DiagramFamilyId::REQUIREMENT => {
                let observation = requirement_route_observation(&document, route)?;
                (
                    observation.value,
                    observation.terminal_digest,
                    observation.target_regions,
                )
            }
            DiagramFamilyId::PIE => {
                let observation = pie_route_observation(&document, route)?;
                (
                    observation.value,
                    observation.terminal_digest,
                    observation.target_regions,
                )
            }
            DiagramFamilyId::BLOCK => {
                let observation = block_route_observation(&document, route)?;
                (
                    observation.value,
                    observation.terminal_digest,
                    observation.target_regions,
                )
            }
            DiagramFamilyId::ZENUML
            | DiagramFamilyId::VENN
            | DiagramFamilyId::ISHIKAWA
            | DiagramFamilyId::EVENT_MODELING
            | DiagramFamilyId::INFO => {
                let observation =
                    terminal_paint::terminal_paint_route_observation(&document, route)?;
                (
                    observation.value,
                    observation.terminal_digest,
                    observation.target_regions,
                )
            }
            DiagramFamilyId::CLASS
            | DiagramFamilyId::ER
            | DiagramFamilyId::MINDMAP
            | DiagramFamilyId::GIT_GRAPH
            | DiagramFamilyId::GANTT
            | DiagramFamilyId::ARCHITECTURE
            | DiagramFamilyId::SWIMLANE => {
                let observation = deep_family_paint::deep_family_route_observation(
                    &document,
                    route,
                    case.id.profile(),
                )?;
                (
                    observation.value,
                    observation.terminal_digest,
                    observation.target_regions,
                )
            }
            _ => {
                return Err(C6ProofError::new(
                    "route-svg-proof",
                    format!("unsupported route family {}", case.id.route().family_id()),
                ));
            }
        };
        let expected = expected_svg_value(route)?;
        c6_ensure!(
            "route-svg-proof",
            actual == expected,
            "{} emitted `{actual}`, expected `{expected}`",
            route_label(route)
        );
        let witness = case.id;
        let mut value = b"merman.c6-route-svg-assertion.v3\0".to_vec();
        append_witness(&mut value, witness);
        append_len_prefixed(&mut value, actual.as_bytes());
        value.extend_from_slice(&terminal_digest);
        if route
            .projections()
            .contains(ThemeRouteCutoverProjection::MarkerPaintFromEdge)
            && matches!(
                route.family_id(),
                DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
            )
        {
            c6_ensure!(
                "route-svg-proof",
                !markers.is_empty(),
                "{} lacks its Marker isolation receipts",
                witness_label(witness)
            );
            for marker in markers {
                value.extend_from_slice(&marker.digest);
            }
        }
        assertions.insert(route, sha256(value));
        match &proof_regions {
            None => proof_regions = Some(target_regions),
            Some(existing) => c6_ensure!(
                "route-svg-proof",
                existing == &target_regions,
                "one cutover case produced inconsistent target regions"
            ),
        }
    }
    let target_regions = proof_regions.ok_or_else(|| {
        C6ProofError::new("route-svg-proof", "route case produced no target regions")
    })?;
    c6_ensure!(
        "route-svg-proof",
        !target_regions.is_empty(),
        "route case produced an empty target region set"
    );
    let target_underlay_colors =
        target_underlay_colors(&document, case.id.route(), &target_regions)?;
    Ok(SvgCutoverProof {
        assertions,
        view_box,
        target_regions,
        target_underlay_colors,
    })
}

fn target_underlay_colors(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
    target_regions: &[[f64; 4]],
) -> C6ProofResult<Vec<Vec<[u8; 3]>>> {
    let region_count = target_regions.len();
    let underlays: Vec<Vec<[u8; 3]>> = match (route.family_id(), route.target(), route.facet()) {
        (DiagramFamilyId::SWIMLANE, ThemeTarget::Node, ThemeRouteCutoverFacet::Fill) => {
            let root_id = document
                .root_element()
                .attribute("id")
                .ok_or_else(|| C6ProofError::new("route-svg-proof", "SVG root lacks an id"))?;
            let selector = format!("#{root_id} .cluster rect");
            let raw = stylesheet_property(document, &selector, "fill")?;
            std::iter::repeat_n(vec![parse_css_rgb(raw)?], region_count).collect()
        }
        (
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE,
            ThemeTarget::Node,
            ThemeRouteCutoverFacet::Stroke,
        ) => {
            let root_id = document
                .root_element()
                .attribute("id")
                .ok_or_else(|| C6ProofError::new("route-svg-proof", "SVG root lacks an id"))?;
            let selector = format!(
                "#{root_id} .node rect,#{root_id} .node circle,#{root_id} .node ellipse,#{root_id} .node polygon,#{root_id} .node path"
            );
            let surface_underlay =
                parse_optional_css_rgb(stylesheet_property(document, &selector, "fill")?)?;
            let surrounding_underlay = if route.family_id() == DiagramFamilyId::SWIMLANE {
                let selector = format!("#{root_id} .cluster rect");
                parse_optional_css_rgb(stylesheet_property(document, &selector, "fill")?)?
            } else {
                None
            };
            document
                .descendants()
                .filter(|node| {
                    node.is_element()
                        && class_contains(*node, "label-container")
                        && style_value(*node, "stroke").is_some()
                })
                .map(|node| {
                    let node_underlay = style_value(node, "fill")
                        .or_else(|| node.attribute("fill"))
                        .map_or(Ok(surface_underlay), parse_optional_css_rgb)?;
                    let mut colors = Vec::with_capacity(2);
                    for color in [node_underlay, surrounding_underlay].into_iter().flatten() {
                        if !colors.contains(&color) {
                            colors.push(color);
                        }
                    }
                    Ok(colors)
                })
                .collect::<C6ProofResult<Vec<_>>>()?
        }
        (DiagramFamilyId::SEQUENCE, _, _) => {
            sequence_role_paint_proof::sequence_underlay_colors(document, route, target_regions)?
        }
        (DiagramFamilyId::SWIMLANE, ThemeTarget::Edge, ThemeRouteCutoverFacet::Stroke) => {
            let root_id = document
                .root_element()
                .attribute("id")
                .ok_or_else(|| C6ProofError::new("route-svg-proof", "SVG root lacks an id"))?;
            let selector = format!("#{root_id} .cluster rect");
            let underlay =
                parse_optional_css_rgb(stylesheet_property(document, &selector, "fill")?)?;
            std::iter::repeat_n(underlay.into_iter().collect(), region_count).collect()
        }
        (DiagramFamilyId::PIE, ThemeTarget::PieSlice, ThemeRouteCutoverFacet::Stroke) => {
            let mut colors = Vec::new();
            for color in document
                .descendants()
                .filter(|node| node.has_tag_name("path") && class_contains(*node, "pieCircle"))
                .filter_map(|node| node.attribute("fill"))
                .map(parse_css_rgb)
            {
                let color = color?;
                if !colors.contains(&color) {
                    colors.push(color);
                }
            }
            c6_ensure!(
                "route-svg-proof",
                !colors.is_empty(),
                "Pie stroke witness lacks slice-fill underlay colors"
            );
            std::iter::repeat_n(colors, region_count).collect()
        }
        (DiagramFamilyId::BLOCK, ThemeTarget::Node, ThemeRouteCutoverFacet::Stroke) => {
            let root_id = document.root_element().attribute("id").ok_or_else(|| {
                C6ProofError::new("route-svg-proof", "Block SVG root lacks an id")
            })?;
            let selector = format!(
                "#{root_id} .node rect,#{root_id} .node circle,#{root_id} .node ellipse,#{root_id} .node polygon,#{root_id} .node path"
            );
            let underlay =
                parse_optional_css_rgb(stylesheet_property(document, &selector, "fill")?)?;
            std::iter::repeat_n(underlay.into_iter().collect(), region_count).collect()
        }
        (
            DiagramFamilyId::ZENUML
            | DiagramFamilyId::VENN
            | DiagramFamilyId::ISHIKAWA
            | DiagramFamilyId::EVENT_MODELING
            | DiagramFamilyId::INFO,
            _,
            ThemeRouteCutoverFacet::Fill,
        ) => terminal_paint::terminal_paint_underlay_colors(document, route, target_regions)?,
        (
            DiagramFamilyId::CLASS
            | DiagramFamilyId::ER
            | DiagramFamilyId::MINDMAP
            | DiagramFamilyId::GIT_GRAPH
            | DiagramFamilyId::GANTT
            | DiagramFamilyId::SWIMLANE,
            _,
            _,
        ) => deep_family_paint::deep_family_underlay_colors(document, route, target_regions)?,
        _ => std::iter::repeat_n(Vec::new(), region_count).collect(),
    };
    c6_ensure!(
        "route-svg-proof",
        underlays.len() == region_count,
        "{} underlay count differs from its target regions: {} != {region_count}",
        route_label(route),
        underlays.len()
    );
    Ok(underlays)
}

fn parse_optional_css_rgb(raw: &str) -> C6ProofResult<Option<[u8; 3]>> {
    match raw.trim() {
        "none" | "transparent" => Ok(None),
        value => parse_css_rgb(value).map(Some),
    }
}

pub(super) fn prove_flowchart_markers_svg(
    svg: &str,
    profile: CutoverWitnessProfile,
) -> C6ProofResult<Vec<FlowchartMarkerObservation>> {
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| C6ProofError::new("route-marker-svg", error.to_string()))?;
    let root = document.root_element();
    let root_id = root
        .attribute("id")
        .ok_or_else(|| C6ProofError::new("route-marker-svg", "Flowchart SVG root lacks an id"))?;
    let view_box = parse_svg_view_box(root.attribute("viewBox").ok_or_else(|| {
        C6ProofError::new("route-marker-svg", "Flowchart SVG root lacks a viewBox")
    })?)?;
    let edges = document
        .descendants()
        .filter(|node| node.attribute("data-edge") == Some("true"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-marker-svg",
        !edges.is_empty(),
        "missing Flowchart edge paths"
    );
    prove_flowchart_edge_profile(&edges, profile)?;

    let marker_selector = format!("#{root_id} .marker");
    let fill = stylesheet_property(&document, &marker_selector, "fill")?.to_owned();
    let stroke = stylesheet_property(&document, &marker_selector, "stroke")?.to_owned();
    c6_ensure!(
        "route-marker-svg",
        fill == DEFAULT_MARKER.css && stroke == DEFAULT_MARKER.css,
        "Flowchart Marker isolation drifted: fill=`{fill}`, stroke=`{stroke}`"
    );
    let mut observations = Vec::new();
    let mut marker_bindings = std::collections::BTreeSet::new();
    for edge in edges {
        let edge_id = edge.attribute("data-id").ok_or_else(|| {
            C6ProofError::new("route-marker-svg", "Flowchart edge path lacks a data-id")
        })?;
        let edge_path = edge.attribute("d").ok_or_else(|| {
            C6ProofError::new("route-marker-svg", "Flowchart edge path lacks geometry")
        })?;
        let (start, end) = transformed_path_terminals(edge, edge_path)?;
        for (attribute, endpoint) in [("marker-start", start), ("marker-end", end)] {
            let Some(marker_reference) = edge.attribute(attribute) else {
                continue;
            };
            let marker_id = marker_reference
                .strip_prefix("url(#")
                .and_then(|value| value.strip_suffix(')'))
                .ok_or_else(|| {
                    C6ProofError::new(
                        "route-marker-svg",
                        format!("invalid Flowchart marker reference `{marker_reference}`"),
                    )
                })?;
            let (marker_kind, marker_margin) =
                canonical_marker_kind(marker_id).ok_or_else(|| {
                    C6ProofError::new(
                        "route-marker-svg",
                        format!("Flowchart edge references non-base marker `{marker_id}`"),
                    )
                })?;
            c6_ensure!(
                "route-marker-svg",
                marker_margin == profile.expects_marker_margin(),
                "{} Edge witness referenced marker `{marker_id}` with margin={marker_margin}",
                profile.id()
            );
            c6_ensure!(
                "route-marker-svg",
                marker_bindings.insert((edge_id, attribute, marker_kind, marker_margin)),
                "{} Edge witness emitted duplicate marker binding for {} {attribute}",
                profile.id(),
                edge_id
            );
            let marker = document
                .descendants()
                .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(marker_id))
                .ok_or_else(|| {
                    C6ProofError::new(
                        "route-marker-svg",
                        format!("missing referenced Flowchart marker `{marker_id}`"),
                    )
                })?;
            c6_ensure!(
                "route-marker-svg",
                class_contains(marker, "marker"),
                "Flowchart edge references a non-canonical marker surface"
            );
            let shapes = marker
                .children()
                .filter(|node| {
                    node.is_element()
                        && matches!(node.tag_name().name(), "path" | "polygon" | "circle")
                })
                .collect::<Vec<_>>();
            c6_ensure!(
                "route-marker-svg",
                !shapes.is_empty()
                    && shapes
                        .iter()
                        .all(|shape| class_contains(*shape, "arrowMarkerPath")),
                "Flowchart marker `{marker_id}` lacks canonical paintable shapes"
            );
            for shape in &shapes {
                for property in ["fill", "stroke"] {
                    c6_ensure!(
                        "route-marker-svg",
                        shape.attribute(property).is_none()
                            && style_value(*shape, property).is_none(),
                        "base Flowchart marker `{marker_id}` retained direct {property} paint"
                    );
                }
            }

            let probe_rect = [
                endpoint.0 - MARKER_PROBE_RADIUS,
                endpoint.1 - MARKER_PROBE_RADIUS,
                MARKER_PROBE_RADIUS * 2.0,
                MARKER_PROBE_RADIUS * 2.0,
            ];
            let mut geometry = b"merman.c6-route-marker-geometry.v3\0".to_vec();
            append_len_prefixed(&mut geometry, profile.id().as_bytes());
            append_len_prefixed(&mut geometry, marker_kind.as_bytes());
            geometry.push(u8::from(marker_margin));
            for shape in shapes {
                append_len_prefixed(&mut geometry, shape.tag_name().name().as_bytes());
                append_len_prefixed(
                    &mut geometry,
                    shape.attribute("d").unwrap_or_default().as_bytes(),
                );
                append_len_prefixed(
                    &mut geometry,
                    shape.attribute("points").unwrap_or_default().as_bytes(),
                );
                for attribute in ["cx", "cy", "r"] {
                    append_len_prefixed(
                        &mut geometry,
                        shape.attribute(attribute).unwrap_or_default().as_bytes(),
                    );
                }
            }
            for marker_attribute in [
                "viewBox",
                "refX",
                "refY",
                "markerWidth",
                "markerHeight",
                "orient",
            ] {
                append_len_prefixed(
                    &mut geometry,
                    marker
                        .attribute(marker_attribute)
                        .unwrap_or_default()
                        .as_bytes(),
                );
            }
            let geometry_digest = sha256(geometry);

            let mut assertion = b"merman.c6-route-marker-svg.v4\0".to_vec();
            append_len_prefixed(&mut assertion, profile.id().as_bytes());
            append_len_prefixed(&mut assertion, edge_id.as_bytes());
            append_len_prefixed(&mut assertion, attribute.as_bytes());
            append_len_prefixed(&mut assertion, marker_kind.as_bytes());
            assertion.push(u8::from(marker_margin));
            assertion.extend_from_slice(&geometry_digest);
            append_len_prefixed(&mut assertion, fill.as_bytes());
            append_len_prefixed(&mut assertion, stroke.as_bytes());
            for value in view_box.into_iter().chain(probe_rect) {
                assertion.extend_from_slice(&value.to_bits().to_be_bytes());
            }
            observations.push(FlowchartMarkerObservation {
                digest: sha256(assertion),
                geometry_digest,
                view_box,
                probe_rect,
                fill: fill.clone(),
                stroke: stroke.clone(),
            });
        }
    }
    let expected_bindings = expected_flowchart_marker_bindings(profile);
    c6_ensure!(
        "route-marker-svg",
        marker_bindings == expected_bindings,
        "{} Edge witness marker mapping drifted: actual={marker_bindings:?}, expected={expected_bindings:?}",
        profile.id()
    );
    Ok(observations)
}

type FlowchartMarkerBinding<'a> = (&'a str, &'static str, &'static str, bool);

fn expected_flowchart_marker_bindings(
    profile: CutoverWitnessProfile,
) -> std::collections::BTreeSet<FlowchartMarkerBinding<'static>> {
    let (circle_edge, cross_edge, point_edge) = if profile.is_animated() {
        ("circles", "crosses", "points")
    } else {
        ("L_A_B_0", "L_B_C_0", "L_C_D_0")
    };
    let margin = profile.expects_marker_margin();
    [
        (circle_edge, "marker-start", "circleStart", margin),
        (circle_edge, "marker-end", "circleEnd", margin),
        (cross_edge, "marker-start", "crossStart", margin),
        (cross_edge, "marker-end", "crossEnd", margin),
        (point_edge, "marker-start", "pointStart", margin),
        (point_edge, "marker-end", "pointEnd", margin),
    ]
    .into_iter()
    .collect()
}

fn prove_flowchart_edge_profile(
    edges: &[roxmltree::Node<'_, '_>],
    profile: CutoverWitnessProfile,
) -> C6ProofResult<()> {
    let expected_look = if profile.is_neo() { "neo" } else { "classic" };
    for edge in edges {
        let edge_id = edge.attribute("data-id").unwrap_or_default();
        let animated = edge.attribute("class").is_some_and(|classes| {
            classes
                .split_ascii_whitespace()
                .any(|class| matches!(class, "edge-animation-fast" | "edge-animation-slow"))
        });
        let masked = style_value(*edge, "stroke-dasharray")
            .is_some_and(|dasharray| dasharray.starts_with("0 "));
        c6_ensure!(
            "route-marker-svg",
            edge.attribute("data-look") == Some(expected_look),
            "{} Edge witness emitted `{}` look for `{edge_id}`",
            profile.id(),
            edge.attribute("data-look").unwrap_or_default()
        );
        c6_ensure!(
            "route-marker-svg",
            animated == profile.is_animated(),
            "{} Edge witness emitted animation={animated} for `{edge_id}`",
            profile.id()
        );
        c6_ensure!(
            "route-marker-svg",
            masked == profile.expects_neo_mask(),
            "{} Edge witness emitted Neo marker mask={masked} for `{edge_id}` style={:?} class={:?}",
            profile.id(),
            edge.attribute("style"),
            edge.attribute("class")
        );
    }
    Ok(())
}

fn canonical_marker_kind(marker_id: &str) -> Option<(&'static str, bool)> {
    let (base_id, margin) = marker_id
        .strip_suffix("-margin")
        .map_or((marker_id, false), |base_id| (base_id, true));
    [
        "pointEnd",
        "pointStart",
        "circleEnd",
        "circleStart",
        "crossEnd",
        "crossStart",
    ]
    .into_iter()
    .find(|kind| base_id.ends_with(kind))
    .map(|kind| (kind, margin))
}

fn stylesheet_property<'a>(
    document: &'a roxmltree::Document<'a>,
    selector: &str,
    property: &str,
) -> C6ProofResult<&'a str> {
    let values = document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .flat_map(|css| exact_writer_css_properties(css, selector, property))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        values.len() == 1,
        "stylesheet expected one exact writer declaration for `{selector}` {property}, found {}",
        values.len()
    );
    Ok(values[0])
}

fn parse_css_rgb(raw: &str) -> C6ProofResult<[u8; 3]> {
    let value = raw.trim();
    let color = value.parse::<svgtypes::Color>().map_err(|error| {
        C6ProofError::new(
            "route-svg-proof",
            format!("invalid terminal underlay color `{value}`: {error}"),
        )
    })?;
    c6_ensure!(
        "route-svg-proof",
        color.alpha == u8::MAX,
        "terminal underlay color must be opaque, got `{value}`"
    );
    Ok([color.red, color.green, color.blue])
}

fn parse_svg_view_box(raw: &str) -> C6ProofResult<[f64; 4]> {
    let values = raw
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<f64>().map_err(|error| {
                C6ProofError::new(
                    "route-marker-svg",
                    format!("invalid Flowchart viewBox value {part:?}: {error}"),
                )
            })
        })
        .collect::<C6ProofResult<Vec<_>>>()?;
    let [left, top, width, height] = values.as_slice() else {
        return Err(C6ProofError::new(
            "route-marker-svg",
            format!("Flowchart viewBox contains {} values", values.len()),
        ));
    };
    c6_ensure!(
        "route-marker-svg",
        values.iter().all(|value| value.is_finite()) && *width > 0.0 && *height > 0.0,
        "Flowchart viewBox must be finite and positive"
    );
    Ok([*left, *top, *width, *height])
}

pub(super) fn transformed_path_terminals(
    node: roxmltree::Node<'_, '_>,
    path: &str,
) -> C6ProofResult<((f64, f64), (f64, f64))> {
    let mut current = (0.0, 0.0);
    let mut subpath_start = (0.0, 0.0);
    let mut path_start = None;
    let mut saw_segment = false;
    for segment in svgtypes::PathParser::from(path) {
        let segment = segment.map_err(|error| {
            C6ProofError::new(
                "route-marker-svg",
                format!("invalid Flowchart edge path: {error}"),
            )
        })?;
        use svgtypes::PathSegment;
        let next = match segment {
            PathSegment::MoveTo { abs, x, y } => point_from_command(current, abs, x, y),
            PathSegment::LineTo { abs, x, y }
            | PathSegment::SmoothQuadratic { abs, x, y }
            | PathSegment::CurveTo { abs, x, y, .. }
            | PathSegment::SmoothCurveTo { abs, x, y, .. }
            | PathSegment::Quadratic { abs, x, y, .. }
            | PathSegment::EllipticalArc { abs, x, y, .. } => {
                point_from_command(current, abs, x, y)
            }
            PathSegment::HorizontalLineTo { abs, x } => {
                (if abs { x } else { current.0 + x }, current.1)
            }
            PathSegment::VerticalLineTo { abs, y } => {
                (current.0, if abs { y } else { current.1 + y })
            }
            PathSegment::ClosePath { .. } => subpath_start,
        };
        if matches!(segment, PathSegment::MoveTo { .. }) {
            subpath_start = next;
            path_start.get_or_insert(next);
        }
        current = next;
        saw_segment = true;
    }
    c6_ensure!(
        "route-marker-svg",
        saw_segment && current.0.is_finite() && current.1.is_finite(),
        "Flowchart edge path lacks a finite endpoint"
    );

    let mut transform = SvgAffineTransform::IDENTITY;
    let transforms = node
        .ancestors()
        .filter_map(|ancestor| ancestor.attribute("transform"))
        .collect::<Vec<_>>();
    for raw in transforms.into_iter().rev() {
        let parsed = raw.parse::<svgtypes::Transform>().map_err(|error| {
            C6ProofError::new(
                "route-marker-svg",
                format!("invalid Flowchart edge transform {raw:?}: {error}"),
            )
        })?;
        transform = transform.multiply(SvgAffineTransform::from_svg(parsed));
    }
    let start = path_start.ok_or_else(|| {
        C6ProofError::new(
            "route-marker-svg",
            "Flowchart edge path lacks a start point",
        )
    })?;
    Ok((
        transform.apply(start.0, start.1),
        transform.apply(current.0, current.1),
    ))
}

fn point_from_command(current: (f64, f64), absolute: bool, x: f64, y: f64) -> (f64, f64) {
    if absolute {
        (x, y)
    } else {
        (current.0 + x, current.1 + y)
    }
}

#[derive(Clone, Copy)]
struct SvgAffineTransform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl SvgAffineTransform {
    const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    fn from_svg(transform: svgtypes::Transform) -> Self {
        Self {
            a: transform.a,
            b: transform.b,
            c: transform.c,
            d: transform.d,
            e: transform.e,
            f: transform.f,
        }
    }

    fn multiply(self, child: Self) -> Self {
        Self {
            a: self.a * child.a + self.c * child.b,
            b: self.b * child.a + self.d * child.b,
            c: self.a * child.c + self.c * child.d,
            d: self.b * child.c + self.d * child.d,
            e: self.a * child.e + self.c * child.f + self.e,
            f: self.b * child.e + self.d * child.f + self.f,
        }
    }

    fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }
}

#[derive(Clone, Copy)]
struct SvgBounds {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl SvgBounds {
    fn from_point(point: (f64, f64)) -> Self {
        Self {
            left: point.0,
            top: point.1,
            right: point.0,
            bottom: point.1,
        }
    }

    fn include(&mut self, point: (f64, f64)) {
        self.left = self.left.min(point.0);
        self.top = self.top.min(point.1);
        self.right = self.right.max(point.0);
        self.bottom = self.bottom.max(point.1);
    }

    fn union(&mut self, other: Self) {
        self.include((other.left, other.top));
        self.include((other.right, other.bottom));
    }

    fn rect(self, padding: f64) -> C6ProofResult<[f64; 4]> {
        let width = self.right - self.left;
        let height = self.bottom - self.top;
        c6_ensure!(
            "route-svg-geometry",
            [self.left, self.top, self.right, self.bottom]
                .into_iter()
                .all(f64::is_finite)
                && width >= 0.0
                && height >= 0.0,
            "route surface bounds must be finite and ordered"
        );
        Ok([
            self.left - padding,
            self.top - padding,
            (width + padding * 2.0).max(1.0),
            (height + padding * 2.0).max(1.0),
        ])
    }
}

fn group_bounds(
    group: roxmltree::Node<'_, '_>,
    facet: ThemeRouteCutoverFacet,
) -> C6ProofResult<[f64; 4]> {
    let mut bounds: Option<SvgBounds> = None;
    for node in group.descendants().filter(|node| {
        node.is_element()
            && matches!(
                node.tag_name().name(),
                "rect" | "circle" | "ellipse" | "line" | "path" | "polygon" | "polyline"
            )
    }) {
        let next = raw_element_bounds(node)?;
        match &mut bounds {
            Some(bounds) => bounds.union(next),
            None => bounds = Some(next),
        }
    }
    bounds
        .ok_or_else(|| C6ProofError::new("route-svg-geometry", "group has no paint geometry"))?
        .rect(region_padding(facet))
}

fn element_bounds(
    node: roxmltree::Node<'_, '_>,
    facet: ThemeRouteCutoverFacet,
) -> C6ProofResult<[f64; 4]> {
    raw_element_bounds(node)?.rect(region_padding(facet))
}

fn region_padding(facet: ThemeRouteCutoverFacet) -> f64 {
    match facet {
        ThemeRouteCutoverFacet::Fill => 0.5,
        ThemeRouteCutoverFacet::Stroke => 3.0,
    }
}

fn raw_element_bounds(node: roxmltree::Node<'_, '_>) -> C6ProofResult<SvgBounds> {
    let transform = node_transform(node)?;
    let mut bounds: Option<SvgBounds> = None;
    let mut include = |point: (f64, f64)| {
        let point = transform.apply(point.0, point.1);
        match &mut bounds {
            Some(bounds) => bounds.include(point),
            None => bounds = Some(SvgBounds::from_point(point)),
        }
    };
    match node.tag_name().name() {
        "rect" => {
            let x = number_attribute_or(node, "x", 0.0)?;
            let y = number_attribute_or(node, "y", 0.0)?;
            let width = number_attribute(node, "width")?;
            let height = number_attribute(node, "height")?;
            include((x, y));
            include((x + width, y));
            include((x, y + height));
            include((x + width, y + height));
        }
        "circle" => {
            let cx = number_attribute_or(node, "cx", 0.0)?;
            let cy = number_attribute_or(node, "cy", 0.0)?;
            let radius = number_attribute(node, "r")?;
            include((cx - radius, cy - radius));
            include((cx + radius, cy + radius));
        }
        "ellipse" => {
            let cx = number_attribute_or(node, "cx", 0.0)?;
            let cy = number_attribute_or(node, "cy", 0.0)?;
            let rx = number_attribute(node, "rx")?;
            let ry = number_attribute(node, "ry")?;
            include((cx - rx, cy - ry));
            include((cx + rx, cy + ry));
        }
        "line" => {
            include((number_attribute(node, "x1")?, number_attribute(node, "y1")?));
            include((number_attribute(node, "x2")?, number_attribute(node, "y2")?));
        }
        "polygon" | "polyline" => {
            let points = node
                .attribute("points")
                .ok_or_else(|| C6ProofError::new("route-svg-geometry", "polygon lacks points"))?;
            let values = points
                .split(|character: char| character.is_ascii_whitespace() || character == ',')
                .filter(|part| !part.is_empty())
                .map(|part| parse_geometry_number(part, "polygon point"))
                .collect::<C6ProofResult<Vec<_>>>()?;
            c6_ensure!(
                "route-svg-geometry",
                values.len() >= 4 && values.len() % 2 == 0,
                "polygon point list must contain coordinate pairs"
            );
            for pair in values.chunks_exact(2) {
                include((pair[0], pair[1]));
            }
        }
        "path" => {
            let path = node
                .attribute("d")
                .ok_or_else(|| C6ProofError::new("route-svg-geometry", "path lacks geometry"))?;
            for point in path_control_points(path)? {
                include(point);
            }
        }
        tag => {
            return Err(C6ProofError::new(
                "route-svg-geometry",
                format!("unsupported geometry element {tag}"),
            ));
        }
    }
    bounds.ok_or_else(|| C6ProofError::new("route-svg-geometry", "empty geometry element"))
}

fn path_control_points(path: &str) -> C6ProofResult<Vec<(f64, f64)>> {
    use svgtypes::PathSegment;
    let mut points = Vec::new();
    let mut current = (0.0, 0.0);
    let mut subpath_start = (0.0, 0.0);
    for segment in svgtypes::PathParser::from(path) {
        let segment = segment.map_err(|error| {
            C6ProofError::new("route-svg-geometry", format!("invalid path: {error}"))
        })?;
        let next = match segment {
            PathSegment::MoveTo { abs, x, y } | PathSegment::LineTo { abs, x, y } => {
                point_from_command(current, abs, x, y)
            }
            PathSegment::HorizontalLineTo { abs, x } => {
                (if abs { x } else { current.0 + x }, current.1)
            }
            PathSegment::VerticalLineTo { abs, y } => {
                (current.0, if abs { y } else { current.1 + y })
            }
            PathSegment::CurveTo {
                abs,
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                points.push(point_from_command(current, abs, x1, y1));
                points.push(point_from_command(current, abs, x2, y2));
                point_from_command(current, abs, x, y)
            }
            PathSegment::SmoothCurveTo { abs, x2, y2, x, y } => {
                points.push(point_from_command(current, abs, x2, y2));
                point_from_command(current, abs, x, y)
            }
            PathSegment::Quadratic { abs, x1, y1, x, y } => {
                points.push(point_from_command(current, abs, x1, y1));
                point_from_command(current, abs, x, y)
            }
            PathSegment::SmoothQuadratic { abs, x, y } => point_from_command(current, abs, x, y),
            PathSegment::EllipticalArc {
                abs, rx, ry, x, y, ..
            } => {
                let end = point_from_command(current, abs, x, y);
                points.extend([
                    (current.0 - rx, current.1 - ry),
                    (current.0 + rx, current.1 + ry),
                    (end.0 - rx, end.1 - ry),
                    (end.0 + rx, end.1 + ry),
                ]);
                end
            }
            PathSegment::ClosePath { .. } => subpath_start,
        };
        if matches!(segment, PathSegment::MoveTo { .. }) {
            subpath_start = next;
        }
        points.push(next);
        current = next;
    }
    c6_ensure!(
        "route-svg-geometry",
        !points.is_empty() && points.iter().all(|(x, y)| x.is_finite() && y.is_finite()),
        "path must contain finite geometry"
    );
    Ok(points)
}

fn node_transform(node: roxmltree::Node<'_, '_>) -> C6ProofResult<SvgAffineTransform> {
    let mut transform = SvgAffineTransform::IDENTITY;
    let transforms = node
        .ancestors()
        .filter_map(|ancestor| ancestor.attribute("transform"))
        .collect::<Vec<_>>();
    for raw in transforms.into_iter().rev() {
        let parsed = raw.parse::<svgtypes::Transform>().map_err(|error| {
            C6ProofError::new(
                "route-svg-geometry",
                format!("invalid SVG transform {raw:?}: {error}"),
            )
        })?;
        transform = transform.multiply(SvgAffineTransform::from_svg(parsed));
    }
    Ok(transform)
}

fn number_attribute(node: roxmltree::Node<'_, '_>, name: &str) -> C6ProofResult<f64> {
    let raw = node.attribute(name).ok_or_else(|| {
        C6ProofError::new(
            "route-svg-geometry",
            format!("{} lacks {name}", node.tag_name().name()),
        )
    })?;
    parse_geometry_number(raw, name)
}

fn number_attribute_or(
    node: roxmltree::Node<'_, '_>,
    name: &str,
    default: f64,
) -> C6ProofResult<f64> {
    node.attribute(name)
        .map_or(Ok(default), |raw| parse_geometry_number(raw, name))
}

fn parse_geometry_number(raw: &str, name: &str) -> C6ProofResult<f64> {
    let value = raw.parse::<f64>().map_err(|error| {
        C6ProofError::new(
            "route-svg-geometry",
            format!("invalid {name} value {raw:?}: {error}"),
        )
    })?;
    c6_ensure!(
        "route-svg-geometry",
        value.is_finite(),
        "{name} must be finite"
    );
    Ok(value)
}

struct FlowchartRouteObservation {
    value: String,
    terminal_digest: [u8; 32],
    target_regions: Vec<[f64; 4]>,
}

struct TreemapRouteObservation {
    value: String,
    terminal_digest: [u8; 32],
    target_regions: Vec<[f64; 4]>,
}

struct RequirementRouteObservation {
    value: String,
    terminal_digest: [u8; 32],
    target_regions: Vec<[f64; 4]>,
}

struct PieRouteObservation {
    value: String,
    terminal_digest: [u8; 32],
    target_regions: Vec<[f64; 4]>,
}

struct BlockRouteObservation {
    value: String,
    terminal_digest: [u8; 32],
    target_regions: Vec<[f64; 4]>,
}

fn flowchart_route_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<FlowchartRouteObservation> {
    let nodes = match route.target() {
        ThemeTarget::Node => document
            .descendants()
            .filter(|node| {
                node.is_element()
                    && class_contains(*node, "label-container")
                    && style_value(*node, facet_property(route.facet())).is_some()
            })
            .collect::<Vec<_>>(),
        ThemeTarget::Edge => document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .collect::<Vec<_>>(),
        target => {
            return Err(C6ProofError::new(
                "route-svg-proof",
                format!("unsupported Flowchart cutover target {}", target.id()),
            ));
        }
    };
    let expected_surface_count = match route.target() {
        ThemeTarget::Node => 2,
        ThemeTarget::Edge => 3,
        _ => 0,
    };
    c6_ensure!(
        "route-svg-proof",
        nodes.len() == expected_surface_count,
        "{} expected {expected_surface_count} terminal surfaces, found {}",
        route_label(route),
        nodes.len()
    );

    let property = facet_property(route.facet());
    let mut common_value = None;
    let mut target_regions = Vec::new();
    let mut terminal = b"merman.c6-route-flowchart-surfaces.v2\0".to_vec();
    for node in nodes {
        let value = style_value(node, property).ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                format!("{} lacks its emitted style property", route_label(route)),
            )
        })?;
        if let Some(common) = common_value {
            c6_ensure!(
                "route-svg-proof",
                common == value,
                "{} terminal surfaces disagree: `{common}` != `{value}`",
                route_label(route)
            );
        } else {
            common_value = Some(value);
        }
        let region = element_bounds(node, route.facet())?;
        append_len_prefixed(
            &mut terminal,
            node.attribute("data-id")
                .or_else(|| node.attribute("id"))
                .unwrap_or_default()
                .as_bytes(),
        );
        append_len_prefixed(&mut terminal, node.tag_name().name().as_bytes());
        append_len_prefixed(&mut terminal, value.as_bytes());
        append_rect(&mut terminal, region);
        target_regions.push(region);
    }
    Ok(FlowchartRouteObservation {
        value: common_value
            .ok_or_else(|| C6ProofError::new("route-svg-proof", "missing route value"))?
            .to_owned(),
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn treemap_route_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<TreemapRouteObservation> {
    c6_ensure!(
        "route-svg-proof",
        route.target() == ThemeTarget::Title && route.facet() == ThemeRouteCutoverFacet::Fill,
        "unsupported Treemap cutover route {}",
        route_label(route)
    );
    let root_id = document
        .root_element()
        .attribute("id")
        .ok_or_else(|| C6ProofError::new("route-svg-proof", "Treemap SVG root lacks an id"))?;
    let selector = format!("#{root_id} .treemapTitle");
    let value = stylesheet_property(document, &selector, "fill")?;
    let titles = document
        .descendants()
        .filter(|node| node.has_tag_name("text") && node.attribute("class") == Some("treemapTitle"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        titles.len() == 1 && titles[0].text() == Some(TREEMAP_TITLE_TEXT),
        "Treemap witness must emit one canonical body title"
    );
    let title = titles[0];
    c6_ensure!(
        "route-svg-proof",
        style_value(title, "fill").is_none() && title.attribute("fill").is_none(),
        "Treemap title retained a direct fill that bypasses the terminal stylesheet"
    );
    let x = number_attribute(title, "x")?;
    let y = number_attribute(title, "y")?;
    c6_ensure!(
        "route-svg-geometry",
        x > 0.0 && y > 0.0,
        "Treemap title coordinates must be positive"
    );
    let region = [0.0, 0.0, x * 2.0, y * 2.0];

    let mut terminal = b"merman.c6-route-treemap-title.v1\0".to_vec();
    append_len_prefixed(&mut terminal, selector.as_bytes());
    append_len_prefixed(&mut terminal, value.as_bytes());
    append_len_prefixed(
        &mut terminal,
        title.attribute("class").unwrap_or_default().as_bytes(),
    );
    append_len_prefixed(&mut terminal, title.text().unwrap_or_default().as_bytes());
    append_rect(&mut terminal, region);
    Ok(TreemapRouteObservation {
        value: value.to_owned(),
        terminal_digest: sha256(terminal),
        target_regions: vec![region],
    })
}

fn requirement_route_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<RequirementRouteObservation> {
    c6_ensure!(
        "route-svg-proof",
        route.target() == ThemeTarget::Requirement && route.facet() == ThemeRouteCutoverFacet::Fill,
        "unsupported Requirement cutover route {}",
        route_label(route)
    );
    requirement_fill_observation(document)
}

fn requirement_fill_observation(
    document: &roxmltree::Document<'_>,
) -> C6ProofResult<RequirementRouteObservation> {
    let nodes = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && class_contains(*node, "node")
                && node.attribute("id").is_some_and(|id| id.ends_with("-req1"))
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        nodes.len() == 1,
        "Requirement witness expected one canonical req1 node, found {}",
        nodes.len()
    );
    let node = nodes[0];
    let terminal_groups = node
        .children()
        .filter(|child| {
            child.has_tag_name("g")
                && class_contains(*child, "basic")
                && class_contains(*child, "label-container")
                && class_contains(*child, "outer-path")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        terminal_groups.len() == 1,
        "Requirement witness expected one canonical terminal surface group, found {}",
        terminal_groups.len()
    );
    let fill_paths = terminal_groups[0]
        .children()
        .filter(|child| {
            child.has_tag_name("path")
                && child.attribute("stroke") == Some("none")
                && child.attribute("stroke-width") == Some("0")
                && child.attribute("fill").is_some()
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        fill_paths.len() == 1,
        "Requirement witness expected one canonical fill path, found {}",
        fill_paths.len()
    );
    let fill_path = fill_paths[0];
    let value = fill_path.attribute("fill").ok_or_else(|| {
        C6ProofError::new(
            "route-svg-proof",
            "Requirement fill path lacks its fill attribute",
        )
    })?;
    let path = fill_path.attribute("d").ok_or_else(|| {
        C6ProofError::new("route-svg-proof", "Requirement fill path lacks geometry")
    })?;
    let region = element_bounds(fill_path, ThemeRouteCutoverFacet::Fill)?;

    let mut terminal = b"merman.c6-route-requirement-fill-path.v1\0".to_vec();
    append_len_prefixed(
        &mut terminal,
        node.attribute("id").unwrap_or_default().as_bytes(),
    );
    append_len_prefixed(
        &mut terminal,
        terminal_groups[0]
            .attribute("class")
            .unwrap_or_default()
            .as_bytes(),
    );
    append_len_prefixed(&mut terminal, path.as_bytes());
    append_len_prefixed(&mut terminal, value.as_bytes());
    append_rect(&mut terminal, region);
    Ok(RequirementRouteObservation {
        value: value.to_owned(),
        terminal_digest: sha256(terminal),
        target_regions: vec![region],
    })
}

fn pie_route_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<PieRouteObservation> {
    c6_ensure!(
        "route-svg-proof",
        route.target() == ThemeTarget::PieSlice && route.facet() == ThemeRouteCutoverFacet::Stroke,
        "unsupported Pie cutover route {}",
        route_label(route)
    );
    let root_id = document
        .root_element()
        .attribute("id")
        .ok_or_else(|| C6ProofError::new("route-svg-proof", "Pie SVG root lacks an id"))?;
    let slice_selector = format!("#{root_id} .pieCircle");
    let outer_selector = format!("#{root_id} .pieOuterCircle");
    let slice_value = stylesheet_property(document, &slice_selector, "stroke")?;
    let outer_value = stylesheet_property(document, &outer_selector, "stroke")?;
    c6_ensure!(
        "route-svg-proof",
        slice_value == outer_value,
        "Pie slice and outer-circle stroke owners disagree: `{slice_value}` != `{outer_value}`"
    );

    let slices = document
        .descendants()
        .filter(|node| node.has_tag_name("path") && class_contains(*node, "pieCircle"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        slices.len() == 2,
        "Pie stroke witness expected two terminal slice paths, found {}",
        slices.len()
    );
    let outer_circles = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("circle") && node.attribute("class") == Some("pieOuterCircle")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        outer_circles.len() == 1,
        "Pie stroke witness expected one terminal outer circle, found {}",
        outer_circles.len()
    );

    let mut terminal = b"merman.c6-route-pie-slice-stroke.v1\0".to_vec();
    append_len_prefixed(&mut terminal, slice_selector.as_bytes());
    append_len_prefixed(&mut terminal, slice_value.as_bytes());
    append_len_prefixed(&mut terminal, outer_selector.as_bytes());
    append_len_prefixed(&mut terminal, outer_value.as_bytes());
    let mut target_regions = Vec::with_capacity(slices.len() + outer_circles.len());
    for surface in slices.into_iter().chain(outer_circles) {
        c6_ensure!(
            "route-svg-proof",
            surface.attribute("stroke").is_none() && style_value(surface, "stroke").is_none(),
            "Pie terminal surface retained a direct stroke that bypasses its final stylesheet owner"
        );
        let region = element_bounds(surface, route.facet())?;
        append_len_prefixed(&mut terminal, surface.tag_name().name().as_bytes());
        append_len_prefixed(
            &mut terminal,
            surface.attribute("class").unwrap_or_default().as_bytes(),
        );
        append_len_prefixed(
            &mut terminal,
            surface.attribute("d").unwrap_or_default().as_bytes(),
        );
        append_len_prefixed(
            &mut terminal,
            surface.attribute("r").unwrap_or_default().as_bytes(),
        );
        append_rect(&mut terminal, region);
        target_regions.push(region);
    }

    Ok(PieRouteObservation {
        value: slice_value.to_owned(),
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

const BLOCK_RECT_SHELLS: &[&str] = &["rect"];
const BLOCK_CIRCLE_SHELLS: &[&str] = &["circle"];
const BLOCK_DOUBLE_CIRCLE_SHELLS: &[&str] = &["circle", "circle"];
const BLOCK_CYLINDER_SHELLS: &[&str] = &["path"];
const BLOCK_POLYGON_SHELLS: &[&str] = &["polygon"];

fn block_route_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<BlockRouteObservation> {
    c6_ensure!(
        "route-svg-proof",
        route.target() == ThemeTarget::Node && route.facet() == ThemeRouteCutoverFacet::Stroke,
        "unsupported Block cutover route {}",
        route_label(route)
    );
    let root_id = document
        .root_element()
        .attribute("id")
        .ok_or_else(|| C6ProofError::new("route-svg-proof", "Block SVG root lacks an id"))?;
    let expected_nodes = [
        ("rect", BLOCK_RECT_SHELLS),
        ("circle", BLOCK_CIRCLE_SHELLS),
        ("double", BLOCK_DOUBLE_CIRCLE_SHELLS),
        ("cylinder", BLOCK_CYLINDER_SHELLS),
        ("polygon", BLOCK_POLYGON_SHELLS),
    ];
    let mut common_value = None;
    let mut target_regions = Vec::with_capacity(6);
    let mut terminal = b"merman.c6-route-block-node-stroke.v1\0".to_vec();
    for (semantic_id, expected_shells) in expected_nodes {
        let node_id = format!("{root_id}-{semantic_id}");
        let nodes = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("g")
                    && class_contains(*node, "node")
                    && node.attribute("id") == Some(node_id.as_str())
            })
            .collect::<Vec<_>>();
        c6_ensure!(
            "route-svg-proof",
            nodes.len() == 1,
            "Block stroke witness expected one node `{node_id}`, found {}",
            nodes.len()
        );
        let shells = nodes[0]
            .descendants()
            .filter(|surface| {
                surface.is_element()
                    && matches!(
                        surface.tag_name().name(),
                        "rect" | "circle" | "path" | "polygon"
                    )
                    && style_value(*surface, "stroke").is_some()
            })
            .collect::<Vec<_>>();
        let actual_shells = shells
            .iter()
            .map(|surface| surface.tag_name().name())
            .collect::<Vec<_>>();
        c6_ensure!(
            "route-svg-proof",
            actual_shells == expected_shells,
            "Block node `{node_id}` shell shape drifted: actual={actual_shells:?}, expected={expected_shells:?}"
        );

        append_len_prefixed(&mut terminal, node_id.as_bytes());
        for shell in shells {
            c6_ensure!(
                "route-svg-proof",
                shell.attribute("stroke").is_none(),
                "Block node `{node_id}` bypassed its terminal inline-style stroke owner"
            );
            let value = style_value(shell, "stroke").ok_or_else(|| {
                C6ProofError::new(
                    "route-svg-proof",
                    format!("Block node `{node_id}` lacks its terminal stroke"),
                )
            })?;
            if let Some(common) = common_value {
                c6_ensure!(
                    "route-svg-proof",
                    common == value,
                    "Block terminal shell strokes disagree: `{common}` != `{value}`"
                );
            } else {
                common_value = Some(value);
            }
            let region = element_bounds(shell, route.facet())?;
            append_len_prefixed(&mut terminal, shell.tag_name().name().as_bytes());
            append_len_prefixed(
                &mut terminal,
                shell.attribute("style").unwrap_or_default().as_bytes(),
            );
            append_len_prefixed(
                &mut terminal,
                shell.attribute("d").unwrap_or_default().as_bytes(),
            );
            append_len_prefixed(
                &mut terminal,
                shell.attribute("points").unwrap_or_default().as_bytes(),
            );
            append_rect(&mut terminal, region);
            target_regions.push(region);
        }
    }
    c6_ensure!(
        "route-svg-proof",
        target_regions.len() == 6,
        "Block stroke witness expected six terminal shells, found {}",
        target_regions.len()
    );

    Ok(BlockRouteObservation {
        value: common_value
            .ok_or_else(|| C6ProofError::new("route-svg-proof", "missing Block stroke value"))?
            .to_owned(),
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQUIREMENT_FILL_SVG: &str = r##"<svg id="fixture" viewBox="0 0 100 100">
<style>#fixture .node path{fill:#000000;}</style>
<g class="nodes">
  <g class="node default" id="fixture-req1" transform="translate(25,30)">
    <g class="basic label-container outer-path" style="">
      <path d="M-10 -5 L10 -5 L10 5 L-10 5" stroke="none" stroke-width="0" fill="#dc2626"/>
      <path d="M-10 -5 L10 -5 L10 5 L-10 5" stroke="#9370DB" stroke-width="1.3" fill="none"/>
    </g>
    <text>Cutover requirement</text>
  </g>
</g>
</svg>"##;

    const ALL_POINT_MARKER_SVG: &str = r##"<svg id="fixture" viewBox="0 0 100 100">
<style>#fixture .marker{fill:#333333;stroke:#333333;}</style>
<defs>
  <marker id="fixture-pointStart" class="marker" viewBox="0 0 10 10" refX="4.5" refY="5" markerWidth="8" markerHeight="8" orient="auto"><path class="arrowMarkerPath" d="M 0 5 L 10 10 L 10 0 z"/></marker>
  <marker id="fixture-pointEnd" class="marker" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="8" markerHeight="8" orient="auto"><path class="arrowMarkerPath" d="M 0 0 L 10 5 L 0 10 z"/></marker>
</defs>
<path data-edge="true" data-id="L_A_B_0" data-look="classic" d="M 0 10 L 10 10" marker-start="url(#fixture-pointStart)" marker-end="url(#fixture-pointEnd)"/>
<path data-edge="true" data-id="L_B_C_0" data-look="classic" d="M 0 20 L 10 20" marker-start="url(#fixture-pointStart)" marker-end="url(#fixture-pointEnd)"/>
<path data-edge="true" data-id="L_C_D_0" data-look="classic" d="M 0 30 L 10 30" marker-start="url(#fixture-pointStart)" marker-end="url(#fixture-pointEnd)"/>
</svg>"##;

    #[test]
    fn requirement_fill_observer_reads_the_final_path_attribute_and_transformed_region() {
        let document =
            roxmltree::Document::parse(REQUIREMENT_FILL_SVG).expect("parse Requirement SVG");
        let observation =
            requirement_fill_observation(&document).expect("observe Requirement fill path");

        assert_eq!(observation.value, SOLID_FILL.css);
        assert_eq!(observation.target_regions, vec![[14.5, 24.5, 21.0, 11.0]]);
        assert_ne!(observation.terminal_digest, [0; 32]);
    }

    #[test]
    fn marker_witness_rejects_all_shapes_mapped_to_point() {
        let result =
            prove_flowchart_markers_svg(ALL_POINT_MARKER_SVG, CutoverWitnessProfile::ClassicStatic);

        let error = match result {
            Ok(_) => panic!(
                "the fixed marker witness must reject circle and cross edges mapped to point markers"
            ),
            Err(error) => error,
        };
        assert!(error.to_string().contains("marker mapping drifted"));
    }

    #[test]
    fn underlay_color_parser_accepts_terminal_hex_and_rgb_but_rejects_alpha() {
        assert_eq!(
            parse_css_rgb("#f1f5f9").expect("hex color"),
            [241, 245, 249]
        );
        assert_eq!(
            parse_css_rgb("rgb(241,245,249)").expect("rgb color"),
            [241, 245, 249]
        );
        assert!(parse_css_rgb("rgba(241,245,249,0.5)").is_err());
    }

    #[test]
    fn stylesheet_observer_does_not_reimplement_browser_cascade() {
        let document = roxmltree::Document::parse(
            r##"<svg><style>#fixture .node{fill:#dc2626;}</style><style>#fixture [class~="node"]{fill:#000!important;}</style></svg>"##,
        )
        .expect("parse fixed writer stylesheet");

        assert_eq!(
            stylesheet_property(&document, "#fixture .node", "fill").unwrap(),
            "#dc2626"
        );
    }

    #[test]
    fn stylesheet_observer_rejects_duplicate_writer_declarations() {
        let document = roxmltree::Document::parse(
            r##"<svg><style>#fixture .node{fill:#dc2626;}</style><style>#fixture .node{fill:#dc2626;}</style></svg>"##,
        )
        .expect("parse duplicate writer stylesheet");

        assert!(stylesheet_property(&document, "#fixture .node", "fill").is_err());
    }
}
