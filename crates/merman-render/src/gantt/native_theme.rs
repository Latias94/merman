use merman_model::families::gantt::GanttRenderTask;
use merman_model::presentation::PresentationConfig;

use crate::text::TextStyle;
use crate::theme::{Color, FamilyTheme, font_family_css};

use super::task_bar::GanttTaskBarState;

#[derive(Debug)]
struct GanttBarPaint {
    fill: String,
    stroke: String,
    ink: String,
    source_fill: bool,
    source_ink: bool,
}

#[derive(Debug)]
pub(crate) struct GanttNativeStyle {
    pub(crate) text: String,
    pub(crate) label_weight: String,
    pub(crate) title_family: String,
    pub(crate) title_weight: u16,
    pub(crate) title_color: String,
    pub(crate) title_size: f64,
    pub(crate) guide: String,
    pub(crate) show_grid: bool,
    pub(crate) tick_size: f64,
    pub(crate) excluded_fill: String,
    pub(crate) excluded_opacity: f64,
    pub(crate) today: String,
    pub(crate) bar_radius: f64,
    pub(crate) bar_stroke_width: f64,
    pub(crate) bar_wireframe: bool,
    pub(super) task_font_size: Option<f64>,
    outside_text: String,
    outside_done_text: String,
    clickable_text: String,
    vertical: String,
    milestone: String,
    milestone_ink: String,
    default: GanttBarPaint,
    active: GanttBarPaint,
    done: GanttBarPaint,
    critical: GanttBarPaint,
    row_fills: [String; 3],
    container: String,
}

impl GanttNativeStyle {
    pub(super) fn new(
        theme: &FamilyTheme,
        config: &PresentationConfig,
        body_family: &str,
        base_size: f64,
    ) -> Self {
        let roles = &theme.roles;
        let typography = &theme.typography;
        let source = |key| source_color(config, key);
        let paint =
            |key, color: Color| source(key).map_or_else(|| color.to_string(), str::to_owned);
        let container = roles.surface_container.first_color();
        let border = roles.stroke_border.first_color();
        let guide = roles.stroke_guide.first_color();
        let strongest_ink = |fill: Color| {
            let primary = roles.text_primary;
            if primary.contrast(fill) >= 4.5 {
                return primary;
            }
            [
                roles.text_on_emphasis,
                Color::rgb(0, 0, 0),
                Color::rgb(255, 255, 255),
            ]
            .into_iter()
            .max_by(|left, right| left.contrast(fill).total_cmp(&right.contrast(fill)))
            .unwrap_or(primary)
        };
        let bar = |fill_key, stroke_key, ink_key, fill, stroke, ink| GanttBarPaint {
            fill: paint(fill_key, fill),
            stroke: paint(stroke_key, stroke),
            ink: paint(ink_key, ink),
            source_fill: source(fill_key).is_some(),
            source_ink: source(ink_key).is_some(),
        };
        Self {
            text: paint("textColor", roles.text_muted),
            label_weight: typography.label_weight.to_string(),
            title_family: if config.explicit_config_owns_path("fontFamily")
                || config.explicit_config_owns_path("themeVariables.fontFamily")
            {
                body_family.to_owned()
            } else {
                font_family_css(typography.stack(typography.title_family))
            },
            title_weight: typography.title_weight,
            title_color: paint("titleColor", roles.text_title),
            title_size: base_size * 1.25,
            guide: config
                .get_str("themeVariables.gantt.axisColor")
                .filter(|_| config.explicit_config_owns_path("themeVariables.gantt.axisColor"))
                .map_or_else(|| paint("gridColor", guide), str::to_owned),
            show_grid: theme.form.chart.gridlines
                || config.explicit_config_owns_path("themeVariables.gantt.axisColor")
                || source("gridColor").is_some(),
            tick_size: base_size,
            excluded_fill: paint("excludeBkgColor", guide),
            excluded_opacity: if source("excludeBkgColor").is_some() {
                1.0
            } else {
                0.28
            },
            today: paint("todayLineColor", roles.status_warn),
            bar_radius: theme.form.chart.bar_radius,
            bar_stroke_width: theme.form.stroke_width,
            bar_wireframe: theme.form.chart.bar_wireframe,
            task_font_size: (!config.explicit_config_owns_path("gantt.fontSize"))
                .then_some(base_size),
            outside_text: paint("taskTextOutsideColor", roles.text_primary),
            outside_done_text: paint("taskTextOutsideColor", roles.text_muted),
            clickable_text: paint("taskTextClickableColor", roles.text_title),
            vertical: paint("vertLineColor", roles.text_title),
            milestone: roles.text_title.to_string(),
            milestone_ink: paint("taskTextColor", strongest_ink(roles.text_title)),
            default: bar(
                "taskBkgColor",
                "taskBorderColor",
                "taskTextColor",
                theme.data.fill(0),
                border,
                theme.data.fill_ink(0),
            ),
            active: bar(
                "activeTaskBkgColor",
                "activeTaskBorderColor",
                "taskTextDarkColor",
                roles.status_info,
                border,
                strongest_ink(roles.status_info),
            ),
            done: bar(
                "doneTaskBkgColor",
                "doneTaskBorderColor",
                "taskTextDarkColor",
                container,
                border,
                roles.text_muted,
            ),
            critical: bar(
                "critBkgColor",
                "critBorderColor",
                "taskTextColor",
                roles.status_danger,
                roles.status_danger,
                strongest_ink(roles.status_danger),
            ),
            row_fills: [
                paint("sectionBkgColor", container),
                source("altSectionBkgColor").unwrap_or("none").to_owned(),
                paint("sectionBkgColor2", container),
            ],
            container: container.to_string(),
        }
    }

    pub(super) fn title_text_style(&self) -> TextStyle {
        TextStyle {
            font_family: Some(self.title_family.clone()),
            font_size: self.title_size,
            font_weight: Some(self.title_weight.to_string()),
            font_style: None,
        }
    }

    pub(crate) fn row_fill(&self, class: &str) -> &str {
        match class
            .strip_prefix("section section")
            .and_then(|suffix| suffix.parse::<usize>().ok())
        {
            Some(0) => &self.row_fills[0],
            Some(2) => &self.row_fills[2],
            Some(index) if index % 2 == 0 => &self.container,
            _ => &self.row_fills[1],
        }
    }

    pub(crate) fn task_paint(&self, task: &GanttRenderTask) -> (&str, &str) {
        let state = GanttTaskBarState::from_task(task);
        let paint = self.bar(state);
        let fill = if task.milestone && !paint.source_fill {
            &self.milestone
        } else {
            &paint.fill
        };
        let stroke = if task.vert {
            &self.vertical
        } else if state.is_critical() {
            &self.critical.stroke
        } else {
            &paint.stroke
        };
        (fill, stroke)
    }

    pub(crate) fn task_ink(&self, task: &GanttRenderTask, class: &str) -> &str {
        let has = |value| class.split_ascii_whitespace().any(|token| token == value);
        let state = GanttTaskBarState::from_task(task);
        let paint = self.bar(state);
        if has("clickable") {
            &self.clickable_text
        } else if task.vert {
            &self.vertical
        } else if has("taskTextOutsideLeft") || has("taskTextOutsideRight") {
            if matches!(
                state,
                GanttTaskBarState::Active | GanttTaskBarState::ActiveCrit
            ) && paint.source_ink
            {
                &paint.ink
            } else if matches!(state, GanttTaskBarState::Done | GanttTaskBarState::DoneCrit) {
                &self.outside_done_text
            } else {
                &self.outside_text
            }
        } else if task.milestone && !paint.source_fill && !paint.source_ink {
            &self.milestone_ink
        } else {
            &paint.ink
        }
    }

    fn bar(&self, state: GanttTaskBarState) -> &GanttBarPaint {
        match state {
            GanttTaskBarState::Default => &self.default,
            GanttTaskBarState::Active | GanttTaskBarState::ActiveCrit => &self.active,
            GanttTaskBarState::Done | GanttTaskBarState::DoneCrit => &self.done,
            GanttTaskBarState::Crit => &self.critical,
        }
    }
}

fn source_color<'a>(config: &'a PresentationConfig, key: &str) -> Option<&'a str> {
    let path = format!("themeVariables.{key}");
    config
        .explicit_config_owns_path(&path)
        .then(|| config.get_str(&path))
        .flatten()
}
