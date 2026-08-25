use super::projection_allocation_failed;
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use merman_core::OperationPhase;
use merman_core::diagrams::sequence::{
    SequenceActorLifecycle as CoreSequenceActorLifecycle, SequenceDiagramRenderModel,
    SequenceMessageKind as CoreSequenceMessageKind,
};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct SequenceActorLifecycle {
    pub(super) created_at: Option<usize>,
    pub(super) destroyed_at: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SequenceLifecycleKind {
    // The variant order mirrors Mermaid's AddMessage state machine: a pending
    // creation consumes the next signal before a pending destruction does.
    Created,
    Destroyed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceLifecycleRequest<'a> {
    actor_id: &'a str,
    actor_index: usize,
    model_index: usize,
    kind: SequenceLifecycleKind,
}

pub(super) fn resolve_actor_lifecycles(
    model: &SequenceDiagramRenderModel,
    participant_index: &HashMap<&str, usize>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<SequenceActorLifecycle>> {
    if let Some(lifecycles) = &model.actor_lifecycles {
        return project_resolved_actor_lifecycles(
            model,
            lifecycles,
            participant_index,
            resources,
            execution,
        );
    }

    resolve_compatibility_actor_lifecycles(model, participant_index, resources, execution)
}

fn project_resolved_actor_lifecycles(
    model: &SequenceDiagramRenderModel,
    resolved: &[CoreSequenceActorLifecycle],
    participant_index: &HashMap<&str, usize>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<SequenceActorLifecycle>> {
    let mut edge_count = 0usize;
    for lifecycle in resolved {
        execution.checkpoint(OperationPhase::Semantic)?;
        let count =
            resources.checked_work_add(edge_count, usize::from(lifecycle.created_at.is_some()))?;
        edge_count =
            resources.checked_work_add(count, usize::from(lifecycle.destroyed_at.is_some()))?;
    }
    // Visit every sidecar slot once to count owned edges and once to validate
    // and project it. Each present lifecycle edge adds one signal lookup.
    let resolved_work = resources.checked_work_mul(resolved.len(), 2)?;
    let work = resources.checked_work_add(participant_index.len(), resolved_work)?;
    let work = resources.checked_work_add(work, edge_count)?;
    execution.checkpoint(OperationPhase::Semantic)?;
    resources.charge_layout_work(work)?;

    let mut lifecycles = Vec::new();
    lifecycles
        .try_reserve_exact(participant_index.len())
        .map_err(|_| projection_allocation_failed())?;
    lifecycles.resize(participant_index.len(), SequenceActorLifecycle::default());
    if resolved.len() != model.actor_order.len() || resolved.len() != participant_index.len() {
        return Err(sequence_lifecycle_feature("actor lifecycle actors"));
    }

    for (actor_index, lifecycle) in resolved.iter().enumerate() {
        execution.checkpoint(OperationPhase::Semantic)?;
        let actor_id = model
            .actor_order
            .get(actor_index)
            .ok_or_else(|| sequence_lifecycle_feature("actor lifecycle actors"))?;
        if actor_lifecycle_index(participant_index, actor_id, "actor lifecycle actors")?
            != actor_index
        {
            return Err(sequence_lifecycle_feature("actor lifecycle actors"));
        }
        if let Some(model_index) = lifecycle.created_at {
            let message = resolved_lifecycle_signal(model, model_index)?;
            if message.to.as_deref() != Some(actor_id.as_str()) {
                return Err(sequence_lifecycle_feature("actor creation messages"));
            }
            lifecycles[actor_index].created_at = Some(model_index);
        }
        if let Some(model_index) = lifecycle.destroyed_at {
            let message = resolved_lifecycle_signal(model, model_index)?;
            if message.from.as_deref() != Some(actor_id.as_str())
                && message.to.as_deref() != Some(actor_id.as_str())
            {
                return Err(sequence_lifecycle_feature("actor destruction messages"));
            }
            lifecycles[actor_index].destroyed_at = Some(model_index);
        }
        validate_lifecycle_order(std::slice::from_ref(&lifecycles[actor_index]), execution)?;
    }

    Ok(lifecycles)
}

fn resolve_compatibility_actor_lifecycles(
    model: &SequenceDiagramRenderModel,
    participant_index: &HashMap<&str, usize>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<SequenceActorLifecycle>> {
    let request_count =
        resources.checked_work_add(model.created_actors.len(), model.destroyed_actors.len())?;
    let sort_levels = if request_count <= 1 {
        0
    } else {
        usize::BITS as usize - (request_count - 1).leading_zeros() as usize
    };
    // Sorting is in-place. Budget two key/comparison visits per request and
    // level, then account separately for collection and ordered consumption.
    let sort_work =
        resources.checked_work_mul(resources.checked_work_mul(request_count, sort_levels)?, 2)?;
    let request_work = resources.checked_work_mul(request_count, 2)?;
    let participant_passes = if request_count == 0 { 1 } else { 2 };
    let participant_work =
        resources.checked_work_mul(participant_index.len(), participant_passes)?;
    let message_work = if request_count == 0 {
        0
    } else {
        model.messages.len()
    };
    let work = resources.checked_work_add(message_work, request_work)?;
    let work = resources.checked_work_add(work, participant_work)?;
    let work = resources.checked_work_add(work, sort_work)?;
    execution.checkpoint(OperationPhase::Semantic)?;
    resources.charge_layout_work(work)?;

    let mut lifecycles = Vec::new();
    lifecycles
        .try_reserve_exact(participant_index.len())
        .map_err(|_| projection_allocation_failed())?;
    lifecycles.resize(participant_index.len(), SequenceActorLifecycle::default());
    if request_count == 0 {
        return Ok(lifecycles);
    }

    let mut requests = Vec::new();
    requests
        .try_reserve_exact(request_count)
        .map_err(|_| projection_allocation_failed())?;
    for (actor_id, model_index) in &model.created_actors {
        execution.checkpoint(OperationPhase::Semantic)?;
        requests.push(SequenceLifecycleRequest {
            actor_id,
            actor_index: actor_lifecycle_index(
                participant_index,
                actor_id,
                "actor lifecycle actors",
            )?,
            model_index: actor_lifecycle_anchor(
                model.messages.len(),
                *model_index,
                "actor lifecycle message indices",
            )?,
            kind: SequenceLifecycleKind::Created,
        });
    }
    for (actor_id, model_index) in &model.destroyed_actors {
        execution.checkpoint(OperationPhase::Semantic)?;
        requests.push(SequenceLifecycleRequest {
            actor_id,
            actor_index: actor_lifecycle_index(
                participant_index,
                actor_id,
                "actor lifecycle actors",
            )?,
            model_index: actor_lifecycle_anchor(
                model.messages.len(),
                *model_index,
                "actor lifecycle message indices",
            )?,
            kind: SequenceLifecycleKind::Destroyed,
        });
    }
    sort_lifecycle_requests(&mut requests, execution)?;

    let mut next_request = 0usize;
    let mut pending_created = None;
    let mut pending_destroyed = None;
    for (model_index, message) in model.messages.iter().enumerate() {
        execution.checkpoint(OperationPhase::Semantic)?;
        while requests
            .get(next_request)
            .is_some_and(|request| request.model_index == model_index)
        {
            execution.checkpoint(OperationPhase::Semantic)?;
            let request = requests[next_request];
            next_request += 1;
            register_pending_lifecycle(request, &mut pending_created, &mut pending_destroyed)?;
        }

        if message.semantic_kind() != CoreSequenceMessageKind::Signal {
            continue;
        }
        if let Some(request) = pending_created.take() {
            if message.to.as_deref() != Some(request.actor_id) {
                return Err(sequence_lifecycle_feature("actor creation messages"));
            }
            lifecycles[request.actor_index].created_at = Some(model_index);
        } else if let Some(request) = pending_destroyed.take() {
            if message.from.as_deref() != Some(request.actor_id)
                && message.to.as_deref() != Some(request.actor_id)
            {
                return Err(sequence_lifecycle_feature("actor destruction messages"));
            }
            lifecycles[request.actor_index].destroyed_at = Some(model_index);
        }
    }

    while let Some(request) = requests.get(next_request).copied() {
        execution.checkpoint(OperationPhase::Semantic)?;
        debug_assert_eq!(request.model_index, model.messages.len());
        next_request += 1;
        register_pending_lifecycle(request, &mut pending_created, &mut pending_destroyed)?;
    }
    if pending_created.is_some() {
        return Err(sequence_lifecycle_feature("actor creation messages"));
    }
    if pending_destroyed.is_some() {
        return Err(sequence_lifecycle_feature("actor destruction messages"));
    }

    validate_lifecycle_order(&lifecycles, execution)?;

    Ok(lifecycles)
}

fn resolved_lifecycle_signal(
    model: &SequenceDiagramRenderModel,
    model_index: usize,
) -> Result<&merman_core::diagrams::sequence::SequenceMessage> {
    let message = model
        .messages
        .get(model_index)
        .ok_or(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "actor lifecycle message indices",
        })?;
    if message.semantic_kind() != CoreSequenceMessageKind::Signal {
        return Err(sequence_lifecycle_feature("actor lifecycle message kinds"));
    }
    Ok(message)
}

fn validate_lifecycle_order(
    lifecycles: &[SequenceActorLifecycle],
    execution: AsciiExecution<'_>,
) -> Result<()> {
    for lifecycle in lifecycles {
        execution.checkpoint(OperationPhase::Semantic)?;
        if let (Some(created_at), Some(destroyed_at)) =
            (lifecycle.created_at, lifecycle.destroyed_at)
            && destroyed_at <= created_at
        {
            return Err(sequence_lifecycle_feature("actor lifecycle order"));
        }
    }
    Ok(())
}

fn sort_lifecycle_requests(
    requests: &mut [SequenceLifecycleRequest<'_>],
    execution: AsciiExecution<'_>,
) -> Result<()> {
    if requests.len() < 2 {
        return Ok(());
    }

    for root in (0..requests.len() / 2).rev() {
        sift_lifecycle_request_down(requests, root, requests.len(), execution)?;
    }
    for end in (1..requests.len()).rev() {
        execution.checkpoint(OperationPhase::Semantic)?;
        requests.swap(0, end);
        sift_lifecycle_request_down(requests, 0, end, execution)?;
    }
    Ok(())
}

fn sift_lifecycle_request_down(
    requests: &mut [SequenceLifecycleRequest<'_>],
    mut root: usize,
    end: usize,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    loop {
        execution.checkpoint(OperationPhase::Semantic)?;
        let Some(left) = root.checked_mul(2).and_then(|value| value.checked_add(1)) else {
            return Ok(());
        };
        if left >= end {
            return Ok(());
        }
        let right = left + 1;
        let child = if right < end
            && lifecycle_request_key(requests[left]) < lifecycle_request_key(requests[right])
        {
            right
        } else {
            left
        };
        if lifecycle_request_key(requests[root]) >= lifecycle_request_key(requests[child]) {
            return Ok(());
        }
        requests.swap(root, child);
        root = child;
    }
}

fn lifecycle_request_key(request: SequenceLifecycleRequest<'_>) -> (usize, SequenceLifecycleKind) {
    (request.model_index, request.kind)
}

fn register_pending_lifecycle<'a>(
    request: SequenceLifecycleRequest<'a>,
    pending_created: &mut Option<SequenceLifecycleRequest<'a>>,
    pending_destroyed: &mut Option<SequenceLifecycleRequest<'a>>,
) -> Result<()> {
    match request.kind {
        SequenceLifecycleKind::Created => {
            if pending_created.replace(request).is_some() {
                return Err(sequence_lifecycle_feature("actor creation messages"));
            }
        }
        SequenceLifecycleKind::Destroyed => {
            if pending_destroyed.replace(request).is_some() {
                return Err(sequence_lifecycle_feature("actor destruction messages"));
            }
        }
    }
    Ok(())
}

fn actor_lifecycle_index(
    participant_index: &HashMap<&str, usize>,
    actor_id: &str,
    feature: &'static str,
) -> Result<usize> {
    participant_index
        .get(actor_id)
        .copied()
        .ok_or(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature,
        })
}

fn actor_lifecycle_anchor(
    message_count: usize,
    model_index: usize,
    index_feature: &'static str,
) -> Result<usize> {
    // Mermaid stores the current message length as an anchor when `create` or
    // `destroy` is parsed. An anchor at EOF is therefore valid input; it just
    // has no following Signal and is rejected after the ordered scan.
    if model_index > message_count {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: index_feature,
        });
    }
    Ok(model_index)
}

fn sequence_lifecycle_feature(feature: &'static str) -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::AsciiExecution;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;
    use merman_core::{Engine, ParseOptions, RenderSemanticModel};

    fn shared_anchor_model() -> SequenceDiagramRenderModel {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                concat!(
                    "sequenceDiagram\n",
                    "participant A\n",
                    "participant C\n",
                    "create participant B\n",
                    "destroy A\n",
                    "C->>B: create\n",
                    "A--xC: destroy\n",
                ),
                ParseOptions::strict(),
            )
            .expect("shared-anchor lifecycle fixture should parse")
            .expect("shared-anchor lifecycle fixture should be detected");
        match parsed.into_parts().1 {
            RenderSemanticModel::Sequence(model) => model,
            other => panic!("expected sequence model, got {}", other.kind()),
        }
    }

    fn participant_index(model: &SequenceDiagramRenderModel) -> HashMap<&str, usize> {
        model
            .actor_order
            .iter()
            .enumerate()
            .map(|(index, actor_id)| (actor_id.as_str(), index))
            .collect()
    }

    #[test]
    fn shared_anchor_resolution_is_ordered_and_budgeted_before_materialization() {
        let model = shared_anchor_model();
        let participant_index = participant_index(&model);
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut resources = ResourceContext::new(unbounded);
        let lifecycles = resolve_actor_lifecycles(
            &model,
            &participant_index,
            &mut resources,
            AsciiExecution::for_test(&unbounded),
        )
        .expect("shared lifecycle anchors should consume successive signals");
        let total_work = resources.layout_work_used();

        assert_eq!(lifecycles[participant_index["B"]].created_at, Some(0));
        assert_eq!(lifecycles[participant_index["A"]].destroyed_at, Some(1));

        let exact = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, total_work)
            .expect("exact lifecycle work limit should be valid");
        let mut exact_resources = ResourceContext::new(exact);
        resolve_actor_lifecycles(
            &model,
            &participant_index,
            &mut exact_resources,
            AsciiExecution::for_test(&exact),
        )
        .expect("exact lifecycle work limit should pass");
        assert_eq!(exact_resources.layout_work_used(), total_work);

        let below = unbounded
            .with_limit(
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                total_work.saturating_sub(1),
            )
            .expect("lifecycle work limit minus one should be valid");
        let mut below_resources = ResourceContext::new(below);
        let error = resolve_actor_lifecycles(
            &model,
            &participant_index,
            &mut below_resources,
            AsciiExecution::for_test(&below),
        )
        .expect_err("lifecycle planning must reject before request materialization");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == total_work
                    && details.max == total_work - 1
        ));
        assert_eq!(below_resources.layout_work_used(), 0);
    }

    #[test]
    fn resolved_actor_lifecycles_require_actor_order_alignment() {
        let mut model = shared_anchor_model();
        model
            .actor_lifecycles
            .as_mut()
            .expect("parser-backed model should expose resolved lifecycle truth")
            .pop();
        let participant_index = participant_index(&model);
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut resources = ResourceContext::new(policy);

        let error = resolve_actor_lifecycles(
            &model,
            &participant_index,
            &mut resources,
            AsciiExecution::for_test(&policy),
        )
        .expect_err("misaligned lifecycle sidecars must be rejected");
        assert!(matches!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "actor lifecycle actors",
            }
        ));
    }
}
