use merman_model::presentation::PresentationConfig;

use merman_render_core::{
    mermaid_style::{font_family_css, resolve_mermaid_absolute_font_size_px},
    text::{TextStyle, parse_css_font_stack},
};
use merman_theme_contract::v2::{Color, FamilyTheme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum SequenceTypographyRole {
    Actor,
    Message,
    Note,
    Loop,
}

impl SequenceTypographyRole {
    const fn config_role(self) -> super::config::SequenceTypographyConfigRole {
        match self {
            Self::Actor => super::config::SequenceTypographyConfigRole::Actor,
            Self::Message | Self::Loop => super::config::SequenceTypographyConfigRole::Message,
            Self::Note => super::config::SequenceTypographyConfigRole::Note,
        }
    }
}

/// SVG text surfaces sharing Sequence typography and foreground roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SequenceTextSurface {
    ParticipantLabel,
    BoxTitle,
    MessageLabel,
    NoteLabel,
    ControlKeyword,
    ControlPrimaryTitle,
    ControlSectionTitle,
}

impl SequenceTextSurface {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 7] = [
        Self::ParticipantLabel,
        Self::BoxTitle,
        Self::MessageLabel,
        Self::NoteLabel,
        Self::ControlKeyword,
        Self::ControlPrimaryTitle,
        Self::ControlSectionTitle,
    ];

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

    pub const fn terminal_selectors(self) -> &'static str {
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
pub struct SequenceResolvedTypography {
    measurement_style: TextStyle,
    terminal_text_style: TextStyle,
    explicit_style: bool,
    native: bool,
}

impl SequenceResolvedTypography {
    fn native(style: TextStyle) -> Self {
        Self {
            terminal_text_style: style.clone(),
            measurement_style: style,
            explicit_style: true,
            native: true,
        }
    }

    fn resolve(
        config: &super::config::SequenceConfigView<'_>,
        role: SequenceTypographyRole,
        inherited_family: &str,
    ) -> Self {
        let (measurement_style, explicit_style) = config
            .resolve_role_typography(role.config_role())
            .into_style();
        let terminal_text_style = cssom_effective_text_style(&measurement_style, inherited_family);
        Self {
            measurement_style,
            terminal_text_style,
            explicit_style,
            native: false,
        }
    }

    pub const fn measurement_style(&self) -> &TextStyle {
        &self.measurement_style
    }

    pub const fn terminal_text_style(&self) -> &TextStyle {
        &self.terminal_text_style
    }

    pub(crate) fn native_terminal_style(&self) -> Option<&TextStyle> {
        self.native.then_some(&self.terminal_text_style)
    }

    pub(crate) fn inline_style(&self, base: &str) -> String {
        if !self.explicit_style {
            return base.to_string();
        }
        let mut style = base.trim().trim_end_matches(';').to_string();
        append_typography_declarations(&mut style, &self.terminal_text_style);
        style.push(';');
        style
    }

    pub fn terminal_style(&self, typed_prefix: &str, legacy: String) -> String {
        if self.explicit_style {
            self.inline_style(typed_prefix)
        } else {
            legacy
        }
    }

    pub(crate) fn css_declarations(&self) -> Option<String> {
        self.explicit_style.then(|| self.inline_style(""))
    }

    pub fn fixed_size_terminal_style(&self) -> Option<String> {
        self.explicit_style.then(|| {
            let mut declarations = String::new();
            append_font_source_declarations(&mut declarations, &self.terminal_text_style);
            declarations.push(';');
            declarations
        })
    }
}

#[derive(Debug)]
pub struct SequenceTypographyPlan {
    compat: super::SequenceCompatBinding,
    root_font_size_px: f64,
    native_title: Option<TextStyle>,
    actor: SequenceResolvedTypography,
    message: SequenceResolvedTypography,
    note: SequenceResolvedTypography,
    loop_label: SequenceResolvedTypography,
}

#[derive(Debug, Clone, Copy)]
pub struct SequenceTerminalTextStyle<'a> {
    pub text_style: &'a TextStyle,
    pub foreground: &'a str,
}

impl SequenceTypographyPlan {
    pub fn for_native(
        config: &PresentationConfig,
        theme: &FamilyTheme,
        background: Option<Color>,
    ) -> Self {
        let compat = super::SequenceCompatBinding::for_native(config, theme, background);
        let typography = &theme.typography;
        let font_size = typography.base_size_px;
        let body = TextStyle {
            font_family: Some(compat.font_family.clone()),
            font_size,
            font_weight: Some(typography.label_weight.to_string()),
            font_style: None,
        };
        let title = TextStyle {
            font_family: Some(font_family_css(typography.stack(typography.title_family))),
            font_weight: Some(typography.title_weight.to_string()),
            ..body.clone()
        };
        Self {
            root_font_size_px: font_size,
            compat,
            native_title: Some(title),
            actor: SequenceResolvedTypography::native(body.clone()),
            message: SequenceResolvedTypography::native(body.clone()),
            note: SequenceResolvedTypography::native(body.clone()),
            loop_label: SequenceResolvedTypography::native(body),
        }
    }

    pub fn resolve(effective_config: &PresentationConfig) -> Self {
        let compat = super::SequenceCompatBinding::resolve(effective_config);
        let config = super::config::SequenceConfigView::from_mermaid_config(effective_config);
        let role = |role| SequenceResolvedTypography::resolve(&config, role, &compat.font_family);
        // The title inherits root CSS, independently of the message's inline font size.
        let root_font_size_px = resolve_mermaid_absolute_font_size_px(&compat.font_size_css)
            .unwrap_or(16.0)
            .max(1.0);
        Self {
            root_font_size_px,
            native_title: None,
            actor: role(SequenceTypographyRole::Actor),
            message: role(SequenceTypographyRole::Message),
            note: role(SequenceTypographyRole::Note),
            loop_label: role(SequenceTypographyRole::Loop),
            compat,
        }
    }

    pub fn compat_binding(&self) -> &super::SequenceCompatBinding {
        &self.compat
    }

    /// Root-inherited text, including the diagram title, has its own size owner.
    pub(crate) fn base_measurement_style(&self) -> TextStyle {
        if let Some(style) = &self.native_title {
            return style.clone();
        }
        TextStyle {
            font_family: Some(self.compat.font_family.clone()),
            font_size: self.root_font_size_px,
            font_weight: None,
            font_style: None,
        }
    }

    pub fn native_title_css(&self) -> Option<String> {
        self.native_title.as_ref().map(|style| {
            let mut css = String::new();
            append_typography_declarations(&mut css, style);
            css
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

    pub const fn actor(&self) -> &SequenceResolvedTypography {
        &self.actor
    }

    pub const fn message(&self) -> &SequenceResolvedTypography {
        &self.message
    }

    pub const fn note(&self) -> &SequenceResolvedTypography {
        &self.note
    }

    pub const fn loop_label(&self) -> &SequenceResolvedTypography {
        &self.loop_label
    }

    pub fn terminal_text_style(
        &self,
        surface: SequenceTextSurface,
    ) -> SequenceTerminalTextStyle<'_> {
        let role = self.role(surface.role());
        SequenceTerminalTextStyle {
            text_style: role.terminal_text_style(),
            foreground: self.compat.text_surface_fill(surface),
        }
    }
}

fn cssom_effective_text_style(
    measurement_style: &TextStyle,
    inherited_font_family: &str,
) -> TextStyle {
    let mut terminal_text_style = measurement_style.clone();
    if let Some(font_family) = super::sequence_inline_font_family(measurement_style)
        && parse_css_font_stack(&font_family).is_some()
    {
        terminal_text_style.font_family = Some(font_family);
        return terminal_text_style;
    }

    terminal_text_style.font_family = Some(inherited_font_family.to_string());
    terminal_text_style
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

        let terminal_style =
            cssom_effective_text_style(&measurement_style, "Theme Sans,sans-serif");

        assert_eq!(
            measurement_style.font_family.as_deref(),
            Some("Legacy Root;")
        );
        assert_eq!(
            terminal_style.font_family.as_deref(),
            Some("Theme Sans,sans-serif")
        );
    }

    #[test]
    fn cssom_terminal_font_serialization_falls_back_for_unrepresentable_eof_strings() {
        let measurement_style = TextStyle {
            font_family: Some("\"Quoted; Family".to_string()),
            ..TextStyle::default()
        };
        let terminal_style =
            cssom_effective_text_style(&measurement_style, "Theme Sans,sans-serif");
        assert_eq!(
            terminal_style.font_family.as_deref(),
            Some("Theme Sans,sans-serif")
        );
    }

    #[test]
    fn cssom_valid_measurement_font_is_shared_with_terminal_text() {
        let measurement_style = TextStyle {
            font_family: Some("Root Sans,serif".to_string()),
            ..TextStyle::default()
        };

        let terminal_style =
            cssom_effective_text_style(&measurement_style, "Theme Sans,sans-serif");

        assert_eq!(
            terminal_style.font_family.as_deref(),
            Some("Root Sans, serif")
        );
    }
}
