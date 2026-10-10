use crate::config::{config_f64, config_string};
use crate::svg::PreparedCommonCss;

#[derive(Debug)]
pub(crate) struct GanttCssBinding {
    pub(crate) common: PreparedCommonCss,
    pub(crate) task_font_size: f64,
    pub(crate) section_font_size: f64,
    pub(crate) text_color: String,
    pub(crate) exclude_bkg_color: String,
    pub(crate) section_bkg_color: String,
    pub(crate) section_bkg_color2: String,
    pub(crate) alt_section_bkg_color: String,
    pub(crate) title_color: String,
    pub(crate) title_text_color: String,
    pub(crate) grid_color: String,
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
    pub(super) fn new(config: &serde_json::Value, font_family: &str) -> Self {
        let option = |key: &str, fallback: &str| {
            config_string(config, &["themeVariables", key]).unwrap_or_else(|| fallback.to_owned())
        };
        let common = PreparedCommonCss::new(config, Some(font_family));
        let text_color = common.text_color().to_owned();
        let gantt_config = config.get("gantt").unwrap_or(config);
        let title_color = option("titleColor", "#333");
        let title_text_color = if title_color.trim().is_empty() {
            text_color.clone()
        } else {
            title_color.clone()
        };
        Self {
            common,
            task_font_size: config_f64(gantt_config, &["fontSize"]).unwrap_or(11.0),
            section_font_size: config_f64(gantt_config, &["sectionFontSize"]).unwrap_or(11.0),
            text_color,
            title_color,
            title_text_color,
            exclude_bkg_color: option("excludeBkgColor", "#eeeeee"),
            section_bkg_color: option("sectionBkgColor", "rgba(102, 102, 255, 0.49)"),
            section_bkg_color2: option("sectionBkgColor2", "#fff400"),
            alt_section_bkg_color: option("altSectionBkgColor", "white"),
            grid_color: option("gridColor", "lightgrey"),
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

    pub(super) fn bind_winners(
        &mut self,
        title: Option<&super::GanttGlobalFillExpectation>,
        text: Option<&super::GanttTextFillExpectation>,
        warning: Option<&super::GanttWarningStrokeExpectation>,
    ) {
        if let Some(title) = title {
            self.title_color = title.css.to_string();
        }
        if let Some(text) = text {
            self.text_color = text.grid_css.to_string();
            self.task_text_color = text.task_css.to_string();
        }
        if let Some(warning) = warning {
            self.today_line_color = warning.today_css.to_string();
            self.vert_line_color = warning.vert_css.to_string();
        }
        self.title_text_color = if self.title_color.trim().is_empty() {
            self.text_color.clone()
        } else {
            self.title_color.clone()
        };
    }
}
