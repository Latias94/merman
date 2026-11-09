use merman_model::presentation::PresentationConfig;

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
    inside_winners: Vec<(usize, ResolvedStyleProperty)>,
    outside_winners: Vec<(usize, ResolvedStyleProperty)>,
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
        let winners = |style: &crate::diagram_theme::ResolvedThemeStyle| {
            style
                .winner_rule_properties()
                .map(|(property, origin)| (origin.rule_index(), property))
                .collect()
        };
        let inside_winners = winners(&inside_style);
        let outside_winners = winners(&outside_style);
        let owns = |path| config.config_paint_overrides_typed_default(path);
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
            inside_winners,
            outside_winners,
        })
    }

    pub(super) fn winners(&self) -> impl Iterator<Item = (usize, ResolvedStyleProperty)> + '_ {
        self.inside_winners
            .iter()
            .chain(&self.outside_winners)
            .copied()
    }

    pub(super) fn typography_winner_rules(&self) -> impl Iterator<Item = usize> + '_ {
        self.winners().filter_map(|(rule_index, property)| {
            matches!(property, ResolvedStyleProperty::Typography(_)).then_some(rule_index)
        })
    }

    pub(super) fn terminal_typography_winner_rules(
        &self,
        class: &str,
    ) -> impl Iterator<Item = usize> + '_ {
        let outside = class
            .split_ascii_whitespace()
            .any(|token| matches!(token, "taskTextOutsideLeft" | "taskTextOutsideRight"));
        let winners = if outside {
            &self.outside_winners
        } else {
            &self.inside_winners
        };
        winners.iter().filter_map(|(rule_index, property)| {
            matches!(property, ResolvedStyleProperty::Typography(_)).then_some(*rule_index)
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, Specified, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    };

    #[test]
    fn typography_receipts_follow_the_final_inside_or_outside_label_role() {
        let mut generic = ThemeStylePatch::default();
        generic.typography.font_size_px = Specified::Value(31.0);
        let mut task = ThemeStylePatch::default();
        task.typography.font_size_px = Specified::Value(40.0);
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(ThemeTarget::Text, generic))
                        .with_rule(ThemeRule::new(ThemeTarget::TaskLabel, task)),
                ),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::GANTT);
        let plan = GanttTaskLabelPlan::resolve(
            &theme,
            &PresentationConfig::empty_object(),
            GanttTaskBarState::Default,
            1,
            &OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            ),
        )
        .unwrap();
        for (class, expected) in [
            ("taskText taskText0", 1),
            ("taskTextOutsideLeft taskTextOutside0", 0),
            ("taskTextOutsideRight taskTextOutside0", 0),
        ] {
            assert_eq!(
                plan.terminal_typography_winner_rules(class)
                    .collect::<Vec<_>>(),
                vec![expected],
                "{class}"
            );
        }
    }
}
