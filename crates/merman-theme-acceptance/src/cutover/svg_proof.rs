use super::*;

pub(super) fn prove_svg_routes(
    case: CutoverCase,
    routes: &[ThemeRouteCutoverDescriptor],
    svg: &str,
    markers: &[FlowchartMarkerObservation],
) -> C6ProofResult<SvgCutoverProof> {
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| C6ProofError::new("route-svg-parse", error.to_string()))?;
    let view_box = parse_svg_view_box(
        document
            .root_element()
            .attribute("viewBox")
            .ok_or_else(|| C6ProofError::new("route-svg-proof", "SVG root lacks a viewBox"))?,
    )?;
    let mut assertions = BTreeMap::new();
    let mut proof_regions = None;
    for &route in routes {
        let (actual, terminal_digest, target_regions) = match case.route.family_id() {
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE => {
                let observation = flowchart_route_observation(&document, route)?;
                (
                    observation.value,
                    observation.terminal_digest,
                    observation.target_regions,
                )
            }
            DiagramFamilyId::SEQUENCE => {
                let observation = sequence_route_observation(&document, route)?;
                (
                    observation.value,
                    observation.terminal_digest,
                    observation.target_regions,
                )
            }
            _ => {
                return Err(C6ProofError::new(
                    "route-svg-proof",
                    format!("unsupported route family {}", case.route.family_id()),
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
        let mut value = b"merman.c6-route-svg-assertion.v2\0".to_vec();
        append_route(&mut value, route);
        append_len_prefixed(&mut value, actual.as_bytes());
        value.extend_from_slice(&terminal_digest);
        if matches!(route.target(), ThemeTarget::Edge) {
            c6_ensure!(
                "route-svg-proof",
                !markers.is_empty(),
                "{} lacks its Marker isolation receipts",
                route_label(route)
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
        target_underlay_colors(&document, case.route, target_regions.len())?;
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
    region_count: usize,
) -> C6ProofResult<Vec<Option<[u8; 3]>>> {
    let underlay = match (route.family_id(), route.target(), route.facet()) {
        (DiagramFamilyId::SWIMLANE, ThemeTarget::Node, ThemeRouteCutoverFacet::Fill) => {
            let root_id = document
                .root_element()
                .attribute("id")
                .ok_or_else(|| C6ProofError::new("route-svg-proof", "SVG root lacks an id"))?;
            let selector = format!("#{root_id} .cluster rect");
            let raw = stylesheet_property(document, &selector, "fill")?;
            Some(parse_css_rgb(raw)?)
        }
        _ => None,
    };
    Ok(std::iter::repeat_n(underlay, region_count).collect())
}

pub(super) fn prove_flowchart_markers_svg(
    svg: &str,
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

    let marker_selector = format!("#{root_id} .marker");
    let fill = last_stylesheet_property(&document, &marker_selector, "fill")?.to_owned();
    let stroke = last_stylesheet_property(&document, &marker_selector, "stroke")?.to_owned();
    c6_ensure!(
        "route-marker-svg",
        fill == DEFAULT_MARKER.css && stroke == DEFAULT_MARKER.css,
        "Flowchart Marker isolation drifted: fill=`{fill}`, stroke=`{stroke}`"
    );
    let mut observations = Vec::new();
    for edge in edges {
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
            let marker_kind = canonical_marker_kind(marker_id).ok_or_else(|| {
                C6ProofError::new(
                    "route-marker-svg",
                    format!("Flowchart edge references non-base marker `{marker_id}`"),
                )
            })?;
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
            let mut geometry = b"merman.c6-route-marker-geometry.v2\0".to_vec();
            append_len_prefixed(&mut geometry, marker_kind.as_bytes());
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

            let mut assertion = b"merman.c6-route-marker-svg.v2\0".to_vec();
            append_len_prefixed(
                &mut assertion,
                edge.attribute("data-id").unwrap_or_default().as_bytes(),
            );
            append_len_prefixed(&mut assertion, attribute.as_bytes());
            append_len_prefixed(&mut assertion, marker_kind.as_bytes());
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
    c6_ensure!(
        "route-marker-svg",
        observations.len() >= 6,
        "Flowchart marker witness covered only {} references",
        observations.len()
    );
    Ok(observations)
}

fn canonical_marker_kind(marker_id: &str) -> Option<&'static str> {
    [
        "pointEnd",
        "pointStart",
        "circleEnd",
        "circleStart",
        "crossEnd",
        "crossStart",
    ]
    .into_iter()
    .find(|kind| marker_id.ends_with(kind))
}

fn last_stylesheet_property<'a>(
    document: &'a roxmltree::Document<'a>,
    selector: &str,
    property: &str,
) -> C6ProofResult<&'a str> {
    stylesheet_property(document, selector, property)
}

fn stylesheet_property<'a>(
    document: &'a roxmltree::Document<'a>,
    selector: &str,
    property: &str,
) -> C6ProofResult<&'a str> {
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .filter_map(|css| css_last_property(css, selector, property))
        .next_back()
        .ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                format!("stylesheet lacks {selector} {property}"),
            )
        })
}

fn parse_css_rgb(raw: &str) -> C6ProofResult<[u8; 3]> {
    let value = raw.trim();
    let hex = value.strip_prefix('#').ok_or_else(|| {
        C6ProofError::new(
            "route-svg-proof",
            format!("underlay color must be a six-digit hex color, got `{value}`"),
        )
    })?;
    c6_ensure!(
        "route-svg-proof",
        hex.len() == 6,
        "underlay color must be a six-digit hex color, got `{value}`"
    );
    let channel = |offset: usize| {
        u8::from_str_radix(&hex[offset..offset + 2], 16).map_err(|error| {
            C6ProofError::new(
                "route-svg-proof",
                format!("invalid underlay color `{value}`: {error}"),
            )
        })
    };
    Ok([channel(0)?, channel(2)?, channel(4)?])
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

fn sequence_route_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<SequenceRouteObservation> {
    c6_ensure!(
        "route-svg-proof",
        route.target() == ThemeTarget::Actor,
        "unsupported Sequence cutover target {}",
        route.target().id()
    );
    let root = document.root_element();
    let root_id = root
        .attribute("id")
        .ok_or_else(|| C6ProofError::new("route-svg-proof", "Sequence SVG root lacks an id"))?;
    let participant_types = document
        .descendants()
        .filter(|node| node.attribute("data-et") == Some("participant"))
        .try_fold(BTreeMap::<String, usize>::new(), |mut types, node| {
            let actor_type = node.attribute("data-type").ok_or_else(|| {
                C6ProofError::new(
                    "route-svg-proof",
                    "Sequence participant surface lacks data-type",
                )
            })?;
            *types.entry(actor_type.to_owned()).or_insert(0) += 1;
            Ok::<_, C6ProofError>(types)
        })?;
    let expected_types = match route.facet() {
        ThemeRouteCutoverFacet::Fill => &[
            "actor",
            "boundary",
            "collections",
            "database",
            "entity",
            "participant",
            "queue",
        ][..],
        ThemeRouteCutoverFacet::Stroke => &[
            "actor",
            "boundary",
            "collections",
            "entity",
            "participant",
            "queue",
        ][..],
    }
    .iter()
    .map(|value| (*value).to_owned())
    .collect::<BTreeSet<_>>();
    let actual_types = participant_types.keys().cloned().collect::<BTreeSet<_>>();
    c6_ensure!(
        "route-svg-proof",
        actual_types == expected_types,
        "Sequence witness participant types differ: actual={actual_types:?}, expected={expected_types:?}"
    );
    let actor_surfaces = document
        .descendants()
        .filter(|node| node.is_element() && class_contains(*node, "actor"))
        .count();
    let actor_man_surfaces = document
        .descendants()
        .filter(|node| node.is_element() && class_contains(*node, "actor-man"))
        .count();
    c6_ensure!(
        "route-svg-proof",
        actor_surfaces > 0 && actor_man_surfaces > 0,
        "Sequence witness lacks actor-owned rectangle/path or actor-man surfaces"
    );

    let property = facet_property(route.facet());
    let actor_selector = format!("#{root_id} .actor");
    let actor_shape_selector = format!(
        "#{root_id} .actor-man line,#{root_id} .actor-man circle,#{root_id} .actor line,#{root_id} .actor circle"
    );
    let actor_value = sequence_stylesheet_property(document, &actor_selector, property)?;
    let actor_shape_value =
        sequence_stylesheet_property(document, &actor_shape_selector, property)?;
    c6_ensure!(
        "route-svg-proof",
        actor_value == actor_shape_value,
        "Sequence Actor surfaces disagree for {property}: actor={actor_value}, actor-shape={actor_shape_value}"
    );

    let participants = document
        .descendants()
        .filter(|node| node.attribute("data-et") == Some("participant"))
        .collect::<Vec<_>>();
    let mut target_regions = Vec::new();
    let mut terminal = b"merman.c6-route-sequence-surfaces.v2\0".to_vec();
    for (actor_type, count) in &participant_types {
        append_len_prefixed(&mut terminal, actor_type.as_bytes());
        terminal.extend_from_slice(&usize_to_u64(*count).to_be_bytes());
    }
    for participant in participants {
        let actor_type = participant.attribute("data-type").unwrap_or_default();
        let paintable = participant.descendants().filter(|node| {
            node.is_element()
                && (class_contains(*node, "actor")
                    || (matches!(node.tag_name().name(), "line" | "circle")
                        && node
                            .ancestors()
                            .any(|ancestor| class_contains(ancestor, "actor-man"))))
        });
        c6_ensure!(
            "route-svg-proof",
            paintable.count() > 0,
            "Sequence {actor_type} participant lacks a typed Actor paint surface"
        );
        for node in participant.descendants().filter(|node| node.is_element()) {
            if let Some(inline) = style_value(node, property) {
                c6_ensure!(
                    "route-svg-proof",
                    inline == actor_value,
                    "Sequence {actor_type} participant overrides typed {property} with `{inline}`"
                );
            }
        }
        let region = group_bounds(participant, route.facet())?;
        append_len_prefixed(
            &mut terminal,
            participant
                .attribute("data-id")
                .unwrap_or_default()
                .as_bytes(),
        );
        append_len_prefixed(&mut terminal, actor_type.as_bytes());
        append_rect(&mut terminal, region);
        target_regions.push(region);
    }
    append_len_prefixed(&mut terminal, actor_selector.as_bytes());
    append_len_prefixed(&mut terminal, actor_value.as_bytes());
    append_len_prefixed(&mut terminal, actor_shape_selector.as_bytes());
    append_len_prefixed(&mut terminal, actor_shape_value.as_bytes());
    terminal.extend_from_slice(&usize_to_u64(actor_surfaces).to_be_bytes());
    terminal.extend_from_slice(&usize_to_u64(actor_man_surfaces).to_be_bytes());

    Ok(SequenceRouteObservation {
        value: actor_value.to_owned(),
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn sequence_stylesheet_property<'a>(
    document: &'a roxmltree::Document<'a>,
    selector: &str,
    property: &str,
) -> C6ProofResult<&'a str> {
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .filter_map(|css| css_last_property(css, selector, property))
        .next_back()
        .ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                format!("Sequence stylesheet lacks {selector} {property}"),
            )
        })
}
