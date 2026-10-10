use merman_core::diagrams::zenuml::ZenumlDiagramRenderModel;

use super::{
    ZENUML_FRAME_TITLE_CLASS, ZENUML_FRAME_TITLE_SELECTOR, ZenumlDiagramLayout,
    resolve_zenuml_title,
};
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, ResolvedDiagramTheme, ThemeTarget,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, SingleTitleFillPlan,
    unsupported_residual_for_facet,
};
use crate::resources::OperationWorkMeter;

/// Final ZenUML frame-title fill shared by stylesheet emission and terminal evidence.
#[derive(Debug)]
pub(crate) struct ZenumlTitleThemePlan {
    fill: SingleTitleFillPlan,
}

impl ZenumlTitleThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        model: &ZenumlDiagramRenderModel,
        layout: &ZenumlDiagramLayout,
        diagram_title: Option<&str>,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let title_present = resolve_zenuml_title(model, diagram_title).is_some();
        let non_title_text_present = has_visible_non_title_text(layout);
        let mut evidence = FamilyThemeEvidence::from_theme(theme);
        if let Some(theme) = theme {
            for route in theme.family_mechanism_routes().iter().copied() {
                match route.mechanism() {
                    FamilyThemeMechanism::BaseTypography(_) => {
                        let key = theme.family_mechanism_key(route);
                        if !title_present && !non_title_text_present {
                            evidence.mark_not_applicable(key);
                        } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                            evidence.mark_residual(
                                key,
                                FamilyThemeResidualReason::UnsupportedTypography,
                            );
                        }
                    }
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::Text,
                        facet,
                        ..
                    } => {
                        let key = theme.family_mechanism_key(route);
                        if !non_title_text_present {
                            evidence.mark_not_applicable(key);
                        } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                            evidence.mark_residual(key, unsupported_residual_for_facet(facet));
                        }
                    }
                    FamilyThemeMechanism::OrdinalPalette {
                        target: ThemeTarget::Text,
                    } => {
                        let key = theme.family_mechanism_key(route);
                        if !non_title_text_present {
                            evidence.mark_not_applicable(key);
                        } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                            evidence.mark_residual(
                                key,
                                FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                            );
                        }
                    }
                    FamilyThemeMechanism::EffectBinding {
                        target: ThemeTarget::Text,
                        ..
                    } => {
                        let key = theme.family_mechanism_key(route);
                        if !non_title_text_present {
                            evidence.mark_not_applicable(key);
                        } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                            evidence
                                .mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                        }
                    }
                    FamilyThemeMechanism::RuleFacet { .. }
                    | FamilyThemeMechanism::OrdinalPalette { .. }
                    | FamilyThemeMechanism::EffectBinding { .. } => {}
                }
            }
        }
        let fill = SingleTitleFillPlan::resolve(theme, title_present, false, evidence, work_meter)?;
        Ok(Self { fill })
    }

    pub(crate) fn fill_css(&self) -> Option<&str> {
        self.fill.fill_css()
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<ZenumlTitleThemeReceipt> {
        self.fill
            .fill_css()
            .map(|fill_css| ZenumlTitleThemeReceipt::new(fill_css.into()))
    }

    pub(crate) fn record_terminal(&self, receipt: ZenumlTitleThemeReceipt) -> bool {
        self.fill
            .fill_css()
            .is_some_and(|fill_css| receipt.proves(fill_css) && self.fill.record_proven_terminal())
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        self.fill.finish_evidence()
    }
}

fn has_visible_non_title_text(layout: &ZenumlDiagramLayout) -> bool {
    layout
        .groups
        .iter()
        .any(|group| !group.name.trim().is_empty())
        || layout.participants.iter().any(|participant| {
            !participant.is_starter
                && (!participant.label.trim().is_empty()
                    || participant
                        .stereotype
                        .as_deref()
                        .is_some_and(|text| !text.trim().is_empty())
                    || participant
                        .emoji
                        .as_deref()
                        .is_some_and(|text| !text.trim().is_empty()))
        })
        || layout
            .messages
            .iter()
            .any(|message| !message.label.trim().is_empty() || !message.number.trim().is_empty())
        || layout
            .self_calls
            .iter()
            .any(|message| !message.label.trim().is_empty() || !message.number.trim().is_empty())
        || layout.creations.iter().any(|creation| {
            !creation.message.label.trim().is_empty() || !creation.message.number.trim().is_empty()
        })
        || layout.returns.iter().any(|returned| {
            !returned.label.trim().is_empty()
                || (!returned.is_self && !returned.number.trim().is_empty())
        })
        || !layout.fragments.is_empty()
        || layout
            .dividers
            .iter()
            .any(|divider| !divider.label.trim_matches('=').trim().is_empty())
        || layout
            .comments
            .iter()
            .any(|comment| !comment.text.trim().is_empty())
}

/// Writer-owned proof that Title.fill reached the stylesheet and exactly one `text.frame-title`.
#[derive(Debug)]
pub(crate) struct ZenumlTitleThemeReceipt {
    expected_fill: Box<str>,
    stylesheet_fill: Option<Box<str>>,
    stylesheet_selector: Option<Box<str>>,
    frame_title_text_count: usize,
    terminal_matches: bool,
}

impl ZenumlTitleThemeReceipt {
    fn new(expected_fill: Box<str>) -> Self {
        Self {
            expected_fill,
            stylesheet_fill: None,
            stylesheet_selector: None,
            frame_title_text_count: 0,
            terminal_matches: true,
        }
    }

    pub(crate) fn record_stylesheet_writer(&mut self, emitted_selector: &str, emitted_fill: &str) {
        if self.stylesheet_fill.is_some() || self.stylesheet_selector.is_some() {
            self.terminal_matches = false;
            return;
        }
        self.terminal_matches &= emitted_selector == ZENUML_FRAME_TITLE_SELECTOR;
        self.terminal_matches &= emitted_fill == self.expected_fill.as_ref();
        self.stylesheet_selector = Some(emitted_selector.into());
        self.stylesheet_fill = Some(emitted_fill.into());
    }

    pub(crate) fn record_frame_title_text_writer(
        &mut self,
        emitted_element: &str,
        emitted_class: &str,
        text: &str,
    ) {
        self.terminal_matches &= emitted_element == "text";
        self.terminal_matches &= emitted_class == ZENUML_FRAME_TITLE_CLASS;
        self.terminal_matches &= !text.is_empty();
        if !text.is_empty() {
            self.frame_title_text_count = self.frame_title_text_count.saturating_add(1);
        }
    }

    fn proves(&self, expected_fill: &str) -> bool {
        self.expected_fill.as_ref() == expected_fill
            && self.stylesheet_fill.as_deref() == Some(expected_fill)
            && self.stylesheet_selector.as_deref() == Some(ZENUML_FRAME_TITLE_SELECTOR)
            && self.frame_title_text_count == 1
            && self.terminal_matches
    }
}

#[cfg(test)]
mod tests {
    use super::ZenumlTitleThemeReceipt;

    #[test]
    fn title_receipt_requires_matching_css_and_one_text_frame_title_writer_event() {
        let mut complete = ZenumlTitleThemeReceipt::new("#123456".into());
        complete.record_stylesheet_writer(super::ZENUML_FRAME_TITLE_SELECTOR, "#123456");
        complete.record_frame_title_text_writer(
            "text",
            super::ZENUML_FRAME_TITLE_CLASS,
            "Themed ZenUML",
        );
        assert!(complete.proves("#123456"));

        let mut missing_text = ZenumlTitleThemeReceipt::new("#123456".into());
        missing_text.record_stylesheet_writer(super::ZENUML_FRAME_TITLE_SELECTOR, "#123456");
        assert!(!missing_text.proves("#123456"));

        let mut wrong_stylesheet = ZenumlTitleThemeReceipt::new("#123456".into());
        wrong_stylesheet.record_stylesheet_writer(super::ZENUML_FRAME_TITLE_SELECTOR, "#abcdef");
        wrong_stylesheet.record_frame_title_text_writer(
            "text",
            super::ZENUML_FRAME_TITLE_CLASS,
            "Themed ZenUML",
        );
        assert!(!wrong_stylesheet.proves("#123456"));

        let mut wrong_terminal = ZenumlTitleThemeReceipt::new("#123456".into());
        wrong_terminal.record_stylesheet_writer(super::ZENUML_FRAME_TITLE_SELECTOR, "#123456");
        wrong_terminal.record_frame_title_text_writer("text", "participant-label", "Not a title");
        assert!(!wrong_terminal.proves("#123456"));

        let mut duplicate_text = ZenumlTitleThemeReceipt::new("#123456".into());
        duplicate_text.record_stylesheet_writer(super::ZENUML_FRAME_TITLE_SELECTOR, "#123456");
        duplicate_text.record_frame_title_text_writer(
            "text",
            super::ZENUML_FRAME_TITLE_CLASS,
            "First",
        );
        duplicate_text.record_frame_title_text_writer(
            "text",
            super::ZENUML_FRAME_TITLE_CLASS,
            "Second",
        );
        assert!(!duplicate_text.proves("#123456"));
    }
}
