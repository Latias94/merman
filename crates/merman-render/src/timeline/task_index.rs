use crate::{Error, Result};
use merman_core::diagrams::timeline::{TimelineDiagramRenderModel, TimelineRenderTask};
use std::collections::HashMap;
use std::collections::hash_map::Entry;

#[derive(Debug)]
pub(super) struct TimelineTaskIndex<'model> {
    tasks_by_section: Vec<Vec<&'model TimelineRenderTask>>,
    orphan_tasks: Vec<&'model TimelineRenderTask>,
}

#[derive(Clone, Copy)]
enum SectionLookup {
    Unique(usize),
    Ambiguous,
}

impl<'model> TimelineTaskIndex<'model> {
    pub(super) fn new(model: &'model TimelineDiagramRenderModel) -> Result<Self> {
        let mut section_lookup = HashMap::with_capacity(model.sections.len());
        for (section_index, section) in model.sections.iter().enumerate() {
            match section_lookup.entry(section.as_str()) {
                Entry::Vacant(entry) => {
                    entry.insert(SectionLookup::Unique(section_index));
                }
                Entry::Occupied(mut entry) => {
                    entry.insert(SectionLookup::Ambiguous);
                }
            }
        }

        let mut tasks_by_section = vec![Vec::new(); model.sections.len()];
        let mut orphan_tasks = Vec::new();
        for task in &model.tasks {
            let resolved_section_index = match task.section_index {
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
                    Some(section_index)
                }
                None => match section_lookup.get(task.section.as_str()) {
                    Some(SectionLookup::Unique(section_index)) => Some(*section_index),
                    Some(SectionLookup::Ambiguous) => {
                        return Err(Error::InvalidModel {
                            message: format!(
                                "Timeline task {} has ambiguous section label `{}` without an occurrence index",
                                task.id, task.section
                            ),
                        });
                    }
                    None => None,
                },
            };
            match resolved_section_index {
                Some(section_index) => tasks_by_section[section_index].push(task),
                None => orphan_tasks.push(task),
            }
        }

        Ok(Self {
            tasks_by_section,
            orphan_tasks,
        })
    }

    pub(super) fn tasks_for_section(&self, section_index: usize) -> &[&'model TimelineRenderTask] {
        &self.tasks_by_section[section_index]
    }

    pub(super) fn orphan_tasks(&self) -> &[&'model TimelineRenderTask] {
        &self.orphan_tasks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(
        id: i64,
        section: &str,
        section_index: Option<usize>,
        label: &str,
    ) -> TimelineRenderTask {
        TimelineRenderTask {
            id,
            section: section.to_string(),
            section_index,
            task_type: section.to_string(),
            task: label.to_string(),
            score: 0,
            events: Vec::new(),
        }
    }

    fn labels<'task>(tasks: &[&'task TimelineRenderTask]) -> Vec<&'task str> {
        tasks.iter().map(|task| task.task.as_str()).collect()
    }

    #[test]
    fn indexes_parser_occurrences_and_unique_legacy_labels_once() {
        let mut model = TimelineDiagramRenderModel::default();
        model.sections = vec!["Repeated".into(), "Repeated".into(), "Unique".into()];
        model.tasks = vec![
            task(0, "Repeated", Some(0), "First"),
            task(1, "Repeated", Some(1), "Second"),
            task(2, "Unique", None, "Legacy A"),
            task(3, "Unique", None, "Legacy B"),
        ];

        let index = TimelineTaskIndex::new(&model).expect("valid occurrence ownership");

        assert_eq!(labels(index.tasks_for_section(0)), ["First"]);
        assert_eq!(labels(index.tasks_for_section(1)), ["Second"]);
        assert_eq!(labels(index.tasks_for_section(2)), ["Legacy A", "Legacy B"]);
        assert!(index.orphan_tasks().is_empty());
    }

    #[test]
    fn keeps_unmatched_legacy_tasks_as_orphans_in_model_order() {
        let mut model = TimelineDiagramRenderModel::default();
        model.sections = vec!["Known".into()];
        model.tasks = vec![
            task(0, "Missing", None, "Unknown"),
            task(1, "Known", Some(0), "Declared"),
            task(2, "", None, "Loose"),
        ];

        let index = TimelineTaskIndex::new(&model).expect("valid legacy ownership");

        assert_eq!(labels(index.tasks_for_section(0)), ["Declared"]);
        assert_eq!(labels(index.orphan_tasks()), ["Unknown", "Loose"]);
    }

    #[test]
    fn rejects_ambiguous_legacy_labels() {
        let mut model = TimelineDiagramRenderModel::default();
        model.sections = vec!["Repeated".into(), "Repeated".into()];
        model.tasks = vec![task(0, "Repeated", None, "Ambiguous")];

        let error = TimelineTaskIndex::new(&model)
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
            let mut model = TimelineDiagramRenderModel::default();
            model.sections = vec!["Known".into()];
            model.tasks = vec![task(0, section, Some(section_index), "Invalid")];

            let error = TimelineTaskIndex::new(&model)
                .expect_err("invalid occurrence ownership must fail closed");

            assert!(matches!(
                error,
                Error::InvalidModel { message } if message.contains(expected_message)
            ));
        }
    }
}
