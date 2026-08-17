use std::collections::BTreeMap;
use std::fmt;

use merman_theme_contract::{
    CanonicalJsonError, DiagramThemeSpecWireV1, SpecifiedWireV1, ThemeCanvasPaintWireV1,
    ThemeCanvasSpecWireV1, ThemeColorTokenV1, ThemeDefinitionV1, ThemeLineHeightWireV1,
    ThemeRuleSetWireV1, ThemeStrokePatchWireV1, ThemeStylePatchWireV1, ThemeTextStyleWireV1,
    ThemeTypographySpecWireV1,
};
use sha2::{Digest as _, Sha256};

use super::{FontStack, LineHeight, ThemeColorValue, ThemeTextStyle};

const DEFAULT_SERIES: [&str; 4] = ["#2563eb", "#16a34a", "#d97706", "#9333ea"];
const DEFAULT_FONT_STACK: [&str; 4] = ["Inter", "ui-sans-serif", "system-ui", "sans-serif"];
const DEFAULT_FONT_SIZE_PX: f32 = 16.0;
const DEFAULT_FONT_WEIGHT: u16 = 400;
const MATERIALIZATION_DIGEST_DOMAIN: &[u8] = b"merman.theme-materialization.v1\0";
pub(crate) const GENERATED_PALETTE_TARGETS: [&str; 2] = ["node", "pie-slice"];

const COLOR_DEFAULTS: [(ThemeColorTokenV1, &str, &str); 12] = [
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
    (
        ThemeColorTokenV1::SubtleText,
        "#475569",
        "/tokens/subtle_text",
    ),
    (ThemeColorTokenV1::Border, "#94a3b8", "/tokens/border"),
    (ThemeColorTokenV1::Line, "#64748b", "/tokens/line"),
    (ThemeColorTokenV1::Accent, "#2563eb", "/tokens/accent"),
    (ThemeColorTokenV1::Error, "#dc2626", "/tokens/error"),
    (ThemeColorTokenV1::Warning, "#d97706", "/tokens/warning"),
    (ThemeColorTokenV1::Success, "#059669", "/tokens/success"),
];

#[derive(Debug, Clone, Copy)]
struct GeneratedRule {
    target: &'static str,
    variant: Option<&'static str>,
    fill: Option<ThemeColorTokenV1>,
    stroke: Option<ThemeColorTokenV1>,
}

const fn generated_rule(
    target: &'static str,
    variant: Option<&'static str>,
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

const GENERATED_RULES: [GeneratedRule; 40] = [
    generated_rule("text", None, Some(ThemeColorTokenV1::Text), None),
    generated_rule("title", None, Some(ThemeColorTokenV1::Text), None),
    generated_rule("node", None, None, Some(ThemeColorTokenV1::Border)),
    generated_rule("node-label", None, Some(ThemeColorTokenV1::Text), None),
    generated_rule(
        "edge",
        None,
        Some(ThemeColorTokenV1::Line),
        Some(ThemeColorTokenV1::Line),
    ),
    generated_rule("edge-label", None, Some(ThemeColorTokenV1::Text), None),
    generated_rule(
        "edge-label-background",
        None,
        Some(ThemeColorTokenV1::Canvas),
        None,
    ),
    generated_rule(
        "cluster",
        None,
        Some(ThemeColorTokenV1::SurfaceMuted),
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule(
        "cluster-label",
        None,
        Some(ThemeColorTokenV1::SubtleText),
        None,
    ),
    generated_rule(
        "actor",
        None,
        Some(ThemeColorTokenV1::Surface),
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule("actor-label", None, Some(ThemeColorTokenV1::Text), None),
    generated_rule(
        "lifeline",
        None,
        Some(ThemeColorTokenV1::Line),
        Some(ThemeColorTokenV1::Line),
    ),
    generated_rule(
        "message",
        None,
        Some(ThemeColorTokenV1::Line),
        Some(ThemeColorTokenV1::Line),
    ),
    generated_rule("message-label", None, Some(ThemeColorTokenV1::Text), None),
    generated_rule(
        "loop",
        None,
        Some(ThemeColorTokenV1::SurfaceAlt),
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule("loop-label", None, Some(ThemeColorTokenV1::Text), None),
    generated_rule(
        "state",
        None,
        Some(ThemeColorTokenV1::Surface),
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule("state-label", None, Some(ThemeColorTokenV1::Text), None),
    generated_rule(
        "transition",
        None,
        Some(ThemeColorTokenV1::Line),
        Some(ThemeColorTokenV1::Line),
    ),
    generated_rule(
        "transition-marker",
        None,
        Some(ThemeColorTokenV1::Line),
        Some(ThemeColorTokenV1::Line),
    ),
    generated_rule(
        "transition-label",
        None,
        Some(ThemeColorTokenV1::Text),
        None,
    ),
    generated_rule(
        "transition-label-background",
        None,
        Some(ThemeColorTokenV1::Canvas),
        None,
    ),
    generated_rule(
        "composite",
        None,
        Some(ThemeColorTokenV1::Canvas),
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule(
        "composite-header",
        None,
        Some(ThemeColorTokenV1::SurfaceAlt),
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule("composite-label", None, Some(ThemeColorTokenV1::Text), None),
    generated_rule(
        "special-state",
        Some("special"),
        Some(ThemeColorTokenV1::Accent),
        Some(ThemeColorTokenV1::Accent),
    ),
    generated_rule(
        "special-state-inner",
        Some("end"),
        Some(ThemeColorTokenV1::Canvas),
        Some(ThemeColorTokenV1::Canvas),
    ),
    generated_rule(
        "marker",
        None,
        Some(ThemeColorTokenV1::Accent),
        Some(ThemeColorTokenV1::Accent),
    ),
    generated_rule(
        "note",
        None,
        Some(ThemeColorTokenV1::SurfaceAlt),
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule("note-label", None, Some(ThemeColorTokenV1::Text), None),
    generated_rule(
        "activation",
        None,
        Some(ThemeColorTokenV1::SurfaceAlt),
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule(
        "task",
        None,
        Some(ThemeColorTokenV1::Surface),
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule(
        "task",
        Some("active"),
        Some(ThemeColorTokenV1::SurfaceMuted),
        Some(ThemeColorTokenV1::Line),
    ),
    generated_rule(
        "task",
        Some("error"),
        Some(ThemeColorTokenV1::SurfaceAlt),
        Some(ThemeColorTokenV1::Error),
    ),
    generated_rule(
        "task",
        Some("warning"),
        Some(ThemeColorTokenV1::SurfaceAlt),
        Some(ThemeColorTokenV1::Warning),
    ),
    generated_rule(
        "task",
        Some("success"),
        Some(ThemeColorTokenV1::SurfaceAlt),
        Some(ThemeColorTokenV1::Success),
    ),
    generated_rule(
        "requirement",
        None,
        Some(ThemeColorTokenV1::Surface),
        Some(ThemeColorTokenV1::Border),
    ),
    generated_rule(
        "relation",
        None,
        Some(ThemeColorTokenV1::Line),
        Some(ThemeColorTokenV1::Line),
    ),
    generated_rule("table", Some("odd"), Some(ThemeColorTokenV1::Surface), None),
    generated_rule(
        "table",
        Some("even"),
        Some(ThemeColorTokenV1::SurfaceAlt),
        None,
    ),
];
pub(crate) const MAX_AUTHORED_RULES: usize =
    super::semantic::MAX_THEME_RULES - GENERATED_RULES.len();

/// Pure versioned lowering from a compact authoring definition to a complete theme-spec wire.
#[derive(Debug, Default, Clone, Copy)]
pub struct ThemeMaterializer;

impl ThemeMaterializer {
    /// Creates the versioned materializer.
    pub const fn new() -> Self {
        Self
    }

    /// Applies versioned defaults and expansion without inspecting render-time capabilities.
    pub fn materialize_theme(
        &self,
        definition: &ThemeDefinitionV1,
    ) -> Result<MaterializedTheme, ThemeMaterializationError> {
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
                target: target.to_owned(),
                colors: tokens.series.clone(),
            });
        let mut additional_palettes = Vec::new();
        for entry in definition.styles() {
            let ThemeRuleSetWireV1::OrdinalPalette { target, colors } = entry else {
                continue;
            };
            match GENERATED_PALETTE_TARGETS
                .iter()
                .position(|generated| *generated == target.as_str())
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
        let digest = ThemeMaterializationDigest::new(definition, &spec)?;
        Ok(MaterializedTheme {
            authoring_schema_version: definition.authoring_schema_version(),
            expansion_version: definition.expansion_version(),
            spec_schema_version: DiagramThemeSpecWireV1::spec_schema_version(),
            spec,
            theme_materialization_digest: digest,
        })
    }
}

/// One successfully materialized versioned authoring definition.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterializedTheme {
    authoring_schema_version: u32,
    expansion_version: u32,
    spec_schema_version: u32,
    spec: DiagramThemeSpecWireV1,
    theme_materialization_digest: ThemeMaterializationDigest,
}

impl MaterializedTheme {
    /// Returns the source authoring schema version.
    pub const fn authoring_schema_version(&self) -> u32 {
        self.authoring_schema_version
    }

    /// Returns the deterministic expansion-table version.
    pub const fn expansion_version(&self) -> u32 {
        self.expansion_version
    }

    /// Returns the complete-spec schema version.
    pub const fn spec_schema_version(&self) -> u32 {
        self.spec_schema_version
    }

    /// Borrows the complete editable theme-spec wire.
    pub const fn spec(&self) -> &DiagramThemeSpecWireV1 {
        &self.spec
    }

    /// Consumes the result and returns the complete editable theme-spec wire.
    pub fn into_spec(self) -> DiagramThemeSpecWireV1 {
        self.spec
    }

    /// Returns the replay identity of the version tuple and complete canonical spec.
    pub const fn theme_materialization_digest(&self) -> ThemeMaterializationDigest {
        self.theme_materialization_digest
    }
}

/// Replay identity for one versioned theme materialization result.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThemeMaterializationDigest([u8; 32]);

impl ThemeMaterializationDigest {
    fn new(
        definition: &ThemeDefinitionV1,
        spec: &DiagramThemeSpecWireV1,
    ) -> Result<Self, CanonicalJsonError> {
        let canonical_spec = spec.canonical_json_bytes()?;
        let mut hasher = Sha256::new();
        hasher.update(MATERIALIZATION_DIGEST_DOMAIN);
        hasher.update(definition.authoring_schema_version().to_be_bytes());
        hasher.update(definition.expansion_version().to_be_bytes());
        hasher.update(DiagramThemeSpecWireV1::spec_schema_version().to_be_bytes());
        hasher.update((canonical_spec.len() as u64).to_be_bytes());
        hasher.update(canonical_spec);
        Ok(Self(hasher.finalize().into()))
    }

    /// Returns the fixed-width digest bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Formats the digest as lower-case hexadecimal.
    pub fn to_hex(self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

impl fmt::Debug for ThemeMaterializationDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ThemeMaterializationDigest")
            .field(&self.to_hex())
            .finish()
    }
}

/// A fatal authoring error. Failures never carry a partial spec.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ThemeMaterializationError {
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
    /// Canonical result serialization failed, so no replay identity can be issued.
    #[error(transparent)]
    Canonicalization(#[from] CanonicalJsonError),
}

impl ThemeMaterializationError {
    /// Returns the stable diagnostic code for this fatal error.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidTokenValue { .. } => "theme-authoring.invalid-token-value",
            Self::EmptySeries => "theme-authoring.empty-series",
            Self::DuplicatePaletteTarget { .. } => "theme-authoring.duplicate-palette-target",
            Self::RuleBudgetExceeded { .. } => "theme-authoring.rule-budget-exceeded",
            Self::EffectReferenceNotSupported { .. } => {
                "theme-authoring.effect-reference-not-supported"
            }
            Self::OrdinalPaletteBudgetExceeded { .. } => "theme-authoring.resource-limit-exceeded",
            Self::Canonicalization(_) => "theme-authoring.invalid-token-value",
        }
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
            actual: authored_rule_count,
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
                if !GENERATED_PALETTE_TARGETS.contains(&target.as_str()) {
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
    colors: [String; 12],
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
        let colors: [String; 12] = colors
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
        &self.colors[color_index(token)]
    }
}

const fn color_index(token: ThemeColorTokenV1) -> usize {
    match token {
        ThemeColorTokenV1::Canvas => 0,
        ThemeColorTokenV1::Surface => 1,
        ThemeColorTokenV1::SurfaceAlt => 2,
        ThemeColorTokenV1::SurfaceMuted => 3,
        ThemeColorTokenV1::Text => 4,
        ThemeColorTokenV1::SubtleText => 5,
        ThemeColorTokenV1::Border => 6,
        ThemeColorTokenV1::Line => 7,
        ThemeColorTokenV1::Accent => 8,
        ThemeColorTokenV1::Error => 9,
        ThemeColorTokenV1::Warning => 10,
        ThemeColorTokenV1::Success => 11,
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
        target: row.target.to_owned(),
        family: None,
        variant: row.variant.map(str::to_owned),
        ordinal: None,
        style,
    }
}

fn solid_color(color: &str) -> ThemeCanvasPaintWireV1 {
    ThemeCanvasPaintWireV1::Color(color.to_owned())
}
