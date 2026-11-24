use merman_model::families::gantt::GanttRenderTask;

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
        if task.semantic.active {
            if task.semantic.crit {
                Self::ActiveCrit
            } else {
                Self::Active
            }
        } else if task.semantic.done {
            if task.semantic.crit {
                Self::DoneCrit
            } else {
                Self::Done
            }
        } else if task.semantic.crit {
            Self::Crit
        } else {
            Self::Default
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

    pub(super) const fn is_critical(self) -> bool {
        matches!(self, Self::Crit | Self::ActiveCrit | Self::DoneCrit)
    }
}

// Mermaid labels retain every authored status class, unlike bars' active-first state.
pub(crate) fn label_state_classes(task: &GanttRenderTask, suffix: &str) -> String {
    let mut classes = Vec::new();
    if task.active {
        classes.push(format!(
            "{}{suffix}",
            if task.crit {
                "activeCritText"
            } else {
                "activeText"
            }
        ));
    }
    if task.done {
        classes.push(format!(
            "{}{suffix}",
            if task.crit {
                "doneCritText"
            } else {
                "doneText"
            }
        ));
    } else if task.crit {
        classes.push(format!("critText{suffix}"));
    }
    classes.join(" ")
}
