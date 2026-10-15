//! User explanations projected from the final renderer-owned reports, not a second admission gate.

use super::{FamilyStyleReport, FamilyStyleVerification};
use crate::diagram_theme::source::{ThemeSourceDocument, ThemeStyleSource};
use crate::diagram_theme::{
    DiagramTheme, FamilyThemeMechanismKey, RootThemeMechanismKey, RootThemeReport, ThemeTarget,
    ThemeTypographyProperty,
};

/// An explanation of theme work that the completed render could not verify.
///
/// A `rule` diagnostic describes the rule as a whole. It does not say that all its properties
/// failed, nor identify a particular losing property. Admission remains the authority for whether
/// an output meets the requested policy; an empty diagnostic list is not a portability guarantee.
/// Identifiers are descriptive and may gain new values as renderer support evolves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeDiagnostic {
    code: &'static str,
    subject: &'static str,
    target: Option<ThemeTarget>,
    property: Option<&'static str>,
    source_document: Option<&'static str>,
    source_paths: Box<[String]>,
    generated: bool,
}

impl ThemeDiagnostic {
    fn new(code: &'static str, subject: &'static str, target: Option<ThemeTarget>) -> Self {
        Self {
            code,
            subject,
            target,
            property: None,
            source_document: None,
            source_paths: Box::default(),
            generated: false,
        }
    }

    fn with_source(
        mut self,
        theme: Option<&DiagramTheme>,
        source: Option<&ThemeStyleSource>,
    ) -> Self {
        if let Some(source) = source {
            self.source_document = theme
                .and_then(|theme| theme.source_map().document())
                .map(|doc| doc.id());
            self.source_paths = source.paths.clone().into_boxed_slice();
            self.generated = source.generated;
        }
        self
    }

    fn with_spec_path(mut self, theme: Option<&DiagramTheme>, path: String) -> Self {
        if matches!(
            theme.and_then(|theme| theme.source_map().document()),
            Some(ThemeSourceDocument::CompleteSpec)
        ) {
            self.source_document = Some("complete_spec");
            self.source_paths = vec![path].into_boxed_slice();
        }
        self
    }

    pub const fn code(&self) -> &'static str {
        self.code
    }
    pub const fn subject(&self) -> &'static str {
        self.subject
    }
    pub const fn target(&self) -> Option<ThemeTarget> {
        self.target
    }
    /// Present only when the report identifies a specific property, such as base typography.
    pub const fn property(&self) -> Option<&'static str> {
        self.property
    }
    /// Payload to which `source_paths` are relative: `definition` or `complete_spec`.
    /// Rust-constructed themes and built-in selections have no original JSON document.
    pub const fn source_document(&self) -> Option<&'static str> {
        self.source_document
    }
    /// RFC 6901 locations in the input payload, not compiled-rule indices or SVG selectors.
    /// Generated defaults have no input location. Multiple locations may contribute to a rule.
    pub fn source_paths(&self) -> &[String] {
        &self.source_paths
    }
    /// Whether this diagnostic concerns an authoring expansion rather than an authored style entry.
    pub const fn generated(&self) -> bool {
        self.generated
    }
}

pub(super) fn project(
    root: &RootThemeReport,
    family: &FamilyStyleReport,
    theme: Option<&DiagramTheme>,
) -> Box<[ThemeDiagnostic]> {
    let mut diagnostics = Vec::new();
    for residual in root.residuals() {
        let code = residual.reason().id();
        let diagnostic = match residual.key() {
            RootThemeMechanismKey::CanvasRule { index } => {
                rule(code, ThemeTarget::Canvas, *index, theme)
            }
            RootThemeMechanismKey::CanvasOrdinalPalette => {
                palette(code, ThemeTarget::Canvas, theme)
            }
            RootThemeMechanismKey::CanvasBase => {
                ThemeDiagnostic::new(code, "canvas", Some(ThemeTarget::Canvas)).with_source(
                    theme,
                    theme.and_then(|theme| theme.source_map().canvas_base()),
                )
            }
            RootThemeMechanismKey::CanvasLayer { index } => {
                ThemeDiagnostic::new(code, "canvas", Some(ThemeTarget::Canvas))
                    .with_spec_path(theme, format!("/canvas/layers/{index}"))
            }
            RootThemeMechanismKey::CanvasGeometry => {
                ThemeDiagnostic::new(code, "canvas", Some(ThemeTarget::Canvas))
                    .with_spec_path(theme, "/canvas/bleed".to_owned())
            }
            RootThemeMechanismKey::EffectBinding { target, .. } => {
                effect_binding(code, *target, theme)
            }
        };
        diagnostics.push(diagnostic);
    }
    if !root.coverage_complete() {
        diagnostics.push(ThemeDiagnostic::new(
            "evidence-incomplete",
            "root-evidence",
            None,
        ));
    }
    for residual in &family.theme_residuals {
        let code = residual.reason().id();
        let diagnostic = match residual.key() {
            FamilyThemeMechanismKey::Rule { index, target } => rule(code, *target, *index, theme),
            FamilyThemeMechanismKey::OrdinalPalette { target } => palette(code, *target, theme),
            FamilyThemeMechanismKey::EffectBinding { target, .. } => {
                effect_binding(code, *target, theme)
            }
            FamilyThemeMechanismKey::Typography(property) => {
                let mut diagnostic = ThemeDiagnostic::new(code, "typography", None);
                diagnostic.property = Some(property.id());
                if let Some(theme) = theme {
                    match theme.source_map().document() {
                        Some(ThemeSourceDocument::CompleteSpec) => {
                            let has_family = theme
                                .spec()
                                .typography()
                                .family_overrides()
                                .any(|(id, _)| id == family.family_id.as_str());
                            let prefix = if has_family {
                                format!("/typography/families/{}", family.family_id.as_str())
                            } else {
                                "/typography/default".to_owned()
                            };
                            diagnostic = diagnostic.with_spec_path(
                                Some(theme),
                                format!("{prefix}/{}", typography_field(*property)),
                            );
                        }
                        Some(ThemeSourceDocument::Definition) => {
                            // Expansion owns the typography defaults. Do not manufacture a token
                            // location when that token may have been omitted by the author.
                            diagnostic.source_document = Some("definition");
                            diagnostic.generated = true;
                        }
                        None => {}
                    }
                }
                diagnostic
            }
        };
        diagnostics.push(diagnostic);
    }
    if !family.theme_coverage_complete() {
        diagnostics.push(ThemeDiagnostic::new(
            "evidence-incomplete",
            "family-evidence",
            None,
        ));
    }
    if family.evaluation == super::FamilyStyleEvaluation::Unadapted {
        diagnostics.push(ThemeDiagnostic::new(
            "family-unadapted",
            "family-evidence",
            None,
        ));
    }
    if family.output_mutated {
        diagnostics.push(ThemeDiagnostic::new(
            "output-mutation",
            "family-evidence",
            None,
        ));
    }
    // These reports can prevent verification without a typed-rule residual. Explain that boundary
    // without misattributing Mermaid/source declarations to the theme JSON.
    if !family.residuals.is_empty() {
        diagnostics.push(ThemeDiagnostic::new(
            "source-style-unverified",
            "source-style",
            None,
        ));
    }
    if family.mermaid_compatibility_residual_count != 0 {
        diagnostics.push(ThemeDiagnostic::new(
            "compatibility-unverified",
            "compatibility",
            None,
        ));
    }
    debug_assert!(
        family.verification() != FamilyStyleVerification::Incomplete || !diagnostics.is_empty()
    );
    diagnostics.into_boxed_slice()
}

fn rule(
    code: &'static str,
    target: ThemeTarget,
    index: usize,
    theme: Option<&DiagramTheme>,
) -> ThemeDiagnostic {
    ThemeDiagnostic::new(code, "rule", Some(target)).with_source(
        theme,
        theme.and_then(|theme| theme.source_map().rule(index)),
    )
}

fn palette(
    code: &'static str,
    target: ThemeTarget,
    theme: Option<&DiagramTheme>,
) -> ThemeDiagnostic {
    ThemeDiagnostic::new(code, "ordinal-palette", Some(target)).with_source(
        theme,
        theme.and_then(|theme| theme.source_map().palette(target)),
    )
}

fn effect_binding(
    code: &'static str,
    target: ThemeTarget,
    theme: Option<&DiagramTheme>,
) -> ThemeDiagnostic {
    // Graph and binding entries share an input array and are split by the decoder. Until that
    // owner retains an exact binding location, identify the container rather than invent an index.
    ThemeDiagnostic::new(code, "effect-binding", Some(target))
        .with_spec_path(theme, "/effects".to_owned())
}

fn typography_field(property: ThemeTypographyProperty) -> &'static str {
    match property {
        ThemeTypographyProperty::FontStack => "font_stack",
        ThemeTypographyProperty::FontSize => "font_size_px",
        ThemeTypographyProperty::FontWeight => "font_weight",
        ThemeTypographyProperty::FontStyle => "font_style",
        ThemeTypographyProperty::LineHeight => "line_height",
        ThemeTypographyProperty::LetterSpacing => "letter_spacing_px",
        ThemeTypographyProperty::WordSpacing => "word_spacing_px",
        ThemeTypographyProperty::Transform => "transform",
        ThemeTypographyProperty::Decoration => "decoration",
        ThemeTypographyProperty::TextAlign => "text_align",
        ThemeTypographyProperty::WhiteSpace => "white_space",
        ThemeTypographyProperty::Wrap => "wrap",
    }
}
