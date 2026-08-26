use std::sync::OnceLock;

use merman_core::MermaidConfig;

use super::RailroadStyle;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, unsupported_residual_for_facet,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RailroadTypographyOutcome {
    Inactive,
    Typed,
    ConfigOwned,
    Unsupported,
}

#[derive(Debug)]
struct RailroadUnsupportedRoute {
    key: FamilyThemeMechanismKey,
    target: ThemeTarget,
    selector: Option<FamilyThemeSelectorShape>,
    reason: FamilyThemeResidualReason,
}

impl RailroadUnsupportedRoute {
    fn is_applicable(&self, receipt: &RailroadSurfaceReceipt) -> bool {
        let occurrence_count = receipt.occurrence_count(self.target);
        self.selector.is_none_or(|selector| {
            selector.ordinal_domain_intersects_occurrence_count(occurrence_count)
        }) && occurrence_count != 0
    }
}

/// Writer-owned proof of the styled Railroad text surfaces emitted to terminal SVG.
#[derive(Debug, Default)]
pub(crate) struct RailroadSurfaceReceipt {
    root_svg_recorded: bool,
    root_svg_id: Option<Box<str>>,
    root_svg_class_verified: bool,
    terminal_text_count: usize,
    nonterminal_text_count: usize,
    comment_text_count: usize,
    special_text_count: usize,
    rule_name_count: usize,
    typography_stylesheet_recorded: bool,
    typography_stylesheet_verified: bool,
}

impl RailroadSurfaceReceipt {
    pub(crate) fn record_root_svg_open(&mut self, root_svg_open: &str) {
        if self.root_svg_recorded {
            self.root_svg_id = None;
            self.root_svg_class_verified = false;
            return;
        }
        self.root_svg_recorded = true;

        let Some(root_svg_open) = root_svg_open.strip_suffix('>') else {
            return;
        };
        if !root_svg_open.starts_with("<svg ") || root_svg_open[4..].contains('<') {
            return;
        }
        let mut root_document = String::with_capacity(root_svg_open.len().saturating_add(7));
        root_document.push_str(root_svg_open);
        root_document.push_str("></svg>");
        let Ok(document) = roxmltree::Document::parse(&root_document) else {
            return;
        };
        let root = document.root_element();
        if !root.has_tag_name(("http://www.w3.org/2000/svg", "svg"))
            || root.children().next().is_some()
        {
            return;
        }
        let Some(root_svg_id) = root.attribute("id").filter(|value| !value.is_empty()) else {
            return;
        };

        self.root_svg_id = Some(root_svg_id.into());
        self.root_svg_class_verified = root
            .attribute("class")
            .unwrap_or_default()
            .split_ascii_whitespace()
            .any(|candidate| candidate == "railroad-diagram");
    }

    pub(crate) fn record_typography_stylesheet(
        &mut self,
        stylesheet: &str,
        expected_font_family: &str,
        expected_font_size: &str,
    ) {
        if self.typography_stylesheet_recorded {
            self.typography_stylesheet_verified = false;
            return;
        }
        self.typography_stylesheet_recorded = true;
        let Some(root_svg_id) = self.root_svg_id.as_deref() else {
            self.typography_stylesheet_verified = false;
            return;
        };
        let diagram_id_css = crate::svg::escape_css_identifier(root_svg_id);
        let root_rule_matches = self.root_svg_class_verified
            && typography_rule_matches(
                stylesheet,
                &format!("#{diagram_id_css}.railroad-diagram"),
                expected_font_family,
                expected_font_size,
            );
        let descendant_rules_match = [
            ".railroad-terminal text",
            ".railroad-nonterminal text",
            ".railroad-comment text",
            ".railroad-special text",
            ".railroad-rule-name",
        ]
        .into_iter()
        .all(|role| {
            typography_rule_matches(
                stylesheet,
                &format!("#{diagram_id_css} {role}"),
                expected_font_family,
                expected_font_size,
            )
        });
        self.typography_stylesheet_verified = root_rule_matches && descendant_rules_match;
    }

    pub(crate) fn record_terminal_text(&mut self, text: &str) {
        if !text.trim().is_empty() {
            self.terminal_text_count = self.terminal_text_count.saturating_add(1);
        }
    }

    pub(crate) fn record_nonterminal_text(&mut self, text: &str) {
        if !text.trim().is_empty() {
            self.nonterminal_text_count = self.nonterminal_text_count.saturating_add(1);
        }
    }

    pub(crate) fn record_comment_text(&mut self, text: &str) {
        if !text.trim().is_empty() {
            self.comment_text_count = self.comment_text_count.saturating_add(1);
        }
    }

    pub(crate) fn record_special_text(&mut self, text: &str) {
        if !text.trim().is_empty() {
            self.special_text_count = self.special_text_count.saturating_add(1);
        }
    }

    pub(crate) fn record_rule_name_occurrence(&mut self) {
        self.rule_name_count = self.rule_name_count.saturating_add(1);
    }

    fn occurrence_count(&self, target: ThemeTarget) -> usize {
        match target {
            ThemeTarget::Text => self.styled_text_count(),
            ThemeTarget::Title => 0,
            _ => 0,
        }
    }

    fn styled_text_count(&self) -> usize {
        self.terminal_text_count
            .saturating_add(self.nonterminal_text_count)
            .saturating_add(self.comment_text_count)
            .saturating_add(self.special_text_count)
            .saturating_add(self.rule_name_count)
    }

    fn has_styled_text(&self) -> bool {
        self.styled_text_count() != 0
    }

    pub(crate) fn typography_stylesheet_verified(&self) -> bool {
        self.typography_stylesheet_recorded && self.typography_stylesheet_verified
    }
}

fn typography_rule_matches(
    stylesheet: &str,
    selector: &str,
    expected_font_family: &str,
    expected_font_size: &str,
) -> bool {
    let Some(body) = stylesheet_rule_body(stylesheet, selector) else {
        return false;
    };
    if !body.ends_with(';') {
        return false;
    }

    let mut font_family_count = 0;
    let mut font_size_count = 0;
    for declaration in body.split_terminator(';') {
        let Some((property, value)) = declaration.split_once(':') else {
            return false;
        };
        match property {
            "font-family" => {
                font_family_count += 1;
                if value != expected_font_family {
                    return false;
                }
            }
            "font-size" => {
                font_size_count += 1;
                if value.strip_suffix("px") != Some(expected_font_size) {
                    return false;
                }
            }
            _ => {}
        }
    }
    font_family_count == 1 && font_size_count == 1
}

fn stylesheet_rule_body<'a>(stylesheet: &'a str, selector: &str) -> Option<&'a str> {
    let rule_start = format!("{selector}{{");
    let body = stylesheet.split_once(&rule_start)?.1;
    if body.contains(&rule_start) {
        return None;
    }
    let body_end = body.find('}')?;
    Some(&body[..body_end])
}

/// Final Railroad style shared by layout measurement, SVG CSS, and terminal evidence.
#[derive(Debug)]
pub(crate) struct RailroadTypographyThemePlan {
    style: RailroadStyle,
    evidence: FamilyThemeEvidence,
    outcome: RailroadTypographyOutcome,
    unsupported_routes: Box<[RailroadUnsupportedRoute]>,
    terminal_receipt: OnceLock<RailroadSurfaceReceipt>,
}

impl RailroadTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        let mut style = super::railroad_style(effective_config.as_value());
        let Some(theme) = theme else {
            return Self {
                style,
                evidence: FamilyThemeEvidence::default(),
                outcome: RailroadTypographyOutcome::Inactive,
                unsupported_routes: Box::new([]),
                terminal_receipt: OnceLock::new(),
            };
        };

        let config_owns_font_stack = railroad_config_owns_font_stack(effective_config);
        let config_owns_font_size = railroad_config_owns_font_size(effective_config);
        let mut typed_font_stack = false;
        let mut typed_font_size = false;
        let mut unsupported_typography = false;
        let mut unsupported_routes = Vec::new();

        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                {
                    typed_font_stack = true;
                }
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                {
                    typed_font_size = true;
                }
                FamilyThemeMechanism::BaseTypography(_) => unsupported_typography = true,
                FamilyThemeMechanism::RuleFacet {
                    target,
                    selector,
                    facet,
                    ..
                } if route.disposition() == FamilyThemeDisposition::Unsupported => {
                    unsupported_routes.push(RailroadUnsupportedRoute {
                        key: theme.family_mechanism_key(route),
                        target,
                        selector: Some(selector),
                        reason: unsupported_residual_for_facet(facet),
                    });
                }
                FamilyThemeMechanism::RuleFacet { .. } => {}
                FamilyThemeMechanism::OrdinalPalette { target } => {
                    debug_assert_eq!(route.disposition(), FamilyThemeDisposition::Unsupported);
                    unsupported_routes.push(RailroadUnsupportedRoute {
                        key: theme.family_mechanism_key(route),
                        target,
                        selector: None,
                        reason: FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                    });
                }
                FamilyThemeMechanism::EffectBinding { target, .. } => {
                    debug_assert_eq!(route.disposition(), FamilyThemeDisposition::Unsupported);
                    unsupported_routes.push(RailroadUnsupportedRoute {
                        key: theme.family_mechanism_key(route),
                        target,
                        selector: None,
                        reason: FamilyThemeResidualReason::UnsupportedEffect,
                    });
                }
            }
        }

        let typed_font_stack_applied =
            typed_font_stack && !config_owns_font_stack && !unsupported_typography;
        let typed_font_size_applied =
            typed_font_size && !config_owns_font_size && !unsupported_typography;
        if typed_font_stack_applied {
            style.font_family = theme.typography().font_stack().as_css();
        }
        if typed_font_size_applied {
            style.font_size = f64::from(theme.typography().font_size_px());
        }

        let requested_typed_typography = typed_font_stack || typed_font_size;
        let applied_typed_typography = typed_font_stack_applied || typed_font_size_applied;
        let outcome = if unsupported_typography {
            RailroadTypographyOutcome::Unsupported
        } else if applied_typed_typography {
            RailroadTypographyOutcome::Typed
        } else if requested_typed_typography {
            RailroadTypographyOutcome::ConfigOwned
        } else {
            RailroadTypographyOutcome::Inactive
        };

        Self {
            style,
            evidence: FamilyThemeEvidence::from_theme(Some(theme)),
            outcome,
            unsupported_routes: unsupported_routes.into_boxed_slice(),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) const fn style(&self) -> &RailroadStyle {
        &self.style
    }

    pub(crate) fn begin_terminal_receipt(&self) -> RailroadSurfaceReceipt {
        RailroadSurfaceReceipt::default()
    }

    pub(crate) fn record_terminal(&self, receipt: RailroadSurfaceReceipt) -> bool {
        self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(receipt) = self.terminal_receipt.get() else {
            return evidence;
        };
        let key = FamilyThemeMechanismKey::Typography;
        if !receipt.has_styled_text() {
            evidence.mark_not_applicable(key);
            for route in &self.unsupported_routes {
                evidence.mark_not_applicable(route.key.clone());
            }
            return evidence;
        }

        match self.outcome {
            RailroadTypographyOutcome::Typed if receipt.typography_stylesheet_verified() => {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            }
            RailroadTypographyOutcome::Typed => {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
            RailroadTypographyOutcome::ConfigOwned => evidence.mark_not_applicable(key),
            RailroadTypographyOutcome::Unsupported => {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
            RailroadTypographyOutcome::Inactive => {}
        }
        for route in &self.unsupported_routes {
            if route.is_applicable(receipt) {
                evidence.mark_residual(route.key.clone(), route.reason);
            } else {
                evidence.mark_not_applicable(route.key.clone());
            }
        }
        evidence
    }
}

fn railroad_config_owns_font_stack(config: &MermaidConfig) -> bool {
    [
        "railroad.fontFamily",
        "themeVariables.fontFamily",
        "fontFamily",
    ]
    .into_iter()
    .any(|path| merman_core::__private::config_path_overrides_typed_default(config, path))
}

fn railroad_config_owns_font_size(config: &MermaidConfig) -> bool {
    ["railroad.fontSize", "themeVariables.fontSize"]
        .into_iter()
        .any(|path| merman_core::__private::config_path_overrides_typed_default(config, path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
    };

    const VALID_ROOT_SVG_OPEN: &str = concat!(
        r#"<svg id="rr" width="100%" xmlns="http://www.w3.org/2000/svg" "#,
        r#"xmlns:xlink="http://www.w3.org/1999/xlink" class="railroad-diagram">"#,
    );
    const VALID_CSS: &str = concat!(
        "#rr.railroad-diagram{font-family:monospace;font-size:18px;}",
        "#rr .railroad-terminal text{fill:black;font-family:monospace;font-size:18px;text-anchor:middle;}",
        "#rr .railroad-nonterminal text{fill:black;font-family:monospace;font-size:18px;text-anchor:middle;}",
        "#rr .railroad-comment text{fill:black;font-style:italic;font-family:monospace;font-size:18px;text-anchor:middle;}",
        "#rr .railroad-special text{fill:black;font-family:monospace;font-size:18px;text-anchor:middle;}",
        "#rr .railroad-rule-name{font-weight:bold;fill:black;font-family:monospace;font-size:18px;}"
    );

    fn typed_railroad_plan() -> RailroadTypographyThemePlan {
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("monospace").expect("valid Railroad test font"))
            .with_font_size_px(18.0)
            .expect("valid Railroad test font size");
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::RAILROAD, typography),
            ))
            .expect("compile Railroad receipt theme");
        let resolved = theme.resolve(DiagramFamilyId::RAILROAD);
        RailroadTypographyThemePlan::resolve(
            Some(&resolved),
            &MermaidConfig::from_value(serde_json::json!({})),
        )
    }

    #[test]
    fn railroad_typography_receipt_requires_every_terminal_text_role_rule() {
        for invalid_css in [
            VALID_CSS.replace(
                ".railroad-terminal text{fill:black;font-family:monospace;",
                ".railroad-terminal text{fill:black;font-family:serif;",
            ),
            VALID_CSS.replace(
                ".railroad-nonterminal text{fill:black;font-family:monospace;",
                ".railroad-nonterminal text{fill:black;",
            ),
            VALID_CSS.replace(
                ".railroad-comment text{fill:black;font-style:italic;font-family:monospace;",
                ".railroad-comment text{fill:black;font-style:italic;font-family:serif;",
            ),
            VALID_CSS.replace(
                ".railroad-special text{fill:black;font-family:monospace;font-size:18px;",
                ".railroad-special text{fill:black;font-family:monospace;font-size:17px;",
            ),
            format!("{VALID_CSS}#rr.railroad-diagram{{font-family:serif;font-size:18px;}}"),
        ] {
            let mut receipt = RailroadSurfaceReceipt::default();
            receipt.record_root_svg_open(VALID_ROOT_SVG_OPEN);
            receipt.record_typography_stylesheet(&invalid_css, "monospace", "18");

            assert!(!receipt.typography_stylesheet_verified());
        }
    }

    #[test]
    fn railroad_typography_receipt_requires_the_selector_to_match_the_emitted_root_svg() {
        for (root_svg_open, stylesheet) in [
            (
                VALID_ROOT_SVG_OPEN,
                VALID_CSS.replacen("#rr.railroad-diagram", "#rr .railroad-diagram", 1),
            ),
            (
                r#"<svg id="other" xmlns="http://www.w3.org/2000/svg" class="railroad-diagram">"#,
                VALID_CSS.to_string(),
            ),
            (
                r#"<svg id="rr" xmlns="http://www.w3.org/2000/svg" class="other-family">"#,
                VALID_CSS.to_string(),
            ),
        ] {
            let mut receipt = RailroadSurfaceReceipt::default();
            receipt.record_root_svg_open(root_svg_open);
            receipt.record_typography_stylesheet(&stylesheet, "monospace", "18");

            assert!(!receipt.typography_stylesheet_verified());
        }
    }

    #[test]
    fn railroad_comment_occurrence_can_verify_typed_typography() {
        let plan = typed_railroad_plan();
        let mut receipt = plan.begin_terminal_receipt();
        receipt.record_root_svg_open(VALID_ROOT_SVG_OPEN);
        receipt.record_typography_stylesheet(VALID_CSS, "monospace", "18");
        receipt.record_comment_text("comment");
        assert!(plan.record_terminal(receipt));

        let evidence = plan.finish_evidence();
        assert_eq!(evidence.applied().len(), 1);
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn unverified_railroad_typography_stylesheet_is_a_strict_residual() {
        let plan = typed_railroad_plan();
        let mut receipt = plan.begin_terminal_receipt();
        let invalid_css = VALID_CSS.replace(
            ".railroad-comment text{fill:black;font-style:italic;font-family:monospace;",
            ".railroad-comment text{fill:black;font-style:italic;font-family:serif;",
        );
        receipt.record_root_svg_open(VALID_ROOT_SVG_OPEN);
        receipt.record_typography_stylesheet(&invalid_css, "monospace", "18");
        receipt.record_terminal_text("terminal");
        assert!(plan.record_terminal(receipt));

        let evidence = plan.finish_evidence();
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedTypography
        );
    }
}
