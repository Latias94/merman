//! State-specific layout-adapter admission and semantic input rules.

#[cfg(feature = "layout-shoal")]
use super::StateNode;
use merman_model::families::state::StateDiagramRenderModel as StateDiagramModel;
use merman_render_core::layout_work::OperationLayoutWorkControl;
use merman_render_core::{Error, Result};
use std::collections::HashMap;

/// Calculates the common admission reservation before the facade dispatches to a State adapter.
pub fn state_layout_adapter_work(
    model: &StateDiagramModel,
    work_control: &OperationLayoutWorkControl,
) -> Result<usize> {
    // This node tranche already reserves the linear parent-index and traversal work below.
    // Keep semantic cycle validation inside this reservation so its W/W-1 boundary stays stable.
    let node_work = work_control.checked_mul(model.nodes.len(), 12)?;
    let edge_work = work_control.checked_mul(model.edges.len(), 8)?;
    let state_work = work_control.checked_mul(model.states.len(), 4)?;
    let relation_work = work_control.checked_mul(model.relations.len(), 2)?;
    let link_work = work_control.checked_mul(model.links.len(), 2)?;
    let style_work = work_control.checked_mul(model.style_classes.len(), 2)?;
    let hidden_prefix_bytes = model.states.iter().try_fold(0usize, |work, (id, state)| {
        if state
            .note
            .as_ref()
            .is_some_and(|note| !note.text.trim().is_empty() && note.position.is_none())
        {
            work_control.checked_add(work, id.len())
        } else {
            Ok(work)
        }
    })?;
    let hidden_candidate_bytes = model.nodes.iter().try_fold(0usize, |work, node| {
        work_control.checked_add(
            work,
            node.id
                .len()
                .checked_add(node.parent_id.as_deref().map_or(0, str::len))
                .ok_or_else(|| work_control.record_arithmetic_overflow())?,
        )
    })?;
    let hidden_edge_bytes = model.edges.iter().try_fold(0usize, |work, edge| {
        let edge_bytes = edge
            .id
            .len()
            .checked_add(edge.start.len())
            .and_then(|units| units.checked_add(edge.end.len()))
            .ok_or_else(|| work_control.record_arithmetic_overflow())?;
        work_control.checked_add(work, edge_bytes)
    })?;
    let hidden_filter_work = work_control.checked_mul(
        work_control.checked_add(
            hidden_prefix_bytes,
            work_control.checked_add(hidden_candidate_bytes, hidden_edge_bytes)?,
        )?,
        4,
    )?;
    work_control.checked_add(
        work_control.checked_add(node_work, edge_work)?,
        work_control.checked_add(
            work_control.checked_add(state_work, relation_work)?,
            work_control.checked_add(
                work_control.checked_add(link_work, style_work)?,
                hidden_filter_work,
            )?,
        )?,
    )
}

#[derive(Default)]
struct HiddenPrefixTrieNode {
    children: HashMap<char, usize>,
    terminal: bool,
}

#[derive(Default)]
pub struct HiddenPrefixMatcher {
    nodes: Vec<HiddenPrefixTrieNode>,
}

impl HiddenPrefixMatcher {
    fn from_prefixes(prefixes: impl IntoIterator<Item = String>) -> Self {
        let mut matcher = Self {
            nodes: vec![HiddenPrefixTrieNode::default()],
        };
        for prefix in prefixes {
            let mut node_idx = 0usize;
            for ch in prefix.chars() {
                let next = if let Some(&next) = matcher.nodes[node_idx].children.get(&ch) {
                    next
                } else {
                    let next = matcher.nodes.len();
                    matcher.nodes.push(HiddenPrefixTrieNode::default());
                    matcher.nodes[node_idx].children.insert(ch, next);
                    next
                };
                node_idx = next;
            }
            matcher.nodes[node_idx].terminal = true;
        }
        matcher
    }

    pub fn is_hidden(&self, id: &str) -> bool {
        let mut node_idx = 0usize;
        for (byte_idx, ch) in id.char_indices() {
            let Some(&next) = self.nodes[node_idx].children.get(&ch) else {
                return false;
            };
            node_idx = next;
            let rest = &id[byte_idx + ch.len_utf8()..];
            if self.nodes[node_idx].terminal && (rest.is_empty() || rest.starts_with("----")) {
                return true;
            }
        }
        self.nodes[node_idx].terminal
    }
}

pub fn state_hidden_prefixes(model: &StateDiagramModel) -> HiddenPrefixMatcher {
    let mut hidden_prefixes: Vec<String> = Vec::new();
    for (id, st) in &model.states {
        let Some(note) = st.note.as_ref() else {
            continue;
        };
        if note.text.trim().is_empty() {
            continue;
        }
        if note.position.is_none() {
            hidden_prefixes.push(id.clone());
        }
    }
    HiddenPrefixMatcher::from_prefixes(hidden_prefixes)
}

pub fn note_group_owner_id(id: &str) -> Option<&str> {
    let (owner, _) = id.rsplit_once("----parent")?;
    (!owner.is_empty()).then_some(owner)
}

#[cfg(feature = "layout-shoal")]
pub fn note_group_container<'a>(
    group: &'a StateNode,
    owner: &'a StateNode,
    is_note_group: impl FnOnce(&str) -> bool,
) -> Option<&'a str> {
    match owner.parent_id.as_deref() {
        Some(parent)
            if note_group_owner_id(parent) == Some(owner.id.as_str()) && is_note_group(parent) =>
        {
            group.parent_id.as_deref()
        }
        parent => parent,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StateParentVisitState {
    Unvisited,
    Visiting,
    Complete,
}

pub fn validate_state_parent_cycles(model: &StateDiagramModel) -> Result<()> {
    validate_state_parent_cycles_with_step(model, || {})
}

fn validate_state_parent_cycles_with_step(
    model: &StateDiagramModel,
    mut before_step: impl FnMut(),
) -> Result<()> {
    use StateParentVisitState::{Complete, Unvisited, Visiting};

    // The last node with a duplicate id owns its parent, matching the previous HashMap collect.
    let index_by_id = model
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut state = vec![Unvisited; model.nodes.len()];
    let mut path = Vec::new();

    // Every input row is checked once. A canonical node follows its parent only while Unvisited,
    // then becomes Complete with the rest of its path, so successful parent hops are also O(N).
    for node in &model.nodes {
        // `state_layout_adapter_work` reserves the corresponding node work before this function.
        before_step();
        let Some(&start) = index_by_id.get(node.id.as_str()) else {
            unreachable!("the State parent index contains every model node")
        };
        if state[start] == Complete {
            continue;
        }

        path.clear();
        let mut current = start;
        loop {
            match state[current] {
                Unvisited => {
                    state[current] = Visiting;
                    path.push(current);
                }
                Visiting => {
                    return Err(Error::InvalidModel {
                        message: format!(
                            "state parent cycle involving {}",
                            model.nodes[current].id
                        ),
                    });
                }
                Complete => break,
            }

            let Some(parent_id) = model.nodes[current].parent_id.as_deref() else {
                break;
            };
            // The test hook runs before following every user-controlled parent link.
            before_step();
            let Some(&parent) = index_by_id.get(parent_id) else {
                break;
            };
            current = parent;
        }

        while let Some(index) = path.pop() {
            state[index] = Complete;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::StateNode;
    use super::*;
    use merman_render_core::resources::{
        OperationWorkMeter, RenderResourcePolicy, ResourceLimitId,
    };
    use std::sync::Arc;

    fn state_parent_node(id: impl Into<String>, parent_id: Option<String>) -> StateNode {
        StateNode {
            id: id.into(),
            label_style: String::new(),
            label: None,
            description: None,
            dom_id: String::new(),
            is_group: false,
            node_type: None,
            parent_id,
            css_classes: String::new(),
            css_compiled_styles: Vec::new(),
            css_styles: Vec::new(),
            dir: None,
            explicit_dir: None,
            padding: None,
            rx: None,
            ry: None,
            shape: "rect".to_string(),
            position: None,
            color_index: None,
            wrapping_width: None,
            min_width: None,
        }
    }

    fn state_parent_chain(node_count: usize) -> StateDiagramModel {
        StateDiagramModel {
            nodes: (0..node_count)
                .map(|index| {
                    state_parent_node(
                        format!("node-{index}"),
                        (index + 1 < node_count).then(|| format!("node-{}", index + 1)),
                    )
                })
                .collect(),
            ..StateDiagramModel::default()
        }
    }

    #[test]
    fn state_parent_cycle_validation_visits_a_long_chain_linearly() {
        const NODE_COUNT: usize = 4_096;
        let model = state_parent_chain(NODE_COUNT);
        let mut traversal_steps = 0usize;

        validate_state_parent_cycles_with_step(&model, || traversal_steps += 1)
            .expect("acyclic State parent chain");

        assert_eq!(traversal_steps, NODE_COUNT + (NODE_COUNT - 1));
    }

    #[test]
    fn state_parent_cycle_validation_preserves_cycle_id_and_linear_steps() {
        let model = StateDiagramModel {
            nodes: vec![
                state_parent_node("entry", Some("cycle-a".to_string())),
                state_parent_node("cycle-a", Some("cycle-b".to_string())),
                state_parent_node("cycle-b", Some("cycle-a".to_string())),
            ],
            ..StateDiagramModel::default()
        };
        let mut traversal_steps = 0usize;

        let error = validate_state_parent_cycles_with_step(&model, || traversal_steps += 1)
            .expect_err("State parent cycle must be rejected");

        let Error::InvalidModel { message } = error else {
            panic!("expected InvalidModel");
        };
        assert_eq!(message, "state parent cycle involving cycle-a");
        assert_eq!(traversal_steps, 4);
    }

    #[test]
    fn state_parent_cycle_validation_keeps_the_existing_work_reservation_boundary() {
        let model = state_parent_chain(32);
        let sizing_meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let sizing_control = OperationLayoutWorkControl::new(sizing_meter);
        let reserved_work = state_layout_adapter_work(&model, &sizing_control)
            .expect("State adapter work reservation");

        let exact_meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, reserved_work)
                .expect("exact State work limit"),
        ));
        let mut exact_control = OperationLayoutWorkControl::new(Arc::clone(&exact_meter));
        exact_control
            .charge_adapter(reserved_work)
            .expect("exact State work reservation");
        validate_state_parent_cycles(&model).expect("reserved validation work");
        assert_eq!(exact_meter.used(), reserved_work);

        let below_meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, reserved_work - 1)
                .expect("below-boundary State work limit"),
        ));
        let mut below_control = OperationLayoutWorkControl::new(Arc::clone(&below_meter));
        let error = below_control
            .charge_adapter(reserved_work)
            .expect_err("State work reservation must reject at W - 1");
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected ResourceLimitExceeded");
        };
        assert_eq!(limit.actual, reserved_work);
        assert_eq!(limit.max, reserved_work - 1);
        assert_eq!(below_meter.used(), 0);
    }

    #[test]
    fn hidden_prefix_matcher_preserves_note_boundary_semantics() {
        let matcher = HiddenPrefixMatcher::from_prefixes([
            "note-root".to_string(),
            "分组".to_string(),
            "note-root".to_string(),
        ]);

        assert!(matcher.is_hidden("note-root"));
        assert!(matcher.is_hidden("note-root----edge"));
        assert!(matcher.is_hidden("分组----节点"));
        assert!(!matcher.is_hidden("note-root---edge"));
        assert!(!matcher.is_hidden("note-root-child"));
        assert!(!matcher.is_hidden("分组节点"));
        assert!(!matcher.is_hidden("other"));
    }
}
