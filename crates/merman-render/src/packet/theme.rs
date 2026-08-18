use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, unsupported_residual_for_facet,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PacketTypographyOutcome {
    Inactive,
    TypedFontStack,
    ConfigOwned,
    Unsupported,
}

#[derive(Debug)]
struct PacketUnsupportedRoute {
    key: FamilyThemeMechanismKey,
    target: ThemeTarget,
    selector: Option<FamilyThemeSelectorShape>,
    reason: FamilyThemeResidualReason,
}

impl PacketUnsupportedRoute {
    fn is_applicable(&self, receipt: &PacketSurfaceReceipt) -> bool {
        let occurrence_count = receipt.occurrence_count(self.target);
        self.selector.is_none_or(|selector| {
            selector.ordinal_domain_intersects_occurrence_count(occurrence_count)
        }) && occurrence_count != 0
    }
}

/// Writer-owned count of the independently styled Packet text surfaces emitted to terminal SVG.
#[derive(Debug, Default)]
pub(crate) struct PacketSurfaceReceipt {
    text_count: usize,
    title_count: usize,
    typography_stylesheet_recorded: bool,
    typography_stylesheet_verified: bool,
}

impl PacketSurfaceReceipt {
    pub(crate) fn record_typography_stylesheet(
        &mut self,
        base_css: &str,
        role_css: &str,
        root_variable_css: &str,
        diagram_id_css: &str,
        expected_font_family: &str,
    ) {
        if self.typography_stylesheet_recorded {
            self.typography_stylesheet_verified = false;
            return;
        }
        self.typography_stylesheet_recorded = true;
        self.typography_stylesheet_verified =
            font_rule_matches(
                base_css,
                &format!("#{diagram_id_css}"),
                expected_font_family,
                ";font-size:16px;fill:#333;",
            ) && font_rule_matches(
                base_css,
                &format!("#{diagram_id_css} svg"),
                expected_font_family,
                ";font-size:16px;",
            ) && root_font_variable_matches(
                root_variable_css,
                diagram_id_css,
                expected_font_family,
            ) && packet_role_rules_inherit_font(role_css, diagram_id_css);
    }

    pub(crate) fn record_non_empty_text(&mut self, text: &str) {
        if !text.trim().is_empty() {
            self.record_text_occurrence();
        }
    }

    pub(crate) fn record_text_occurrence(&mut self) {
        self.text_count = self.text_count.saturating_add(1);
    }

    pub(crate) fn record_title_occurrence(&mut self) {
        self.title_count = self.title_count.saturating_add(1);
    }

    fn occurrence_count(&self, target: ThemeTarget) -> usize {
        match target {
            ThemeTarget::Text => self.text_count,
            ThemeTarget::Title => self.title_count,
            _ => 0,
        }
    }

    fn has_visible_text(&self) -> bool {
        self.text_count != 0 || self.title_count != 0
    }

    pub(crate) fn typography_stylesheet_verified(&self) -> bool {
        self.typography_stylesheet_recorded && self.typography_stylesheet_verified
    }
}

fn font_rule_matches(
    stylesheet: &str,
    selector: &str,
    expected_font_family: &str,
    expected_suffix: &str,
) -> bool {
    stylesheet_rule_body(stylesheet, selector)
        .and_then(|body| body.strip_prefix("font-family:"))
        .and_then(|body| body.strip_suffix(expected_suffix))
        == Some(expected_font_family)
}

fn root_font_variable_matches(
    stylesheet: &str,
    diagram_id_css: &str,
    expected_font_family: &str,
) -> bool {
    let selector = format!("#{diagram_id_css} :root");
    single_rule_body(stylesheet, &selector)
        .and_then(|body| body.strip_prefix("--mermaid-font-family:"))
        .and_then(|body| body.strip_suffix(';'))
        == Some(expected_font_family)
}

fn packet_role_rules_inherit_font(stylesheet: &str, diagram_id_css: &str) -> bool {
    const ROLE_RULES: [(&str, &[&str]); 6] = [
        (".packetByte", &["font-size"]),
        (".packetByte.start", &["fill"]),
        (".packetByte.end", &["fill"]),
        (".packetLabel", &["fill", "font-size"]),
        (".packetTitle", &["fill", "font-size"]),
        (".packetBlock", &["stroke", "stroke-width", "fill"]),
    ];

    let mut remaining = stylesheet;
    for (role, expected_properties) in ROLE_RULES {
        let selector = format!("#{diagram_id_css} {role}");
        let Some((body, rest)) = take_leading_rule(remaining, &selector) else {
            return false;
        };
        if !declarations_match(body, expected_properties) {
            return false;
        }
        remaining = rest;
    }
    remaining.is_empty()
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

fn single_rule_body<'a>(stylesheet: &'a str, selector: &str) -> Option<&'a str> {
    let (body, trailing) = take_leading_rule(stylesheet, selector)?;
    trailing.is_empty().then_some(body)
}

fn take_leading_rule<'a>(stylesheet: &'a str, selector: &str) -> Option<(&'a str, &'a str)> {
    let body = stylesheet.strip_prefix(selector)?.strip_prefix('{')?;
    let body_end = body.find('}')?;
    Some((&body[..body_end], &body[body_end + 1..]))
}

fn declarations_match(body: &str, expected_properties: &[&str]) -> bool {
    if !body.ends_with(';') {
        return false;
    }
    let mut declarations = body.split_terminator(';');
    for expected_property in expected_properties {
        let Some(declaration) = declarations.next() else {
            return false;
        };
        let Some((property, value)) = declaration.split_once(':') else {
            return false;
        };
        if property != *expected_property
            || value.trim().is_empty()
            || value
                .chars()
                .any(|character| matches!(character, ';' | '{' | '}'))
        {
            return false;
        }
    }
    declarations.next().is_none()
}

/// Final Packet base font shared by stylesheet emission and terminal evidence.
#[derive(Debug)]
pub(crate) struct PacketTypographyThemePlan {
    font_family_css: Box<str>,
    evidence: FamilyThemeEvidence,
    outcome: PacketTypographyOutcome,
    unsupported_routes: Box<[PacketUnsupportedRoute]>,
    terminal_receipt: OnceLock<PacketSurfaceReceipt>,
}

impl PacketTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        let configured_font = crate::config::config_font_family_css(effective_config.as_value());
        let Some(theme) = theme else {
            return Self {
                font_family_css: configured_font.into_boxed_str(),
                evidence: FamilyThemeEvidence::default(),
                outcome: PacketTypographyOutcome::Inactive,
                unsupported_routes: Box::new([]),
                terminal_receipt: OnceLock::new(),
            };
        };

        let config_owns_font_stack = packet_config_owns_font_stack(effective_config);
        let mut typed_font_stack = false;
        let mut unsupported_typography = false;
        let mut unsupported_routes = Vec::new();

        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                {
                    typed_font_stack = true;
                }
                FamilyThemeMechanism::BaseTypography(_) => unsupported_typography = true,
                FamilyThemeMechanism::RuleFacet {
                    target,
                    selector,
                    facet,
                    ..
                } => {
                    debug_assert_eq!(route.disposition(), FamilyThemeDisposition::Unsupported);
                    unsupported_routes.push(PacketUnsupportedRoute {
                        key: theme.family_mechanism_key(route),
                        target,
                        selector: Some(selector),
                        reason: unsupported_residual_for_facet(facet),
                    });
                }
                FamilyThemeMechanism::OrdinalPalette { target } => {
                    debug_assert_eq!(route.disposition(), FamilyThemeDisposition::Unsupported);
                    unsupported_routes.push(PacketUnsupportedRoute {
                        key: theme.family_mechanism_key(route),
                        target,
                        selector: None,
                        reason: FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                    });
                }
                FamilyThemeMechanism::EffectBinding { target, .. } => {
                    debug_assert_eq!(route.disposition(), FamilyThemeDisposition::Unsupported);
                    unsupported_routes.push(PacketUnsupportedRoute {
                        key: theme.family_mechanism_key(route),
                        target,
                        selector: None,
                        reason: FamilyThemeResidualReason::UnsupportedEffect,
                    });
                }
            }
        }

        let font_family_css = if typed_font_stack && !config_owns_font_stack {
            theme.typography().font_stack().as_css()
        } else {
            configured_font
        };
        let outcome = if unsupported_typography {
            PacketTypographyOutcome::Unsupported
        } else if typed_font_stack && config_owns_font_stack {
            PacketTypographyOutcome::ConfigOwned
        } else if typed_font_stack {
            PacketTypographyOutcome::TypedFontStack
        } else {
            PacketTypographyOutcome::Inactive
        };

        Self {
            font_family_css: font_family_css.into_boxed_str(),
            evidence: FamilyThemeEvidence::from_theme(Some(theme)),
            outcome,
            unsupported_routes: unsupported_routes.into_boxed_slice(),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        &self.font_family_css
    }

    pub(crate) fn begin_terminal_receipt(&self) -> PacketSurfaceReceipt {
        PacketSurfaceReceipt::default()
    }

    pub(crate) fn record_terminal(&self, receipt: PacketSurfaceReceipt) -> bool {
        self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(receipt) = self.terminal_receipt.get() else {
            return evidence;
        };
        let key = FamilyThemeMechanismKey::Typography;
        if !receipt.has_visible_text() {
            evidence.mark_not_applicable(key);
            for route in &self.unsupported_routes {
                evidence.mark_not_applicable(route.key.clone());
            }
            return evidence;
        }
        match self.outcome {
            PacketTypographyOutcome::TypedFontStack if receipt.typography_stylesheet_verified() => {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            }
            PacketTypographyOutcome::TypedFontStack => {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
            PacketTypographyOutcome::ConfigOwned => evidence.mark_not_applicable(key),
            PacketTypographyOutcome::Unsupported => {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
            PacketTypographyOutcome::Inactive => {}
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

fn packet_config_owns_font_stack(config: &MermaidConfig) -> bool {
    merman_core::__private::config_path_overrides_typed_default(config, "themeVariables.fontFamily")
        || merman_core::__private::config_path_overrides_typed_default(config, "fontFamily")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
    };

    const VALID_BASE_CSS: &str = concat!(
        "#pkt{font-family:monospace;font-size:16px;fill:#333;}",
        "#pkt svg{font-family:monospace;font-size:16px;}"
    );
    const VALID_ROLE_CSS: &str = concat!(
        "#pkt .packetByte{font-size:10px;}",
        "#pkt .packetByte.start{fill:black;}",
        "#pkt .packetByte.end{fill:black;}",
        "#pkt .packetLabel{fill:black;font-size:12px;}",
        "#pkt .packetTitle{fill:black;font-size:14px;}",
        "#pkt .packetBlock{stroke:black;stroke-width:1;fill:#efefef;}"
    );
    const VALID_ROOT_VARIABLE_CSS: &str = "#pkt :root{--mermaid-font-family:monospace;}";

    fn typed_packet_plan() -> PacketTypographyThemePlan {
        let typography = ThemeTextStyle::default().with_font_stack(
            FontStack::single("monospace").expect("valid Packet receipt font stack"),
        );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::PACKET, typography),
            ))
            .expect("compile Packet receipt theme");
        let resolved = theme.resolve(DiagramFamilyId::PACKET);
        PacketTypographyThemePlan::resolve(
            Some(&resolved),
            &MermaidConfig::from_value(serde_json::json!({})),
        )
    }

    #[test]
    fn packet_typography_receipt_requires_all_terminal_font_rules() {
        for (base_css, role_css, root_variable_css) in [
            (
                "#pkt{font-family:serif;font-size:16px;fill:#333;}#pkt svg{font-family:monospace;font-size:16px;}",
                VALID_ROLE_CSS,
                VALID_ROOT_VARIABLE_CSS,
            ),
            (
                "#pkt{font-family:monospace;font-size:16px;fill:#333;}#pkt svg{font-family:serif;font-size:16px;}",
                VALID_ROLE_CSS,
                VALID_ROOT_VARIABLE_CSS,
            ),
            (
                VALID_BASE_CSS,
                VALID_ROLE_CSS,
                "#pkt :root{--mermaid-font-family:serif;}",
            ),
            (
                concat!(
                    "#pkt{font-family:monospace;font-size:16px;fill:#333;}",
                    "#pkt svg{font-family:monospace;font-size:16px;}",
                    "#pkt{font-family:serif;font-size:16px;fill:#333;}"
                ),
                VALID_ROLE_CSS,
                VALID_ROOT_VARIABLE_CSS,
            ),
            (
                VALID_BASE_CSS,
                concat!(
                    "#pkt .packetByte{font-size:10px;}",
                    "#pkt .packetByte.start{fill:black;}",
                    "#pkt .packetByte.end{fill:black;}",
                    "#pkt .packetLabel{fill:red;font-family:serif;font-size:12px;}",
                    "#pkt .packetTitle{fill:black;font-size:14px;}",
                    "#pkt .packetBlock{stroke:black;stroke-width:1;fill:#efefef;}"
                ),
                VALID_ROOT_VARIABLE_CSS,
            ),
        ] {
            let mut receipt = PacketSurfaceReceipt::default();
            receipt.record_typography_stylesheet(
                base_css,
                role_css,
                root_variable_css,
                "pkt",
                "monospace",
            );

            assert!(!receipt.typography_stylesheet_verified());
        }
    }

    #[test]
    fn unverified_packet_typography_stylesheet_is_a_strict_residual() {
        let plan = typed_packet_plan();
        let mut receipt = plan.begin_terminal_receipt();
        receipt.record_typography_stylesheet(
            VALID_BASE_CSS,
            concat!(
                "#pkt .packetByte{font-size:10px;}",
                "#pkt .packetByte.start{fill:black;}",
                "#pkt .packetByte.end{fill:black;}",
                "#pkt .packetLabel{fill:red;font-family:serif;font-size:12px;}",
                "#pkt .packetTitle{fill:black;font-size:14px;}",
                "#pkt .packetBlock{stroke:black;stroke-width:1;fill:#efefef;}"
            ),
            VALID_ROOT_VARIABLE_CSS,
            "pkt",
            "monospace",
        );
        receipt.record_text_occurrence();
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
