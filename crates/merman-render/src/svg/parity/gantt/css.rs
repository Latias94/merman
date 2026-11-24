use super::super::SvgDiagramIdValue;
use super::super::util::css_selector_diagram_id;
use std::fmt::Write as _;

pub(super) fn gantt_css<I>(diagram_id: I, theme: &crate::gantt::GanttCssBinding) -> String
where
    I: SvgDiagramIdValue,
{
    let id = css_selector_diagram_id(diagram_id);
    let label_priority = "!important";
    let mut out = String::new();
    theme
        .common
        .write_prefix(&mut out, diagram_id)
        .expect("String-backed Gantt base CSS emission cannot fail");
    let font = theme.common.font_family();
    if let Some(axis_color) = theme.axis_color.as_deref() {
        // D3 tick lines inherit currentColor from the grid group.
        let _ = write!(&mut out, "#{id} .grid{{color:{axis_color};}}");
    }
    let text_color = theme.text_color.as_str();
    let exclude_bkg_color = &theme.exclude_bkg_color;
    let section_bkg_color = &theme.section_bkg_color;
    let section_bkg_color2 = &theme.section_bkg_color2;
    let alt_section_bkg_color = &theme.alt_section_bkg_color;
    let title_color = theme.title_color.as_str();
    let grid_color = &theme.grid_color;
    let today_line_color = &theme.today_line_color;
    let task_text_dark_color = &theme.task_text_dark_color;
    let task_text_clickable_color = &theme.task_text_clickable_color;
    let task_text_color = &theme.task_text_color;
    let task_bkg_color = &theme.task_bkg_color;
    let task_border_color = &theme.task_border_color;
    let task_text_outside_color = &theme.task_text_outside_color;
    let active_task_bkg_color = &theme.active_task_bkg_color;
    let active_task_border_color = &theme.active_task_border_color;
    let done_task_border_color = &theme.done_task_border_color;
    let done_task_bkg_color = &theme.done_task_bkg_color;
    let crit_border_color = &theme.crit_border_color;
    let crit_bkg_color = &theme.crit_bkg_color;
    let vert_line_color = &theme.vert_line_color;
    let title_text_color = &theme.title_text_color;

    fn push_outside_done_text_rules<I>(
        out: &mut String,
        id: I,
        class_prefix: &str,
        color: &str,
        priority: &str,
    ) where
        I: Copy + std::fmt::Display,
    {
        let mut first = true;
        for i in 0..4 {
            for side in ["Left", "Right"] {
                if first {
                    first = false;
                } else {
                    out.push(',');
                }
                let _ = write!(
                    out,
                    "#{} .{}{}.taskTextOutside{}",
                    id, class_prefix, i, side
                );
            }
        }
        let _ = write!(out, "{{fill:{}{};}}", color, priority);
    }

    let _ = write!(
        &mut out,
        r#"#{} .mermaid-main-font{{font-family:{};}}"#,
        id, font
    );
    let _ = write!(
        &mut out,
        r#"#{} .exclude-range{{fill:{};}}"#,
        id, exclude_bkg_color
    );
    let _ = write!(&mut out, r#"#{} .section{{stroke:none;opacity:0.2;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} .section0{{fill:{};}}"#,
        id, section_bkg_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .section2{{fill:{};}}"#,
        id, section_bkg_color2
    );
    let _ = write!(
        &mut out,
        r#"#{} .section1,#{} .section3{{fill:{};opacity:0.2;}}"#,
        id, id, alt_section_bkg_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .sectionTitle0{{fill:{};}}#{} .sectionTitle1{{fill:{};}}#{} .sectionTitle2{{fill:{};}}#{} .sectionTitle3{{fill:{};}}"#,
        id, title_color, id, title_color, id, title_color, id, title_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .sectionTitle{{text-anchor:start;font-family:{};}}"#,
        id, font
    );
    let _ = write!(
        &mut out,
        r#"#{} .grid .tick{{stroke:{};opacity:0.8;shape-rendering:crispEdges;}}#{} .grid .tick text{{font-family:{};fill:{};}}#{} .grid path{{stroke-width:0;}}"#,
        id, grid_color, id, font, text_color, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .today{{fill:none;stroke:{};stroke-width:2px;}}"#,
        id, today_line_color
    );
    let _ = write!(&mut out, r#"#{} .task{{stroke-width:2;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} .taskText{{text-anchor:middle;font-family:{};}}#{} .taskTextOutsideRight{{fill:{};text-anchor:start;font-family:{};}}#{} .taskTextOutsideLeft{{fill:{};text-anchor:end;}}"#,
        id, font, id, task_text_dark_color, font, id, task_text_dark_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .task.clickable{{cursor:pointer;}}#{} .taskText.clickable{{cursor:pointer;fill:{}{label_priority};font-weight:bold;}}#{} .taskTextOutsideLeft.clickable{{cursor:pointer;fill:{}{label_priority};font-weight:bold;}}#{} .taskTextOutsideRight.clickable{{cursor:pointer;fill:{}{label_priority};font-weight:bold;}}"#,
        id,
        id,
        task_text_clickable_color,
        id,
        task_text_clickable_color,
        id,
        task_text_clickable_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .taskText0,#{} .taskText1,#{} .taskText2,#{} .taskText3{{fill:{};}}"#,
        id, id, id, id, task_text_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .task0,#{} .task1,#{} .task2,#{} .task3{{fill:{};stroke:{};}}"#,
        id, id, id, id, task_bkg_color, task_border_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .taskTextOutside0,#{} .taskTextOutside2{{fill:{};}}#{} .taskTextOutside1,#{} .taskTextOutside3{{fill:{};}}"#,
        id, id, task_text_outside_color, id, id, task_text_outside_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .active0,#{} .active1,#{} .active2,#{} .active3{{fill:{};stroke:{};}}"#,
        id, id, id, id, active_task_bkg_color, active_task_border_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .activeText0,#{} .activeText1,#{} .activeText2,#{} .activeText3{{fill:{}{label_priority};}}"#,
        id, id, id, id, task_text_dark_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .done0,#{} .done1,#{} .done2,#{} .done3{{stroke:{};fill:{};stroke-width:2;}}"#,
        id, id, id, id, done_task_border_color, done_task_bkg_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .doneText0,#{} .doneText1,#{} .doneText2,#{} .doneText3{{fill:{}{label_priority};}}"#,
        id, id, id, id, task_text_dark_color
    );
    push_outside_done_text_rules(
        &mut out,
        id,
        "doneText",
        task_text_outside_color,
        label_priority,
    );
    let _ = write!(
        &mut out,
        r#"#{} .crit0,#{} .crit1,#{} .crit2,#{} .crit3{{stroke:{};fill:{};stroke-width:2;}}"#,
        id, id, id, id, crit_border_color, crit_bkg_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .activeCrit0,#{} .activeCrit1,#{} .activeCrit2,#{} .activeCrit3{{stroke:{};fill:{};stroke-width:2;}}"#,
        id, id, id, id, crit_border_color, active_task_bkg_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .doneCrit0,#{} .doneCrit1,#{} .doneCrit2,#{} .doneCrit3{{stroke:{};fill:{};stroke-width:2;cursor:pointer;shape-rendering:crispEdges;}}"#,
        id, id, id, id, crit_border_color, done_task_bkg_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .milestone{{transform:rotate(45deg) scale(0.8,0.8);}}#{} .milestoneText{{font-style:italic;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .doneCritText0,#{} .doneCritText1,#{} .doneCritText2,#{} .doneCritText3{{fill:{}{label_priority};}}"#,
        id, id, id, id, task_text_dark_color
    );
    push_outside_done_text_rules(
        &mut out,
        id,
        "doneCritText",
        task_text_outside_color,
        label_priority,
    );
    let _ = write!(
        &mut out,
        r#"#{} .vert{{stroke:{};}}#{} .vertText{{font-size:15px;text-anchor:middle;fill:{}{label_priority};}}"#,
        id, vert_line_color, id, vert_line_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .activeCritText0,#{} .activeCritText1,#{} .activeCritText2,#{} .activeCritText3{{fill:{}{label_priority};}}"#,
        id, id, id, id, task_text_dark_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .titleText{{text-anchor:middle;font-size:18px;fill:{};font-family:{};}}"#,
        id, title_text_color, font
    );

    theme
        .common
        .write_root(&mut out, diagram_id, diagram_id)
        .expect("String-backed Gantt root CSS emission cannot fail");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn gantt_css_honors_mermaid_11_15_theme_options() {
        let cfg = serde_json::json!({
            "themeVariables": {
                "fontFamily": "\"ibm plex sans\", arial, sans-serif",
                "textColor": "#707070",
                "excludeBkgColor": "#101010",
                "sectionBkgColor": "#202020",
                "sectionBkgColor2": "#303030",
                "altSectionBkgColor": "#404040",
                "titleColor": "#505050",
                "gridColor": "#606060",
                "todayLineColor": "#808080",
                "taskTextDarkColor": "#909090",
                "taskTextClickableColor": "#a0a0a0",
                "taskTextColor": "#b0b0b0",
                "taskBkgColor": "#c0c0c0",
                "taskBorderColor": "#d0d0d0",
                "taskTextOutsideColor": "#e0e0e0",
                "activeTaskBkgColor": "#111111",
                "activeTaskBorderColor": "#222222",
                "doneTaskBorderColor": "#333333",
                "doneTaskBkgColor": "#444444",
                "critBorderColor": "#555555",
                "critBkgColor": "#666666",
                "vertLineColor": "#777777"
            }
        });

        let config = crate::test_support::presentation_config(cfg);
        let style = crate::gantt::GanttStyle::resolve(&config, None);
        let css = gantt_css("g", style.css_binding());

        assert!(css.contains(r#"#g .exclude-range{fill:#101010;}"#));
        assert!(css.contains(r#"#g .section0{fill:#202020;}"#));
        assert!(css.contains(r#"#g .section2{fill:#303030;}"#));
        assert!(css.contains(r#"#g .grid .tick{stroke:#606060;"#));
        assert!(
            css.contains(
                r#"#g .taskText0,#g .taskText1,#g .taskText2,#g .taskText3{fill:#b0b0b0;}"#
            )
        );
        assert!(
            css.contains(
                r#"#g .task0,#g .task1,#g .task2,#g .task3{fill:#c0c0c0;stroke:#d0d0d0;}"#
            )
        );
        assert!(
            css.contains(r#"#g .doneText0.taskTextOutsideLeft,#g .doneText0.taskTextOutsideRight"#)
        );
        assert!(css.contains(r#"fill:#e0e0e0!important;"#));
        assert!(css.contains(r#"#g .titleText{text-anchor:middle;font-size:18px;fill:#505050;font-family:"ibm plex sans",arial,sans-serif;}"#));
    }

    #[test]
    fn prepared_gantt_binding_resolves_gantt_roles() {
        let cfg = json!({
            "themeVariables": {
                "fontFamily": "\"ibm plex sans\", arial, sans-serif",
                "textColor": "#707070",
                "excludeBkgColor": "#101010",
                "sectionBkgColor": "#202020",
                "sectionBkgColor2": "#303030",
                "altSectionBkgColor": "#404040",
                "titleColor": "#505050",
                "gridColor": "#606060",
                "todayLineColor": "#808080",
                "taskTextDarkColor": "#909090",
                "taskTextClickableColor": "#a0a0a0",
                "taskTextColor": "#b0b0b0",
                "taskBkgColor": "#c0c0c0",
                "taskBorderColor": "#d0d0d0",
                "taskTextOutsideColor": "#e0e0e0",
                "activeTaskBkgColor": "#111111",
                "activeTaskBorderColor": "#222222",
                "doneTaskBorderColor": "#333333",
                "doneTaskBkgColor": "#444444",
                "critBorderColor": "#555555",
                "critBkgColor": "#666666",
                "vertLineColor": "#777777"
            }
        });

        let config = crate::test_support::presentation_config(cfg);
        let style = crate::gantt::GanttStyle::resolve(&config, None);
        let gantt = style.css_binding();

        assert_eq!(
            style.font_family_css(),
            r#""ibm plex sans",arial,sans-serif"#
        );
        assert_eq!(gantt.text_color, "#707070");
        assert_eq!(gantt.exclude_bkg_color, "#101010");
        assert_eq!(gantt.section_bkg_color, "#202020");
        assert_eq!(gantt.section_bkg_color2, "#303030");
        assert_eq!(gantt.alt_section_bkg_color, "#404040");
        assert_eq!(gantt.title_color, "#505050");
        let css = gantt_css("g", gantt);
        assert!(css.contains(r#"#g .titleText{text-anchor:middle;font-size:18px;fill:#505050;"#));
        assert_eq!(gantt.grid_color, "#606060");
        assert_eq!(gantt.today_line_color, "#808080");
        assert_eq!(gantt.task_text_dark_color, "#909090");
        assert_eq!(gantt.task_text_clickable_color, "#a0a0a0");
        assert_eq!(gantt.task_text_color, "#b0b0b0");
        assert_eq!(gantt.task_bkg_color, "#c0c0c0");
        assert_eq!(gantt.task_border_color, "#d0d0d0");
        assert_eq!(gantt.task_text_outside_color, "#e0e0e0");
        assert_eq!(gantt.active_task_bkg_color, "#111111");
        assert_eq!(gantt.active_task_border_color, "#222222");
        assert_eq!(gantt.done_task_border_color, "#333333");
        assert_eq!(gantt.done_task_bkg_color, "#444444");
        assert_eq!(gantt.crit_border_color, "#555555");
        assert_eq!(gantt.crit_bkg_color, "#666666");
        assert_eq!(gantt.vert_line_color, "#777777");
    }

    #[test]
    fn prepared_gantt_binding_uses_text_color_for_empty_title_color() {
        let cfg = json!({
            "themeVariables": {
                "textColor": "#707070",
                "titleColor": "   "
            }
        });

        let config = crate::test_support::presentation_config(cfg);
        let style = crate::gantt::GanttStyle::resolve(&config, None);
        let gantt = style.css_binding();

        assert_eq!(gantt.title_color, "   ");
        let css = gantt_css("g", gantt);
        assert!(css.contains(r#"#g .titleText{text-anchor:middle;font-size:18px;fill:#707070;"#));
    }
}
