use std::collections::BTreeSet;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeRuleFacet, ResolvedDiagramTheme,
    ResolvedThemeStyle, ThemeTarget, ThemeTextStyle, ThemeTypographyProperty, ThemeVariant,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};
use crate::text::{ParsedCssFontStack, TextStyle, parse_css_font_stack, resolve_css_font_weight};

const DIRECT_SEQUENCE_TYPOGRAPHY_PROPERTIES: [ThemeTypographyProperty; 4] = [
    ThemeTypographyProperty::FontStack,
    ThemeTypographyProperty::FontSize,
    ThemeTypographyProperty::FontWeight,
    ThemeTypographyProperty::FontStyle,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum SequenceTypographyRole {
    Actor,
    Message,
    Note,
    Loop,
}

impl SequenceTypographyRole {
    pub(crate) const fn target(self) -> ThemeTarget {
        match self {
            Self::Actor => ThemeTarget::ActorLabel,
            Self::Message => ThemeTarget::MessageLabel,
            Self::Note => ThemeTarget::NoteLabel,
            Self::Loop => ThemeTarget::LoopLabel,
        }
    }

    pub(crate) const fn from_target(target: ThemeTarget) -> Option<Self> {
        match target {
            ThemeTarget::ActorLabel => Some(Self::Actor),
            ThemeTarget::MessageLabel => Some(Self::Message),
            ThemeTarget::NoteLabel => Some(Self::Note),
            ThemeTarget::LoopLabel => Some(Self::Loop),
            _ => None,
        }
    }

    fn config_paths(self, property: ThemeTypographyProperty) -> &'static [&'static str] {
        match (self, property) {
            (Self::Actor, ThemeTypographyProperty::FontStack) => {
                &["fontFamily", "sequence.actorFontFamily"]
            }
            (Self::Actor, ThemeTypographyProperty::FontSize) => {
                &["fontSize", "sequence.actorFontSize"]
            }
            (Self::Actor, ThemeTypographyProperty::FontWeight) => {
                &["fontWeight", "sequence.actorFontWeight"]
            }
            (Self::Message | Self::Loop, ThemeTypographyProperty::FontStack) => {
                &["fontFamily", "sequence.messageFontFamily"]
            }
            (Self::Message | Self::Loop, ThemeTypographyProperty::FontSize) => {
                &["fontSize", "sequence.messageFontSize"]
            }
            (Self::Message | Self::Loop, ThemeTypographyProperty::FontWeight) => {
                &["fontWeight", "sequence.messageFontWeight"]
            }
            (Self::Note, ThemeTypographyProperty::FontStack) => {
                &["fontFamily", "sequence.noteFontFamily"]
            }
            (Self::Note, ThemeTypographyProperty::FontSize) => {
                &["fontSize", "sequence.noteFontSize"]
            }
            (Self::Note, ThemeTypographyProperty::FontWeight) => &[
                "fontWeight",
                "sequence.noteFontWeight",
                "themeVariables.noteFontWeight",
            ],
            _ => &[],
        }
    }
}

#[derive(Debug)]
pub(crate) struct SequenceResolvedTypography {
    text_style: TextStyle,
    prepared_typography: ThemeTextStyle,
    source_font_stack: Option<ParsedCssFontStack>,
    resolved_style: Option<ResolvedThemeStyle>,
    typed_properties: BTreeSet<ThemeTypographyProperty>,
    config_overrides: BTreeSet<ThemeTypographyProperty>,
}

impl SequenceResolvedTypography {
    fn resolve(
        role: SequenceTypographyRole,
        mut text_style: TextStyle,
        effective_config: &MermaidConfig,
        resolved_theme: Option<&ResolvedDiagramTheme>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let config_overrides = DIRECT_SEQUENCE_TYPOGRAPHY_PROPERTIES
            .into_iter()
            .filter(|property| {
                role.config_paths(*property).iter().any(|path| {
                    merman_core::__private::config_path_overrides_typed_default(
                        effective_config,
                        path,
                    )
                })
            })
            .collect::<BTreeSet<_>>();
        let resolved_style = resolved_theme
            .filter(|theme| {
                theme.family_mechanism_routes().iter().any(|route| {
                    matches!(
                        route.mechanism(),
                        FamilyThemeMechanism::RuleFacet { target, .. } if target == role.target()
                    )
                })
            })
            .map(|theme| {
                theme.style_with_work_meter(role.target(), ThemeVariant::Default, None, work_meter)
            })
            .transpose()?;
        let mut typed_properties = BTreeSet::new();
        if let (Some(theme), Some(style)) = (resolved_theme, resolved_style.as_ref()) {
            for property in DIRECT_SEQUENCE_TYPOGRAPHY_PROPERTIES {
                let Some(origin) = style.typography_resolution().winner(property) else {
                    continue;
                };
                if theme.rule_facet_disposition(
                    origin.rule_index(),
                    FamilyThemeRuleFacet::Typography(property),
                ) != Some(FamilyThemeDisposition::TypedAdapter)
                    || config_overrides.contains(&property)
                {
                    continue;
                }
                apply_direct_property(&mut text_style, style, property);
                typed_properties.insert(property);
            }
        }
        let (prepared_typography, source_font_stack) = prepared_typography(&text_style);
        Ok(Self {
            text_style,
            prepared_typography,
            source_font_stack,
            resolved_style,
            typed_properties,
            config_overrides,
        })
    }

    pub(crate) const fn text_style(&self) -> &TextStyle {
        &self.text_style
    }

    pub(crate) const fn prepared_typography(&self) -> &ThemeTextStyle {
        &self.prepared_typography
    }

    pub(crate) const fn source_font_stack(&self) -> Option<&ParsedCssFontStack> {
        self.source_font_stack.as_ref()
    }

    pub(crate) const fn resolved_style(&self) -> Option<&ResolvedThemeStyle> {
        self.resolved_style.as_ref()
    }

    pub(crate) const fn config_overrides(&self) -> &BTreeSet<ThemeTypographyProperty> {
        &self.config_overrides
    }

    pub(crate) fn has_typed_emission(&self) -> bool {
        !self.typed_properties.is_empty()
    }

    pub(crate) fn requires_resolved_emission(&self) -> bool {
        self.has_typed_emission() || !self.config_overrides.is_empty()
    }

    pub(crate) fn inline_style(&self, base: &str) -> String {
        if !self.requires_resolved_emission() {
            return base.to_string();
        }
        let mut style = base.trim().trim_end_matches(';').to_string();
        append_typography_declarations(&mut style, &self.text_style);
        style.push(';');
        style
    }

    pub(crate) fn terminal_style(&self, typed_prefix: &str, legacy: String) -> String {
        if self.requires_resolved_emission() {
            self.inline_style(typed_prefix)
        } else {
            legacy
        }
    }

    pub(crate) fn css_declarations(&self) -> Option<String> {
        self.requires_resolved_emission().then(|| {
            let mut declarations = String::new();
            append_typography_declarations(&mut declarations, &self.text_style);
            declarations.push(';');
            declarations
        })
    }

    pub(crate) fn fixed_size_terminal_style(&self) -> Option<String> {
        self.requires_resolved_emission().then(|| {
            let mut declarations = String::new();
            append_font_source_declarations(&mut declarations, &self.text_style);
            declarations.push(';');
            declarations
        })
    }
}

#[derive(Debug)]
pub(crate) struct SequenceTypographyPlan {
    actor: SequenceResolvedTypography,
    message: SequenceResolvedTypography,
    note: SequenceResolvedTypography,
    loop_label: SequenceResolvedTypography,
}

impl SequenceTypographyPlan {
    pub(crate) fn resolve(
        effective_config: &MermaidConfig,
        resolved_theme: Option<&ResolvedDiagramTheme>,
        work_meter: &OperationWorkMeter,
        actor: TextStyle,
        message: TextStyle,
        mut note: TextStyle,
    ) -> Result<Self, OperationWorkError> {
        if merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.noteFontWeight",
        ) && let Some(weight) =
            super::config::SequenceConfigView::new(effective_config.as_value())
                .theme_variable_font_weight("noteFontWeight")
        {
            note.font_weight = Some(weight);
        }
        let loop_label = message.clone();
        Ok(Self {
            actor: SequenceResolvedTypography::resolve(
                SequenceTypographyRole::Actor,
                actor,
                effective_config,
                resolved_theme,
                work_meter,
            )?,
            message: SequenceResolvedTypography::resolve(
                SequenceTypographyRole::Message,
                message,
                effective_config,
                resolved_theme,
                work_meter,
            )?,
            note: SequenceResolvedTypography::resolve(
                SequenceTypographyRole::Note,
                note,
                effective_config,
                resolved_theme,
                work_meter,
            )?,
            loop_label: SequenceResolvedTypography::resolve(
                SequenceTypographyRole::Loop,
                loop_label,
                effective_config,
                resolved_theme,
                work_meter,
            )?,
        })
    }

    pub(crate) const fn role(&self, role: SequenceTypographyRole) -> &SequenceResolvedTypography {
        match role {
            SequenceTypographyRole::Actor => &self.actor,
            SequenceTypographyRole::Message => &self.message,
            SequenceTypographyRole::Note => &self.note,
            SequenceTypographyRole::Loop => &self.loop_label,
        }
    }

    pub(crate) const fn actor(&self) -> &SequenceResolvedTypography {
        &self.actor
    }

    pub(crate) const fn message(&self) -> &SequenceResolvedTypography {
        &self.message
    }

    pub(crate) const fn note(&self) -> &SequenceResolvedTypography {
        &self.note
    }

    pub(crate) const fn loop_label(&self) -> &SequenceResolvedTypography {
        &self.loop_label
    }
}

fn apply_direct_property(
    text_style: &mut TextStyle,
    resolved: &ResolvedThemeStyle,
    property: ThemeTypographyProperty,
) {
    let computed = resolved.typography();
    match property {
        ThemeTypographyProperty::FontStack => {
            text_style.font_family = Some(computed.font_stack().as_css());
        }
        ThemeTypographyProperty::FontSize => {
            text_style.font_size = f64::from(computed.font_size_px());
        }
        ThemeTypographyProperty::FontWeight => {
            text_style.font_weight = Some(computed.font_weight().to_string());
        }
        ThemeTypographyProperty::FontStyle => {
            text_style.font_style = Some(computed.font_style().id().to_string());
        }
        ThemeTypographyProperty::LineHeight
        | ThemeTypographyProperty::LetterSpacing
        | ThemeTypographyProperty::WordSpacing
        | ThemeTypographyProperty::Transform
        | ThemeTypographyProperty::Decoration
        | ThemeTypographyProperty::TextAlign
        | ThemeTypographyProperty::WhiteSpace
        | ThemeTypographyProperty::Wrap => {}
    }
}

fn prepared_typography(text_style: &TextStyle) -> (ThemeTextStyle, Option<ParsedCssFontStack>) {
    let source_font_stack = text_style
        .font_family
        .as_deref()
        .and_then(parse_css_font_stack);
    let mut prepared = ThemeTextStyle::default();
    if let Some(stack) = source_font_stack.as_ref() {
        prepared = prepared.with_font_stack(stack.font_stack().clone());
    }
    prepared = prepared
        .with_font_size_px(text_style.font_size.max(1.0) as f32)
        .expect("Sequence role font size is normalized before prepared-text admission");
    if let Some(weight) = text_style
        .font_weight
        .as_deref()
        .and_then(|weight| resolve_css_font_weight(weight, 400))
    {
        prepared = prepared
            .with_font_weight(weight)
            .expect("resolved CSS font weight is inside the typed domain");
    }
    if let Some(style) = text_style.font_style.as_deref() {
        prepared = prepared.with_font_style(match style.trim().to_ascii_lowercase().as_str() {
            "italic" => crate::diagram_theme::FontStyle::Italic,
            "oblique" => crate::diagram_theme::FontStyle::Oblique,
            _ => crate::diagram_theme::FontStyle::Normal,
        });
    }
    (prepared, source_font_stack)
}

fn append_typography_declarations(style: &mut String, typography: &TextStyle) {
    append_font_family_declaration(style, typography);
    style.push_str("font-size:");
    style.push_str(&typography.font_size.to_string());
    style.push_str("px;");
    append_font_weight_and_style_declarations(style, typography);
}

fn append_font_source_declarations(style: &mut String, typography: &TextStyle) {
    append_font_family_declaration(style, typography);
    append_font_weight_and_style_declarations(style, typography);
}

fn append_font_family_declaration(style: &mut String, typography: &TextStyle) {
    if !style.is_empty() {
        style.push(';');
    }
    if let Some(font_family) = typography.font_family.as_deref() {
        style.push_str("font-family:");
        style.push_str(font_family);
        style.push(';');
    }
}

fn append_font_weight_and_style_declarations(style: &mut String, typography: &TextStyle) {
    style.push_str("font-weight:");
    style.push_str(typography.font_weight.as_deref().unwrap_or("400"));
    style.push_str(";font-style:");
    style.push_str(typography.font_style.as_deref().unwrap_or("normal"));
}
