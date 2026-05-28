#![cfg(feature = "render")]

use merman::render::HeadlessRenderer;
use std::panic::{AssertUnwindSafe, catch_unwind};

const ZED_57389_SEQUENCE_LOOP_END: &str =
    include_str!("../../../fixtures/zed_issues/zed_57389_sequence_loop_end.mmd");
const ZED_57363_FLOWCHART_HYPHEN_EDGE_LABELS: &str =
    include_str!("../../../fixtures/zed_issues/zed_57363_flowchart_hyphen_edge_labels.mmd");
const ZED_57323_ER_ENTITY_STYLE_TEXT: &str =
    include_str!("../../../fixtures/zed_issues/zed_57323_er_entity_style_text.mmd");
const ZED_56767_FLOWCHART_FOREIGN_OBJECT_LABELS: &str =
    include_str!("../../../fixtures/zed_issues/zed_56767_flowchart_foreign_object_labels.mmd");
const ZED_51480_COMPLEX_FLOWCHART_CONNECTIONS: &str =
    include_str!("../../../fixtures/zed_issues/zed_51480_complex_flowchart_connections.mmd");
const ZED_51142_SEQUENCE_RECT_RGB: &str =
    include_str!("../../../fixtures/zed_issues/zed_51142_sequence_rect_rgb.mmd");
const ZED_50558_CLASS_INHERITANCE: &str =
    include_str!("../../../fixtures/zed_issues/zed_50558_class_inheritance.mmd");
const ZED_50243_GANTT_COMPACT_FRONTMATTER: &str =
    include_str!("../../../fixtures/zed_issues/zed_50243_gantt_compact_frontmatter.mmd");
const ZED_56199_FLOWCHART_PARTIAL_PARALLELOGRAM: &str =
    include_str!("../../../fixtures/zed_issues/zed_56199_flowchart_partial_parallelogram.mmd");

fn renderer(id: &str) -> HeadlessRenderer {
    HeadlessRenderer::new()
        .with_vendored_text_measurer()
        .with_diagram_id(id)
}

fn render_resvg_safe(name: &str, source: &str) -> String {
    renderer(name)
        .render_svg_resvg_safe_sync(source)
        .unwrap_or_else(|err| panic!("{name}: headless render failed: {err}"))
        .unwrap_or_else(|| panic!("{name}: no diagram detected"))
}

#[test]
fn zed_issue_fixtures_render_headless_resvg_safe() {
    let cases: &[(&str, &str, &[&str])] = &[
        (
            "zed-57389",
            ZED_57389_SEQUENCE_LOOP_END,
            &["for each iteration", "Post loop"],
        ),
        (
            "zed-57363",
            ZED_57363_FLOWCHART_HYPHEN_EDGE_LABELS,
            &["single-string-with-hyphens"],
        ),
        (
            "zed-57323",
            ZED_57323_ER_ENTITY_STYLE_TEXT,
            &["EMPRESA", "empresa_id", "UNIDADE", "USUARIO", "employs"],
        ),
        (
            "zed-56767",
            ZED_56767_FLOWCHART_FOREIGN_OBJECT_LABELS,
            &["Start", "feature/example-branch", "Open pull request"],
        ),
        (
            "zed-51480",
            ZED_51480_COMPLEX_FLOWCHART_CONNECTIONS,
            &[
                "Stage 1: MVP",
                "Stage 2: Production",
                "Versioned market definitions",
            ],
        ),
        (
            "zed-51142",
            ZED_51142_SEQUENCE_RECT_RGB,
            &[
                "Phase 1 - Startup",
                "startup_reconcile()",
                "filesystem timestamps",
            ],
        ),
        (
            "zed-50558",
            ZED_50558_CLASS_INHERITANCE,
            &["Animal", "Dog", "bark()"],
        ),
        (
            "zed-50243",
            ZED_50243_GANTT_COMPACT_FRONTMATTER,
            &["MacOS 26 Tahoe", "x86 hardware"],
        ),
    ];

    for (name, source, expected_labels) in cases {
        let svg = render_resvg_safe(name, source);
        assert!(svg.starts_with("<svg"), "{name}: expected SVG output");
        assert!(
            !svg.contains("<foreignObject"),
            "{name}: resvg-safe output should not rely on foreignObject"
        );
        assert!(
            !svg.contains("@keyframes") && !svg.contains(":root"),
            "{name}: resvg-safe output should strip unsupported CSS constructs"
        );
        assert!(
            !svg.contains("NaN") && !svg.contains("undefined"),
            "{name}: output should not leak non-finite geometry"
        );

        for label in *expected_labels {
            assert!(
                svg.contains(label),
                "{name}: expected rendered SVG to contain label {label:?}"
            );
        }
    }
}

#[test]
fn zed_sequence_rect_rgb_renders_as_background_rect() {
    let svg = render_resvg_safe("zed-51142-rect", ZED_51142_SEQUENCE_RECT_RGB);

    assert!(
        svg.contains(r#"class="rect""#),
        "expected sequence rect block to render as a rectangle element"
    );
    assert!(
        svg.contains(r#"fill="rgb(240, 245, 255)""#),
        "expected rect rgb payload to become the rectangle fill"
    );
    assert!(
        !svg.contains(">rgb(240, 245, 255)<"),
        "rect rgb payload must not be emitted as visible message text"
    );
}

#[test]
fn zed_flowchart_foreign_object_labels_have_text_fallbacks() {
    let svg = render_resvg_safe(
        "zed-56767-foreign-object",
        ZED_56767_FLOWCHART_FOREIGN_OBJECT_LABELS,
    );

    assert!(
        svg.contains(r#"data-merman-foreignobject="fallback""#),
        "expected resvg-safe output to retain generated text fallback groups"
    );
    assert!(
        svg.contains(">Prepare change</text>") || svg.contains(">Prepare change</tspan>"),
        "expected a readable SVG text fallback for a flowchart node label"
    );
}

#[test]
fn zed_class_generics_fallback_text_is_not_double_escaped() {
    let svg = render_resvg_safe(
        "zed-class-generics",
        r#"classDiagram
    class Shelter {
        -List~Animal~ animals
        +adopt(Animal a) bool
    }"#,
    );

    assert!(
        !svg.contains("&amp;lt;") && !svg.contains("&amp;gt;"),
        "fallback text should not double-escape class generic markers: {svg}"
    );
    assert!(
        svg.contains("List&lt;Animal"),
        "expected class generic marker to remain readable in fallback text: {svg}"
    );
}

#[test]
fn zed_resvg_safe_output_drops_empty_rect_placeholders() {
    let svg = render_resvg_safe("zed-empty-rects", "flowchart TD\n    A[Hello] --> B[World]");

    assert!(
        !svg.contains("<rect/>"),
        "resvg-safe output should drop empty rect placeholders: {svg}"
    );
}

#[test]
fn zed_old_mermaid_rs_partial_parallelogram_stays_inside_result_boundary() {
    let result = catch_unwind(AssertUnwindSafe(|| {
        HeadlessRenderer::new()
            .with_lenient_parsing()
            .with_diagram_id("zed-56199")
            .render_svg_resvg_safe_sync(ZED_56199_FLOWCHART_PARTIAL_PARALLELOGRAM)
    }));

    let render_result =
        result.expect("renderer must not panic on partially typed flowchart shapes");
    let svg = render_result
        .expect("lenient parser should return an error diagram instead of a render error")
        .expect("lenient parser should still produce an SVG error diagram");

    assert!(svg.contains(r#"aria-roledescription="error""#));
    assert!(svg.contains("Syntax error in text"));
    assert!(
        !svg.contains("@keyframes") && !svg.contains(":root"),
        "error diagrams should still pass through resvg-safe cleanup"
    );
}
