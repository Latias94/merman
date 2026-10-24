use merman_core::diagrams::zenuml::ZenumlDiagramModel;

use super::{ZenumlDiagramLayout, resolve_zenuml_title};
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    FamilyThemeResidualReason, TerminalVariantDomain, UnsupportedTerminalDomain,
    reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolve_direct_static_stroke,
};
use crate::resources::OperationWorkMeter;

/// Prepared source compatibility values shared by ZenUML measurement and paint.
#[derive(Debug, Default)]
pub(crate) struct ZenumlSourceStyle {
    pub(crate) typography: super::ZenumlTypography,
    colors: std::collections::BTreeMap<&'static str, String>,
    typed_colors: std::collections::BTreeMap<&'static str, DirectStaticPaint>,
    typed_fonts: std::collections::BTreeSet<ThemeTypographyProperty>,
    transparent_frame: bool,
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
            "titleTextColor",
            "messageTextColor",
            "messageLineColor",
            "lifelineColor",
            "sequenceNumberTextColor",
            "fragmentBorderColor",
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
                    "titleTextColor"
                    | "messageTextColor"
                    | "sequenceNumberTextColor"
                    | "actorTextColor"
                    | "noteTextColor"
                    | "groupTextColor"
                    | "subtleTextColor"
                    | "fragmentTextColor" => &["textColor"],
                    "messageLineColor" | "lifelineColor" => &["lineColor"],
                    "fragmentBorderColor"
                    | "actorBorderColor"
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
            ..Self::default()
        }
    }

    pub(crate) fn color<'a>(&'a self, key: &str, fallback: &'a str) -> &'a str {
        self.configured_color(key).unwrap_or(fallback)
    }

    pub(crate) fn transparent_frame(&self) -> bool {
        self.transparent_frame
    }

    pub(crate) fn configured_color(&self, key: &str) -> Option<&str> {
        self.colors.get(key).map(String::as_str)
    }

    fn source_owns_color(&self, key: &str) -> bool {
        self.colors.contains_key(key) && !self.typed_colors.contains_key(key)
    }

    pub(crate) fn uses_default_css(&self) -> bool {
        self.colors.is_empty() && self.typography.uses_default_css()
    }
}

impl ZenumlSourceStyle {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_core::presentation::PresentationConfig,
        work_meter: &OperationWorkMeter,
        replace_default_background: bool,
    ) -> crate::Result<Self> {
        let mut style = Self::new(config.as_value());
        let Some(theme) = theme else {
            return Ok(style);
        };
        for slot in PAINT_SLOTS {
            if style.colors.contains_key(slot.key) {
                continue;
            }
            if merman_core::__private::config_paint_overrides_typed_default(config, slot.global)
                && let Some(value) = config.get_str(slot.global)
            {
                style.colors.insert(slot.key, value.to_owned());
                continue;
            }
            let resolved = if slot.textual {
                theme.text_style_with_work_meter(
                    slot.target,
                    ThemeVariant::Default,
                    None,
                    work_meter,
                )?
            } else {
                theme.style_with_work_meter(slot.target, ThemeVariant::Default, None, work_meter)?
            };
            let accepted = if slot.textual {
                vec![ThemeTarget::Text, slot.target]
            } else {
                vec![slot.target]
            };
            let paint = if slot.stroke {
                resolve_direct_static_stroke(
                    theme,
                    &resolved,
                    &accepted,
                    DirectStaticSelectorDomain::Default,
                )
            } else {
                resolve_direct_static_fill(
                    theme,
                    &resolved,
                    &accepted,
                    DirectStaticSelectorDomain::Default,
                )
            };
            if let Some(paint) = paint {
                style.colors.insert(slot.key, paint.css().to_owned());
                style.typed_colors.insert(slot.key, paint);
            }
        }
        if !style.colors.contains_key("backgroundColor")
            && merman_core::__private::config_paint_overrides_typed_default(
                config,
                "themeVariables.background",
            )
            && let Some(color) =
                crate::config::value_at(config.as_value(), &["themeVariables", "background"])
                    .and_then(serde_json::Value::as_str)
        {
            style.colors.insert("backgroundColor", color.to_owned());
        }
        // The host canvas already owns background decoration. A transparent inner frame
        // reveals it instead of painting the companion's historical white rectangle over it.
        style.transparent_frame =
            replace_default_background && !style.colors.contains_key("backgroundColor");
        if style.transparent_frame {
            style
                .colors
                .insert("backgroundColor", "transparent".to_owned());
        }
        // Source actor colors remain local; typed actor strokes also supply the frame fallback.
        if !style.colors.contains_key("borderColor")
            && let Some(paint) = style.typed_colors.get("actorBorderColor")
        {
            style.colors.insert("borderColor", paint.css().to_owned());
        }
        let base = theme.typography();
        if style.typography.font_family.is_none()
            && base.is_specified(ThemeTypographyProperty::FontStack)
        {
            style.typography.font_family = Some(base.font_stack().as_css());
            style.typed_fonts.insert(ThemeTypographyProperty::FontStack);
        }
        if style.typography.font_size.is_none()
            && base.is_specified(ThemeTypographyProperty::FontSize)
        {
            style.typography.font_size = Some(f64::from(base.font_size_px()));
            style.typed_fonts.insert(ThemeTypographyProperty::FontSize);
        }
        Ok(style)
    }
}

struct PaintSlot {
    key: &'static str,
    target: ThemeTarget,
    stroke: bool,
    textual: bool,
    global: &'static str,
}

const fn slot(
    key: &'static str,
    target: ThemeTarget,
    stroke: bool,
    textual: bool,
    global: &'static str,
) -> PaintSlot {
    PaintSlot {
        key,
        target,
        stroke,
        textual,
        global,
    }
}

const PAINT_SLOTS: &[PaintSlot] = &[
    slot(
        "titleTextColor",
        ThemeTarget::Title,
        false,
        false,
        "themeVariables.textColor",
    ),
    slot(
        "actorBackgroundColor",
        ThemeTarget::Actor,
        false,
        false,
        "themeVariables.actorBkg",
    ),
    slot(
        "actorBorderColor",
        ThemeTarget::Actor,
        true,
        false,
        "themeVariables.actorBorder",
    ),
    slot(
        "actorTextColor",
        ThemeTarget::ActorLabel,
        false,
        true,
        "themeVariables.actorTextColor",
    ),
    slot(
        "lifelineColor",
        ThemeTarget::Lifeline,
        true,
        false,
        "themeVariables.actorLineColor",
    ),
    slot(
        "messageLineColor",
        ThemeTarget::Message,
        true,
        false,
        "themeVariables.signalColor",
    ),
    slot(
        "messageTextColor",
        ThemeTarget::MessageLabel,
        false,
        true,
        "themeVariables.signalTextColor",
    ),
    slot(
        "sequenceNumberTextColor",
        ThemeTarget::SequenceNumberLabel,
        false,
        true,
        "themeVariables.textColor",
    ),
    slot(
        "noteBackgroundColor",
        ThemeTarget::Note,
        false,
        false,
        "themeVariables.noteBkgColor",
    ),
    slot(
        "noteBorderColor",
        ThemeTarget::Note,
        true,
        false,
        "themeVariables.noteBorderColor",
    ),
    slot(
        "noteTextColor",
        ThemeTarget::NoteLabel,
        false,
        true,
        "themeVariables.noteTextColor",
    ),
    slot(
        "groupBackgroundColor",
        ThemeTarget::Cluster,
        false,
        false,
        "themeVariables.clusterBkg",
    ),
    slot(
        "groupBorderColor",
        ThemeTarget::Cluster,
        true,
        false,
        "themeVariables.clusterBorder",
    ),
    slot(
        "groupTextColor",
        ThemeTarget::ClusterLabel,
        false,
        true,
        "themeVariables.textColor",
    ),
    slot(
        "fragmentBorderColor",
        ThemeTarget::Loop,
        true,
        false,
        "themeVariables.lineColor",
    ),
    slot(
        "mutedSurfaceColor",
        ThemeTarget::LoopLabelBackground,
        false,
        false,
        "themeVariables.tertiaryColor",
    ),
    slot(
        "fragmentTextColor",
        ThemeTarget::LoopLabel,
        false,
        true,
        "themeVariables.textColor",
    ),
    slot(
        "activationBackgroundColor",
        ThemeTarget::Activation,
        false,
        false,
        "themeVariables.activationBkgColor",
    ),
    slot(
        "activationBorderColor",
        ThemeTarget::Activation,
        true,
        false,
        "themeVariables.activationBorderColor",
    ),
    slot(
        "subtleTextColor",
        ThemeTarget::Text,
        false,
        true,
        "themeVariables.textColor",
    ),
];

#[derive(Debug)]
pub(crate) struct ZenumlThemePlan {
    source_style: ZenumlSourceStyle,
    evidence: FamilyThemeOutcomes,
    rules: Vec<(FamilyThemeMechanismKey, usize)>,
    font_routes: Vec<ThemeTypographyProperty>,
    fallback_not_applicable: std::collections::BTreeSet<FamilyThemeMechanismKey>,
    terminal: std::sync::OnceLock<ZenumlThemeReceipt>,
}

impl ZenumlThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        model: &ZenumlDiagramModel,
        layout: &ZenumlDiagramLayout,
        diagram_title: Option<&str>,
        work_meter: &OperationWorkMeter,
        source_style: ZenumlSourceStyle,
    ) -> crate::Result<Self> {
        let mut evidence = FamilyThemeOutcomes::default();
        let mut rules = Vec::new();
        let mut font_routes = Vec::new();
        let mut fallback_not_applicable = std::collections::BTreeSet::new();
        if let Some(theme) = theme {
            for route in theme.family_mechanism_routes().iter().copied() {
                let key = theme.family_mechanism_key(route);
                match route.mechanism() {
                    FamilyThemeMechanism::BaseTypography(property) => {
                        if route.disposition() == FamilyThemeDisposition::Unsupported {
                            if has_visible_text(layout)
                                || resolve_zenuml_title(model, diagram_title).is_some()
                            {
                                evidence.mark_residual(
                                    key,
                                    FamilyThemeResidualReason::UnsupportedTypography,
                                );
                            } else {
                                evidence.mark_not_applicable(key);
                            }
                        } else {
                            font_routes.push(property);
                        }
                    }
                    FamilyThemeMechanism::RuleFacet { rule_index, .. }
                        if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                    {
                        rules.push((key, rule_index));
                    }
                    _ => {}
                }
            }
            let mut targets = std::collections::BTreeSet::new();
            for slot in PAINT_SLOTS {
                targets.insert(slot.target);
            }
            for target in targets {
                let (fill, stroke) =
                    terminal_source_owners(target, model, layout, diagram_title, &source_style);
                let variants = TerminalVariantDomain::uniform(fill.len(), ThemeVariant::Default);
                let owned_targets = [ThemeTarget::Text, target];
                let domain = if target != ThemeTarget::Text
                    && PAINT_SLOTS
                        .iter()
                        .any(|slot| slot.target == target && slot.textual)
                {
                    UnsupportedTerminalDomain::textual(target, &owned_targets, variants)
                } else {
                    UnsupportedTerminalDomain::direct(target, variants)
                }
                .with_source_owned_fill(&fill)
                .with_source_owned_stroke(&stroke);
                // Text is a fallback for several independent roles. Reconcile each role
                // separately, and retain any residual before applying absence receipts.
                let mut fallbacks = FamilyThemeOutcomes::default();
                reconcile_unsupported_terminal_domains(
                    theme,
                    &mut fallbacks,
                    &[domain],
                    work_meter,
                )?;
                for residual in fallbacks.residuals() {
                    evidence.mark_residual(residual.key().clone(), residual.reason());
                }
                fallback_not_applicable
                    .extend(fallbacks.not_applicable_mechanisms().iter().cloned());
            }
        }
        Ok(Self {
            source_style,
            evidence,
            rules,
            font_routes,
            fallback_not_applicable,
            terminal: std::sync::OnceLock::new(),
        })
    }

    pub(crate) fn source_style(&self) -> &ZenumlSourceStyle {
        &self.source_style
    }
    pub(crate) fn begin_terminal_receipt(&self) -> ZenumlThemeReceipt {
        ZenumlThemeReceipt::default()
    }
    pub(crate) fn record_terminal(&self, receipt: ZenumlThemeReceipt) -> bool {
        self.terminal.set(receipt).is_ok()
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
        let Some(receipt) = self.terminal.get().filter(|receipt| receipt.stylesheet) else {
            return;
        };
        for (key, index) in &self.rules {
            let paints = self
                .source_style
                .typed_colors
                .iter()
                .filter(|(slot, paint)| {
                    paint.rule_index() == *index && receipt.colors.contains(**slot)
                })
                .map(|(_, paint)| paint.capability())
                .collect::<Vec<_>>();
            if !paints.is_empty() {
                evidence.mark_applied_with_capabilities(key.clone(), paints);
            } else {
                evidence.mark_not_applicable(key.clone());
            }
        }
        for property in &self.font_routes {
            let key = FamilyThemeMechanismKey::Typography(*property);
            if receipt.visible_text && self.source_style.typed_fonts.contains(property) {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            } else {
                evidence.mark_not_applicable(key);
            }
        }
        for key in &self.fallback_not_applicable {
            evidence.mark_not_applicable(key.clone());
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ZenumlThemeReceipt {
    stylesheet: bool,
    visible_text: bool,
    colors: std::collections::BTreeSet<&'static str>,
}

impl ZenumlThemeReceipt {
    pub(crate) fn record_stylesheet(&mut self) {
        self.stylesheet = true;
    }
    pub(crate) fn record_colors(&mut self, colors: &[&'static str]) {
        self.colors.extend(colors.iter().copied());
    }
    pub(crate) fn record_text(&mut self, text: &str) {
        self.visible_text |= !text.trim().is_empty();
    }
}

fn terminal_source_owners(
    target: ThemeTarget,
    model: &ZenumlDiagramModel,
    layout: &ZenumlDiagramLayout,
    title: Option<&str>,
    source_style: &ZenumlSourceStyle,
) -> (Vec<bool>, Vec<bool>) {
    let source_owns = |stroke| {
        PAINT_SLOTS.iter().any(|slot| {
            slot.target == target
                && slot.stroke == stroke
                && source_style.source_owns_color(slot.key)
        })
    };
    let count = terminal_count(target, model, layout, title);
    let mut fill = vec![source_owns(false); count];
    let stroke = vec![source_owns(true); count];
    if !source_owns(false) {
        match target {
            ThemeTarget::Actor => {
                fill = rendered_participants(layout)
                    .map(|participant| participant.color.is_some())
                    .collect();
            }
            ThemeTarget::MessageLabel => {
                fill = layout
                    .messages
                    .iter()
                    .map(|message| message.style.contains_key("fill"))
                    .chain(
                        layout
                            .self_calls
                            .iter()
                            .map(|message| message.style.contains_key("fill")),
                    )
                    .chain(
                        layout
                            .creations
                            .iter()
                            .map(|creation| creation.message.style.contains_key("fill")),
                    )
                    .chain(layout.returns.iter().map(|_| false))
                    .collect();
            }
            ThemeTarget::Text => {
                fill = layout
                    .comments
                    .iter()
                    .map(|comment| comment.style.contains_key("fill"))
                    .collect();
            }
            _ => {}
        }
    }
    (fill, stroke)
}

fn rendered_participants(
    layout: &ZenumlDiagramLayout,
) -> impl Iterator<Item = &super::ZenumlParticipantLayout> {
    let creation_names = layout
        .creations
        .iter()
        .map(|creation| creation.participant.name.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    layout
        .participants
        .iter()
        .filter(move |participant| !creation_names.contains(participant.name.as_str()))
        .chain(
            layout
                .creations
                .iter()
                .map(|creation| &creation.participant),
        )
}

fn terminal_count(
    target: ThemeTarget,
    model: &ZenumlDiagramModel,
    layout: &ZenumlDiagramLayout,
    title: Option<&str>,
) -> usize {
    match target {
        ThemeTarget::Title => usize::from(resolve_zenuml_title(model, title).is_some()),
        ThemeTarget::Actor => rendered_participants(layout).count(),
        ThemeTarget::ActorLabel => rendered_participants(layout)
            .filter(|participant| !participant.is_starter)
            .count(),
        ThemeTarget::Lifeline => layout.lifelines.len(),
        ThemeTarget::Message => {
            layout.messages.len()
                + layout.self_calls.len()
                + layout.creations.len()
                + layout
                    .returns
                    .iter()
                    .filter(|returned| !returned.is_self)
                    .count()
        }
        ThemeTarget::MessageLabel => {
            layout.messages.len()
                + layout.self_calls.len()
                + layout.creations.len()
                + layout.returns.len()
        }
        ThemeTarget::SequenceNumberLabel => layout
            .messages
            .iter()
            .map(|message| message.number.as_str())
            .chain(
                layout
                    .self_calls
                    .iter()
                    .map(|message| message.number.as_str()),
            )
            .chain(
                layout
                    .creations
                    .iter()
                    .map(|creation| creation.message.number.as_str()),
            )
            .chain(
                layout
                    .returns
                    .iter()
                    .filter(|returned| !returned.is_self)
                    .map(|returned| returned.number.as_str()),
            )
            .chain(
                layout
                    .fragments
                    .iter()
                    .map(|fragment| fragment.number.as_str()),
            )
            .filter(|number| !number.is_empty())
            .count(),
        ThemeTarget::Note | ThemeTarget::NoteLabel => layout.dividers.len(),
        ThemeTarget::Cluster | ThemeTarget::ClusterLabel => layout.groups.len(),
        ThemeTarget::Loop | ThemeTarget::LoopLabel | ThemeTarget::LoopLabelBackground => {
            layout.fragments.len()
        }
        ThemeTarget::Activation => layout.occurrences.len(),
        ThemeTarget::Text => layout.comments.len(),
        _ => 0,
    }
}

fn has_visible_text(layout: &ZenumlDiagramLayout) -> bool {
    layout
        .participants
        .iter()
        .any(|participant| !participant.is_starter && !participant.label.is_empty())
        || !layout.messages.is_empty()
        || !layout.self_calls.is_empty()
        || !layout.creations.is_empty()
        || !layout.returns.is_empty()
        || !layout.fragments.is_empty()
        || !layout.dividers.is_empty()
        || !layout.comments.is_empty()
        || layout.groups.iter().any(|group| !group.name.is_empty())
}

#[cfg(test)]
mod tests {

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
}
