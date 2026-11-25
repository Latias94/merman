use std::borrow::Cow;

use merman_model::families::state::StateDiagramRenderNode;
use merman_model::presentation::PresentationConfig;
use merman_theme_contract::v2::EdgeLabelBackground;
use serde_json::Value;

use crate::theme::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StateNodeKind {
    Ordinary,
    Note,
    Composite,
    Special,
}

impl StateNodeKind {
    pub(crate) fn from_node(node: &StateDiagramRenderNode) -> Self {
        match node.shape.as_str() {
            "note" | "noteGroup" => Self::Note,
            "roundedWithTitle" => Self::Composite,
            "stateStart" | "stateEnd" | "choice" | "fork" | "join" => Self::Special,
            _ if node.is_group || node.node_type.as_deref() == Some("group") => Self::Composite,
            _ => Self::Ordinary,
        }
    }
}

/// State surfaces whose paint follows the selected Mermaid look.
///
/// These are writer surfaces rather than authoring targets. Native and HTML transition-label
/// backgrounds, for example, deliberately remain distinct because their terminal owners differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StateTerminalSurface {
    StateClassic,
    StateHandDrawn,
    StateNeo,
    #[cfg(feature = "layout-shoal")]
    Transition,
    NoteClassic,
    NoteHandDrawn,
    NoteNeo,
    CompositeBodyClassic,
    CompositeBodyClassicAlt,
    CompositeDividerClassic,
    CompositeBodyHandDrawn,
    CompositeBodyHandDrawnAlt,
    CompositeDividerHandDrawn,
    CompositeBodyNeo,
    CompositeBodyNeoAlt,
    CompositeDividerNeo,
    CompositeHeaderClassic,
    CompositeHeaderHandDrawn,
    CompositeHeaderNeo,
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

#[derive(Debug, Clone)]
struct StateResolvedValue<T> {
    value: T,
}

enum StateTerminalPaintRef<'a> {
    Css(&'a str),
    ScopedGradient,
}

impl<T> StateResolvedValue<T> {
    fn constant(value: T) -> Self {
        Self { value }
    }

    fn map<U>(self, map: impl FnOnce(T) -> U) -> StateResolvedValue<U> {
        StateResolvedValue {
            value: map(self.value),
        }
    }
}

struct StateCompatibilityResolver<'a> {
    value: &'a Value,
    config: Option<&'a PresentationConfig>,
}

impl<'a> StateCompatibilityResolver<'a> {
    fn new(value: &'a Value, config: Option<&'a PresentationConfig>) -> Self {
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
            return StateResolvedValue { value };
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
            return StateResolvedValue { value };
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
            return StateResolvedValue { value };
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
            return StateResolvedValue { value };
        }
        StateResolvedValue::constant(fallback)
    }

    fn owned_font_family(&self) -> Option<String> {
        // Generated theme defaults must not hide an explicitly configured root font value.
        // Among explicit values, retain Mermaid's themeVariables-before-root precedence.
        ["themeVariables.fontFamily", "fontFamily"]
            .into_iter()
            .filter(|path| self.path_is_owned(path))
            .find_map(|path| {
                let value = value_at_dotted_path(self.value, path)?.as_str()?;
                let css = crate::config::normalize_css_font_family(value);
                (!css.is_empty()).then_some(css)
            })
    }

    fn owned_font_size(&self) -> Option<f64> {
        self.path_is_owned("themeVariables.fontSize")
            .then(|| {
                crate::config::config_f64_css_absolute_px(
                    self.value,
                    &["themeVariables", "fontSize"],
                )
            })
            .flatten()
            .filter(|size| *size >= 0.0)
            .or_else(|| {
                self.path_is_owned("fontSize")
                    .then(|| crate::config::config_f64_css_px(self.value, &["fontSize"]))
                    .flatten()
                    .filter(|size| *size >= 0.0)
            })
    }

    fn path_is_owned(&self, path: &str) -> bool {
        self.config
            .is_some_and(|config| config.explicit_config_owns_path(path))
    }
}

fn state_dark_mode(value: &Value) -> bool {
    let theme = value
        .get("theme")
        .and_then(Value::as_str)
        .unwrap_or("default");
    merman_model::config::MermaidThemeId::parse(theme)
        .is_ok_and(merman_model::config::MermaidThemeId::is_dark)
}

fn state_dependency_numeric_zero_width(value: &Value) -> bool {
    value
        .pointer("/themeVariables/strokeWidth")
        .and_then(Value::as_f64)
        == Some(0.0)
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

#[derive(Debug, Clone, Copy)]
pub(crate) struct StateEdgeLabelPaint {
    pub(crate) mode: EdgeLabelBackground,
    pub(crate) canvas: Color,
    pub(crate) node: Color,
    pub(crate) border: Color,
}

/// State look, paint and typography shared by layout and SVG emission.
#[derive(Debug, Clone)]
pub(crate) struct StateCompatibilityPlan {
    pub(crate) redux_palette: Vec<(String, Option<String>)>,
    pub(crate) palette_look: String,
    pub(crate) serialized_look: String,
    pub(crate) root_font_family_css: String,
    pub(crate) dependency_numeric_zero_width: bool,
    pub(crate) dark_mode: bool,
    look: StateCompatibilityLook,
    html_labels: bool,
    pub(crate) font_family_css: String,
    pub(crate) font_size_px: f64,
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
    neo_rough_stroke_width: String,
    pub(crate) note_border: String,
    pub(crate) note_bkg: String,
    pub(crate) note_text: String,
    pub(crate) label_background: String,
    pub(crate) edge_label_background: String,
    pub(crate) native_edge_label_paint: Option<StateEdgeLabelPaint>,
    pub(crate) transition_label_color: String,
    pub(crate) special_state_color: String,
    pub(crate) inner_end_background: String,
    pub(crate) composite_background: String,
    pub(crate) state_bkg: String,
    pub(crate) state_border: String,
    end_inner_fill: String,
    pub(crate) composite_title_background: String,
    pub(crate) state_label_color: String,
    pub(crate) drop_shadow: String,
    pub(crate) use_gradient: bool,
    pub(crate) gradient_start: String,
    pub(crate) gradient_stop: String,
    pub(crate) neo_radius: f64,
    pub(crate) rect_radius: f64,
    pub(crate) node_shadow: bool,
    pub(crate) native_title: Option<crate::text::TextStyle>,
    pub(crate) native_label_weight: Option<String>,
    pub(crate) native_layout_scale: [f64; 3],
    pub(crate) native_marker: merman_theme_contract::v2::MarkerStyle,
}

impl StateCompatibilityPlan {
    pub(crate) const CLASSIC_COMPOSITE_OUTER_RADIUS: f64 = 5.0;

    pub(crate) fn from_config(config: &PresentationConfig) -> Self {
        Self::resolve(StateCompatibilityResolver::new(
            config.as_value(),
            Some(config),
        ))
    }

    pub(super) fn from_native(
        theme: &crate::theme::FamilyTheme,
        config: &PresentationConfig,
        background: Option<Color>,
    ) -> Self {
        use merman_theme_contract::v2::Density;

        let value = config.as_value();
        let resolver = StateCompatibilityResolver::new(value, Some(config));
        let diagram_look = crate::config::config_diagram_look(value);
        let html_labels = crate::config::config_effective_html_labels(value);
        let owns = |path| config.explicit_config_owns_path(path);
        let roles = &theme.roles;
        let canvas = background.unwrap_or_else(|| roles.canvas.first_color());
        let surface = roles.surface_node.first_color();
        let border = roles.stroke_border.first_color();
        let container = roles.surface_container.first_color().to_string();
        let surface_css = surface.to_string();
        let border_css = border.to_string();
        let canvas_css = canvas.to_string();
        let connector = roles.stroke_connector.first_color().to_string();
        let primary = roles.text_primary.to_string();
        let callout_text = roles.text_callout.to_string();
        let source_edge_background = owns("themeVariables.edgeLabelBackground");
        let source_svg_background = !html_labels && owns("themeVariables.labelBackgroundColor");
        let native_edge_label_paint = (!source_edge_background && !source_svg_background)
            .then_some(StateEdgeLabelPaint {
                mode: theme.form.edge_label_bg,
                canvas,
                node: surface,
                border,
            });
        let (label_background, edge_label_background) = if native_edge_label_paint.is_some() {
            (String::new(), String::new())
        } else {
            let edge = config
                .get_str("themeVariables.edgeLabelBackground")
                .unwrap_or("rgba(232,232,232, 0.8)");
            // The more specific SVG rect rule normally uses labelBackgroundColor.
            let label = if source_edge_background && !source_svg_background {
                edge
            } else {
                config
                    .get_str("themeVariables.labelBackgroundColor")
                    .or_else(|| config.get_str("themeVariables.mainBkg"))
                    .unwrap_or("#ECECFF")
            };
            (label.to_owned(), edge.to_owned())
        };
        let transition_paths = [
            "themeVariables.transitionLabelColor",
            "themeVariables.tertiaryTextColor",
        ];
        let transition_label_color = if transition_paths.into_iter().any(owns) {
            transition_paths
                .into_iter()
                .find_map(|path| config.get_str(path).filter(|color| !color.is_empty()))
                .or_else(|| config.get_str("themeVariables.textColor"))
                .unwrap_or("#333")
                .to_owned()
        } else {
            primary.clone()
        };
        let alt_background = if owns("themeVariables.altBackground") {
            config
                .get_str("themeVariables.altBackground")
                .filter(|color| !color.is_empty())
                .unwrap_or("#efefef")
                .to_owned()
        } else {
            container.clone()
        };
        let composite_background = if owns("themeVariables.compositeBackground") {
            config
                .get_str("themeVariables.compositeBackground")
                .filter(|color| !color.is_empty())
                .or_else(|| config.get_str("themeVariables.background"))
                .unwrap_or("white")
                .to_owned()
        } else {
            container
        };
        let composite_title_background = if owns("themeVariables.compositeTitleBackground") {
            config
                .get_str("themeVariables.compositeTitleBackground")
                .or_else(|| config.get_str("themeVariables.mainBkg"))
                .unwrap_or("#ECECFF")
                .to_owned()
        } else {
            roles.surface_emphasis.first_color().to_string()
        };
        let (padding, spacing) = match theme.form.density {
            Density::Compact => (0.85, 0.85),
            Density::Balanced => (1.0, 1.0),
            Density::Airy => (1.25, 1.2),
        };
        let native_layout_scale = [
            ("state.padding", padding),
            ("state.nodeSpacing", spacing),
            ("state.rankSpacing", spacing),
        ]
        .map(|(path, scale)| if owns(path) { 1.0 } else { scale });

        let typography = &theme.typography;
        let (font_family_css, root_font_family_css) =
            if let Some(font) = resolver.owned_font_family() {
                (font, crate::config::config_root_font_family_css(value))
            } else {
                let font = crate::theme::font_family_css(typography.stack(typography.body_family));
                (font.clone(), font)
            };
        let font_size_px = resolver
            .owned_font_size()
            .unwrap_or(typography.base_size_px);
        let width = crate::number_format::canonical_number(theme.form.stroke_width).to_string();
        let width_px = format!("{width}px");
        Self {
            redux_palette: Vec::new(),
            palette_look: String::new(),
            serialized_look: diagram_look.serialized().to_owned(),
            root_font_family_css,
            dependency_numeric_zero_width: state_dependency_numeric_zero_width(value),
            dark_mode: state_dark_mode(value),
            look: StateCompatibilityLook::from_value(diagram_look.as_str()),
            html_labels,
            font_family_css,
            font_size_px,
            text_color: primary.clone(),
            line_color: connector.clone(),
            error_bkg: roles.status_danger.to_string(),
            error_text: callout_text.clone(),
            transition_color: connector.clone(),
            node_border: border_css.clone(),
            background: canvas_css.clone(),
            main_bkg: surface_css.clone(),
            alt_background,
            stroke_width: width.clone(),
            stroke_width_px: width_px.clone(),
            neo_stroke_width: width.clone(),
            neo_stroke_width_px: width_px,
            neo_rough_stroke_width: width,
            note_border: border_css.clone(),
            note_bkg: roles.surface_callout.first_color().to_string(),
            note_text: callout_text,
            label_background,
            edge_label_background,
            native_edge_label_paint,
            transition_label_color,
            special_state_color: connector.clone(),
            inner_end_background: canvas_css,
            composite_background,
            state_bkg: surface_css,
            state_border: border_css,
            end_inner_fill: connector,
            composite_title_background,
            state_label_color: primary,
            drop_shadow: "none".to_owned(),
            use_gradient: false,
            gradient_start: String::new(),
            gradient_stop: String::new(),
            neo_radius: theme.form.radius,
            rect_radius: theme.form.radius,
            node_shadow: false,
            native_title: Some(crate::text::TextStyle {
                font_family: Some(crate::theme::font_family_css(
                    typography.stack(typography.title_family),
                )),
                font_size: typography.base_size_px * 1.2,
                font_weight: Some(typography.title_weight.to_string()),
                ..Default::default()
            }),
            native_label_weight: Some(typography.label_weight.to_string()),
            native_layout_scale,
            native_marker: theme.form.marker_style,
        }
    }

    #[cfg(any(feature = "layout-dagre", test))]
    pub(crate) fn from_value(value: &Value) -> Self {
        Self::resolve(StateCompatibilityResolver::new(value, None))
    }

    fn resolve(resolver: StateCompatibilityResolver<'_>) -> Self {
        let diagram_look = crate::config::config_diagram_look(resolver.value);
        let look = StateCompatibilityLook::from_value(diagram_look.as_str());
        let html_labels = crate::config::config_effective_html_labels(resolver.value);
        let font_family_css = resolver
            .owned_font_family()
            .unwrap_or_else(|| crate::config::config_font_family_css(resolver.value));
        let font_size_px = resolver.owned_font_size().unwrap_or_else(|| {
            crate::config::config_theme_or_root_font_size_px(resolver.value, 16.0)
        });

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
        let end_inner_fill =
            resolver.nullish_string(&["themeVariables.stateBorder"], || node_border.clone());
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
        let neo_radius = resolver.direct_css_px(&["themeVariables.radius"], 5.0);
        let neo_radius = neo_radius.map(|value| value.max(0.0));
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

        let redux_palette = if matches!(
            resolver
                .value
                .get("theme")
                .and_then(serde_json::Value::as_str),
            Some("redux-color" | "redux-dark-color")
        ) {
            let css_color = |value: &serde_json::Value| {
                value
                    .as_str()
                    .map(|value| value.trim().to_owned())
                    .unwrap_or_else(|| value.to_string())
            };
            let backgrounds = resolver
                .value
                .pointer("/themeVariables/bkgColorArray")
                .and_then(serde_json::Value::as_array)
                .filter(|values| !values.is_empty());
            resolver
                .value
                .pointer("/themeVariables/borderColorArray")
                .and_then(serde_json::Value::as_array)
                .map(|borders| {
                    borders
                        .iter()
                        .enumerate()
                        .map(|(index, border)| {
                            (
                                css_color(border),
                                backgrounds.map(|colors| css_color(&colors[index % colors.len()])),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let palette_look = resolver
            .value
            .get("look")
            .and_then(|value| match value {
                serde_json::Value::String(value) => Some(value.clone()),
                serde_json::Value::Number(value) => Some(value.to_string()),
                _ => None,
            })
            .filter(|look| {
                !look.is_empty()
                    && look
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            })
            .unwrap_or_else(|| "classic".to_owned());
        Self {
            redux_palette,
            palette_look,
            serialized_look: diagram_look.serialized().to_owned(),
            root_font_family_css: crate::config::config_root_font_family_css(resolver.value),
            dependency_numeric_zero_width: state_dependency_numeric_zero_width(resolver.value),
            dark_mode: state_dark_mode(resolver.value),
            look,
            html_labels,
            font_family_css,
            font_size_px,
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
            neo_rough_stroke_width: neo_rough_stroke_width.value,
            note_border: note_border.value,
            note_bkg: note_bkg.value,
            note_text: note_text.value,
            label_background: label_background.value,
            edge_label_background: edge_label_background.value,
            native_edge_label_paint: None,
            transition_label_color: transition_label_color.value,
            special_state_color: special_state_color.value,
            inner_end_background: inner_end_background.value,
            composite_background: composite_background.value,
            state_bkg: state_bkg.value,
            state_border: state_border.value,
            end_inner_fill: end_inner_fill.value,
            composite_title_background: composite_title_background.value,
            state_label_color: state_label_color.value,
            drop_shadow: drop_shadow.value,
            use_gradient: use_gradient.value,
            gradient_start: gradient_start.value,
            gradient_stop: gradient_stop.value,
            neo_radius: neo_radius.value,
            rect_radius,
            node_shadow,
            native_title: None,
            native_label_weight: None,
            native_layout_scale: [1.0; 3],
            native_marker: merman_theme_contract::v2::MarkerStyle::Classic,
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

    fn terminal_paint(
        &self,
        surface: StateTerminalSurface,
        property: StateTerminalPaintProperty,
    ) -> Option<StateTerminalPaintRef<'_>> {
        use StateTerminalPaintProperty::{Fill, Stroke, StrokeWidth};
        use StateTerminalSurface::*;

        Some(match (surface, property) {
            (StateClassic | StateNeo, Fill) => StateTerminalPaintRef::Css(&self.state_bkg),
            (
                StateClassic
                | CompositeBodyClassic
                | CompositeBodyClassicAlt
                | CompositeDividerClassic
                | CompositeHeaderClassic,
                Stroke,
            ) => StateTerminalPaintRef::Css(&self.state_border),
            (
                StateClassic
                | StateNeo
                | CompositeBodyClassic
                | CompositeBodyClassicAlt
                | CompositeDividerClassic
                | CompositeHeaderClassic,
                StrokeWidth,
            ) => StateTerminalPaintRef::Css(&self.stroke_width_px),
            (
                StateHandDrawn
                | CompositeDividerNeo
                | CompositeHeaderNeo
                | SpecialEndOuterClassic
                | SpecialEndOuterHandDrawn
                | SpecialEndOuterNeo
                | SpecialChoiceClassic
                | SpecialChoiceHandDrawn
                | SpecialChoiceNeo,
                Fill,
            ) => StateTerminalPaintRef::Css(&self.main_bkg),
            (
                StateHandDrawn
                | CompositeBodyHandDrawn
                | CompositeBodyHandDrawnAlt
                | CompositeDividerHandDrawn
                | CompositeHeaderHandDrawn
                | SpecialChoiceClassic
                | SpecialChoiceHandDrawn,
                Stroke,
            ) => StateTerminalPaintRef::Css(&self.node_border),
            (
                StateHandDrawn
                | NoteClassic
                | NoteHandDrawn
                | SpecialForkJoinClassic
                | SpecialForkJoinHandDrawn
                | SpecialChoiceClassic
                | SpecialChoiceHandDrawn,
                StrokeWidth,
            ) => StateTerminalPaintRef::Css("1.3"),
            (
                StateNeo | NoteNeo | SpecialStartNeo | SpecialEndOuterNeo | SpecialForkJoinNeo
                | SpecialChoiceNeo,
                Stroke,
            ) if self.use_gradient => StateTerminalPaintRef::ScopedGradient,
            (
                StateNeo | NoteNeo | SpecialStartNeo | SpecialEndOuterNeo | SpecialForkJoinNeo
                | SpecialChoiceNeo,
                Stroke,
            ) => StateTerminalPaintRef::Css(&self.node_border),
            #[cfg(feature = "layout-shoal")]
            (Transition, StrokeWidth) => StateTerminalPaintRef::Css(&self.stroke_width),
            #[cfg(feature = "layout-shoal")]
            (Transition, Fill | Stroke) => return None,
            (NoteClassic | NoteHandDrawn | NoteNeo, Fill) => {
                StateTerminalPaintRef::Css(&self.note_bkg)
            }
            (NoteClassic | NoteHandDrawn, Stroke) => StateTerminalPaintRef::Css(&self.note_border),
            (NoteNeo | SpecialEndOuterNeo | SpecialForkJoinNeo | SpecialChoiceNeo, StrokeWidth) => {
                StateTerminalPaintRef::Css(&self.neo_rough_stroke_width)
            }
            // The `.inner` rules outrank the generic Neo fill on both ordinary and alt bodies.
            (CompositeBodyClassic | CompositeBodyHandDrawn | CompositeBodyNeo, Fill) => {
                StateTerminalPaintRef::Css(&self.composite_background)
            }
            (
                CompositeBodyClassicAlt
                | CompositeDividerClassic
                | CompositeBodyHandDrawnAlt
                | CompositeBodyNeoAlt,
                Fill,
            ) => StateTerminalPaintRef::Css(&self.alt_background),
            (CompositeHeaderClassic | CompositeHeaderHandDrawn, Fill) => {
                StateTerminalPaintRef::Css(&self.composite_title_background)
            }
            (
                CompositeBodyHandDrawn
                | CompositeBodyHandDrawnAlt
                | CompositeDividerHandDrawn
                | CompositeHeaderHandDrawn
                | SpecialStartClassic
                | SpecialStartHandDrawn
                | SpecialStartNeo,
                StrokeWidth,
            ) => StateTerminalPaintRef::Css("1"),
            (CompositeDividerHandDrawn, Fill) => StateTerminalPaintRef::Css("lightgrey"),
            (
                CompositeBodyNeo | CompositeBodyNeoAlt | CompositeDividerNeo | CompositeHeaderNeo,
                Stroke,
            ) if self.use_gradient => StateTerminalPaintRef::ScopedGradient,
            (
                CompositeBodyNeo | CompositeBodyNeoAlt | CompositeDividerNeo | CompositeHeaderNeo,
                Stroke,
            ) => StateTerminalPaintRef::Css(&self.state_border),
            (
                CompositeBodyNeo | CompositeBodyNeoAlt | CompositeDividerNeo | CompositeHeaderNeo,
                StrokeWidth,
            ) => StateTerminalPaintRef::Css(&self.neo_stroke_width),
            (SpecialStartClassic | SpecialStartNeo, Fill) => {
                StateTerminalPaintRef::Css(&self.special_state_color)
            }
            (SpecialStartClassic, Stroke) => StateTerminalPaintRef::Css(&self.special_state_color),
            (
                SpecialStartHandDrawn
                | SpecialForkJoinClassic
                | SpecialForkJoinHandDrawn
                | SpecialForkJoinNeo,
                Fill,
            ) => StateTerminalPaintRef::Css(&self.line_color),
            (
                SpecialStartHandDrawn
                | SpecialEndOuterClassic
                | SpecialEndOuterHandDrawn
                | SpecialForkJoinClassic
                | SpecialForkJoinHandDrawn,
                Stroke,
            ) => StateTerminalPaintRef::Css(&self.line_color),
            (
                SpecialEndOuterClassic | SpecialEndOuterHandDrawn | SpecialEndInnerClassic,
                StrokeWidth,
            ) => StateTerminalPaintRef::Css("2"),
            (SpecialEndInnerClassic, Fill | Stroke) => {
                StateTerminalPaintRef::Css(&self.end_inner_fill)
            }
        })
    }

    pub(crate) fn terminal_paint_css<I: std::fmt::Display>(
        &self,
        surface: StateTerminalSurface,
        property: StateTerminalPaintProperty,
        diagram_id: I,
    ) -> Cow<'_, str> {
        match self
            .terminal_paint(surface, property)
            .unwrap_or_else(|| panic!("missing State terminal paint for {surface:?}.{property:?}"))
        {
            StateTerminalPaintRef::Css(value) => Cow::Borrowed(value),
            StateTerminalPaintRef::ScopedGradient => {
                Cow::Owned(format!("url(#{diagram_id}-gradient)"))
            }
        }
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
        target: StateNodeKind,
    ) -> Option<StateTerminalSurface> {
        match target {
            StateNodeKind::Ordinary if self.is_neo() => Some(StateTerminalSurface::StateNeo),
            StateNodeKind::Ordinary if self.is_hand_drawn() => {
                Some(StateTerminalSurface::StateHandDrawn)
            }
            StateNodeKind::Ordinary => Some(StateTerminalSurface::StateClassic),
            StateNodeKind::Note if self.is_neo() => Some(StateTerminalSurface::NoteNeo),
            StateNodeKind::Note if self.is_hand_drawn() => {
                Some(StateTerminalSurface::NoteHandDrawn)
            }
            StateNodeKind::Note => Some(StateTerminalSurface::NoteClassic),
            StateNodeKind::Composite if self.is_neo() && node.shape == "divider" => {
                Some(StateTerminalSurface::CompositeDividerNeo)
            }
            StateNodeKind::Composite
                if self.is_neo()
                    && node
                        .css_classes
                        .split_whitespace()
                        .any(|class| class == "statediagram-cluster-alt") =>
            {
                Some(StateTerminalSurface::CompositeBodyNeoAlt)
            }
            StateNodeKind::Composite if self.is_neo() => {
                Some(StateTerminalSurface::CompositeBodyNeo)
            }
            StateNodeKind::Composite if self.is_hand_drawn() && node.shape == "divider" => {
                Some(StateTerminalSurface::CompositeDividerHandDrawn)
            }
            StateNodeKind::Composite
                if self.is_hand_drawn()
                    && node
                        .css_classes
                        .split_whitespace()
                        .any(|class| class == "statediagram-cluster-alt") =>
            {
                Some(StateTerminalSurface::CompositeBodyHandDrawnAlt)
            }
            StateNodeKind::Composite if self.is_hand_drawn() => {
                Some(StateTerminalSurface::CompositeBodyHandDrawn)
            }
            StateNodeKind::Composite if node.shape == "divider" => {
                Some(StateTerminalSurface::CompositeDividerClassic)
            }
            StateNodeKind::Composite
                if node
                    .css_classes
                    .split_whitespace()
                    .any(|class| class == "statediagram-cluster-alt") =>
            {
                Some(StateTerminalSurface::CompositeBodyClassicAlt)
            }
            StateNodeKind::Composite => Some(StateTerminalSurface::CompositeBodyClassic),
            StateNodeKind::Special => match node.shape.as_str() {
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
    use merman_model::families::state::StateDiagramRenderNode;
    use serde_json::json;

    #[test]
    fn redux_palette_binding_keeps_gate_coercion_and_cyclic_backgrounds() {
        let config = json!({
            "theme": "redux-color", "look": "neo", "fontFamily": "Root Sans",
            "themeVariables": {
                "borderColorArray": [" red ", 12, null],
                "bkgColorArray": [" var(--tint) "], "strokeWidth": 0
            }
        });
        let plan = StateCompatibilityPlan::from_value(&config);
        assert_eq!(
            plan.redux_palette,
            vec![
                ("red".to_owned(), Some("var(--tint)".to_owned())),
                ("12".to_owned(), Some("var(--tint)".to_owned())),
                ("null".to_owned(), Some("var(--tint)".to_owned())),
            ]
        );
        assert_eq!(plan.palette_look, "neo");
        assert_eq!(plan.root_font_family_css, "Root Sans");
        assert!(plan.dependency_numeric_zero_width);
        let mut string_zero = config.clone();
        string_zero["themeVariables"]["strokeWidth"] = json!("0");
        assert!(!StateCompatibilityPlan::from_value(&string_zero).dependency_numeric_zero_width);
        let mut ungated = config;
        ungated["theme"] = json!("redux");
        assert!(
            StateCompatibilityPlan::from_value(&ungated)
                .redux_palette
                .is_empty()
        );
    }

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
        assert_eq!(empty.end_inner_fill, "");

        let whitespace = StateCompatibilityPlan::from_value(&json!({
            "themeVariables": {
                "stateBorder": "   ",
                "nodeBorder": "#123456",
            }
        }));

        assert_eq!(whitespace.state_border, "   ");
        assert_eq!(whitespace.end_inner_fill, "   ");
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
            neo_zero.terminal_stroke_width_value(StateTerminalSurface::StateNeo),
            1.0
        );
        assert_eq!(
            neo_zero.terminal_stroke_width_value(StateTerminalSurface::CompositeBodyNeo),
            0.0
        );
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
    fn gradient_scoping_formats_each_id_only_when_its_terminal_uses_a_gradient() {
        struct DiagramId<'a>(&'a std::cell::Cell<usize>, &'static str);

        impl std::fmt::Display for DiagramId<'_> {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.set(self.0.get() + 1);
                formatter.write_str(self.1)
            }
        }

        let calls = std::cell::Cell::new(0);
        let plan = StateCompatibilityPlan::from_value(&json!({
            "themeVariables": { "useGradient": true },
        }));
        let fill = plan.terminal_paint_css(
            StateTerminalSurface::StateNeo,
            StateTerminalPaintProperty::Fill,
            DiagramId(&calls, "unused"),
        );
        assert!(matches!(fill, Cow::Borrowed(_)));
        assert_eq!(calls.get(), 0);

        let first = plan.terminal_paint_css(
            StateTerminalSurface::StateNeo,
            StateTerminalPaintProperty::Stroke,
            DiagramId(&calls, "first"),
        );
        let second = plan.terminal_paint_css(
            StateTerminalSurface::CompositeHeaderNeo,
            StateTerminalPaintProperty::Stroke,
            DiagramId(&calls, "second"),
        );
        assert_eq!(first, "url(#first-gradient)");
        assert_eq!(second, "url(#second-gradient)");
        assert!(matches!(first, Cow::Owned(_)));
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn state_hand_drawn_terminal_is_not_classified_as_classic() {
        let plan = StateCompatibilityPlan::from_value(&json!({ "look": "handDrawn" }));

        assert_eq!(
            plan.node_shape_surface(&ordinary_state_node(), StateNodeKind::Ordinary),
            Some(StateTerminalSurface::StateHandDrawn)
        );
    }
}
