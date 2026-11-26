mod anchors;

use super::ir::{Axis, Direction, MeasuredGraph, Point, Size, SolvedGraph};
use super::shoal_bridge::{self, NodeHint, OrderHint, Placement, WidthPolicy};
use crate::layout_frame::{FrameDirection, GraphPlacement, ShoalFrameNode};
use crate::layout_work::OperationLayoutWorkControl;
use crate::{
    DiagramFamilyId, Error, LayoutAdaptation, LayoutAdaptationObservation, LayoutExecution, Result,
    ShoalLayoutFrame,
};
use merman_layout_shoal::LayoutScratch;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(crate) struct Constraints<'a> {
    pub(crate) flow_relative_nodes: &'a [(usize, Axis)],
    pub(crate) split_incoming_ports: &'a [usize],
    pub(crate) lane_order: &'a [u32],
    pub(crate) capture_ranks: bool,
    pub(crate) allow_turn: bool,
}

/// IDs are supplied in measured node and group order so persisted hints survive reindexing.
pub(crate) fn solve<'id>(
    graph: &MeasuredGraph,
    node_ids: impl ExactSizeIterator<Item = &'id str> + Clone,
    group_ids: impl ExactSizeIterator<Item = &'id str> + Clone,
    execution: Option<&LayoutExecution<'_>>,
    constraints: Constraints<'_>,
    work: &mut OperationLayoutWorkControl,
) -> Result<(SolvedGraph, GraphPlacement)> {
    let projected = shoal_bridge::Constraints {
        flow_relative_nodes: constraints.flow_relative_nodes,
        lane_order: constraints.lane_order,
        capture_ranks: constraints.capture_ranks,
        ..Default::default()
    };
    let split = constraints.split_incoming_ports;
    let Some(execution) = execution else {
        let solved = shoal_bridge::solve(
            graph,
            Placement {
                constraints: projected,
                ..Placement::default()
            },
            split,
            &mut LayoutScratch::with_max_retained_bytes(0),
            work,
        )?;
        return Ok((solved, GraphPlacement::default()));
    };
    let fit_width = execution.adaptation == LayoutAdaptation::FitWidth;
    let width = if fit_width {
        WidthPolicy::FitWidth {
            max_width: execution.container_width,
            allow_turn: constraints.allow_turn,
            turn_breakpoint: 640.0,
            fold_wide_ranks: execution.fold_wide_ranks,
        }
    } else {
        WidthPolicy::Natural
    };
    let family = execution.family_id().as_str();
    let previous = execution
        .previous_shoal_frame
        .as_deref()
        .filter(|frame| frame.family == family && !frame.nodes.is_empty());
    let capture = execution.capture_shoal_frame;
    if (capture || previous.is_some())
        && (node_ids.len() != graph.nodes.len() || group_ids.len() != graph.groups.len())
    {
        return Err(Error::InvalidModel {
            message: "Shoal frame identities do not match the measured graph".into(),
        });
    }
    let anchors = if graph.groups.is_empty()
        && (capture
            || previous.is_some_and(|frame| frame.nodes.values().any(|n| n.position.is_some())))
    {
        Some(anchors::Components::new(graph, node_ids.clone(), work)?)
    } else {
        None
    };
    let hints = if let Some(previous) = previous {
        hints(
            graph,
            node_ids.clone(),
            group_ids.clone(),
            previous,
            anchors.as_ref(),
            work,
        )?
    } else {
        Vec::new()
    };
    let reuse_components = !fit_width && anchors.is_some();
    let placement = Placement {
        width,
        spacing: spacing_for(graph, execution),
        capture_ranks: capture,
        reuse_components,
        hints: &hints,
        incident_signatures: anchors
            .as_ref()
            .map_or(&[], anchors::Components::signatures),
        constraints: projected,
    };
    if !capture
        && execution.previous_shoal_frame.is_none()
        && placement.constraints.lane_order.is_empty()
        && let Some(pool) = execution.family.session().shoal_scratch()
    {
        let solved =
            pool.solve(|scratch| shoal_bridge::solve(graph, placement, split, scratch, work))?;
        let placement = GraphPlacement {
            adaptation: adaptation(execution, &solved),
            frame: None,
        };
        return Ok((solved, placement));
    }
    let mut scratch = if reuse_components {
        execution.shoal_preview_scratch(previous)
    } else {
        LayoutScratch::with_max_retained_bytes(0)
    };
    let solved = shoal_bridge::solve(graph, placement, split, &mut scratch, work)?;
    let placement = GraphPlacement {
        adaptation: adaptation(execution, &solved),
        frame: if capture {
            let mut frame = frame(graph, &solved, node_ids, group_ids, family, anchors, work)?;
            if scratch.retained_bytes() != 0 {
                frame.scratch = Some(Arc::new(Mutex::new(Some(scratch))));
            }
            Some(Arc::new(frame))
        } else {
            None
        },
    };
    Ok((solved, placement))
}

fn adaptation(
    execution: &LayoutExecution<'_>,
    solved: &SolvedGraph,
) -> Option<LayoutAdaptationObservation> {
    (execution.adaptation == LayoutAdaptation::FitWidth).then(|| {
        LayoutAdaptationObservation::fit_width(execution.container_width, solved.bounds.width)
    })
}

fn spacing_for(graph: &MeasuredGraph, execution: &LayoutExecution<'_>) -> Option<(f64, f64, f64)> {
    let config = execution.presentation_config?;
    let (sections, node_gap, rank_gap, component_is_node_gap): (&[&str], _, _, _) =
        match execution.family_id() {
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::AGENTFLOW => {
                (&["flowchart"], "nodeSpacing", "rankSpacing", true)
            }
            DiagramFamilyId::CLASS | DiagramFamilyId::STATE | DiagramFamilyId::REQUIREMENT => (
                &["", "state", "flowchart"],
                "nodeSpacing",
                "rankSpacing",
                false,
            ),
            DiagramFamilyId::ER => (&["er"], "nodeSpacing", "rankSpacing", false),
            DiagramFamilyId::USECASE => (&["", "usecase"], "nodeSpacing", "rankSpacing", false),
            _ => return None,
        };
    let spacing = merman_layout_shoal::Spacing::default();
    let scale = execution.native_family_theme().map_or(1.0, |theme| {
        use merman_theme_contract::v2::Density;
        match theme.form.density {
            Density::Compact => 0.85,
            Density::Balanced => 1.0,
            Density::Airy => 1.2,
        }
    });
    let owns = |key| {
        sections.iter().any(|section| match *section {
            "" => config.explicit_config_owns_path(key),
            section => config.explicit_config_owns_path(&format!("{section}.{key}")),
        })
    };
    let node = if owns(node_gap) {
        graph.node_spacing
    } else {
        spacing.node * scale
    };
    let rank = if owns(rank_gap) {
        graph.rank_spacing
    } else {
        spacing.rank * scale
    };
    Some((
        node,
        rank,
        if component_is_node_gap {
            node
        } else {
            node.max(rank)
        },
    ))
}

#[cfg(any(feature = "diagram-architecture", feature = "diagram-c4"))]
pub(crate) fn placement_for_solved<'id>(
    graph: &MeasuredGraph,
    solved: &SolvedGraph,
    node_ids: impl ExactSizeIterator<Item = &'id str>,
    group_ids: impl ExactSizeIterator<Item = &'id str>,
    execution: &LayoutExecution<'_>,
    work: &mut OperationLayoutWorkControl,
) -> Result<GraphPlacement> {
    let frame = if execution.capture_shoal_frame {
        Some(Arc::new(frame(
            graph,
            solved,
            node_ids,
            group_ids,
            execution.family_id().as_str(),
            None,
            work,
        )?))
    } else {
        None
    };
    Ok(GraphPlacement {
        adaptation: adaptation(execution, solved),
        frame,
    })
}

fn hints<'id>(
    graph: &MeasuredGraph,
    nodes: impl ExactSizeIterator<Item = &'id str> + Clone,
    groups: impl ExactSizeIterator<Item = &'id str>,
    previous: &ShoalLayoutFrame,
    anchors: Option<&anchors::Components>,
    work: &mut OperationLayoutWorkControl,
) -> Result<Vec<Option<NodeHint>>> {
    let comparisons = previous.nodes.len().ilog2() as usize + 1;
    work.charge_adapter(work.checked_mul(nodes.len(), comparisons)?)?;
    work.charge_adapter(previous.groups.len())?;
    let mut previous_groups = HashMap::with_capacity(previous.groups.len());
    for (index, id) in previous.groups.iter().enumerate() {
        work.charge_adapter(id.len())?;
        previous_groups.insert(id.as_str(), index);
    }
    let mut parents = Vec::with_capacity(groups.len());
    for id in groups {
        work.charge_adapter(id.len())?;
        parents.push(previous_groups.get(id).copied());
    }
    let matching = anchors
        .map(|anchors| anchors.matches(nodes.clone(), previous, work))
        .transpose()?;
    let mut hints = Vec::with_capacity(nodes.len());
    for (index, (id, measured)) in nodes.zip(&graph.nodes).enumerate() {
        work.charge_adapter(id.len())?;
        let hint = previous.nodes.get(id).and_then(|node| {
            let same_parent = match (measured.parent, node.parent) {
                (None, None) => true,
                (Some(group), Some(previous_group)) => {
                    parents[group as usize] == Some(previous_group as usize)
                }
                _ => false,
            };
            same_parent.then(|| NodeHint {
                order: OrderHint {
                    rank: i32::try_from(node.rank).unwrap_or(i32::MAX),
                    cross_key: node.cross_key,
                },
                direction: Direction::from(node.direction),
                position: anchors
                    .zip(node.position)
                    .filter(|_| matching.as_ref().is_some_and(|matching| matching[index]))
                    .map(|(anchors, position)| {
                        (
                            merman_layout_shoal::PositionHint {
                                center: Point {
                                    x: position.x,
                                    y: position.y,
                                },
                                size: Size {
                                    width: position.width,
                                    height: position.height,
                                },
                                incident_signature: Some(position.incident_signature.bits()),
                            },
                            anchors.signature(index),
                        )
                    }),
            })
        });
        hints.push(hint);
    }
    Ok(hints)
}

fn frame<'id>(
    graph: &MeasuredGraph,
    solved: &SolvedGraph,
    node_ids: impl ExactSizeIterator<Item = &'id str>,
    group_ids: impl ExactSizeIterator<Item = &'id str>,
    family: &str,
    anchors: Option<anchors::Components>,
    work: &mut OperationLayoutWorkControl,
) -> Result<ShoalLayoutFrame> {
    if solved.node_ranks.len() != node_ids.len() {
        return Err(Error::InvalidModel {
            message: "Shoal omitted the requested node ranks".into(),
        });
    }
    let comparisons = node_ids.len().checked_ilog2().unwrap_or(0) as usize + 1;
    work.charge_adapter(work.checked_mul(node_ids.len(), comparisons)?)?;
    let mut groups = Vec::with_capacity(group_ids.len());
    for id in group_ids {
        work.charge_adapter(id.len())?;
        groups.push(id.to_owned());
    }
    let mut nodes = BTreeMap::new();
    let positions = anchors
        .map(|anchors| anchors.positions(&solved.nodes, work))
        .transpose()?;
    for (index, ((id, measured), (&rank, rect))) in node_ids
        .zip(&graph.nodes)
        .zip(solved.node_ranks.iter().zip(&solved.nodes))
        .enumerate()
    {
        work.charge_adapter(id.len())?;
        let direction = measured.parent.map_or(solved.direction, |index| {
            solved.groups[index as usize].direction
        });
        let center = rect.center();
        let cross_key = match direction {
            Direction::Down | Direction::Up => center.x,
            Direction::Right | Direction::Left => center.y,
        };
        nodes.insert(
            id.to_owned(),
            ShoalFrameNode {
                parent: measured.parent,
                rank,
                cross_key,
                direction: direction.into(),
                position: positions.as_ref().map(|positions| positions[index]),
            },
        );
    }
    Ok(ShoalLayoutFrame {
        family: family.to_owned(),
        groups,
        nodes,
        scratch: None,
    })
}

impl From<FrameDirection> for Direction {
    fn from(direction: FrameDirection) -> Self {
        match direction {
            FrameDirection::Down => Self::Down,
            FrameDirection::Up => Self::Up,
            FrameDirection::Left => Self::Left,
            FrameDirection::Right => Self::Right,
        }
    }
}

impl From<Direction> for FrameDirection {
    fn from(direction: Direction) -> Self {
        match direction {
            Direction::Down => Self::Down,
            Direction::Up => Self::Up,
            Direction::Left => Self::Left,
            Direction::Right => Self::Right,
        }
    }
}

#[cfg(all(test, feature = "diagram-flowchart"))]
#[test]
fn flat_preview_reuses_scratch_only_from_its_in_memory_frame() {
    use crate::environment::RenderEnvironment;
    use crate::{LayoutOptions, RenderResourcePolicy, ResourceLimitId};
    use merman_core::{MermaidEngine, ParseOptions};

    let parse = |source| {
        MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation()
    };
    let before = parse(
        "---\nconfig:\n  layout: shoal\n---\nflowchart TB\nA --> B --> C --> D --> E --> F\nX --> Y",
    );
    let after = parse(
        "---\nconfig:\n  layout: shoal\n---\nflowchart TB\nA --> B --> C --> D --> E --> F\nX[Edited] --> Y",
    );
    let render = |parsed, policy, previous| {
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session()
            .unwrap();
        let options = LayoutOptions::default().with_shoal_layout_frame(previous);
        let svg = crate::family::prepare(parsed, &options, session)
            .unwrap()
            .render_svg(&Default::default(), &Default::default())
            .unwrap();
        let frame = svg.shoal_layout_frame().unwrap();
        let (svg, report) = svg.into_completion().into_output_and_report();
        (svg, frame, report.session_report().layout_work_units())
    };

    let interactive = RenderResourcePolicy::interactive();
    let (_, first, _) = render(&before, interactive, None);
    let (_, independent, _) = render(&before, interactive, None);
    assert!(!Arc::ptr_eq(
        first.scratch.as_ref().unwrap(),
        independent.scratch.as_ref().unwrap(),
    ));
    let wire = serde_json::to_value(&*first).unwrap();
    assert!(!wire.as_object().unwrap().contains_key("scratch"));
    let decoded = Arc::new(serde_json::from_value::<ShoalLayoutFrame>(wire).unwrap());
    assert_eq!(first, decoded);
    assert!(decoded.scratch.is_none());

    let constrained = RenderResourcePolicy::constrained();
    let mut restricted = interactive;
    let limit = ResourceLimitId::MaxLayoutWorkUnits;
    restricted
        .apply_limit(limit, constrained.value(limit).unwrap())
        .unwrap();
    for policy in [constrained, restricted] {
        let (_, next, _) = render(&after, policy, Some(first.clone()));
        assert!(next.scratch.is_none());
    }

    let (cold, cold_frame, cold_work) = render(&after, interactive, Some(decoded));
    let (warm, warm_frame, warm_work) = render(&after, interactive, Some(first.clone()));
    assert_eq!(warm, cold);
    assert_eq!(warm_frame, cold_frame);
    assert!(warm_work < cold_work, "{warm_work} >= {cold_work}");
    assert!(first.take_scratch().is_none());
}
