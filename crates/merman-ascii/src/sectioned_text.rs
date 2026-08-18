use crate::Result;
use crate::error::AsciiError;
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::charge_text_layout;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

pub(crate) trait SectionedTextTask {
    fn section_label(&self) -> &str;
    fn section_index(&self) -> Option<usize>;
}

#[derive(Debug)]
pub(crate) struct SectionedTextPlan {
    tasks_by_section: Vec<Vec<usize>>,
    orphan_task_indices: Vec<usize>,
}

impl SectionedTextPlan {
    pub(crate) fn tasks_for_section(&self, section_index: usize) -> &[usize] {
        self.tasks_by_section
            .get(section_index)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub(crate) fn orphan_task_indices(&self) -> &[usize] {
        &self.orphan_task_indices
    }

    fn materialize<T: SectionedTextTask>(
        diagram_type: &'static str,
        sections: &[String],
        tasks: &[T],
        admission: SectionedTextAdmission,
        resources: &ResourceContext,
    ) -> Result<Self> {
        resources.checkpoint()?;
        let mut section_lookup = HashMap::new();
        section_lookup
            .try_reserve(admission.section_capacity)
            .map_err(|_| layout_allocation_error())?;
        for (section_index, section) in sections.iter().enumerate() {
            resources.checkpoint()?;
            match section_lookup.entry(section.as_str()) {
                Entry::Vacant(entry) => {
                    entry.insert(SectionLookup::Unique(section_index));
                }
                Entry::Occupied(mut entry) => {
                    entry.insert(SectionLookup::Ambiguous);
                }
            }
        }

        resources.checkpoint()?;
        let mut tasks_by_section = Vec::new();
        tasks_by_section
            .try_reserve(admission.section_capacity)
            .map_err(|_| layout_allocation_error())?;
        for _ in sections {
            resources.checkpoint()?;
            tasks_by_section.push(Vec::new());
        }
        let mut orphan_task_indices = Vec::new();

        for (task_index, task) in tasks.iter().enumerate() {
            resources.checkpoint()?;
            let section_index = match task.section_index() {
                Some(section_index) => Some(section_index),
                None => {
                    let section_label = task.section_label();
                    resources.checkpoint()?;
                    match section_lookup.get(section_label) {
                        Some(SectionLookup::Unique(section_index)) => Some(*section_index),
                        Some(SectionLookup::Ambiguous) => {
                            return Err(AsciiError::UnsupportedFeature {
                                diagram_type,
                                feature: "ambiguous section label without occurrence index",
                            });
                        }
                        None => None,
                    }
                }
            };
            let destination = match section_index {
                Some(section_index) => tasks_by_section.get_mut(section_index).ok_or(
                    AsciiError::UnsupportedFeature {
                        diagram_type,
                        feature: "section occurrence index outside declared sections",
                    },
                )?,
                None => &mut orphan_task_indices,
            };
            resources.checkpoint()?;
            destination
                .try_reserve(1)
                .map_err(|_| layout_allocation_error())?;
            destination.push(task_index);
        }

        Ok(Self {
            tasks_by_section,
            orphan_task_indices,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct SectionedTextAdmission {
    section_capacity: usize,
}

impl SectionedTextAdmission {
    fn preflight<T: SectionedTextTask>(
        diagram_type: &'static str,
        sections: &[String],
        tasks: &[T],
        resources: &ResourceContext,
    ) -> Result<Self> {
        // Each section pays for lookup insertion, bucket ownership, and one indexed visit. Each
        // task pays for its result slot, ownership validation, and single assignment visit.
        let section_work = resources.checked_work_mul(sections.len(), 3)?;
        let task_work = resources.checked_work_mul(tasks.len(), 3)?;
        let structural_work = resources.checked_work_add(section_work, task_work)?;
        resources.charge_layout_work(structural_work)?;

        for section in sections {
            charge_text_layout(resources, section)?;
        }
        for task in tasks {
            charge_text_layout(resources, task.section_label())?;
            if let Some(section_index) = task.section_index() {
                let Some(section) = sections.get(section_index) else {
                    return Err(AsciiError::UnsupportedFeature {
                        diagram_type,
                        feature: "section occurrence index outside declared sections",
                    });
                };
                if section != task.section_label() {
                    return Err(AsciiError::UnsupportedFeature {
                        diagram_type,
                        feature: "section occurrence label mismatch",
                    });
                }
            }
        }

        Ok(Self {
            section_capacity: sections.len(),
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum SectionLookup {
    Unique(usize),
    Ambiguous,
}

pub(crate) fn plan_sectioned_text<T: SectionedTextTask>(
    diagram_type: &'static str,
    sections: &[String],
    tasks: &[T],
    resources: &ResourceContext,
) -> Result<SectionedTextPlan> {
    resources.transaction(|resources| {
        let admission =
            SectionedTextAdmission::preflight(diagram_type, sections, tasks, resources)?;
        SectionedTextPlan::materialize(diagram_type, sections, tasks, admission, resources)
    })
}

fn layout_allocation_error() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::diagrams::journey::JourneyRenderTask;
    use merman_core::diagrams::timeline::TimelineRenderTask;
    use merman_core::resources::ResourceProfile;
    use merman_core::{CancelReason, OperationControl, OperationPhase};
    use std::cell::Cell;

    fn assert_exact_and_max_minus_one<T: SectionedTextTask>(
        diagram_type: &'static str,
        sections: &[String],
        tasks: &[T],
    ) {
        const SECTION_PLAN_WORK: usize = 48;
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, SECTION_PLAN_WORK)
            .expect("exact section-planning limit should be valid");
        let exact_resources = ResourceContext::new(exact_policy);
        plan_sectioned_text(diagram_type, sections, tasks, &exact_resources)
            .expect("exact section-planning limit should permit materialization");
        assert_eq!(exact_resources.layout_work_used(), SECTION_PLAN_WORK);

        let below_policy = unbounded
            .with_limit(
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                SECTION_PLAN_WORK - 1,
            )
            .expect("max-minus-one section-planning limit should be valid");
        let below_resources = ResourceContext::new(below_policy);
        let error = plan_sectioned_text(diagram_type, sections, tasks, &below_resources)
            .expect_err("max-minus-one section-planning limit should reject");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == SECTION_PLAN_WORK
                    && details.max == SECTION_PLAN_WORK - 1
        ));
        assert_eq!(below_resources.layout_work_used(), 0);
    }

    #[test]
    fn timeline_section_plan_admits_exact_work_before_allocating() {
        let sections = vec!["Repeated".to_string(), "Repeated".to_string()];
        let tasks = vec![
            TimelineRenderTask {
                id: 0,
                section: "Repeated".to_string(),
                section_index: Some(0),
                task_type: "Repeated".to_string(),
                task: "First".to_string(),
                score: 0,
                events: Vec::new(),
            },
            TimelineRenderTask {
                id: 1,
                section: "Repeated".to_string(),
                section_index: Some(1),
                task_type: "Repeated".to_string(),
                task: "Second".to_string(),
                score: 0,
                events: Vec::new(),
            },
        ];

        assert_exact_and_max_minus_one("timeline", &sections, &tasks);
    }

    #[test]
    fn journey_section_plan_admits_exact_work_before_allocating() {
        let sections = vec!["Repeated".to_string(), "Repeated".to_string()];
        let tasks = vec![
            JourneyRenderTask {
                score: 5,
                score_is_nan: false,
                people: Vec::new(),
                section: "Repeated".to_string(),
                section_index: Some(0),
                task_type: "Repeated".to_string(),
                task: "First".to_string(),
            },
            JourneyRenderTask {
                score: 3,
                score_is_nan: false,
                people: Vec::new(),
                section: "Repeated".to_string(),
                section_index: Some(1),
                task_type: "Repeated".to_string(),
                task: "Second".to_string(),
            },
        ];

        assert_exact_and_max_minus_one("journey", &sections, &tasks);
    }

    #[test]
    fn ambiguous_section_materialization_rolls_back_admission_work() {
        const PRIOR_WORK: usize = 3;
        const PRIOR_CELLS: usize = 2;
        let sections = vec!["Repeated".to_string(), "Repeated".to_string()];
        let tasks = vec![JourneyRenderTask {
            score: 5,
            score_is_nan: false,
            people: Vec::new(),
            section: "Repeated".to_string(),
            section_index: None,
            task_type: "Repeated".to_string(),
            task: "Task".to_string(),
        }];
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(policy);
        resources
            .charge_usage(PRIOR_WORK, PRIOR_CELLS)
            .expect("prior ledger usage should fit");

        let error = plan_sectioned_text("journey", &sections, &tasks, &resources)
            .expect_err("an unindexed duplicate section label must remain ambiguous");

        assert_eq!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "journey",
                feature: "ambiguous section label without occurrence index",
            }
        );
        assert_eq!(resources.layout_work_used(), PRIOR_WORK);
        assert_eq!(resources.document_cells_used(), PRIOR_CELLS);
    }

    struct CancellingSectionTask {
        section_calls: Cell<usize>,
        control: OperationControl,
    }

    impl SectionedTextTask for CancellingSectionTask {
        fn section_label(&self) -> &str {
            let calls = self.section_calls.get() + 1;
            self.section_calls.set(calls);
            if calls == 2 {
                self.control.cancel();
            }
            "Only"
        }

        fn section_index(&self) -> Option<usize> {
            None
        }
    }

    #[test]
    fn section_index_materialization_cancellation_rolls_back_admission_work() {
        const PRIOR_WORK: usize = 3;
        const PRIOR_CELLS: usize = 2;
        let sections = vec!["Only".to_string()];
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(policy);
        resources
            .charge_usage(PRIOR_WORK, PRIOR_CELLS)
            .expect("prior ledger usage should fit");
        let control = OperationControl::new();
        let tasks = vec![CancellingSectionTask {
            section_calls: Cell::new(0),
            control: control.clone(),
        }];
        let controlled = resources.controlled(control, OperationPhase::Layout);

        let error = plan_sectioned_text("journey", &sections, &tasks, &controlled)
            .expect_err("index materialization must observe cancellation before lookup");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), PRIOR_WORK);
        assert_eq!(resources.document_cells_used(), PRIOR_CELLS);
    }
}
