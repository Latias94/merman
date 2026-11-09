use std::collections::BTreeSet;
use std::sync::OnceLock;

use merman_model::presentation::PresentationConfig;

use super::RailroadStyle;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, ResolvedDiagramTheme, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    FamilyThemeResidualReason, StaticSurfacePaint, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
};

#[derive(Debug, Default)]
struct RailroadPaintOwnership {
    fill: [bool; 3],
    stroke: [bool; 3],
    label: [bool; 3],
    edge: bool,
    marker: bool,
}

impl RailroadPaintOwnership {
    fn resolve(config: &PresentationConfig) -> Self {
        Self {
            fill: [
                railroad_paint_role_owned(
                    config,
                    "terminalFill",
                    "secondBkg",
                    Some("secondaryColor"),
                ),
                railroad_paint_role_owned(config, "nonTerminalFill", "mainBkg", Some("background")),
                railroad_paint_role_owned(
                    config,
                    "specialFill",
                    "tertiaryColor",
                    Some("secondaryColor"),
                ),
            ],
            stroke: [
                railroad_paint_role_owned(
                    config,
                    "terminalStroke",
                    "secondaryBorderColor",
                    Some("lineColor"),
                ),
                railroad_paint_role_owned(
                    config,
                    "nonTerminalStroke",
                    "primaryBorderColor",
                    Some("lineColor"),
                ),
                railroad_paint_role_owned(
                    config,
                    "specialStroke",
                    "tertiaryBorderColor",
                    Some("secondaryBorderColor"),
                ),
            ],
            label: [
                railroad_paint_role_owned(
                    config,
                    "terminalTextColor",
                    "secondaryTextColor",
                    Some("textColor"),
                ),
                railroad_paint_role_owned(
                    config,
                    "nonTerminalTextColor",
                    "primaryTextColor",
                    Some("textColor"),
                ),
                railroad_paint_role_owned(
                    config,
                    "nonTerminalTextColor",
                    "primaryTextColor",
                    Some("textColor"),
                ),
            ],
            edge: railroad_paint_role_owned(config, "lineColor", "lineColor", None),
            marker: railroad_paint_role_owned(config, "markerFill", "lineColor", None),
        }
    }
}

fn railroad_paint_role_owned(
    config: &PresentationConfig,
    local_key: &str,
    theme_key: &str,
    theme_fallback: Option<&str>,
) -> bool {
    let valid = |value: Option<&serde_json::Value>| {
        value
            .and_then(serde_json::Value::as_str)
            .map(super::trim_ecmascript_whitespace)
            .is_some_and(super::is_valid_color_value)
    };
    if valid(super::railroad_config_value(config.as_value(), local_key)) {
        return config.config_paint_overrides_typed_default(&format!("railroad.{local_key}"));
    }
    // Follow the same primary/alias selection as `railroad_style`, including Default snapshots.
    let key = match config
        .as_value()
        .get("themeVariables")
        .and_then(|variables| variables.get(theme_key))
    {
        Some(value) if !value.is_null() => Some(theme_key),
        _ => theme_fallback,
    };
    key.is_some_and(|key| {
        valid(super::theme_config_value(config.as_value(), key, None))
            && config.config_paint_overrides_typed_default(&format!("themeVariables.{key}"))
    })
}

#[derive(Debug)]
struct RailroadUnsupportedRoute {
    key: FamilyThemeMechanismKey,
    target: ThemeTarget,
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

/// Application visibility recorded at the existing text writer events.
#[derive(Debug, Default)]
pub(crate) struct RailroadSurfaceReceipt {
    requested: bool,
    terminal_text_count: usize,
    nonterminal_text_count: usize,
    comment_text_count: usize,
    special_text_count: usize,
    rule_name_count: usize,
}

impl RailroadSurfaceReceipt {
    pub(crate) fn record_terminal_text(&mut self, text: &str) {
        if self.requested && !text.trim().is_empty() {
            self.terminal_text_count += 1;
        }
    }

    pub(crate) fn record_nonterminal_text(&mut self, text: &str) {
        if self.requested && !text.trim().is_empty() {
            self.nonterminal_text_count += 1;
        }
    }

    pub(crate) fn record_comment_text(&mut self, text: &str) {
        if self.requested && !text.trim().is_empty() {
            self.comment_text_count += 1;
        }
    }

    pub(crate) fn record_special_text(&mut self, text: &str) {
        if self.requested && !text.trim().is_empty() {
            self.special_text_count += 1;
        }
    }

    pub(crate) fn record_rule_name_occurrence(&mut self, _emitted_fill: &str) {
        if self.requested {
            self.rule_name_count += 1;
        }
    }

    fn has_styled_text(&self) -> bool {
        self.terminal_text_count
            + self.nonterminal_text_count
            + self.comment_text_count
            + self.special_text_count
            + self.rule_name_count
            != 0
    }
}

/// Final Railroad style shared by layout measurement, SVG CSS, and terminal evidence.
#[derive(Debug)]
pub(crate) struct RailroadTypographyThemePlan {
    style: RailroadStyle,
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
    text_fill_winner_rule: Option<usize>,
    unsupported_terminal_evidence: Option<FamilyThemeOutcomes>,
    text_fill_routes: Box<[FamilyThemeMechanismKey]>,
    unsupported_routes: Box<[RailroadUnsupportedRoute]>,
    terminal_receipt: OnceLock<RailroadSurfaceReceipt>,
    core_ownership: RailroadPaintOwnership,
    core_paints: Vec<StaticSurfacePaint>,
}

impl RailroadTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> Result<Self, crate::resources::OperationWorkError> {
        let mut style = super::railroad_style(effective_config.as_value());
        let Some(theme) = theme else {
            return Ok(Self {
                style,
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
                text_fill_winner_rule: None,
                unsupported_terminal_evidence: None,
                text_fill_routes: Box::new([]),
                unsupported_routes: Box::new([]),
                terminal_receipt: OnceLock::new(),
                core_ownership: RailroadPaintOwnership::default(),
                core_paints: Vec::new(),
            });
        };

        let config_owns_font_stack = railroad_config_owns_font_stack(effective_config);
        let config_owns_font_size = railroad_config_owns_font_size(effective_config);
        let title_fill_config_owned = railroad_config_owns_title_fill(effective_config);
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
                            });
                        }
                    }
                }
                FamilyThemeMechanism::RuleFacet { target, .. }
                    if route.disposition() == FamilyThemeDisposition::Unsupported =>
                {
                    unsupported_routes.push(RailroadUnsupportedRoute {
                        key: theme.family_mechanism_key(route),
                        target,
                    });
                }
                FamilyThemeMechanism::RuleFacet { .. } => {}
                FamilyThemeMechanism::OrdinalPalette { target } => {
                    debug_assert_eq!(route.disposition(), FamilyThemeDisposition::Unsupported);
                    unsupported_routes.push(RailroadUnsupportedRoute {
                        key: theme.family_mechanism_key(route),
                        target,
                    });
                }
                FamilyThemeMechanism::EffectBinding { target, .. } => {
                    debug_assert_eq!(route.disposition(), FamilyThemeDisposition::Unsupported);
                    unsupported_routes.push(RailroadUnsupportedRoute {
                        key: theme.family_mechanism_key(route),
                        target,
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
            rule_name: !title_style_has_winner && !title_fill_config_owned,
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
            text_fill_winner_rule,
            unsupported_terminal_evidence: None,
            text_fill_routes: text_fill_routes.into_iter().collect(),
            unsupported_routes: unsupported_routes.into_boxed_slice(),
            terminal_receipt: OnceLock::new(),
            core_ownership: RailroadPaintOwnership::resolve(effective_config),
            core_paints: Vec::new(),
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
        let mut source_fill = Vec::new();
        let mut source_stroke = Vec::new();
        let mut source_label = Vec::new();
        let mut edge_count = 0usize;
        let text_visible = self.style.font_size > 0.0;
        for rule in &layout.rules {
            work_meter.charge(1)?;
            edge_count += rule.paths.len();
            for element in &rule.elements {
                work_meter.charge(1)?;
                let role = match element.kind.as_str() {
                    "terminal" => 0,
                    "nonterminal" => 1,
                    "special" => 2,
                    _ => continue,
                };
                source_fill.push(self.core_ownership.fill[role]);
                source_stroke.push(self.core_ownership.stroke[role]);
                if text_visible && !element.label.trim().is_empty() {
                    source_label.push(self.core_ownership.label[role]);
                }
            }
        }
        let node = StaticSurfacePaint::resolve(
            Some(theme),
            ThemeTarget::Node,
            &source_fill,
            &source_stroke,
            work_meter,
        )?;
        let label = StaticSurfacePaint::resolve(
            Some(theme),
            ThemeTarget::NodeLabel,
            &source_label,
            &vec![false; source_label.len()],
            work_meter,
        )?;
        let edge = StaticSurfacePaint::resolve(
            Some(theme),
            ThemeTarget::Edge,
            &vec![true; edge_count],
            &vec![self.core_ownership.edge; edge_count],
            work_meter,
        )?;
        let marker_count = layout.rules.len().saturating_mul(2);
        let marker = StaticSurfacePaint::resolve(
            Some(theme),
            ThemeTarget::Marker,
            &vec![self.core_ownership.marker; marker_count],
            &vec![false; marker_count],
            work_meter,
        )?;
        for (role, (fill, stroke)) in [
            (
                &mut self.style.terminal_fill,
                &mut self.style.terminal_stroke,
            ),
            (
                &mut self.style.non_terminal_fill,
                &mut self.style.non_terminal_stroke,
            ),
            (&mut self.style.special_fill, &mut self.style.special_stroke),
        ]
        .into_iter()
        .enumerate()
        {
            if !self.core_ownership.fill[role]
                && let Some(paint) = &node.fill
            {
                *fill = paint.css().to_owned();
            }
            if !self.core_ownership.stroke[role]
                && let Some(paint) = &node.stroke
            {
                *stroke = paint.css().to_owned();
            }
        }
        if let Some(paint) = &label.fill {
            if !self.core_ownership.label[0] {
                self.style.terminal_text_color = paint.css().to_owned();
            }
            if !self.core_ownership.label[1] {
                self.style.non_terminal_text_color = paint.css().to_owned();
            }
            self.text_fill_roles.terminal &= self.core_ownership.label[0];
            self.text_fill_roles.nonterminal &= self.core_ownership.label[1];
        }
        if let Some(paint) = &edge.stroke {
            self.style.line_color = paint.css().to_owned();
        }
        if let Some(paint) = &marker.fill {
            self.style.marker_fill = paint.css().to_owned();
        }
        self.core_paints = vec![node, label, edge, marker];
        if !text_visible
            || !self
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
        let mut evidence = FamilyThemeOutcomes::default();
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
        self.unsupported_terminal_evidence = Some(evidence);
        Ok(())
    }

    pub(crate) const fn style(&self) -> &RailroadStyle {
        &self.style
    }

    pub(crate) fn begin_terminal_receipt(&self) -> RailroadSurfaceReceipt {
        RailroadSurfaceReceipt {
            // Every text role, including rule names, uses the final prepared font size.
            requested: self.style.font_size > 0.0
                && (self.typed_font_stack_requested
                    || self.typed_font_size_requested
                    || !self.unsupported_properties.is_empty()
                    || !self.unsupported_routes.is_empty()
                    || !self.title_fill_routes.is_empty()
                    || !self.text_fill_routes.is_empty()),
            ..RailroadSurfaceReceipt::default()
        }
    }

    pub(crate) fn record_terminal(&self, receipt: RailroadSurfaceReceipt) -> bool {
        self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        let Some(receipt) = self.terminal_receipt.get() else {
            return;
        };
        for paint in &self.core_paints {
            paint.record_evidence(evidence);
        }
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
            return;
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
            } else {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            }
        }
        for property in &self.unsupported_properties {
            evidence.mark_residual(
                FamilyThemeMechanismKey::Typography(*property),
                FamilyThemeResidualReason::UnsupportedTypography,
            );
        }
        if let Some(terminal_evidence) = &self.unsupported_terminal_evidence {
            evidence.merge_borrowed(terminal_evidence);
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
            } else {
                if let Some(capability) = self.title_fill_capability_for(key) {
                    evidence.mark_applied_with_capabilities(key.clone(), [capability]);
                } else if self.is_title_fill_shadowed(key) {
                    evidence.mark_not_applicable(key.clone());
                } else {
                    evidence
                        .mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                }
            }
        }
        for key in &self.text_fill_routes {
            if !self.text_fill_roles.has_visible_occurrence(receipt)
                || self.is_text_fill_shadowed(key)
            {
                evidence.mark_not_applicable(key.clone());
            } else {
                if let Some(capability) = self.text_fill_capability_for(key) {
                    evidence.mark_applied_with_capabilities(key.clone(), [capability]);
                } else {
                    evidence
                        .mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                }
            }
        }
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

fn railroad_config_owns_font_stack(config: &PresentationConfig) -> bool {
    let valid = |value| !super::sanitize_font_family_value(value, "").is_empty();
    if valid(super::railroad_config_value(
        config.as_value(),
        "fontFamily",
    )) {
        config.config_path_overrides_typed_default("railroad.fontFamily")
    } else {
        valid(super::theme_config_value(
            config.as_value(),
            "fontFamily",
            None,
        )) && config.config_path_overrides_typed_default("themeVariables.fontFamily")
    }
}

fn railroad_config_owns_font_size(config: &PresentationConfig) -> bool {
    if super::parse_number_value(super::railroad_config_value(config.as_value(), "fontSize"))
        .is_some_and(|size| size.is_finite() && size >= 0.0)
    {
        config.config_path_overrides_typed_default("railroad.fontSize")
    } else {
        super::parse_theme_font_size(super::theme_config_value(
            config.as_value(),
            "fontSize",
            None,
        ))
        .is_some()
            && config.config_path_overrides_typed_default("themeVariables.fontSize")
    }
}

fn railroad_config_owns_title_fill(config: &PresentationConfig) -> bool {
    railroad_paint_role_owned(config, "ruleNameColor", "titleColor", Some("textColor"))
}

fn railroad_config_owns_terminal_text_fill(config: &PresentationConfig) -> bool {
    railroad_paint_role_owned(
        config,
        "terminalTextColor",
        "secondaryTextColor",
        Some("textColor"),
    )
}

fn railroad_config_owns_nonterminal_text_fill(config: &PresentationConfig) -> bool {
    railroad_paint_role_owned(
        config,
        "nonTerminalTextColor",
        "primaryTextColor",
        Some("textColor"),
    )
}

fn railroad_config_owns_comment_text_fill(config: &PresentationConfig) -> bool {
    railroad_paint_role_owned(
        config,
        "commentTextColor",
        "tertiaryTextColor",
        Some("textColor"),
    )
}
