use super::{
    MeasuredLabels, MeasurementTarget, RelationTerminals, measure_into, positive_label_size,
};
use crate::class::{
    ClassDiagramModel, ClassLayoutSettings, ClassRenderConfig, ClassTextThemePlan, normalize_dir,
};
use crate::graph_layout::contour::ShapePortProfile;
use crate::graph_layout::ir::{
    Direction, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredGroup, MeasuredNode, RoutingStyle,
    Side, Size, TitlePosition,
};
use crate::layout_work::OperationLayoutWorkControl;
use crate::math::MathRenderer;
use crate::text::TextMeasurer;
use crate::{Error, Result};
use indexmap::IndexSet;
use kurbo::Insets;
use merman_model::presentation::PresentationConfig;
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use std::borrow::Cow;
use std::collections::hash_map::Entry;

/// Class identities and paint profiles parallel the dense arrays in the measured IR.
pub(in crate::class) struct ShoalMeasurement<'a> {
    pub(in crate::class) graph: MeasuredGraph,
    pub(in crate::class) direction_name: String,
    pub(in crate::class) node_ids: Vec<&'a str>,
    pub(in crate::class) group_ids: Vec<&'a str>,
    pub(in crate::class) edge_ids: Vec<Cow<'a, str>>,
    pub(in crate::class) profiles: Vec<ShapePortProfile>,
}

impl ShoalMeasurement<'_> {
    pub(in crate::class) fn endpoint_id(&self, endpoint: Endpoint) -> &str {
        match endpoint {
            Endpoint::Node(index) => self.node_ids[index as usize],
            Endpoint::Group(index) => self.group_ids[index as usize],
        }
    }
}

pub(in crate::class) fn measure_for_shoal<'a>(
    model: &'a ClassDiagramModel,
    config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    typography: &ClassTextThemePlan,
    render: &ClassRenderConfig,
    work: &mut OperationLayoutWorkControl,
) -> Result<(ShoalMeasurement<'a>, MeasuredLabels)> {
    let direction_name = normalize_dir(&model.model().direction);
    let direction = match direction_name.as_str() {
        "LR" => Direction::Right,
        "RL" => Direction::Left,
        "BT" => Direction::Up,
        _ => Direction::Down,
    };
    let settings = render.layout_settings();
    let mut target = ShoalInput {
        graph: MeasuredGraph {
            direction,
            routing: RoutingStyle::Curved,
            node_spacing: settings.nodesep,
            rank_spacing: settings.ranksep,
            component_spacing: settings.nodesep.max(settings.ranksep),
            inherit_crossed_group_direction: true,
            nodes: Vec::new(),
            groups: Vec::new(),
            edges: Vec::new(),
        },
        node_ids: Vec::new(),
        group_ids: Vec::new(),
        profiles: Vec::new(),
        endpoints: FxHashMap::default(),
        namespaces: model
            .namespaces()
            .map(|(_, ns)| ns.namespace.id.as_str())
            .collect(),
        invalid_parents: FxHashMap::default(),
        edges: IndexSet::default(),
        settings,
        neo: render.look == "neo",
        work,
    };
    let labels = measure_into(
        model,
        config,
        measurer,
        math_renderer,
        render,
        false,
        typography,
        Some(work),
        &mut target,
    )?;
    let ShoalInput {
        graph,
        node_ids,
        group_ids,
        profiles,
        endpoints,
        invalid_parents,
        edges,
        ..
    } = target;
    work.charge_adapter(work.checked_mul(endpoints.len(), 2)?)?;
    work.charge_adapter(work.checked_add(graph.edges.len(), model.relations().len())?)?;
    if let Some(parent) = invalid_parents.values().next() {
        return Err(Error::InvalidModel {
            message: format!("Class parent `{parent}` is not a namespace"),
        });
    }
    Ok((
        ShoalMeasurement {
            graph,
            direction_name,
            node_ids,
            group_ids,
            edge_ids: edges.into_iter().map(|(_, _, id)| id).collect(),
            profiles,
        },
        labels,
    ))
}

type NamedEdge<'a> = (&'a str, &'a str, Cow<'a, str>);

struct ShoalInput<'a, 'op> {
    graph: MeasuredGraph,
    node_ids: Vec<&'a str>,
    group_ids: Vec<&'a str>,
    profiles: Vec<ShapePortProfile>,
    endpoints: FxHashMap<&'a str, Endpoint>,
    namespaces: FxHashSet<&'a str>,
    invalid_parents: FxHashMap<&'a str, &'a str>,
    edges: IndexSet<NamedEdge<'a>, FxBuildHasher>,
    settings: &'op ClassLayoutSettings,
    neo: bool,
    work: &'op OperationLayoutWorkControl,
}

impl<'a> ShoalInput<'a, '_> {
    fn endpoint(&mut self, id: &'a str, size: Option<(f64, f64)>) -> Result<Endpoint> {
        let next_seq = self.endpoints.len();
        let endpoint = match self.endpoints.entry(id) {
            Entry::Occupied(entry) => {
                let endpoint = *entry.get();
                if let (Endpoint::Node(index), Some(size)) = (endpoint, size) {
                    let index = index as usize;
                    let measured = &mut self.graph.nodes[index];
                    let (node, profile) =
                        leaf_geometry(size, self.graph.direction, measured.seq, measured.parent);
                    *measured = node;
                    self.profiles[index] = profile;
                }
                endpoint
            }
            Entry::Vacant(entry) => {
                let seq = dense_index(next_seq, self.work)?;
                let endpoint = if self.namespaces.contains(id) {
                    let index = dense_index(self.graph.groups.len(), self.work)?;
                    self.graph
                        .groups
                        .push(namespace_geometry(self.settings, seq, None, None));
                    self.group_ids.push(id);
                    Endpoint::Group(index)
                } else {
                    let index = dense_index(self.graph.nodes.len(), self.work)?;
                    let (node, profile) =
                        leaf_geometry(size.unwrap_or((1.0, 1.0)), self.graph.direction, seq, None);
                    self.graph.nodes.push(node);
                    self.profiles.push(profile);
                    self.node_ids.push(id);
                    Endpoint::Node(index)
                };
                entry.insert(endpoint);
                endpoint
            }
        };
        Ok(endpoint)
    }

    fn set_parent(&mut self, id: &'a str, child: Endpoint, parent: Option<&'a str>) -> Result<()> {
        let Some(parent) = parent else { return Ok(()) };
        // A forward parent reference reserves its sequence slot at first sight. The namespace
        // inventory lets us reserve it directly in the correct dense array before measuring it.
        let Endpoint::Group(group) = self.endpoint(parent, None)? else {
            // A typed model's namespace key can differ from its payload ID. As in Graphlib, a
            // later assignment for the same child replaces the parent before it is validated.
            self.invalid_parents.insert(id, parent);
            return Ok(());
        };
        self.invalid_parents.remove(id);
        match child {
            Endpoint::Node(index) => self.graph.nodes[index as usize].parent = Some(group),
            Endpoint::Group(index) => self.graph.groups[index as usize].parent = Some(group),
        }
        Ok(())
    }
}

impl<'a> MeasurementTarget<'a> for ShoalInput<'a, '_> {
    fn namespace(&mut self, id: &'a str, parent: Option<&'a str>, title: (f64, f64)) -> Result<()> {
        let endpoint = self.endpoint(id, None)?;
        let Endpoint::Group(index) = endpoint else {
            unreachable!("namespace identities are known before inserting endpoints");
        };
        let group = &mut self.graph.groups[index as usize];
        *group = namespace_geometry(self.settings, group.seq, group.parent, label_size(title));
        self.set_parent(id, endpoint, parent)
    }

    fn node(&mut self, id: &'a str, parent: Option<&'a str>, size: (f64, f64)) -> Result<()> {
        let endpoint = self.endpoint(id, Some(size))?;
        self.set_parent(id, endpoint, parent)
    }

    fn edge(
        &mut self,
        from: &'a str,
        to: &'a str,
        id: Cow<'a, str>,
        size: (f64, f64),
        terminals: Option<RelationTerminals<'a>>,
    ) -> Result<()> {
        let source = self.endpoint(from, None)?;
        let target = self.endpoint(to, None)?;
        let (start_marker, end_marker) =
            terminals
                .as_ref()
                .map_or_else(Default::default, |terminals| {
                    let shape = &terminals.relation.relation;
                    (
                        crate::svg::class_canonical_marker(shape.type1, true, self.neo),
                        crate::svg::class_canonical_marker(shape.type2, false, self.neo),
                    )
                });
        let edge = MeasuredEdge {
            source,
            target,
            min_rank_span: 1,
            weight: 1.0,
            label: label_size(size),
            start_label: terminals
                .as_ref()
                .and_then(|terminals| terminals.start_right)
                .map(size_from_pair),
            end_label: terminals
                .and_then(|terminals| terminals.end_left)
                .map(size_from_pair),
            start_marker,
            end_marker,
        };
        // A named multiedge keeps its first slot when its full (from, to, name) key is replaced.
        let (index, inserted) = self.edges.insert_full((from, to, id));
        if inserted {
            self.graph.edges.push(edge);
        } else {
            self.graph.edges[index] = edge;
        }
        Ok(())
    }
}

fn dense_index(index: usize, work: &OperationLayoutWorkControl) -> Result<u32> {
    u32::try_from(index).map_err(|_| work.record_arithmetic_overflow().into())
}

fn leaf_geometry(
    (width, height): (f64, f64),
    direction: Direction,
    seq: u32,
    parent: Option<u32>,
) -> (MeasuredNode, ShapePortProfile) {
    let size = Size {
        width: width.max(1.0),
        height: height.max(1.0),
    };
    let profile = ShapePortProfile::rectangle(size.width, size.height);
    let (face, perpendicular) = match direction {
        Direction::Down | Direction::Up => (Side::Top, Side::Left),
        Direction::Left | Direction::Right => (Side::Left, Side::Top),
    };
    (
        MeasuredNode {
            size,
            seq,
            parent,
            port_span: Some(profile.port_span(face)),
            perpendicular_port_span: Some(profile.port_span(perpendicular)),
            anchor_y: None,
        },
        profile,
    )
}

fn namespace_geometry(
    settings: &ClassLayoutSettings,
    seq: u32,
    parent: Option<u32>,
    title: Option<Size>,
) -> MeasuredGroup {
    let pad = settings.namespace_padding.max(0.0);
    let title_band = title.map_or(pad, |size| {
        2.0 * pad
            + size.height
            + settings.title_margin_top.max(0.0)
            + settings.title_margin_bottom.max(0.0)
    });
    MeasuredGroup {
        seq,
        parent,
        direction: None,
        padding: Insets::new(pad, title_band, pad, pad),
        min_size: Size {
            width: 2.0 * pad + title.map_or(0.0, |size| size.width),
            height: title_band + pad,
        },
        title,
        title_position: TitlePosition::TopCenter,
    }
}

fn label_size(metrics: (f64, f64)) -> Option<Size> {
    positive_label_size(metrics).map(size_from_pair)
}

fn size_from_pair((width, height): (f64, f64)) -> Size {
    Size { width, height }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::DeterministicTextMeasurer;
    use crate::{RenderResourcePolicy, resources::OperationWorkMeter};
    use merman_core::{MermaidEngine, ParseOptions};
    use merman_model::presentation::RenderSemanticModel;
    use std::sync::Arc;

    #[test]
    fn forward_namespace_and_named_multiedge_replacement_keep_their_slots() {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                r#"classDiagram
direction LR
namespace Parent {
  namespace Child {
    class A
  }
}
namespace Sibling {
  class B
}
note for A "notice"
A "1" o--|> "many" B : first
A -- B : other
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let RenderSemanticModel::Class(model) = parsed.model() else {
            panic!("expected Class semantics");
        };
        let config = &parsed.metadata().effective_config;
        let mut model = model.clone().into_model();
        let parent = model.namespaces.shift_remove("Parent").unwrap();
        model.namespaces.insert("Parent".into(), parent);
        let [first, other] = model.relations.as_slice() else {
            panic!("expected two named relations");
        };
        let mut replacement = first.clone();
        replacement.title = "replacement title".into();
        replacement.relation.type1 = 2;
        let expected_edges = ["edgeNote0".to_owned(), first.id.clone(), other.id.clone()];
        model.relations.push(replacement);
        let model = ClassDiagramModel::new(model, config.as_config());
        let typography = ClassTextThemePlan::resolve(None, config);
        let render = ClassRenderConfig::resolve(config, &typography);
        let mut work = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::default(),
        )));

        let (measured, _) = measure_for_shoal(
            &model,
            config,
            &DeterministicTextMeasurer::default(),
            None,
            &typography,
            &render,
            &mut work,
        )
        .unwrap();
        assert_eq!(measured.group_ids, ["Parent.Child", "Parent", "Sibling"]);
        assert_eq!(measured.graph.groups[0].parent, Some(1));
        assert_eq!(measured.node_ids, ["A", "B", "note0"]);
        assert_eq!(measured.graph.nodes[0].parent, Some(0));
        assert_eq!(measured.graph.nodes[1].parent, Some(2));
        assert_eq!(measured.graph.nodes[0].seq, 3);
        assert_eq!(measured.edge_ids, expected_edges);
        let [attachment, relation, parallel] = measured.graph.edges.as_slice() else {
            panic!("expected a note attachment and two relations");
        };
        assert_eq!(attachment.source, Endpoint::Node(2));
        assert_eq!(attachment.target, Endpoint::Node(0));
        assert_eq!(relation.source, Endpoint::Node(0));
        assert_eq!(relation.target, Endpoint::Node(1));
        assert!(relation.label.unwrap().width > parallel.label.unwrap().width);
        assert!(relation.start_label.is_some() && relation.end_label.is_some());
        assert_eq!(
            relation.start_marker.width,
            crate::svg::class_canonical_marker(2, true, render.look == "neo").width
        );
    }
}
