use super::*;
use serde_json::json;

#[test]
fn mermaid_theme_adapter_prefers_explicit_dark_mode_over_theme_name() {
    let typed_dark = json!({
        "theme": "base",
        "themeVariables": { "darkMode": true }
    });
    assert!(MermaidThemeAdapter::new(&typed_dark).common.is_dark_theme());

    let explicitly_light = json!({
        "theme": "dark",
        "darkMode": false,
        "themeVariables": { "darkMode": true }
    });
    assert!(
        !MermaidThemeAdapter::new(&explicitly_light)
            .common
            .is_dark_theme()
    );
}

#[test]
fn mermaid_theme_adapter_node_diagram_uses_shared_fallbacks() {
    let cfg = json!({});
    let theme = MermaidThemeAdapter::new(&cfg);
    let node = theme.node_diagram();

    assert_eq!(node.common.text_color, "#333");
    assert_eq!(node.common.line_color, "#333333");
    assert_eq!(node.node_text_color, "#333");
    assert_eq!(node.title_color, "#333");
    assert_eq!(node.main_bkg, "#ECECFF");
    assert_eq!(node.node_border, "#9370DB");
    assert_eq!(node.arrowhead_color, "#333333");
    assert_eq!(node.stroke_width, "1");
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

    let meter = crate::resources::OperationWorkMeter::new(
        crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
    );
    let task_theme = crate::gantt::GanttTaskTheme::resolve(
        None,
        &merman_core::MermaidConfig::from_value(cfg),
        &[],
        &meter,
    )
    .expect("resolve Gantt CSS binding");
    let gantt = task_theme.css_binding();

    assert_eq!(
        task_theme.font_family_css(),
        r#""ibm plex sans",arial,sans-serif"#
    );
    assert_eq!(gantt.text_color, "#707070");
    assert_eq!(gantt.exclude_bkg_color, "#101010");
    assert_eq!(gantt.section_bkg_color, "#202020");
    assert_eq!(gantt.section_bkg_color2, "#303030");
    assert_eq!(gantt.alt_section_bkg_color, "#404040");
    assert_eq!(gantt.title_color, "#505050");
    let css = crate::svg::parity::css::gantt_css("g", &task_theme);
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

    let meter = crate::resources::OperationWorkMeter::new(
        crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
    );
    let task_theme = crate::gantt::GanttTaskTheme::resolve(
        None,
        &merman_core::MermaidConfig::from_value(cfg),
        &[],
        &meter,
    )
    .expect("resolve Gantt CSS binding");
    let gantt = task_theme.css_binding();

    assert_eq!(gantt.title_color, "   ");
    let css = crate::svg::parity::css::gantt_css("g", &task_theme);
    assert!(css.contains(r#"#g .titleText{text-anchor:middle;font-size:18px;fill:#707070;"#));
}

#[test]
fn prepared_kanban_binding_resolves_theme_roles() {
    let cfg = json!({
        "themeVariables": {
            "background": "#0f172a",
            "nodeBorder": "#38bdf8",
            "textColor": "#f8fafc",
            "git0": "#22c55e",
            "gitBranchLabel0": "#020617",
            "cScale0": "hsl(160, 80%, 40%)",
            "cScaleLabel0": "#f8fafc",
            "cScaleInv0": "#111827"
        }
    });

    let kanban = crate::kanban::KanbanCssBinding::for_test(&cfg, None).unwrap();

    assert_eq!(kanban.common.text_color(), "#f8fafc");
    assert_eq!(kanban.background, "#0f172a");
    assert_eq!(kanban.node_border, "#38bdf8");
    assert_eq!(kanban.root_fill, "#22c55e");
    assert_eq!(kanban.root_label, "#020617");
    assert_eq!(kanban.sections[0].c_scale, "hsl(160, 80%, 40%)");
    assert_eq!(kanban.sections[0].section_fill, "hsl(160, 80%, 50%)");
    assert_eq!(kanban.sections[0].c_scale_label, "#f8fafc");
    assert_eq!(kanban.sections[0].c_scale_inv, "#111827");
}

#[test]
fn prepared_kanban_binding_dark_mode_adjusts_section_fill_down() {
    let cfg = json!({
        "darkMode": true
    });

    let kanban = crate::kanban::KanbanCssBinding::for_test(&cfg, None).unwrap();

    assert_eq!(kanban.sections[0].c_scale, "hsl(240, 100%, 76.2745098039%)");
    assert_eq!(
        kanban.sections[0].section_fill,
        "hsl(240, 100%, 66.2745098039%)"
    );
    assert_eq!(kanban.sections[0].c_scale_label, "#ffffff");
    assert_eq!(
        kanban.sections[0].c_scale_inv,
        "hsl(60, 100%, 86.2745098039%)"
    );
}

#[test]
fn prepared_kanban_binding_uses_khroma_for_every_supported_color_syntax() {
    let cases = [
        ("#ff0000", "hsl(0, 100%, 60%)"),
        ("rebeccapurple", "hsl(270, 50%, 50%)"),
        (
            "rgb(18 52 86 / .5)",
            "hsla(210, 65.3846153846%, 30.3921568627%, 0.5)",
        ),
    ];

    for (input, expected) in cases {
        let cfg = json!({ "themeVariables": { "cScale0": input } });
        let kanban = crate::kanban::KanbanCssBinding::for_test(&cfg, None).unwrap();
        assert_eq!(kanban.sections[0].section_fill, expected, "{input}");
    }
}

#[test]
fn prepared_kanban_binding_rejects_invalid_derived_colors() {
    let cfg = json!({ "themeVariables": { "cScale0": "var(--runtime-color)" } });
    let error = crate::kanban::KanbanCssBinding::for_test(&cfg, None).unwrap_err();
    assert!(matches!(error, crate::Error::Color(_)));
}

#[test]
fn radar_binding_resolves_style_roles() {
    let cfg = json!({
        "fontFamily": "Ignored, sans-serif",
        "themeVariables": {
            "fontFamily": "\"ibm plex sans\", arial, sans-serif",
            "fontSize": 18,
            "textColor": "#101010",
            "lineColor": "#111111",
            "errorBkgColor": "#121212",
            "errorTextColor": "#131313",
            "titleColor": "#202020",
            "cScale0": "#303030",
            "radar": {
                "axisColor": "#404040",
                "axisStrokeWidth": 2,
                "axisLabelFontSize": 12,
                "graticuleColor": "#505050",
                "graticuleOpacity": 0.3,
                "graticuleStrokeWidth": 1,
                "legendFontSize": 12,
                "curveOpacity": 0.5,
                "curveStrokeWidth": 2
            }
        },
        "radar": {
            "axisColor": "#606060",
            "axisStrokeWidth": 4,
            "axisLabelFontSize": 14,
            "graticuleColor": "#707070",
            "graticuleOpacity": 0.8,
            "graticuleStrokeWidth": 5,
            "legendFontSize": 16,
            "curveOpacity": 0.9,
            "curveStrokeWidth": 6
        }
    });

    let radar = crate::radar::RadarCssBinding::resolve(&cfg);

    assert_eq!(radar.text_color, "#101010");
    assert_eq!(radar.line_color, "#111111");
    assert_eq!(radar.error_bkg_color, "#121212");
    assert_eq!(radar.error_text_color, "#131313");
    assert_eq!(radar.title_color, "#202020");
    assert_eq!(radar.axis_color, "#606060");
    assert_eq!(radar.axis_stroke_width, 4.0);
    assert_eq!(radar.axis_label_font_size, 14.0);
    assert_eq!(radar.graticule_color, "#707070");
    assert_eq!(radar.graticule_opacity, 0.8);
    assert_eq!(radar.graticule_stroke_width, 5.0);
    assert_eq!(radar.legend_font_size, 16.0);
    assert_eq!(radar.curve_opacity, 0.9);
    assert_eq!(radar.curve_stroke_width, 6.0);
    let series_colors = &radar.series_colors;
    assert_eq!(series_colors[0], "#303030");
    assert_eq!(series_colors[11], "hsl(210, 100%, 76.2745098039%)");
}

#[test]
fn radar_binding_uses_default_style_roles() {
    let cfg = json!({});

    let radar = crate::radar::RadarCssBinding::resolve(&cfg);

    assert_eq!(radar.text_color, "#333");
    assert_eq!(radar.line_color, "#333333");
    assert_eq!(radar.error_bkg_color, "#552222");
    assert_eq!(radar.error_text_color, "#552222");
    assert_eq!(radar.title_color, "#333");
    assert_eq!(radar.axis_color, "#333333");
    assert_eq!(radar.axis_stroke_width, 2.0);
    assert_eq!(radar.axis_label_font_size, 12.0);
    assert_eq!(radar.graticule_color, "#DEDEDE");
    assert_eq!(radar.graticule_opacity, 0.3);
    assert_eq!(radar.graticule_stroke_width, 1.0);
    assert_eq!(radar.legend_font_size, 12.0);
    assert_eq!(radar.curve_opacity, 0.5);
    assert_eq!(radar.curve_stroke_width, 2.0);
    let series_colors = &radar.series_colors;
    assert_eq!(series_colors[0], "hsl(240, 100%, 76.2745098039%)");
}
