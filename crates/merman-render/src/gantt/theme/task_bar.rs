use merman_core::diagrams::gantt::GanttRenderTask;

use crate::diagram_theme::ThemeVariant;

/// Canonical terminal state for one Gantt task bar.
///
/// Mermaid gives `active` precedence over `done`, then combines the selected state with `crit`.
/// Milestone and vertical-marker geometry do not alter this paint state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GanttTaskBarState {
    Default,
    Active,
    Done,
    Crit,
    ActiveCrit,
    DoneCrit,
}

impl GanttTaskBarState {
    pub(super) const fn from_task(task: &GanttRenderTask) -> Self {
        if task.active {
            if task.crit {
                Self::ActiveCrit
            } else {
                Self::Active
            }
        } else if task.done {
            if task.crit {
                Self::DoneCrit
            } else {
                Self::Done
            }
        } else if task.crit {
            Self::Crit
        } else {
            Self::Default
        }
    }

    pub(super) const fn theme_variant(self) -> ThemeVariant {
        match self {
            Self::Default => ThemeVariant::Default,
            Self::Active | Self::ActiveCrit => ThemeVariant::Active,
            Self::Done | Self::DoneCrit => ThemeVariant::Success,
            Self::Crit => ThemeVariant::Error,
        }
    }

    pub(super) const fn bar_class_prefix(self) -> &'static str {
        match self {
            Self::Default => "task",
            Self::Active => "active",
            Self::Done => "done",
            Self::Crit => "crit",
            Self::ActiveCrit => "activeCrit",
            Self::DoneCrit => "doneCrit",
        }
    }

    pub(super) const fn final_fill_path(self) -> &'static str {
        match self {
            Self::Default => "themeVariables.taskBkgColor",
            Self::Active | Self::ActiveCrit => "themeVariables.activeTaskBkgColor",
            Self::Done | Self::DoneCrit => "themeVariables.doneTaskBkgColor",
            Self::Crit => "themeVariables.critBkgColor",
        }
    }
}
