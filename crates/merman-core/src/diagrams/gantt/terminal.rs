use super::{GanttClickEvent, GanttDiagramModel, GanttTask};
#[cfg(feature = "presentation")]
use super::{GanttDiagramRenderModel, GanttRenderTask};
use std::collections::HashMap;

/// Borrowed diagram fields shared by canonical and full Gantt terminal output.
#[derive(Debug, Clone, Copy)]
pub struct GanttTerminalMetadata<'a> {
    pub title: &'a Option<String>,
    pub acc_title: &'a Option<String>,
    pub acc_descr: &'a Option<String>,
    pub date_format: &'a str,
    pub axis_format: &'a str,
    pub tick_interval: &'a Option<String>,
    pub today_marker: &'a str,
    pub includes: &'a [String],
    pub excludes: &'a [String],
    pub inclusive_end_dates: bool,
    pub display_mode: &'a str,
    pub top_axis: bool,
    pub weekday: &'a str,
    pub weekend: &'a str,
    pub sections: &'a [String],
    pub links: &'a HashMap<String, String>,
    pub click_events: &'a HashMap<String, GanttClickEvent>,
}

impl<'a> GanttTerminalMetadata<'a> {
    fn new<Task>(model: &'a GanttDiagramModel<Task>, display_mode: &'a str) -> Self {
        Self {
            title: &model.title,
            acc_title: &model.acc_title,
            acc_descr: &model.acc_descr,
            date_format: &model.date_format,
            axis_format: &model.axis_format,
            tick_interval: &model.tick_interval,
            today_marker: &model.today_marker,
            includes: &model.includes,
            excludes: &model.excludes,
            inclusive_end_dates: model.inclusive_end_dates,
            display_mode,
            top_axis: model.top_axis,
            weekday: &model.weekday,
            weekend: &model.weekend,
            sections: &model.sections,
            links: &model.links,
            click_events: &model.click_events,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Tasks<'a> {
    Canonical(&'a [GanttTask]),
    #[cfg(feature = "presentation")]
    Full(&'a [GanttRenderTask]),
}

impl<'a> Tasks<'a> {
    fn len(self) -> usize {
        match self {
            Self::Canonical(tasks) => tasks.len(),
            #[cfg(feature = "presentation")]
            Self::Full(tasks) => tasks.len(),
        }
    }

    fn get(self, index: usize) -> Option<GanttTerminalTaskView<'a>> {
        Some(match self {
            Self::Canonical(tasks) => {
                let task = tasks.get(index)?;
                GanttTerminalTaskView {
                    semantic: task,
                    render_end_ms: task.render_end_ms,
                }
            }
            #[cfg(feature = "presentation")]
            Self::Full(tasks) => {
                let task = tasks.get(index)?;
                GanttTerminalTaskView {
                    semantic: &task.semantic,
                    render_end_ms: task.render_end_ms,
                }
            }
        })
    }
}

/// One borrowed source for Gantt terminal metadata and task endpoint facts.
#[derive(Debug, Clone, Copy)]
pub struct GanttTerminalView<'a> {
    model: Model<'a>,
    display_mode: &'a str,
}

#[derive(Debug, Clone, Copy)]
enum Model<'a> {
    Canonical(&'a GanttDiagramModel),
    #[cfg(feature = "presentation")]
    Full(&'a GanttDiagramRenderModel),
}

impl<'a> GanttTerminalView<'a> {
    pub fn from_model(model: &'a GanttDiagramModel, config: &'a crate::MermaidConfig) -> Self {
        Self {
            model: Model::Canonical(model),
            display_mode: config.get_str("gantt.displayMode").unwrap_or_default(),
        }
    }

    #[cfg(feature = "presentation")]
    pub fn from_render(model: &'a GanttDiagramRenderModel) -> Self {
        Self {
            model: Model::Full(model),
            display_mode: &model.display_mode,
        }
    }

    pub fn metadata(self) -> GanttTerminalMetadata<'a> {
        match self.model {
            Model::Canonical(model) => GanttTerminalMetadata::new(model, self.display_mode),
            #[cfg(feature = "presentation")]
            Model::Full(model) => GanttTerminalMetadata::new(&model.model, self.display_mode),
        }
    }

    fn task_slice(self) -> Tasks<'a> {
        match self.model {
            Model::Canonical(model) => Tasks::Canonical(&model.tasks),
            #[cfg(feature = "presentation")]
            Model::Full(model) => Tasks::Full(&model.tasks),
        }
    }

    pub fn tasks(self) -> GanttTerminalTaskIter<'a> {
        GanttTerminalTaskIter {
            tasks: self.task_slice(),
            index: 0,
        }
    }

    pub fn task(self, index: usize) -> Option<GanttTerminalTaskView<'a>> {
        self.task_slice().get(index)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GanttTerminalTaskView<'a> {
    pub semantic: &'a GanttTask,
    pub render_end_ms: Option<i64>,
}

impl std::ops::Deref for GanttTerminalTaskView<'_> {
    type Target = GanttTask;

    fn deref(&self) -> &Self::Target {
        self.semantic
    }
}

#[derive(Debug, Clone)]
pub struct GanttTerminalTaskIter<'a> {
    tasks: Tasks<'a>,
    index: usize,
}

impl<'a> Iterator for GanttTerminalTaskIter<'a> {
    type Item = GanttTerminalTaskView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let task = self.tasks.get(self.index)?;
        self.index += 1;
        Some(task)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.tasks.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for GanttTerminalTaskIter<'_> {}
impl std::iter::FusedIterator for GanttTerminalTaskIter<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_terminal_view_borrows_metadata_and_exact_task_iterator() {
        let model = GanttDiagramModel {
            sections: vec!["Delivery".to_string()],
            tasks: vec![GanttTask {
                render_end_ms: Some(23),
                ..Default::default()
            }],
            ..Default::default()
        };
        let config = crate::MermaidConfig::from_value(
            serde_json::json!({"gantt": {"displayMode": "compact"}}),
        );
        let view = GanttTerminalView::from_model(&model, &config);
        let metadata = view.metadata();
        assert_eq!(metadata.display_mode, "compact");
        assert!(std::ptr::eq(
            metadata.sections.as_ptr(),
            model.sections.as_ptr()
        ));
        assert!(std::ptr::eq(metadata.links, &model.links));
        assert!(std::ptr::eq(metadata.click_events, &model.click_events));
        let mut tasks = view.tasks();
        assert_eq!(tasks.len(), 1);
        let task = tasks.next().unwrap();
        assert!(std::ptr::eq(task.semantic, &model.tasks[0]));
        assert_eq!(task.render_end_ms, Some(23));
        assert_eq!(tasks.len(), 0);
        assert!(tasks.next().is_none());
        assert!(tasks.next().is_none());
        assert!(view.task(1).is_none());
    }

    #[cfg(feature = "presentation")]
    #[test]
    fn full_terminal_view_retains_detached_visible_end_override() {
        let mut full = GanttDiagramRenderModel::default();
        full.tasks.push(GanttRenderTask {
            semantic: GanttTask {
                render_end_ms: Some(23),
                ..Default::default()
            },
            render_end_ms: Some(42),
        });
        let view = GanttTerminalView::from_render(&full);
        let task = view.tasks().next().unwrap();
        assert!(std::ptr::eq(task.semantic, &full.tasks[0].semantic));
        assert_eq!(task.render_end_ms, Some(42));
        assert_eq!(task.semantic.render_end_ms, Some(23));
        assert!(std::ptr::eq(view.metadata().links, &full.model.links));
    }
}
