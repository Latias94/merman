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
    ) -> Result<Self> {
        let mut section_lookup = HashMap::new();
        section_lookup
            .try_reserve(admission.section_capacity)
            .map_err(|_| layout_allocation_error())?;
        for (section_index, section) in sections.iter().enumerate() {
            match section_lookup.entry(section.as_str()) {
                Entry::Vacant(entry) => {
                    entry.insert(SectionLookup::Unique(section_index));
                }
                Entry::Occupied(mut entry) => {
                    entry.insert(SectionLookup::Ambiguous);
                }
            }
        }

        let mut tasks_by_section = Vec::new();
        tasks_by_section
            .try_reserve(admission.section_capacity)
            .map_err(|_| layout_allocation_error())?;
        for _ in sections {
            tasks_by_section.push(Vec::new());
        }
        let mut orphan_task_indices = Vec::new();

        for (task_index, task) in tasks.iter().enumerate() {
            let section_index = match task.section_index() {
                Some(section_index) => Some(section_index),
                None => match section_lookup.get(task.section_label()) {
                    Some(SectionLookup::Unique(section_index)) => Some(*section_index),
                    Some(SectionLookup::Ambiguous) => {
                        return Err(AsciiError::UnsupportedFeature {
                            diagram_type,
                            feature: "ambiguous section label without occurrence index",
                        });
                    }
                    None => None,
                },
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
        resources: &mut ResourceContext,
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
    resources: &mut ResourceContext,
) -> Result<SectionedTextPlan> {
    admit_then_materialize_sectioned_text(diagram_type, sections, tasks, resources, |admission| {
        SectionedTextPlan::materialize(diagram_type, sections, tasks, admission)
    })
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn plan_sectioned_text_with_probe<T: SectionedTextTask>(
    diagram_type: &'static str,
    sections: &[String],
    tasks: &[T],
    resources: &mut ResourceContext,
    materialized: &std::cell::Cell<bool>,
) -> Result<SectionedTextPlan> {
    admit_then_materialize_sectioned_text(diagram_type, sections, tasks, resources, |admission| {
        materialized.set(true);
        SectionedTextPlan::materialize(diagram_type, sections, tasks, admission)
    })
}

fn admit_then_materialize_sectioned_text<T>(
    diagram_type: &'static str,
    sections: &[String],
    tasks: &[T],
    resources: &mut ResourceContext,
    materialize: impl FnOnce(SectionedTextAdmission) -> Result<SectionedTextPlan>,
) -> Result<SectionedTextPlan>
where
    T: SectionedTextTask,
{
    let admission = SectionedTextAdmission::preflight(diagram_type, sections, tasks, resources)?;
    materialize(admission)
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
    use std::cell::Cell;

    fn assert_exact_and_max_minus_one<T: SectionedTextTask>(
        diagram_type: &'static str,
        sections: &[String],
        tasks: &[T],
    ) {
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut measured = ResourceContext::new(unbounded);
        plan_sectioned_text(diagram_type, sections, tasks, &mut measured)
            .expect("unbounded section planning should succeed");
        let exact_work = measured.layout_work_used();
        assert!(exact_work > 1);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact section-planning limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        let exact_materialized = Cell::new(false);
        plan_sectioned_text_with_probe(
            diagram_type,
            sections,
            tasks,
            &mut exact_resources,
            &exact_materialized,
        )
        .expect("exact section-planning limit should permit materialization");
        assert!(exact_materialized.get());
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("max-minus-one section-planning limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        let below_materialized = Cell::new(false);
        let error = plan_sectioned_text_with_probe(
            diagram_type,
            sections,
            tasks,
            &mut below_resources,
            &below_materialized,
        )
        .expect_err("max-minus-one section-planning limit should reject before materialization");
        assert!(!below_materialized.get());
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual > details.max
                    && details.max == exact_work - 1
        ));
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
}
