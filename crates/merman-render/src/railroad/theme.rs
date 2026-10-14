use std::collections::BTreeSet;
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use super::RailroadStyle;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, ResolvedDiagramTheme, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    TerminalVariantDomain, UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolve_direct_static_fill, unsupported_residual_for_facet,
};

#[derive(Debug)]
struct RailroadUnsupportedRoute {
    key: FamilyThemeMechanismKey,
    target: ThemeTarget,
    reason: FamilyThemeResidualReason,
}

#[derive(Debug, Clone, Copy, Default)]
struct RailroadTextFillRoles {
    terminal: bool,
    nonterminal: bool,
    comment: bool,
    rule_name: bool,
}

impl RailroadTextFillRoles {
    fn has_visible_occurrence(self, receipt: &RailroadSurfaceReceipt) -> bool {
        (self.terminal && receipt.terminal_text_count != 0)
            || (self.nonterminal
                && (receipt.nonterminal_text_count != 0 || receipt.special_text_count != 0))
            || (self.comment && receipt.comment_text_count != 0)
            || (self.rule_name && receipt.rule_name_count != 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RailroadTextRole {
    Terminal,
    Nonterminal,
    Comment,
    Special,
    RuleName,
}

impl RailroadTextFillRoles {
    fn owns_role(self, role: RailroadTextRole) -> bool {
        match role {
            RailroadTextRole::Terminal => self.terminal,
            RailroadTextRole::Nonterminal | RailroadTextRole::Special => self.nonterminal,
            RailroadTextRole::Comment => self.comment,
            RailroadTextRole::RuleName => self.rule_name,
        }
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
    expected_text_roles: Option<Box<[RailroadTextRole]>>,
    next_text_role: usize,
    text_roles_match: bool,
    expected_title_fill: Option<Box<str>>,
    expected_terminal_text_fill: Option<Box<str>>,
    expected_nonterminal_text_fill: Option<Box<str>>,
    expected_comment_text_fill: Option<Box<str>>,
    expected_special_text_fill: Option<Box<str>>,
    expected_rule_name_text_fill: Option<Box<str>>,
    title_fill_stylesheet_verified: bool,
    title_fill_terminal_matches: bool,
    text_fill_stylesheet_verified: bool,
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
        let Ok(document) = crate::xml_dom::Document::parse(&root_document) else {
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

    pub(crate) fn record_typography_stylesheet<E>(
        &mut self,
        serialized_stylesheet: &str,
        expected_font_family: &str,
        expected_font_size: &str,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<(), E> {
        if self.typography_stylesheet_recorded {
            self.title_fill_stylesheet_verified = false;
            self.text_fill_stylesheet_verified = false;
            self.typography_stylesheet_verified = false;
            return Ok(());
        }
        self.typography_stylesheet_recorded = true;
        let Some(root_svg_id) = self.root_svg_id.as_deref() else {
            self.typography_stylesheet_verified = false;
            return Ok(());
        };
        // The writer slice contains terminal XML character data, not logical CSS. Decode once
        // before interpreting declarations so literal entity spellings remain font-name text.
        let Some(stylesheet) =
            crate::svg::decode_xml_style_text_with_checkpoints(serialized_stylesheet, checkpoint)?
        else {
            return Ok(());
        };
        let stylesheet = stylesheet.as_ref();
        let diagram_id_css = crate::svg::escape_css_identifier(root_svg_id);
        let root_rule_matches = self.root_svg_class_verified
            && stylesheet_rule_matches(
                stylesheet,
                &format!("#{diagram_id_css}.railroad-diagram"),
                expected_font_family,
                expected_font_size,
                None,
            );
        let title_typography_rule_matches = stylesheet_rule_matches(
            stylesheet,
            &format!("#{diagram_id_css} .railroad-rule-name"),
            expected_font_family,
            expected_font_size,
            None,
        );
        let descendant_typography_rules_match = [
            ".railroad-terminal text",
            ".railroad-nonterminal text",
            ".railroad-comment text",
            ".railroad-special text",
        ]
        .into_iter()
        .all(|role| {
            stylesheet_rule_matches(
                stylesheet,
                &format!("#{diagram_id_css} {role}"),
                expected_font_family,
                expected_font_size,
                None,
            )
        });
        let title_fill_stylesheet_verified =
            self.expected_title_fill
                .as_deref()
                .is_none_or(|expected_fill| {
                    stylesheet_fill_rule_matches(
                        stylesheet,
                        &format!("#{diagram_id_css} .railroad-rule-name"),
                        expected_fill,
                    )
                });
        let text_fill_stylesheet_verified = self
            .expected_rule_name_text_fill
            .as_deref()
            .is_none_or(|expected_fill| {
                stylesheet_fill_rule_matches(
                    stylesheet,
                    &format!("#{diagram_id_css} .railroad-rule-name"),
                    expected_fill,
                )
            })
            && [
                (
                    ".railroad-terminal text",
                    self.expected_terminal_text_fill.as_deref(),
                ),
                (
                    ".railroad-nonterminal text",
                    self.expected_nonterminal_text_fill.as_deref(),
                ),
                (
                    ".railroad-comment text",
                    self.expected_comment_text_fill.as_deref(),
                ),
                (
                    ".railroad-special text",
                    self.expected_special_text_fill.as_deref(),
                ),
            ]
            .into_iter()
            .all(|(role, expected_fill)| {
                expected_fill.is_none_or(|expected_fill| {
                    stylesheet_fill_rule_matches(
                        stylesheet,
                        &format!("#{diagram_id_css} {role}"),
                        expected_fill,
                    )
                })
            });
        checkpoint()?;
        self.title_fill_stylesheet_verified = title_fill_stylesheet_verified;
        self.text_fill_stylesheet_verified = text_fill_stylesheet_verified;
        self.typography_stylesheet_verified =
            root_rule_matches && title_typography_rule_matches && descendant_typography_rules_match;
        Ok(())
    }

    pub(crate) fn record_terminal_text(&mut self, text: &str) {
        if !text.trim().is_empty() {
            self.terminal_text_count = self.terminal_text_count.saturating_add(1);
            self.record_text_role(RailroadTextRole::Terminal);
        }
    }

    pub(crate) fn record_nonterminal_text(&mut self, text: &str) {
        if !text.trim().is_empty() {
            self.nonterminal_text_count = self.nonterminal_text_count.saturating_add(1);
            self.record_text_role(RailroadTextRole::Nonterminal);
        }
    }

    pub(crate) fn record_comment_text(&mut self, text: &str) {
        if !text.trim().is_empty() {
            self.comment_text_count = self.comment_text_count.saturating_add(1);
            self.record_text_role(RailroadTextRole::Comment);
        }
    }

    pub(crate) fn record_special_text(&mut self, text: &str) {
        if !text.trim().is_empty() {
            self.special_text_count = self.special_text_count.saturating_add(1);
            self.record_text_role(RailroadTextRole::Special);
        }
    }

    pub(crate) fn record_rule_name_occurrence(&mut self, emitted_fill: &str) {
        self.rule_name_count = self.rule_name_count.saturating_add(1);
        self.record_text_role(RailroadTextRole::RuleName);
        if let Some(expected_fill) = self
            .expected_title_fill
            .as_deref()
            .or(self.expected_rule_name_text_fill.as_deref())
        {
            self.title_fill_terminal_matches &= emitted_fill == expected_fill;
        }
    }

    fn record_text_role(&mut self, role: RailroadTextRole) {
        if let Some(expected) = &self.expected_text_roles {
            self.text_roles_match &= expected.get(self.next_text_role) == Some(&role);
            self.next_text_role = self.next_text_role.saturating_add(1);
        }
    }

    fn proves_terminal_domain(&self) -> bool {
        self.expected_text_roles
            .as_ref()
            .is_some_and(|roles| self.text_roles_match && self.next_text_role == roles.len())
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

    fn title_fill_proved(&self) -> bool {
        self.expected_title_fill.is_some()
            && self.rule_name_count != 0
            && self.title_fill_stylesheet_verified
            && self.title_fill_terminal_matches
    }

    fn text_fill_proved(&self, roles: RailroadTextFillRoles) -> bool {
        self.text_fill_stylesheet_verified
            && roles.has_visible_occurrence(self)
            && (!roles.rule_name || self.title_fill_terminal_matches)
    }
}

fn stylesheet_rule_matches(
    stylesheet: &str,
    selector: &str,
    expected_font_family: &str,
    expected_font_size: &str,
    expected_fill: Option<&str>,
) -> bool {
    let Some(body) = stylesheet_rule_body(stylesheet, selector) else {
        return false;
    };
    if !body.ends_with(';') {
        return false;
    }

    let mut font_family_count = 0usize;
    let mut font_size_count = 0usize;
    let mut fill_count = 0usize;
    let mut valid = true;
    let mut checkpoint = || Ok::<(), std::convert::Infallible>(());
    let _ = crate::mermaid_style::visit_style_declaration_boundaries_with_checkpoints(
        body,
        &mut checkpoint,
        |boundary| {
            let Some(declaration) = crate::mermaid_style::parse_style_declaration(boundary.raw())
            else {
                valid = false;
                return Ok(false);
            };
            let matches = match declaration.property() {
                "font-family" => {
                    font_family_count = font_family_count.saturating_add(1);
                    !declaration.important() && declaration.value() == expected_font_family
                }
                "font-size" => {
                    font_size_count = font_size_count.saturating_add(1);
                    !declaration.important()
                        && declaration.value().strip_suffix("px") == Some(expected_font_size)
                }
                "fill" if expected_fill.is_some() => {
                    fill_count = fill_count.saturating_add(1);
                    !declaration.important()
                        && expected_fill.is_some_and(|expected| declaration.value() == expected)
                }
                _ => true,
            };
            if !matches {
                valid = false;
            }
            Ok(matches)
        },
    );
    valid
        && font_family_count == 1
        && font_size_count == 1
        && expected_fill.is_none_or(|_| fill_count == 1)
}

fn stylesheet_fill_rule_matches(stylesheet: &str, selector: &str, expected_fill: &str) -> bool {
    let Some(body) = stylesheet_rule_body(stylesheet, selector) else {
        return false;
    };
    if !body.ends_with(';') {
        return false;
    }

    let mut fill_count = 0usize;
    let mut valid = true;
    let mut checkpoint = || Ok::<(), std::convert::Infallible>(());
    let _ = crate::mermaid_style::visit_style_declaration_boundaries_with_checkpoints(
        body,
        &mut checkpoint,
        |boundary| {
            let Some(declaration) = crate::mermaid_style::parse_style_declaration(boundary.raw())
            else {
                valid = false;
                return Ok(false);
            };
            if declaration.property() == "fill" {
                fill_count = fill_count.saturating_add(1);
                if declaration.important() || declaration.value() != expected_fill {
                    valid = false;
                }
            }
            Ok(valid)
        },
    );
    valid && fill_count == 1
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
    unsupported_properties: BTreeSet<ThemeTypographyProperty>,
    typed_font_stack_requested: bool,
    typed_font_size_requested: bool,
    typed_font_stack_active: bool,
    typed_font_size_active: bool,
    title_fill: Option<DirectStaticPaint>,
    title_fill_config_owned: bool,
    title_fill_routes: Box<[FamilyThemeMechanismKey]>,
    text_fill: Option<DirectStaticPaint>,
    text_fill_roles: RailroadTextFillRoles,
    text_fill_config_owned: bool,
    text_fill_winner_rule: Option<usize>,
    expected_text_roles: Option<Box<[RailroadTextRole]>>,
    unsupported_terminal_evidence: Option<FamilyThemeEvidence>,
    text_fill_routes: Box<[FamilyThemeMechanismKey]>,
    unsupported_routes: Box<[RailroadUnsupportedRoute]>,
    terminal_receipt: OnceLock<RailroadSurfaceReceipt>,
}

impl RailroadTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> Result<Self, crate::resources::OperationWorkError> {
        let mut style = super::railroad_style(effective_config.as_value());
        let Some(theme) = theme else {
            return Ok(Self {
                style,
                evidence: FamilyThemeEvidence::default(),
                unsupported_properties: BTreeSet::new(),
                typed_font_stack_requested: false,
                typed_font_size_requested: false,
                typed_font_stack_active: false,
                typed_font_size_active: false,
                title_fill: None,
                title_fill_config_owned: false,
                title_fill_routes: Box::new([]),
                text_fill: None,
                text_fill_roles: RailroadTextFillRoles::default(),
                text_fill_config_owned: false,
                text_fill_winner_rule: None,
                expected_text_roles: None,
                unsupported_terminal_evidence: None,
                text_fill_routes: Box::new([]),
                unsupported_routes: Box::new([]),
                terminal_receipt: OnceLock::new(),
            });
        };

        let config_owns_font_stack = railroad_config_owns_font_stack(effective_config);
        let config_owns_font_size = railroad_config_owns_font_size(effective_config);
        let title_fill_config_owned = railroad_config_owns_title_fill(effective_config);
        let text_fill_config_owned = railroad_config_owns_text_fill(effective_config);
        let mut typed_font_stack = false;
        let mut typed_font_size = false;
        let mut unsupported_properties = BTreeSet::new();
        let mut unsupported_routes = Vec::new();
        let mut title_fill_routes = BTreeSet::new();
        let mut text_fill_routes = BTreeSet::new();

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
                FamilyThemeMechanism::BaseTypography(property)
                    if route.disposition() == FamilyThemeDisposition::Unsupported =>
                {
                    unsupported_properties.insert(property);
                }
                FamilyThemeMechanism::BaseTypography(_) => {}
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Text,
                    selector: _,
                    facet:
                        FamilyThemeRuleFacet::Fill(
                            FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                        ),
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    match route.disposition() {
                        FamilyThemeDisposition::TypedAdapter => {
                            text_fill_routes.insert(key);
                        }
                        FamilyThemeDisposition::Unsupported => {
                            unsupported_routes.push(RailroadUnsupportedRoute {
                                key,
                                target: ThemeTarget::Text,
                                reason: FamilyThemeResidualReason::UnsupportedPaint,
                            });
                        }
                    }
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index: _,
                    target: ThemeTarget::Title,
                    selector: _,
                    facet:
                        FamilyThemeRuleFacet::Fill(
                            FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                        ),
                } => {
                    let key = theme.family_mechanism_key(route);
                    match route.disposition() {
                        FamilyThemeDisposition::TypedAdapter => {
                            title_fill_routes.insert(key);
                        }
                        FamilyThemeDisposition::Unsupported => {
                            unsupported_routes.push(RailroadUnsupportedRoute {
                                key,
                                target: ThemeTarget::Title,
                                reason: FamilyThemeResidualReason::UnsupportedPaint,
                            });
                        }
                    }
                }
                FamilyThemeMechanism::RuleFacet {
                    target,
                    selector: _,
                    facet,
                    ..
                } if route.disposition() == FamilyThemeDisposition::Unsupported => {
                    unsupported_routes.push(RailroadUnsupportedRoute {
                        key: theme.family_mechanism_key(route),
                        target,
                        reason: unsupported_residual_for_facet(facet),
                    });
                }
                FamilyThemeMechanism::RuleFacet { .. } => {}
                FamilyThemeMechanism::OrdinalPalette { target } => {
                    debug_assert_eq!(route.disposition(), FamilyThemeDisposition::Unsupported);
                    unsupported_routes.push(RailroadUnsupportedRoute {
                        key: theme.family_mechanism_key(route),
                        target,
                        reason: FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                    });
                }
                FamilyThemeMechanism::EffectBinding { target, .. } => {
                    debug_assert_eq!(route.disposition(), FamilyThemeDisposition::Unsupported);
                    unsupported_routes.push(RailroadUnsupportedRoute {
                        key: theme.family_mechanism_key(route),
                        target,
                        reason: FamilyThemeResidualReason::UnsupportedEffect,
                    });
                }
            }
        }

        let title_style = theme.style_with_work_meter(
            ThemeTarget::Title,
            ThemeVariant::Default,
            Some(1),
            work_meter,
        )?;
        let title_style_has_winner = title_style.fill_resolution().winner().is_some();
        let title_fill = if title_fill_config_owned || title_fill_routes.is_empty() {
            None
        } else {
            resolve_direct_static_fill(
                theme,
                &title_style,
                &[ThemeTarget::Title],
                DirectStaticSelectorDomain::Default,
            )
        };

        let text_style = theme.style_with_work_meter(
            ThemeTarget::Text,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let text_fill_winner_rule = text_style
            .fill_resolution()
            .winner()
            .map(|origin| origin.rule_index());
        let text_fill = if text_fill_routes.is_empty() {
            None
        } else {
            resolve_direct_static_fill(
                theme,
                &text_style,
                &[ThemeTarget::Text],
                DirectStaticSelectorDomain::Default,
            )
        };
        let text_fill_roles = RailroadTextFillRoles {
            terminal: !railroad_config_owns_terminal_text_fill(effective_config),
            nonterminal: !railroad_config_owns_nonterminal_text_fill(effective_config),
            comment: !railroad_config_owns_comment_text_fill(effective_config),
            rule_name: !title_style_has_winner
                && !railroad_config_owns_rule_name_text_fill(effective_config),
        };
        if let Some(title_fill) = title_fill.as_ref() {
            style.rule_name_color = title_fill.css().to_owned();
        }
        if let Some(text_fill) = text_fill.as_ref() {
            if text_fill_roles.terminal {
                style.terminal_text_color = text_fill.css().to_owned();
            }
            if text_fill_roles.nonterminal {
                style.non_terminal_text_color = text_fill.css().to_owned();
            }
            if text_fill_roles.comment {
                style.comment_text_color = text_fill.css().to_owned();
            }
            if text_fill_roles.rule_name {
                style.rule_name_color = text_fill.css().to_owned();
            }
        }

        let typed_font_stack_applied = typed_font_stack && !config_owns_font_stack;
        let typed_font_size_applied = typed_font_size && !config_owns_font_size;
        if typed_font_stack_applied {
            style.font_family = theme.typography().font_stack().as_css();
        }
        if typed_font_size_applied {
            style.font_size = f64::from(theme.typography().font_size_px());
        }

        Ok(Self {
            style,
            evidence: FamilyThemeEvidence::from_theme(Some(theme)),
            unsupported_properties,
            typed_font_stack_requested: typed_font_stack,
            typed_font_size_requested: typed_font_size,
            typed_font_stack_active: typed_font_stack_applied,
            typed_font_size_active: typed_font_size_applied,
            title_fill,
            title_fill_config_owned,
            title_fill_routes: title_fill_routes.into_iter().collect(),
            text_fill,
            text_fill_roles,
            text_fill_config_owned,
            text_fill_winner_rule,
            expected_text_roles: None,
            unsupported_terminal_evidence: None,
            text_fill_routes: text_fill_routes.into_iter().collect(),
            unsupported_routes: unsupported_routes.into_boxed_slice(),
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn prepare_terminal_domain(
        &mut self,
        theme: Option<&ResolvedDiagramTheme>,
        layout: &crate::model::RailroadDiagramLayout,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> Result<(), crate::resources::OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(());
        };
        if !self
            .unsupported_routes
            .iter()
            .any(|route| matches!(route.target, ThemeTarget::Text | ThemeTarget::Title))
        {
            return Ok(());
        }
        let mut roles = Vec::new();
        for rule in &layout.rules {
            work_meter.charge(1)?;
            for element in &rule.elements {
                work_meter.charge(1)?;
                if element.label.trim().is_empty() {
                    continue;
                }
                let role = match element.kind.as_str() {
                    "terminal" => RailroadTextRole::Terminal,
                    "nonterminal" => RailroadTextRole::Nonterminal,
                    "comment" => RailroadTextRole::Comment,
                    "special" => RailroadTextRole::Special,
                    _ => continue,
                };
                roles.push(role);
            }
            roles.push(RailroadTextRole::RuleName);
        }
        let text_owned_fill = roles
            .iter()
            .map(|role| !self.text_fill_roles.owns_role(*role))
            .collect::<Vec<_>>();
        let title_owned_fill = vec![self.title_fill_config_owned; layout.rules.len()];
        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[
                UnsupportedTerminalDomain::direct(
                    ThemeTarget::Text,
                    TerminalVariantDomain::uniform(roles.len(), ThemeVariant::Default),
                )
                .with_source_owned_fill(&text_owned_fill),
                UnsupportedTerminalDomain::direct(
                    ThemeTarget::Title,
                    TerminalVariantDomain::uniform(layout.rules.len(), ThemeVariant::Default),
                )
                .with_source_owned_fill(&title_owned_fill),
            ],
            work_meter,
        )?;
        self.expected_text_roles = Some(roles.into_boxed_slice());
        self.unsupported_terminal_evidence = Some(evidence);
        Ok(())
    }

    pub(crate) const fn style(&self) -> &RailroadStyle {
        &self.style
    }

    pub(crate) fn begin_terminal_receipt(&self) -> RailroadSurfaceReceipt {
        let text_fill_is_active = self.text_fill.is_some();
        RailroadSurfaceReceipt {
            expected_title_fill: self
                .title_fill
                .as_ref()
                .map(|paint| paint.css().to_owned().into_boxed_str()),
            expected_terminal_text_fill: (text_fill_is_active && self.text_fill_roles.terminal)
                .then(|| self.style.terminal_text_color.clone().into_boxed_str()),
            expected_nonterminal_text_fill: (text_fill_is_active
                && self.text_fill_roles.nonterminal)
                .then(|| self.style.non_terminal_text_color.clone().into_boxed_str()),
            expected_comment_text_fill: (text_fill_is_active && self.text_fill_roles.comment)
                .then(|| self.style.comment_text_color.clone().into_boxed_str()),
            expected_special_text_fill: (text_fill_is_active && self.text_fill_roles.nonterminal)
                .then(|| self.style.non_terminal_text_color.clone().into_boxed_str()),
            expected_rule_name_text_fill: (text_fill_is_active && self.text_fill_roles.rule_name)
                .then(|| self.style.rule_name_color.clone().into_boxed_str()),
            title_fill_terminal_matches: true,
            expected_text_roles: self.expected_text_roles.clone(),
            text_roles_match: true,
            ..RailroadSurfaceReceipt::default()
        }
    }

    pub(crate) fn record_terminal(&self, receipt: RailroadSurfaceReceipt) -> bool {
        self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(receipt) = self.terminal_receipt.get() else {
            return evidence;
        };
        if !receipt.has_styled_text() {
            for (property, requested) in [
                (
                    ThemeTypographyProperty::FontStack,
                    self.typed_font_stack_requested,
                ),
                (
                    ThemeTypographyProperty::FontSize,
                    self.typed_font_size_requested,
                ),
            ] {
                if requested {
                    evidence.mark_not_applicable(FamilyThemeMechanismKey::Typography(property));
                }
            }
            for property in &self.unsupported_properties {
                evidence.mark_not_applicable(FamilyThemeMechanismKey::Typography(*property));
            }
            for route in &self.unsupported_routes {
                evidence.mark_not_applicable(route.key.clone());
            }
            for key in &self.title_fill_routes {
                evidence.mark_not_applicable(key.clone());
            }
            for key in &self.text_fill_routes {
                evidence.mark_not_applicable(key.clone());
            }
            return evidence;
        }

        for (property, requested, active) in [
            (
                ThemeTypographyProperty::FontStack,
                self.typed_font_stack_requested,
                self.typed_font_stack_active,
            ),
            (
                ThemeTypographyProperty::FontSize,
                self.typed_font_size_requested,
                self.typed_font_size_active,
            ),
        ] {
            if !requested {
                continue;
            }
            let key = FamilyThemeMechanismKey::Typography(property);
            if !active {
                evidence.mark_not_applicable(key);
            } else if receipt.typography_stylesheet_verified() {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            } else {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
        }
        for property in &self.unsupported_properties {
            evidence.mark_residual(
                FamilyThemeMechanismKey::Typography(*property),
                FamilyThemeResidualReason::UnsupportedTypography,
            );
        }
        if receipt.proves_terminal_domain() {
            if let Some(terminal_evidence) = &self.unsupported_terminal_evidence {
                evidence.merge_accounted_from(terminal_evidence.clone());
            }
        } else {
            for route in &self.unsupported_routes {
                if matches!(route.target, ThemeTarget::Text | ThemeTarget::Title) {
                    evidence.mark_residual(route.key.clone(), route.reason);
                }
            }
        }
        for route in &self.unsupported_routes {
            if !matches!(route.target, ThemeTarget::Text | ThemeTarget::Title) {
                evidence.mark_not_applicable(route.key.clone());
            }
        }
        for key in &self.title_fill_routes {
            if self.title_fill_config_owned
                || self.title_fill.is_none()
                || receipt.rule_name_count == 0
            {
                evidence.mark_not_applicable(key.clone());
            } else if receipt.title_fill_proved() {
                if let Some(capability) = self.title_fill_capability_for(key) {
                    evidence.mark_applied_with_capabilities(key.clone(), [capability]);
                } else if self.is_title_fill_shadowed(key) {
                    evidence.mark_not_applicable(key.clone());
                } else {
                    evidence
                        .mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                }
            } else if matches!(
                key,
                FamilyThemeMechanismKey::Rule { index, .. }
                    if self
                        .title_fill
                        .as_ref()
                        .is_some_and(|winner| winner.rule_index() != *index)
            ) {
                evidence.mark_not_applicable(key.clone());
            } else {
                evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
            }
        }
        for key in &self.text_fill_routes {
            if self.text_fill_config_owned
                || !self.text_fill_roles.has_visible_occurrence(receipt)
                || self.is_text_fill_shadowed(key)
            {
                evidence.mark_not_applicable(key.clone());
            } else if receipt.text_fill_proved(self.text_fill_roles) {
                if let Some(capability) = self.text_fill_capability_for(key) {
                    evidence.mark_applied_with_capabilities(key.clone(), [capability]);
                } else {
                    evidence
                        .mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                }
            } else {
                evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
            }
        }
        evidence
    }

    fn title_fill_capability_for(&self, key: &FamilyThemeMechanismKey) -> Option<ThemeCapability> {
        let FamilyThemeMechanismKey::Rule {
            index,
            target: ThemeTarget::Title,
        } = key
        else {
            return None;
        };
        self.title_fill
            .as_ref()
            .filter(|winner| winner.rule_index() == *index)
            .map(DirectStaticPaint::capability)
    }

    fn is_title_fill_shadowed(&self, key: &FamilyThemeMechanismKey) -> bool {
        matches!(
            key,
            FamilyThemeMechanismKey::Rule { index, .. }
                if self
                    .title_fill
                    .as_ref()
                    .is_some_and(|winner| winner.rule_index() != *index)
        )
    }

    fn text_fill_capability_for(&self, key: &FamilyThemeMechanismKey) -> Option<ThemeCapability> {
        let FamilyThemeMechanismKey::Rule {
            index,
            target: ThemeTarget::Text,
        } = key
        else {
            return None;
        };
        self.text_fill
            .as_ref()
            .filter(|winner| winner.rule_index() == *index)
            .map(DirectStaticPaint::capability)
    }

    fn is_text_fill_shadowed(&self, key: &FamilyThemeMechanismKey) -> bool {
        matches!(
            key,
            FamilyThemeMechanismKey::Rule {
                index,
                target: ThemeTarget::Text,
            } if self
                .text_fill_winner_rule
                .is_some_and(|winner| winner != *index)
        )
    }
}

fn railroad_config_owns_font_stack(config: &MermaidConfig) -> bool {
    railroad_config_owns_any(
        config,
        [
            "railroad.fontFamily",
            "themeVariables.fontFamily",
            "fontFamily",
        ],
    )
}

fn railroad_config_owns_font_size(config: &MermaidConfig) -> bool {
    railroad_config_owns_any(config, ["railroad.fontSize", "themeVariables.fontSize"])
}

fn railroad_config_owns_title_fill(config: &MermaidConfig) -> bool {
    railroad_config_owns_any(
        config,
        [
            "railroad.ruleNameColor",
            "themeVariables.titleColor",
            "themeVariables.textColor",
        ],
    )
}

fn railroad_config_owns_text_fill(config: &MermaidConfig) -> bool {
    railroad_config_owns_any(config, ["themeVariables.textColor"])
}

fn railroad_config_owns_terminal_text_fill(config: &MermaidConfig) -> bool {
    railroad_config_owns_any(
        config,
        [
            "railroad.terminalTextColor",
            "themeVariables.secondaryTextColor",
            "themeVariables.textColor",
        ],
    )
}

fn railroad_config_owns_nonterminal_text_fill(config: &MermaidConfig) -> bool {
    railroad_config_owns_any(
        config,
        [
            "railroad.nonTerminalTextColor",
            "themeVariables.primaryTextColor",
            "themeVariables.textColor",
        ],
    )
}

fn railroad_config_owns_comment_text_fill(config: &MermaidConfig) -> bool {
    railroad_config_owns_any(
        config,
        [
            "railroad.commentTextColor",
            "themeVariables.tertiaryTextColor",
            "themeVariables.textColor",
        ],
    )
}

fn railroad_config_owns_rule_name_text_fill(config: &MermaidConfig) -> bool {
    railroad_config_owns_any(
        config,
        [
            "railroad.ruleNameColor",
            "themeVariables.titleColor",
            "themeVariables.textColor",
        ],
    )
}

fn railroad_config_owns_any<const N: usize>(config: &MermaidConfig, paths: [&str; N]) -> bool {
    paths
        .into_iter()
        .any(|path| merman_core::__private::config_path_overrides_typed_default(config, path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, FontStack, OrdinalPalette,
        ThemeColorValue, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTextStyle, TypographySpec,
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
            &crate::resources::OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            ),
        )
        .unwrap()
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
            receipt
                .record_typography_stylesheet(&invalid_css, "monospace", "18", &mut || {
                    Ok::<(), std::convert::Infallible>(())
                })
                .unwrap();

            assert!(!receipt.typography_stylesheet_verified());
        }
    }

    #[test]
    fn railroad_typography_receipt_uses_css_aware_declaration_boundaries() {
        let stylesheet =
            VALID_CSS.replace("font-family:monospace", "font-family:\"a;b\",sans-serif");
        let mut receipt = RailroadSurfaceReceipt::default();
        receipt.record_root_svg_open(VALID_ROOT_SVG_OPEN);
        receipt
            .record_typography_stylesheet(&stylesheet, "\"a;b\",sans-serif", "18", &mut || {
                Ok::<(), std::convert::Infallible>(())
            })
            .unwrap();

        assert!(receipt.typography_stylesheet_verified());
    }

    #[test]
    fn railroad_typography_receipt_decodes_xml_text_exactly_once() {
        for (serialized_font, expected_font, verified) in [
            ("\"A&amp;B\"", "\"A&B\"", true),
            ("\"A&amp;amp;B\"", "\"A&amp;B\"", true),
            ("\"A&amp;amp;B\"", "\"A&B\"", false),
            ("\"A&unknown;B\"", "\"A&unknown;B\"", false),
        ] {
            let stylesheet = VALID_CSS.replace(
                "font-family:monospace",
                &format!("font-family:{serialized_font}"),
            );
            let mut receipt = RailroadSurfaceReceipt::default();
            receipt.record_root_svg_open(VALID_ROOT_SVG_OPEN);
            receipt
                .record_typography_stylesheet(&stylesheet, expected_font, "18", &mut || {
                    Ok::<(), std::convert::Infallible>(())
                })
                .unwrap();

            assert_eq!(receipt.typography_stylesheet_verified(), verified);
        }
    }

    #[test]
    fn railroad_typography_receipt_does_not_certify_cancelled_xml_decoding() {
        let stylesheet = VALID_CSS.replace("font-family:monospace", "font-family:\"A&amp;B\"");
        let mut receipt = RailroadSurfaceReceipt::default();
        receipt.record_root_svg_open(VALID_ROOT_SVG_OPEN);
        let mut checkpoints = 0;
        let result =
            receipt.record_typography_stylesheet(&stylesheet, "\"A&B\"", "18", &mut || {
                checkpoints += 1;
                if checkpoints == 5 {
                    Err("cancelled")
                } else {
                    Ok(())
                }
            });

        assert_eq!(result, Err("cancelled"));
        assert!(!receipt.typography_stylesheet_verified());
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
            receipt
                .record_typography_stylesheet(&stylesheet, "monospace", "18", &mut || {
                    Ok::<(), std::convert::Infallible>(())
                })
                .unwrap();

            assert!(!receipt.typography_stylesheet_verified());
        }
    }

    #[test]
    fn railroad_comment_occurrence_can_verify_typed_typography() {
        let plan = typed_railroad_plan();
        let mut receipt = plan.begin_terminal_receipt();
        receipt.record_root_svg_open(VALID_ROOT_SVG_OPEN);
        receipt
            .record_typography_stylesheet(VALID_CSS, "monospace", "18", &mut || {
                Ok::<(), std::convert::Infallible>(())
            })
            .unwrap();
        receipt.record_comment_text("comment");
        assert!(plan.record_terminal(receipt));

        let evidence = plan.finish_evidence();
        assert_eq!(evidence.applied().len(), 2);
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
        receipt
            .record_typography_stylesheet(&invalid_css, "monospace", "18", &mut || {
                Ok::<(), std::convert::Infallible>(())
            })
            .unwrap();
        receipt.record_terminal_text("terminal");
        assert!(plan.record_terminal(receipt));

        let evidence = plan.finish_evidence();
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 2);
        assert!(
            evidence
                .residuals()
                .iter()
                .all(|residual| residual.reason()
                    == FamilyThemeResidualReason::UnsupportedTypography)
        );
    }

    #[test]
    fn typed_title_fill_shadows_unsupported_ordinal_palette() {
        let palette = OrdinalPalette::new([
            ThemeColorValue::parse("#abcdef").expect("valid Railroad ordinal palette color")
        ])
        .expect("non-empty Railroad ordinal palette");
        let resolved = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Title,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#123456").expect("valid Railroad title fill"),
                            ),
                        ))
                        .with_ordinal_palette(ThemeTarget::Title, palette),
                ),
            )
            .expect("compile Railroad title fill and palette theme")
            .resolve(DiagramFamilyId::RAILROAD);
        let config = MermaidConfig::from_value(serde_json::json!({}));
        let mut plan = RailroadTypographyThemePlan::resolve(
            Some(&resolved),
            &config,
            &crate::resources::OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            ),
        )
        .unwrap();
        let parsed = merman_core::Engine::new()
            .parse_diagram_for_render_model_sync(
                "railroad-beta\nexpr = terminal(\"\") ;",
                merman_core::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let merman_core::RenderSemanticModel::Railroad(model) = parsed.model() else {
            panic!("Railroad model");
        };
        let layout = super::super::layout_railroad_diagram_typed_for_type_with_theme(
            model,
            "railroad",
            &plan,
            &crate::text::DeterministicTextMeasurer::default(),
        )
        .unwrap();
        plan.prepare_terminal_domain(
            Some(&resolved),
            &layout,
            &crate::resources::OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            ),
        )
        .unwrap();
        let mut receipt = plan.begin_terminal_receipt();
        let stylesheet = VALID_CSS.replace(
            "#rr .railroad-rule-name{font-weight:bold;fill:black;",
            "#rr .railroad-rule-name{font-weight:bold;fill:#123456;",
        );
        receipt.record_root_svg_open(VALID_ROOT_SVG_OPEN);
        receipt
            .record_typography_stylesheet(&stylesheet, "monospace", "18", &mut || {
                Ok::<(), std::convert::Infallible>(())
            })
            .unwrap();
        receipt.record_rule_name_occurrence("#123456");
        assert!(plan.record_terminal(receipt));

        let evidence = plan.finish_evidence();
        assert_eq!(evidence.required_mechanisms().len(), 2);
        assert_eq!(evidence.applied().len(), 1);
        assert_eq!(evidence.not_applicable_mechanisms().len(), 1);
        assert!(evidence.residuals().is_empty());
    }
}
