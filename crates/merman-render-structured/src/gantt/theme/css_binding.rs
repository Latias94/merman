use merman_model::presentation::PresentationConfig;
use merman_render_core::config::{ConfigLookup, config_f64, config_string};
use merman_render_core::mermaid_style::font_family_css;
use merman_render_core::svg_css::PreparedCommonCss;
use merman_theme_contract::v2::FamilyTheme;

use super::super::{GanttNativeStyle, gantt_section_or_root};
use super::font_size::GanttFontSize;

#[derive(Debug, Clone)]
pub(crate) struct GanttCssBinding {
    pub(crate) common: PreparedCommonCss,
    pub(crate) task_font_size: GanttFontSize,
    pub(crate) section_font_size: GanttFontSize,
    pub(crate) text_color: String,
    pub(crate) exclude_bkg_color: String,
    pub(crate) section_bkg_color: String,
    pub(crate) section_bkg_color2: String,
    pub(crate) alt_section_bkg_color: String,
    pub(crate) title_color: String,
    pub(crate) title_text_color: String,
    pub(crate) grid_color: String,
    pub(crate) axis_color: Option<String>,
    pub(crate) today_line_color: String,
    pub(crate) task_text_dark_color: String,
    pub(crate) task_text_clickable_color: String,
    pub(crate) task_text_color: String,
    pub(crate) task_bkg_color: String,
    pub(crate) task_border_color: String,
    pub(crate) task_text_outside_color: String,
    pub(crate) active_task_bkg_color: String,
    pub(crate) active_task_border_color: String,
    pub(crate) done_task_border_color: String,
    pub(crate) done_task_bkg_color: String,
    pub(crate) crit_border_color: String,
    pub(crate) crit_bkg_color: String,
    pub(crate) vert_line_color: String,
}

impl GanttCssBinding {
    pub(super) fn for_native(
        theme: &FamilyTheme,
        config: &PresentationConfig,
    ) -> (GanttNativeStyle, Self) {
        let typography = &theme.typography;
        let font = ["themeVariables.fontFamily", "fontFamily"]
            .into_iter()
            .filter(|path| config.explicit_config_owns_path(path))
            .find_map(|path| {
                config
                    .get_str(path)
                    .and_then(merman_render_core::config::normalized_config_font_family)
            })
            .unwrap_or_else(|| font_family_css(typography.stack(typography.body_family)));
        let base_size = [
            (
                "themeVariables.fontSize",
                &["themeVariables", "fontSize"][..],
            ),
            ("fontSize", &["fontSize"][..]),
        ]
        .into_iter()
        .filter(|(path, _)| config.explicit_config_owns_path(path))
        .find_map(|(_, path)| merman_render_core::config::config_f64_css_absolute_px(config, path))
        .filter(|size| *size >= 0.0)
        .unwrap_or(typography.base_size_px);
        let native = GanttNativeStyle::new(theme, config, &font, base_size);
        let mut css = Self::new(config, &font);
        css.common =
            PreparedCommonCss::with_resolved_typography(config, font, format!("{base_size}px"));
        let gantt_config = gantt_section_or_root(config);
        let task_size = serde_json::json!(
            native
                .task_font_size
                .unwrap_or_else(|| config_f64(gantt_config, &["fontSize"]).unwrap_or(11.0))
        );
        let section_size = serde_json::json!(base_size);
        let section_size = if config.explicit_config_owns_path("gantt.sectionFontSize") {
            gantt_config.get("sectionFontSize")
        } else {
            Some(&section_size)
        };
        css.task_font_size = GanttFontSize::resolve(Some(&task_size), base_size);
        css.section_font_size = GanttFontSize::resolve(section_size, base_size);
        (native, css)
    }

    pub(super) fn new(config: &dyn ConfigLookup, font_family: &str) -> Self {
        let option = |key: &str, fallback: &str| {
            config_string(config, &["themeVariables", key]).unwrap_or_else(|| fallback.to_owned())
        };
        let common = PreparedCommonCss::new(config, Some(font_family));
        let inherited_font_size = merman_render_core::mermaid_style::resolve_mermaid_font_size_px(
            common.font_size_css(),
            merman_render_core::mermaid_style::CssFontSizeContext::uniform(16.0),
        )
        .expect("prepared common CSS has an absolute root font size");
        let text_color = common.text_color().to_owned();
        let gantt_config = gantt_section_or_root(config);
        let task_size = serde_json::json!(config_f64(gantt_config, &["fontSize"]).unwrap_or(11.0));
        let task_font_size = GanttFontSize::resolve(Some(&task_size), inherited_font_size);
        let section_font_size =
            GanttFontSize::resolve(gantt_config.get("sectionFontSize"), inherited_font_size);
        let title_color = option("titleColor", "#333");
        let title_text_color = if title_color.trim().is_empty() {
            text_color.clone()
        } else {
            title_color.clone()
        };
        Self {
            common,
            task_font_size,
            section_font_size,
            text_color,
            title_color,
            title_text_color,
            exclude_bkg_color: option("excludeBkgColor", "#eeeeee"),
            section_bkg_color: option("sectionBkgColor", "rgba(102, 102, 255, 0.49)"),
            section_bkg_color2: option("sectionBkgColor2", "#fff400"),
            alt_section_bkg_color: option("altSectionBkgColor", "white"),
            grid_color: option("gridColor", "lightgrey"),
            axis_color: config_string(config, &["themeVariables", "gantt", "axisColor"]),
            today_line_color: option("todayLineColor", "red"),
            task_text_dark_color: option("taskTextDarkColor", "black"),
            task_text_clickable_color: option("taskTextClickableColor", "#003163"),
            task_text_color: option("taskTextColor", "white"),
            task_bkg_color: option("taskBkgColor", "#8a90dd"),
            task_border_color: option("taskBorderColor", "#534fbc"),
            task_text_outside_color: option("taskTextOutsideColor", "black"),
            active_task_bkg_color: option("activeTaskBkgColor", "#bfc7ff"),
            active_task_border_color: option("activeTaskBorderColor", "#534fbc"),
            done_task_border_color: option("doneTaskBorderColor", "grey"),
            done_task_bkg_color: option("doneTaskBkgColor", "lightgrey"),
            crit_border_color: option("critBorderColor", "#ff8888"),
            crit_bkg_color: option("critBkgColor", "red"),
            vert_line_color: option("vertLineColor", "navy"),
        }
    }
}
