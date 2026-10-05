use std::collections::BTreeSet;

use merman_core::MermaidConfig;

use super::{FamilyThemeEvidence, FamilyThemeResidualReason};
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
/// only the final value and the FontStack ownership outcome. Unsupported sibling properties are
/// tracked separately so they cannot suppress an independently typed FontStack route.
#[derive(Debug)]
pub(crate) struct InheritedFontStackPlan {
    font_family_css: Box<str>,
    outcome: InheritedFontStackOutcome,
    typed_font_stack_requested: bool,
    typed_font_stack_active: bool,
    unsupported_typography_properties: BTreeSet<ThemeTypographyProperty>,
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

    pub(crate) fn runs(&self) -> &[InheritedTextRunFacts] {
        &self.runs
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
    /// Resolves the font stack independently from unsupported sibling typography properties.
    pub(crate) fn resolve_property_local(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        let configured_font = crate::config::config_font_family_css(effective_config.as_value());
        Self::resolve_inner(theme, effective_config, configured_font)
    }

    fn resolve_inner(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        configured_font: String,
    ) -> Self {
        let Some(theme) = theme else {
            return Self {
                font_family_css: configured_font.into_boxed_str(),
                outcome: InheritedFontStackOutcome::Inactive,
                typed_font_stack_requested: false,
                typed_font_stack_active: false,
                unsupported_typography_properties: BTreeSet::new(),
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
        let mut font_stack_unsupported = false;
        let mut unsupported_typography_properties = BTreeSet::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match (route.mechanism(), route.disposition()) {
                (
                    FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack),
                    FamilyThemeDisposition::TypedAdapter,
                ) => {
                    typed_font_stack = true;
                }
                (
                    FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack),
                    FamilyThemeDisposition::LegacyCompatibility,
                ) => {
                    // The compatibility ledger owns this route; it is not a native FontStack
                    // outcome and must not request a typed receipt.
                }
                (
                    FamilyThemeMechanism::BaseTypography(property),
                    FamilyThemeDisposition::Unsupported,
                ) => {
                    font_stack_unsupported |= property == ThemeTypographyProperty::FontStack;
                    unsupported_typography_properties.insert(property);
                }
                (FamilyThemeMechanism::BaseTypography(_), _)
                | (FamilyThemeMechanism::RuleFacet { .. }, _)
                | (FamilyThemeMechanism::OrdinalPalette { .. }, _)
                | (FamilyThemeMechanism::EffectBinding { .. }, _) => {}
            }
        }

        // Legacy and unsupported siblings remain non-portable, but neither owns the font-family
        // assignment. Each property is settled by its own evidence or compatibility ledger.
        let typed_font_stack_active = typed_font_stack && !config_owns_font_stack;
        let font_family_css = if typed_font_stack_active {
            theme.typography().font_stack().as_css()
        } else {
            configured_font
        };
        let outcome = match () {
            _ if typed_font_stack && config_owns_font_stack => {
                InheritedFontStackOutcome::ConfigOwned
            }
            _ if typed_font_stack => InheritedFontStackOutcome::Typed,
            _ if font_stack_unsupported => InheritedFontStackOutcome::Unsupported,
            _ => InheritedFontStackOutcome::Inactive,
        };

        Self {
            font_family_css: font_family_css.into_boxed_str(),
            outcome,
            typed_font_stack_requested: typed_font_stack,
            typed_font_stack_active,
            unsupported_typography_properties,
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
    /// configuration owns the winning value or a sibling typography property is handled by a
    /// separate evidence ledger.
    pub(crate) const fn typed_font_stack_requested(&self) -> bool {
        self.typed_font_stack_requested
    }

    /// Whether the typed font stack owns the emitted and measured font-family value.
    ///
    /// Property-local families settle legacy and unsupported sibling properties through their own
    /// evidence keys, so those siblings do not change this font-stack ownership result.
    pub(crate) const fn typed_font_stack_active(&self) -> bool {
        self.typed_font_stack_active
    }

    pub(crate) fn has_unsupported_typography_properties(&self) -> bool {
        !self.unsupported_typography_properties.is_empty()
    }

    /// Whether this family has any native typography evidence to settle.
    ///
    /// Legacy-compatible properties belong to the compatibility ledger and therefore do not make
    /// the native terminal receipt path run by themselves.
    pub(crate) fn typography_requested(&self) -> bool {
        self.typed_font_stack_requested || self.has_unsupported_typography_properties()
    }

    pub(crate) fn mark_unsupported_typography_evidence(
        &self,
        evidence: &mut FamilyThemeEvidence,
        has_visible_terminals: bool,
    ) {
        for property in self.unsupported_typography_properties.iter().copied() {
            let key = crate::diagram_theme::FamilyThemeMechanismKey::Typography(property);
            if has_visible_terminals {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            } else {
                evidence.mark_not_applicable(key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
    };

    fn resolved_typography_theme(
        family: DiagramFamilyId,
        style: ThemeTextStyle,
    ) -> ResolvedDiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_typography(TypographySpec::default().with_family_style(family, style)),
            )
            .expect("compile typography fixture")
            .resolve(family)
    }

    #[test]
    fn legacy_font_size_does_not_request_native_font_stack_evidence() {
        let theme = resolved_typography_theme(
            DiagramFamilyId::REQUIREMENT,
            ThemeTextStyle::default()
                .with_font_size_px(24.0)
                .expect("valid legacy font size"),
        );
        let plan =
            InheritedFontStackPlan::resolve_property_local(Some(&theme), &MermaidConfig::default());

        assert_eq!(plan.outcome(), InheritedFontStackOutcome::Inactive);
        assert!(!plan.typed_font_stack_requested());
        assert!(!plan.typography_requested());
        assert!(!plan.has_unsupported_typography_properties());
    }

    #[test]
    fn unsupported_sibling_does_not_suppress_typed_font_stack() {
        let theme = resolved_typography_theme(
            DiagramFamilyId::WARDLEY,
            ThemeTextStyle::default()
                .with_font_stack(FontStack::single("Typed Wardley").expect("valid font stack"))
                .with_font_size_px(24.0)
                .expect("valid unsupported font size"),
        );
        let plan =
            InheritedFontStackPlan::resolve_property_local(Some(&theme), &MermaidConfig::default());

        assert_eq!(plan.outcome(), InheritedFontStackOutcome::Typed);
        assert!(plan.typed_font_stack_requested());
        assert!(plan.typed_font_stack_active());
        assert!(plan.typography_requested());
        assert!(
            plan.unsupported_typography_properties
                .contains(&ThemeTypographyProperty::FontSize)
        );
    }
}
