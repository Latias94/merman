use merman_core::MermaidConfig;
use serde_json::{Map, Value};
use std::sync::OnceLock;

use crate::presentation::{
    HostTheme as PresentationHostTheme, HostThemeAppearance as PresentationAppearance,
    HostThemePreset as PresentationThemePreset, Presentation, PresentationProfile, ThemeRole,
};

use super::pipeline::{CssOverridePolicy, SvgOutputPolicy, SvgPipeline, SvgPipelinePreset};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HostThemeAppearance {
    #[default]
    Light,
    Dark,
}

impl HostThemeAppearance {
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HostThemePreset {
    /// Neutral light editor preview palette.
    #[default]
    EditorLight,
    /// Neutral dark editor preview palette.
    EditorDark,
    /// One Dark-inspired editor preview palette.
    OneDark,
    /// Gruvbox light-inspired editor preview palette.
    GruvboxLight,
    /// Gruvbox dark-inspired editor preview palette.
    GruvboxDark,
    /// Ayu light-inspired editor preview palette.
    AyuLight,
    /// Ayu dark-inspired editor preview palette.
    AyuDark,
    /// Merman's modern flowchart rendering profile.
    MermanModern,
    /// Upstream Mermaid rendering defaults and parity output.
    Mermaid,
}

impl HostThemePreset {
    /// All built-in host profile presets.
    pub const ALL: [Self; 9] = [
        Self::EditorLight,
        Self::EditorDark,
        Self::OneDark,
        Self::GruvboxLight,
        Self::GruvboxDark,
        Self::AyuLight,
        Self::AyuDark,
        Self::MermanModern,
        Self::Mermaid,
    ];

    /// Stable `host_theme.preset` value accepted by bindings.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EditorLight => "editor-light",
            Self::EditorDark => "editor-dark",
            Self::OneDark => "one-dark",
            Self::GruvboxLight => "gruvbox-light",
            Self::GruvboxDark => "gruvbox-dark",
            Self::AyuLight => "ayu-light",
            Self::AyuDark => "ayu-dark",
            Self::MermanModern => "merman-modern",
            Self::Mermaid => "mermaid",
        }
    }
}

/// Returns built-in host/editor theme preset names.
///
/// These are semantic host presets such as `one-dark` and are intentionally separate from Mermaid
/// core theme names returned by `merman_core::supported_themes()`.
pub fn supported_host_theme_presets() -> &'static [&'static str] {
    static NAMES: OnceLock<Vec<&'static str>> = OnceLock::new();
    NAMES
        .get_or_init(|| {
            HostThemePreset::ALL
                .iter()
                .copied()
                .map(HostThemePreset::as_str)
                .collect()
        })
        .as_slice()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostThemeRoles {
    pub canvas: Option<String>,
    pub surface: Option<String>,
    pub surface_alt: Option<String>,
    pub surface_muted: Option<String>,
    pub text: Option<String>,
    pub subtle_text: Option<String>,
    pub border: Option<String>,
    pub line: Option<String>,
    pub edge_label_background: Option<String>,
    pub cluster_background: Option<String>,
    pub cluster_border: Option<String>,
    pub note_background: Option<String>,
    pub note_border: Option<String>,
    pub note_text: Option<String>,
    pub actor_background: Option<String>,
    pub actor_border: Option<String>,
    pub actor_text: Option<String>,
    pub activation_background: Option<String>,
    pub activation_border: Option<String>,
    pub error: Option<String>,
    pub warning: Option<String>,
    pub success: Option<String>,
}

impl HostThemeRoles {
    fn has_values(&self) -> bool {
        self.canvas.is_some()
            || self.surface.is_some()
            || self.surface_alt.is_some()
            || self.surface_muted.is_some()
            || self.text.is_some()
            || self.subtle_text.is_some()
            || self.border.is_some()
            || self.line.is_some()
            || self.edge_label_background.is_some()
            || self.cluster_background.is_some()
            || self.cluster_border.is_some()
            || self.note_background.is_some()
            || self.note_border.is_some()
            || self.note_text.is_some()
            || self.actor_background.is_some()
            || self.actor_border.is_some()
            || self.actor_text.is_some()
            || self.activation_background.is_some()
            || self.activation_border.is_some()
            || self.error.is_some()
            || self.warning.is_some()
            || self.success.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HostThemePipelinePreset {
    /// Keep Mermaid-parity SVG output.
    #[default]
    Parity,
    /// Keep native `<foreignObject>` labels and add readable SVG text fallbacks.
    ///
    /// This is useful for consumers that need both browser-like SVG and non-HTML label fallbacks.
    /// For browser/editor display surfaces, prefer [`Self::ResvgSafe`] if duplicate labels are a
    /// risk.
    Readable,
    /// Add readable fallback text, remove native `<foreignObject>` labels, and sanitize common
    /// rasterization hazards.
    ResvgSafe,
}

impl From<HostThemePipelinePreset> for SvgPipelinePreset {
    fn from(value: HostThemePipelinePreset) -> Self {
        match value {
            HostThemePipelinePreset::Parity => Self::Parity,
            HostThemePipelinePreset::Readable => Self::Readable,
            HostThemePipelinePreset::ResvgSafe => Self::ResvgSafe,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum HostThemeRootBackground {
    #[default]
    None,
    Canvas,
    Color(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostThemeOutput {
    pub pipeline: HostThemePipelinePreset,
    pub css_override_policy: CssOverridePolicy,
    pub root_background: HostThemeRootBackground,
    pub drop_native_duplicate_fallbacks: bool,
    pub scoped_css: Option<String>,
}

impl Default for HostThemeOutput {
    fn default() -> Self {
        Self {
            pipeline: HostThemePipelinePreset::Parity,
            css_override_policy: CssOverridePolicy::Preserve,
            root_background: HostThemeRootBackground::None,
            drop_native_duplicate_fallbacks: false,
            scoped_css: None,
        }
    }
}

impl HostThemeOutput {
    /// Returns product-neutral defaults for editor previews and raster-oriented host surfaces.
    ///
    /// The preset selects `resvg-safe` output, strips existing `!important` CSS so host theme rules
    /// can win predictably, and uses the profile canvas as the root SVG background. Duplicate
    /// native/fallback text cleanup stays opt-in because repeated labels can be intentional in
    /// unrelated nodes. Callers can still add scoped CSS or override individual fields.
    pub fn resvg_safe_editor() -> Self {
        Self {
            pipeline: HostThemePipelinePreset::ResvgSafe,
            css_override_policy: CssOverridePolicy::StripExistingImportant,
            root_background: HostThemeRootBackground::Canvas,
            drop_native_duplicate_fallbacks: false,
            scoped_css: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostThemeProfile {
    pub appearance: HostThemeAppearance,
    pub font_family: Option<String>,
    pub font_size: Option<String>,
    pub roles: HostThemeRoles,
    pub series_palette: Vec<String>,
    pub output: HostThemeOutput,
    pub theme_variables: Map<String, Value>,
    pub site_config: Map<String, Value>,
}

impl Default for HostThemeProfile {
    fn default() -> Self {
        Self {
            appearance: HostThemeAppearance::Light,
            font_family: None,
            font_size: None,
            roles: HostThemeRoles::default(),
            series_palette: Vec::new(),
            output: HostThemeOutput::default(),
            theme_variables: Map::new(),
            site_config: Map::new(),
        }
    }
}

impl HostThemeProfile {
    pub fn builder() -> HostThemeProfileBuilder {
        HostThemeProfileBuilder::default()
    }

    pub fn from_preset(preset: HostThemePreset) -> Self {
        match preset {
            HostThemePreset::EditorLight => Self::editor_light(),
            HostThemePreset::EditorDark => Self::editor_dark(),
            HostThemePreset::OneDark => Self::one_dark(),
            HostThemePreset::GruvboxLight => Self::gruvbox_light(),
            HostThemePreset::GruvboxDark => Self::gruvbox_dark(),
            HostThemePreset::AyuLight => Self::ayu_light(),
            HostThemePreset::AyuDark => Self::ayu_dark(),
            HostThemePreset::MermanModern => Self::merman_modern(),
            HostThemePreset::Mermaid => Self::mermaid(),
        }
    }

    /// Uses Merman's modern flowchart defaults without changing the SVG output pipeline.
    pub fn merman_modern() -> Self {
        let resolved = Presentation::new()
            .with_profile(PresentationProfile::MermanModern)
            .resolve();
        let mut site_config = resolved
            .mermaid_config()
            .as_value()
            .as_object()
            .cloned()
            .unwrap_or_default();
        if let Some(policy) = resolved.flowchart_policy() {
            let flowchart = site_config
                .entry("flowchart")
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .expect("presentation profile flowchart config must be an object");
            if let Some(radius) = policy.edge_corner_radius {
                flowchart.insert("edgeCornerRadius".to_string(), legacy_number(radius));
            }
            flowchart.insert(
                "edgeLabelPadding".to_string(),
                legacy_number(policy.edge_label_padding),
            );
            flowchart.insert(
                "compactEdgeCorners".to_string(),
                Value::Bool(policy.compact_edge_corners),
            );
        }

        Self {
            site_config,
            ..Self::default()
        }
    }

    /// Uses upstream Mermaid defaults and parity SVG output.
    pub fn mermaid() -> Self {
        Self::default()
    }

    pub fn editor_light() -> Self {
        Self::from_presentation_theme(PresentationThemePreset::EditorLight)
    }

    pub fn editor_dark() -> Self {
        Self::from_presentation_theme(PresentationThemePreset::EditorDark)
    }

    pub fn one_dark() -> Self {
        Self::from_presentation_theme(PresentationThemePreset::OneDark)
    }

    pub fn gruvbox_light() -> Self {
        Self::from_presentation_theme(PresentationThemePreset::GruvboxLight)
    }

    pub fn gruvbox_dark() -> Self {
        Self::from_presentation_theme(PresentationThemePreset::GruvboxDark)
    }

    pub fn ayu_light() -> Self {
        Self::from_presentation_theme(PresentationThemePreset::AyuLight)
    }

    pub fn ayu_dark() -> Self {
        Self::from_presentation_theme(PresentationThemePreset::AyuDark)
    }

    fn from_presentation_theme(preset: PresentationThemePreset) -> Self {
        let theme = PresentationHostTheme::from_preset(preset);
        Self {
            appearance: match theme.appearance().unwrap_or_default() {
                PresentationAppearance::Light => HostThemeAppearance::Light,
                PresentationAppearance::Dark => HostThemeAppearance::Dark,
            },
            font_family: theme.font_family().map(str::to_string),
            font_size: theme.font_size().map(str::to_string),
            roles: legacy_roles(&theme),
            series_palette: theme.series_palette().to_vec(),
            output: HostThemeOutput::resvg_safe_editor(),
            theme_variables: Map::new(),
            site_config: Map::new(),
        }
    }

    pub fn compile(&self) -> CompiledHostTheme {
        let has_profile_theme_input = self.appearance.is_dark()
            || self.font_family.is_some()
            || self.font_size.is_some()
            || self.roles.has_values()
            || !self.series_palette.is_empty()
            || !self.theme_variables.is_empty();
        let appearance = has_profile_theme_input.then_some(match self.appearance {
            HostThemeAppearance::Light => PresentationAppearance::Light,
            HostThemeAppearance::Dark => PresentationAppearance::Dark,
        });
        let theme = PresentationHostTheme::from_parts_unchecked(
            appearance,
            nonempty(self.font_family.as_deref()),
            nonempty(self.font_size.as_deref()),
            presentation_roles(&self.roles),
            self.series_palette.clone(),
        );
        let mut site_config = theme.mermaid_config_patch();
        if !self.theme_variables.is_empty() {
            let mut override_root = Map::new();
            override_root.insert(
                "themeVariables".to_string(),
                Value::Object(self.theme_variables.clone()),
            );
            site_config.deep_merge(&Value::Object(override_root));
        }
        site_config.deep_merge(&Value::Object(self.site_config.clone()));

        let canvas_color = site_config
            .as_value()
            .get("themeVariables")
            .and_then(Value::as_object)
            .and_then(|variables| variables.get("background"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|color| !color.is_empty())
            .map(str::to_owned);

        CompiledHostTheme {
            site_config,
            output: SvgOutputPolicy {
                preset: self.output.pipeline.into(),
                css_override_policy: self.output.css_override_policy,
                root_background_color: match &self.output.root_background {
                    HostThemeRootBackground::None => None,
                    HostThemeRootBackground::Canvas => canvas_color,
                    HostThemeRootBackground::Color(color) => Some(color.clone()),
                },
                drop_native_duplicate_fallbacks: self.output.drop_native_duplicate_fallbacks,
                scoped_css: self.output.scoped_css.clone(),
            },
        }
    }
}

fn legacy_roles(theme: &PresentationHostTheme) -> HostThemeRoles {
    let role = |role| theme.role(role).map(str::to_string);
    HostThemeRoles {
        canvas: role(ThemeRole::Canvas),
        surface: role(ThemeRole::Surface),
        surface_alt: role(ThemeRole::SurfaceAlt),
        surface_muted: role(ThemeRole::SurfaceMuted),
        text: role(ThemeRole::Text),
        subtle_text: role(ThemeRole::SubtleText),
        border: role(ThemeRole::Border),
        line: role(ThemeRole::Line),
        edge_label_background: role(ThemeRole::EdgeLabelBackground),
        cluster_background: role(ThemeRole::ClusterBackground),
        cluster_border: role(ThemeRole::ClusterBorder),
        note_background: role(ThemeRole::NoteBackground),
        note_border: role(ThemeRole::NoteBorder),
        note_text: role(ThemeRole::NoteText),
        actor_background: role(ThemeRole::ActorBackground),
        actor_border: role(ThemeRole::ActorBorder),
        actor_text: role(ThemeRole::ActorText),
        activation_background: role(ThemeRole::ActivationBackground),
        activation_border: role(ThemeRole::ActivationBorder),
        error: role(ThemeRole::Error),
        warning: role(ThemeRole::Warning),
        success: role(ThemeRole::Success),
    }
}

fn presentation_roles(
    roles: &HostThemeRoles,
) -> [(ThemeRole, Option<String>); ThemeRole::ALL.len()] {
    [
        (ThemeRole::Canvas, roles.canvas.clone()),
        (ThemeRole::Surface, roles.surface.clone()),
        (ThemeRole::SurfaceAlt, roles.surface_alt.clone()),
        (ThemeRole::SurfaceMuted, roles.surface_muted.clone()),
        (ThemeRole::Text, roles.text.clone()),
        (ThemeRole::SubtleText, roles.subtle_text.clone()),
        (ThemeRole::Border, roles.border.clone()),
        (ThemeRole::Line, roles.line.clone()),
        (
            ThemeRole::EdgeLabelBackground,
            roles.edge_label_background.clone(),
        ),
        (
            ThemeRole::ClusterBackground,
            roles.cluster_background.clone(),
        ),
        (ThemeRole::ClusterBorder, roles.cluster_border.clone()),
        (ThemeRole::NoteBackground, roles.note_background.clone()),
        (ThemeRole::NoteBorder, roles.note_border.clone()),
        (ThemeRole::NoteText, roles.note_text.clone()),
        (ThemeRole::ActorBackground, roles.actor_background.clone()),
        (ThemeRole::ActorBorder, roles.actor_border.clone()),
        (ThemeRole::ActorText, roles.actor_text.clone()),
        (
            ThemeRole::ActivationBackground,
            roles.activation_background.clone(),
        ),
        (ThemeRole::ActivationBorder, roles.activation_border.clone()),
        (ThemeRole::Error, roles.error.clone()),
        (ThemeRole::Warning, roles.warning.clone()),
        (ThemeRole::Success, roles.success.clone()),
    ]
}

fn nonempty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn legacy_number(value: f64) -> Value {
    if value.fract() == 0.0 && value >= i64::MIN as f64 && value <= i64::MAX as f64 {
        Value::from(value as i64)
    } else {
        Value::from(value)
    }
}

#[derive(Debug, Clone, Default)]
pub struct HostThemeProfileBuilder {
    profile: HostThemeProfile,
}

impl HostThemeProfileBuilder {
    pub fn appearance(mut self, appearance: HostThemeAppearance) -> Self {
        self.profile.appearance = appearance;
        self
    }

    pub fn font_family(mut self, font_family: impl Into<String>) -> Self {
        self.profile.font_family = Some(font_family.into());
        self
    }

    pub fn font_size(mut self, font_size: impl Into<String>) -> Self {
        self.profile.font_size = Some(font_size.into());
        self
    }

    pub fn roles(mut self, roles: HostThemeRoles) -> Self {
        self.profile.roles = roles;
        self
    }

    pub fn series_palette(mut self, palette: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.profile.series_palette = palette.into_iter().map(Into::into).collect();
        self
    }

    pub fn output(mut self, output: HostThemeOutput) -> Self {
        self.profile.output = output;
        self
    }

    pub fn theme_variable(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.profile
            .theme_variables
            .insert(key.into(), value.into());
        self
    }

    pub fn site_config(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.profile.site_config.insert(key.into(), value.into());
        self
    }

    pub fn build(self) -> HostThemeProfile {
        self.profile
    }
}

#[derive(Debug, Clone)]
pub struct CompiledHostTheme {
    pub site_config: MermaidConfig,
    pub output: SvgOutputPolicy,
}

impl CompiledHostTheme {
    pub fn into_parts(self) -> (MermaidConfig, SvgOutputPolicy) {
        (self.site_config, self.output)
    }

    pub fn pipeline(&self) -> SvgPipeline {
        self.output.pipeline()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sentinel_roles() -> HostThemeRoles {
        HostThemeRoles {
            canvas: Some("#010101".to_string()),
            surface: Some("#020202".to_string()),
            surface_alt: Some("#030303".to_string()),
            surface_muted: Some("#040404".to_string()),
            text: Some("#050505".to_string()),
            subtle_text: Some("#060606".to_string()),
            border: Some("#070707".to_string()),
            line: Some("#080808".to_string()),
            edge_label_background: Some("#090909".to_string()),
            cluster_background: Some("#0a0a0a".to_string()),
            cluster_border: Some("#0b0b0b".to_string()),
            note_background: Some("#0c0c0c".to_string()),
            note_border: Some("#0d0d0d".to_string()),
            note_text: Some("#0e0e0e".to_string()),
            actor_background: Some("#0f0f0f".to_string()),
            actor_border: Some("#101010".to_string()),
            actor_text: Some("#111111".to_string()),
            activation_background: Some("#121212".to_string()),
            activation_border: Some("#131313".to_string()),
            error: Some("#141414".to_string()),
            warning: Some("#151515".to_string()),
            success: Some("#161616".to_string()),
        }
    }

    fn compiled_sentinel_config() -> Value {
        HostThemeProfile::builder()
            .roles(sentinel_roles())
            .build()
            .compile()
            .site_config
            .as_value()
            .clone()
    }

    #[test]
    fn dark_editor_profile_compiles_common_theme_variables() {
        let compiled = HostThemeProfile::editor_dark().compile();
        let cfg = compiled.site_config.as_value();
        let vars = cfg["themeVariables"].as_object().unwrap();

        assert_eq!(cfg["theme"], "base");
        assert_eq!(cfg["darkMode"], true);
        assert_eq!(vars["background"], "#0f172a");
        assert_eq!(vars["mainBkg"], "#111827");
        assert_eq!(vars["nodeTextColor"], "#e5e7eb");
        assert_eq!(vars["lineColor"], "#94a3b8");
        assert_eq!(vars["noteBkgColor"], "#422006");
        assert_eq!(vars["actorBkg"], "#1f2937");
        assert_eq!(
            vars["xyChart"]["plotColorPalette"],
            "#60a5fa,#34d399,#f59e0b,#c084fc,#22d3ee,#fb7185,#facc15,#a3e635"
        );
        assert_eq!(vars["pie1"], "#60a5fa");
        assert_eq!(vars["git0"], "#60a5fa");
        assert_eq!(vars["gitBranchLabel0"], "#ffffff");
    }

    #[test]
    fn host_theme_roles_compile_to_theme_variable_sentinels() {
        let cfg = compiled_sentinel_config();
        let vars = cfg["themeVariables"].as_object().unwrap();

        assert_eq!(cfg["theme"], "base");
        assert_eq!(vars["background"], "#010101");
        assert_eq!(vars["primaryColor"], "#020202");
        assert_eq!(vars["mainBkg"], "#020202");
        assert_eq!(vars["secondaryColor"], "#030303");
        assert_eq!(vars["tertiaryColor"], "#040404");
        assert_eq!(vars["primaryTextColor"], "#050505");
        assert_eq!(vars["nodeTextColor"], "#050505");
        assert_eq!(vars["textColor"], "#050505");
        assert_eq!(vars["titleColor"], "#050505");
        assert_eq!(vars["secondaryTextColor"], "#060606");
        assert_eq!(vars["tertiaryTextColor"], "#060606");
        assert_eq!(vars["primaryBorderColor"], "#070707");
        assert_eq!(vars["nodeBorder"], "#070707");
        assert_eq!(vars["lineColor"], "#080808");
        assert_eq!(vars["arrowheadColor"], "#080808");
        assert_eq!(vars["edgeLabelBackground"], "#090909");
        assert_eq!(vars["clusterBkg"], "#0a0a0a");
        assert_eq!(vars["clusterBorder"], "#0b0b0b");
        assert_eq!(vars["noteBkgColor"], "#0c0c0c");
        assert_eq!(vars["noteBorderColor"], "#0d0d0d");
        assert_eq!(vars["noteTextColor"], "#0e0e0e");
        assert_eq!(vars["actorBkg"], "#0f0f0f");
        assert_eq!(vars["actorBorder"], "#101010");
        assert_eq!(vars["actorTextColor"], "#111111");
        assert_eq!(vars["activationBkgColor"], "#121212");
        assert_eq!(vars["activationBorderColor"], "#131313");
        assert_eq!(vars["critBkgColor"], "#141414");
        assert_eq!(vars["vertLineColor"], "#151515");
        assert_eq!(vars["doneTaskBkgColor"], "#161616");

        assert_eq!(vars["relationLabelBackground"], "#090909");
        assert_eq!(vars["requirementEdgeLabelBackground"], "#090909");
        assert_eq!(vars["archGroupBorderColor"], "#0b0b0b");
        assert_eq!(vars["emSwimlaneBackgroundOdd"], "#0a0a0a");
        assert_eq!(vars["emSwimlaneBackgroundStroke"], "#0b0b0b");
        assert_eq!(vars["treeView"]["labelColor"], "#050505");
        assert_eq!(vars["treeView"]["lineColor"], "#080808");
    }

    #[test]
    fn host_theme_roles_compile_to_diagram_config_sentinels() {
        let cfg = compiled_sentinel_config();

        assert_eq!(cfg["packet"]["startByteColor"], "#080808");
        assert_eq!(cfg["packet"]["endByteColor"], "#070707");
        assert_eq!(cfg["packet"]["labelColor"], "#050505");
        assert_eq!(cfg["packet"]["titleColor"], "#050505");
        assert_eq!(cfg["packet"]["blockStrokeColor"], "#070707");
        assert_eq!(cfg["packet"]["blockFillColor"], "#020202");

        assert_eq!(cfg["treemap"]["titleColor"], "#050505");
        assert_eq!(cfg["treemap"]["labelColor"], "#050505");
        assert_eq!(cfg["treemap"]["valueColor"], "#060606");
        assert_eq!(cfg["treemap"]["sectionStrokeColor"], "#070707");
        assert_eq!(cfg["treemap"]["sectionFillColor"], "#030303");
        assert_eq!(cfg["treemap"]["leafStrokeColor"], "#070707");
        assert_eq!(cfg["treemap"]["leafFillColor"], "#020202");

        assert_eq!(cfg["radar"]["axisColor"], "#080808");
        assert_eq!(cfg["radar"]["graticuleColor"], "#070707");

        assert_eq!(cfg["c4"]["person_bg_color"], "#020202");
        assert_eq!(cfg["c4"]["person_border_color"], "#070707");
        assert_eq!(cfg["c4"]["external_component_queue_bg_color"], "#020202");
        assert_eq!(
            cfg["c4"]["external_component_queue_border_color"],
            "#070707"
        );
    }

    #[test]
    fn host_theme_role_fallbacks_preserve_context_specific_targets() {
        let cfg = HostThemeProfile::builder()
            .roles(HostThemeRoles {
                canvas: Some("#101010".to_string()),
                surface: Some("#202020".to_string()),
                surface_alt: Some("#303030".to_string()),
                surface_muted: Some("#404040".to_string()),
                ..HostThemeRoles::default()
            })
            .build()
            .compile()
            .site_config
            .as_value()
            .clone();
        let vars = cfg["themeVariables"].as_object().unwrap();

        assert_eq!(vars["edgeLabelBackground"], "#101010");
        assert_eq!(vars["relationLabelBackground"], "#101010");
        assert_eq!(vars["requirementEdgeLabelBackground"], "#101010");
        assert_eq!(vars["commitLabelBackground"], "#202020");

        assert_eq!(vars["clusterBkg"], "#303030");
        assert_eq!(vars["sectionBkgColor"], "#303030");
        assert_eq!(vars["emSwimlaneBackgroundOdd"], "#404040");
    }

    #[test]
    fn common_editor_presets_compile_named_palettes() {
        let cases = [
            (HostThemePreset::EditorLight, "#ffffff", "#2563eb"),
            (HostThemePreset::EditorDark, "#0f172a", "#60a5fa"),
            (HostThemePreset::OneDark, "#282c34", "#61afef"),
            (HostThemePreset::GruvboxDark, "#282828", "#83a598"),
            (HostThemePreset::GruvboxLight, "#fbf1c7", "#458588"),
            (HostThemePreset::AyuDark, "#0b0e14", "#59c2ff"),
            (HostThemePreset::AyuLight, "#fafafa", "#55b4d4"),
        ];

        for (preset, background, first_series_color) in cases {
            let compiled = HostThemeProfile::from_preset(preset).compile();
            let cfg = compiled.site_config.as_value();
            let vars = cfg["themeVariables"].as_object().unwrap();

            assert_eq!(cfg["theme"], "base", "{preset:?}");
            assert_eq!(vars["background"], background, "{preset:?}");
            assert_eq!(vars["pie1"], first_series_color, "{preset:?}");
            assert_eq!(
                compiled.output.preset,
                SvgPipelinePreset::ResvgSafe,
                "{preset:?}"
            );
            assert!(
                !compiled.output.drop_native_duplicate_fallbacks,
                "{preset:?}"
            );
            assert_eq!(
                vars["xyChart"]["accentColor"], first_series_color,
                "{preset:?}"
            );
        }
    }

    #[test]
    fn modern_and_mermaid_presets_compile_explicit_rendering_policies() {
        let modern = HostThemeProfile::merman_modern().compile();

        let cfg = modern.site_config.as_value();
        assert_eq!(cfg["theme"], "redux");
        assert_eq!(cfg["look"], "neo");
        assert_eq!(cfg["flowchart"]["defaultRenderer"], "elk");
        assert_eq!(cfg["flowchart"]["edgeLabelPadding"], 4);
        assert_eq!(cfg["flowchart"]["compactEdgeCorners"], true);
        assert_eq!(cfg["themeVariables"]["mainBkg"], "#F8FAFC");
        assert_eq!(cfg["themeVariables"]["nodeBorder"], "#64748B");
        assert_eq!(cfg["themeVariables"]["lineColor"], "#64748B");
        assert_eq!(cfg["themeVariables"]["edgeLabelBackground"], "#FFFFFF");
        assert_eq!(modern.output.preset, SvgPipelinePreset::Parity);

        let mermaid = HostThemeProfile::mermaid().compile();
        assert_eq!(mermaid.site_config.as_value(), &Value::Object(Map::new()));
        assert_eq!(mermaid.output.preset, SvgPipelinePreset::Parity);
    }

    #[test]
    fn resvg_safe_host_output_can_drop_native_duplicate_fallbacks() {
        let mut output = HostThemeOutput::resvg_safe_editor();
        output.drop_native_duplicate_fallbacks = true;

        let compiled = HostThemeProfile::builder().output(output).build().compile();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
<text class="task">Make tea</text>
<g transform="translate(0,0)">
  <foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>Make tea</p></div></foreignObject>
</g>
<g transform="translate(0,40)">
  <foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>Only fallback</p></div></foreignObject>
</g>
</svg>"##;

        let out = compiled
            .pipeline()
            .process_to_string(svg, &session)
            .unwrap();

        assert_eq!(
            out.matches(r#"data-merman-foreignobject="fallback""#)
                .count(),
            1,
            "{out}"
        );
        assert!(out.contains("Only fallback"));
        assert!(out.contains(r#"<text class="task">Make tea</text>"#));
        assert!(!out.contains("<foreignObject"));
    }

    #[test]
    fn host_theme_preset_names_are_binding_stable() {
        let names = HostThemePreset::ALL.map(HostThemePreset::as_str);

        assert_eq!(
            names,
            [
                "editor-light",
                "editor-dark",
                "one-dark",
                "gruvbox-light",
                "gruvbox-dark",
                "ayu-light",
                "ayu-dark",
                "merman-modern",
                "mermaid"
            ]
        );
    }

    #[test]
    fn explicit_profile_theme_variables_override_derived_roles() {
        let profile = HostThemeProfile::builder()
            .roles(HostThemeRoles {
                border: Some("#111111".to_string()),
                ..HostThemeRoles::default()
            })
            .theme_variable("nodeBorder", "#abcdef")
            .build();

        let compiled = profile.compile();
        let vars = compiled.site_config.as_value()["themeVariables"]
            .as_object()
            .unwrap();

        assert_eq!(vars["nodeBorder"], "#abcdef");
        assert_eq!(vars["primaryBorderColor"], "#111111");
    }

    #[test]
    fn nested_theme_overrides_preserve_unrelated_derived_fields() {
        let profile = HostThemeProfile::builder()
            .roles(HostThemeRoles {
                text: Some("#111111".to_string()),
                line: Some("#222222".to_string()),
                ..HostThemeRoles::default()
            })
            .theme_variable("treeView", serde_json::json!({ "labelColor": "#abcdef" }))
            .site_config("packet", serde_json::json!({ "rowHeight": 42 }))
            .build();

        let compiled = profile.compile();
        let config = compiled.site_config.as_value();

        assert_eq!(
            config["themeVariables"]["treeView"]["labelColor"],
            "#abcdef"
        );
        assert_eq!(config["themeVariables"]["treeView"]["lineColor"], "#222222");
        assert_eq!(config["packet"]["rowHeight"], 42);
        assert_eq!(config["packet"]["labelColor"], "#111111");
    }

    #[test]
    fn canvas_output_uses_the_final_effective_theme_variable() {
        let profile = HostThemeProfile::builder()
            .roles(HostThemeRoles {
                canvas: Some("#010101".to_string()),
                ..HostThemeRoles::default()
            })
            .output(HostThemeOutput::resvg_safe_editor())
            .site_config(
                "themeVariables",
                serde_json::json!({ "background": "#fefefe" }),
            )
            .build();

        let compiled = profile.compile();

        assert_eq!(
            compiled.site_config.as_value()["themeVariables"]["background"],
            "#fefefe"
        );
        assert_eq!(
            compiled.output.root_background_color.as_deref(),
            Some("#fefefe")
        );
    }

    #[test]
    fn empty_profile_compiles_to_empty_site_config() {
        let compiled = HostThemeProfile::default().compile();

        assert_eq!(compiled.site_config.as_value(), &Value::Object(Map::new()));
        assert_eq!(compiled.output.preset, SvgPipelinePreset::Parity);
        assert!(compiled.output.root_background_color.is_none());
    }

    #[test]
    fn compiled_output_builds_host_pipeline() {
        let compiled = HostThemeProfile::editor_dark().compile();
        let pipeline = compiled.pipeline();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let out = pipeline
            .process_to_string(
                r#"<svg id="host" style="background-color: white;"><style>.node{fill:red !important;}</style><text>A</text></svg>"#,
                &session,
            )
            .unwrap();

        assert!(!out.contains("!important"));
        let document = roxmltree::Document::parse(&out).unwrap();
        let style = document
            .root_element()
            .attribute("style")
            .unwrap_or_default();
        assert!(style.contains("background-color:#0f172a"), "{style}");
    }
}
