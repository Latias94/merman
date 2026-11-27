#[test]
fn class_svg_terminal_title_literal_fallback_replaces_only_first_row() {
    let source = "&amp;gt;<br>**MM**";
    let label = super::ClassSvgTitleLabel::new(source, source);
    assert!(label.literal_first_row());
    assert_eq!(label.lines().len(), 2);
    assert_eq!(label.lines()[0][0].0, source);
    assert_eq!(label.lines()[1][0].0, "MM");
    assert_eq!(
        label.lines()[1][0].1,
        merman_render_core::text::inline_html::MermaidMarkdownWordType::Strong
    );
}
#[test]
fn class_svg_terminal_title_literal_bounds_use_retained_rows() {
    struct LiteralRows;
    impl merman_render_core::text::TextMeasurer for LiteralRows {
        fn measure(
            &self,
            text: &str,
            _: &merman_render_core::text::TextStyle,
        ) -> merman_render_core::text::TextMetrics {
            merman_render_core::text::TextMetrics {
                width: text.chars().count() as f64,
                height: if text.contains("<br>") { 100.0 } else { 10.0 },
                line_count: 1,
            }
        }
        fn measure_svg_tspan_text_bbox_height_px(
            &self,
            text: &str,
            style: &merman_render_core::text::TextStyle,
        ) -> f64 {
            assert_eq!(style.font_weight.as_deref(), Some("bolder"));
            if text == "MM" { 20.0 } else { 10.0 }
        }
    }
    let source = "&amp;gt;<br>**MM**";
    let label = super::ClassSvgTitleLabel::new(source, source);
    let metrics = label.metrics(
        &LiteralRows,
        &merman_render_core::text::TextStyle::default(),
    );
    assert_eq!(metrics.line_count, 2);
    assert!((metrics.height - 37.6).abs() < 1e-9, "{metrics:?}");
}
