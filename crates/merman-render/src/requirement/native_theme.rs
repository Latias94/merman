use merman_model::presentation::PresentationConfig;
use merman_theme_contract::v2::{EdgeLabelBackground, EdgeStyle};

use crate::theme::{Color, FamilyTheme, FamilyThemeAdapter, ThemeCapabilityFlags, font_family_css};

pub(crate) struct RequirementThemeAdapter;

impl FamilyThemeAdapter for RequirementThemeAdapter {
    const FAMILY: crate::DiagramFamilyId = crate::DiagramFamilyId::REQUIREMENT;
    const CAPS: ThemeCapabilityFlags = ThemeCapabilityFlags::CANVAS
        .union(ThemeCapabilityFlags::NODE_SURFACE)
        .union(ThemeCapabilityFlags::EMPHASIS)
        .union(ThemeCapabilityFlags::CONNECTOR)
        .union(ThemeCapabilityFlags::STATUS_COLORS)
        .union(ThemeCapabilityFlags::RADIUS)
        .union(ThemeCapabilityFlags::EDGE_STYLE)
        .union(ThemeCapabilityFlags::TYPOGRAPHY);
}

#[derive(Debug)]
pub(crate) struct RequirementNativeStyle {
    pub(crate) emphasis: Option<String>,
    pub(crate) emphasis_ink: Option<String>,
    pub(crate) title_family: String,
    pub(crate) title_weight: String,
    pub(crate) title_ink: String,
    pub(crate) guide: String,
    pub(crate) radius: f64,
    pub(crate) border_width: f64,
    #[cfg(feature = "layout-shoal")]
    pub(crate) connector_width: Option<f64>,
    pub(crate) edge_style: EdgeStyle,
    risk_ink: Option<[String; 4]>,
}

impl RequirementNativeStyle {
    pub(crate) fn risk_ink(&self, risk: &str) -> Option<&str> {
        let risk = risk.trim();
        let [low, medium, high, other] = self.risk_ink.as_ref()?;
        Some(if risk.eq_ignore_ascii_case("low") {
            low
        } else if risk.eq_ignore_ascii_case("medium") {
            medium
        } else if risk.eq_ignore_ascii_case("high") {
            high
        } else {
            other
        })
    }
}

impl super::RequirementCssBinding {
    pub(super) fn resolve_native(
        theme: &FamilyTheme,
        config: &PresentationConfig,
        canvas: Option<Color>,
    ) -> Self {
        let roles = &theme.roles;
        let typography = &theme.typography;
        let owns = |key: &str| config.explicit_config_owns_path(key);
        let native_font = (!owns("fontFamily") && !owns("themeVariables.fontFamily"))
            .then(|| font_family_css(typography.stack(typography.body_family)));
        let native_size = (!owns("fontSize") && !owns("themeVariables.fontSize"))
            .then_some(typography.base_size_px);
        let size_css = native_size.map(|size| format!("{size}px"));
        let mut binding = Self::resolve(
            config.as_value(),
            native_font.as_deref(),
            size_css.as_deref(),
        );
        if let Some(size) = native_size {
            binding.layout.calculation_font_size = size;
        }
        let authored_fill =
            owns("themeVariables.mainBkg") || owns("themeVariables.requirementBackground");
        if owns("themeVariables.requirementBackground") && !owns("themeVariables.mainBkg") {
            binding
                .default_fill
                .clone_from(&binding.requirement_background);
        } else if !authored_fill {
            binding.default_fill = roles.surface_node.first_color().to_string();
            binding
                .requirement_background
                .clone_from(&binding.default_fill);
        }
        if !owns("themeVariables.nodeBorder") && !owns("themeVariables.requirementBorderColor") {
            binding.default_stroke = roles.stroke_border.first_color().to_string();
            binding
                .requirement_border_color
                .clone_from(&binding.default_stroke);
            binding.node_border.clone_from(&binding.default_stroke);
        }
        let authored_ink = owns("themeVariables.nodeTextColor")
            || owns("themeVariables.textColor")
            || owns("themeVariables.requirementTextColor");
        if !authored_ink {
            binding.node_text_color = roles.text_primary.to_string();
            binding
                .requirement_text_color
                .clone_from(&binding.node_text_color);
        }
        if !owns("themeVariables.relationLabelColor") {
            binding.relation_label_color = roles.text_primary.to_string();
        }
        if !owns("themeVariables.lineColor") {
            binding.line_color = roles.stroke_connector.first_color().to_string();
        }
        if !owns("themeVariables.relationColor") {
            binding.relation_color.clone_from(&binding.line_color);
        }
        let connector_width =
            (!owns("themeVariables.strokeWidth")).then_some(theme.form.connector_width);
        if let Some(width) = connector_width {
            binding.relationship_line_stroke_width = format!("{width}px");
        } else if let Some(width) = crate::config::config_css_number_or_string(
            config.as_value(),
            &["themeVariables", "strokeWidth"],
        ) {
            binding.relationship_line_stroke_width = width;
        }
        if !owns("themeVariables.edgeLabelBackground")
            && !owns("themeVariables.requirementEdgeLabelBackground")
        {
            let background = match theme.form.edge_label_bg {
                EdgeLabelBackground::Auto | EdgeLabelBackground::Canvas => {
                    canvas.unwrap_or_else(|| roles.canvas.first_color())
                }
                EdgeLabelBackground::Pill => roles.surface_node.first_color(),
                EdgeLabelBackground::Transparent => Color::TRANSPARENT,
            };
            binding.edge_label_background = background.to_string();
            binding
                .requirement_edge_label_background
                .clone_from(&binding.edge_label_background);
        }
        let title_family = if native_font.is_some() {
            font_family_css(typography.stack(typography.title_family))
        } else {
            binding.layout.font_family.clone()
        };
        let title_ink = if owns("themeVariables.textColor") {
            binding.common.text_color().to_owned()
        } else {
            roles.text_title.to_string()
        };
        let border_width = if owns("themeVariables.requirementBorderSize") {
            crate::config::config_f64_css_px(
                config.as_value(),
                &["themeVariables", "requirementBorderSize"],
            )
            .unwrap_or(theme.form.stroke_width)
        } else {
            theme.form.stroke_width
        };
        binding.native = Some(RequirementNativeStyle {
            emphasis: (!authored_fill).then(|| roles.surface_emphasis.first_color().to_string()),
            emphasis_ink: (!authored_ink).then(|| roles.text_on_emphasis.to_string()),
            title_family,
            title_weight: typography.title_weight.to_string(),
            title_ink,
            guide: roles.stroke_guide.first_color().to_string(),
            radius: theme.form.radius,
            border_width,
            #[cfg(feature = "layout-shoal")]
            connector_width,
            edge_style: theme.form.edge_style,
            risk_ink: (!authored_ink).then(|| {
                [
                    roles.status_success,
                    roles.status_warn,
                    roles.status_danger,
                    roles.status_info,
                ]
                .map(|color| color.to_string())
            }),
        });
        binding
    }
}

#[cfg(all(test, feature = "layout-dagre"))]
mod tests {
    use merman_core::{MermaidEngine, ParseOptions};

    use crate::environment::RenderEnvironment;
    use crate::svg::{SvgDebugOptions, SvgRenderOptions};
    use crate::theme::{NativeTheme, ThemeCompiler};
    use crate::{LayoutOptions, family};

    #[test]
    fn native_theme_v2_requirement_uses_header_and_risk_roles() {
        let theme = ThemeCompiler::new().compile_json(br##"{
            "schema_version":2,"extends":"paper",
            "roles":{"surface.node":"#EEEEEE","surface.emphasis":"#345678","status.danger":"#7A1616","stroke.connector":"#123456"},
            "typography":{"sans":["Requirement Body","sans-serif"],"base_size_px":18},
            "form":{"radius":7,"edge_style":"straight"}
        }"##, None).unwrap();
        let environment = RenderEnvironment::deterministic();
        for html in [false, true] {
            let source = format!(
                "---\nconfig:\n  htmlLabels: {html}\n---\nrequirementDiagram\nrequirement secured {{\n  id: 1\n  text: A requirement\n  risk: high\n  verifymethod: analysis\n}}\n"
            );
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let prepared = family::prepare(
                parsed.presentation(),
                &LayoutOptions::default(),
                environment
                    .begin_session_with_native_theme(NativeTheme::new(theme.clone()))
                    .unwrap(),
            )
            .unwrap();
            let render = prepared
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let dom = roxmltree::Document::parse(render.svg()).unwrap();
            assert!(dom.descendants().any(|node| {
                node.attribute("class") == Some("requirement-native-box")
                    && node.attribute("rx") == Some("7")
                    && node
                        .attribute("style")
                        .is_some_and(|style| style.contains("fill:#EEEEEE"))
            }));
            assert!(dom.descendants().any(|node| node.attribute("class")
                == Some("requirement-native-header")
                && node.attribute("style") == Some("fill:#345678;stroke:none")));
            let risk = dom
                .descendants()
                .find(|node| {
                    node.is_text() && node.text().is_some_and(|text| text.contains("Risk:"))
                })
                .unwrap();
            assert!(risk.ancestors().any(|node| {
                node.attribute("style")
                    .is_some_and(|style| style.contains("#7A1616"))
            }));
        }
    }
}
