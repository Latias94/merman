use std::collections::{BTreeMap, BTreeSet};

use merman_core::MermaidConfig;
use merman_core::diagrams::state::StateDiagramRenderNode;
use serde_json::Value;

use crate::diagram_theme::{ThemeTarget, ThemeTypographyProperty};

/// Concrete State terminal whose Mermaid compatibility value can outrank typed paint.
///
/// These are writer surfaces rather than authoring targets. Native and HTML transition-label
/// backgrounds, for example, deliberately remain distinct because their terminal owners differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StateTerminalSurface {
    StateClassic,
    StateHandDrawn,
    StateNeo,
    StateLabel,
    Transition,
    TransitionMarker,
    TransitionLabel,
    TransitionLabelBackgroundNative,
    TransitionLabelBackgroundHtml,
    NoteClassic,
    NoteHandDrawn,
    NoteNeo,
    NoteLabel,
    CompositeBodyClassic,
    CompositeBodyClassicAlt,
    CompositeDividerClassic,
    CompositeBodyHandDrawn,
    CompositeBodyHandDrawnAlt,
    CompositeDividerHandDrawn,
    CompositeBodyNeo,
    CompositeHeaderClassic,
    CompositeHeaderHandDrawn,
    CompositeHeaderNeo,
    CompositeLabel,
    SpecialStartClassic,
    SpecialStartHandDrawn,
    SpecialStartNeo,
    SpecialEndOuterClassic,
    SpecialEndOuterHandDrawn,
    SpecialEndOuterNeo,
    SpecialForkJoinClassic,
    SpecialForkJoinHandDrawn,
    SpecialForkJoinNeo,
    SpecialChoiceClassic,
    SpecialChoiceHandDrawn,
    SpecialChoiceNeo,
    SpecialEndInnerClassic,
    SpecialEndInnerHandDrawn,
    SpecialEndInnerNeo,
    Title,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StateTerminalPaintProperty {
    Fill,
    Stroke,
    StrokeWidth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StateCompatibilityLook {
    Classic,
    HandDrawn,
    Neo,
    Other(String),
}

impl StateCompatibilityLook {
    fn from_value(value: &str) -> Self {
        match value {
            "classic" => Self::Classic,
            "handDrawn" => Self::HandDrawn,
            "neo" => Self::Neo,
            other => Self::Other(other.to_string()),
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Self::Classic => "classic",
            Self::HandDrawn => "handDrawn",
            Self::Neo => "neo",
            Self::Other(value) => value,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct StateValueProvenance {
    selected_paths: Vec<&'static str>,
    owned: bool,
}

impl StateValueProvenance {
    fn selected(path: &'static str, owned: bool) -> Self {
        Self {
            selected_paths: vec![path],
            owned,
        }
    }

    fn derived<'a>(inputs: impl IntoIterator<Item = &'a Self>) -> Self {
        let mut provenance = Self::default();
        for input in inputs {
            provenance.owned |= input.owned;
            for path in &input.selected_paths {
                if !provenance.selected_paths.contains(path) {
                    provenance.selected_paths.push(path);
                }
            }
        }
        provenance
    }
}

#[derive(Debug, Clone)]
struct StateResolvedValue<T> {
    value: T,
    provenance: StateValueProvenance,
}

#[derive(Debug, Clone)]
enum StateTerminalPaintValue {
    Css(String),
    ScopedGradient,
}

#[derive(Debug, Clone)]
struct StateTerminalPaintSelection {
    value: StateTerminalPaintValue,
    provenance: StateValueProvenance,
}

impl StateTerminalPaintSelection {
    fn css(value: &StateResolvedValue<String>) -> Self {
        Self {
            value: StateTerminalPaintValue::Css(value.value.clone()),
            provenance: value.provenance.clone(),
        }
    }

    fn scoped_gradient(provenance: StateValueProvenance) -> Self {
        Self {
            value: StateTerminalPaintValue::ScopedGradient,
            provenance,
        }
    }

    fn css_value<I: std::fmt::Display>(&self, diagram_id: I) -> String {
        match &self.value {
            StateTerminalPaintValue::Css(value) => value.clone(),
            StateTerminalPaintValue::ScopedGradient => {
                format!("url(#{diagram_id}-gradient)")
            }
        }
    }
}

impl<T> StateResolvedValue<T> {
    fn constant(value: T) -> Self {
        Self {
            value,
            provenance: StateValueProvenance::default(),
        }
    }

    fn map<U>(self, map: impl FnOnce(T) -> U) -> StateResolvedValue<U> {
        StateResolvedValue {
            value: map(self.value),
            provenance: self.provenance,
        }
    }
}

struct StateCompatibilityResolver<'a> {
    value: &'a Value,
    config: Option<&'a MermaidConfig>,
}

impl<'a> StateCompatibilityResolver<'a> {
    fn new(value: &'a Value, config: Option<&'a MermaidConfig>) -> Self {
        Self { value, config }
    }

    fn direct_string(
        &self,
        candidates: &[&'static str],
        fallback: impl FnOnce() -> StateResolvedValue<String>,
    ) -> StateResolvedValue<String> {
        for path in candidates {
            let Some(value) = value_at_dotted_path(self.value, path).and_then(Value::as_str) else {
                continue;
            };
            return StateResolvedValue {
                value: value.to_string(),
                provenance: StateValueProvenance::selected(path, self.path_is_owned(path)),
            };
        }
        fallback()
    }

    fn truthy_string(
        &self,
        candidates: &[&'static str],
        fallback: impl FnOnce() -> StateResolvedValue<String>,
    ) -> StateResolvedValue<String> {
        for path in candidates {
            let Some(value) = value_at_dotted_path(self.value, path).and_then(Value::as_str) else {
                continue;
            };
            // JavaScript's `||` rejects the exact empty string, but whitespace remains truthy.
            if value.is_empty() {
                continue;
            }
            return StateResolvedValue {
                value: value.to_string(),
                provenance: StateValueProvenance::selected(path, self.path_is_owned(path)),
            };
        }
        fallback()
    }

    fn nullish_string(
        &self,
        candidates: &[&'static str],
        fallback: impl FnOnce() -> StateResolvedValue<String>,
    ) -> StateResolvedValue<String> {
        for path in candidates {
            let Some(value) = value_at_dotted_path(self.value, path) else {
                continue;
            };
            if value.is_null() {
                continue;
            }
            let Some(value) = value.as_str() else {
                continue;
            };
            return StateResolvedValue {
                value: value.to_string(),
                provenance: StateValueProvenance::selected(path, self.path_is_owned(path)),
            };
        }
        fallback()
    }

    fn direct_css_value(
        &self,
        candidates: &[&'static str],
        fallback: impl FnOnce() -> StateResolvedValue<String>,
    ) -> StateResolvedValue<String> {
        for path in candidates {
            let Some(value) =
                value_at_dotted_path(self.value, path).and_then(state_js_css_interpolation_value)
            else {
                continue;
            };
            return StateResolvedValue {
                value,
                provenance: StateValueProvenance::selected(path, self.path_is_owned(path)),
            };
        }
        fallback()
    }

    fn truthy_css_value(
        &self,
        candidates: &[&'static str],
        fallback: impl FnOnce() -> StateResolvedValue<String>,
    ) -> StateResolvedValue<String> {
        for path in candidates {
            let Some(raw) = value_at_dotted_path(self.value, path) else {
                continue;
            };
            if !state_js_truthy(raw) {
                continue;
            }
            let Some(value) = state_js_css_interpolation_value(raw) else {
                continue;
            };
            return StateResolvedValue {
                value,
                provenance: StateValueProvenance::selected(path, self.path_is_owned(path)),
            };
        }
        fallback()
    }

    fn nullish_css_value(
        &self,
        candidates: &[&'static str],
        fallback: impl FnOnce() -> StateResolvedValue<String>,
    ) -> StateResolvedValue<String> {
        for path in candidates {
            let Some(raw) = value_at_dotted_path(self.value, path) else {
                continue;
            };
            if raw.is_null() {
                continue;
            }
            let Some(value) = state_js_css_interpolation_value(raw) else {
                continue;
            };
            return StateResolvedValue {
                value,
                provenance: StateValueProvenance::selected(path, self.path_is_owned(path)),
            };
        }
        fallback()
    }

    fn truthy(&self, candidates: &[&'static str], fallback: bool) -> StateResolvedValue<bool> {
        for path in candidates {
            let Some(value) = value_at_dotted_path(self.value, path) else {
                continue;
            };
            return StateResolvedValue {
                value: state_js_truthy(value),
                provenance: StateValueProvenance::selected(path, self.path_is_owned(path)),
            };
        }
        StateResolvedValue::constant(fallback)
    }

    fn direct_css_px(&self, candidates: &[&'static str], fallback: f64) -> StateResolvedValue<f64> {
        for path in candidates {
            let Some(value) =
                value_at_dotted_path(self.value, path).and_then(crate::config::json_f64_css_px)
            else {
                continue;
            };
            return StateResolvedValue {
                value,
                provenance: StateValueProvenance::selected(path, self.path_is_owned(path)),
            };
        }
        StateResolvedValue::constant(fallback)
    }

    fn path_is_owned(&self, path: &str) -> bool {
        self.config.is_some_and(|config| {
            merman_core::__private::config_path_overrides_typed_default(config, path)
        })
    }
}

fn state_js_css_interpolation_value(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| crate::config::json_css_number_or_string(value))
}

fn state_js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// State's single compatibility decision for both the typed adapter and terminal SVG writer.
///
/// Each fallback is resolved once. The same selected value supplies emitted compatibility CSS,
/// while its selected path and surviving config provenance decide whether typed paint must yield.
#[derive(Debug, Clone)]
pub(crate) struct StateCompatibilityPlan {
    pub(crate) dark_mode: bool,
    look: StateCompatibilityLook,
    html_labels: bool,
    pub(crate) font_family_css: String,
    pub(crate) font_size_px: f64,
    pub(crate) typography_config_owned: BTreeSet<ThemeTypographyProperty>,
    pub(crate) configured_font_stack: Option<crate::text::ParsedCssFontStack>,
    pub(crate) text_color: String,
    pub(crate) line_color: String,
    pub(crate) error_bkg: String,
    pub(crate) error_text: String,
    pub(crate) transition_color: String,
    pub(crate) node_border: String,
    pub(crate) background: String,
    pub(crate) main_bkg: String,
    pub(crate) alt_background: String,
    pub(crate) stroke_width: String,
    pub(crate) stroke_width_px: String,
    pub(crate) neo_stroke_width: String,
    pub(crate) neo_stroke_width_px: String,
    pub(crate) note_border: String,
    pub(crate) note_bkg: String,
    pub(crate) note_text: String,
    pub(crate) label_background: String,
    pub(crate) edge_label_background: String,
    pub(crate) transition_label_color: String,
    pub(crate) special_state_color: String,
    pub(crate) inner_end_background: String,
    pub(crate) composite_background: String,
    pub(crate) state_bkg: String,
    pub(crate) state_border: String,
    pub(crate) composite_title_background: String,
    pub(crate) state_label_color: String,
    pub(crate) drop_shadow: String,
    pub(crate) use_gradient: bool,
    pub(crate) gradient_start: String,
    pub(crate) gradient_stop: String,
    pub(crate) neo_radius: f64,
    pub(crate) rect_radius: f64,
    pub(crate) node_shadow: bool,
    terminal_paints:
        BTreeMap<(StateTerminalSurface, StateTerminalPaintProperty), StateTerminalPaintSelection>,
}

impl StateCompatibilityPlan {
    pub(crate) fn from_config(config: &MermaidConfig) -> Self {
        Self::resolve(StateCompatibilityResolver::new(
            config.as_value(),
            Some(config),
        ))
    }

    pub(crate) fn from_value(value: &Value) -> Self {
        Self::resolve(StateCompatibilityResolver::new(value, None))
    }

    fn resolve(resolver: StateCompatibilityResolver<'_>) -> Self {
        let theme_name = resolver.direct_string(&["theme"], || {
            StateResolvedValue::constant("default".to_string())
        });
        let dark_mode = theme_name.value.contains("dark");
        let look = StateCompatibilityLook::from_value(
            crate::config::config_diagram_look(resolver.value).as_str(),
        );
        let html_labels = crate::config::config_effective_html_labels(resolver.value);
        // Generated theme defaults must not hide an explicitly configured root font value.
        // Among explicit values, retain Mermaid's themeVariables-before-root precedence.
        let owned_font_family = ["themeVariables.fontFamily", "fontFamily"]
            .into_iter()
            .filter(|path| resolver.path_is_owned(path))
            .find_map(|path| {
                let value = value_at_dotted_path(resolver.value, path)?.as_str()?;
                let css = crate::config::normalize_css_font_family(value);
                (!css.is_empty()).then_some(css)
            });
        let owned_font_size = ["themeVariables.fontSize", "fontSize"]
            .into_iter()
            .filter(|path| resolver.path_is_owned(path))
            .find_map(|path| {
                value_at_dotted_path(resolver.value, path).and_then(crate::config::json_f64_css_px)
            });
        let mut typography_config_owned = BTreeSet::new();
        if owned_font_family.is_some() {
            typography_config_owned.insert(ThemeTypographyProperty::FontStack);
        }
        if owned_font_size.is_some() {
            typography_config_owned.insert(ThemeTypographyProperty::FontSize);
        }
        let font_family_css = owned_font_family
            .unwrap_or_else(|| crate::config::config_font_family_css(resolver.value));
        let configured_font_stack = typography_config_owned
            .contains(&ThemeTypographyProperty::FontStack)
            .then(|| crate::text::parse_css_font_stack(&font_family_css))
            .flatten();
        let font_size_px = owned_font_size
            .unwrap_or_else(|| {
                crate::config::config_theme_or_root_font_size_px(resolver.value, 16.0)
            })
            .max(1.0);

        let text_color = resolver.direct_string(&["themeVariables.textColor"], || {
            StateResolvedValue::constant("#333".to_string())
        });
        let line_color = resolver.direct_string(&["themeVariables.lineColor"], || {
            StateResolvedValue::constant("#333333".to_string())
        });
        let error_bkg = resolver.direct_string(&["themeVariables.errorBkgColor"], || {
            StateResolvedValue::constant("#552222".to_string())
        });
        let error_text = resolver.direct_string(&["themeVariables.errorTextColor"], || {
            StateResolvedValue::constant("#552222".to_string())
        });
        let transition_color =
            resolver.direct_string(&["themeVariables.transitionColor"], || line_color.clone());
        let node_border = resolver.direct_string(&["themeVariables.nodeBorder"], || {
            StateResolvedValue::constant("#9370DB".to_string())
        });
        let background = resolver.direct_string(&["themeVariables.background"], || {
            StateResolvedValue::constant("white".to_string())
        });
        let main_bkg = resolver.direct_string(&["themeVariables.mainBkg"], || {
            StateResolvedValue::constant("#ECECFF".to_string())
        });
        let alt_background = resolver.truthy_string(&["themeVariables.altBackground"], || {
            StateResolvedValue::constant("#efefef".to_string())
        });
        let stroke_width = resolver.truthy_css_value(&["themeVariables.strokeWidth"], || {
            StateResolvedValue::constant("1".to_string())
        });
        let stroke_width_px = stroke_width.clone().map(|value| {
            if value.trim_end().ends_with("px") {
                value
            } else {
                format!("{value}px")
            }
        });
        let neo_stroke_width = resolver.nullish_css_value(&["themeVariables.strokeWidth"], || {
            StateResolvedValue::constant("1".to_string())
        });
        let neo_stroke_width_px = neo_stroke_width.clone().map(|value| {
            if value.trim_end().ends_with("px") {
                value
            } else {
                format!("{value}px")
            }
        });
        let neo_rough_stroke_width = neo_stroke_width.clone().map(|value| {
            let width = state_terminal_stroke_width_value(&value, 1.0);
            if (width - 1.0).abs() <= 1e-9 {
                "1.3".to_string()
            } else {
                width.to_string()
            }
        });
        let hand_drawn_stroke_width = StateResolvedValue::constant("1.3".to_string());
        let rough_default_stroke_width = StateResolvedValue::constant("1".to_string());
        let end_stroke_width = StateResolvedValue::constant("2".to_string());
        let note_border = resolver.direct_string(&["themeVariables.noteBorderColor"], || {
            StateResolvedValue::constant("#aaaa33".to_string())
        });
        let note_bkg = resolver.direct_string(&["themeVariables.noteBkgColor"], || {
            StateResolvedValue::constant("#fff5ad".to_string())
        });
        let note_text = resolver.direct_string(&["themeVariables.noteTextColor"], || {
            StateResolvedValue::constant("black".to_string())
        });
        let label_background = resolver
            .direct_string(&["themeVariables.labelBackgroundColor"], || {
                main_bkg.clone()
            });
        let edge_label_background = resolver
            .direct_string(&["themeVariables.edgeLabelBackground"], || {
                StateResolvedValue::constant("rgba(232,232,232, 0.8)".to_string())
            });
        let transition_label_color = resolver.truthy_string(
            &[
                "themeVariables.transitionLabelColor",
                "themeVariables.tertiaryTextColor",
            ],
            || text_color.clone(),
        );
        let special_state_color =
            resolver.direct_string(&["themeVariables.specialStateColor"], || line_color.clone());
        let inner_end_background = resolver
            .direct_string(&["themeVariables.innerEndBackground"], || {
                node_border.clone()
            });
        let end_outer_fill = main_bkg.clone();
        let end_outer_stroke = line_color.clone();
        let end_inner_fill =
            resolver.nullish_string(&["themeVariables.stateBorder"], || node_border.clone());
        let end_inner_stroke = end_inner_fill.clone();
        let composite_background = resolver
            .truthy_string(&["themeVariables.compositeBackground"], || {
                background.clone()
            });
        let state_bkg = resolver.truthy_string(&["themeVariables.stateBkg"], || main_bkg.clone());
        let state_border =
            resolver.truthy_string(&["themeVariables.stateBorder"], || node_border.clone());
        let composite_title_background = resolver
            .direct_string(&["themeVariables.compositeTitleBackground"], || {
                main_bkg.clone()
            });
        let state_label_color = resolver.direct_string(&["themeVariables.stateLabelColor"], || {
            StateResolvedValue::constant("#131300".to_string())
        });
        let drop_shadow = resolver.direct_css_value(&["themeVariables.dropShadow"], || {
            StateResolvedValue::constant("none".to_string())
        });
        let use_gradient = resolver.truthy(&["themeVariables.useGradient"], false);
        let gradient_start = resolver.direct_string(&["themeVariables.gradientStart"], || {
            resolver.direct_string(&["themeVariables.primaryBorderColor"], || {
                StateResolvedValue::constant("#9370DB".to_string())
            })
        });
        let gradient_stop = resolver.direct_string(&["themeVariables.gradientStop"], || {
            resolver.direct_string(&["themeVariables.secondaryBorderColor"], || {
                gradient_start.clone()
            })
        });
        let neo_radius = resolver
            .direct_css_px(&["themeVariables.radius"], 5.0)
            .map(|value| value.max(0.0));
        let rect_radius = if resolver
            .value
            .pointer("/themeVariables/radius")
            .and_then(Value::as_f64)
            == Some(0.0)
        {
            10.0
        } else {
            crate::config::config_f64(resolver.value, &["themeVariables", "radius"]).unwrap_or(5.0)
        };
        let node_shadow = resolver
            .value
            .pointer("/themeVariables/nodeShadow")
            .is_some_and(crate::config::json_value_is_truthy);

        let mut terminal_paints = BTreeMap::new();
        macro_rules! bind_css {
            ($surface:expr, $property:expr, $value:expr) => {
                terminal_paints.insert(
                    ($surface, $property),
                    StateTerminalPaintSelection::css($value),
                );
            };
        }
        macro_rules! bind_selection {
            ($surface:expr, $property:expr, $selection:expr) => {
                terminal_paints.insert(($surface, $property), $selection.clone());
            };
        }

        for surface in [
            StateTerminalSurface::StateClassic,
            StateTerminalSurface::StateNeo,
        ] {
            bind_css!(surface, StateTerminalPaintProperty::Fill, &state_bkg);
            bind_css!(
                surface,
                StateTerminalPaintProperty::StrokeWidth,
                &stroke_width_px
            );
        }
        bind_css!(
            StateTerminalSurface::StateClassic,
            StateTerminalPaintProperty::Stroke,
            &state_border
        );
        bind_css!(
            StateTerminalSurface::StateHandDrawn,
            StateTerminalPaintProperty::Fill,
            &main_bkg
        );
        bind_css!(
            StateTerminalSurface::StateHandDrawn,
            StateTerminalPaintProperty::Stroke,
            &node_border
        );
        bind_css!(
            StateTerminalSurface::StateHandDrawn,
            StateTerminalPaintProperty::StrokeWidth,
            &hand_drawn_stroke_width
        );
        bind_css!(
            StateTerminalSurface::StateLabel,
            StateTerminalPaintProperty::Fill,
            &state_label_color
        );
        bind_css!(
            StateTerminalSurface::Transition,
            StateTerminalPaintProperty::Stroke,
            &transition_color
        );
        bind_css!(
            StateTerminalSurface::Transition,
            StateTerminalPaintProperty::StrokeWidth,
            &stroke_width
        );
        for property in [
            StateTerminalPaintProperty::Fill,
            StateTerminalPaintProperty::Stroke,
        ] {
            bind_css!(
                StateTerminalSurface::TransitionMarker,
                property,
                &transition_color
            );
        }
        bind_css!(
            StateTerminalSurface::TransitionLabel,
            StateTerminalPaintProperty::Fill,
            &transition_label_color
        );
        bind_css!(
            StateTerminalSurface::TransitionLabelBackgroundNative,
            StateTerminalPaintProperty::Fill,
            &label_background
        );
        bind_css!(
            StateTerminalSurface::TransitionLabelBackgroundHtml,
            StateTerminalPaintProperty::Fill,
            &edge_label_background
        );
        for surface in [
            StateTerminalSurface::NoteClassic,
            StateTerminalSurface::NoteHandDrawn,
            StateTerminalSurface::NoteNeo,
        ] {
            bind_css!(surface, StateTerminalPaintProperty::Fill, &note_bkg);
        }
        for surface in [
            StateTerminalSurface::NoteClassic,
            StateTerminalSurface::NoteHandDrawn,
        ] {
            bind_css!(surface, StateTerminalPaintProperty::Stroke, &note_border);
            bind_css!(
                surface,
                StateTerminalPaintProperty::StrokeWidth,
                &hand_drawn_stroke_width
            );
        }
        bind_css!(
            StateTerminalSurface::NoteNeo,
            StateTerminalPaintProperty::StrokeWidth,
            &neo_rough_stroke_width
        );
        bind_css!(
            StateTerminalSurface::NoteLabel,
            StateTerminalPaintProperty::Fill,
            &note_text
        );
        for (surface, fill) in [
            (
                StateTerminalSurface::CompositeBodyClassic,
                &composite_background,
            ),
            (
                StateTerminalSurface::CompositeBodyClassicAlt,
                &alt_background,
            ),
            (
                StateTerminalSurface::CompositeDividerClassic,
                &alt_background,
            ),
            (
                StateTerminalSurface::CompositeHeaderClassic,
                &composite_title_background,
            ),
        ] {
            bind_css!(surface, StateTerminalPaintProperty::Fill, fill);
            bind_css!(surface, StateTerminalPaintProperty::Stroke, &state_border);
            bind_css!(
                surface,
                StateTerminalPaintProperty::StrokeWidth,
                &stroke_width_px
            );
        }
        let hand_drawn_divider_fill = StateResolvedValue::constant("lightgrey".to_string());
        for (surface, fill) in [
            (
                StateTerminalSurface::CompositeBodyHandDrawn,
                &composite_background,
            ),
            (
                StateTerminalSurface::CompositeBodyHandDrawnAlt,
                &alt_background,
            ),
            (
                StateTerminalSurface::CompositeDividerHandDrawn,
                &hand_drawn_divider_fill,
            ),
            (
                StateTerminalSurface::CompositeHeaderHandDrawn,
                &composite_title_background,
            ),
        ] {
            bind_css!(surface, StateTerminalPaintProperty::Fill, fill);
            bind_css!(surface, StateTerminalPaintProperty::Stroke, &node_border);
            bind_css!(
                surface,
                StateTerminalPaintProperty::StrokeWidth,
                &rough_default_stroke_width
            );
        }
        let neo_gradient_stroke_provenance = StateValueProvenance::derived([
            &use_gradient.provenance,
            &gradient_start.provenance,
            &gradient_stop.provenance,
        ]);
        let neo_node_stroke = if use_gradient.value {
            StateTerminalPaintSelection::scoped_gradient(neo_gradient_stroke_provenance.clone())
        } else {
            StateTerminalPaintSelection::css(&node_border)
        };
        let neo_cluster_stroke = if use_gradient.value {
            StateTerminalPaintSelection::scoped_gradient(neo_gradient_stroke_provenance)
        } else {
            StateTerminalPaintSelection::css(&state_border)
        };
        bind_selection!(
            StateTerminalSurface::StateNeo,
            StateTerminalPaintProperty::Stroke,
            &neo_node_stroke
        );
        bind_selection!(
            StateTerminalSurface::NoteNeo,
            StateTerminalPaintProperty::Stroke,
            &neo_node_stroke
        );
        for surface in [
            StateTerminalSurface::CompositeBodyNeo,
            StateTerminalSurface::CompositeHeaderNeo,
        ] {
            bind_css!(surface, StateTerminalPaintProperty::Fill, &main_bkg);
            bind_selection!(
                surface,
                StateTerminalPaintProperty::Stroke,
                &neo_cluster_stroke
            );
            bind_css!(
                surface,
                StateTerminalPaintProperty::StrokeWidth,
                &neo_stroke_width
            );
        }
        bind_css!(
            StateTerminalSurface::CompositeLabel,
            StateTerminalPaintProperty::Fill,
            &state_label_color
        );
        for property in [
            StateTerminalPaintProperty::Fill,
            StateTerminalPaintProperty::Stroke,
        ] {
            bind_css!(
                StateTerminalSurface::SpecialStartClassic,
                property,
                &special_state_color
            );
        }
        bind_css!(
            StateTerminalSurface::SpecialStartClassic,
            StateTerminalPaintProperty::StrokeWidth,
            &rough_default_stroke_width
        );
        for property in [
            StateTerminalPaintProperty::Fill,
            StateTerminalPaintProperty::Stroke,
        ] {
            bind_css!(
                StateTerminalSurface::SpecialStartHandDrawn,
                property,
                &line_color
            );
        }
        bind_css!(
            StateTerminalSurface::SpecialStartHandDrawn,
            StateTerminalPaintProperty::StrokeWidth,
            &rough_default_stroke_width
        );
        bind_css!(
            StateTerminalSurface::SpecialEndOuterClassic,
            StateTerminalPaintProperty::Fill,
            &end_outer_fill
        );
        bind_css!(
            StateTerminalSurface::SpecialEndOuterClassic,
            StateTerminalPaintProperty::Stroke,
            &end_outer_stroke
        );
        bind_css!(
            StateTerminalSurface::SpecialEndOuterClassic,
            StateTerminalPaintProperty::StrokeWidth,
            &end_stroke_width
        );
        bind_css!(
            StateTerminalSurface::SpecialEndOuterHandDrawn,
            StateTerminalPaintProperty::Fill,
            &end_outer_fill
        );
        bind_css!(
            StateTerminalSurface::SpecialEndOuterHandDrawn,
            StateTerminalPaintProperty::Stroke,
            &end_outer_stroke
        );
        bind_css!(
            StateTerminalSurface::SpecialEndOuterHandDrawn,
            StateTerminalPaintProperty::StrokeWidth,
            &end_stroke_width
        );
        for property in [
            StateTerminalPaintProperty::Fill,
            StateTerminalPaintProperty::Stroke,
        ] {
            bind_css!(
                StateTerminalSurface::SpecialForkJoinClassic,
                property,
                &line_color
            );
        }
        bind_css!(
            StateTerminalSurface::SpecialForkJoinClassic,
            StateTerminalPaintProperty::StrokeWidth,
            &hand_drawn_stroke_width
        );
        for property in [
            StateTerminalPaintProperty::Fill,
            StateTerminalPaintProperty::Stroke,
        ] {
            bind_css!(
                StateTerminalSurface::SpecialForkJoinHandDrawn,
                property,
                &line_color
            );
        }
        bind_css!(
            StateTerminalSurface::SpecialForkJoinHandDrawn,
            StateTerminalPaintProperty::StrokeWidth,
            &hand_drawn_stroke_width
        );
        bind_css!(
            StateTerminalSurface::SpecialChoiceClassic,
            StateTerminalPaintProperty::Fill,
            &main_bkg
        );
        bind_css!(
            StateTerminalSurface::SpecialChoiceClassic,
            StateTerminalPaintProperty::Stroke,
            &node_border
        );
        bind_css!(
            StateTerminalSurface::SpecialChoiceClassic,
            StateTerminalPaintProperty::StrokeWidth,
            &hand_drawn_stroke_width
        );
        bind_css!(
            StateTerminalSurface::SpecialChoiceHandDrawn,
            StateTerminalPaintProperty::Fill,
            &main_bkg
        );
        bind_css!(
            StateTerminalSurface::SpecialChoiceHandDrawn,
            StateTerminalPaintProperty::Stroke,
            &node_border
        );
        bind_css!(
            StateTerminalSurface::SpecialChoiceHandDrawn,
            StateTerminalPaintProperty::StrokeWidth,
            &hand_drawn_stroke_width
        );
        bind_css!(
            StateTerminalSurface::SpecialStartNeo,
            StateTerminalPaintProperty::Fill,
            &special_state_color
        );
        bind_selection!(
            StateTerminalSurface::SpecialStartNeo,
            StateTerminalPaintProperty::Stroke,
            &neo_node_stroke
        );
        bind_css!(
            StateTerminalSurface::SpecialStartNeo,
            StateTerminalPaintProperty::StrokeWidth,
            &rough_default_stroke_width
        );
        for (surface, fill) in [
            (StateTerminalSurface::SpecialEndOuterNeo, &end_outer_fill),
            (StateTerminalSurface::SpecialForkJoinNeo, &line_color),
            (StateTerminalSurface::SpecialChoiceNeo, &main_bkg),
        ] {
            bind_css!(surface, StateTerminalPaintProperty::Fill, fill);
            bind_selection!(
                surface,
                StateTerminalPaintProperty::Stroke,
                &neo_node_stroke
            );
            bind_css!(
                surface,
                StateTerminalPaintProperty::StrokeWidth,
                &neo_rough_stroke_width
            );
        }
        bind_css!(
            StateTerminalSurface::SpecialEndInnerClassic,
            StateTerminalPaintProperty::Fill,
            &end_inner_fill
        );
        bind_css!(
            StateTerminalSurface::SpecialEndInnerClassic,
            StateTerminalPaintProperty::Stroke,
            &end_inner_stroke
        );
        bind_css!(
            StateTerminalSurface::SpecialEndInnerClassic,
            StateTerminalPaintProperty::StrokeWidth,
            &end_stroke_width
        );
        bind_css!(
            StateTerminalSurface::SpecialEndInnerHandDrawn,
            StateTerminalPaintProperty::Fill,
            &end_inner_fill
        );
        bind_css!(
            StateTerminalSurface::SpecialEndInnerHandDrawn,
            StateTerminalPaintProperty::Stroke,
            &end_inner_stroke
        );
        bind_css!(
            StateTerminalSurface::SpecialEndInnerHandDrawn,
            StateTerminalPaintProperty::StrokeWidth,
            &end_stroke_width
        );
        bind_css!(
            StateTerminalSurface::SpecialEndInnerNeo,
            StateTerminalPaintProperty::Fill,
            &end_inner_fill
        );
        bind_selection!(
            StateTerminalSurface::SpecialEndInnerNeo,
            StateTerminalPaintProperty::Stroke,
            &neo_node_stroke
        );
        bind_css!(
            StateTerminalSurface::SpecialEndInnerNeo,
            StateTerminalPaintProperty::StrokeWidth,
            &neo_rough_stroke_width
        );
        bind_css!(
            StateTerminalSurface::Title,
            StateTerminalPaintProperty::Fill,
            &text_color
        );

        Self {
            dark_mode,
            look,
            html_labels,
            font_family_css,
            font_size_px,
            typography_config_owned,
            configured_font_stack,
            text_color: text_color.value,
            line_color: line_color.value,
            error_bkg: error_bkg.value,
            error_text: error_text.value,
            transition_color: transition_color.value,
            node_border: node_border.value,
            background: background.value,
            main_bkg: main_bkg.value,
            alt_background: alt_background.value,
            stroke_width: stroke_width.value,
            stroke_width_px: stroke_width_px.value,
            neo_stroke_width: neo_stroke_width.value,
            neo_stroke_width_px: neo_stroke_width_px.value,
            note_border: note_border.value,
            note_bkg: note_bkg.value,
            note_text: note_text.value,
            label_background: label_background.value,
            edge_label_background: edge_label_background.value,
            transition_label_color: transition_label_color.value,
            special_state_color: special_state_color.value,
            inner_end_background: inner_end_background.value,
            composite_background: composite_background.value,
            state_bkg: state_bkg.value,
            state_border: state_border.value,
            composite_title_background: composite_title_background.value,
            state_label_color: state_label_color.value,
            drop_shadow: drop_shadow.value,
            use_gradient: use_gradient.value,
            gradient_start: gradient_start.value,
            gradient_stop: gradient_stop.value,
            neo_radius: neo_radius.value,
            rect_radius,
            node_shadow,
            terminal_paints,
        }
    }

    pub(crate) fn diagram_look(&self) -> &str {
        self.look.as_str()
    }

    pub(crate) const fn html_labels(&self) -> bool {
        self.html_labels
    }

    pub(crate) const fn is_classic(&self) -> bool {
        matches!(&self.look, StateCompatibilityLook::Classic)
    }

    pub(crate) const fn is_hand_drawn(&self) -> bool {
        matches!(&self.look, StateCompatibilityLook::HandDrawn)
    }

    pub(crate) const fn is_neo(&self) -> bool {
        matches!(&self.look, StateCompatibilityLook::Neo)
    }

    pub(crate) fn terminal_paint_css<I: std::fmt::Display>(
        &self,
        surface: StateTerminalSurface,
        property: StateTerminalPaintProperty,
        diagram_id: I,
    ) -> String {
        self.terminal_paints
            .get(&(surface, property))
            .unwrap_or_else(|| panic!("missing State terminal paint for {surface:?}.{property:?}"))
            .css_value(diagram_id)
    }

    pub(crate) fn owns(
        &self,
        surface: StateTerminalSurface,
        property: StateTerminalPaintProperty,
    ) -> bool {
        self.terminal_paints
            .get(&(surface, property))
            .is_some_and(|selection| selection.provenance.owned)
    }

    pub(crate) fn source_shape_property_reaches(
        &self,
        surface: StateTerminalSurface,
        property: &str,
    ) -> bool {
        match surface {
            StateTerminalSurface::SpecialStartClassic
            | StateTerminalSurface::SpecialStartHandDrawn
            | StateTerminalSurface::SpecialStartNeo
            | StateTerminalSurface::CompositeBodyHandDrawn
            | StateTerminalSurface::CompositeBodyHandDrawnAlt
            | StateTerminalSurface::CompositeDividerHandDrawn
            | StateTerminalSurface::CompositeHeaderHandDrawn => false,
            StateTerminalSurface::NoteHandDrawn
            | StateTerminalSurface::SpecialForkJoinHandDrawn => {
                !matches!(property, "fill" | "stroke")
            }
            _ => true,
        }
    }

    pub(crate) fn terminal_stroke_width_value(&self, surface: StateTerminalSurface) -> f64 {
        state_terminal_stroke_width_value(
            &self.terminal_paint_css(surface, StateTerminalPaintProperty::StrokeWidth, ""),
            1.0,
        )
    }

    pub(crate) fn node_shape_surface(
        &self,
        node: &StateDiagramRenderNode,
        target: ThemeTarget,
    ) -> Option<StateTerminalSurface> {
        match target {
            ThemeTarget::State if self.is_neo() => Some(StateTerminalSurface::StateNeo),
            ThemeTarget::State if self.is_hand_drawn() => {
                Some(StateTerminalSurface::StateHandDrawn)
            }
            ThemeTarget::State => Some(StateTerminalSurface::StateClassic),
            ThemeTarget::Note if self.is_neo() => Some(StateTerminalSurface::NoteNeo),
            ThemeTarget::Note if self.is_hand_drawn() => Some(StateTerminalSurface::NoteHandDrawn),
            ThemeTarget::Note => Some(StateTerminalSurface::NoteClassic),
            ThemeTarget::Composite if self.is_neo() => Some(StateTerminalSurface::CompositeBodyNeo),
            ThemeTarget::Composite if self.is_hand_drawn() && node.shape == "divider" => {
                Some(StateTerminalSurface::CompositeDividerHandDrawn)
            }
            ThemeTarget::Composite
                if self.is_hand_drawn()
                    && node
                        .css_classes
                        .split_whitespace()
                        .any(|class| class == "statediagram-cluster-alt") =>
            {
                Some(StateTerminalSurface::CompositeBodyHandDrawnAlt)
            }
            ThemeTarget::Composite if self.is_hand_drawn() => {
                Some(StateTerminalSurface::CompositeBodyHandDrawn)
            }
            ThemeTarget::Composite if node.shape == "divider" => {
                Some(StateTerminalSurface::CompositeDividerClassic)
            }
            ThemeTarget::Composite
                if node
                    .css_classes
                    .split_whitespace()
                    .any(|class| class == "statediagram-cluster-alt") =>
            {
                Some(StateTerminalSurface::CompositeBodyClassicAlt)
            }
            ThemeTarget::Composite => Some(StateTerminalSurface::CompositeBodyClassic),
            ThemeTarget::SpecialState => match node.shape.as_str() {
                "stateStart" if self.is_neo() => Some(StateTerminalSurface::SpecialStartNeo),
                "stateStart" if self.is_hand_drawn() => {
                    Some(StateTerminalSurface::SpecialStartHandDrawn)
                }
                "stateStart" => Some(StateTerminalSurface::SpecialStartClassic),
                "stateEnd" if self.is_neo() => Some(StateTerminalSurface::SpecialEndOuterNeo),
                "stateEnd" if self.is_hand_drawn() => {
                    Some(StateTerminalSurface::SpecialEndOuterHandDrawn)
                }
                "stateEnd" => Some(StateTerminalSurface::SpecialEndOuterClassic),
                "fork" | "join" if self.is_neo() => Some(StateTerminalSurface::SpecialForkJoinNeo),
                "fork" | "join" if self.is_hand_drawn() => {
                    Some(StateTerminalSurface::SpecialForkJoinHandDrawn)
                }
                "fork" | "join" => Some(StateTerminalSurface::SpecialForkJoinClassic),
                "choice" if self.is_neo() => Some(StateTerminalSurface::SpecialChoiceNeo),
                "choice" if self.is_hand_drawn() => {
                    Some(StateTerminalSurface::SpecialChoiceHandDrawn)
                }
                "choice" => Some(StateTerminalSurface::SpecialChoiceClassic),
                _ => None,
            },
            _ => None,
        }
    }

    pub(crate) fn node_label_surface(target: ThemeTarget) -> Option<StateTerminalSurface> {
        match target {
            ThemeTarget::StateLabel => Some(StateTerminalSurface::StateLabel),
            ThemeTarget::NoteLabel => Some(StateTerminalSurface::NoteLabel),
            ThemeTarget::CompositeLabel => Some(StateTerminalSurface::CompositeLabel),
            _ => None,
        }
    }

    pub(crate) const fn composite_header_surface(&self) -> StateTerminalSurface {
        if self.is_neo() {
            StateTerminalSurface::CompositeHeaderNeo
        } else if self.is_hand_drawn() {
            StateTerminalSurface::CompositeHeaderHandDrawn
        } else {
            StateTerminalSurface::CompositeHeaderClassic
        }
    }

    pub(crate) const fn special_state_inner_surface(&self) -> StateTerminalSurface {
        if self.is_neo() {
            StateTerminalSurface::SpecialEndInnerNeo
        } else if self.is_hand_drawn() {
            StateTerminalSurface::SpecialEndInnerHandDrawn
        } else {
            StateTerminalSurface::SpecialEndInnerClassic
        }
    }

    pub(crate) fn classic_stroke_width_value(&self) -> f64 {
        self.stroke_width_px
            .trim()
            .strip_suffix("px")
            .unwrap_or(self.stroke_width_px.trim())
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or(1.0)
    }
}

fn state_terminal_stroke_width_value(value: &str, fallback: f64) -> f64 {
    value
        .trim()
        .strip_suffix("px")
        .unwrap_or(value.trim())
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
        .max(0.0)
}

fn value_at_dotted_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(value, |value, key| value.get(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_core::diagrams::state::StateDiagramRenderNode;
    use serde_json::json;

    fn ordinary_state_node() -> StateDiagramRenderNode {
        StateDiagramRenderNode {
            id: "Ready".to_string(),
            label_style: String::new(),
            label: Some(json!("Ready")),
            description: None,
            dom_id: "state-Ready-0".to_string(),
            is_group: false,
            node_type: None,
            parent_id: None,
            css_classes: "statediagram-state".to_string(),
            css_compiled_styles: Vec::new(),
            css_styles: Vec::new(),
            dir: None,
            explicit_dir: None,
            padding: Some(8.0),
            rx: Some(10.0),
            ry: Some(10.0),
            shape: "rect".to_string(),
            position: None,
            color_index: None,
            wrapping_width: None,
            min_width: None,
        }
    }

    #[test]
    fn state_string_fallbacks_distinguish_direct_truthy_and_nullish_values() {
        let empty = StateCompatibilityPlan::from_value(&json!({
            "themeVariables": {
                "textColor": "",
                "stateBorder": "",
                "nodeBorder": "#123456",
            }
        }));

        assert_eq!(empty.text_color, "");
        assert_eq!(empty.state_border, "#123456");
        let empty_inner = empty.special_state_inner_surface();
        assert_eq!(
            empty.terminal_paint_css(empty_inner, StateTerminalPaintProperty::Fill, "diagram"),
            ""
        );
        assert_eq!(
            empty.terminal_paint_css(empty_inner, StateTerminalPaintProperty::Stroke, "diagram"),
            ""
        );

        let whitespace = StateCompatibilityPlan::from_value(&json!({
            "themeVariables": {
                "stateBorder": "   ",
                "nodeBorder": "#123456",
            }
        }));

        assert_eq!(whitespace.state_border, "   ");
        let whitespace_inner = whitespace.special_state_inner_surface();
        assert_eq!(
            whitespace.terminal_paint_css(
                whitespace_inner,
                StateTerminalPaintProperty::Fill,
                "diagram",
            ),
            "   "
        );
        assert_eq!(
            whitespace.terminal_paint_css(
                whitespace_inner,
                StateTerminalPaintProperty::Stroke,
                "diagram",
            ),
            "   "
        );
    }

    #[test]
    fn state_stroke_width_fallbacks_distinguish_classic_or_from_neo_nullish() {
        let classic_zero = StateCompatibilityPlan::from_value(&json!({
            "look": "classic",
            "themeVariables": { "strokeWidth": 0 },
        }));
        assert_eq!(classic_zero.stroke_width, "1");
        assert_eq!(classic_zero.stroke_width_px, "1px");
        assert_eq!(classic_zero.neo_stroke_width, "0");
        assert_eq!(classic_zero.neo_stroke_width_px, "0px");
        assert_eq!(
            classic_zero.terminal_stroke_width_value(StateTerminalSurface::SpecialChoiceClassic),
            1.3
        );

        let neo_zero = StateCompatibilityPlan::from_value(&json!({
            "look": "neo",
            "themeVariables": { "strokeWidth": 0 },
        }));
        assert_eq!(neo_zero.stroke_width, "1");
        assert_eq!(neo_zero.stroke_width_px, "1px");
        assert_eq!(neo_zero.neo_stroke_width, "0");
        assert_eq!(neo_zero.neo_stroke_width_px, "0px");
        assert_eq!(
            neo_zero.terminal_stroke_width_value(StateTerminalSurface::SpecialChoiceNeo),
            0.0
        );

        let empty = StateCompatibilityPlan::from_value(&json!({
            "look": "neo",
            "themeVariables": { "strokeWidth": "" },
        }));
        assert_eq!(empty.stroke_width, "1");
        assert_eq!(empty.neo_stroke_width, "");
        assert_eq!(empty.neo_stroke_width_px, "px");

        let whitespace = StateCompatibilityPlan::from_value(&json!({
            "look": "neo",
            "themeVariables": { "strokeWidth": "   " },
        }));
        assert_eq!(whitespace.stroke_width, "   ");
        assert_eq!(whitespace.stroke_width_px, "   px");
        assert_eq!(whitespace.neo_stroke_width, "   ");
        assert_eq!(whitespace.neo_stroke_width_px, "   px");
    }

    #[test]
    fn state_use_gradient_follows_javascript_truthiness() {
        for (value, expected) in [
            (json!(false), false),
            (json!(0), false),
            (json!(""), false),
            (json!(true), true),
            (json!(1), true),
            (json!("false"), true),
            (json!("0"), true),
            (json!([]), true),
            (json!({}), true),
        ] {
            let plan = StateCompatibilityPlan::from_value(&json!({
                "themeVariables": { "useGradient": value },
            }));
            assert_eq!(plan.use_gradient, expected, "useGradient={value}");
        }
    }

    #[test]
    fn state_hand_drawn_terminal_is_not_classified_as_classic() {
        let plan = StateCompatibilityPlan::from_value(&json!({ "look": "handDrawn" }));

        assert_eq!(
            plan.node_shape_surface(&ordinary_state_node(), ThemeTarget::State),
            Some(StateTerminalSurface::StateHandDrawn)
        );
    }
}
