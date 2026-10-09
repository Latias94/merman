use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, ResolvedThemeStyle, ThemeTarget, ThemeVariant,
};
use crate::resources::OperationWorkMeter;
use merman_core::MermaidConfig;

use super::SequenceNumberLabelThemeReceipt;

mod actor;
mod control;
mod lines;
mod rect;
pub(crate) use actor::SequencePreparedActorTheme;
pub(crate) use control::SequencePreparedControlTheme;
pub(crate) use lines::{SequencePreparedLifelineTheme, SequencePreparedMessageTheme};
pub(crate) use rect::SequencePreparedStaticRectTheme;

#[derive(Debug)]
pub(crate) struct SequencePreparedTerminalTheme {
    pub(crate) actor: SequencePreparedActorTheme,
    pub(crate) lifeline: SequencePreparedLifelineTheme,
    pub(crate) message: SequencePreparedMessageTheme,
    pub(crate) keyword: SequencePreparedControlTheme,
    pub(crate) frame: SequencePreparedControlTheme,
    pub(crate) note: SequencePreparedStaticRectTheme,
    pub(crate) number: SequencePreparedNumberTheme,
    pub(crate) note_count: usize,
}

impl SequencePreparedTerminalTheme {
    pub(super) fn resolve(
        model: &merman_core::diagrams::sequence::SequenceDiagramRenderModel,
        config: &MermaidConfig,
        theme: Option<&ResolvedDiagramTheme>,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let owns = |path| merman_core::__private::config_path_overrides_typed_default(config, path);
        let mut note_count = 0usize;
        let mut has_lines = false;
        for message in &model.messages {
            work_meter.charge(1)?;
            let kind = message.semantic_kind();
            note_count +=
                usize::from(kind == merman_core::diagrams::sequence::SequenceMessageKind::Note);
            has_lines |= kind == merman_core::diagrams::sequence::SequenceMessageKind::Signal
                && message.from.is_some()
                && message.to.is_some();
        }
        Ok(Self {
            actor: SequencePreparedActorTheme::resolve(theme, config, model, work_meter)?,
            lifeline: SequencePreparedLifelineTheme::resolve(
                theme,
                !model.actor_order.is_empty(),
                owns("themeVariables.actorLineColor"),
                work_meter,
            )?,
            message: SequencePreparedMessageTheme::resolve(
                theme,
                has_lines,
                owns("themeVariables.signalColor"),
                work_meter,
            )?,
            keyword: SequencePreparedControlTheme::resolve(
                theme,
                ThemeTarget::LoopLabelBackground,
                owns("themeVariables.labelBoxBkgColor"),
                owns("themeVariables.labelBoxBorderColor"),
                work_meter,
            )?,
            frame: SequencePreparedControlTheme::resolve(
                theme,
                ThemeTarget::Loop,
                true,
                owns("themeVariables.labelBoxBorderColor"),
                work_meter,
            )?,
            note: SequencePreparedStaticRectTheme::resolve(
                theme,
                ThemeTarget::Note,
                note_count != 0,
                owns("themeVariables.noteBkgColor"),
                owns("themeVariables.noteBorderColor"),
                work_meter,
            )?,
            number: SequencePreparedNumberTheme::resolve(theme, config, work_meter)?,
            note_count,
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct SequencePreparedNumberTheme {
    typed_fill: Option<String>,
    fill_overridden: bool,
    selected_style: Option<ResolvedThemeStyle>,
}

impl SequencePreparedNumberTheme {
    pub(super) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &MermaidConfig,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let mut prepared = Self {
            fill_overridden: merman_core::__private::config_path_overrides_typed_default(
                config,
                "themeVariables.sequenceNumberColor",
            ),
            ..Self::default()
        };
        let Some(theme) = theme else {
            return Ok(prepared);
        };
        if !theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::SequenceNumberLabel,
                    ..
                }
            )
        }) {
            return Ok(prepared);
        }
        let style = theme.style_with_work_meter(
            ThemeTarget::SequenceNumberLabel,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        if !prepared.fill_overridden {
            prepared.typed_fill = typed_static_sequence_fill(theme, &style);
        }
        prepared.selected_style = Some(style);
        Ok(prepared)
    }

    pub(crate) fn typed_fill(&self) -> Option<&str> {
        self.typed_fill.as_deref()
    }

    pub(crate) const fn fill_overridden(&self) -> bool {
        self.fill_overridden
    }

    pub(crate) fn fresh_receipt(&self) -> SequenceNumberLabelThemeReceipt {
        let mut receipt = SequenceNumberLabelThemeReceipt::default();
        if let Some(style) = &self.selected_style {
            receipt.record_static_style(style);
        }
        receipt
    }
}

pub(crate) fn typed_static_sequence_fill(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
) -> Option<String> {
    let origin = style.fill_resolution().winner()?;
    let facet = FamilyThemeRuleFacet::fill(style.fill_resolution().specified())?;
    if theme.rule_facet_disposition(origin.rule_index(), facet)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    css_paint(style.fill())
}

pub(crate) fn css_paint(paint: Option<&CanvasPaint>) -> Option<String> {
    match paint? {
        CanvasPaint::Transparent => Some("transparent".to_owned()),
        CanvasPaint::Solid(color) => Some(color.as_css()),
        CanvasPaint::LinearGradient(_)
        | CanvasPaint::RadialGradient(_)
        | CanvasPaint::Pattern(_) => None,
    }
}

pub(super) fn typed_static_sequence_stroke(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
) -> Option<String> {
    let origin = style.stroke_resolution().winner()?;
    let facet = FamilyThemeRuleFacet::stroke(style.stroke_resolution().specified())?;
    if theme.rule_facet_disposition(origin.rule_index(), facet)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    css_paint(style.stroke())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    };
    use crate::resources::RenderResourcePolicy;

    #[test]
    fn prepared_number_selection_keeps_explicit_override_and_winner() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::SequenceNumberLabel,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    )),
                ),
            )
            .unwrap();
        let resolved = theme.resolve(crate::DiagramFamilyId::SEQUENCE);
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let metadata = merman_core::Engine::new()
            .with_site_config(MermaidConfig::from_value(serde_json::json!({
                "themeVariables": {"sequenceNumberColor": "#abcdef"}
            })))
            .parse_metadata_sync("sequenceDiagram\nautonumber\nAlice->>Bob: Hello")
            .expect("parse explicit Sequence number color");
        let prepared = SequencePreparedNumberTheme::resolve(
            Some(&resolved),
            &metadata.effective_config,
            &meter,
        )
        .unwrap();
        assert!(prepared.fill_overridden());
        assert_eq!(prepared.typed_fill(), None);
        assert!(
            prepared
                .selected_style
                .as_ref()
                .unwrap()
                .fill_resolution()
                .winner()
                .is_some()
        );

        assert_eq!(
            prepared.selected_style.as_ref().unwrap().fill(),
            Some(&CanvasPaint::solid("#123456").unwrap())
        );

        let typed = SequencePreparedNumberTheme::resolve(
            Some(&resolved),
            &MermaidConfig::default(),
            &meter,
        )
        .unwrap();
        assert_eq!(typed.typed_fill(), Some("#123456"));
    }
}
