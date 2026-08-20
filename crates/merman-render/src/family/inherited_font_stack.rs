use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, ResolvedDiagramTheme, ThemeTypographyProperty,
};
use crate::text::{TextMeasurer, TextStyle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InheritedFontStackOutcome {
    Inactive,
    Typed,
    ConfigOwned,
    Unsupported,
}

/// Shared resolution core for families whose terminal text inherits one root font stack.
///
/// The owning family remains responsible for its writer and occurrence receipts. This core shares
/// only the final value and the typed/config/unsupported ownership outcome.
#[derive(Debug)]
pub(crate) struct InheritedFontStackPlan {
    font_family_css: Box<str>,
    outcome: InheritedFontStackOutcome,
    typed_font_stack_requested: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct InheritedTextRunSpec<'a> {
    text: &'a str,
    font_size_px: f64,
    baseline_x_px: f64,
    baseline_y_px: f64,
}

impl<'a> InheritedTextRunSpec<'a> {
    pub(crate) const fn new(
        text: &'a str,
        font_size_px: f64,
        baseline_x_px: f64,
        baseline_y_px: f64,
    ) -> Self {
        Self {
            text,
            font_size_px,
            baseline_x_px,
            baseline_y_px,
        }
    }
}

/// Prepared geometry for one inherited SVG text run.
///
/// The family plan creates these facts once from the final font winner. Layout, writer, and
/// terminal evidence then consume the same coordinates instead of independently reinterpreting
/// typography or DOM state.
#[derive(Debug, Clone)]
pub(crate) struct InheritedTextRunFacts {
    text: Box<str>,
    font_size_px: f64,
    x: f64,
    y: f64,
}

impl InheritedTextRunFacts {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) const fn font_size_px(&self) -> f64 {
        self.font_size_px
    }

    pub(crate) const fn x(&self) -> f64 {
        self.x
    }

    pub(crate) const fn y(&self) -> f64 {
        self.y
    }

    pub(crate) fn matches_terminal(&self, text: &str, font_size_px: f64, x: f64, y: f64) -> bool {
        self.text() == text && self.font_size_px == font_size_px && self.x == x && self.y == y
    }
}

/// One measurement-owned viewport and its final inherited text runs.
#[derive(Debug)]
pub(crate) struct InheritedTextViewportFacts {
    width_px: f64,
    height_px: f64,
    runs: Box<[InheritedTextRunFacts]>,
}

impl InheritedTextViewportFacts {
    pub(crate) fn prepare<'a>(
        baseline_width_px: f64,
        baseline_height_px: f64,
        specs: impl IntoIterator<Item = InheritedTextRunSpec<'a>>,
        font_family_css: &str,
        measurer: &dyn TextMeasurer,
    ) -> Self {
        debug_assert!(baseline_width_px.is_finite() && baseline_width_px > 0.0);
        debug_assert!(baseline_height_px.is_finite() && baseline_height_px > 0.0);

        struct MeasuredRun<'a> {
            spec: InheritedTextRunSpec<'a>,
            anchor_x_ratio: f64,
            anchor_y_ratio: f64,
        }

        let mut width_px = baseline_width_px;
        let mut height_px = baseline_height_px;
        let mut measured = Vec::new();
        for spec in specs {
            debug_assert!(spec.font_size_px.is_finite() && spec.font_size_px > 0.0);
            debug_assert!(spec.baseline_x_px > 0.0 && spec.baseline_x_px < baseline_width_px);
            debug_assert!(spec.baseline_y_px > 0.0 && spec.baseline_y_px < baseline_height_px);
            let anchor_x_ratio =
                (spec.baseline_x_px / baseline_width_px).clamp(f64::EPSILON, 1.0 - f64::EPSILON);
            let anchor_y_ratio =
                (spec.baseline_y_px / baseline_height_px).clamp(f64::EPSILON, 1.0 - f64::EPSILON);
            let style = TextStyle {
                font_family: Some(font_family_css.to_string()),
                font_size: spec.font_size_px,
                ..TextStyle::default()
            };
            let (left, right) = measurer.measure_svg_text_bbox_x(spec.text, &style);
            let bbox_height = measurer.measure_svg_raw_text_bbox_height_px(spec.text, &style);
            width_px = width_px
                .max(required_axis_extent(left.max(0.0), anchor_x_ratio))
                .max(required_axis_extent(right.max(0.0), 1.0 - anchor_x_ratio));
            height_px = height_px
                .max(required_axis_extent(bbox_height.max(0.0), anchor_y_ratio))
                .max(required_axis_extent(
                    bbox_height.max(0.0),
                    1.0 - anchor_y_ratio,
                ));
            measured.push(MeasuredRun {
                spec,
                anchor_x_ratio,
                anchor_y_ratio,
            });
        }

        let runs = measured
            .into_iter()
            .map(|measured| InheritedTextRunFacts {
                text: measured.spec.text.into(),
                font_size_px: measured.spec.font_size_px,
                x: if width_px == baseline_width_px {
                    measured.spec.baseline_x_px
                } else {
                    measured.anchor_x_ratio * width_px
                },
                y: if height_px == baseline_height_px {
                    measured.spec.baseline_y_px
                } else {
                    measured.anchor_y_ratio * height_px
                },
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Self {
            width_px,
            height_px,
            runs,
        }
    }

    pub(crate) const fn width_px(&self) -> f64 {
        self.width_px
    }

    pub(crate) const fn height_px(&self) -> f64 {
        self.height_px
    }

    pub(crate) fn run(&self, index: usize) -> &InheritedTextRunFacts {
        &self.runs[index]
    }
}

fn required_axis_extent(measured_extent: f64, available_ratio: f64) -> f64 {
    if measured_extent == 0.0 {
        return 0.0;
    }
    measured_extent / available_ratio
}

impl InheritedFontStackPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        let configured_font = crate::config::config_font_family_css(effective_config.as_value());
        let Some(theme) = theme else {
            return Self {
                font_family_css: configured_font.into_boxed_str(),
                outcome: InheritedFontStackOutcome::Inactive,
                typed_font_stack_requested: false,
            };
        };

        let config_owns_font_stack =
            ["themeVariables.fontFamily", "fontFamily"]
                .into_iter()
                .any(|path| {
                    merman_core::__private::config_path_overrides_typed_default(
                        effective_config,
                        path,
                    )
                });
        let mut typed_font_stack = false;
        let mut unsupported_typography = false;
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                {
                    typed_font_stack = true;
                }
                FamilyThemeMechanism::BaseTypography(_) => unsupported_typography = true,
                FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        let font_family_css = if typed_font_stack && !config_owns_font_stack {
            theme.typography().font_stack().as_css()
        } else {
            configured_font
        };
        let outcome = if unsupported_typography {
            InheritedFontStackOutcome::Unsupported
        } else if typed_font_stack && config_owns_font_stack {
            InheritedFontStackOutcome::ConfigOwned
        } else if typed_font_stack {
            InheritedFontStackOutcome::Typed
        } else {
            InheritedFontStackOutcome::Inactive
        };

        Self {
            font_family_css: font_family_css.into_boxed_str(),
            outcome,
            typed_font_stack_requested: typed_font_stack,
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        &self.font_family_css
    }

    pub(crate) const fn outcome(&self) -> InheritedFontStackOutcome {
        self.outcome
    }

    /// Whether this operation requested the directly owned inherited font-stack route.
    ///
    /// Families whose unthemed baseline has no font writer use this bit to avoid changing their
    /// terminal DOM unless the typed route was actually requested. It remains true when explicit
    /// configuration owns the winning value or a sibling typography property makes the combined
    /// mechanism fail closed.
    pub(crate) const fn typed_font_stack_requested(&self) -> bool {
        self.typed_font_stack_requested
    }
}
