use crate::resources::OperationWorkMeter;
use crate::{Error, Result};
use merman_model::OperationPhase;
use merman_model::families::timeline::{TimelineDiagramModel, TimelineTask};
use std::collections::HashMap;
use std::collections::hash_map::Entry;

#[derive(Debug)]
pub(super) struct TimelineTaskIndex<'model> {
    tasks_by_section: Vec<Vec<&'model TimelineTask>>,
    section_buckets: Vec<usize>,
    orphan_tasks: Vec<&'model TimelineTask>,
}

#[derive(Clone, Copy)]
struct SectionLookup {
    bucket_index: usize,
    ambiguous: bool,
}

impl<'model> TimelineTaskIndex<'model> {
    pub(super) fn new(
        model: &'model TimelineDiagramModel,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self> {
        work_meter.checkpoint(OperationPhase::Layout)?;
        let mut section_lookup = HashMap::with_capacity(model.sections.len());
        let mut tasks_by_section = Vec::new();
        let mut section_buckets = Vec::with_capacity(model.sections.len());
        let mut bucket_work = Vec::new();
        for section in &model.sections {
            let section_work = section
                .len()
                .checked_add(1)
                .ok_or_else(|| work_meter.arithmetic_overflow())?;
            work_meter.charge(section_work)?;
            let bucket_index = match section_lookup.entry(section.as_str()) {
                Entry::Vacant(entry) => {
                    let bucket_index = tasks_by_section.len();
                    entry.insert(SectionLookup {
                        bucket_index,
                        ambiguous: false,
                    });
                    tasks_by_section.push(Vec::new());
                    bucket_work.push(0usize);
                    bucket_index
                }
                Entry::Occupied(mut entry) => {
                    entry.get_mut().ambiguous = true;
                    entry.get().bucket_index
                }
            };
            section_buckets.push(bucket_index);
        }

        let mut orphan_tasks = Vec::new();
        let mut orphan_work = 0usize;
        for task in &model.tasks {
            work_meter.charge(1)?;
            let bucket_index = match task.section_index {
                Some(section_index) => {
                    let Some(section) = model.sections.get(section_index) else {
                        return Err(Error::InvalidModel {
                            message: format!(
                                "Timeline task {} section occurrence index {} is outside {} declared sections",
                                task.id,
                                section_index,
                                model.sections.len()
                            ),
                        });
                    };
                    if section != &task.section {
                        return Err(Error::InvalidModel {
                            message: format!(
                                "Timeline task {} section occurrence {} names `{}` but the task owns `{}`",
                                task.id, section_index, section, task.section
                            ),
                        });
                    }
                    Some(section_buckets[section_index])
                }
                None => match section_lookup.get(task.section.as_str()) {
                    Some(section) if section.ambiguous => {
                        return Err(Error::InvalidModel {
                            message: format!(
                                "Timeline task {} has ambiguous section label `{}` without an occurrence index",
                                task.id, task.section
                            ),
                        });
                    }
                    Some(section) => Some(section.bucket_index),
                    None => None,
                },
            };
            let mut task_work = task
                .task
                .len()
                .checked_add(1)
                .ok_or_else(|| work_meter.arithmetic_overflow())?;
            for event in &task.events {
                work_meter.charge(1)?;
                task_work = task_work
                    .checked_add(event.len())
                    .and_then(|work| work.checked_add(1))
                    .ok_or_else(|| work_meter.arithmetic_overflow())?;
            }
            match bucket_index {
                Some(bucket_index) => {
                    tasks_by_section[bucket_index].push(task);
                    bucket_work[bucket_index] = bucket_work[bucket_index]
                        .checked_add(task_work)
                        .ok_or_else(|| work_meter.arithmetic_overflow())?;
                }
                None => {
                    orphan_tasks.push(task);
                    orphan_work = orphan_work
                        .checked_add(task_work)
                        .ok_or_else(|| work_meter.arithmetic_overflow())?;
                }
            }
        }

        // Admit the rendered expansion before layout allocates repeated nodes. Buckets retain
        // each task once; every authored section consumes its shared bucket's output work.
        if model.sections.is_empty() {
            work_meter.charge(orphan_work)?;
        } else {
            for &bucket_index in &section_buckets {
                work_meter.charge(bucket_work[bucket_index])?;
            }
        }

        Ok(Self {
            tasks_by_section,
            section_buckets,
            orphan_tasks,
        })
    }

    pub(super) fn tasks_for_section(&self, section_index: usize) -> &[&'model TimelineTask] {
        &self.tasks_by_section[self.section_buckets[section_index]]
    }

    pub(super) fn orphan_tasks(&self) -> &[&'model TimelineTask] {
        &self.orphan_tasks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::RenderResourcePolicy;

    fn task(id: i64, section: &str, section_index: Option<usize>, label: &str) -> TimelineTask {
        TimelineTask {
            id,
            section: section.to_string(),
            section_index,
            task_type: section.to_string(),
            task: label.to_string(),
            score: 0,
            events: Vec::new(),
        }
    }

    fn labels<'task>(tasks: &[&'task TimelineTask]) -> Vec<&'task str> {
        tasks.iter().map(|task| task.task.as_str()).collect()
    }

    #[test]
    fn shares_named_buckets_without_changing_authored_occurrences() {
        let model = TimelineDiagramModel {
            sections: vec!["Repeated".into(), "Repeated".into(), "Unique".into()],
            tasks: vec![
                task(0, "Repeated", Some(0), "First"),
                task(1, "Repeated", Some(1), "Second"),
                task(2, "Unique", None, "Legacy A"),
                task(3, "Unique", None, "Legacy B"),
            ],
            ..Default::default()
        };

        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let index =
            TimelineTaskIndex::new(&model, &work_meter).expect("valid occurrence ownership");

        assert_eq!(labels(index.tasks_for_section(0)), ["First", "Second"]);
        assert_eq!(labels(index.tasks_for_section(1)), ["First", "Second"]);
        assert_eq!(labels(index.tasks_for_section(2)), ["Legacy A", "Legacy B"]);
        assert!(std::ptr::eq(
            index.tasks_for_section(0),
            index.tasks_for_section(1)
        ));
        assert_eq!(
            index.tasks_by_section.iter().map(Vec::len).sum::<usize>(),
            model.tasks.len()
        );
        assert_eq!(model.tasks[0].section_index, Some(0));
        assert_eq!(model.tasks[1].section_index, Some(1));
        assert!(index.orphan_tasks().is_empty());
    }

    #[test]
    fn keeps_unmatched_legacy_tasks_as_orphans_in_model_order() {
        let model = TimelineDiagramModel {
            sections: vec!["Known".into()],
            tasks: vec![
                task(0, "Missing", None, "Unknown"),
                task(1, "Known", Some(0), "Declared"),
                task(2, "", None, "Loose"),
            ],
            ..Default::default()
        };

        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let index = TimelineTaskIndex::new(&model, &work_meter).expect("valid legacy ownership");

        assert_eq!(labels(index.tasks_for_section(0)), ["Declared"]);
        assert_eq!(labels(index.orphan_tasks()), ["Unknown", "Loose"]);
    }

    #[test]
    fn rejects_ambiguous_legacy_labels() {
        let model = TimelineDiagramModel {
            sections: vec!["Repeated".into(), "Repeated".into()],
            tasks: vec![task(0, "Repeated", None, "Ambiguous")],
            ..Default::default()
        };

        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let error = TimelineTaskIndex::new(&model, &work_meter)
            .expect_err("duplicate labels require occurrence ownership");

        assert!(matches!(
            error,
            Error::InvalidModel { message }
                if message.contains("ambiguous section label `Repeated`")
        ));
    }

    #[test]
    fn rejects_invalid_explicit_occurrences() {
        for (section, section_index, expected_message) in [
            ("Known", 1, "outside 1 declared sections"),
            ("Other", 0, "names `Known` but the task owns `Other`"),
        ] {
            let model = TimelineDiagramModel {
                sections: vec!["Known".into()],
                tasks: vec![task(0, section, Some(section_index), "Invalid")],
                ..Default::default()
            };

            let work_meter =
                OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
            let error = TimelineTaskIndex::new(&model, &work_meter)
                .expect_err("invalid occurrence ownership must fail closed");

            assert!(matches!(
                error,
                Error::InvalidModel { message } if message.contains(expected_message)
            ));
        }
    }
}
