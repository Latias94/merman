//! Block-owned source-order grid packing over the flat semantic hierarchy.

mod pack;
mod validate;

use crate::error::{AsciiError, Result};
use crate::graph::positioned::{
    PositionedEdge, PositionedGraph, PositionedMarker, PositionedShape, PositionedSize,
    PositionedStroke, PositionedTextAdmission,
};
use crate::graph::{DeferredGraphLabelSectionPlan, DeferredGraphNodeLabelPlan};
use crate::operation::AsciiExecution;
use crate::options::{AsciiRenderOptions, GraphLayoutPolicy};
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use merman_model::OperationPhase;
use merman_model::families::block::{
    BlockDiagramModel, BlockEdgePattern, BlockEdgeThickness, BlockNode,
};

pub(crate) fn render_block_diagram_with_execution(
    model: &BlockDiagramModel,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    execution.checkpoint(OperationPhase::Layout)?;
    options.validate()?;
    let mut resources = execution.new_resource_context(OperationPhase::Layout);
    let transaction = resources.clone();
    transaction.transaction(|_| {
        let candidate = transaction.transaction_preserving_layout_work(|_| {
            render_inner(model, options, &mut resources, execution)
        });
        match candidate {
            Err(error)
                if execution.preserves_graph_work_on_error(&error, options.graph_presentation) =>
            {
                Ok(Err(error))
            }
            Ok(rendered) => Ok(Ok(rendered)),
            Err(error) => Err(error),
        }
    })?
}

fn render_inner(
    model: &BlockDiagramModel,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let hierarchy = validate::validate(model, resources, execution)?;
    if hierarchy.root.is_none() {
        return Ok(String::new());
    }
    let policy = block_policy(options);
    let mut graph = PositionedGraph::try_new(
        "block",
        policy,
        hierarchy.preorder.len(),
        model.edges.len(),
        resources,
    )?;
    let mut items = pack::prepare_items(model.blocks_flat.len())?;
    project_sources(
        model, &hierarchy, options, &mut graph, &mut items, resources,
    )?;
    pack::pack(model, &hierarchy, &mut items, resources, execution)?;
    pack::place(model, &hierarchy, &mut items, &mut graph, resources)?;
    graph.render(options, resources, execution)
}

/// Copies the admitted source batch without rescanning labels or replaying normalized rows.
fn project_sources(
    model: &BlockDiagramModel,
    hierarchy: &validate::Hierarchy,
    options: &AsciiRenderOptions,
    graph: &mut PositionedGraph,
    items: &mut [pack::Item],
    resources: &ResourceContext,
) -> Result<()> {
    let annotations = prepare_annotations(model, hierarchy, resources)?;
    let mut plans = plan_projection_text(model, hierarchy, options, &annotations, resources)?;
    for &index in &hierarchy.preorder {
        resources.checkpoint()?;
        let node = &model.blocks_flat[index];
        items[index].span = validate::span(node, resources)?;
        if Some(index) == hierarchy.root || node.block_type == "space" {
            continue;
        }
        let text = plans[index]
            .take()
            .ok_or_else(|| unsupported("missing Block label plan"))?
            .materialize_after_admission(resources)?;
        let (element, minimum) = if node.block_type == "composite" {
            graph.try_add_frame(
                &node.id,
                text,
                node.children.iter().filter_map(|&child| {
                    let node = &model.blocks_flat[child];
                    (node.block_type != "space").then_some(node.id.as_str())
                }),
                resources,
            )?
        } else {
            let (shape, _) = shape(node)?;
            graph.try_add_node(&node.id, text, shape, resources)?
        };
        items[index].element = Some(element);
        items[index].minimum = minimum;
        if node.block_type == "composite" {
            // Positioned frame metrics reserve title rows before any child geometry.
            items[index].inset = PositionedSize {
                width: 2,
                height: minimum.height - 2,
            };
        }
    }
    for edge in &model.edges {
        resources.checkpoint()?;
        let stroke = match (edge.thickness, edge.pattern) {
            (BlockEdgeThickness::Normal, BlockEdgePattern::Solid) => PositionedStroke::Normal,
            (BlockEdgeThickness::Thick, BlockEdgePattern::Solid) => PositionedStroke::Thick,
            (BlockEdgeThickness::Normal, BlockEdgePattern::Dotted) => PositionedStroke::Dotted,
            (BlockEdgeThickness::Thick, BlockEdgePattern::Dotted) => {
                return Err(unsupported("simultaneously thick and dotted Block edges"));
            }
        };
        graph.try_add_edge(
            PositionedEdge {
                id: &edge.id,
                from: &edge.start,
                to: &edge.end,
                label: &edge.label,
                stroke,
                start_marker: marker(edge.arrow_type_start.as_deref())?,
                end_marker: marker(edge.arrow_type_end.as_deref())?,
            },
            resources,
        )?;
    }
    Ok(())
}

fn block_policy(options: &AsciiRenderOptions) -> GraphLayoutPolicy {
    // Columns are authored, so Flowchart's direction, wrapping and compact rank policy do not
    // participate. Host glyph/width choices and explicit node padding still apply.
    GraphLayoutPolicy {
        routing_space: Default::default(),
        node_padding_x: options.node_padding_x.unwrap_or(options.box_border_padding),
        node_padding_y: options.node_padding_y.unwrap_or(0),
        rank_gap_x: pack::COLUMN_GAP,
        rank_gap_y: pack::ROW_GAP,
        group_padding_x: 2,
        group_padding_y: 2,
        group_title_clearance: 3,
        edge_label_lane_radius: GraphLayoutPolicy::DEFAULT_EDGE_LABEL_LANE_RADIUS,
        structural_charset: options.structural_charset(),
        terminal_width_profile: options.terminal_width_profile,
    }
}

fn plan_projection_text<'a>(
    model: &'a BlockDiagramModel,
    hierarchy: &validate::Hierarchy,
    options: &AsciiRenderOptions,
    annotations: &'a [Option<String>],
    resources: &ResourceContext,
) -> Result<Vec<Option<DeferredGraphNodeLabelPlan<'a>>>> {
    let mut plans = Vec::new();
    validate::reserve(&mut plans, model.blocks_flat.len())?;
    plans.resize_with(model.blocks_flat.len(), || None);
    let mut admission = PositionedTextAdmission::default();
    for &index in &hierarchy.preorder {
        resources.charge_layout_work(1)?;
        admission.include_work(1, resources)?;
        let node = &model.blocks_flat[index];
        if Some(index) == hierarchy.root || node.block_type == "space" {
            continue;
        }
        let label = annotations[index].as_deref().unwrap_or(&node.label);
        let plan = DeferredGraphNodeLabelPlan::single(
            DeferredGraphLabelSectionPlan::try_single(
                label,
                None,
                "block",
                options.terminal_width_profile,
                resources,
            )?,
            None,
            "block",
            options.terminal_width_profile,
            resources,
        )?;
        admission.include_label(&plan, resources)?;
        if annotations[index].is_some() {
            // The bounded annotation assembly remains live until the complete source batch
            // has been copied. Count it alongside the final projected label, not as free storage.
            admission.include_retained_bytes(label.len(), resources)?;
        }
        admission.include_copy_work(resources.checked_work_mul(node.id.len(), 2)?, resources)?;
        if node.block_type == "composite" {
            for &child in &node.children {
                resources.charge_layout_work(1)?;
                let child = &model.blocks_flat[child];
                if child.block_type != "space" {
                    admission.include_work(1, resources)?;
                    admission.include_copy_work(child.id.len(), resources)?;
                }
            }
        }
        plans[index] = Some(plan);
    }
    for edge in &model.edges {
        resources.charge_layout_work(1)?;
        admission.include_work(1, resources)?;
        for text in [&edge.id, &edge.start, &edge.end] {
            admission.include_copy_work(text.len(), resources)?;
        }
        admission.include_edge_label(&edge.label, options.terminal_width_profile, resources)?;
    }
    admission.admit(resources)?;
    Ok(plans)
}

/// Annotations are same-row source prefixes, so they need a bounded intermediate before the
/// final borrowed label plans can be built. Admit the whole assembly before copying any label.
fn prepare_annotations(
    model: &BlockDiagramModel,
    hierarchy: &validate::Hierarchy,
    resources: &ResourceContext,
) -> Result<Vec<Option<String>>> {
    let mut annotations = validate::filled_vec(model.blocks_flat.len(), None)?;
    let mut lengths = validate::filled_vec(model.blocks_flat.len(), None)?;
    let mut bytes = 0usize;
    for &index in &hierarchy.preorder {
        resources.charge_layout_work(1)?;
        let node = &model.blocks_flat[index];
        if Some(index) == hierarchy.root
            || matches!(node.block_type.as_str(), "space" | "composite")
        {
            continue;
        }
        let (_, role) = shape(node)?;
        if let Some(role) = role {
            let mut length = resources.checked_work_add(node.label.len(), role.len())?;
            length = resources.checked_work_add(length, 3)?;
            for direction in &node.directions {
                resources.charge_layout_work(1)?;
                length = resources.checked_work_add(length, direction.len())?;
                length = resources.checked_work_add(length, 1)?;
            }
            bytes = bytes
                .checked_add(length)
                .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
            lengths[index] = Some(length);
        }
    }
    resources.check(AsciiResourceLimitId::MaxOutputBytes, bytes)?;
    resources.charge_layout_work(bytes)?;
    for &index in &hierarchy.preorder {
        resources.checkpoint()?;
        let Some(length) = lengths[index] else {
            continue;
        };
        let node = &model.blocks_flat[index];
        let (_, role) = shape(node)?;
        let role = role.ok_or_else(|| unsupported("missing Block annotation role"))?;
        annotations[index] = Some(annotated_label(node, role, length, resources)?);
    }
    Ok(annotations)
}

fn marker(value: Option<&str>) -> Result<PositionedMarker> {
    match value {
        None | Some("" | "arrow_open") => Ok(PositionedMarker::Open),
        Some("arrow_point") => Ok(PositionedMarker::Point),
        Some("arrow_circle") => Ok(PositionedMarker::Circle),
        Some("arrow_cross") => Ok(PositionedMarker::Cross),
        Some(_) => Err(unsupported("unknown Block edge marker")),
    }
}

fn shape(node: &BlockNode) -> Result<(PositionedShape, Option<&'static str>)> {
    let shape = match node.block_type.as_str() {
        "na" | "square" => PositionedShape::Rect,
        "round" => PositionedShape::Rounded,
        "circle" => PositionedShape::Circle,
        "doublecircle" => PositionedShape::DoubleCircle,
        "stadium" => PositionedShape::Stadium,
        "subroutine" => PositionedShape::Subroutine,
        "cylinder" => PositionedShape::Cylinder,
        "diamond" => PositionedShape::Diamond,
        "hexagon" => PositionedShape::Hexagon,
        "lean_right" => PositionedShape::LeanRight,
        "lean_left" => PositionedShape::LeanLeft,
        "trapezoid" => PositionedShape::Trapezoid,
        "inv_trapezoid" => PositionedShape::TrapezoidAlt,
        "rect_left_inv_arrow" => {
            return Ok((PositionedShape::Rect, Some("rect_left_inv_arrow")));
        }
        "block_arrow" => return Ok((PositionedShape::Rect, Some("block-arrow"))),
        _ => return Err(unsupported("unknown Block shape")),
    };
    Ok((shape, None))
}

fn annotated_label(
    node: &BlockNode,
    role: &str,
    length: usize,
    resources: &ResourceContext,
) -> Result<String> {
    resources.checkpoint()?;
    let mut label = String::new();
    label
        .try_reserve(length)
        .map_err(|_| AsciiError::allocation_failed("ascii_layout_work"))?;
    label.push('[');
    label.push_str(role);
    for (index, direction) in node.directions.iter().enumerate() {
        resources.checkpoint()?;
        label.push(if index == 0 { ':' } else { '+' });
        label.push_str(direction);
    }
    label.push_str("] ");
    let mut start = 0;
    while start < node.label.len() {
        resources.checkpoint()?;
        let mut end = start.saturating_add(8 * 1024).min(node.label.len());
        while !node.label.is_char_boundary(end) {
            end -= 1;
        }
        label.push_str(&node.label[start..end]);
        start = end;
    }
    Ok(label)
}

pub(super) fn unsupported(feature: &'static str) -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "block",
        feature,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TerminalWidthProfile;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_model::OperationControl;

    fn model(label: &str) -> BlockDiagramModel {
        BlockDiagramModel {
            root: Some(0),
            blocks_flat: vec![
                BlockNode {
                    id: "root".into(),
                    block_type: "composite".into(),
                    children: vec![1, 2],
                    ..BlockNode::default()
                },
                BlockNode {
                    id: "A".into(),
                    label: label.into(),
                    block_type: "square".into(),
                    ..BlockNode::default()
                },
                BlockNode {
                    id: "B".into(),
                    label: label.into(),
                    block_type: "square".into(),
                    ..BlockNode::default()
                },
            ],
            ..BlockDiagramModel::default()
        }
    }

    #[test]
    fn aggregate_raw_text_rejects_before_normalization_or_projection_copies() {
        let model = model(&"x".repeat(128 * 1024));
        // Each individual label is below this ceiling; their complete batch is above it.
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, 192 * 1024)
            .unwrap();
        let control = OperationControl::new();
        let execution = AsciiExecution::new(&control, &policy);
        let resources = execution.new_resource_context(OperationPhase::Layout);
        assert!(matches!(
            validate::validate(&model, &resources, execution),
            Err(AsciiError::ResourceLimitExceeded(error))
                if error.limit == AsciiResourceLimitId::MaxOutputBytes
        ));
        assert_eq!(resources.document_cells_used(), 0);
    }

    #[test]
    fn borrowed_label_batch_accepts_exact_display_cells_and_rejects_one_less() {
        let model = model(&"·".repeat(32));
        for (profile, exact) in [
            (TerminalWidthProfile::Unicode, 64),
            (TerminalWidthProfile::Cjk, 128),
        ] {
            for limit in [exact, exact - 1] {
                let policy = AsciiResourcePolicy::default()
                    .with_limit(AsciiResourceLimitId::MaxDocumentCells, limit)
                    .unwrap();
                let control = OperationControl::new();
                let execution = AsciiExecution::new(&control, &policy);
                let resources = execution.new_resource_context(OperationPhase::Layout);
                let hierarchy = validate::validate(&model, &resources, execution).unwrap();
                let annotations = prepare_annotations(&model, &hierarchy, &resources).unwrap();
                let result = plan_projection_text(
                    &model,
                    &hierarchy,
                    &AsciiRenderOptions::ascii().with_terminal_width_profile(profile),
                    &annotations,
                    &resources,
                );
                if limit == exact {
                    result.unwrap();
                } else {
                    assert!(
                        matches!(&result, Err(AsciiError::ResourceLimitExceeded(error))
                        if error.limit == AsciiResourceLimitId::MaxDocumentCells),
                        "{profile:?}/{limit}: {result:?}"
                    );
                }
                // This pass has retained borrowed plans only, without materializing graph text.
                assert_eq!(resources.document_cells_used(), 0);
            }
        }
    }

    #[test]
    fn source_copy_batch_accepts_exact_work_before_any_row_replay() {
        let label = "客户α👩‍💻".repeat(4 * 1024);
        let plain = model(&label);
        let mut annotated = model(&label);
        annotated.blocks_flat[1].block_type = "block_arrow".into();
        annotated.blocks_flat[1].directions = vec!["right".into(), "down".into()];
        annotated.blocks_flat[2].block_type = "composite".into();
        annotated.blocks_flat[2].children = vec![3];
        annotated.blocks_flat.push(BlockNode {
            id: "child".into(),
            label: label.clone(),
            block_type: "rect_left_inv_arrow".into(),
            ..BlockNode::default()
        });
        annotated
            .edges
            .push(merman_model::families::block::BlockEdge {
                id: "edge".into(),
                start: "A".into(),
                end: "child".into(),
                label: "edge label\nsecond row".into(),
                ..Default::default()
            });
        let options =
            AsciiRenderOptions::ascii().with_terminal_width_profile(TerminalWidthProfile::Cjk);
        for model in [plain, annotated] {
            let unbounded = AsciiResourcePolicy::unbounded();
            let resources = ResourceContext::new(unbounded);
            let hierarchy =
                validate::validate(&model, &resources, AsciiExecution::for_test(&unbounded))
                    .unwrap();
            let mut graph = PositionedGraph::try_new(
                "block",
                block_policy(&options),
                hierarchy.preorder.len(),
                model.edges.len(),
                &resources,
            )
            .unwrap();
            let mut items = pack::prepare_items(model.blocks_flat.len()).unwrap();
            project_sources(
                &model, &hierarchy, &options, &mut graph, &mut items, &resources,
            )
            .unwrap();
            let exact = resources.layout_work_used();

            for limit in [exact, exact - 1] {
                let policy = unbounded
                    .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
                    .unwrap();
                let resources = ResourceContext::new(policy);
                let hierarchy =
                    validate::validate(&model, &resources, AsciiExecution::for_test(&policy))
                        .unwrap();
                let mut graph = PositionedGraph::try_new(
                    "block",
                    block_policy(&options),
                    hierarchy.preorder.len(),
                    model.edges.len(),
                    &resources,
                )
                .unwrap();
                let mut items = pack::prepare_items(model.blocks_flat.len()).unwrap();
                let result = project_sources(
                    &model, &hierarchy, &options, &mut graph, &mut items, &resources,
                );
                if limit == exact {
                    result.expect("exact source-copy work must materialize the complete batch");
                    assert!(
                        hierarchy
                            .preorder
                            .iter()
                            .filter(|&&index| Some(index) != hierarchy.root)
                            .all(|&index| items[index].element.is_some())
                    );
                    assert_eq!(resources.layout_work_used(), exact);
                    assert_eq!(resources.document_cells_used(), 0);
                    // Row replay is a distinct later phase. Exhaustion here must not be moved
                    // into the middle of source copies by repeated label scans/materialization.
                    assert!(matches!(graph.materialize_labels(&resources),
                        Err(AsciiError::ResourceLimitExceeded(error))
                            if error.limit == AsciiResourceLimitId::MaxLayoutWorkUnits));
                } else {
                    assert!(
                        matches!(result, Err(AsciiError::ResourceLimitExceeded(error))
                        if error.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                            && error.actual == exact && error.max == limit)
                    );
                    assert!(
                        items.iter().all(|item| item.element.is_none()),
                        "the aggregate rejection must precede the first projected source"
                    );
                    assert_eq!(resources.document_cells_used(), 0);
                }
            }
        }
    }

    #[test]
    fn annotation_intermediates_have_an_exact_aggregate_copy_boundary() {
        let label = "客户👩‍💻".repeat(2 * 1024);
        let mut model = model(&label);
        for node in &mut model.blocks_flat[1..] {
            node.block_type = "block_arrow".into();
            node.directions = ["right", "down", "left", "up", "x", "y"]
                .map(str::to_string)
                .into();
        }
        let expected = format!("[block-arrow:right+down+left+up+x+y] {label}");
        let exact = expected.len() * 2;
        for limit in [exact, exact - 1] {
            let policy = AsciiResourcePolicy::unbounded()
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, limit)
                .unwrap();
            let resources = ResourceContext::new(policy);
            let hierarchy =
                validate::validate(&model, &resources, AsciiExecution::for_test(&policy)).unwrap();
            let result = prepare_annotations(&model, &hierarchy, &resources);
            if limit == exact {
                let annotations =
                    result.expect("the exact annotation assembly allowance should fit");
                assert!(annotations[0].is_none());
                assert_eq!(annotations[1].as_deref(), Some(expected.as_str()));
                assert_eq!(annotations[2].as_deref(), Some(expected.as_str()));
            } else {
                assert!(
                    matches!(result, Err(AsciiError::ResourceLimitExceeded(error))
                    if error.limit == AsciiResourceLimitId::MaxOutputBytes
                        && error.actual == exact && error.max == limit)
                );
            }
        }
    }
}
