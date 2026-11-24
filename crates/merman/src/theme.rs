use std::sync::Arc;

use merman_model::family::DiagramFamilyId;
use merman_theme_contract::v2::SCHEMA_VERSION;
use serde::Serialize;

pub use merman_theme_contract::v2::{
    Appearance, BuiltinPreset, Category, Color, Paint, ResolvedThemeV2, ThemeCapabilityFlags,
    ThemeCompiler, ThemeDocumentV2, ThemeError, ThemeNotice,
};

/// Decides which theme to use when the source selects a Mermaid theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePrecedence {
    /// The selected native theme owns global visual defaults.
    #[default]
    Host,
    /// An authored Mermaid theme selects the Mermaid rendering path.
    Source,
}

/// The rendering mode actually used for a completed output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    MermaidParity,
    MermanNative,
}

/// A message from theme compilation or the final selection.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ThemeOutcomeNotice {
    Compilation(ThemeNotice),
    Rendering(&'static str),
}

/// Visual defaults used for a render and any notices relevant to the caller.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ThemeOutcome {
    pub mode: ThemeMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset_id: Option<BuiltinPreset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appearance: Option<Appearance>,
    pub notices: Vec<ThemeOutcomeNotice>,
}

/// Theme choices available to editors, bindings, and CLI callers.
#[derive(Debug, Serialize)]
pub struct ThemeCatalog {
    pub schema_version: u32,
    pub presets: Vec<ThemePresetEntry>,
    pub mermaid_themes: &'static [&'static str],
    pub families: Vec<FamilyThemeEntry>,
}

/// An editable built-in preset and its default appearance's six-color swatch.
#[derive(Debug, Serialize)]
pub struct ThemePresetEntry {
    pub id: BuiltinPreset,
    pub name: String,
    pub description: String,
    pub category: Category,
    pub appearances: Vec<Appearance>,
    pub default_appearance: Appearance,
    /// Canvas, node, container, title, primary text, and the first data fill.
    pub swatches: [Color; 6],
}

/// Native visual capabilities of a logical diagram family.
#[derive(Debug, Serialize)]
pub struct FamilyThemeEntry {
    pub id: DiagramFamilyId,
    pub capabilities: ThemeCapabilityFlags,
}

/// Lists built-in themes and the native visual capabilities supported by this build.
pub fn catalog() -> Result<ThemeCatalog, ThemeError> {
    let compiler = ThemeCompiler::new();
    let presets = BuiltinPreset::ALL
        .iter()
        .map(|&id| {
            let document = id.document()?;
            let theme = compiler.compile_preset(id, None)?;
            let roles = theme.roles();
            Ok(ThemePresetEntry {
                id,
                name: document.name().unwrap_or(id.as_str()).to_owned(),
                description: document.description().unwrap_or_default().to_owned(),
                category: document.category().unwrap_or(Category::Custom),
                appearances: document.appearances().collect(),
                default_appearance: theme.appearance(),
                swatches: [
                    roles.canvas.first_color(),
                    roles.surface_node.first_color(),
                    roles.surface_container.first_color(),
                    roles.text_title,
                    roles.text_primary,
                    theme.data().fill(0),
                ],
            })
        })
        .collect::<Result<_, ThemeError>>()?;
    let families = merman_model::family::FAMILY_CATALOG
        .iter()
        .filter(|family| family.selector.is_some())
        .map(|family| FamilyThemeEntry {
            id: family.id,
            #[cfg(feature = "svg")]
            capabilities: merman_render::theme::family_capabilities(family.id),
            #[cfg(not(feature = "svg"))]
            capabilities: ThemeCapabilityFlags::default(),
        })
        .collect();
    Ok(ThemeCatalog {
        schema_version: SCHEMA_VERSION,
        presets,
        mermaid_themes: crate::MermaidThemeId::NAMES,
        families,
    })
}

/// A native theme selected for a graphical or terminal render.
#[derive(Clone, Debug)]
pub struct ThemeSelection {
    input: ThemeInput,
    precedence: ThemePrecedence,
    background: Option<Color>,
}

#[derive(Clone, Debug)]
enum ThemeInput {
    Preset(BuiltinPreset, Option<Appearance>),
    Compiled(Arc<ResolvedThemeV2>),
}

impl ThemeSelection {
    /// Selects a built-in design and its default or explicit appearance.
    pub fn preset(preset: BuiltinPreset, appearance: Option<Appearance>) -> Self {
        Self {
            input: ThemeInput::Preset(preset, appearance),
            precedence: ThemePrecedence::Host,
            background: None,
        }
    }

    /// Selects a compiled design for reuse across diagrams.
    pub fn compiled(theme: Arc<ResolvedThemeV2>) -> Self {
        Self {
            input: ThemeInput::Compiled(theme),
            precedence: ThemePrecedence::Host,
            background: None,
        }
    }

    /// Controls the choice when the source also selects a Mermaid theme.
    pub const fn with_precedence(mut self, precedence: ThemePrecedence) -> Self {
        self.precedence = precedence;
        self
    }

    /// Replaces the native SVG canvas base with a host color.
    pub const fn with_background(mut self, background: Color) -> Self {
        self.background = Some(background);
        self
    }

    pub(crate) fn resolve(
        &self,
        source_config: &merman_model::MermaidConfig,
        diagram_type: &str,
    ) -> Result<Option<Arc<ResolvedThemeV2>>, ThemeError> {
        if self.prefer_source(source_config, diagram_type) {
            return Ok(None);
        }
        Ok(Some(match &self.input {
            ThemeInput::Preset(preset, appearance) => {
                ThemeCompiler::new().compile_preset(*preset, *appearance)?
            }
            ThemeInput::Compiled(theme) => Arc::clone(theme),
        }))
    }

    pub(crate) fn prefer_source(
        &self,
        source_config: &merman_model::MermaidConfig,
        diagram_type: &str,
    ) -> bool {
        self.precedence == ThemePrecedence::Source
            && merman_core::source_declares_theme(source_config, diagram_type)
    }

    #[cfg(feature = "svg")]
    pub(crate) fn outcome(&self, resolved: Option<&ResolvedThemeV2>) -> ThemeOutcome {
        match resolved {
            Some(resolved) => ThemeOutcome {
                mode: ThemeMode::MermanNative,
                preset_id: match self.input {
                    ThemeInput::Preset(preset, _) => Some(preset),
                    ThemeInput::Compiled(_) => None,
                },
                appearance: Some(resolved.appearance()),
                notices: resolved
                    .notices()
                    .iter()
                    .cloned()
                    .map(ThemeOutcomeNotice::Compilation)
                    .collect(),
            },
            None => ThemeOutcome {
                notices: vec![ThemeOutcomeNotice::Rendering(
                    "The source selected a Mermaid theme.",
                )],
                ..ThemeOutcome::default()
            },
        }
    }

    #[cfg(feature = "svg")]
    pub(crate) fn resolve_svg(
        &self,
        metadata: &merman_model::presentation::PresentationMetadata,
    ) -> Result<Option<merman_render::theme::NativeTheme>, ThemeError> {
        Ok(self
            .resolve(&metadata.config, &metadata.diagram_type)?
            .map(|resolved| {
                let theme = merman_render::theme::NativeTheme::new(resolved);
                match self.background {
                    Some(background) => theme.with_background(background),
                    None => theme,
                }
            }))
    }
}
