use std::collections::BTreeSet;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, ResolvedThemeStyle, ThemeTarget, ThemeTextStyle, ThemeTypographyProperty,
    ThemeVariant,
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

    const fn config_role(self) -> super::config::SequenceTypographyConfigRole {
        match self {
            Self::Actor => super::config::SequenceTypographyConfigRole::Actor,
            Self::Message | Self::Loop => super::config::SequenceTypographyConfigRole::Message,
            Self::Note => super::config::SequenceTypographyConfigRole::Note,
        }
    }
}

#[derive(Debug)]
struct SequenceBaseTypography {
    configured_font_family_css: String,
    font_family_css: String,
    font_stack: ParsedCssFontStack,
    font_size_px: f64,
    typed_properties: BTreeSet<ThemeTypographyProperty>,
}

impl SequenceBaseTypography {
    fn resolve(
        effective_config: &MermaidConfig,
        resolved_theme: Option<&ResolvedDiagramTheme>,
    ) -> Self {
        let mut config_overrides = BTreeSet::new();
        for (property, paths) in [
            (
                ThemeTypographyProperty::FontStack,
                &["fontFamily", "themeVariables.fontFamily"][..],
            ),
            (ThemeTypographyProperty::FontSize, &["fontSize"][..]),
        ] {
            if paths.iter().any(|path| {
                merman_core::__private::config_path_overrides_typed_default(effective_config, path)
            }) {
                config_overrides.insert(property);
            }
        }

        let configured_font_family =
            crate::config::config_font_family_css(effective_config.as_value());
        let config = super::config::SequenceConfigView::new(effective_config.as_value());
        let configured_font_size = config
            .root_json_number("fontSize")
            .or_else(|| config.sequence_json_number("messageFontSize"))
            .unwrap_or(16.0)
            .max(1.0);
        let mut font_family_css = configured_font_family.clone();
        let mut font_size_px = configured_font_size;
        let mut typed_properties = BTreeSet::new();

        if let Some(theme) = resolved_theme {
            for route in theme.family_mechanism_routes().iter().copied() {
                let FamilyThemeMechanism::BaseTypography(property) = route.mechanism() else {
                    continue;
                };
                if route.disposition() != FamilyThemeDisposition::TypedAdapter
                    || config_overrides.contains(&property)
                {
                    continue;
                }
                match property {
                    ThemeTypographyProperty::FontStack => {
                        font_family_css = theme.typography().font_stack().as_css();
                        typed_properties.insert(property);
                    }
                    ThemeTypographyProperty::FontSize => {
                        font_size_px = f64::from(theme.typography().font_size_px());
                        typed_properties.insert(property);
                    }
                    ThemeTypographyProperty::FontWeight
                    | ThemeTypographyProperty::FontStyle
                    | ThemeTypographyProperty::LineHeight
                    | ThemeTypographyProperty::LetterSpacing
                    | ThemeTypographyProperty::WordSpacing
                    | ThemeTypographyProperty::Transform
                    | ThemeTypographyProperty::Decoration
                    | ThemeTypographyProperty::TextAlign
                    | ThemeTypographyProperty::WhiteSpace
                    | ThemeTypographyProperty::Wrap => {}
                }
            }
        }

        let font_stack = parse_css_font_stack(&font_family_css)
            .expect("normalized Sequence font-family CSS must remain parseable");
        Self {
            configured_font_family_css: configured_font_family,
            font_family_css,
            font_stack,
            font_size_px,
            typed_properties,
        }
    }

    fn apply_to_role(
        &self,
        role_config: &super::config::SequenceRoleTypographyConfig,
        text_style: &mut TextStyle,
    ) -> BTreeSet<ThemeTypographyProperty> {
        let mut applied = BTreeSet::new();
        for property in [
            ThemeTypographyProperty::FontStack,
            ThemeTypographyProperty::FontSize,
        ] {
            let overridden = role_config.config_owns(property);
            if self.typed_properties.contains(&property) && !overridden {
                apply_base_property(text_style, self, property);
                applied.insert(property);
            }
        }
        applied
    }
}

/// Private terminal text surfaces that share public Sequence typography targets.
///
/// Public theme targets intentionally remain role-oriented. This internal seam keeps terminal
/// Mermaid ownership and writer receipts local to the selector arm that actually consumes the
/// fill, without duplicating font resolution or layout measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum SequenceTextSurface {
    ParticipantLabel,
    BoxTitle,
    MessageLabel,
    NoteLabel,
    ControlKeyword,
    ControlPrimaryTitle,
    ControlSectionTitle,
}

impl SequenceTextSurface {
    pub(crate) const ALL: [Self; 7] = [
        Self::ParticipantLabel,
        Self::BoxTitle,
        Self::MessageLabel,
        Self::NoteLabel,
        Self::ControlKeyword,
        Self::ControlPrimaryTitle,
        Self::ControlSectionTitle,
    ];
    pub(crate) const COUNT: usize = Self::ALL.len();

    pub(crate) const fn role(self) -> SequenceTypographyRole {
        match self {
            Self::ParticipantLabel | Self::BoxTitle => SequenceTypographyRole::Actor,
            Self::MessageLabel => SequenceTypographyRole::Message,
            Self::NoteLabel => SequenceTypographyRole::Note,
            Self::ControlKeyword | Self::ControlPrimaryTitle | Self::ControlSectionTitle => {
                SequenceTypographyRole::Loop
            }
        }
    }

    pub(crate) const fn index(self) -> usize {
        match self {
            Self::ParticipantLabel => 0,
            Self::BoxTitle => 1,
            Self::MessageLabel => 2,
            Self::NoteLabel => 3,
            Self::ControlKeyword => 4,
            Self::ControlPrimaryTitle => 5,
            Self::ControlSectionTitle => 6,
        }
    }

    pub(crate) const fn mermaid_fill_owner_path(self) -> &'static str {
        match self {
            Self::ParticipantLabel => "themeVariables.actorTextColor",
            Self::BoxTitle => "themeVariables.textColor",
            Self::MessageLabel => "themeVariables.signalTextColor",
            Self::NoteLabel => "themeVariables.noteTextColor",
            Self::ControlKeyword => "themeVariables.labelTextColor",
            Self::ControlPrimaryTitle | Self::ControlSectionTitle => "themeVariables.loopTextColor",
        }
    }

    pub(crate) const fn terminal_selectors(self) -> &'static str {
        match self {
            Self::ParticipantLabel => "text.actor,text.actor>tspan",
            Self::BoxTitle => "text.text,text.text>tspan",
            Self::MessageLabel => ".messageText,.messageText>tspan",
            Self::NoteLabel => ".noteText,.noteText>tspan",
            Self::ControlKeyword => ".labelText,.labelText>tspan",
            Self::ControlPrimaryTitle => ".loopText,.loopText>tspan",
            Self::ControlSectionTitle => ".sectionTitle,.sectionTitle>tspan",
        }
    }
}

#[derive(Debug)]
pub(crate) struct SequenceResolvedTypography {
    role: SequenceTypographyRole,
    measurement_style: TextStyle,
    terminal_text_style: TextStyle,
    prepared_typography: ThemeTextStyle,
    font_stack: ParsedCssFontStack,
    resolved_style: Option<ResolvedThemeStyle>,
    typed_properties: BTreeSet<ThemeTypographyProperty>,
    base_typed_properties: BTreeSet<ThemeTypographyProperty>,
    config_overrides: BTreeSet<ThemeTypographyProperty>,
    typed_fill: Option<String>,
    fill_overrides: BTreeSet<SequenceTextSurface>,
}

impl SequenceResolvedTypography {
    fn resolve(
        role: SequenceTypographyRole,
        effective_config: &MermaidConfig,
        resolved_theme: Option<&ResolvedDiagramTheme>,
        work_meter: &OperationWorkMeter,
        base_typography: &SequenceBaseTypography,
    ) -> Result<Self, OperationWorkError> {
        let role_config = super::config::SequenceConfigView::from_mermaid_config(effective_config)
            .resolve_role_typography(role.config_role());
        let config_overrides = DIRECT_SEQUENCE_TYPOGRAPHY_PROPERTIES
            .into_iter()
            .filter(|property| role_config.config_owns(*property))
            .collect::<BTreeSet<_>>();
        let fill_overrides = SequenceTextSurface::ALL
            .into_iter()
            .filter(|surface| surface.role() == role)
            .filter(|surface| {
                merman_core::__private::config_path_overrides_typed_default(
                    effective_config,
                    surface.mermaid_fill_owner_path(),
                )
            })
            .collect::<BTreeSet<_>>();
        let mut text_style = role_config.measurement_style().clone();
        let mut base_typed_properties =
            base_typography.apply_to_role(&role_config, &mut text_style);
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
                base_typed_properties.remove(&property);
            }
        }
        let typed_fill = resolved_theme
            .zip(resolved_style.as_ref())
            .and_then(|(theme, style)| typed_static_fill(theme, style));
        let measurement_style = text_style;
        let inherited_font_family =
            if config_overrides.contains(&ThemeTypographyProperty::FontStack) {
                &base_typography.configured_font_family_css
            } else {
                &base_typography.font_family_css
            };
        let (terminal_text_style, font_stack) =
            cssom_effective_text_style(&measurement_style, inherited_font_family);
        let prepared_typography = prepared_typography(&terminal_text_style, &font_stack);
        Ok(Self {
            role,
            measurement_style,
            terminal_text_style,
            prepared_typography,
            font_stack,
            resolved_style,
            typed_properties,
            base_typed_properties,
            config_overrides,
            typed_fill,
            fill_overrides,
        })
    }

    pub(crate) const fn measurement_style(&self) -> &TextStyle {
        &self.measurement_style
    }

    pub(crate) const fn terminal_text_style(&self) -> &TextStyle {
        &self.terminal_text_style
    }

    pub(crate) const fn prepared_typography(&self) -> &ThemeTextStyle {
        &self.prepared_typography
    }

    pub(crate) const fn font_stack(&self) -> &ParsedCssFontStack {
        &self.font_stack
    }

    pub(crate) const fn resolved_style(&self) -> Option<&ResolvedThemeStyle> {
        self.resolved_style.as_ref()
    }

    pub(crate) const fn config_overrides(&self) -> &BTreeSet<ThemeTypographyProperty> {
        &self.config_overrides
    }

    pub(crate) const fn typed_properties(&self) -> &BTreeSet<ThemeTypographyProperty> {
        &self.typed_properties
    }

    pub(crate) const fn base_typed_properties(&self) -> &BTreeSet<ThemeTypographyProperty> {
        &self.base_typed_properties
    }

    pub(crate) fn typed_fill_for(&self, surface: SequenceTextSurface) -> Option<&str> {
        debug_assert_eq!(surface.role(), self.role);
        (!self.fill_overrides.contains(&surface))
            .then_some(self.typed_fill.as_deref())
            .flatten()
    }

    pub(crate) fn fill_overridden(&self, surface: SequenceTextSurface) -> bool {
        debug_assert_eq!(surface.role(), self.role);
        self.fill_overrides.contains(&surface)
    }

    pub(crate) fn has_typed_emission(&self) -> bool {
        !self.typed_properties.is_empty()
    }

    pub(crate) fn requires_resolved_emission(&self) -> bool {
        self.has_typed_emission()
            || !self.base_typed_properties.is_empty()
            || !self.config_overrides.is_empty()
    }

    pub(crate) fn requires_terminal_evidence_for(&self, surface: SequenceTextSurface) -> bool {
        debug_assert_eq!(surface.role(), self.role);
        self.has_typed_emission()
            || !self.base_typed_properties.is_empty()
            || self.typed_fill_for(surface).is_some()
    }

    pub(crate) fn inline_style(&self, base: &str) -> String {
        if !self.requires_resolved_emission() {
            return base.to_string();
        }
        let mut style = base.trim().trim_end_matches(';').to_string();
        append_typography_declarations(&mut style, &self.terminal_text_style);
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

    pub(crate) fn css_declarations_for(&self, surface: SequenceTextSurface) -> Option<String> {
        let typed_fill = self.typed_fill_for(surface);
        (self.requires_resolved_emission() || typed_fill.is_some()).then(|| {
            let mut declarations = String::new();
            if self.requires_resolved_emission() {
                append_typography_declarations(&mut declarations, &self.terminal_text_style);
                declarations.push(';');
            }
            if let Some(fill) = typed_fill {
                declarations.push_str("fill:");
                declarations.push_str(fill);
                declarations.push(';');
            }
            declarations
        })
    }

    pub(crate) fn fixed_size_terminal_style(&self) -> Option<String> {
        self.requires_resolved_emission().then(|| {
            let mut declarations = String::new();
            append_font_source_declarations(&mut declarations, &self.terminal_text_style);
            declarations.push(';');
            declarations
        })
    }
}

fn css_paint(paint: Option<&CanvasPaint>) -> Option<String> {
    match paint? {
        CanvasPaint::Transparent => Some("transparent".to_string()),
        CanvasPaint::Solid(color) => Some(color.as_css()),
        CanvasPaint::LinearGradient(_)
        | CanvasPaint::RadialGradient(_)
        | CanvasPaint::Pattern(_) => None,
    }
}

fn typed_static_fill(theme: &ResolvedDiagramTheme, style: &ResolvedThemeStyle) -> Option<String> {
    let origin = style.fill_resolution().winner()?;
    let facet = FamilyThemeRuleFacet::fill(style.fill_resolution().specified())?;
    if theme.rule_facet_disposition(origin.rule_index(), facet)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    css_paint(style.fill())
}

#[derive(Debug)]
pub(crate) struct SequenceTypographyPlan {
    base_typography: SequenceBaseTypography,
    actor: SequenceResolvedTypography,
    message: SequenceResolvedTypography,
    note: SequenceResolvedTypography,
    loop_label: SequenceResolvedTypography,
    terminal_foregrounds: [String; SequenceTextSurface::COUNT],
    terminal_foreground_provenance:
        [SequenceTerminalForegroundProvenance; SequenceTextSurface::COUNT],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SequenceTerminalForegroundProvenance {
    MermaidConfig,
    TypedTheme,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SequenceTerminalTextStyle<'a> {
    pub(crate) text_style: &'a TextStyle,
    pub(crate) foreground: &'a str,
    pub(crate) foreground_provenance: SequenceTerminalForegroundProvenance,
}

impl SequenceTypographyPlan {
    pub(crate) fn resolve(
        effective_config: &MermaidConfig,
        resolved_theme: Option<&ResolvedDiagramTheme>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let base_typography = SequenceBaseTypography::resolve(effective_config, resolved_theme);
        let actor = SequenceResolvedTypography::resolve(
            SequenceTypographyRole::Actor,
            effective_config,
            resolved_theme,
            work_meter,
            &base_typography,
        )?;
        let message = SequenceResolvedTypography::resolve(
            SequenceTypographyRole::Message,
            effective_config,
            resolved_theme,
            work_meter,
            &base_typography,
        )?;
        let note = SequenceResolvedTypography::resolve(
            SequenceTypographyRole::Note,
            effective_config,
            resolved_theme,
            work_meter,
            &base_typography,
        )?;
        let loop_label = SequenceResolvedTypography::resolve(
            SequenceTypographyRole::Loop,
            effective_config,
            resolved_theme,
            work_meter,
            &base_typography,
        )?;
        let mut terminal_foregrounds =
            crate::svg::render_theme::sequence_text_surface_fills(effective_config.as_value());
        let mut terminal_foreground_provenance =
            [SequenceTerminalForegroundProvenance::MermaidConfig; SequenceTextSurface::COUNT];
        for surface in SequenceTextSurface::ALL {
            let typography = match surface.role() {
                SequenceTypographyRole::Actor => &actor,
                SequenceTypographyRole::Message => &message,
                SequenceTypographyRole::Note => &note,
                SequenceTypographyRole::Loop => &loop_label,
            };
            if let Some(fill) = typography.typed_fill_for(surface) {
                terminal_foregrounds[surface.index()] = fill.to_owned();
                terminal_foreground_provenance[surface.index()] =
                    SequenceTerminalForegroundProvenance::TypedTheme;
            }
        }
        Ok(Self {
            base_typography,
            actor,
            message,
            note,
            loop_label,
            terminal_foregrounds,
            terminal_foreground_provenance,
        })
    }

    pub(crate) const fn inherited_font_stack(&self) -> &ParsedCssFontStack {
        &self.base_typography.font_stack
    }

    pub(crate) fn base_font_family_css(&self) -> &str {
        &self.base_typography.font_family_css
    }

    pub(crate) const fn base_font_size_px(&self) -> f64 {
        self.base_typography.font_size_px
    }

    pub(crate) fn base_prepared_typography(&self) -> ThemeTextStyle {
        ThemeTextStyle::default()
            .with_font_stack(self.base_typography.font_stack.font_stack().clone())
            .with_font_size_px(self.base_typography.font_size_px as f32)
            .expect("resolved Sequence base font size remains positive and finite")
    }

    pub(crate) const fn base_typed_properties(&self) -> &BTreeSet<ThemeTypographyProperty> {
        &self.base_typography.typed_properties
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

    pub(crate) fn terminal_text_style(
        &self,
        surface: SequenceTextSurface,
    ) -> SequenceTerminalTextStyle<'_> {
        SequenceTerminalTextStyle {
            text_style: self.role(surface.role()).terminal_text_style(),
            foreground: &self.terminal_foregrounds[surface.index()],
            foreground_provenance: self.terminal_foreground_provenance[surface.index()],
        }
    }
}

fn apply_base_property(
    text_style: &mut TextStyle,
    base: &SequenceBaseTypography,
    property: ThemeTypographyProperty,
) {
    match property {
        ThemeTypographyProperty::FontStack => {
            text_style.font_family = Some(base.font_family_css.clone());
        }
        ThemeTypographyProperty::FontSize => {
            text_style.font_size = base.font_size_px;
        }
        ThemeTypographyProperty::FontWeight
        | ThemeTypographyProperty::FontStyle
        | ThemeTypographyProperty::LineHeight
        | ThemeTypographyProperty::LetterSpacing
        | ThemeTypographyProperty::WordSpacing
        | ThemeTypographyProperty::Transform
        | ThemeTypographyProperty::Decoration
        | ThemeTypographyProperty::TextAlign
        | ThemeTypographyProperty::WhiteSpace
        | ThemeTypographyProperty::Wrap => {}
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

fn cssom_effective_text_style(
    measurement_style: &TextStyle,
    inherited_font_family: &str,
) -> (TextStyle, ParsedCssFontStack) {
    let mut terminal_text_style = measurement_style.clone();
    if let Some(font_stack) = measurement_style
        .font_family
        .as_deref()
        .and_then(parse_css_font_stack)
    {
        return (terminal_text_style, font_stack);
    }

    let font_stack = parse_css_font_stack(inherited_font_family)
        .expect("normalized Mermaid font-family CSS must remain parseable");
    terminal_text_style.font_family = Some(inherited_font_family.to_string());
    (terminal_text_style, font_stack)
}

fn prepared_typography(text_style: &TextStyle, font_stack: &ParsedCssFontStack) -> ThemeTextStyle {
    let mut prepared = ThemeTextStyle::default().with_font_stack(font_stack.font_stack().clone());
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
    prepared
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cssom_rejected_measurement_font_only_falls_back_for_terminal_text() {
        let measurement_style = TextStyle {
            font_family: Some("Legacy Root;".to_string()),
            ..TextStyle::default()
        };

        let (terminal_style, stack) =
            cssom_effective_text_style(&measurement_style, "Theme Sans,sans-serif");

        assert_eq!(
            measurement_style.font_family.as_deref(),
            Some("Legacy Root;")
        );
        assert_eq!(
            terminal_style.font_family.as_deref(),
            Some("Theme Sans,sans-serif")
        );
        assert_eq!(stack.families(), ["Theme Sans", "sans-serif"]);
    }

    #[test]
    fn cssom_valid_measurement_font_is_shared_with_terminal_text() {
        let measurement_style = TextStyle {
            font_family: Some("Root Sans,serif".to_string()),
            ..TextStyle::default()
        };

        let (terminal_style, stack) =
            cssom_effective_text_style(&measurement_style, "Theme Sans,sans-serif");

        assert_eq!(
            terminal_style.font_family.as_deref(),
            Some("Root Sans,serif")
        );
        assert_eq!(stack.families(), ["Root Sans", "serif"]);
    }
}
