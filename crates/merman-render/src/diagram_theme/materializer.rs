use std::collections::BTreeMap;

use merman_theme_contract::{
    DiagramThemeSpecWireV1, MaterializedThemeWireV1, SpecifiedWireV1, ThemeCanvasPaintWireV1,
    ThemeCanvasSpecWireV1, ThemeColorTokenV1, ThemeDefinitionV1, ThemeLineHeightWireV1,
    ThemeMaterializationDiagnosticV1, ThemeMaterializationErrorV1, ThemeRuleSetWireV1,
    ThemeStrokePatchWireV1, ThemeStylePatchWireV1, ThemeTextStyleWireV1, ThemeTypographySpecWireV1,
};

use super::{
    FontStack, LineHeight, ThemeColorValue, ThemeResourcePolicy, ThemeTarget, ThemeTextStyle,
    ThemeVariant,
};

const DEFAULT_SERIES: [&str; 4] = ["#2563eb", "#16a34a", "#d97706", "#9333ea"];
const DEFAULT_FONT_STACK: [&str; 4] = ["Inter", "ui-sans-serif", "system-ui", "sans-serif"];
const DEFAULT_FONT_SIZE_PX: f32 = 16.0;
const DEFAULT_FONT_WEIGHT: u16 = 400;
pub(crate) const GENERATED_PALETTE_TARGETS: [ThemeTarget; 2] =
    [ThemeTarget::Node, ThemeTarget::PieSlice];

const COLOR_DEFAULTS: [(ThemeColorTokenV1, &str, &str); 8] = [
    (ThemeColorTokenV1::Canvas, "#ffffff", "/tokens/canvas"),
    (ThemeColorTokenV1::Surface, "#f8fafc", "/tokens/surface"),
    (
        ThemeColorTokenV1::SurfaceAlt,
        "#e2e8f0",
        "/tokens/surface_alt",
    ),
    (
        ThemeColorTokenV1::SurfaceMuted,
        "#f1f5f9",
        "/tokens/surface_muted",
    ),
    (ThemeColorTokenV1::Text, "#0f172a", "/tokens/text"),
    (ThemeColorTokenV1::Border, "#94a3b8", "/tokens/border"),
    (ThemeColorTokenV1::Line, "#64748b", "/tokens/line"),
    (ThemeColorTokenV1::Accent, "#2563eb", "/tokens/accent"),
];

#[derive(Debug, Clone, Copy)]
struct GeneratedRule {
    target: ThemeTarget,
    variant: Option<ThemeVariant>,
    fill: Option<ThemeColorTokenV1>,
    stroke: Option<ThemeColorTokenV1>,
}

const fn generated_rule(
    target: ThemeTarget,
    variant: Option<ThemeVariant>,
    fill: Option<ThemeColorTokenV1>,
    stroke: Option<ThemeColorTokenV1>,
) -> GeneratedRule {
    GeneratedRule {
        target,
        variant,
        fill,
        stroke,
    }
}

const GENERATED_RULES: [GeneratedRule; 23] = [
    generated_rule(ThemeTarget::Text, None, Some(ThemeColorTokenV1::Text), None),
    generated_rule(
        ThemeTarget::Title,
        None,
        Some(ThemeColorTokenV1::Text),
        None,
    ),
    generated_rule(
        ThemeTarget::Node,
        None,
        None,
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule(ThemeTarget::Edge, None, None, Some(ThemeColorTokenV1::Line)),
    generated_rule(
        ThemeTarget::Cluster,
        None,
        Some(ThemeColorTokenV1::SurfaceMuted),
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule(
        ThemeTarget::Actor,
        None,
        Some(ThemeColorTokenV1::Surface),
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule(
        ThemeTarget::Lifeline,
        None,
        None,
        Some(ThemeColorTokenV1::Line),
    ),
    generated_rule(
        ThemeTarget::Message,
        None,
        None,
        Some(ThemeColorTokenV1::Line),
    ),
    generated_rule(
        ThemeTarget::State,
        None,
        Some(ThemeColorTokenV1::Surface),
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule(
        ThemeTarget::StateLabel,
        None,
        Some(ThemeColorTokenV1::Text),
        None,
    ),
    generated_rule(
        ThemeTarget::Transition,
        None,
        None,
        Some(ThemeColorTokenV1::Line),
    ),
    generated_rule(
        ThemeTarget::TransitionMarker,
        None,
        Some(ThemeColorTokenV1::Line),
        Some(ThemeColorTokenV1::Line),
    ),
    generated_rule(
        ThemeTarget::TransitionLabel,
        None,
        Some(ThemeColorTokenV1::Text),
        None,
    ),
    generated_rule(
        ThemeTarget::TransitionLabelBackground,
        None,
        Some(ThemeColorTokenV1::Canvas),
        None,
    ),
    generated_rule(
        ThemeTarget::Composite,
        None,
        Some(ThemeColorTokenV1::Canvas),
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule(
        ThemeTarget::CompositeHeader,
        None,
        Some(ThemeColorTokenV1::SurfaceAlt),
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule(
        ThemeTarget::CompositeLabel,
        None,
        Some(ThemeColorTokenV1::Text),
        None,
    ),
    generated_rule(
        ThemeTarget::SpecialState,
        Some(ThemeVariant::Special),
        Some(ThemeColorTokenV1::Accent),
        Some(ThemeColorTokenV1::Accent),
    ),
    generated_rule(
        ThemeTarget::SpecialStateInner,
        Some(ThemeVariant::End),
        Some(ThemeColorTokenV1::Canvas),
        Some(ThemeColorTokenV1::Canvas),
    ),
    generated_rule(
        ThemeTarget::Note,
        None,
        Some(ThemeColorTokenV1::SurfaceAlt),
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule(
        ThemeTarget::NoteLabel,
        None,
        Some(ThemeColorTokenV1::Text),
        None,
    ),
    generated_rule(
        ThemeTarget::Activation,
        None,
        Some(ThemeColorTokenV1::SurfaceAlt),
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule(
        ThemeTarget::Entity,
        None,
        Some(ThemeColorTokenV1::Surface),
        Some(ThemeColorTokenV1::Border),
    ),
];
pub(crate) const MAX_AUTHORED_RULES: usize =
    super::semantic::MAX_THEME_RULES - GENERATED_RULES.len();

pub(super) const fn first_rejected_actual(max: usize) -> usize {
    max.saturating_add(1)
}

/// Resource-bounded versioned lowering from compact authoring input to a complete theme-spec wire.
#[derive(Debug, Clone)]
pub struct ThemeMaterializer {
    resources: ThemeResourcePolicy,
}

impl Default for ThemeMaterializer {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemeMaterializer {
    /// Creates the versioned materializer with the interactive resource policy.
    pub const fn new() -> Self {
        Self {
            resources: ThemeResourcePolicy::interactive(),
        }
    }

    /// Replaces the caller-owned resource policy used for typed and JSON admission.
    pub fn with_resource_policy(mut self, resources: ThemeResourcePolicy) -> Self {
        self.resources = resources;
        self
    }

    /// Admits and materializes one typed definition without compiling render-time capabilities.
    pub fn materialize_theme(
        &self,
        definition: &ThemeDefinitionV1,
    ) -> Result<MaterializedThemeWireV1, ThemeMaterializationErrorV1> {
        super::definition_admission::admit_typed_definition(&self.resources, definition)
            .map_err(super::definition_admission::admission_contract_error)?;
        self.materialize_admitted_theme(definition)
            .map_err(ThemeMaterializationError::into_contract_error)
    }

    /// Decodes, admits, and materializes one JSON definition without compiling it.
    pub fn materialize_theme_json(
        &self,
        bytes: &[u8],
    ) -> Result<MaterializedThemeWireV1, ThemeMaterializationErrorV1> {
        let definition = super::definition_admission::decode_bounded_theme_definition_json(
            &self.resources,
            bytes,
        )?;
        self.materialize_admitted_theme(&definition)
            .map_err(ThemeMaterializationError::into_contract_error)
    }

    pub(crate) fn materialize_admitted_theme(
        &self,
        definition: &ThemeDefinitionV1,
    ) -> Result<MaterializedThemeWireV1, ThemeMaterializationError> {
        validate_definition_shape(definition)?;
        let tokens = ResolvedTokensV1::resolve(definition)?;
        let mut styles = GENERATED_RULES
            .iter()
            .map(|row| materialize_generated_rule(*row, &tokens))
            .collect::<Vec<_>>();
        styles.extend(
            definition
                .styles()
                .iter()
                .filter(|entry| matches!(entry, ThemeRuleSetWireV1::Rule { .. }))
                .cloned(),
        );

        let mut palettes =
            GENERATED_PALETTE_TARGETS.map(|target| ThemeRuleSetWireV1::OrdinalPalette {
                target: target.id().to_owned(),
                colors: tokens.series.clone(),
            });
        let mut additional_palettes = Vec::new();
        for entry in definition.styles() {
            let ThemeRuleSetWireV1::OrdinalPalette { target, colors } = entry else {
                continue;
            };
            match GENERATED_PALETTE_TARGETS
                .iter()
                .position(|generated| generated.id() == target)
            {
                Some(index) => palettes[index] = entry.clone(),
                None => additional_palettes.push(ThemeRuleSetWireV1::OrdinalPalette {
                    target: target.clone(),
                    colors: colors.clone(),
                }),
            }
        }
        styles.extend(palettes);
        styles.extend(additional_palettes);

        let canvas = solid_color(tokens.color(ThemeColorTokenV1::Canvas));
        let spec = DiagramThemeSpecWireV1 {
            typography: Some(ThemeTypographySpecWireV1 {
                default: Some(tokens.typography),
                families: None,
            }),
            styles: Some(styles),
            canvas: Some(ThemeCanvasSpecWireV1 {
                base: Some(canvas),
                layers: None,
                bleed: None,
            }),
            ..DiagramThemeSpecWireV1::default()
        };
        MaterializedThemeWireV1::try_new(spec)
            .map_err(|_| ThemeMaterializationError::InvalidTokenValue { path: "/styles" })
    }
}

/// A fatal authoring error. Failures never carry a partial spec.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum ThemeMaterializationError {
    /// One token could not be lowered into its typed theme domain.
    #[error("theme authoring value at `{path}` is invalid")]
    InvalidTokenValue {
        /// RFC 6901 path to the rejected token.
        path: &'static str,
    },
    /// The explicitly authored ordinal series was empty.
    #[error("theme authoring series must not be empty")]
    EmptySeries,
    /// Two authored palettes targeted the same semantic role.
    #[error(
        "authored palette target `{target}` is duplicated at indices {first_authored_index} and {duplicate_authored_index}"
    )]
    DuplicatePaletteTarget {
        /// The duplicated raw semantic target identifier.
        target: String,
        /// Index of the first authored palette entry.
        first_authored_index: usize,
        /// Index of the duplicate authored palette entry.
        duplicate_authored_index: usize,
    },
    /// Generated and authored rules would exceed the complete-spec rule ceiling.
    #[error("authored rule count {actual} exceeds the version one maximum {max}")]
    RuleBudgetExceeded {
        /// Number of authored rules in the rejected definition.
        actual: usize,
        /// Maximum authored rules allowed by this expansion version.
        max: usize,
    },
    /// An authored rule named an effect graph absent from the version one envelope.
    #[error("authored rule at index {authored_index} references an unsupported effect graph")]
    EffectReferenceNotSupported {
        /// Index of the authored rule entry carrying the concrete reference.
        authored_index: usize,
    },
    /// Generated and authored ordinal palettes would exceed the complete-spec ceiling.
    #[error("materialized ordinal palette count {actual} exceeds the maximum {max}")]
    OrdinalPaletteBudgetExceeded {
        /// Number of palettes that the definition would materialize.
        actual: usize,
        /// Maximum palettes accepted by the complete theme-spec contract.
        max: usize,
    },
}

impl ThemeMaterializationError {
    pub(crate) fn into_contract_error(self) -> ThemeMaterializationErrorV1 {
        let diagnostic = match self {
            Self::InvalidTokenValue { path } => {
                ThemeMaterializationDiagnosticV1::invalid_token_value(
                    path,
                    expected_domain_id(path),
                    "theme authoring value is outside its expected domain",
                )
            }
            Self::EmptySeries => ThemeMaterializationDiagnosticV1::empty_series(
                "theme authoring series must not be empty",
            ),
            Self::DuplicatePaletteTarget {
                target,
                first_authored_index,
                duplicate_authored_index,
            } => ThemeMaterializationDiagnosticV1::duplicate_palette_target(
                format!("/styles/{duplicate_authored_index}/target"),
                target,
                first_authored_index,
                duplicate_authored_index,
                "authored palette target is duplicated",
            ),
            Self::RuleBudgetExceeded { actual, max } => {
                ThemeMaterializationDiagnosticV1::rule_budget_exceeded(
                    "max_authored_rules",
                    actual,
                    max,
                    "authored rule budget is exceeded",
                )
            }
            Self::EffectReferenceNotSupported { authored_index } => {
                ThemeMaterializationDiagnosticV1::effect_reference_not_supported(
                    format!("/styles/{authored_index}/style/effect"),
                    authored_index,
                    "effect references are not supported by authoring version one",
                )
            }
            Self::OrdinalPaletteBudgetExceeded { actual, max } => {
                ThemeMaterializationDiagnosticV1::resource_limit_exceeded(
                    "/styles",
                    "max_theme_ordinal_palettes",
                    actual,
                    max,
                    "materialized ordinal palette budget is exceeded",
                )
            }
        };
        ThemeMaterializationErrorV1::from_diagnostic(diagnostic)
    }
}

fn expected_domain_id(path: &str) -> &'static str {
    match path {
        "/tokens/canvas"
        | "/tokens/surface"
        | "/tokens/surface_alt"
        | "/tokens/surface_muted"
        | "/tokens/text"
        | "/tokens/border"
        | "/tokens/line"
        | "/tokens/accent" => "css-color",
        "/tokens/series" | "/styles/ordinal-palette/colors" => "theme-color-series",
        "/tokens/typography/font_stack" | "/styles/rule/style/typography/font_stack" => {
            "font-stack"
        }
        "/tokens/typography/font_size_px" => "positive-finite-pixels",
        "/tokens/typography/font_weight" => "font-weight-1-1000",
        "/tokens/typography/line_height" => "line-height",
        "/styles" => "finite-theme-style-values",
        _ => "theme-authoring-value",
    }
}

fn validate_authored_styles(
    styles: &[ThemeRuleSetWireV1],
) -> Result<(), ThemeMaterializationError> {
    let authored_rule_count = styles
        .iter()
        .filter(|entry| matches!(entry, ThemeRuleSetWireV1::Rule { .. }))
        .count();
    if authored_rule_count > MAX_AUTHORED_RULES {
        return Err(ThemeMaterializationError::RuleBudgetExceeded {
            actual: first_rejected_actual(MAX_AUTHORED_RULES),
            max: MAX_AUTHORED_RULES,
        });
    }

    let mut palette_targets = BTreeMap::<&str, usize>::new();
    let mut materialized_palette_count = GENERATED_PALETTE_TARGETS.len();
    for (authored_index, entry) in styles.iter().enumerate() {
        match entry {
            ThemeRuleSetWireV1::Rule { style, .. }
                if matches!(style.effect, SpecifiedWireV1::Value(_)) =>
            {
                return Err(ThemeMaterializationError::EffectReferenceNotSupported {
                    authored_index,
                });
            }
            ThemeRuleSetWireV1::OrdinalPalette { target, .. } => {
                if let Some(first_authored_index) = palette_targets.insert(target, authored_index) {
                    return Err(ThemeMaterializationError::DuplicatePaletteTarget {
                        target: target.clone(),
                        first_authored_index,
                        duplicate_authored_index: authored_index,
                    });
                }
                if !GENERATED_PALETTE_TARGETS
                    .iter()
                    .any(|generated| generated.id() == target)
                {
                    materialized_palette_count = materialized_palette_count.saturating_add(1);
                    if materialized_palette_count > super::semantic::MAX_THEME_ORDINAL_PALETTES {
                        return Err(ThemeMaterializationError::OrdinalPaletteBudgetExceeded {
                            actual: materialized_palette_count,
                            max: super::semantic::MAX_THEME_ORDINAL_PALETTES,
                        });
                    }
                }
            }
            ThemeRuleSetWireV1::Rule { .. } => {}
        }
    }
    Ok(())
}

pub(crate) fn validate_definition_shape(
    definition: &ThemeDefinitionV1,
) -> Result<(), ThemeMaterializationError> {
    validate_authored_styles(definition.styles())?;

    if let Some(series) = definition.tokens().series() {
        if series.is_empty() {
            return Err(ThemeMaterializationError::EmptySeries);
        }
        if series.len() > super::semantic::MAX_THEME_PALETTE_COLORS {
            return Err(ThemeMaterializationError::InvalidTokenValue {
                path: "/tokens/series",
            });
        }
    }

    if let Some(font_stack) = definition
        .tokens()
        .typography()
        .and_then(|typography| typography.font_stack())
        && !font_stack_shape_is_valid(font_stack)
    {
        return Err(ThemeMaterializationError::InvalidTokenValue {
            path: "/tokens/typography/font_stack",
        });
    }

    for entry in definition.styles() {
        match entry {
            ThemeRuleSetWireV1::Rule { style, .. } => {
                let Some(typography) = style.typography.as_ref() else {
                    continue;
                };
                let SpecifiedWireV1::Value(font_stack) = &typography.font_stack else {
                    continue;
                };
                if !font_stack_shape_is_valid(font_stack) {
                    return Err(ThemeMaterializationError::InvalidTokenValue {
                        path: "/styles/rule/style/typography/font_stack",
                    });
                }
            }
            ThemeRuleSetWireV1::OrdinalPalette { colors, .. }
                if colors.is_empty()
                    || colors.len() > super::semantic::MAX_THEME_PALETTE_COLORS =>
            {
                return Err(ThemeMaterializationError::InvalidTokenValue {
                    path: "/styles/ordinal-palette/colors",
                });
            }
            ThemeRuleSetWireV1::OrdinalPalette { .. } => {}
        }
    }

    Ok(())
}

fn font_stack_shape_is_valid(font_stack: &[String]) -> bool {
    !font_stack.is_empty()
        && font_stack.len() <= super::typography::MAX_FONT_STACK_ENTRIES
        && font_stack.iter().all(|family| {
            !family.is_empty() && family.len() <= super::typography::MAX_FONT_FAMILY_BYTES
        })
}

struct ResolvedTokensV1 {
    colors: [String; ThemeColorTokenV1::ALL.len()],
    series: Vec<String>,
    typography: ThemeTextStyleWireV1,
}

impl ResolvedTokensV1 {
    fn resolve(definition: &ThemeDefinitionV1) -> Result<Self, ThemeMaterializationError> {
        let authored = definition.tokens();
        let mut colors = Vec::with_capacity(COLOR_DEFAULTS.len());
        for (token, default, path) in COLOR_DEFAULTS {
            colors.push(resolve_color(
                authored.color(token).unwrap_or(default),
                path,
            )?);
        }
        let colors: [String; ThemeColorTokenV1::ALL.len()] = colors
            .try_into()
            .expect("the version one color table has a fixed width");

        let series_len = authored
            .series()
            .map_or(DEFAULT_SERIES.len(), <[String]>::len);
        if series_len == 0 {
            return Err(ThemeMaterializationError::EmptySeries);
        }
        if series_len > super::semantic::MAX_THEME_PALETTE_COLORS {
            return Err(ThemeMaterializationError::InvalidTokenValue {
                path: "/tokens/series",
            });
        }
        let series = match authored.series() {
            Some(series) => series
                .iter()
                .map(|color| resolve_color(color, "/tokens/series"))
                .collect::<Result<Vec<_>, _>>()?,
            None => DEFAULT_SERIES
                .iter()
                .map(|color| resolve_color(color, "/tokens/series"))
                .collect::<Result<Vec<_>, _>>()?,
        };

        let typography = resolve_typography(definition)?;
        Ok(Self {
            colors,
            series,
            typography,
        })
    }

    fn color(&self, token: ThemeColorTokenV1) -> &str {
        let index = COLOR_DEFAULTS
            .iter()
            .position(|(candidate, _, _)| *candidate == token)
            .expect("every version one color token has a default");
        &self.colors[index]
    }
}

fn resolve_color(value: &str, path: &'static str) -> Result<String, ThemeMaterializationError> {
    ThemeColorValue::parse(value)
        .map(|color| color.as_css())
        .map_err(|_| ThemeMaterializationError::InvalidTokenValue { path })
}

fn resolve_typography(
    definition: &ThemeDefinitionV1,
) -> Result<ThemeTextStyleWireV1, ThemeMaterializationError> {
    let authored = definition.tokens().typography();
    let font_stack = authored
        .and_then(|typography| typography.font_stack())
        .map(<[String]>::to_vec)
        .unwrap_or_else(|| DEFAULT_FONT_STACK.map(str::to_owned).to_vec());
    let font_size_px = authored
        .and_then(|typography| typography.font_size_px())
        .unwrap_or(DEFAULT_FONT_SIZE_PX);
    let font_weight = authored
        .and_then(|typography| typography.font_weight())
        .unwrap_or(DEFAULT_FONT_WEIGHT);
    let line_height = authored
        .and_then(|typography| typography.line_height())
        .cloned()
        .unwrap_or_else(|| ThemeLineHeightWireV1::Keyword("normal".to_owned()));

    let stack =
        FontStack::new(font_stack).map_err(|_| ThemeMaterializationError::InvalidTokenValue {
            path: "/tokens/typography/font_stack",
        })?;
    let internal_line_height = match &line_height {
        ThemeLineHeightWireV1::Keyword(value) if value == "normal" => LineHeight::Normal,
        ThemeLineHeightWireV1::Multiplier(value) => LineHeight::Multiplier(*value),
        ThemeLineHeightWireV1::Px { px } => LineHeight::Px(*px),
        ThemeLineHeightWireV1::Keyword(_) => {
            return Err(ThemeMaterializationError::InvalidTokenValue {
                path: "/tokens/typography/line_height",
            });
        }
    };
    let style = ThemeTextStyle::default().with_font_stack(stack.clone());
    let style = style.with_font_size_px(font_size_px).map_err(|_| {
        ThemeMaterializationError::InvalidTokenValue {
            path: "/tokens/typography/font_size_px",
        }
    })?;
    let style = style.with_font_weight(font_weight).map_err(|_| {
        ThemeMaterializationError::InvalidTokenValue {
            path: "/tokens/typography/font_weight",
        }
    })?;
    style.with_line_height(internal_line_height).map_err(|_| {
        ThemeMaterializationError::InvalidTokenValue {
            path: "/tokens/typography/line_height",
        }
    })?;

    Ok(ThemeTextStyleWireV1 {
        font_stack: Some(stack.families().to_vec()),
        font_size_px: Some(font_size_px),
        font_weight: Some(font_weight),
        line_height: Some(line_height),
        ..ThemeTextStyleWireV1::default()
    })
}

fn materialize_generated_rule(row: GeneratedRule, tokens: &ResolvedTokensV1) -> ThemeRuleSetWireV1 {
    let mut style = ThemeStylePatchWireV1::default();
    if let Some(fill) = row.fill {
        style.fill = SpecifiedWireV1::Value(solid_color(tokens.color(fill)));
    }
    if let Some(stroke) = row.stroke {
        style.stroke = Some(ThemeStrokePatchWireV1 {
            paint: SpecifiedWireV1::Value(solid_color(tokens.color(stroke))),
            ..ThemeStrokePatchWireV1::default()
        });
    }
    ThemeRuleSetWireV1::Rule {
        target: row.target.id().to_owned(),
        family: None,
        variant: row.variant.map(ThemeVariant::id).map(str::to_owned),
        ordinal: None,
        style,
    }
}

fn solid_color(color: &str) -> ThemeCanvasPaintWireV1 {
    ThemeCanvasPaintWireV1::Color(color.to_owned())
}

#[cfg(test)]
mod tests {
    use crate::DiagramFamilyId;

    use super::super::family_mechanism_matrix::{
        FamilyThemeDisposition, FamilyThemePaintKind, FamilyThemeRuleFacet,
        FamilyThemeSelectorShape, classify_rule_facet, compile_ordinal_palette_route,
    };
    use super::*;

    fn direct_consumer_for_generated_rule(row: GeneratedRule) -> Option<DiagramFamilyId> {
        let selector = FamilyThemeSelectorShape::Static {
            variant: row.variant,
        };
        DiagramFamilyId::all().iter().copied().find(|family| {
            if !row.target.valid_for(*family) {
                return false;
            }
            [
                FamilyThemePaintKind::Solid,
                FamilyThemePaintKind::Transparent,
            ]
            .into_iter()
            .all(|paint_kind| {
                let fill_is_direct = row.fill.is_none_or(|_| {
                    classify_rule_facet(
                        *family,
                        row.target,
                        selector,
                        FamilyThemeRuleFacet::Fill(paint_kind),
                    ) == FamilyThemeDisposition::TypedAdapter
                });
                let stroke_is_direct = row.stroke.is_none_or(|_| {
                    classify_rule_facet(
                        *family,
                        row.target,
                        selector,
                        FamilyThemeRuleFacet::Stroke(paint_kind),
                    ) == FamilyThemeDisposition::TypedAdapter
                });
                fill_is_direct && stroke_is_direct
            })
        })
    }

    fn direct_consumer_for_generated_palette(target: ThemeTarget) -> Option<DiagramFamilyId> {
        DiagramFamilyId::all().iter().copied().find(|family| {
            target.valid_for(*family)
                && compile_ordinal_palette_route(*family, target).disposition()
                    == FamilyThemeDisposition::TypedAdapter
        })
    }

    #[test]
    fn generated_expansion_rows_have_direct_consumers() {
        for row in GENERATED_RULES {
            let direct_family = direct_consumer_for_generated_rule(row);
            assert!(
                direct_family.is_some(),
                "{} generated rule lacks one family that directly consumes every emitted facet",
                row.target.id(),
            );
        }

        for target in GENERATED_PALETTE_TARGETS {
            let direct_family = direct_consumer_for_generated_palette(target);
            assert!(
                direct_family.is_some(),
                "{} palette lacks a direct consumer",
                target.id(),
            );
        }
    }

    #[test]
    fn invalid_family_targets_cannot_borrow_state_direct_classification() {
        let invalid_rule = generated_rule(
            ThemeTarget::Requirement,
            None,
            Some(ThemeColorTokenV1::Surface),
            None,
        );

        assert_eq!(direct_consumer_for_generated_rule(invalid_rule), None);
        assert_eq!(
            direct_consumer_for_generated_palette(ThemeTarget::Requirement),
            None
        );
    }
}
