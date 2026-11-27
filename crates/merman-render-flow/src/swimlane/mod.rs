//! Measured Mermaid Swimlane layout, shared by Flow, ER, and Mindmap.

mod bounds;
pub mod config;
mod direction;
mod geometry;
pub mod lanes;
mod routing;
mod sugiyama;
#[cfg(test)]
mod tests;
mod work_budget;
pub mod working;

use merman_render_core::{Result, resources::OperationWorkMeter};
use std::{cmp::Ordering, sync::Arc};

/// Compare identifiers by their UTF-16 code units.
///
/// This matches JavaScript's relational string ordering while remaining independent of host
/// locale and bundled collation data. Every Swimlane algorithmic tie-break uses this comparator so
/// hash-map iteration order cannot affect cycle removal, layering, or lane ordering.
fn deterministic_identifier_cmp(left: &str, right: &str) -> Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}

/// Runs the Mermaid Swimlane passes on an admitted, measured working graph.
///
/// The caller supplies lane ids and edge slots in its projected order and performs source
/// admission before preparation. The solver retains the order of `original_edges`.
pub fn solve(
    working: &mut working::WorkingLayout,
    config: config::SwimlaneConfig,
    use_bpmn_drawn_size: bool,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<()> {
    let reversed = sugiyama::run(working, config);
    for edge in &mut working.original_edges {
        edge.reversed_for_layout = reversed.contains(&edge.id);
    }
    bounds::assign_canonical_group_bounds(working);
    let mut work_budget = work_budget::LayoutWorkBudget::for_operation(work_meter);
    routing::route(working, &mut work_budget, use_bpmn_drawn_size)?;
    direction::post_process(working, &mut work_budget)?;

    Ok(())
}

pub fn source_layout_work_units(nodes: usize, edges: usize) -> usize {
    // Source-item passes include projection and occurrence-identity preparation.
    nodes
        .saturating_mul(4)
        .saturating_add(edges.saturating_mul(5))
}

pub fn admit_layout_work(
    work_meter: &OperationWorkMeter,
    source_work: usize,
    visible_edges: usize,
) -> Result<()> {
    // Routing can compare each visible route with every earlier one. Also allow for a pair pass
    // in direction post-processing, which charges its inspected candidates separately.
    let route_pairs = work_budget::unordered_pair_count(visible_edges);
    let core_work = source_work.saturating_add(route_pairs);
    work_meter.preflight(core_work.saturating_add(route_pairs))?;
    work_meter.charge(core_work)?;
    Ok(())
}

#[cfg(test)]
mod work_tests {
    use super::{OperationWorkMeter, admit_layout_work, deterministic_identifier_cmp};
    use merman_model::OperationControl;
    use merman_render_core::Error;
    use merman_render_core::resources::{RenderResourcePolicy, ResourceLimitId};

    #[test]
    fn identifier_order_is_stable_utf16_code_unit_order() {
        let mut ids = [
            "B", "a", "A", "b", "a.1", "a-1", "a_1", "Z", "z", "é", "e", "中", "\u{e000}", "😀",
            "🧪",
        ];
        ids.sort_by(|left, right| deterministic_identifier_cmp(left, right));
        assert_eq!(
            ids,
            [
                "A", "B", "Z", "a", "a-1", "a.1", "a_1", "b", "e", "z", "é", "中", "😀", "🧪",
                "\u{e000}",
            ]
        );
    }

    #[test]
    fn core_layout_cost_accounts_routing_pairs_once() {
        let meter = OperationWorkMeter::new(RenderResourcePolicy::interactive());
        // Four nodes plus five source edges consume 41 linear units. The three visible edges
        // contribute three unordered route pairs.
        admit_layout_work(&meter, super::source_layout_work_units(4, 5), 3).unwrap();
        assert_eq!(meter.used(), 44);
    }

    #[test]
    fn preflight_reserves_direction_pairs_without_charging_them_to_core() {
        let policy = RenderResourcePolicy::interactive()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 36)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let error =
            admit_layout_work(&meter, super::source_layout_work_units(4, 3), 3).unwrap_err();
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected a layout work ceiling");
        };
        assert_eq!((limit.actual, limit.max), (37, 36));
        assert_eq!(meter.used(), 0);
    }

    #[test]
    fn layout_admission_observes_cancellation_after_source_preflight() {
        let control = OperationControl::new();
        let meter = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::interactive(),
            control.clone(),
        );
        let source_work = super::source_layout_work_units(4, 5);
        meter.preflight(source_work).unwrap();
        control.cancel();

        assert!(matches!(
            admit_layout_work(&meter, source_work, 1),
            Err(Error::Cancelled(_))
        ));
        assert_eq!(meter.used(), 0);
    }

    #[test]
    fn swimlane_layout_work_estimates_saturate() {
        assert_eq!(super::source_layout_work_units(usize::MAX, 0), usize::MAX);
        assert_eq!(super::source_layout_work_units(0, usize::MAX), usize::MAX);
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        admit_layout_work(&meter, 31, usize::MAX).unwrap();
        assert_eq!(meter.used(), usize::MAX);
    }
}
