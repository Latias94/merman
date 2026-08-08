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
    text: ThemeColorValue,
    subtle_text: ThemeColorValue,
    border: ThemeColorValue,
    line: ThemeColorValue,
    accent: ThemeColorValue,
    note: ThemeColorValue,
    actor: ThemeColorValue,
    activation: ThemeColorValue,
    series: Vec<ThemeColorValue>,
    typography: TypographySpec,
}

impl Default for ThemeTokens {
    fn default() -> Self {
        Self {
            canvas: ThemeColorValue::parse("#ffffff").expect("static theme color"),
            surface: ThemeColorValue::parse("#f8fafc").expect("static theme color"),
            surface_alt: ThemeColorValue::parse("#f1f5f9").expect("static theme color"),
            text: ThemeColorValue::parse("#1e293b").expect("static theme color"),
            subtle_text: ThemeColorValue::parse("#64748b").expect("static theme color"),
            border: ThemeColorValue::parse("#cbd5e1").expect("static theme color"),
            line: ThemeColorValue::parse("#64748b").expect("static theme color"),
            accent: ThemeColorValue::parse("#2563eb").expect("static theme color"),
            note: ThemeColorValue::parse("#fef3c7").expect("static theme color"),
            actor: ThemeColorValue::parse("#e2e8f0").expect("static theme color"),
            activation: ThemeColorValue::parse("#cbd5e1").expect("static theme color"),
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

    pub fn with_note(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.note = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_actor(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.actor = ThemeColorValue::parse(value.as_ref())?;
        Ok(self)
    }

    pub fn with_activation(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, ThemeCompileValidationError> {
        self.activation = ThemeColorValue::parse(value.as_ref())?;
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
            .with_rule(ThemeRule::new(
                ThemeTarget::Node,
                fill_stroke(&self.surface, &self.border),
            ))
            .with_rule(ThemeRule::new(ThemeTarget::NodeLabel, text(&self.text)))
            .with_rule(ThemeRule::new(
                ThemeTarget::Edge,
                fill_stroke(&self.line, &self.line),
            ))
            .with_rule(ThemeRule::new(ThemeTarget::EdgeLabel, fill(&self.canvas)))
            .with_rule(ThemeRule::new(
                ThemeTarget::Cluster,
                fill_stroke(&self.surface_alt, &self.border),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::ClusterLabel,
                text(&self.subtle_text),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::Actor,
                fill_stroke(&self.actor, &self.border),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::Message,
                fill_stroke(&self.line, &self.line),
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
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::SpecialState,
                    fill_stroke(&self.accent, &self.accent),
                )
                .with_variant(ThemeVariant::Special),
            )
            .with_rule(ThemeRule::new(
                ThemeTarget::Marker,
                fill_stroke(&self.accent, &self.accent),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::Note,
                fill_stroke(&self.note, &self.border),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::Activation,
                fill_stroke(&self.activation, &self.border),
            ))
            .with_ordinal_palette(
                ThemeTarget::ChartSeries,
                OrdinalPalette::new(self.series)
                    .expect("ThemeTokens always contains a non-empty series palette"),
            );

        let canvas = CanvasSpec::default().with_base(CanvasPaint::Solid(self.canvas));
        let typography = self.typography;
        DiagramThemeSpec::new()
            .with_typography(typography)
            .with_styles(styles)
            .with_canvas(canvas)
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
