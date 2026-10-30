//! Canonical Gantt data, full output facts, and borrowed terminal views.

use std::collections::HashMap;

mod terminal;
pub use terminal::{
    GanttTerminalMetadata, GanttTerminalTaskIter, GanttTerminalTaskView, GanttTerminalView,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct GanttDiagramModel<Task = GanttTask> {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default, rename = "dateFormat")]
    pub date_format: String,
    #[serde(default, rename = "axisFormat")]
    pub axis_format: String,
    #[serde(default, rename = "tickInterval")]
    pub tick_interval: Option<String>,
    #[serde(default, rename = "todayMarker")]
    pub today_marker: String,
    #[serde(default)]
    pub includes: Vec<String>,
    #[serde(default)]
    pub excludes: Vec<String>,
    #[serde(default, rename = "inclusiveEndDates")]
    pub inclusive_end_dates: bool,
    #[serde(default, rename = "topAxis")]
    pub top_axis: bool,
    #[serde(default)]
    pub weekday: String,
    #[serde(default)]
    pub weekend: String,
    #[serde(default)]
    pub sections: Vec<String>,
    #[serde(default)]
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub links: HashMap<String, String>,
    #[serde(default, rename = "clickEvents")]
    pub click_events: HashMap<String, GanttClickEvent>,
}

#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct GanttDiagramRenderModel {
    #[serde(flatten)]
    pub model: GanttDiagramModel<GanttRenderTask>,
    #[serde(default, rename = "displayMode")]
    pub display_mode: String,
    #[serde(skip)]
    output_data: GanttOutputData,
}

#[cfg(feature = "presentation-data")]
impl serde::Serialize for GanttDiagramRenderModel {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut record = serializer.serialize_struct("GanttDiagramRenderModel", 18)?;
        record.serialize_field("title", &self.model.title)?;
        record.serialize_field("accTitle", &self.model.acc_title)?;
        record.serialize_field("accDescr", &self.model.acc_descr)?;
        record.serialize_field("dateFormat", &self.model.date_format)?;
        record.serialize_field("axisFormat", &self.model.axis_format)?;
        record.serialize_field("tickInterval", &self.model.tick_interval)?;
        record.serialize_field("todayMarker", &self.model.today_marker)?;
        record.serialize_field("includes", &self.model.includes)?;
        record.serialize_field("excludes", &self.model.excludes)?;
        record.serialize_field("inclusiveEndDates", &self.model.inclusive_end_dates)?;
        record.serialize_field("displayMode", &self.display_mode)?;
        record.serialize_field("topAxis", &self.model.top_axis)?;
        record.serialize_field("weekday", &self.model.weekday)?;
        record.serialize_field("weekend", &self.model.weekend)?;
        record.serialize_field("sections", &self.model.sections)?;
        record.serialize_field("tasks", &self.model.tasks)?;
        record.serialize_field("links", &self.model.links)?;
        record.serialize_field("clickEvents", &self.model.click_events)?;
        record.end()
    }
}

#[cfg(feature = "presentation-data")]
impl std::ops::Deref for GanttDiagramRenderModel {
    type Target = GanttDiagramModel<GanttRenderTask>;
    fn deref(&self) -> &Self::Target {
        &self.model
    }
}

#[cfg(feature = "presentation-data")]
impl std::ops::DerefMut for GanttDiagramRenderModel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.model
    }
}

#[cfg(feature = "presentation-data")]
impl GanttDiagramRenderModel {
    /// Constructs full data with an explicitly declared compatibility output shape.
    ///
    /// Producers must retain this fact even when no JSON projection is requested. The declaration
    /// does not authenticate source admission; a manually built or deserialized model defaults to
    /// `GanttOutputData::Model`.
    pub fn from_output_data(
        model: GanttDiagramModel<GanttRenderTask>,
        display_mode: String,
        output_data: GanttOutputData,
    ) -> Self {
        Self {
            model,
            display_mode,
            output_data,
        }
    }

    /// Returns the declared compatibility output shape, independently of serializer selection.
    pub fn output_data(&self) -> GanttOutputData {
        self.output_data
    }
}

#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct GanttRenderTask {
    #[serde(flatten)]
    pub semantic: GanttTask,
    #[serde(default, rename = "renderEndTime")]
    pub render_end_ms: Option<i64>,
}

#[cfg(feature = "presentation-data")]
impl serde::Serialize for GanttRenderTask {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut record = serializer.serialize_struct("GanttRenderTask", 21)?;
        record.serialize_field("id", &self.semantic.id)?;
        record.serialize_field("task", &self.semantic.task)?;
        record.serialize_field("section", &self.semantic.section)?;
        record.serialize_field("sectionIndex", &self.semantic.section_index)?;
        record.serialize_field("type", &self.semantic.task_type)?;
        record.serialize_field("classes", &self.semantic.classes)?;
        record.serialize_field("active", &self.semantic.active)?;
        record.serialize_field("done", &self.semantic.done)?;
        record.serialize_field("crit", &self.semantic.crit)?;
        record.serialize_field("milestone", &self.semantic.milestone)?;
        record.serialize_field("vert", &self.semantic.vert)?;
        record.serialize_field("order", &self.semantic.order)?;
        record.serialize_field("prevTaskId", &self.semantic.prev_task_id)?;
        record.serialize_field("startConstraint", &self.semantic.start_constraint)?;
        record.serialize_field("endConstraint", &self.semantic.end_constraint)?;
        record.serialize_field("processed", &self.semantic.processed)?;
        record.serialize_field("manualEndTime", &self.semantic.manual_end_time)?;
        record.serialize_field("raw", &self.semantic.raw)?;
        record.serialize_field("startTime", &self.semantic.start_ms)?;
        record.serialize_field("endTime", &self.semantic.end_ms)?;
        record.serialize_field("renderEndTime", &self.render_end_ms)?;
        record.end()
    }
}

#[cfg(feature = "presentation-data")]
impl std::ops::Deref for GanttRenderTask {
    type Target = GanttTask;
    fn deref(&self) -> &Self::Target {
        &self.semantic
    }
}

#[cfg(feature = "presentation-data")]
impl std::ops::DerefMut for GanttRenderTask {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.semantic
    }
}

/// The compatibility output shape retained by a full Gantt producer.
///
/// Empty input and a header-only diagram can have identical default task data while requiring
/// different compatibility output. Keep this declaration with presentation data, rather than
/// deriving it from serialized values or gating its capture on a JSON feature.
#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GanttOutputData {
    Empty,
    #[default]
    Model,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct GanttTask {
    pub id: String,
    pub task: String,
    pub section: String,
    #[serde(default, rename = "sectionIndex")]
    pub section_index: Option<usize>,
    #[serde(rename = "type")]
    pub task_type: String,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub crit: bool,
    #[serde(default)]
    pub milestone: bool,
    #[serde(default)]
    pub vert: bool,
    #[serde(default)]
    pub order: i64,
    #[serde(default, rename = "prevTaskId")]
    pub prev_task_id: Option<String>,
    #[serde(rename = "startConstraint")]
    pub start_constraint: GanttTaskStartConstraint,
    #[serde(rename = "endConstraint")]
    pub end_constraint: GanttTaskEndConstraint,
    #[serde(default)]
    pub processed: bool,
    #[serde(default, rename = "manualEndTime")]
    pub manual_end_time: bool,
    #[serde(default)]
    pub raw: GanttTaskRaw,
    #[serde(rename = "startTime")]
    pub start_ms: i64,
    #[serde(rename = "endTime")]
    pub end_ms: i64,
    /// The visible end retained by the same excludes-adjustment pass as `end_ms`.
    #[serde(skip)]
    pub render_end_ms: Option<i64>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct GanttTaskRaw {
    pub data: String,
    #[serde(rename = "startTime")]
    pub start_time: GanttTaskStart,
    #[serde(rename = "endTime")]
    pub end_time: GanttTaskEnd,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum GanttTaskStart {
    #[serde(rename = "prevTaskEnd")]
    PrevTaskEnd { id: Option<String> },
    #[serde(rename = "getStartDate")]
    GetStartDate {
        #[serde(rename = "startData")]
        start_data: String,
    },
}

impl Default for GanttTaskStart {
    fn default() -> Self {
        Self::PrevTaskEnd { id: None }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct GanttTaskEnd {
    pub data: String,
}

/// The authored rule that determines a task's start instant.
///
/// This is independent of `GanttTask::prev_task_id`, which remains part of Mermaid's
/// compatibility model but is not sufficient to represent explicit multi-task dependencies.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GanttTaskStartConstraint {
    PreviousTaskEnd {
        #[serde(
            default,
            rename = "dependencyId",
            skip_serializing_if = "Option::is_none"
        )]
        dependency_id: Option<String>,
    },
    Fixed {
        value: String,
    },
    After {
        #[serde(rename = "dependencyIds")]
        dependency_ids: Vec<String>,
    },
}

impl Default for GanttTaskStartConstraint {
    fn default() -> Self {
        Self::PreviousTaskEnd {
            dependency_id: None,
        }
    }
}

/// The authored rule that determines a task's end instant.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GanttTaskEndConstraint {
    #[default]
    Unspecified,
    Fixed {
        value: String,
    },
    Duration {
        value: String,
    },
    Until {
        #[serde(rename = "dependencyIds")]
        dependency_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GanttClickEvent {
    pub function_name: String,
    pub function_args: Vec<String>,
    pub raw_function_args: Option<String>,
}

#[cfg(test)]
mod tests;
