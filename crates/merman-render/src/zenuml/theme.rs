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

/// Prepared source compatibility values shared by ZenUML measurement and paint.
#[derive(Debug, Default)]
pub(crate) struct ZenumlSourceStyle {
    pub(crate) typography: super::ZenumlTypography,
    colors: std::collections::BTreeMap<&'static str, String>,
}

impl ZenumlSourceStyle {
    pub(crate) fn new(config: &serde_json::Value) -> Self {
        let Some(config) = crate::config::value_at(config, &["themeVariables", "zenuml"])
            .and_then(serde_json::Value::as_object)
        else {
            return Self::default();
        };
        let configured = |key: &str| {
            config
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        let colors = [
            "backgroundColor",
            "borderColor",
            "textColor",
            "lineColor",
            "actorBackgroundColor",
            "actorBorderColor",
            "actorTextColor",
            "noteBackgroundColor",
            "noteBorderColor",
            "noteTextColor",
            "groupBackgroundColor",
            "groupBorderColor",
            "groupTextColor",
            "activationBackgroundColor",
            "activationBorderColor",
            "mutedSurfaceColor",
            "fragmentTextColor",
            "subtleTextColor",
        ]
        .into_iter()
        .filter_map(|key| {
            let color = configured(key).or_else(|| {
                let inherited: &[&str] = match key {
                    "actorBackgroundColor"
                    | "noteBackgroundColor"
                    | "groupBackgroundColor"
                    | "mutedSurfaceColor" => &["surfaceColor"],
                    "actorTextColor" | "noteTextColor" | "groupTextColor" | "subtleTextColor"
                    | "fragmentTextColor" => &["textColor"],
                    "actorBorderColor"
                    | "noteBorderColor"
                    | "groupBorderColor"
                    | "activationBorderColor" => &["borderColor"],
                    "activationBackgroundColor" => &["mutedSurfaceColor", "surfaceColor"],
                    _ => &[],
                };
                inherited.iter().find_map(|key| configured(key))
            });
            color.map(|value| (key, value))
        })
        .collect();
        Self {
            typography: super::ZenumlTypography::from_config(config),
            colors,
        }
    }

    pub(crate) fn color<'a>(&'a self, key: &str, fallback: &'a str) -> &'a str {
        self.configured_color(key).unwrap_or(fallback)
    }

    pub(crate) fn configured_color(&self, key: &str) -> Option<&str> {
        self.colors.get(key).map(String::as_str)
    }
}

/// Final ZenUML frame-title fill shared by stylesheet emission and terminal evidence.
#[derive(Debug)]
pub(crate) struct ZenumlTitleThemePlan {
    fill: SingleTitleFillPlan,
    source_style: ZenumlSourceStyle,
}

impl ZenumlTitleThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        model: &ZenumlDiagramRenderModel,
        layout: &ZenumlDiagramLayout,
        diagram_title: Option<&str>,
        work_meter: &OperationWorkMeter,
        source_style: ZenumlSourceStyle,
    ) -> crate::Result<Self> {
        let Some(theme) = theme else {
            return Ok(Self {
                fill: SingleTitleFillPlan::default(),
                source_style,
            });
        };
        let title_present = resolve_zenuml_title(model, diagram_title).is_some();
        let non_title_text_present = has_visible_non_title_text(layout);
        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(_) => {
                    let key = theme.family_mechanism_key(route);
                    if !title_present && !non_title_text_present {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence
                            .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
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
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    }
                }
                FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }
        let fill = SingleTitleFillPlan::resolve(theme, title_present, false, evidence, work_meter)?;
        Ok(Self { fill, source_style })
    }

    pub(crate) fn source_style(&self) -> &ZenumlSourceStyle {
        &self.source_style
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
    fn source_colors_inherit_within_zenuml_and_keep_explicit_terminal_overrides() {
        let source = super::ZenumlSourceStyle::new(&serde_json::json!({
            "themeVariables": {"zenuml": {
                "textColor": "#123456", "surfaceColor": "#abcdef", "borderColor": "#765432"
            }}
        }));
        for key in [
            "actorTextColor",
            "noteTextColor",
            "groupTextColor",
            "fragmentTextColor",
            "subtleTextColor",
        ] {
            assert_eq!(source.configured_color(key), Some("#123456"), "{key}");
        }
        for key in [
            "actorBackgroundColor",
            "noteBackgroundColor",
            "groupBackgroundColor",
            "mutedSurfaceColor",
            "activationBackgroundColor",
        ] {
            assert_eq!(source.configured_color(key), Some("#abcdef"), "{key}");
        }
        for key in [
            "actorBorderColor",
            "noteBorderColor",
            "groupBorderColor",
            "activationBorderColor",
        ] {
            assert_eq!(source.configured_color(key), Some("#765432"), "{key}");
        }
        let overridden = super::ZenumlSourceStyle::new(&serde_json::json!({
            "themeVariables": {"zenuml": {
                "textColor": "#123456", "surfaceColor": "#abcdef", "borderColor": "#765432",
                "actorTextColor": "#111111", "noteTextColor": "#222222", "groupTextColor": "#333333",
                "actorBackgroundColor": "#444444", "actorBorderColor": "#555555", "mutedSurfaceColor": "#666666"
            }}
        }));
        for (key, color) in [
            ("actorTextColor", "#111111"),
            ("noteTextColor", "#222222"),
            ("groupTextColor", "#333333"),
            ("actorBackgroundColor", "#444444"),
            ("actorBorderColor", "#555555"),
            ("activationBackgroundColor", "#666666"),
        ] {
            assert_eq!(overridden.configured_color(key), Some(color), "{key}");
        }
        let unrelated = super::ZenumlSourceStyle::new(&serde_json::json!({
            "themeVariables": {"textColor": "#999999", "surfaceColor": "#999999", "borderColor": "#999999"}
        }));
        assert_eq!(unrelated.configured_color("actorTextColor"), None);
        assert_eq!(unrelated.configured_color("actorBackgroundColor"), None);
        assert_eq!(unrelated.configured_color("actorBorderColor"), None);

        for (muted, activation, expected) in [
            (serde_json::json!(""), serde_json::Value::Null, ""),
            (serde_json::json!(false), serde_json::Value::Null, "#abcdef"),
            (
                serde_json::json!("#123456"),
                serde_json::json!("#654321"),
                "#654321",
            ),
        ] {
            let source = super::ZenumlSourceStyle::new(&serde_json::json!({
                "themeVariables": {"zenuml": {
                    "surfaceColor": "#abcdef", "mutedSurfaceColor": muted,
                    "activationBackgroundColor": activation,
                    "actorBackgroundColor": "", "noteBackgroundColor": 7
                }}
            }));
            assert_eq!(
                source.configured_color("activationBackgroundColor"),
                Some(expected)
            );
            assert_eq!(source.configured_color("actorBackgroundColor"), Some(""));
            assert_eq!(
                source.configured_color("noteBackgroundColor"),
                Some("#abcdef")
            );
        }
    }

    #[test]
    fn source_style_defaults_without_a_zenuml_object_and_keeps_namespace_isolation() {
        for namespace in [
            serde_json::Value::Null,
            serde_json::json!(false),
            serde_json::json!(7),
            serde_json::json!("Inter"),
            serde_json::json!([{"fontFamily": "Inter", "textColor": "red"}]),
            serde_json::json!({}),
        ] {
            let source = super::ZenumlSourceStyle::new(&serde_json::json!({
                "fontFamily": "Root Font", "fontSize": 40,
                "themeVariables": {
                    "fontFamily": "Global Font", "fontSize": 32,
                    "textColor": "red", "surfaceColor": "blue", "zenuml": namespace
                }
            }));
            assert_eq!(source.typography.css_family(), "Helvetica,Verdana,serif");
            assert_eq!(source.typography.message_style().font_size, 16.0);
            assert_eq!(source.configured_color("actorTextColor"), None);
            assert_eq!(source.configured_color("actorBackgroundColor"), None);
        }
    }

    #[test]
    fn source_typography_keeps_authored_empty_family_and_existing_size_parsing() {
        for (family, size, expected_family, expected_size) in [
            (
                serde_json::json!("Inter, sans-serif"),
                serde_json::json!("24px"),
                Some("Inter, sans-serif"),
                21.0,
            ),
            (serde_json::json!(""), serde_json::json!(16), Some(""), 14.0),
            (serde_json::json!([]), serde_json::json!("0px"), None, 16.0),
        ] {
            let source = super::ZenumlSourceStyle::new(&serde_json::json!({
                "themeVariables": {"zenuml": {"fontFamily": family, "fontSize": size}}
            }));
            assert_eq!(source.typography.font_family.as_deref(), expected_family);
            assert_eq!(source.typography.message_style().font_size, expected_size);
        }
    }

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
