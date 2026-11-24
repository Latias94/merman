use merman_model::presentation::PresentationConfig;

use crate::theme::{Color, FamilyTheme, font_family_css};

const CIRCLE_OPACITY: f64 = 0.45;

#[derive(Clone, Copy)]
pub(super) struct VennFontSizes {
    pub(super) title: f64,
    pub(super) area: f64,
    pub(super) text_node: f64,
}

impl VennFontSizes {
    pub(super) fn new(native_base: Option<f64>, scale: f64) -> Self {
        native_base.map_or(
            Self {
                title: 32.0 * scale,
                area: 48.0 * scale,
                text_node: 40.0 * scale,
            },
            |base| {
                let body = 2.0 * scale * base;
                Self {
                    title: body * 1.25,
                    area: body,
                    text_node: body,
                }
            },
        )
    }
}

pub(crate) fn native_font_size(
    theme: Option<&FamilyTheme>,
    config: &PresentationConfig,
) -> Option<f64> {
    let theme = theme?;
    if config.explicit_config_owns_path("themeVariables.fontSize") {
        return crate::config::config_f64_css_px(
            config.as_value(),
            &["themeVariables", "fontSize"],
        );
    }
    if config.explicit_config_owns_path("fontSize") {
        return crate::config::config_f64_css_px(config.as_value(), &["fontSize"]);
    }
    Some(theme.typography.base_size_px)
}

#[derive(Debug)]
pub(super) struct VennNativeTypography {
    pub(super) body_family: Option<String>,
    pub(super) title_family: Option<String>,
    pub(super) title_weight: u16,
    pub(super) label_weight: u16,
    pub(super) base_size: Option<f64>,
}

impl VennNativeTypography {
    pub(super) fn resolve(theme: &FamilyTheme, config: &PresentationConfig) -> Self {
        let typography = &theme.typography;
        let native_family = !config.explicit_config_owns_path("fontFamily")
            && !config.explicit_config_owns_path("themeVariables.fontFamily");
        Self {
            body_family: native_family
                .then(|| font_family_css(typography.stack(typography.body_family))),
            title_family: native_family
                .then(|| font_family_css(typography.stack(typography.title_family))),
            title_weight: typography.title_weight,
            label_weight: typography.label_weight,
            base_size: native_font_size(Some(theme), config),
        }
    }
}

fn css_color(css: &str) -> Option<Color> {
    let rgba = css.parse::<svgtypes::Color>().ok()?;
    Color::parse(&format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        rgba.red, rgba.green, rgba.blue, rgba.alpha
    ))
    .ok()
}

fn contrast_ink(ink: Color, background: Color) -> String {
    if ink.contrast(background) >= 4.5 {
        return ink.to_string();
    }
    let black = Color::rgb(0, 0, 0);
    let white = Color::rgb(255, 255, 255);
    if white.contrast(background) > black.contrast(background) {
        white
    } else {
        black
    }
    .to_string()
}

pub(super) fn apply_paints(
    binding: &mut super::theme::VennCssBinding,
    theme: &FamilyTheme,
    config: &PresentationConfig,
    layout: &crate::model::VennDiagramLayout,
    canvas: Option<Color>,
    work: &crate::resources::OperationWorkMeter,
) -> crate::Result<()> {
    use super::VennAreaPaintBinding::{Circle, Intersection};

    let roles = &theme.roles;
    let base = canvas.unwrap_or_else(|| roles.canvas.first_color());
    let owns = |path: &str| config.explicit_config_owns_path(path);
    let owns_text = owns("themeVariables.textColor");
    if !owns("themeVariables.vennTitleTextColor") && !owns("themeVariables.titleColor") {
        binding.title_color = roles.text_title.to_string();
    }
    if !owns_text {
        binding.inherited_text_fill = Some(roles.text_primary.to_string());
    }
    if !owns("themeVariables.vennSetTextColor") && !owns_text {
        binding.text_node_color = Some(roles.text_primary.to_string());
    }

    let mut visible_circles = Vec::new();
    for (area, paint) in layout.areas.iter().zip(binding.areas.iter_mut()) {
        let Circle(paint) = paint else { continue };
        work.charge(1)?;
        let slot = visible_circles.len();
        let authored_fill = paint.source_fill_owned
            || paint.fill_config_role.map_or_else(
                || owns("themeVariables.primaryColor"),
                |index| owns(&format!("themeVariables.venn{index}")),
            );
        if !authored_fill {
            paint.fill = theme.data.fill(slot).to_string();
        }
        if !paint.source_stroke_owned && !authored_fill {
            paint.stroke = theme.data.ink(slot).to_string();
        }
        if !paint.source_opacity_owned {
            paint.fill_opacity = CIRCLE_OPACITY.to_string();
        }
        let opacity = paint
            .fill_opacity
            .parse::<f64>()
            .ok()
            .filter(|opacity| opacity.is_finite())
            .unwrap_or(CIRCLE_OPACITY);
        visible_circles.push((area.circles.first(), css_color(&paint.fill), opacity));
    }

    let overlaid = |x: f64, y: f64| {
        visible_circles
            .iter()
            .fold(base, |bg, (circle, fill, opacity)| {
                let Some(circle) = circle else { return bg };
                let dx = x - circle.x;
                let dy = y - circle.y;
                if dx * dx + dy * dy <= circle.radius * circle.radius
                    && let Some(fill) = fill
                {
                    fill.composite(bg, *opacity)
                } else {
                    bg
                }
            })
    };
    for (area, paint) in layout.areas.iter().zip(binding.areas.iter_mut()) {
        if super::rendered_area_label(area).is_empty() {
            continue;
        }
        if match paint {
            Circle(paint) => paint.source_text_owned,
            Intersection {
                source_text_owned,
                config_text_role,
                ..
            } => *source_text_owned || config_text_role.is_some_and(owns),
        } || owns_text
        {
            continue;
        }
        work.charge(visible_circles.len().max(1))?;
        let background = overlaid(area.text_x, area.text_y);
        match paint {
            Circle(paint) => {
                paint.text = contrast_ink(roles.text_primary, background);
            }
            Intersection { fill, text, .. } => {
                let background = fill
                    .as_deref()
                    .and_then(css_color)
                    .map_or(background, |fill| fill.composite(background, 1.0));
                *text = Some(contrast_ink(roles.text_primary, background));
            }
        }
    }
    if !owns_text && !owns("themeVariables.vennSetTextColor") {
        for node in &layout.text_nodes {
            if !binding.text_node_colors.contains_key(&node.id) {
                work.charge(visible_circles.len().max(1))?;
                let background = overlaid(node.x + node.width / 2.0, node.y + node.height / 2.0);
                binding.text_node_colors.insert(
                    node.id.clone(),
                    contrast_ink(roles.text_primary, background),
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use merman_core::{MermaidEngine, ParseOptions};

    use crate::environment::RenderEnvironment;
    use crate::svg::{SvgDebugOptions, SvgRenderOptions};
    use crate::theme::{NativeTheme, ThemeCompiler};
    use crate::{LayoutOptions, family};

    #[test]
    fn native_theme_v2_venn_composites_circle_ink_and_preserves_source_paint() {
        let theme = ThemeCompiler::new()
            .compile_json(
                br##"{
                    "schema_version":2,"extends":"paper",
                    "roles":{"canvas":"#FFFFFF","text.primary":"#FFFFFF","text.title":"#123456"},
                    "typography":{"sans":["Venn Body","sans-serif"],"base_size_px":18},
                    "appearances":{"light":{"data":{"ink":["#15505D","#54216B","#713336","#433511"],"fill":["#CCEEDD","#EEDDEE","#EEDDDD","#EEEEDD"]}}}
                }"##,
                None,
            )
            .unwrap();
        let source = concat!(
            "venn-beta\ntitle Venn Roles\n",
            "set A[\"Alpha\"]:20\nset B[\"Beta\"]:12\nunion A,B[\"Overlap\"]:3\n",
            "style A fill:#aabbcc\n",
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let prepared = family::prepare(
            parsed.presentation(),
            &LayoutOptions::default(),
            RenderEnvironment::deterministic()
                .begin_session_with_native_theme(NativeTheme::new(theme))
                .unwrap(),
        )
        .unwrap();
        let rendered = prepared
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let dom = roxmltree::Document::parse(rendered.svg()).unwrap();
        let circles = dom
            .descendants()
            .filter(|node| {
                node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|class| class == "venn-circle")
                })
            })
            .map(|node| {
                node.children()
                    .find(|node| node.has_tag_name("path"))
                    .unwrap()
                    .attribute("style")
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert!(circles[0].contains("fill: #aabbcc; fill-opacity: 0.45;"));
        assert!(circles[1].contains("fill: #EEDDEE; fill-opacity: 0.45; stroke: #54216B;"));
        let overlap = dom
            .descendants()
            .find(|node| node.has_tag_name("tspan") && node.text() == Some("Overlap"))
            .unwrap();
        assert!(
            overlap
                .parent_element()
                .unwrap()
                .attribute("style")
                .unwrap()
                .contains("fill: #000000;")
        );
        let title = dom
            .descendants()
            .find(|node| node.attribute("class") == Some("venn-title"))
            .unwrap();
        assert_eq!(title.attribute("font-size"), Some("22.5px"));
    }
}
