#![cfg(feature = "render")]

use merman::render::HeadlessRenderer;

fn render_svg(name: &str, source: &str) -> String {
    HeadlessRenderer::new()
        .with_vendored_text_measurer()
        .with_diagram_id(name)
        .render_svg_sync(source)
        .unwrap_or_else(|err| panic!("{name}: render failed: {err}"))
        .unwrap_or_else(|| panic!("{name}: no diagram detected"))
}

fn assert_renderable_theme_signals(
    name: &str,
    svg: &str,
    expected_labels: &[&str],
    expected_colors: &[&str],
) {
    assert!(svg.starts_with("<svg"), "{name}: expected SVG output");
    assert!(!svg.contains("NaN"), "{name}: SVG leaked NaN geometry");

    let svg_without_upstream_placeholder_classes = svg
        .replace(r#"class="cluster undefined "#, r#"class="cluster "#)
        .replace(r#"class="node undefined"#, r#"class="node"#);
    assert!(
        !svg_without_upstream_placeholder_classes.contains("undefined"),
        "{name}: SVG leaked undefined tokens"
    );

    for label in expected_labels {
        assert!(
            svg.contains(label),
            "{name}: expected rendered label {label:?}"
        );
    }

    for color in expected_colors {
        assert!(
            svg.contains(color),
            "{name}: expected visible theme color {color:?}"
        );
    }
}

#[test]
fn representative_dark_theme_diagrams_keep_visible_theme_signals() {
    let cases: &[(&str, &str, &[&str], &[&str])] = &[
        (
            "theme-flowchart",
            r##"%%{init: {"themeVariables": {"mainBkg": "#111827", "primaryTextColor": "#f8fafc", "nodeBorder": "#38bdf8", "lineColor": "#f59e0b", "edgeLabelBackground": "#0f172a", "nodeTextColor": "#f8fafc"}}}%%
flowchart TD
  A[Dark Node] -->|Readable Edge| B[Other]
"##,
            &["Dark Node", "Readable Edge", "Other"],
            &["#111827", "#f8fafc", "#38bdf8", "#f59e0b"],
        ),
        (
            "theme-sequence",
            r##"%%{init: {"themeVariables": {"actorBkg": "#111827", "actorBorder": "#38bdf8", "actorTextColor": "#f8fafc", "signalColor": "#22c55e", "signalTextColor": "#facc15", "noteBkgColor": "#1f2937", "noteBorderColor": "#f97316", "noteTextColor": "#fef3c7"}}}%%
sequenceDiagram
  participant A as Alpha
  participant B as Beta
  A->>B: Request
  Note over A,B: Dark note
"##,
            &["Alpha", "Beta", "Request", "Dark note"],
            &[
                "#111827", "#38bdf8", "#f8fafc", "#22c55e", "#facc15", "#1f2937", "#f97316",
            ],
        ),
        (
            "theme-kanban",
            r##"%%{init: {"themeVariables": {"background": "#0f172a", "nodeBorder": "#38bdf8", "textColor": "#f8fafc", "git0": "#22c55e", "gitBranchLabel0": "#020617", "cScale0": "hsl(160, 80%, 40%)", "cScaleLabel0": "#f8fafc", "cScaleInv0": "#111827"}}}%%
kanban
  todo[Todo]
    card[Dark Card]@{ assigned: "Core", priority: "High" }
"##,
            &["Todo", "Dark Card", "Core"],
            &["#0f172a", "#38bdf8", "#f8fafc", "#22c55e", "#020617"],
        ),
        (
            "theme-gitgraph",
            r##"%%{init: {"themeVariables": {"git0": "#22c55e", "gitBranchLabel0": "#020617", "git1": "#38bdf8", "gitBranchLabel1": "#0f172a", "commitLabelColor": "#f8fafc", "commitLabelBackground": "#111827", "commitLineColor": "#f59e0b"}}}%%
gitGraph
  commit id: "A"
  branch dev
  checkout dev
  commit id: "B"
"##,
            &["main", "dev"],
            &[
                "#22c55e", "#020617", "#38bdf8", "#0f172a", "#f8fafc", "#111827", "#f59e0b",
            ],
        ),
        (
            "theme-xychart",
            r##"%%{init: {"themeVariables": {"xyChart": {"backgroundColor": "#0f172a", "titleColor": "#f8fafc", "xAxisLabelColor": "#93c5fd", "xAxisTitleColor": "#bfdbfe", "xAxisTickColor": "#38bdf8", "xAxisLineColor": "#60a5fa", "yAxisLabelColor": "#fde68a", "yAxisTitleColor": "#facc15", "yAxisTickColor": "#f97316", "yAxisLineColor": "#fb923c", "plotColorPalette": "#22c55e,#e879f9"}}}}%%
xychart
  title "Theme Chart"
  x-axis Months [Jan, Feb]
  y-axis Revenue 0 --> 10
  bar [3, 7]
  line [3, 7]
"##,
            &["Theme Chart", "Months", "Revenue", "Jan"],
            &[
                "#0f172a", "#f8fafc", "#93c5fd", "#bfdbfe", "#38bdf8", "#60a5fa", "#fde68a",
                "#facc15", "#f97316", "#fb923c", "#22c55e", "#e879f9",
            ],
        ),
    ];

    for (name, source, expected_labels, expected_colors) in cases {
        let svg = render_svg(name, source);
        assert_renderable_theme_signals(name, &svg, expected_labels, expected_colors);
    }
}
