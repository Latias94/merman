use serde_json::json;

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
