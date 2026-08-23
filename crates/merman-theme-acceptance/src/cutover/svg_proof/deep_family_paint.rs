use super::*;

const MARKER_RASTER_OUTSET: f64 = 1.0;

pub(super) struct DeepFamilyRouteObservation {
    pub(super) value: String,
    pub(super) terminal_digest: [u8; 32],
    pub(super) target_regions: Vec<[f64; 4]>,
}

pub(super) fn deep_family_route_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
    profile: CutoverWitnessProfile,
) -> C6ProofResult<DeepFamilyRouteObservation> {
    match route.family_id() {
        DiagramFamilyId::SWIMLANE
            if route.target() == ThemeTarget::Cluster
                && matches!(
                    route.facet(),
                    ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke
                ) =>
        {
            swimlane_cluster_observation(document, route, profile)
        }
        DiagramFamilyId::CLASS
            if matches!(route.target(), ThemeTarget::Node | ThemeTarget::NodeLabel)
                && route.facet() == ThemeRouteCutoverFacet::Fill =>
        {
            class_node_observation(document, route, profile)
        }
        DiagramFamilyId::CLASS
            if route.target() == ThemeTarget::Node
                && route.facet() == ThemeRouteCutoverFacet::Stroke =>
        {
            class_node_observation(document, route, profile)
        }
        DiagramFamilyId::ER
            if route.target() == ThemeTarget::Text
                && route.facet() == ThemeRouteCutoverFacet::Fill =>
        {
            er_text_observation(document, route, profile)
        }
        DiagramFamilyId::CLASS => class_edge_observation(document, route, profile),
        DiagramFamilyId::ER => er_relation_observation(document, route, profile),
        DiagramFamilyId::MINDMAP => mindmap_edge_observation(document, route, profile),
        DiagramFamilyId::GIT_GRAPH => gitgraph_edge_observation(document, route, profile),
        DiagramFamilyId::GANTT if route.facet() == ThemeRouteCutoverFacet::Stroke => {
            gantt_task_stroke_observation(document, route, profile)
        }
        DiagramFamilyId::GANTT => gantt_task_fill_observation(document, route, profile),
        DiagramFamilyId::KANBAN => kanban_task_stroke_observation(document, route, profile),
        DiagramFamilyId::JOURNEY => journey_task_paint_observation(document, route, profile),
        family => Err(C6ProofError::new(
            "route-svg-proof",
            format!("unsupported deep-family cutover route {family}"),
        )),
    }
}

pub(super) fn deep_family_underlay_colors(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
    target_regions: &[[f64; 4]],
) -> C6ProofResult<Vec<Vec<[u8; 3]>>> {
    if route.family_id() == DiagramFamilyId::CLASS
        && route.target() == ThemeTarget::Node
        && route.facet() == ThemeRouteCutoverFacet::Fill
    {
        // A transparent Class Node.fill control exposes the root SVG canvas. There is no solid
        // terminal underlay to prove, so the pixel proof only checks that typed fill is absent.
        return Ok(std::iter::repeat_n(Vec::new(), target_regions.len()).collect());
    }
    let colors = match route.family_id() {
        DiagramFamilyId::CLASS => class_underlay_colors(document)?,
        DiagramFamilyId::ER => er_underlay_colors(document)?,
        DiagramFamilyId::SWIMLANE
            if route.target() == ThemeTarget::Cluster
                && route.facet() == ThemeRouteCutoverFacet::Fill =>
        {
            Vec::new()
        }
        DiagramFamilyId::SWIMLANE
            if route.target() == ThemeTarget::Cluster
                && route.facet() == ThemeRouteCutoverFacet::Stroke =>
        {
            return swimlane_cluster_stroke_underlays(document, target_regions);
        }
        DiagramFamilyId::ARCHITECTURE | DiagramFamilyId::SWIMLANE => {
            generic_underlay_colors(document, route)?
        }
        DiagramFamilyId::MINDMAP => Vec::new(),
        DiagramFamilyId::GIT_GRAPH => Vec::new(),
        DiagramFamilyId::GANTT if route.facet() == ThemeRouteCutoverFacet::Stroke => {
            gantt_task_fill_underlay_colors(document)?
        }
        DiagramFamilyId::GANTT => {
            prove_gantt_transparent_underlay(document)?;
            gantt_task_text_underlay_colors(document)?
        }
        DiagramFamilyId::KANBAN => kanban_task_underlay_colors(document)?,
        DiagramFamilyId::JOURNEY => journey_task_underlay_colors(document, route)?,
        family => {
            return Err(C6ProofError::new(
                "route-svg-proof",
                format!("unsupported deep-family underlay route {family}"),
            ));
        }
    };
    Ok(std::iter::repeat_n(colors, target_regions.len()).collect())
}

fn journey_task_paint_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
    profile: CutoverWitnessProfile,
) -> C6ProofResult<DeepFamilyRouteObservation> {
    require_route(route, ThemeTarget::JourneyTask, route.facet())?;
    require_classic_profile(profile, "Journey Task paint")?;
    c6_ensure!(
        "route-svg-proof",
        route.projections().len() == 1
            && route
                .projections()
                .contains(ThemeRouteCutoverProjection::JourneyTaskPaint),
        "Journey Task paint route lost its dedicated projection contract"
    );

    let tasks = document
        .descendants()
        .filter(|node| node.has_tag_name("rect") && class_contains(*node, "task"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        tasks.len() == 1,
        "Journey Task paint witness expected one task rect, found {}",
        tasks.len()
    );

    let property = facet_property(route.facet());
    let mut common_value = None;
    let mut target_regions = Vec::with_capacity(tasks.len());
    let mut terminal = b"merman.c6-route-journey-task-paint.v1\0".to_vec();
    for task in tasks {
        let class = task.attribute("class").ok_or_else(|| {
            C6ProofError::new("route-svg-proof", "Journey task rect lacks a class")
        })?;
        c6_ensure!(
            "route-svg-proof",
            class == "task task-type-0"
                && ["x", "y", "width", "height", "rx", "ry"]
                    .into_iter()
                    .all(|attribute| task.attribute(attribute).is_some()),
            "Journey task rect lost its terminal geometry or identity: class={class}"
        );
        let value = required_style_value(task, property, "Journey task rect")?;
        observe_common_value(&mut common_value, value, "Journey task rect")?;
        let region = element_bounds(task, route.facet())?;
        target_regions.push(region);
        append_len_prefixed(&mut terminal, class.as_bytes());
        append_len_prefixed(&mut terminal, value.as_bytes());
        append_rect(&mut terminal, region);
    }

    Ok(DeepFamilyRouteObservation {
        value: common_value.ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                "Journey task witness has no terminal value",
            )
        })?,
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn kanban_task_stroke_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
    profile: CutoverWitnessProfile,
) -> C6ProofResult<DeepFamilyRouteObservation> {
    require_route(route, ThemeTarget::Task, ThemeRouteCutoverFacet::Stroke)?;
    require_classic_profile(profile, "Kanban Task.stroke")?;
    c6_ensure!(
        "route-svg-proof",
        route.projections().len() == 1
            && route
                .projections()
                .contains(ThemeRouteCutoverProjection::KanbanTaskStroke),
        "Kanban Task.stroke route lost its dedicated projection contract"
    );

    let tasks = document
        .descendants()
        .filter(|node| node.has_tag_name("rect") && class_contains(*node, "label-container"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        tasks.len() == 2,
        "Kanban Task.stroke witness expected two task rects, found {}",
        tasks.len()
    );

    let mut common_value = None;
    let mut target_regions = Vec::with_capacity(tasks.len() * 4);
    let mut terminal = b"merman.c6-route-kanban-task-stroke.v1\0".to_vec();
    for task in tasks {
        let class = task.attribute("class").ok_or_else(|| {
            C6ProofError::new("route-svg-proof", "Kanban task rect lacks a class")
        })?;
        c6_ensure!(
            "route-svg-proof",
            ["basic", "label-container", "__APA__"]
                .into_iter()
                .all(|token| class.split_ascii_whitespace().any(|class| class == token))
                && ["x", "y", "width", "height", "rx", "ry"]
                    .into_iter()
                    .all(|attribute| task.attribute(attribute).is_some()),
            "Kanban task rect lost its terminal geometry or identity: class={class}"
        );
        let value = required_style_value(task, "stroke", "Kanban task rect")?;
        observe_common_value(&mut common_value, value, "Kanban task rect")?;
        c6_ensure!(
            "route-svg-proof",
            task.attribute("stroke").is_none() && task.attribute("fill").is_none(),
            "Kanban task rect exposed a non-style stroke or fill owner"
        );
        append_len_prefixed(&mut terminal, class.as_bytes());
        append_len_prefixed(&mut terminal, value.as_bytes());
        for region in kanban_task_stroke_regions(task)? {
            target_regions.push(region);
            append_rect(&mut terminal, region);
        }
    }

    Ok(DeepFamilyRouteObservation {
        value: common_value.ok_or_else(|| {
            C6ProofError::new("route-svg-proof", "Kanban task witness has no stroke value")
        })?,
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn kanban_task_stroke_regions(task: roxmltree::Node<'_, '_>) -> C6ProofResult<Vec<[f64; 4]>> {
    let [x, y, width, height] = raw_element_bounds(task)?.rect(0.0)?;
    let edge = 3.0;
    let horizontal_inset = number_attribute_or(task, "rx", 0.0)? + edge;
    let vertical_inset = number_attribute_or(task, "ry", 0.0)? + edge;
    c6_ensure!(
        "route-svg-geometry",
        width > horizontal_inset * 2.0 && height > vertical_inset * 2.0,
        "Kanban task rect is too small for a stroke-only evidence ROI"
    );
    Ok(vec![
        [
            x + horizontal_inset,
            y,
            width - horizontal_inset * 2.0,
            edge,
        ],
        [
            x + horizontal_inset,
            y + height - edge,
            width - horizontal_inset * 2.0,
            edge,
        ],
        [x, y + vertical_inset, edge, height - vertical_inset * 2.0],
        [
            x + width - edge,
            y + vertical_inset,
            edge,
            height - vertical_inset * 2.0,
        ],
    ])
}

fn kanban_task_underlay_colors(document: &roxmltree::Document<'_>) -> C6ProofResult<Vec<[u8; 3]>> {
    let root_id = root_id(document, "Kanban")?;
    let selector = format!(
        "#{root_id} .node rect,#{root_id} .node circle,#{root_id} .node ellipse,#{root_id} .node polygon,#{root_id} .node path"
    );
    let mut colors = Vec::new();
    for raw in document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .flat_map(|css| exact_writer_css_properties(css, &selector, "fill"))
    {
        if let Some(color) = parse_optional_css_rgb(raw)?
            && !colors.contains(&color)
        {
            colors.push(color);
        }
    }
    Ok(colors)
}

fn journey_task_underlay_colors(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<Vec<[u8; 3]>> {
    if route.facet() == ThemeRouteCutoverFacet::Fill {
        return Ok(Vec::new());
    }
    let root_id = root_id(document, "Journey")?;
    let selector = format!("#{root_id} .task-type-0,#{root_id} .section-type-0");
    Ok(
        parse_optional_css_rgb(stylesheet_property(document, &selector, "fill")?)?
            .into_iter()
            .collect(),
    )
}

fn swimlane_cluster_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
    profile: CutoverWitnessProfile,
) -> C6ProofResult<DeepFamilyRouteObservation> {
    require_classic_profile(profile, "Swimlane Cluster")?;
    let groups = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && class_contains(*node, "swimlane"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        groups.len() == 1,
        "Swimlane Cluster witness expected one terminal group, found {}",
        groups.len()
    );
    let shells = swimlane_cluster_shells(groups[0])?;
    let property = facet_property(route.facet());
    let mut common_value = None;
    let mut target_regions = Vec::with_capacity(match route.facet() {
        ThemeRouteCutoverFacet::Fill => shells.len(),
        ThemeRouteCutoverFacet::Stroke => shells.len() * 4,
    });
    let mut terminal = b"merman.c6-route-swimlane-cluster-surface.v3\0".to_vec();
    for shell in shells {
        let value = style_value(shell, property)
            .or_else(|| shell.attribute(property))
            .ok_or_else(|| {
                C6ProofError::new(
                    "route-svg-proof",
                    format!(
                        "Swimlane Cluster shell `{}` lacks its emitted {property}",
                        shell.attribute("class").unwrap_or_default()
                    ),
                )
            })?;
        observe_common_value(&mut common_value, value, "Swimlane Cluster")?;
        append_len_prefixed(
            &mut terminal,
            shell.attribute("class").unwrap_or_default().as_bytes(),
        );
        append_len_prefixed(&mut terminal, value.as_bytes());
        for region in swimlane_cluster_target_regions(shell, route.facet())? {
            target_regions.push(region);
            append_rect(&mut terminal, region);
        }
    }
    Ok(DeepFamilyRouteObservation {
        value: common_value.ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                "Swimlane Cluster has no emitted terminal value",
            )
        })?,
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn swimlane_cluster_target_regions(
    shell: roxmltree::Node<'_, '_>,
    facet: ThemeRouteCutoverFacet,
) -> C6ProofResult<Vec<[f64; 4]>> {
    if facet == ThemeRouteCutoverFacet::Fill {
        return Ok(vec![element_bounds(shell, facet)?]);
    }

    let [left, top, width, height] = raw_element_bounds(shell)?.rect(0.0)?;
    let raw_stroke_width = style_value(shell, "stroke-width")
        .or_else(|| shell.attribute("stroke-width"))
        .unwrap_or("1");
    let stroke_width = raw_stroke_width
        .trim()
        .strip_suffix("px")
        .unwrap_or(raw_stroke_width.trim())
        .parse::<f64>()
        .map_err(|error| {
            C6ProofError::new(
                "route-svg-geometry",
                format!(
                    "invalid Swimlane Cluster terminal stroke-width `{raw_stroke_width}`: {error}"
                ),
            )
        })?;
    c6_ensure!(
        "route-svg-geometry",
        stroke_width.is_finite() && stroke_width > 0.0,
        "Swimlane Cluster terminal stroke-width must be finite and positive"
    );
    let stroke_half_width = stroke_width * affine_linear_scale(node_transform(shell)?)? / 2.0;
    let raster_outset = region_padding(ThemeRouteCutoverFacet::Stroke) + stroke_half_width;
    let right = left + width;
    let bottom = top + height;
    let band = raster_outset + stroke_half_width;
    Ok(vec![
        [
            left - raster_outset,
            top - raster_outset,
            width + raster_outset * 2.0,
            band,
        ],
        [
            left - raster_outset,
            bottom - stroke_half_width,
            width + raster_outset * 2.0,
            band,
        ],
        [
            left - raster_outset,
            top - raster_outset,
            band,
            height + raster_outset * 2.0,
        ],
        [
            right - stroke_half_width,
            top - raster_outset,
            band,
            height + raster_outset * 2.0,
        ],
    ])
}

fn swimlane_cluster_shells<'a, 'input>(
    group: roxmltree::Node<'a, 'input>,
) -> C6ProofResult<[roxmltree::Node<'a, 'input>; 2]> {
    let find = |class_name| {
        group
            .children()
            .find(|node| node.has_tag_name("rect") && class_contains(*node, class_name))
            .ok_or_else(|| {
                let children = group
                    .children()
                    .filter(|node| node.is_element())
                    .map(|node| {
                        format!(
                            "{}:{}",
                            node.tag_name().name(),
                            node.attribute("class").unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                C6ProofError::new(
                    "route-svg-proof",
                    format!(
                        "Swimlane Cluster lacks `{class_name}` terminal shell; children={children}"
                    ),
                )
            })
    };
    Ok([find("swimlane-body")?, find("swimlane-title")?])
}

fn swimlane_cluster_stroke_underlays(
    document: &roxmltree::Document<'_>,
    target_regions: &[[f64; 4]],
) -> C6ProofResult<Vec<Vec<[u8; 3]>>> {
    let group = document
        .descendants()
        .find(|node| node.has_tag_name("g") && class_contains(*node, "swimlane"))
        .ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                "Swimlane Cluster witness lacks its terminal group",
            )
        })?;
    let shells = swimlane_cluster_shells(group)?;
    c6_ensure!(
        "route-svg-proof",
        shells.len() * 4 == target_regions.len(),
        "Swimlane Cluster shell stroke underlays differ from target regions"
    );
    let shell_facts = shells
        .into_iter()
        .map(|shell| {
            Ok((
                raw_element_bounds(shell)?.rect(0.0)?,
                parse_optional_css_rgb(swimlane_cluster_effective_fill(document, shell)?)?
                    .into_iter()
                    .collect::<Vec<_>>(),
            ))
        })
        .collect::<C6ProofResult<Vec<_>>>()?;
    let mut underlays = Vec::with_capacity(target_regions.len());
    for (shell_index, (bounds, fill)) in shell_facts.iter().enumerate() {
        for edge in 0..4 {
            let mut colors = fill.clone();
            for (other_index, (other_bounds, other_fill)) in shell_facts.iter().enumerate() {
                if shell_index != other_index
                    && swimlane_shell_edge_touches(*bounds, *other_bounds, edge)
                {
                    colors.extend(other_fill);
                }
            }
            colors.sort_unstable();
            colors.dedup();
            underlays.push(colors);
        }
    }
    Ok(underlays)
}

fn swimlane_cluster_effective_fill<'document, 'input: 'document>(
    document: &'document roxmltree::Document<'input>,
    shell: roxmltree::Node<'document, 'input>,
) -> C6ProofResult<&'document str> {
    if let Some(fill) = style_value(shell, "fill") {
        return Ok(fill);
    }
    let root_id = root_id(document, "Swimlane")?;
    let selector = format!("#{root_id} .cluster rect");
    stylesheet_property(document, &selector, "fill")
}

fn swimlane_shell_edge_touches(shell: [f64; 4], other: [f64; 4], edge: usize) -> bool {
    const GEOMETRY_EPSILON: f64 = 1.0e-6;
    let [left, top, width, height] = shell;
    let [other_left, other_top, other_width, other_height] = other;
    let right = left + width;
    let bottom = top + height;
    let other_right = other_left + other_width;
    let other_bottom = other_top + other_height;
    let horizontal_overlap = left < other_right && other_left < right;
    let vertical_overlap = top < other_bottom && other_top < bottom;
    match edge {
        0 => (top - other_bottom).abs() <= GEOMETRY_EPSILON && horizontal_overlap,
        1 => (bottom - other_top).abs() <= GEOMETRY_EPSILON && horizontal_overlap,
        2 => (left - other_right).abs() <= GEOMETRY_EPSILON && vertical_overlap,
        3 => (right - other_left).abs() <= GEOMETRY_EPSILON && vertical_overlap,
        _ => false,
    }
}

fn class_node_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
    profile: CutoverWitnessProfile,
) -> C6ProofResult<DeepFamilyRouteObservation> {
    require_classic_profile(profile, "Class Node")?;
    let groups = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && class_contains(*node, "node")
                && node.attribute("id").is_some()
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        groups.len() == 2,
        "Class Node witness expected two node groups, found {}",
        groups.len()
    );
    generic_group_surface_observation(
        route,
        profile,
        groups,
        2,
        "Class Node",
        if route.target() == ThemeTarget::NodeLabel {
            GroupSurfaceValue::Text
        } else {
            GroupSurfaceValue::NonNonePaint
        },
    )
}

fn er_text_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
    profile: CutoverWitnessProfile,
) -> C6ProofResult<DeepFamilyRouteObservation> {
    require_route(route, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill)?;
    require_classic_profile(profile, "ER Text")?;
    let relation_groups = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && class_contains(*node, "edgeLabel")
                && !class_contains(*node, "merman-foreignobject-fallback")
                && node.descendants().any(|descendant| {
                    descendant.has_tag_name("text") && has_visible_text_content(descendant)
                })
        })
        .collect::<Vec<_>>();
    let entity_text_groups = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && class_contains(*node, "merman-foreignobject-fallback")
                && class_contains(*node, "nodeLabel")
                && node.descendants().any(|descendant| {
                    descendant.has_tag_name("text") && has_visible_text_content(descendant)
                })
        })
        .collect::<Vec<_>>();
    let entity_geometry_groups = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && class_contains(*node, "node")
                && !class_contains(*node, "merman-foreignobject-fallback")
                && node
                    .ancestors()
                    .any(|ancestor| class_contains(ancestor, "nodes"))
                && node.descendants().any(|descendant| {
                    descendant.has_tag_name("rect") && class_contains(descendant, "label-container")
                })
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        relation_groups.len() == 1
            && entity_text_groups.len() == 2
            && entity_geometry_groups.len() == 2,
        "ER Text witness expected one relation label and two entity labels/geometries, found relation={} entity-text={} entity-geometry={}",
        relation_groups.len(),
        entity_text_groups.len(),
        entity_geometry_groups.len()
    );

    let property = facet_property(route.facet());
    let mut common_value = None;
    let mut target_regions = Vec::with_capacity(3);
    let mut terminal = b"merman.c6-route-er-text-surface.v1\0".to_vec();
    let relation = relation_groups[0];
    append_er_text_terminal(
        &mut terminal,
        &mut common_value,
        relation,
        relation,
        property,
        GroupSurfaceValue::TextWithAncestor,
        &mut target_regions,
    )?;
    for (text_group, geometry_group) in entity_text_groups
        .into_iter()
        .zip(entity_geometry_groups.into_iter())
    {
        append_er_text_terminal(
            &mut terminal,
            &mut common_value,
            text_group,
            geometry_group,
            property,
            GroupSurfaceValue::TextWithAncestor,
            &mut target_regions,
        )?;
    }
    Ok(DeepFamilyRouteObservation {
        value: common_value.ok_or_else(|| {
            C6ProofError::new("route-svg-proof", "ER Text has no emitted terminal value")
        })?,
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn append_er_text_terminal(
    terminal: &mut Vec<u8>,
    common_value: &mut Option<String>,
    text_group: roxmltree::Node<'_, '_>,
    geometry_group: roxmltree::Node<'_, '_>,
    property: &str,
    value_kind: GroupSurfaceValue,
    target_regions: &mut Vec<[f64; 4]>,
) -> C6ProofResult<()> {
    let value = group_surface_value(text_group, property, value_kind).ok_or_else(|| {
        C6ProofError::new(
            "route-svg-proof",
            format!("ER Text terminal group lacks its emitted {property}"),
        )
    })?;
    observe_common_value(common_value, value, "ER Text")?;
    let region = group_bounds(geometry_group, ThemeRouteCutoverFacet::Fill)?;
    target_regions.push(region);
    append_len_prefixed(
        terminal,
        text_group.attribute("class").unwrap_or_default().as_bytes(),
    );
    append_len_prefixed(terminal, visible_text_content(text_group).trim().as_bytes());
    append_len_prefixed(terminal, value.as_bytes());
    append_rect(terminal, region);
    Ok(())
}

#[derive(Clone, Copy)]
enum GroupSurfaceValue {
    NonNonePaint,
    Text,
    TextWithAncestor,
}

fn group_surface_value<'a>(
    group: roxmltree::Node<'a, '_>,
    property: &str,
    value_kind: GroupSurfaceValue,
) -> Option<&'a str> {
    group
        .descendants()
        .filter(|node| match value_kind {
            GroupSurfaceValue::NonNonePaint => true,
            GroupSurfaceValue::Text => {
                node.has_tag_name("text")
                    || (node.has_tag_name("span")
                        && !node.text().unwrap_or_default().trim().is_empty())
            }
            GroupSurfaceValue::TextWithAncestor => {
                node.has_tag_name("text")
                    || node.has_tag_name("tspan")
                    || (node.has_tag_name("span")
                        && !node.text().unwrap_or_default().trim().is_empty())
            }
        })
        .filter_map(|node| {
            style_value(node, property)
                .or_else(|| node.attribute(property))
                .or_else(|| {
                    if !matches!(value_kind, GroupSurfaceValue::TextWithAncestor) {
                        return None;
                    }
                    let mut ancestor = node.parent();
                    while let Some(candidate) = ancestor {
                        if let Some(value) = style_value(candidate, property)
                            .or_else(|| candidate.attribute(property))
                        {
                            return Some(value);
                        }
                        if candidate == group {
                            break;
                        }
                        ancestor = candidate.parent();
                    }
                    None
                })
        })
        .find(|value| !matches!(value_kind, GroupSurfaceValue::NonNonePaint) || *value != "none")
}

fn generic_group_surface_observation(
    route: ThemeRouteCutoverDescriptor,
    _profile: CutoverWitnessProfile,
    groups: Vec<roxmltree::Node<'_, '_>>,
    expected_count: usize,
    label: &str,
    value_kind: GroupSurfaceValue,
) -> C6ProofResult<DeepFamilyRouteObservation> {
    c6_ensure!(
        "route-svg-proof",
        groups.len() == expected_count,
        "{label} witness expected {expected_count} terminal groups, found {}",
        groups.len()
    );
    let property = facet_property(route.facet());
    let mut common_value = None;
    let mut target_regions = Vec::with_capacity(groups.len());
    let mut terminal = format!(
        "merman.c6-route-{}-surface.v1\0",
        label.to_ascii_lowercase()
    )
    .into_bytes();
    for group in groups {
        let value = group_surface_value(group, property, value_kind).ok_or_else(|| {
                let styles = group
                    .descendants()
                    .filter(|node| node.attribute("style").is_some())
                    .map(|node| {
                        format!(
                            "{}:{}",
                            node.tag_name().name(),
                            node.attribute("style").unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(";");
                C6ProofError::new(
                    "route-svg-proof",
                    format!(
                        "{label} terminal group lacks its emitted {property}: class={} text={} styles={styles}",
                        group.attribute("class").unwrap_or_default(),
                        group.text().unwrap_or_default().trim()
                    ),
                )
            })?;
        observe_common_value(&mut common_value, value, label)?;
        let region = group_bounds(group, route.facet())?;
        target_regions.push(region);
        append_len_prefixed(
            &mut terminal,
            group.attribute("id").unwrap_or_default().as_bytes(),
        );
        append_len_prefixed(
            &mut terminal,
            group.attribute("class").unwrap_or_default().as_bytes(),
        );
        append_len_prefixed(&mut terminal, value.as_bytes());
        append_rect(&mut terminal, region);
    }
    Ok(DeepFamilyRouteObservation {
        value: common_value
            .ok_or_else(|| C6ProofError::new("route-svg-proof", "surface has no emitted value"))?,
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn generic_underlay_colors(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<Vec<[u8; 3]>> {
    let property = facet_property(route.facet());
    let target_color = route_control_color(route)?.rgb;
    let mut colors = Vec::new();
    for node in document.descendants().filter(|node| node.is_element()) {
        for candidate in [
            style_value(node, "fill"),
            node.attribute("fill"),
            style_value(node, "stroke"),
            node.attribute("stroke"),
        ]
        .into_iter()
        .flatten()
        {
            if candidate == "transparent" || candidate == "none" || candidate == property {
                continue;
            }
            if parse_css_rgb(candidate).ok() == Some(target_color) {
                continue;
            }
            push_optional_color(&mut colors, candidate)?;
        }
    }
    Ok(colors)
}

fn class_edge_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
    profile: CutoverWitnessProfile,
) -> C6ProofResult<DeepFamilyRouteObservation> {
    require_route(route, ThemeTarget::Edge, ThemeRouteCutoverFacet::Stroke)?;
    c6_ensure!(
        "route-svg-proof",
        matches!(
            profile,
            CutoverWitnessProfile::ClassicStatic | CutoverWitnessProfile::HandDrawnStatic
        ),
        "Class Edge witness uses unsupported profile {}",
        profile.id()
    );
    c6_ensure!(
        "route-svg-proof",
        route
            .projections()
            .contains(ThemeRouteCutoverProjection::EdgeStroke)
            && route
                .projections()
                .contains(ThemeRouteCutoverProjection::MarkerPaintFromEdge)
            && route.projections().len() == 2,
        "Class Edge route lost its path-plus-Marker projection contract"
    );

    let relations = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path")
                && node.attribute("data-edge") == Some("true")
                && node
                    .attribute("data-id")
                    .is_some_and(|id| id.starts_with("id_"))
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        relations.len() == CLASS_RELATION_OCCURRENCE_COUNT,
        "Class Edge witness expected five real relation paths, found {}",
        relations.len()
    );
    let expected_relation_ids = [
        "id_A_B_1",
        "id_C_D_2",
        "id_E_F_3",
        "id_G_H_4",
        "id_interface0_J_5",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();

    let mut common_value = None;
    let mut relation_ids = BTreeSet::new();
    let mut bindings = Vec::new();
    let mut target_regions = Vec::with_capacity(CLASS_RELATION_OCCURRENCE_COUNT * 2);
    let mut terminal = b"merman.c6-route-class-edge-stroke.v1\0".to_vec();
    append_len_prefixed(&mut terminal, profile.id().as_bytes());
    for relation in relations {
        let relation_id = relation.attribute("data-id").ok_or_else(|| {
            C6ProofError::new("route-svg-proof", "Class relation lacks a data-id")
        })?;
        c6_ensure!(
            "route-svg-proof",
            relation_ids.insert(relation_id) && class_contains(relation, "relation"),
            "Class Edge witness emitted duplicate relation {relation_id}"
        );
        let value = required_style_value(relation, "stroke", "Class relation")?;
        observe_common_value(&mut common_value, value, "Class relation")?;
        c6_ensure!(
            "route-svg-proof",
            relation.attribute("data-look") == Some(profile.look())
                && relation.attribute("data-et") == Some("edge")
                && relation
                    .attribute("data-points")
                    .is_some_and(|value| !value.is_empty())
                && relation
                    .attribute("d")
                    .is_some_and(|value| !value.is_empty()),
            "Class relation {relation_id} lost its final DOM identity or geometry"
        );
        if profile == CutoverWitnessProfile::HandDrawnStatic {
            c6_ensure!(
                "route-svg-proof",
                relation.attribute("stroke") == Some(value),
                "HandDrawn Class relation {relation_id} did not bind its display stroke"
            );
        } else {
            c6_ensure!(
                "route-svg-proof",
                relation.attribute("stroke").is_none(),
                "Classic Class relation {relation_id} unexpectedly retained a display stroke"
            );
        }

        let region = element_bounds(relation, route.facet())?;
        target_regions.push(region);
        append_node_terminal(&mut terminal, relation, value, region);
        let path = relation
            .attribute("d")
            .expect("guarded Class path geometry");
        let (start, end) = transformed_path_terminals(relation, path)?;
        for (attribute, endpoint) in [("marker-start", start), ("marker-end", end)] {
            let Some(reference) = relation.attribute(attribute) else {
                continue;
            };
            let marker_id = marker_reference_id(reference, "Class")?;
            let marker_kind = class_marker_kind(marker_id).ok_or_else(|| {
                C6ProofError::new(
                    "route-svg-proof",
                    format!("Class relation {relation_id} references unknown marker {marker_id}"),
                )
            })?;
            c6_ensure!(
                "route-svg-proof",
                marker_direction_matches(attribute, marker_kind),
                "Class relation {relation_id} binds {marker_kind} through {attribute}"
            );
            bindings.push((
                relation,
                relation_id,
                attribute,
                marker_id,
                marker_kind,
                endpoint,
            ));
        }
    }
    c6_ensure!(
        "route-svg-proof",
        relation_ids == expected_relation_ids,
        "Class Edge relation identities drifted: {relation_ids:?}"
    );

    let marker_kinds = bindings
        .iter()
        .map(|(_, _, _, _, kind, _)| *kind)
        .collect::<BTreeSet<_>>();
    let expected_marker_kinds = [
        "aggregationStart",
        "compositionStart",
        "dependencyEnd",
        "extensionStart",
        "lollipopStart",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    c6_ensure!(
        "route-svg-proof",
        bindings.len() == CLASS_RELATION_OCCURRENCE_COUNT && marker_kinds == expected_marker_kinds,
        "Class Edge marker domain drifted: bindings={}, kinds={marker_kinds:?}",
        bindings.len()
    );

    let common_value = common_value.ok_or_else(|| {
        C6ProofError::new("route-svg-proof", "Class Edge witness has no stroke value")
    })?;
    for (relation, relation_id, attribute, marker_id, marker_kind, endpoint) in bindings {
        let marker = exact_marker(document, marker_id, "Class")?;
        let marker_class = class_marker_class(marker_kind);
        c6_ensure!(
            "route-svg-proof",
            class_contains(marker, "marker")
                && class_contains(marker, marker_class)
                && class_contains(marker, "class"),
            "Class marker {marker_id} lost class `marker {marker_class} class`"
        );
        let shapes = marker_shapes(marker);
        c6_ensure!(
            "route-svg-proof",
            shapes.len() == 1,
            "Class marker {marker_id} expected one terminal shape, found {}",
            shapes.len()
        );
        let shape = shapes[0];
        c6_ensure!(
            "route-svg-proof",
            shape.has_tag_name(if marker_kind == "lollipopStart" {
                "circle"
            } else {
                "path"
            }),
            "Class marker {marker_id} changed its terminal shape"
        );
        let stroke = required_style_value(shape, "stroke", "Class marker shape")?;
        c6_ensure!(
            "route-svg-proof",
            stroke == common_value.as_str(),
            "Class marker {marker_id} stroke `{stroke}` differs from `{common_value}`"
        );
        if matches!(marker_kind, "compositionStart" | "dependencyEnd") {
            c6_ensure!(
                "route-svg-proof",
                style_value(shape, "fill") == Some(common_value.as_str()),
                "filled Class marker {marker_id} did not inherit Edge.stroke"
            );
        }
        let probe =
            marker_occurrence_region(document, relation, marker, &shapes, endpoint, "Class")?;
        target_regions.push(probe);
        append_len_prefixed(&mut terminal, relation_id.as_bytes());
        append_len_prefixed(&mut terminal, attribute.as_bytes());
        append_len_prefixed(&mut terminal, marker_kind.as_bytes());
        append_marker_terminal(&mut terminal, marker, &shapes);
        append_rect(&mut terminal, probe);
    }

    Ok(DeepFamilyRouteObservation {
        value: common_value,
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn er_relation_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
    profile: CutoverWitnessProfile,
) -> C6ProofResult<DeepFamilyRouteObservation> {
    require_route(route, ThemeTarget::Relation, ThemeRouteCutoverFacet::Stroke)?;
    require_classic_profile(profile, "ER Relation")?;
    require_edge_stroke_only(route, "ER Relation")?;

    let paths = document
        .descendants()
        .filter(|node| node.has_tag_name("path") && class_contains(*node, "relationshipLine"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        paths.len() == 4,
        "ER Relation witness expected one normal path plus three self-loop segments, found {}",
        paths.len()
    );
    let expected_ids = [
        "id_entity-A-0_entity-B-1_0",
        "entity-SELF-2-cyclic-special-1",
        "entity-SELF-2-cyclic-special-mid",
        "entity-SELF-2-cyclic-special-2",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();

    let mut common_value = None;
    let mut ids = BTreeSet::new();
    let mut normal_count = 0usize;
    let mut self_segments = BTreeSet::new();
    let mut bindings = Vec::new();
    let mut target_regions = Vec::with_capacity(8);
    let mut terminal = b"merman.c6-route-er-relation-stroke.v1\0".to_vec();
    for path in paths {
        let id = path.attribute("data-id").ok_or_else(|| {
            C6ProofError::new("route-svg-proof", "ER relationship path lacks a data-id")
        })?;
        c6_ensure!(
            "route-svg-proof",
            ids.insert(id),
            "ER Relation witness emitted duplicate path {id}"
        );
        if id.starts_with("id_") {
            normal_count += 1;
        }
        for suffix in [
            "-cyclic-special-1",
            "-cyclic-special-mid",
            "-cyclic-special-2",
        ] {
            if id.ends_with(suffix) {
                self_segments.insert(suffix);
            }
        }
        let value = required_style_value(path, "stroke", "ER relationship path")?;
        observe_common_value(&mut common_value, value, "ER relationship path")?;
        c6_ensure!(
            "route-svg-proof",
            path.attribute("data-look") == Some("classic")
                && path.attribute("data-edge") == Some("true")
                && path.attribute("data-et") == Some("edge")
                && path.attribute("class")
                    == Some("edge-thickness-normal edge-pattern-solid relationshipLine")
                && path
                    .attribute("data-points")
                    .is_some_and(|value| !value.is_empty())
                && path.attribute("d").is_some_and(|value| !value.is_empty()),
            "ER relationship path {id} lost its final DOM identity or geometry"
        );
        let marker_count = ["marker-start", "marker-end"]
            .into_iter()
            .filter(|attribute| path.attribute(*attribute).is_some())
            .count();
        if id.ends_with("-cyclic-special-mid") {
            c6_ensure!(
                "route-svg-proof",
                marker_count == 0,
                "ER self-loop middle segment unexpectedly owns a cardinality marker"
            );
        }

        let region = element_bounds(path, route.facet())?;
        target_regions.push(region);
        append_node_terminal(&mut terminal, path, value, region);
        let geometry = path.attribute("d").expect("guarded ER path geometry");
        let (start, end) = transformed_path_terminals(path, geometry)?;
        for (attribute, endpoint) in [("marker-start", start), ("marker-end", end)] {
            let Some(reference) = path.attribute(attribute) else {
                continue;
            };
            let marker_id = marker_reference_id(reference, "ER")?;
            let marker_kind = er_marker_kind(marker_id).ok_or_else(|| {
                C6ProofError::new(
                    "route-svg-proof",
                    format!("ER path {id} references unexpected marker {marker_id}"),
                )
            })?;
            c6_ensure!(
                "route-svg-proof",
                marker_direction_matches(attribute, marker_kind),
                "ER path {id} binds {marker_kind} through {attribute}"
            );
            bindings.push((path, id, attribute, marker_id, marker_kind, endpoint));
        }
    }

    let expected_segments = [
        "-cyclic-special-1",
        "-cyclic-special-mid",
        "-cyclic-special-2",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    c6_ensure!(
        "route-svg-proof",
        ids == expected_ids && normal_count == 1 && self_segments == expected_segments,
        "ER Relation path domain drifted: normal={normal_count}, self={self_segments:?}"
    );
    let marker_kinds = bindings
        .iter()
        .map(|(_, _, _, _, kind, _)| *kind)
        .collect::<BTreeSet<_>>();
    let expected_marker_kinds = ["onlyOneStart", "zeroOrMoreEnd"]
        .into_iter()
        .collect::<BTreeSet<_>>();
    c6_ensure!(
        "route-svg-proof",
        bindings.len() == 4 && marker_kinds == expected_marker_kinds,
        "ER cardinality marker domain drifted: bindings={}, kinds={marker_kinds:?}",
        bindings.len()
    );

    let common_value = common_value.ok_or_else(|| {
        C6ProofError::new("route-svg-proof", "ER Relation witness has no stroke value")
    })?;
    for (path, path_id, attribute, marker_id, marker_kind, endpoint) in bindings {
        let marker = exact_marker(document, marker_id, "ER")?;
        let marker_class = er_marker_class(marker_kind);
        c6_ensure!(
            "route-svg-proof",
            class_contains(marker, "marker")
                && class_contains(marker, marker_class)
                && class_contains(marker, "er"),
            "ER marker {marker_id} lost class `marker {marker_class} er`"
        );
        let stroke = required_style_value(marker, "stroke", "ER marker")?;
        c6_ensure!(
            "route-svg-proof",
            stroke == common_value.as_str(),
            "ER marker {marker_id} stroke `{stroke}` differs from `{common_value}`"
        );
        let shapes = marker_shapes(marker);
        c6_ensure!(
            "route-svg-proof",
            match marker_kind {
                "onlyOneStart" => shapes.len() == 1 && shapes[0].has_tag_name("path"),
                "zeroOrMoreEnd" => {
                    shapes.len() == 2
                        && shapes[0].has_tag_name("circle")
                        && shapes[1].has_tag_name("path")
                }
                _ => false,
            },
            "ER marker {marker_id} changed its terminal geometry"
        );
        let probe = marker_occurrence_region(document, path, marker, &shapes, endpoint, "ER")?;
        target_regions.push(probe);
        append_len_prefixed(&mut terminal, path_id.as_bytes());
        append_len_prefixed(&mut terminal, attribute.as_bytes());
        append_len_prefixed(&mut terminal, marker_kind.as_bytes());
        append_marker_terminal(&mut terminal, marker, &shapes);
        append_rect(&mut terminal, probe);
    }

    Ok(DeepFamilyRouteObservation {
        value: common_value,
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn mindmap_edge_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
    profile: CutoverWitnessProfile,
) -> C6ProofResult<DeepFamilyRouteObservation> {
    require_route(route, ThemeTarget::Edge, ThemeRouteCutoverFacet::Stroke)?;
    require_classic_profile(profile, "Mindmap Edge")?;
    require_edge_stroke_only(route, "Mindmap Edge")?;

    let paths = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path")
                && node.attribute("data-edge") == Some("true")
                && node.attribute("data-et") == Some("edge")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        paths.len() == 3,
        "Mindmap Edge witness expected three real data-edge paths, found {}",
        paths.len()
    );
    let expected_ids = ["edge_0_1", "edge_0_3", "edge_1_2"]
        .into_iter()
        .collect::<BTreeSet<_>>();
    let actual_ids = paths
        .iter()
        .filter_map(|path| path.attribute("data-id"))
        .collect::<BTreeSet<_>>();
    c6_ensure!(
        "route-svg-proof",
        actual_ids == expected_ids,
        "Mindmap Edge identities drifted: {actual_ids:?}"
    );

    let mut common_value = None;
    let mut target_regions = Vec::with_capacity(paths.len());
    let mut terminal = b"merman.c6-route-mindmap-edge-stroke.v1\0".to_vec();
    for path in paths {
        let id = path.attribute("data-id").expect("guarded Mindmap edge id");
        let expected_class = match id {
            "edge_0_1" => {
                "edge-thickness-normal edge-pattern-solid edge section-edge-0 edge-depth-1"
            }
            "edge_1_2" => {
                "edge-thickness-normal edge-pattern-solid edge section-edge-0 edge-depth-3"
            }
            "edge_0_3" => {
                "edge-thickness-normal edge-pattern-solid edge section-edge-1 edge-depth-1"
            }
            _ => unreachable!("guarded Mindmap edge id"),
        };
        let value = required_style_value(path, "stroke", "Mindmap edge path")?;
        observe_common_value(&mut common_value, value, "Mindmap edge path")?;
        c6_ensure!(
            "route-svg-proof",
            path.attribute("class") == Some(expected_class)
                && path.attribute("data-look") == Some("classic")
                && path
                    .attribute("data-points")
                    .is_some_and(|value| !value.is_empty())
                && path.attribute("d").is_some_and(|value| !value.is_empty())
                && path.attribute("marker-start").is_none()
                && path.attribute("marker-end").is_none(),
            "Mindmap edge {id} lost geometry or unexpectedly acquired a Marker: class={:?}, look={:?}, points={:?}, d={:?}, marker-start={:?}, marker-end={:?}",
            path.attribute("class"),
            path.attribute("data-look"),
            path.attribute("data-points"),
            path.attribute("d"),
            path.attribute("marker-start"),
            path.attribute("marker-end")
        );
        let region = element_bounds(path, route.facet())?;
        target_regions.push(region);
        append_node_terminal(&mut terminal, path, value, region);
    }

    Ok(DeepFamilyRouteObservation {
        value: common_value.ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                "Mindmap Edge witness has no stroke value",
            )
        })?,
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn gitgraph_edge_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
    profile: CutoverWitnessProfile,
) -> C6ProofResult<DeepFamilyRouteObservation> {
    require_route(route, ThemeTarget::Edge, ThemeRouteCutoverFacet::Stroke)?;
    require_classic_profile(profile, "GitGraph Edge")?;
    require_edge_stroke_only(route, "GitGraph Edge")?;

    let branches = document
        .descendants()
        .filter(|node| node.has_tag_name("line") && class_contains(*node, "branch"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        branches.len() == 2,
        "GitGraph Edge witness expected two line.branch terminals, found {}",
        branches.len()
    );
    let mut common_value = None;
    let mut branch_classes = BTreeSet::new();
    let mut target_regions = Vec::with_capacity(branches.len());
    let mut terminal = b"merman.c6-route-gitgraph-edge-stroke.v1\0".to_vec();
    for branch in branches {
        let class = branch.attribute("class").ok_or_else(|| {
            C6ProofError::new("route-svg-proof", "GitGraph branch line lacks a class")
        })?;
        c6_ensure!(
            "route-svg-proof",
            branch_classes.insert(class),
            "GitGraph Edge witness emitted duplicate branch class {class}"
        );
        let value = required_style_value(branch, "stroke", "GitGraph branch line")?;
        observe_common_value(&mut common_value, value, "GitGraph branch line")?;
        let region = element_bounds(branch, route.facet())?;
        target_regions.push(region);
        append_node_terminal(&mut terminal, branch, value, region);
    }
    let expected_branch_classes = ["branch branch0", "branch branch1"]
        .into_iter()
        .collect::<BTreeSet<_>>();
    c6_ensure!(
        "route-svg-proof",
        branch_classes == expected_branch_classes,
        "GitGraph Edge branch identities drifted: {branch_classes:?}"
    );
    let common_value = common_value.ok_or_else(|| {
        C6ProofError::new(
            "route-svg-proof",
            "GitGraph Edge witness has no stroke value",
        )
    })?;
    let arrows = document
        .descendants()
        .filter(|node| node.has_tag_name("path") && class_contains(*node, "arrow"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        !arrows.is_empty()
            && arrows
                .iter()
                .all(|arrow| style_value(*arrow, "stroke") != Some(common_value.as_str())),
        "GitGraph arrowN Node-palette terminals were incorrectly admitted as Edge.stroke"
    );
    terminal.extend_from_slice(&usize_to_u64(arrows.len()).to_be_bytes());
    for arrow in arrows {
        append_len_prefixed(
            &mut terminal,
            arrow.attribute("class").unwrap_or_default().as_bytes(),
        );
        append_len_prefixed(
            &mut terminal,
            arrow.attribute("d").unwrap_or_default().as_bytes(),
        );
    }

    Ok(DeepFamilyRouteObservation {
        value: common_value,
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn gantt_task_fill_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
    profile: CutoverWitnessProfile,
) -> C6ProofResult<DeepFamilyRouteObservation> {
    require_route(route, ThemeTarget::Task, ThemeRouteCutoverFacet::Fill)?;
    require_classic_profile(profile, "Gantt Task.fill")?;
    for projection in [
        ThemeRouteCutoverProjection::GanttTaskDefaultFill,
        ThemeRouteCutoverProjection::GanttTaskActiveFill,
        ThemeRouteCutoverProjection::GanttTaskSuccessFill,
        ThemeRouteCutoverProjection::GanttTaskErrorFill,
    ] {
        c6_ensure!(
            "route-svg-proof",
            route.projections().contains(projection),
            "Gantt Task.fill route lacks {}",
            projection.contribution_id()
        );
    }
    c6_ensure!(
        "route-svg-proof",
        route.projections().len() == 4,
        "Gantt Task.fill route has an unexpected projection"
    );

    let root_id = root_id(document, "Gantt")?;
    let task_stroke_width =
        stylesheet_property(document, &format!("#{root_id} .task"), "stroke-width")?
            .trim()
            .parse::<f64>()
            .map_err(|error| {
                C6ProofError::new(
                    "route-svg-proof",
                    format!("Gantt task stroke-width is not numeric: {error}"),
                )
            })?;
    c6_ensure!(
        "route-svg-proof",
        task_stroke_width.is_finite() && task_stroke_width > 0.0,
        "Gantt task stroke-width must be positive and finite"
    );

    let tasks = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && node
                    .attribute("class")
                    .and_then(|classes| classes.split_ascii_whitespace().next())
                    == Some("task")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        tasks.len() == 6,
        "Gantt Task.fill witness expected six state rects, found {}",
        tasks.len()
    );

    let expected = [
        (
            "default-task",
            "task0",
            ThemeRouteCutoverProjection::GanttTaskDefaultFill,
            "themeVariables.taskBkgColor",
        ),
        (
            "active-task",
            "active0",
            ThemeRouteCutoverProjection::GanttTaskActiveFill,
            "themeVariables.activeTaskBkgColor",
        ),
        (
            "done-task",
            "done0",
            ThemeRouteCutoverProjection::GanttTaskSuccessFill,
            "themeVariables.doneTaskBkgColor",
        ),
        (
            "crit-task",
            "crit0",
            ThemeRouteCutoverProjection::GanttTaskErrorFill,
            "themeVariables.critBkgColor",
        ),
        (
            "active-crit-task",
            "activeCrit0",
            ThemeRouteCutoverProjection::GanttTaskActiveFill,
            "themeVariables.activeTaskBkgColor",
        ),
        (
            "done-crit-task",
            "doneCrit0",
            ThemeRouteCutoverProjection::GanttTaskSuccessFill,
            "themeVariables.doneTaskBkgColor",
        ),
    ];
    let mut common_value = None;
    let mut seen = BTreeSet::new();
    let mut target_regions = Vec::with_capacity(tasks.len());
    let mut terminal = b"merman.c6-route-gantt-task-fill.v1\0".to_vec();
    for task in tasks {
        let id = task.attribute("id").ok_or_else(|| {
            C6ProofError::new("route-svg-proof", "Gantt task rect lacks a terminal id")
        })?;
        let (semantic_id, state_class, projection, final_fill_path) = expected
            .iter()
            .copied()
            .filter(|(semantic_id, _, _, _)| {
                id == *semantic_id
                    || id
                        .strip_suffix(semantic_id)
                        .is_some_and(|prefix| prefix.ends_with('-'))
            })
            .max_by_key(|(semantic_id, _, _, _)| semantic_id.len())
            .ok_or_else(|| {
                C6ProofError::new(
                    "route-svg-proof",
                    format!("unexpected Gantt task terminal {id}"),
                )
            })?;
        c6_ensure!(
            "route-svg-proof",
            seen.insert(semantic_id)
                && task.attribute("class") == Some(format!("task {state_class}").as_str()),
            "Gantt task {id} lost or duplicated state class {state_class}"
        );
        let value = required_style_value(task, "fill", "Gantt task rect")?;
        observe_common_value(&mut common_value, value, "Gantt task rect")?;
        c6_ensure!(
            "route-svg-proof",
            task.attribute("fill").is_none()
                && ["x", "y", "width", "height", "rx", "ry", "transform-origin"]
                    .into_iter()
                    .all(|attribute| task.attribute(attribute).is_some()),
            "Gantt task {id} lost its final inline owner or geometry"
        );
        // The SVG stroke is centered on the task rect.  Shrink the fill ROI by the
        // writer-owned stroke width so transparent proof observes only the interior
        // fill; the stroke remains covered by the terminal SVG geometry contract.
        let region = raw_element_bounds(task)?.rect(-task_stroke_width)?;
        target_regions.push(region);
        append_node_terminal(&mut terminal, task, value, region);
        append_len_prefixed(&mut terminal, semantic_id.as_bytes());
        append_len_prefixed(&mut terminal, state_class.as_bytes());
        append_len_prefixed(&mut terminal, projection.contribution_id().as_bytes());
        append_len_prefixed(&mut terminal, final_fill_path.as_bytes());
    }
    c6_ensure!(
        "route-svg-proof",
        seen.len() == expected.len(),
        "Gantt Task.fill did not cover every canonical state terminal"
    );

    Ok(DeepFamilyRouteObservation {
        value: common_value.ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                "Gantt Task.fill witness has no fill value",
            )
        })?,
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn gantt_task_stroke_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
    profile: CutoverWitnessProfile,
) -> C6ProofResult<DeepFamilyRouteObservation> {
    require_route(route, ThemeTarget::Task, ThemeRouteCutoverFacet::Stroke)?;
    require_classic_profile(profile, "Gantt Task.stroke")?;
    for projection in [
        ThemeRouteCutoverProjection::GanttTaskDefaultStroke,
        ThemeRouteCutoverProjection::GanttTaskActiveStroke,
        ThemeRouteCutoverProjection::GanttTaskSuccessStroke,
        ThemeRouteCutoverProjection::GanttTaskErrorStroke,
    ] {
        c6_ensure!(
            "route-svg-proof",
            route.projections().contains(projection),
            "Gantt Task.stroke route lacks {}",
            projection.contribution_id()
        );
    }
    c6_ensure!(
        "route-svg-proof",
        route.projections().len() == 4,
        "Gantt Task.stroke route has an unexpected projection"
    );

    let tasks = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && node
                    .attribute("class")
                    .and_then(|classes| classes.split_ascii_whitespace().next())
                    == Some("task")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        tasks.len() == 6,
        "Gantt Task.stroke witness expected six state rects, found {}",
        tasks.len()
    );

    let expected = [
        (
            "default-task",
            "task0",
            ThemeRouteCutoverProjection::GanttTaskDefaultStroke,
            "themeVariables.taskBorderColor",
        ),
        (
            "active-task",
            "active0",
            ThemeRouteCutoverProjection::GanttTaskActiveStroke,
            "themeVariables.activeTaskBorderColor",
        ),
        (
            "done-task",
            "done0",
            ThemeRouteCutoverProjection::GanttTaskSuccessStroke,
            "themeVariables.doneTaskBorderColor",
        ),
        (
            "crit-task",
            "crit0",
            ThemeRouteCutoverProjection::GanttTaskErrorStroke,
            "themeVariables.critBorderColor",
        ),
        (
            "active-crit-task",
            "activeCrit0",
            ThemeRouteCutoverProjection::GanttTaskActiveStroke,
            "themeVariables.activeTaskBorderColor",
        ),
        (
            "done-crit-task",
            "doneCrit0",
            ThemeRouteCutoverProjection::GanttTaskSuccessStroke,
            "themeVariables.doneTaskBorderColor",
        ),
    ];
    let mut common_value = None;
    let mut seen = BTreeSet::new();
    let mut target_regions = Vec::with_capacity(tasks.len());
    let mut terminal = b"merman.c6-route-gantt-task-stroke.v1\0".to_vec();
    for task in tasks {
        let id = task.attribute("id").ok_or_else(|| {
            C6ProofError::new("route-svg-proof", "Gantt task rect lacks a terminal id")
        })?;
        let (semantic_id, state_class, projection, final_stroke_path) = expected
            .iter()
            .copied()
            .filter(|(semantic_id, _, _, _)| {
                id == *semantic_id
                    || id
                        .strip_suffix(semantic_id)
                        .is_some_and(|prefix| prefix.ends_with('-'))
            })
            .max_by_key(|(semantic_id, _, _, _)| semantic_id.len())
            .ok_or_else(|| {
                C6ProofError::new(
                    "route-svg-proof",
                    format!("unexpected Gantt task terminal {id}"),
                )
            })?;
        c6_ensure!(
            "route-svg-proof",
            seen.insert(semantic_id)
                && task.attribute("class") == Some(format!("task {state_class}").as_str()),
            "Gantt task {id} lost or duplicated state class {state_class}"
        );
        let value = required_style_value(task, "stroke", "Gantt task rect")?;
        observe_common_value(&mut common_value, value, "Gantt task rect")?;
        c6_ensure!(
            "route-svg-proof",
            task.attribute("stroke").is_none()
                && ["x", "y", "width", "height", "rx", "ry", "transform-origin"]
                    .into_iter()
                    .all(|attribute| task.attribute(attribute).is_some()),
            "Gantt task {id} lost its final inline stroke owner or geometry"
        );
        let region = raw_element_bounds(task)?.rect(0.0)?;
        target_regions.push(region);
        append_node_terminal(&mut terminal, task, value, region);
        append_len_prefixed(&mut terminal, semantic_id.as_bytes());
        append_len_prefixed(&mut terminal, state_class.as_bytes());
        append_len_prefixed(&mut terminal, projection.contribution_id().as_bytes());
        append_len_prefixed(&mut terminal, final_stroke_path.as_bytes());
    }
    c6_ensure!(
        "route-svg-proof",
        seen.len() == expected.len(),
        "Gantt Task.stroke did not cover every canonical state terminal"
    );

    Ok(DeepFamilyRouteObservation {
        value: common_value.ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                "Gantt Task.stroke witness has no stroke value",
            )
        })?,
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn class_underlay_colors(document: &roxmltree::Document<'_>) -> C6ProofResult<Vec<[u8; 3]>> {
    let root_id = root_id(document, "Class")?;
    let selector = format!(
        "#{root_id} .node rect,#{root_id} .node circle,#{root_id} .node ellipse,#{root_id} .node polygon,#{root_id} .node path"
    );
    let mut colors = Vec::new();
    push_optional_color(
        &mut colors,
        stylesheet_property(document, &selector, "fill")?,
    )?;
    push_optional_color(
        &mut colors,
        stylesheet_property(document, &selector, "stroke")?,
    )?;
    let lollipop_selector = format!("#{root_id} [id$=\"-lollipopStart\"],#{root_id} .lollipop");
    push_optional_color(
        &mut colors,
        stylesheet_property(document, &lollipop_selector, "fill")?,
    )?;
    Ok(colors)
}

fn er_underlay_colors(document: &roxmltree::Document<'_>) -> C6ProofResult<Vec<[u8; 3]>> {
    let root_id = root_id(document, "ER")?;
    let selector = format!(
        "#{root_id} .node rect,#{root_id} .node circle,#{root_id} .node ellipse,#{root_id} .node polygon"
    );
    let mut colors = Vec::new();
    push_optional_color(
        &mut colors,
        stylesheet_property(document, &selector, "fill")?,
    )?;
    push_optional_color(
        &mut colors,
        stylesheet_property(document, &selector, "stroke")?,
    )?;
    for fill in document
        .descendants()
        .filter(|node| matches!(node.tag_name().name(), "path" | "circle" | "polygon"))
        .filter(|node| {
            node.ancestors()
                .any(|ancestor| ancestor.has_tag_name("marker"))
        })
        .filter_map(|node| node.attribute("fill"))
    {
        push_optional_color(&mut colors, fill)?;
    }
    Ok(colors)
}

fn prove_gantt_transparent_underlay(document: &roxmltree::Document<'_>) -> C6ProofResult<()> {
    let root_id = root_id(document, "Gantt")?;
    let section = stylesheet_property(document, &format!("#{root_id} .section0"), "fill")?;
    let grid = stylesheet_property(document, &format!("#{root_id} .grid .tick"), "stroke")?;
    c6_ensure!(
        "route-svg-proof",
        parse_optional_css_rgb(section)?.is_none() && parse_optional_css_rgb(grid)?.is_none(),
        "Gantt Task.fill witness did not isolate its section/grid underlay"
    );
    Ok(())
}

fn gantt_task_text_underlay_colors(
    document: &roxmltree::Document<'_>,
) -> C6ProofResult<Vec<[u8; 3]>> {
    let root_id = root_id(document, "Gantt")?;
    let mut colors = Vec::new();
    for text in document.descendants().filter(|node| {
        node.has_tag_name("text")
            && node.attribute("class").is_some_and(|classes| {
                classes
                    .split_ascii_whitespace()
                    .any(|class| class.contains("Text"))
            })
    }) {
        if let Some(raw) = style_value(text, "fill").or_else(|| text.attribute("fill")) {
            push_optional_color(&mut colors, raw)?;
        }
        for class in text
            .attribute("class")
            .unwrap_or_default()
            .split_ascii_whitespace()
            .filter(|class| class.contains("Text"))
        {
            for raw in gantt_stylesheet_fill_values_for_class(document, &root_id, class) {
                push_optional_color(&mut colors, raw)?;
            }
        }
    }
    c6_ensure!(
        "route-svg-proof",
        !colors.is_empty(),
        "Gantt Task.fill witness lacks independently observed task-text underlay colors"
    );
    Ok(colors)
}

fn gantt_task_fill_underlay_colors(
    document: &roxmltree::Document<'_>,
) -> C6ProofResult<Vec<[u8; 3]>> {
    let root_id = root_id(document, "Gantt")?;
    let mut colors = Vec::new();
    for task in document.descendants().filter(|node| {
        node.has_tag_name("rect")
            && node
                .attribute("class")
                .is_some_and(|classes| classes.split_ascii_whitespace().next() == Some("task"))
    }) {
        if let Some(raw) = style_value(task, "fill").or_else(|| task.attribute("fill")) {
            push_optional_color(&mut colors, raw)?;
        }
        for class in task
            .attribute("class")
            .unwrap_or_default()
            .split_ascii_whitespace()
        {
            for raw in gantt_stylesheet_fill_values_for_class(document, &root_id, class) {
                push_optional_color(&mut colors, raw)?;
            }
        }
    }
    Ok(colors)
}

fn gantt_stylesheet_fill_values_for_class<'a>(
    document: &'a roxmltree::Document<'a>,
    root_id: &str,
    class: &str,
) -> Vec<&'a str> {
    let marker = format!(".{class}");
    let root_marker = format!("#{root_id}");
    let mut values = Vec::new();
    for css in document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
    {
        for rule in css.split('}') {
            let Some((selectors, declarations)) = rule.split_once('{') else {
                continue;
            };
            let matches_class = selectors.split(',').map(str::trim).any(|selector| {
                selector.contains(&root_marker)
                    && selector.match_indices(&marker).any(|(offset, _)| {
                        selector[offset + marker.len()..]
                            .chars()
                            .next()
                            .is_none_or(|character| {
                                !character.is_ascii_alphanumeric()
                                    && character != '-'
                                    && character != '_'
                            })
                    })
            });
            if !matches_class {
                continue;
            }
            for declaration in declarations.split(';') {
                let Some((name, value)) = declaration.split_once(':') else {
                    continue;
                };
                if name.trim() == "fill" {
                    values.push(
                        value
                            .trim()
                            .strip_suffix("!important")
                            .unwrap_or(value.trim())
                            .trim(),
                    );
                }
            }
        }
    }
    values
}

fn require_route(
    route: ThemeRouteCutoverDescriptor,
    target: ThemeTarget,
    facet: ThemeRouteCutoverFacet,
) -> C6ProofResult<()> {
    c6_ensure!(
        "route-svg-proof",
        route.target() == target && route.facet() == facet,
        "unsupported deep-family cutover route {}",
        route_label(route)
    );
    Ok(())
}

fn require_classic_profile(profile: CutoverWitnessProfile, label: &str) -> C6ProofResult<()> {
    c6_ensure!(
        "route-svg-proof",
        profile == CutoverWitnessProfile::ClassicStatic,
        "{label} witness uses unsupported profile {}",
        profile.id()
    );
    Ok(())
}

fn require_edge_stroke_only(route: ThemeRouteCutoverDescriptor, label: &str) -> C6ProofResult<()> {
    c6_ensure!(
        "route-svg-proof",
        route.projections().len() == 1
            && route
                .projections()
                .contains(ThemeRouteCutoverProjection::EdgeStroke),
        "{label} route lost its EdgeStroke-only projection contract"
    );
    Ok(())
}

fn required_style_value<'a>(
    node: roxmltree::Node<'a, '_>,
    property: &str,
    label: &str,
) -> C6ProofResult<&'a str> {
    style_value(node, property).ok_or_else(|| {
        C6ProofError::new(
            "route-svg-proof",
            format!("{label} lacks its final inline {property}"),
        )
    })
}

fn observe_common_value(
    common: &mut Option<String>,
    actual: &str,
    label: &str,
) -> C6ProofResult<()> {
    if let Some(expected) = common.as_deref() {
        c6_ensure!(
            "route-svg-proof",
            expected == actual,
            "{label} terminals disagree: `{expected}` != `{actual}`"
        );
    } else {
        *common = Some(actual.to_owned());
    }
    Ok(())
}

fn marker_reference_id<'a>(reference: &'a str, family: &str) -> C6ProofResult<&'a str> {
    reference
        .strip_prefix("url(#")
        .and_then(|value| value.strip_suffix(')'))
        .ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                format!("invalid {family} marker reference `{reference}`"),
            )
        })
}

fn exact_marker<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    marker_id: &str,
    family: &str,
) -> C6ProofResult<roxmltree::Node<'a, 'input>> {
    let markers = document
        .descendants()
        .filter(|node| node.has_tag_name("marker") && node.attribute("id") == Some(marker_id))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        markers.len() == 1,
        "{family} marker {marker_id} expected one definition, found {}",
        markers.len()
    );
    Ok(markers[0])
}

fn marker_shapes<'a, 'input>(
    marker: roxmltree::Node<'a, 'input>,
) -> Vec<roxmltree::Node<'a, 'input>> {
    marker
        .children()
        .filter(|node| {
            node.is_element() && matches!(node.tag_name().name(), "path" | "polygon" | "circle")
        })
        .collect()
}

fn class_marker_kind(marker_id: &str) -> Option<&'static str> {
    [
        "aggregationStart",
        "aggregationEnd",
        "extensionStart",
        "extensionEnd",
        "compositionStart",
        "compositionEnd",
        "dependencyStart",
        "dependencyEnd",
        "lollipopStart",
        "lollipopEnd",
    ]
    .into_iter()
    .find(|kind| marker_id.ends_with(kind))
}

fn er_marker_kind(marker_id: &str) -> Option<&'static str> {
    ["onlyOneStart", "zeroOrMoreEnd"]
        .into_iter()
        .find(|kind| marker_id.ends_with(kind))
}

fn class_marker_class(marker_kind: &str) -> &'static str {
    if marker_kind.starts_with("aggregation") {
        "aggregation"
    } else if marker_kind.starts_with("extension") {
        "extension"
    } else if marker_kind.starts_with("composition") {
        "composition"
    } else if marker_kind.starts_with("dependency") {
        "dependency"
    } else {
        "lollipop"
    }
}

fn er_marker_class(marker_kind: &str) -> &'static str {
    if marker_kind.starts_with("onlyOne") {
        "onlyOne"
    } else {
        "zeroOrMore"
    }
}

fn marker_direction_matches(attribute: &str, marker_kind: &str) -> bool {
    matches!(
        (
            attribute,
            marker_kind.ends_with("Start"),
            marker_kind.ends_with("End")
        ),
        ("marker-start", true, false) | ("marker-end", false, true)
    )
}

fn marker_occurrence_region(
    document: &roxmltree::Document<'_>,
    path: roxmltree::Node<'_, '_>,
    marker: roxmltree::Node<'_, '_>,
    shapes: &[roxmltree::Node<'_, '_>],
    endpoint: (f64, f64),
    family: &str,
) -> C6ProofResult<[f64; 4]> {
    c6_ensure!(
        "route-svg-geometry",
        marker.attribute("orient") == Some("auto") && marker.attribute("viewBox").is_none(),
        "{family} marker changed its auto-oriented local coordinate contract"
    );
    let ref_x = number_attribute(marker, "refX")?;
    let ref_y = number_attribute(marker, "refY")?;
    let marker_width = number_attribute(marker, "markerWidth")?;
    let marker_height = number_attribute(marker, "markerHeight")?;
    c6_ensure!(
        "route-svg-geometry",
        marker_width > 0.0 && marker_height > 0.0,
        "{family} marker viewport must be positive"
    );

    let mut bounds: Option<SvgBounds> = None;
    let mut stroke_outset = 0.0_f64;
    for shape in shapes {
        let shape_bounds = raw_element_bounds(*shape)?;
        match &mut bounds {
            Some(bounds) => bounds.union(shape_bounds),
            None => bounds = Some(shape_bounds),
        }
        stroke_outset = stroke_outset.max(match shape.tag_name().name() {
            "path" | "polygon" => 4.0,
            "circle" => 0.5,
            _ => 0.0,
        });
    }
    let bounds = bounds.ok_or_else(|| {
        C6ProofError::new(
            "route-svg-geometry",
            format!("{family} marker has no paint geometry"),
        )
    })?;
    let coordinate_scale = match marker.attribute("markerUnits") {
        None | Some("strokeWidth") => terminal_stroke_width(document, path, family)?,
        Some("userSpaceOnUse") => 1.0,
        Some(units) => {
            return Err(C6ProofError::new(
                "route-svg-geometry",
                format!("unsupported {family} markerUnits `{units}`"),
            ));
        }
    };
    let transform_scale = affine_linear_scale(node_transform(path)?)?;
    let radius = [
        (bounds.left - stroke_outset, bounds.top - stroke_outset),
        (bounds.left - stroke_outset, bounds.bottom + stroke_outset),
        (bounds.right + stroke_outset, bounds.top - stroke_outset),
        (bounds.right + stroke_outset, bounds.bottom + stroke_outset),
    ]
    .into_iter()
    .map(|(x, y)| (x - ref_x).hypot(y - ref_y))
    .fold(0.0, f64::max)
        * coordinate_scale
        * transform_scale
        + MARKER_RASTER_OUTSET;
    c6_ensure!(
        "route-svg-geometry",
        endpoint.0.is_finite()
            && endpoint.1.is_finite()
            && radius.is_finite()
            && radius > MARKER_RASTER_OUTSET,
        "{family} marker occurrence has invalid terminal geometry"
    );
    Ok(marker_probe(endpoint, radius))
}

fn terminal_stroke_width(
    document: &roxmltree::Document<'_>,
    path: roxmltree::Node<'_, '_>,
    family: &str,
) -> C6ProofResult<f64> {
    let raw = if let Some(raw) =
        style_value(path, "stroke-width").or_else(|| path.attribute("stroke-width"))
    {
        raw
    } else {
        let root_id = root_id(document, family)?;
        let selector = match family {
            "Class" => format!("#{root_id} .relation"),
            "ER" => format!("#{root_id} .relationshipLine"),
            _ => {
                return Err(C6ProofError::new(
                    "route-svg-geometry",
                    format!("no marker stroke-width selector for {family}"),
                ));
            }
        };
        stylesheet_property(document, &selector, "stroke-width")?
    };
    let width = raw
        .trim()
        .strip_suffix("px")
        .unwrap_or(raw.trim())
        .parse::<f64>()
        .map_err(|error| {
            C6ProofError::new(
                "route-svg-geometry",
                format!("invalid {family} terminal stroke-width `{raw}`: {error}"),
            )
        })?;
    c6_ensure!(
        "route-svg-geometry",
        width.is_finite() && width > 0.0,
        "{family} terminal stroke-width must be finite and positive"
    );
    Ok(width)
}

fn affine_linear_scale(transform: SvgAffineTransform) -> C6ProofResult<f64> {
    let xx = transform.a * transform.a + transform.b * transform.b;
    let yy = transform.c * transform.c + transform.d * transform.d;
    let xy = transform.a * transform.c + transform.b * transform.d;
    let discriminant = ((xx - yy) * (xx - yy) + 4.0 * xy * xy).sqrt();
    let scale = ((xx + yy + discriminant) * 0.5).sqrt();
    c6_ensure!(
        "route-svg-geometry",
        scale.is_finite() && scale > 0.0,
        "marker ancestor transform has an invalid linear scale"
    );
    Ok(scale)
}

fn marker_probe(endpoint: (f64, f64), radius: f64) -> [f64; 4] {
    [
        endpoint.0 - radius,
        endpoint.1 - radius,
        radius * 2.0,
        radius * 2.0,
    ]
}

fn append_node_terminal(
    terminal: &mut Vec<u8>,
    node: roxmltree::Node<'_, '_>,
    value: &str,
    region: [f64; 4],
) {
    append_len_prefixed(terminal, node.tag_name().name().as_bytes());
    for attribute in [
        "id",
        "class",
        "style",
        "data-id",
        "data-edge",
        "data-et",
        "data-look",
        "data-points",
        "fill",
        "stroke",
        "stroke-width",
        "d",
        "x",
        "y",
        "x1",
        "y1",
        "x2",
        "y2",
        "width",
        "height",
        "rx",
        "ry",
        "transform-origin",
        "marker-start",
        "marker-end",
    ] {
        append_len_prefixed(
            terminal,
            node.attribute(attribute).unwrap_or_default().as_bytes(),
        );
    }
    append_len_prefixed(terminal, value.as_bytes());
    append_rect(terminal, region);
}

fn append_marker_terminal(
    terminal: &mut Vec<u8>,
    marker: roxmltree::Node<'_, '_>,
    shapes: &[roxmltree::Node<'_, '_>],
) {
    for attribute in [
        "id",
        "class",
        "style",
        "viewBox",
        "refX",
        "refY",
        "markerWidth",
        "markerHeight",
        "markerUnits",
        "orient",
    ] {
        append_len_prefixed(
            terminal,
            marker.attribute(attribute).unwrap_or_default().as_bytes(),
        );
    }
    terminal.extend_from_slice(&usize_to_u64(shapes.len()).to_be_bytes());
    for shape in shapes {
        append_len_prefixed(terminal, shape.tag_name().name().as_bytes());
        for attribute in [
            "class",
            "style",
            "fill",
            "stroke",
            "stroke-width",
            "viewBox",
            "d",
            "points",
            "cx",
            "cy",
            "r",
        ] {
            append_len_prefixed(
                terminal,
                shape.attribute(attribute).unwrap_or_default().as_bytes(),
            );
        }
    }
}

fn root_id<'document, 'input: 'document>(
    document: &'document roxmltree::Document<'input>,
    family: &str,
) -> C6ProofResult<&'document str> {
    document.root_element().attribute("id").ok_or_else(|| {
        C6ProofError::new("route-svg-proof", format!("{family} SVG root lacks an id"))
    })
}

fn push_optional_color(colors: &mut Vec<[u8; 3]>, raw: &str) -> C6ProofResult<()> {
    let raw = raw
        .trim()
        .strip_suffix("!important")
        .unwrap_or(raw.trim())
        .trim();
    if let Some(color) = parse_optional_css_rgb(raw)?
        && !colors.contains(&color)
    {
        colors.push(color);
    }
    Ok(())
}
