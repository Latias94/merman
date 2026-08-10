use super::ThemeCompileValidationError;
use super::canvas::{CanvasPaint, CanvasSpec, ThemeColorValue};
use super::semantic::{
    OrdinalPalette, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use super::spec::DiagramThemeSpec;
use super::typography::{FontStack, TextStyle, TypographySpec};

/// Small design-system adapter. It accepts already-resolved host values and expands them into a
/// complete `DiagramThemeSpec`; it never reads CSS variables or stores raw selectors.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeTokens {
    canvas: ThemeColorValue,
    surface: ThemeColorValue,
    surface_alt: ThemeColorValue,
    surface_muted: ThemeColorValue,
    text: ThemeColorValue,
    subtle_text: ThemeColorValue,
    border: ThemeColorValue,
    line: ThemeColorValue,
    accent: ThemeColorValue,
    edge_label_background: ThemeColorValue,
    cluster_background: ThemeColorValue,
    cluster_border: ThemeColorValue,
    note_background: ThemeColorValue,
    note_border: ThemeColorValue,
    note_text: ThemeColorValue,
    actor_background: ThemeColorValue,
    actor_border: ThemeColorValue,
    actor_text: ThemeColorValue,
    activation_background: ThemeColorValue,
    activation_border: ThemeColorValue,
    error: ThemeColorValue,
    warning: ThemeColorValue,
    success: ThemeColorValue,
    series: Vec<ThemeColorValue>,
    typography: TypographySpec,
}

/// Frozen compatibility values retained only for the temporary family bridge.
///
/// Arbitrary theme specs cannot construct this record. It is emitted only by `ThemeTokens`, so
/// family-scoped rules never become cross-family Mermaid variables while built-in presets retain
/// their pre-cutover appearance on families that do not yet have typed consumers.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct FrozenLegacyThemeCompatibility {
    pub(super) canvas: ThemeColorValue,
    pub(super) surface: ThemeColorValue,
    pub(super) surface_alt: ThemeColorValue,
    pub(super) surface_muted: ThemeColorValue,
    pub(super) text: ThemeColorValue,
    pub(super) subtle_text: ThemeColorValue,
    pub(super) border: ThemeColorValue,
    pub(super) line: ThemeColorValue,
    pub(super) accent: ThemeColorValue,
    pub(super) edge_label_background: ThemeColorValue,
    pub(super) cluster_background: ThemeColorValue,
    pub(super) cluster_border: ThemeColorValue,
    pub(super) note_background: ThemeColorValue,
    pub(super) note_border: ThemeColorValue,
    pub(super) note_text: ThemeColorValue,
    pub(super) error: ThemeColorValue,
    pub(super) warning: ThemeColorValue,
    pub(super) success: ThemeColorValue,
    pub(super) series: Vec<ThemeColorValue>,
}

impl From<&ThemeTokens> for FrozenLegacyThemeCompatibility {
    fn from(tokens: &ThemeTokens) -> Self {
        Self {
            canvas: tokens.canvas.clone(),
            surface: tokens.surface.clone(),
            surface_alt: tokens.surface_alt.clone(),
            surface_muted: tokens.surface_muted.clone(),
            text: tokens.text.clone(),
            subtle_text: tokens.subtle_text.clone(),
            border: tokens.border.clone(),
            line: tokens.line.clone(),
            accent: tokens.accent.clone(),
            edge_label_background: tokens.edge_label_background.clone(),
            cluster_background: tokens.cluster_background.clone(),
            cluster_border: tokens.cluster_border.clone(),
            note_background: tokens.note_background.clone(),
            note_border: tokens.note_border.clone(),
            note_text: tokens.note_text.clone(),
            error: tokens.error.clone(),
            warning: tokens.warning.clone(),
            success: tokens.success.clone(),
            series: tokens.series.clone(),
        }
    }
}

impl Default for ThemeTokens {
    fn default() -> Self {
        Self {
            canvas: ThemeColorValue::parse("#ffffff").expect("static theme color"),
            surface: ThemeColorValue::parse("#f8fafc").expect("static theme color"),
            surface_alt: ThemeColorValue::parse("#e2e8f0").expect("static theme color"),
            surface_muted: ThemeColorValue::parse("#f1f5f9").expect("static theme color"),
            text: ThemeColorValue::parse("#0f172a").expect("static theme color"),
            subtle_text: ThemeColorValue::parse("#475569").expect("static theme color"),
            border: ThemeColorValue::parse("#94a3b8").expect("static theme color"),
            line: ThemeColorValue::parse("#64748b").expect("static theme color"),
            accent: ThemeColorValue::parse("#2563eb").expect("static theme color"),
            edge_label_background: ThemeColorValue::parse("#ffffff").expect("static theme color"),
            cluster_background: ThemeColorValue::parse("#f1f5f9").expect("static theme color"),
            cluster_border: ThemeColorValue::parse("#cbd5e1").expect("static theme color"),
            note_background: ThemeColorValue::parse("#fff7ed").expect("static theme color"),
            note_border: ThemeColorValue::parse("#fdba74").expect("static theme color"),
            note_text: ThemeColorValue::parse("#7c2d12").expect("static theme color"),
            actor_background: ThemeColorValue::parse("#f8fafc").expect("static theme color"),
            actor_border: ThemeColorValue::parse("#94a3b8").expect("static theme color"),
            actor_text: ThemeColorValue::parse("#0f172a").expect("static theme color"),
            activation_background: ThemeColorValue::parse("#e2e8f0").expect("static theme color"),
            activation_border: ThemeColorValue::parse("#94a3b8").expect("static theme color"),
            error: ThemeColorValue::parse("#dc2626").expect("static theme color"),
            warning: ThemeColorValue::parse("#d97706").expect("static theme color"),
            success: ThemeColorValue::parse("#059669").expect("static theme color"),
            series: ["#2563eb", "#16a34a", "#d97706", "#9333ea"]
                .into_iter()
                .map(|value| ThemeColorValue::parse(value).expect("static theme color"))
                .collect(),
            typography: TypographySpec::default().with_default(
                TextStyle::default().with_font_stack(
                    FontStack::new(["Inter", "ui-sans-serif", "system-ui", "sans-serif"])
                        .expect("static font stack"),
                ),
            ),
        }
    }
}

impl ThemeTokens {
    pub fn with_canvas(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.canvas = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_surface(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.surface = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_surface_alt(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.surface_alt = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_surface_muted(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.surface_muted = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_text(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.text = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_subtle_text(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.subtle_text = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_border(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.border = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_line(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.line = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_accent(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.accent = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_edge_label_background(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.edge_label_background = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_cluster_background(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.cluster_background = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_cluster_border(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.cluster_border = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_note_background(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.note_background = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_note_border(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.note_border = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_note_text(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.note_text = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_actor_background(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.actor_background = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_actor_border(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.actor_border = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_actor_text(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.actor_text = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_activation_background(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.activation_background = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_activation_border(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.activation_border = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_error(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.error = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_warning(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.warning = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_success(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.success = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_series(
        mut self,
        values: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.series = values
            .into_iter()
            .map(|value| ThemeColorValue::parse(value.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        if self.series.is_empty() {
            return Err(ThemeCompileValidationError::InvalidCollection {
                field: "tokens.series",
            });
        }
        Ok(self)
    }

    pub fn with_typography(mut self, typography: TypographySpec) -> Self {
        self.typography = typography;
        self
    }

    pub fn into_theme_spec(self) -> DiagramThemeSpec {
        let frozen_legacy_compatibility = FrozenLegacyThemeCompatibility::from(&self);
        let fill = |color: &ThemeColorValue| {
            ThemeStylePatch::default().with_fill(CanvasPaint::Solid(color.clone()))
        };
        let fill_stroke = |fill_color: &ThemeColorValue, stroke_color: &ThemeColorValue| {
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::Solid(fill_color.clone()))
                .with_stroke(CanvasPaint::Solid(stroke_color.clone()))
        };
        let text = |color: &ThemeColorValue| ThemeStylePatch {
            typography: super::typography::TextStylePatch {
                ..Default::default()
            },
            paint: super::semantic::ThemePaintPatch {
                fill: super::typography::Specified::Value(CanvasPaint::Solid(color.clone())),
                ..Default::default()
            },
            ..Default::default()
        };
        let styles = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(ThemeTarget::Text, text(&self.text)))
            .with_rule(ThemeRule::new(ThemeTarget::Title, text(&self.text)))
            .with_rule(ThemeRule::new(
                ThemeTarget::Node,
                fill_stroke(&self.surface, &self.border),
            ))
            .with_rule(ThemeRule::new(ThemeTarget::NodeLabel, text(&self.text)))
            .with_rule(ThemeRule::new(
                ThemeTarget::Edge,
                fill_stroke(&self.line, &self.line),
            ))
            .with_rule(ThemeRule::new(ThemeTarget::EdgeLabel, text(&self.text)))
            .with_rule(ThemeRule::new(
                ThemeTarget::EdgeLabelBackground,
                fill(&self.edge_label_background),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::Cluster,
                fill_stroke(&self.cluster_background, &self.cluster_border),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::ClusterLabel,
                text(&self.subtle_text),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::Actor,
                fill_stroke(&self.actor_background, &self.actor_border),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::ActorLabel,
                text(&self.actor_text),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::Lifeline,
                fill_stroke(&self.actor_border, &self.actor_border),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::Message,
                fill_stroke(&self.line, &self.line),
            ))
            .with_rule(ThemeRule::new(ThemeTarget::MessageLabel, text(&self.text)))
            .with_rule(ThemeRule::new(
                ThemeTarget::Loop,
                fill_stroke(&self.surface_alt, &self.border),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::LoopLabel,
                text(&self.actor_text),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::State,
                fill_stroke(&self.surface, &self.border),
            ))
            .with_rule(ThemeRule::new(ThemeTarget::StateLabel, text(&self.text)))
            .with_rule(ThemeRule::new(
                ThemeTarget::Transition,
                fill_stroke(&self.line, &self.line),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::TransitionMarker,
                fill_stroke(&self.line, &self.line),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::TransitionLabel,
                text(&self.text),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::TransitionLabelBackground,
                fill(&self.edge_label_background),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::Composite,
                fill_stroke(&self.canvas, &self.border),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::CompositeHeader,
                fill_stroke(&self.surface_alt, &self.border),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::CompositeLabel,
                text(&self.text),
            ))
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::SpecialState,
                    fill_stroke(&self.accent, &self.accent),
                )
                .with_variant(ThemeVariant::Special),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::SpecialStateInner,
                    fill_stroke(&self.canvas, &self.canvas),
                )
                .with_variant(ThemeVariant::End),
            )
            .with_rule(ThemeRule::new(
                ThemeTarget::Marker,
                fill_stroke(&self.accent, &self.accent),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::Note,
                fill_stroke(&self.note_background, &self.note_border),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::NoteLabel,
                text(&self.note_text),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::Activation,
                fill_stroke(&self.activation_background, &self.activation_border),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::Task,
                fill_stroke(&self.surface, &self.border),
            ))
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Task,
                    fill_stroke(&self.surface_muted, &self.line),
                )
                .with_variant(ThemeVariant::Active),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Task,
                    fill_stroke(&self.surface_alt, &self.error),
                )
                .with_variant(ThemeVariant::Error),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Task,
                    fill_stroke(&self.surface_alt, &self.warning),
                )
                .with_variant(ThemeVariant::Warning),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Task,
                    fill_stroke(&self.surface_alt, &self.success),
                )
                .with_variant(ThemeVariant::Success),
            )
            .with_rule(ThemeRule::new(
                ThemeTarget::Requirement,
                fill_stroke(&self.surface, &self.border),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::Relation,
                fill_stroke(&self.line, &self.line),
            ))
            .with_rule(
                ThemeRule::new(ThemeTarget::Table, fill(&self.surface))
                    .with_variant(ThemeVariant::Odd),
            )
            .with_rule(
                ThemeRule::new(ThemeTarget::Table, fill(&self.surface_alt))
                    .with_variant(ThemeVariant::Even),
            )
            .with_rule(ThemeRule::new(
                ThemeTarget::Axis,
                fill_stroke(&self.text, &self.line),
            ))
            .with_rule(ThemeRule::new(ThemeTarget::Legend, text(&self.subtle_text)))
            .with_ordinal_palette(
                ThemeTarget::Node,
                OrdinalPalette::new(self.series.clone())
                    .expect("ThemeTokens always contains a non-empty series palette"),
            )
            .with_ordinal_palette(
                ThemeTarget::Task,
                OrdinalPalette::new(self.series.clone())
                    .expect("ThemeTokens always contains a non-empty series palette"),
            )
            .with_ordinal_palette(
                ThemeTarget::ChartSeries,
                OrdinalPalette::new(self.series.clone())
                    .expect("ThemeTokens always contains a non-empty series palette"),
            )
            .with_ordinal_palette(
                ThemeTarget::PieSlice,
                OrdinalPalette::new(self.series.clone())
                    .expect("ThemeTokens always contains a non-empty series palette"),
            )
            .with_ordinal_palette(
                ThemeTarget::TimelineEvent,
                OrdinalPalette::new(self.series.clone())
                    .expect("ThemeTokens always contains a non-empty series palette"),
            )
            .with_ordinal_palette(
                ThemeTarget::JourneyTask,
                OrdinalPalette::new(self.series)
                    .expect("ThemeTokens always contains a non-empty series palette"),
            );

        let canvas = CanvasSpec::default().with_base(CanvasPaint::Solid(self.canvas));
        let typography = self.typography;
        DiagramThemeSpec::new()
            .with_typography(typography)
            .with_styles(styles)
            .with_canvas(canvas)
            .with_frozen_legacy_compatibility(frozen_legacy_compatibility)
    }

    pub fn typography(&self) -> &TypographySpec {
        &self.typography
    }

    pub fn default_font_stack() -> FontStack {
        FontStack::default()
    }

    pub fn default_text_style() -> TextStyle {
        TextStyle::default()
    }
}
