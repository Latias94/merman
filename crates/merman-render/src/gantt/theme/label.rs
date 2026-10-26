use merman_core::presentation::PresentationConfig;

use crate::diagram_theme::{
    ResolvedDiagramTheme, ResolvedStyleProperty, ThemeTarget, ThemeVariant,
};
use crate::family::{DirectStaticPaint, DirectStaticSelectorDomain, resolve_direct_static_fill};
use crate::resources::OperationWorkMeter;

use super::super::task_bar::GanttTaskBarState;

/// Prepared paint candidates for the real inside/outside Gantt label terminals.
/// Explicit Mermaid variables own their corresponding final CSS role independently.
#[derive(Debug, Clone, Default)]
pub(super) struct GanttTaskLabelPlan {
    inside: Option<DirectStaticPaint>,
    outside: Option<DirectStaticPaint>,
    state: Option<GanttTaskBarState>,
    source_owned: GanttTaskLabelSourceOwners,
    winners: Vec<(usize, ResolvedStyleProperty)>,
}

#[derive(Debug, Clone, Default)]
struct GanttTaskLabelSourceOwners {
    ordinary: bool,
    dark: bool,
    outside: bool,
    clickable: bool,
    vertical: bool,
}

impl GanttTaskLabelPlan {
    pub(super) fn resolve(
        theme: &ResolvedDiagramTheme,
        config: &PresentationConfig,
        state: GanttTaskBarState,
        ordinal: usize,
        meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let outside_style = theme.style_with_work_meter(
            ThemeTarget::Text,
            ThemeVariant::Default,
            Some(ordinal),
            meter,
        )?;
        let mut inside_style = outside_style.clone();
        let role = theme.style_with_work_meter(
            ThemeTarget::TaskLabel,
            state.theme_variant(),
            Some(ordinal),
            meter,
        )?;
        inside_style.merge_from(&role);
        let selector = if inside_style
            .fill_resolution()
            .winner()
            .is_some_and(|origin| origin.target() == ThemeTarget::Text)
        {
            DirectStaticSelectorDomain::Default
        } else {
            DirectStaticSelectorDomain::State(state.theme_variant())
        };
        let inside = resolve_direct_static_fill(
            theme,
            &inside_style,
            &[ThemeTarget::Text, ThemeTarget::TaskLabel],
            selector,
        );
        // Outside labels rest on the diagram canvas, independently of the task fill.
        let outside = resolve_direct_static_fill(
            theme,
            &outside_style,
            &[ThemeTarget::Text],
            DirectStaticSelectorDomain::Default,
        );
        let winners = inside_style
            .winner_rule_properties()
            .chain(outside_style.winner_rule_properties())
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect();
        let owns =
            |path| merman_core::__private::config_paint_overrides_typed_default(config, path);
        let source_owned = GanttTaskLabelSourceOwners {
            ordinary: owns("themeVariables.taskTextColor"),
            dark: owns("themeVariables.taskTextDarkColor"),
            outside: owns("themeVariables.taskTextOutsideColor"),
            clickable: owns("themeVariables.taskTextClickableColor"),
            vertical: owns("themeVariables.vertLineColor"),
        };
        Ok(Self {
            inside,
            outside,
            state: Some(state),
            source_owned,
            winners,
        })
    }

    pub(super) fn winners(&self) -> impl Iterator<Item = (usize, ResolvedStyleProperty)> + '_ {
        self.winners.iter().copied()
    }

    pub(super) fn typed_candidates(&self) -> impl Iterator<Item = &DirectStaticPaint> {
        self.inside.iter().chain(self.outside.iter())
    }

    pub(super) fn paint(&self, class: &str) -> Option<&DirectStaticPaint> {
        self.state?;
        let has = |value: &str| class.split_ascii_whitespace().any(|token| token == value);
        let outside = has("taskTextOutsideLeft") || has("taskTextOutsideRight");
        let has_state = |prefix: &str| {
            class
                .split_ascii_whitespace()
                .any(|token| token.starts_with(prefix))
        };
        let source_owned = if outside && (has_state("doneText") || has_state("doneCritText")) {
            self.source_owned.outside
        } else if has("clickable") {
            self.source_owned.clickable
        } else if has_state("activeCritText") {
            self.source_owned.dark
        } else if has("vertText") {
            self.source_owned.vertical
        } else if has_state("activeText") || has_state("doneText") || has_state("doneCritText") {
            self.source_owned.dark
        } else if outside {
            self.source_owned.outside
        } else {
            self.source_owned.ordinary
        };
        if source_owned {
            return None;
        }
        if outside {
            self.outside.as_ref()
        } else {
            self.inside.as_ref()
        }
    }
}
