use super::{FlowchartMeasurement, FlowchartRenderModelRef, direction, direction_name, index};
use crate::Result;
use crate::flowchart::FlowchartLayoutSettings;
use crate::graph_layout::ir::{
    Direction, Endpoint, MeasuredGraph, MeasuredGroup, Size, SolvedGroup, TitlePosition,
};
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{LayoutCluster, LayoutLabel};
use kurbo::Insets;
use merman_model::presentation::PresentationConfig;
use std::collections::{HashMap, HashSet};

pub(super) struct GroupBindings<'a> {
    pub(super) declarations: Vec<usize>,
    pub(super) collapsed: Vec<usize>,
    pub(super) parents: HashMap<&'a str, u32>,
    pub(super) directions: Vec<Direction>,
}

fn title_position(raw: &str) -> Option<TitlePosition> {
    Some(match raw {
        "top-left" => TitlePosition::TopLeft,
        "top-center" => TitlePosition::TopCenter,
        "top-right" => TitlePosition::TopRight,
        "bottom-left" => TitlePosition::BottomLeft,
        "bottom-center" => TitlePosition::BottomCenter,
        "bottom-right" => TitlePosition::BottomRight,
        _ => return None,
    })
}

pub(super) fn measure<'a>(
    view: FlowchartRenderModelRef<'a>,
    measurement: &FlowchartMeasurement<'_>,
    config: &PresentationConfig,
    graph: &mut MeasuredGraph,
    endpoints: &mut HashMap<&'a str, Endpoint>,
    work: &mut OperationLayoutWorkControl,
) -> Result<GroupBindings<'a>> {
    work.charge_adapter(view.subgraphs.len())?;
    let mut seen = HashSet::with_capacity(view.subgraphs.len());
    let mut declarations = Vec::with_capacity(view.subgraphs.len());
    let mut collapsed = Vec::new();
    let mut parents = HashMap::new();
    let settings = &measurement.settings;
    let default_position = config
        .get_str("flowchart.subGraphTitlePosition")
        .and_then(title_position)
        .unwrap_or_default();
    for (ordinal, group) in view.semantic.subgraphs.iter().enumerate() {
        work.charge_adapter(group.id.len())?;
        if view.collapsed_replacement(&group.id).is_some() || !seen.insert(group.id.as_str()) {
            continue;
        }
        if view.is_subgraph_collapsed(&group.id) {
            collapsed.push(ordinal);
            continue;
        }
        let id = index(graph.groups.len(), work)?;
        endpoints.insert(&group.id, Endpoint::Group(id));
        declarations.push(ordinal);
        let metrics =
            measurement.subgraph_title(group, ordinal, |units| work.charge_adapter(units))?;
        let title = (!super::super::flowchart_label_is_empty_for_render(
            view.subgraph_title_for_render(ordinal, group),
        ))
        .then_some(Size {
            width: metrics.width,
            height: metrics.height,
        });
        let position = group
            .metadata
            .as_ref()
            .and_then(|value| value.get("titlePosition"))
            .and_then(|value| value.as_str())
            .and_then(title_position)
            .unwrap_or(default_position);
        let padding = settings.cluster_padding;
        let title_band = padding
            + title.map_or(0.0, |size| {
                size.height
                    + settings.title_margin_top.max(0.0)
                    + settings.title_margin_bottom.max(0.0)
                    + padding
            });
        let at_bottom = matches!(
            position,
            TitlePosition::BottomLeft | TitlePosition::BottomCenter | TitlePosition::BottomRight
        );
        let (top, bottom) = if at_bottom {
            (padding, title_band)
        } else {
            (title_band, padding)
        };
        graph.groups.push(MeasuredGroup {
            seq: index(work.checked_add(view.nodes.len(), ordinal)?, work)?,
            parent: None,
            direction: group.dir.as_deref().map(direction).transpose()?,
            padding: Insets::new(padding, top, padding, bottom),
            min_size: Size {
                width: padding * 2.0 + title.map_or(0.0, |size| size.width),
                height: top + bottom,
            },
            title,
            title_position: position,
        });
    }
    for (member, parent) in view.visible_memberships() {
        work.charge_adapter(1)?;
        if let Some(Endpoint::Group(id)) = endpoints.get(parent) {
            parents.insert(member, *id);
        }
    }
    for (&ordinal, group) in declarations.iter().zip(&mut graph.groups) {
        group.parent = parents.get(view.subgraphs[ordinal].id.as_str()).copied();
    }
    let directions = shape_directions(graph, work)?;
    Ok(GroupBindings {
        declarations,
        collapsed,
        parents,
        directions,
    })
}

fn shape_directions(
    graph: &MeasuredGraph,
    work: &mut OperationLayoutWorkControl,
) -> Result<Vec<Direction>> {
    work.charge_adapter(graph.groups.len())?;
    let mut resolved = Vec::from_iter(graph.groups.iter().map(|group| group.direction));
    let mut visited = vec![false; resolved.len()];
    let mut path = Vec::new();
    for start in 0..resolved.len() {
        let mut next = Some(start);
        let direction = loop {
            let Some(group) = next else {
                break graph.direction;
            };
            if let Some(dir) = resolved[group] {
                break dir;
            }
            if visited[group] {
                break graph.direction;
            }
            work.charge_adapter(1)?;
            visited[group] = true;
            path.push(group);
            next = graph.groups[group].parent.map(|parent| parent as usize);
        };
        for group in path.drain(..) {
            resolved[group] = Some(direction);
        }
    }
    Ok(resolved
        .into_iter()
        .map(|dir| dir.unwrap_or(graph.direction))
        .collect())
}

pub(super) fn project(
    declarations: &[usize],
    solved: Vec<SolvedGroup>,
    model: FlowchartRenderModelRef<'_>,
    settings: &FlowchartLayoutSettings,
) -> Vec<LayoutCluster> {
    declarations
        .iter()
        .zip(solved)
        .map(|(&ordinal, geometry)| {
            let group = &model.subgraphs[ordinal];
            let center = geometry.rect.center();
            LayoutCluster {
                id: group.id.clone(),
                x: center.x,
                y: center.y,
                width: geometry.rect.width,
                height: geometry.rect.height,
                diff: 0.0,
                offset_y: 0.0,
                title: model.subgraph_title_for_render(ordinal, group).to_owned(),
                title_label: geometry.title.unwrap_or(LayoutLabel {
                    x: center.x,
                    y: center.y,
                    width: 0.0,
                    height: 0.0,
                }),
                requested_dir: group.dir.clone(),
                effective_dir: direction_name(geometry.direction).into(),
                padding: settings.cluster_padding,
                title_margin_top: settings.title_margin_top,
                title_margin_bottom: settings.title_margin_bottom,
            }
        })
        .collect()
}
