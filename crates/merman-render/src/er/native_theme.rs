use merman_model::presentation::PresentationConfig;
use merman_theme_contract::v2::{EdgeLabelBackground, EdgeStyle};

use crate::theme::{Color, FamilyTheme, FamilyThemeAdapter, ThemeCapabilityFlags, font_family_css};

pub(crate) struct ErThemeAdapter;

impl FamilyThemeAdapter for ErThemeAdapter {
    const FAMILY: crate::DiagramFamilyId = crate::DiagramFamilyId::ER;
    const CAPS: ThemeCapabilityFlags = ThemeCapabilityFlags::CANVAS
        .union(ThemeCapabilityFlags::NODE_SURFACE)
        .union(ThemeCapabilityFlags::CONTAINER)
        .union(ThemeCapabilityFlags::EMPHASIS)
        .union(ThemeCapabilityFlags::CONNECTOR)
        .union(ThemeCapabilityFlags::RADIUS)
        .union(ThemeCapabilityFlags::EDGE_STYLE)
        .union(ThemeCapabilityFlags::TYPOGRAPHY);
}

#[derive(Debug)]
pub(crate) struct ErNativeStyle {
    pub(crate) body_family: Option<String>,
    pub(crate) code_family: String,
    pub(crate) title_family: Option<String>,
    pub(crate) font_size: Option<f64>,
    pub(crate) title_weight: String,
    pub(crate) heading_ink: String,
    pub(crate) guide: String,
    pub(crate) radius: f64,
    pub(crate) border_width: f64,
    pub(crate) connector_width: f64,
    pub(crate) edge_style: EdgeStyle,
    pub(crate) label_background: String,
}

impl ErNativeStyle {
    pub(crate) fn resolve(
        theme: &FamilyTheme,
        config: &PresentationConfig,
        canvas: Option<Color>,
    ) -> Self {
        let typography = &theme.typography;
        let roles = &theme.roles;
        let owns = |paths: &[&str]| {
            paths
                .iter()
                .any(|path| config.explicit_config_owns_path(path))
        };
        let family = |role| font_family_css(typography.stack(role));
        let theme_family_active = !owns(&["fontFamily", "themeVariables.fontFamily"]);
        let label_background = match theme.form.edge_label_bg {
            EdgeLabelBackground::Transparent => Color::TRANSPARENT,
            EdgeLabelBackground::Pill => roles.surface_node.first_color(),
            EdgeLabelBackground::Auto | EdgeLabelBackground::Canvas => {
                canvas.unwrap_or_else(|| roles.canvas.first_color())
            }
        }
        .to_string();
        Self {
            body_family: theme_family_active.then(|| family(typography.body_family)),
            code_family: family(typography.code_family),
            title_family: theme_family_active.then(|| family(typography.title_family)),
            font_size: (!owns(&["fontSize", "themeVariables.fontSize", "er.fontSize"]))
                .then_some(typography.base_size_px),
            title_weight: typography.title_weight.to_string(),
            heading_ink: roles.text_on_emphasis.to_string(),
            guide: roles.stroke_guide.first_color().to_string(),
            radius: theme.form.radius,
            border_width: theme.form.stroke_width,
            connector_width: theme.form.connector_width,
            edge_style: theme.form.edge_style,
            label_background,
        }
    }

    pub(super) fn apply_layout(&self, settings: &mut super::ErLayoutSettings) {
        settings.code_family = Some(self.code_family.clone());
        settings.calculation_style = settings.label_style.clone();
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
    fn native_theme_v2_er_uses_table_roles_and_relationship_background() {
        let theme = ThemeCompiler::new().compile_json(br##"{
            "schema_version":2,"extends":"paper",
            "roles":{"surface.node":"#EEEEEE","surface.emphasis":"#345678","surface.container":"#DFEBFC","stroke.connector":"#123456"},
            "typography":{"mono":["Attribute Mono","monospace"]},
            "form":{"radius":6,"edge_label_bg":"canvas","edge_style":"straight"}
        }"##, None).unwrap();
        let environment = RenderEnvironment::deterministic();
        for html in [false, true] {
            let source = format!(
                "---\nconfig:\n  htmlLabels: {html}\n---\nerDiagram\nORDER ||--o{{ LINE : contains\nLINE {{\n  string sku\n  int quantity\n}}\n"
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
                    .unwrap()
                    .with_canvas_background(crate::theme::Color::rgb(250, 251, 252)),
            )
            .unwrap();
            let render = prepared
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let dom = roxmltree::Document::parse(render.svg()).unwrap();
            for (class, fill) in [
                ("er-native-header", "#345678"),
                ("row-rect-odd", "#EEEEEE"),
                ("row-rect-even", "#DFEBFC"),
            ] {
                assert!(
                    dom.descendants()
                        .any(|node| node.attribute("class") == Some(class)
                            && node
                                .attribute("style")
                                .is_some_and(|style| style.contains(fill))),
                    "missing {class} fill"
                );
            }
            let css = dom
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .unwrap()
                .text()
                .unwrap();
            assert!(
                css.contains(".label.attribute-type{font-family:\"Attribute Mono\", monospace;")
            );
            assert!(css.contains(".labelBkg{background-color:#FAFBFC;}"));
            assert!(css.contains(".edgeLabel .label rect{fill:#FAFBFC;}"));
            assert!(dom.descendants().any(|node| {
                node.attribute("marker-end")
                    .is_some_and(|marker| marker.contains("zeroOrMore"))
            }));
        }
    }
}
