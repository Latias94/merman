use crate::mermaid_style::HtmlTextGeometryDeclarations;
#[cfg(any(feature = "layout-dagre", feature = "layout-elk", test))]
use crate::model::{Bounds, LayoutEdge, LayoutNode};
#[cfg(all(
    test,
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-mindmap"
    )
))]
use crate::text::{TextMeasurer, TextMetrics, TextStyle};
use crate::text::{WrapMode, trim_html_collapsible_ascii_whitespace};
use merman_model::presentation::PresentationConfig;
#[cfg(feature = "markdown-labels")]
pub(crate) use merman_render_core::text::create_text_html::{
    FlowchartLabelMetricsRequest, flowchart_label_html_source,
};
#[cfg(all(
    test,
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-mindmap"
    )
))]
pub(crate) use merman_render_core::text::flowchart_wrap_svg_source_word_lines;
pub(crate) use merman_render_core::text::html_label_layout::flowchart_html_label_box;
#[cfg(feature = "markdown-labels")]
pub(crate) use merman_render_core::text::html_label_layout::flowchart_label_plain_text_for_layout;
pub(crate) use merman_render_core::text::literal_label::{
    FlowchartSvgLabelSource, FlowchartSvgWidthMode, flowchart_non_markdown_svg_source_word_lines,
};

/// The FlowDB minimum only reaches ordinary nodes that use `labelHelper`.
/// Asset shapes set `node.width` before calling it; fixed-size symbols and label-derived
/// circles opt out of it.
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) fn flowchart_node_label_min_width(
    raw_label: &str,
    layout_shape: Option<&str>,
    config: &PresentationConfig,
) -> f64 {
    use crate::flowchart::FlowchartShape;

    let shape = FlowchartShape::resolve(layout_shape.unwrap_or("squareRect"));
    if raw_label.is_empty()
        || matches!(
            shape,
            Some(
                FlowchartShape::Anchor
                    | FlowchartShape::Circle
                    | FlowchartShape::Choice
                    | FlowchartShape::CrossedCircle
                    | FlowchartShape::DoubleCircle
                    | FlowchartShape::FilledCircle
                    | FlowchartShape::ForkJoin
                    | FlowchartShape::FramedCircle
                    | FlowchartShape::SmallCircle
                    | FlowchartShape::LightningBolt
                    | FlowchartShape::Icon
                    | FlowchartShape::IconCircle
                    | FlowchartShape::IconRounded
                    | FlowchartShape::IconSquare
                    | FlowchartShape::ImageSquare
                    | FlowchartShape::CollapsedGroup
            )
        )
    {
        return 0.0;
    }
    crate::config::config_f64(config, &["flowchart", "minNodeWidth"])
        .unwrap_or(0.0)
        .max(0.0)
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) fn flowchart_node_svg_width_mode(
    raw_label: &str,
    label_type: &str,
    wrap_mode: WrapMode,
    layout_shape: &str,
) -> FlowchartSvgWidthMode {
    if wrap_mode == WrapMode::SvgLike
        && label_type != "markdown"
        && !raw_label.contains('<')
        && !raw_label.contains('>')
        && crate::flowchart::is_flowchart_process_shape(layout_shape)
    {
        FlowchartSvgWidthMode::ComputedLength
    } else {
        FlowchartSvgWidthMode::Bbox
    }
}

// Mermaid 11.16 inserts decoded labels through `addHtmlSpan(...).html(...)`. HTML collapses the
// ASCII whitespace set at a boundary, but U+00A0 remains visible and contributes to the line box.
pub(crate) fn flowchart_trim_html_collapsible_whitespace(input: &str) -> &str {
    trim_html_collapsible_ascii_whitespace(input)
}

pub(crate) fn flowchart_label_text_is_empty_for_mode(text: &str, _html_labels: bool) -> bool {
    flowchart_trim_html_collapsible_whitespace(text).is_empty()
}

pub(crate) fn flowchart_label_is_empty_for_render(label: &str) -> bool {
    // Mermaid admits an edge label whenever FlowDB's already-trimmed label string is non-empty.
    // Do not infer emptiness from extracted text: image-, icon-, and math-only HTML labels still
    // have visible DOM content, and parsing here would duplicate the later measurement work.
    label.is_empty()
}

#[cfg(feature = "markdown-labels")]
pub(crate) fn flowchart_label_metrics_for_layout(
    request: FlowchartLabelMetricsRequest<'_>,
) -> crate::text::TextMetrics {
    flowchart_label_metrics_with_html_geometry(request, HtmlTextGeometryDeclarations::default())
}

#[cfg(feature = "markdown-labels")]
pub(crate) fn flowchart_label_metrics_with_html_geometry(
    request: FlowchartLabelMetricsRequest<'_>,
    geometry: HtmlTextGeometryDeclarations,
) -> crate::text::TextMetrics {
    if request.raw_label.is_empty() {
        return crate::text::TextMetrics {
            width: 0.0,
            height: 0.0,
            line_count: 0,
        };
    }
    if let Some(metrics) = flowchart_math_label_metrics_for_layout(&request) {
        return metrics;
    }
    if request.wrap_mode == WrapMode::HtmlLike {
        let source =
            flowchart_label_html_source(request.raw_label, request.label_type, request.config);
        let sanitize_config = flowchart_label_sanitize_config(request.config);
        let xhtml = merman_markup::sanitize::sanitize_xhtml(&source, &sanitize_config);
        return flowchart_html_label_metrics_with_geometry(request, &xhtml, geometry);
    }
    if request.label_type == "markdown" {
        crate::text::measure_wrapped_markdown_with_inline_styles(
            request.measurer,
            request.raw_label,
            request.style,
            request.max_width_px,
            request.wrap_mode,
        )
    } else {
        FlowchartSvgLabelSource::new(request.raw_label).metrics(
            request.measurer,
            request.style,
            request.max_width_px,
            FlowchartSvgWidthMode::Bbox,
        )
    }
}

#[cfg(feature = "markdown-labels")]
fn flowchart_math_label_metrics_for_layout(
    request: &FlowchartLabelMetricsRequest<'_>,
) -> Option<crate::text::TextMetrics> {
    if request.wrap_mode != WrapMode::HtmlLike || !request.raw_label.contains("$$") {
        return None;
    }
    let math_renderer = request.math_renderer?;
    let resolved_config;
    let config = if request.config.get_str("securityLevel").is_none() {
        let mut values = request.config.as_config().clone();
        values.set_value("securityLevel", serde_json::json!("strict"));
        let parts = merman_model::presentation::FrozenPresentationConfigParts::new(values)
            .with_explicit_config_paths(std::sync::Arc::clone(
                request.config.shared_explicit_config_paths(),
            ));
        // Flowchart's standalone measurement defaults to strict without declaring an assignment.
        // Construction validates the derived values without charging the request's model limits.
        resolved_config = PresentationConfig::try_from_frozen_parts_controlled(
            parts,
            &merman_model::resources::InputResourcePolicy::for_profile(
                merman_model::resources::ResourceProfile::UnboundedForTrustedInput,
            ),
            &merman_model::OperationControl::new(),
        )
        .ok()?
        .ok()?;
        &resolved_config
    } else {
        request.config
    };
    crate::math::math_label_metrics_for_layout(crate::math::MathLabelMetricsRequest {
        measurer: request.measurer,
        raw_label: request.raw_label,
        style: request.style,
        max_width_px: request.max_width_px,
        wrap_mode: request.wrap_mode,
        config,
        math_renderer: Some(math_renderer),
    })
}

#[cfg(feature = "markdown-labels")]
pub(crate) use merman_render_core::text::create_text_html::flowchart_label_sanitize_config;

/// Measures the exact sanitized XHTML retained for the corresponding HTML-label terminal.
#[cfg(all(test, feature = "markdown-labels"))]
pub(crate) fn flowchart_html_label_metrics_for_layout(
    request: FlowchartLabelMetricsRequest<'_>,
    xhtml: &str,
) -> crate::text::TextMetrics {
    flowchart_html_label_metrics_with_geometry(
        request,
        xhtml,
        HtmlTextGeometryDeclarations::default(),
    )
}

#[cfg(feature = "markdown-labels")]
pub(crate) fn flowchart_html_label_metrics_with_geometry(
    request: FlowchartLabelMetricsRequest<'_>,
    xhtml: &str,
    geometry: HtmlTextGeometryDeclarations,
) -> crate::text::TextMetrics {
    merman_render_core::text::html_label_layout::measure_flowchart_html_label(
        request,
        xhtml,
        geometry,
        cfg!(any(
            feature = "diagram-flowchart",
            feature = "diagram-swimlane",
            feature = "diagram-agentflow"
        )),
    )
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk", test))]
fn compute_bounds_impl<E>(
    nodes: &[LayoutNode],
    edges: &[LayoutEdge],
    mut charge: impl FnMut(usize) -> std::result::Result<(), E>,
) -> std::result::Result<Option<Bounds>, E> {
    fn include(bounds: &mut Option<Bounds>, x: f64, y: f64) {
        let Some(bounds) = bounds.as_mut() else {
            *bounds = Some(Bounds {
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            });
            return;
        };
        bounds.min_x = bounds.min_x.min(x);
        bounds.min_y = bounds.min_y.min(y);
        bounds.max_x = bounds.max_x.max(x);
        bounds.max_y = bounds.max_y.max(y);
    }

    let mut bounds = None;
    for n in nodes {
        charge(2)?;
        let hw = n.width / 2.0;
        let hh = n.height / 2.0;
        include(&mut bounds, n.x - hw, n.y - hh);
        include(&mut bounds, n.x + hw, n.y + hh);
    }
    for e in edges {
        charge(1)?;
        charge(e.points.len())?;
        for p in &e.points {
            include(&mut bounds, p.x, p.y);
        }
        if let Some(l) = &e.label {
            charge(2)?;
            let hw = l.width / 2.0;
            let hh = l.height / 2.0;
            include(&mut bounds, l.x - hw, l.y - hh);
            include(&mut bounds, l.x + hw, l.y + hh);
        }
    }
    Ok(bounds)
}

#[cfg(feature = "layout-elk")]
pub(crate) fn compute_bounds(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
    match compute_bounds_impl(nodes, edges, |_| Ok::<(), std::convert::Infallible>(())) {
        Ok(bounds) => bounds,
        Err(never) => match never {},
    }
}

#[cfg(any(feature = "layout-dagre", test))]
pub(crate) fn compute_bounds_controlled(
    nodes: &[LayoutNode],
    edges: &[LayoutEdge],
    charge: impl FnMut(usize) -> crate::Result<()>,
) -> crate::Result<Option<Bounds>> {
    compute_bounds_impl(nodes, edges, charge)
}

#[cfg(all(
    test,
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-mindmap"
    )
))]
mod tests {
    use super::*;
    use crate::math::MathRenderer;
    use crate::model::{LayoutLabel, LayoutPoint};

    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    #[test]
    fn node_minimum_excludes_circles_fixed_symbols_assets_and_empty_labels() {
        let config = crate::test_support::presentation_config(serde_json::json!({
            "flowchart": { "minNodeWidth": 120.0 }
        }));
        assert_eq!(
            flowchart_node_label_min_width("X", Some("rect"), &config),
            120.0
        );
        assert_eq!(
            flowchart_node_label_min_width("", Some("rect"), &config),
            0.0
        );
        for shape in [
            "circle",
            "circ",
            "doublecircle",
            "dbl-circ",
            "double-circle",
            "icon",
            "iconCircle",
            "iconRounded",
            "iconSquare",
            "imageSquare",
            "anchor",
            "choice",
            "cross-circ",
            "f-circ",
            "fork",
            "stop",
            "start",
            "bolt",
            "collapsedGroup",
        ] {
            assert_eq!(
                flowchart_node_label_min_width("X", Some(shape), &config),
                0.0,
                "{shape}"
            );
        }
    }

    #[derive(Debug)]
    struct PreciseMathRenderer;

    impl MathRenderer for PreciseMathRenderer {
        fn render_html_label(&self, text: &str, _config: &PresentationConfig) -> Option<String> {
            text.contains("$$").then(|| text.to_string())
        }

        fn measure_html_label(
            &self,
            text: &str,
            _config: &PresentationConfig,
            _style: &TextStyle,
            _max_width_px: Option<f64>,
            _wrap_mode: WrapMode,
        ) -> Option<TextMetrics> {
            (text.starts_with("$$") && text.ends_with("$$")).then_some(TextMetrics {
                width: 10.008,
                height: 20.008,
                line_count: 1,
            })
        }
    }

    struct PreciseTextMeasurer;

    impl TextMeasurer for PreciseTextMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 1.001,
                height: 2.002,
                line_count: 1,
            }
        }
    }

    #[derive(Default)]
    struct ImageTextMeasurer(std::cell::RefCell<Vec<String>>);

    impl TextMeasurer for ImageTextMeasurer {
        fn measure(&self, text: &str, _style: &TextStyle) -> TextMetrics {
            self.0.borrow_mut().push(text.to_string());
            TextMetrics {
                width: 1.001,
                height: 2.002,
                line_count: 1,
            }
        }

        fn measure_wrapped(
            &self,
            text: &str,
            style: &TextStyle,
            max_width: Option<f64>,
            wrap_mode: WrapMode,
        ) -> TextMetrics {
            assert_eq!(max_width, Some(120.0));
            assert_eq!(wrap_mode, WrapMode::HtmlLike);
            self.measure(text, style)
        }
    }

    fn image_label_metrics(label: &str, measurer: &dyn TextMeasurer) -> TextMetrics {
        flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer,
            raw_label: label,
            label_type: "html",
            style: &TextStyle::default(),
            max_width_px: Some(120.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &PresentationConfig::default(),
            math_renderer: None,
        })
    }

    #[test]
    fn html_image_layout_preserves_metrics_and_measurement_requests() {
        // Keep the headless fallback for images without intrinsic metadata, and only use
        // actual img elements and src attributes from the sanitized label.
        let cases: &[(&str, &[&str], usize, f64)] = &[
            ("<img src='x'>", &[], 1, 80.0),
            ("<IMG src=''>", &[], 0, 80.0),
            (
                "<span>中😀</span><IMGx src='not-an-image'><IMG src='图'>尾",
                &["中😀", "尾"],
                1,
                120.0,
            ),
            ("a<im😀>b<img src='x'>", &["ab"], 1, 120.0),
            ("a<img😀 src='x'><br中>b<img src='x'>", &["ab"], 1, 120.0),
            ("a<img>", &["a"], 0, 120.0),
            ("a<img src='  '>", &["a"], 0, 120.0),
            ("a<img src=x>", &["a"], 1, 120.0),
            ("a<img src = 'x'>", &["a"], 1, 120.0),
            (
                "<span style='min-width: 80px;'>long text</span><img src='x'>",
                &["long text"],
                1,
                120.0,
            ),
            ("a<img data-src='x'>", &["a"], 0, 120.0),
            ("a<img src='a>b'>尾", &["a", "尾"], 1, 120.0),
            (
                "<span> \tA\u{00a0} </span><BR><br /> B <img SRC=' x '> C",
                &["A\u{00a0}\n\nB", "C"],
                1,
                120.0,
            ),
            (
                "a<img src='x'>b<img src=''>c<img src='y'>d",
                &["a", "b", "c", "d"],
                2,
                120.0,
            ),
            // HTML5 recovery preserves these visible stray angle brackets as text.
            ("head<img src='x'>tail<<中", &["head", "tail<<中"], 1, 120.0),
        ];
        for &(label, expected_requests, images_with_src, width) in cases {
            let measurer = ImageTextMeasurer::default();
            let metrics = image_label_metrics(label, &measurer);
            assert_eq!(*measurer.0.borrow(), expected_requests, "{label}");
            assert_eq!(metrics.width, width, "{label}");
            let height = images_with_src as f64 * width + expected_requests.len() as f64 * 2.002;
            assert_eq!(
                metrics.height,
                crate::text::ceil_to_1_64_px(height),
                "{label}"
            );
            assert_eq!(
                metrics.line_count,
                images_with_src + expected_requests.len(),
                "{label}"
            );
        }
    }

    #[test]
    fn incomplete_image_tag_uses_final_plain_text_measurement() {
        let config = crate::test_support::presentation_config(
            serde_json::json!({ "securityLevel": "strict" }),
        );
        let html = flowchart_label_html_source("head<img", "html", &config);
        let xhtml = merman_markup::sanitize::sanitize_xhtml(&html, config.as_config());
        let document = roxmltree::Document::parse(&xhtml)
            .expect("HTML5 recovery closes the paragraph after discarding the incomplete image");
        assert!(!document.descendants().any(|node| node.has_tag_name("img")));
        assert_eq!(document.root_element().text(), Some("head"));

        let measurer = ImageTextMeasurer::default();
        let metrics = image_label_metrics("head<img", &measurer);
        assert_eq!(*measurer.0.borrow(), ["head"]);
        assert_eq!(metrics.width, 1.001);
        assert_eq!(metrics.height, 2.002);
        assert_eq!(metrics.line_count, 1);
    }

    #[test]
    fn html_image_layout_many_tags_preserves_text_and_metrics() {
        for count in [1, 32, 512, 4096] {
            let plain = format!("{}<img src='x'>", "α".repeat(count));
            let tagged = format!("{}<iMg src='x'>", "<span>α</span>".repeat(count));
            let plain_measurer = ImageTextMeasurer::default();
            let tagged_measurer = ImageTextMeasurer::default();
            let plain_metrics = image_label_metrics(&plain, &plain_measurer);
            let tagged_metrics = image_label_metrics(&tagged, &tagged_measurer);
            assert_eq!(tagged_metrics.width, plain_metrics.width, "count={count}");
            assert_eq!(tagged_metrics.height, plain_metrics.height, "count={count}");
            assert_eq!(
                tagged_metrics.line_count, plain_metrics.line_count,
                "count={count}"
            );
            assert_eq!(
                *tagged_measurer.0.borrow(),
                *plain_measurer.0.borrow(),
                "count={count}"
            );
        }
    }

    #[test]
    fn html_plain_text_discards_only_synthetic_terminal_block_breaks() {
        assert_eq!(
            flowchart_label_plain_text_for_layout("<p>approval</p>", "html", true),
            "approval"
        );
        assert_eq!(
            flowchart_label_plain_text_for_layout("<p>&nbsp;Edge&nbsp;</p>", "html", true,),
            "\u{00A0}Edge\u{00A0}"
        );
        assert_eq!(
            flowchart_label_plain_text_for_layout("<p>A<br>&nbsp;</p>", "html", true),
            "A\n\u{00A0}"
        );
        assert_eq!(
            flowchart_label_plain_text_for_layout("<p>A<br></p>", "html", true),
            "A\n"
        );
    }

    #[test]
    fn mixed_math_metrics_preserve_fragment_precision() {
        let config = PresentationConfig::default();
        let style = TextStyle::default();
        let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer: &PreciseTextMeasurer,
            raw_label: "a$$x$$b",
            label_type: "text",
            style: &style,
            max_width_px: None,
            wrap_mode: WrapMode::HtmlLike,
            config: &config,
            math_renderer: Some(&PreciseMathRenderer),
        });

        assert!((metrics.width - 12.01).abs() < 1e-12, "{metrics:?}");
        assert!((metrics.height - 20.008).abs() < 1e-12, "{metrics:?}");
    }

    #[test]
    fn strict_math_default_is_flowchart_local_and_preserves_frozen_declarations() {
        use std::sync::Arc;

        #[derive(Debug)]
        struct ConfigCheckingMathRenderer<'a> {
            original: &'a PresentationConfig,
            security: Option<&'static str>,
        }

        impl MathRenderer for ConfigCheckingMathRenderer<'_> {
            fn render_html_label(&self, _: &str, _: &PresentationConfig) -> Option<String> {
                unreachable!("the measurement callback handles this fixture")
            }

            fn measure_html_label(
                &self,
                _: &str,
                config: &PresentationConfig,
                _: &TextStyle,
                _: Option<f64>,
                _: WrapMode,
            ) -> Option<TextMetrics> {
                assert_eq!(config.get_str("securityLevel"), self.security);
                assert!(Arc::ptr_eq(
                    config.shared_explicit_config_paths(),
                    self.original.shared_explicit_config_paths(),
                ));
                assert!(config.explicit_config_owns_path("fontFamily"));
                assert_eq!(config.get_str("fontFamily"), Some("serif"));
                assert!(!config.explicit_config_owns_path("securityLevel"));
                Some(TextMetrics {
                    width: 123.5,
                    height: 20.0,
                    line_count: 1,
                })
            }
        }

        let config = crate::test_support::presentation_config(serde_json::json!({
            "fontFamily": "serif",
        }));
        let style = TextStyle::default();
        let generic =
            crate::math::math_label_metrics_for_layout(crate::math::MathLabelMetricsRequest {
                measurer: &PreciseTextMeasurer,
                raw_label: "$$x$$",
                style: &style,
                max_width_px: None,
                wrap_mode: WrapMode::HtmlLike,
                config: &config,
                math_renderer: Some(&ConfigCheckingMathRenderer {
                    original: &config,
                    security: None,
                }),
            })
            .expect("generic math keeps the caller's configuration");
        let flowchart = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer: &PreciseTextMeasurer,
            raw_label: "$$x$$",
            label_type: "text",
            style: &style,
            max_width_px: None,
            wrap_mode: WrapMode::HtmlLike,
            config: &config,
            math_renderer: Some(&ConfigCheckingMathRenderer {
                original: &config,
                security: Some("strict"),
            }),
        });
        assert_eq!(generic.width, 123.5);
        assert_eq!(flowchart.width, 123.5);
        assert_eq!(config.get_str("securityLevel"), None);
    }

    #[test]
    fn html_label_metrics_keep_nbsp_only_trailing_lines() {
        let config = PresentationConfig::default();
        let style = TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let measurer = crate::text::DeterministicTextMeasurer::default();

        for (raw_label, label_type) in [("A<br>&nbsp;", "string"), ("A\n\u{00A0}", "markdown")] {
            let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label,
                label_type,
                style: &style,
                max_width_px: None,
                wrap_mode: WrapMode::HtmlLike,
                config: &config,
                math_renderer: None,
            });

            assert_eq!(metrics.line_count, 2, "{raw_label:?}: {metrics:?}");
            assert!(
                metrics.height.is_finite() && metrics.height > style.font_size,
                "{raw_label:?}: {metrics:?}"
            );
        }
    }

    #[test]
    fn html_label_metrics_collapse_plain_paragraph_spaces_but_preserve_nbsp() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = TextStyle::default();
        let config = PresentationConfig::default();
        let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: "A&nbsp;&nbsp;B  C   D",
            label_type: "text",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &config,
            math_renderer: None,
        });
        let expected = measurer.measure_wrapped(
            "A\u{00a0}\u{00a0}B C D",
            &style,
            Some(200.0),
            WrapMode::HtmlLike,
        );
        assert_eq!(metrics.width, expected.width);
        assert_eq!(metrics.height, expected.height);
        assert_eq!(metrics.line_count, expected.line_count);
    }

    #[test]
    fn markdown_html_label_metrics_keep_the_browser_width_lattice() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = TextStyle::default();
        let config = PresentationConfig::default();
        let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: "Label",
            label_type: "markdown",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &config,
            math_renderer: None,
        });
        let expected =
            crate::text::round_to_1_64_px(measurer.measure_html_line_width_px("Label", &style));
        assert_eq!(metrics.width, expected);
    }

    #[test]
    fn markdown_html_raw_block_text_keeps_plain_measurement_precision() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = TextStyle::default();
        let config = PresentationConfig::default();
        let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: "Hello\n  - l1\n  - l2",
            label_type: "markdown",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &config,
            math_renderer: None,
        });
        let expected =
            measurer.measure_wrapped("Hello\n- l1 - l2", &style, Some(200.0), WrapMode::HtmlLike);
        assert_eq!(metrics.width, expected.width);
        assert_eq!(metrics.height, expected.height);
        assert_eq!(metrics.line_count, expected.line_count);
    }

    #[test]
    fn markdown_html_min_content_overflow_preserves_host_precision() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_family: Some("\"Recursive Variable\",arial,sans-serif".to_string()),
            font_size: 14.0,
            ..TextStyle::default()
        };
        let config = crate::test_support::presentation_config(serde_json::json!({
            "markdownAutoWrap": true,
            "securityLevel": "strict"
        }));
        let raw_label = "**angular_velocity**\n      *angular_displacement / duration*\n      [rad/s, 1/s]\n      {vector}";
        let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label,
            label_type: "markdown",
            style: &style,
            max_width_px: Some(120.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &config,
            math_renderer: None,
        });
        let expected = measurer.measure_wrapped(
            "angular_velocity\nangular_displacement / duration\n[rad/s, 1/s]\n{vector}",
            &style,
            Some(120.0),
            WrapMode::HtmlLike,
        );
        assert!(expected.width > 120.0);
        assert_eq!(metrics.width, expected.width);
        assert_eq!(metrics.height, expected.height);
        assert_eq!(metrics.line_count, expected.line_count);
    }

    #[test]
    fn markdown_html_min_content_overflow_preserves_authored_html_styles() {
        let measurer =
            crate::text::DeterministicTextMeasurer::default().with_width_callback(|text, style| {
                let advance = if style.font_weight.as_deref() == Some("700") {
                    20.0
                } else {
                    10.0
                };
                text.chars().count() as f64 * advance
            });
        let style = TextStyle::default();
        let config = crate::test_support::presentation_config(serde_json::json!({
            "securityLevel": "strict"
        }));
        let word = "abcdefghijklmnopqrstuvwx";
        for (raw_label, expected_width) in [
            (word.to_string(), 240.0),
            (format!("**{word}**"), 240.0),
            (format!("<strong>{word}</strong>"), 480.0),
            (format!("<div><strong>{word}</strong></div>"), 480.0),
        ] {
            for max_width in [40.0, 120.0] {
                let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
                    measurer: measurer.as_ref(),
                    raw_label: &raw_label,
                    label_type: "markdown",
                    style: &style,
                    max_width_px: Some(max_width),
                    wrap_mode: WrapMode::HtmlLike,
                    config: &config,
                    math_renderer: None,
                });
                assert_eq!(metrics.width, expected_width, "{raw_label}, {max_width}");
                assert_eq!(metrics.line_count, 1, "{raw_label}, {max_width}");
            }
        }
    }

    #[test]
    fn markdown_html_entity_only_root_text_keeps_plain_measurement_precision() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = TextStyle::default();
        let config = crate::test_support::presentation_config(serde_json::json!({
            "securityLevel": "strict",
            "dompurifyConfig": { "FORBID_TAGS": ["p"] }
        }));
        let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: "&amp;",
            label_type: "markdown",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &config,
            math_renderer: None,
        });
        let expected = measurer.measure_wrapped("&", &style, Some(200.0), WrapMode::HtmlLike);
        assert_eq!(metrics.width, expected.width);
        assert_eq!(metrics.line_count, expected.line_count);
    }

    #[test]
    fn markdown_html_entity_text_prevents_an_image_only_extra_line() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = TextStyle::default();
        let config = PresentationConfig::default();
        let measure = |xhtml| {
            flowchart_html_label_metrics_for_layout(
                FlowchartLabelMetricsRequest {
                    measurer: &measurer,
                    raw_label: "",
                    label_type: "markdown",
                    style: &style,
                    max_width_px: Some(200.0),
                    wrap_mode: WrapMode::HtmlLike,
                    config: &config,
                    math_renderer: None,
                },
                xhtml,
            )
        };
        let entity = measure("<p>&amp;<img src=\"x\" /></p>");
        let plain = measure("<p>X<img src=\"x\" /></p>");
        assert_eq!(entity.line_count, plain.line_count);
        assert_eq!(entity.height, plain.height);
    }

    #[test]
    fn non_markdown_html_metrics_share_create_text_explicit_line_breaks() {
        let config = PresentationConfig::default();
        let style = TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let measurer = crate::text::DeterministicTextMeasurer::default();

        for (raw_label, expected_lines) in [
            ("第一行\n第二行", 2),
            ("first\\nsecond\\nthird", 3),
            ("A<br><br>B", 3),
            ("<br>", 1),
            ("<br><br>", 2),
        ] {
            let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label,
                label_type: "text",
                style: &style,
                max_width_px: Some(200.0),
                wrap_mode: WrapMode::HtmlLike,
                config: &config,
                math_renderer: None,
            });
            assert_eq!(
                metrics.line_count, expected_lines,
                "{raw_label:?}: {metrics:?}"
            );
            assert_eq!(
                metrics.height,
                expected_lines as f64 * 24.0,
                "{raw_label:?}: {metrics:?}"
            );
        }
    }

    #[test]
    fn svg_non_markdown_closing_br_uses_the_sanitized_break_semantics() {
        let text = flowchart_label_plain_text_for_layout(
            "`This is **bold** </br>and <strong>strong</strong>`",
            "text",
            false,
        );

        assert_eq!(text, "`This is **bold**\nand <strong> strong </strong> `");
        assert!(!text.contains("</br>"));
    }

    #[test]
    fn svg_long_word_wrapping_preserves_grapheme_clusters() {
        struct ScalarWidthMeasurer;

        impl TextMeasurer for ScalarWidthMeasurer {
            fn measure(&self, text: &str, _style: &TextStyle) -> TextMetrics {
                TextMetrics {
                    width: text.chars().count() as f64 * 10.0,
                    height: 10.0,
                    line_count: 1,
                }
            }
        }

        let source = vec![vec!["e\u{0301}x".to_string(), "👨‍👩‍👧‍👦y".to_string()]];
        let wrapped = flowchart_wrap_svg_source_word_lines(
            &ScalarWidthMeasurer,
            &source,
            &TextStyle::default(),
            Some(5.0),
            true,
        );

        assert_eq!(
            wrapped,
            vec![
                vec!["e\u{0301}".to_string()],
                vec!["x".to_string()],
                vec!["👨‍👩‍👧‍👦".to_string()],
                vec!["y".to_string()],
            ]
        );
    }

    #[test]
    fn svg_word_wrapping_uses_computed_text_length_instead_of_bbox_width() {
        struct DivergentSvgWidthMeasurer;

        impl TextMeasurer for DivergentSvgWidthMeasurer {
            fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
                TextMetrics {
                    width: 100.0,
                    height: 10.0,
                    line_count: 1,
                }
            }

            fn measure_svg_text_computed_length_px(&self, text: &str, _style: &TextStyle) -> f64 {
                text.chars().count() as f64 * 10.0
            }
        }

        let source = vec![vec!["ab".to_string(), "cd".to_string()]];
        let wrapped = flowchart_wrap_svg_source_word_lines(
            &DivergentSvgWidthMeasurer,
            &source,
            &TextStyle::default(),
            Some(50.0),
            true,
        );

        assert_eq!(wrapped, source);
    }

    #[test]
    fn svg_word_wrapping_preserves_opaque_measurer_request_order() {
        struct RecordingMeasurer {
            calls: std::cell::RefCell<Vec<String>>,
        }

        impl TextMeasurer for RecordingMeasurer {
            fn measure(&self, text: &str, _style: &TextStyle) -> TextMetrics {
                TextMetrics {
                    width: text.chars().count() as f64 * 10.0,
                    height: 10.0,
                    line_count: 1,
                }
            }

            fn measure_svg_text_computed_length_px(&self, text: &str, _style: &TextStyle) -> f64 {
                self.calls.borrow_mut().push(text.to_string());
                text.chars().count() as f64 * 10.0
            }
        }

        let measurer = RecordingMeasurer {
            calls: std::cell::RefCell::new(Vec::new()),
        };
        let source = vec![vec!["aa".to_string(), "bb".to_string(), "cc".to_string()]];
        let wrapped = flowchart_wrap_svg_source_word_lines(
            &measurer,
            &source,
            &TextStyle::default(),
            Some(25.0),
            true,
        );

        assert_eq!(
            wrapped,
            vec![
                vec!["aa".to_string()],
                vec!["bb".to_string()],
                vec!["cc".to_string()],
            ]
        );
        assert_eq!(
            measurer.calls.into_inner(),
            ["aa bb cc", "aa", "aa bb", "bb", "bb cc", "cc"]
        );

        let measurer = RecordingMeasurer {
            calls: std::cell::RefCell::new(Vec::new()),
        };
        let wrapped = flowchart_wrap_svg_source_word_lines(
            &measurer,
            &[vec!["abcd".to_string()]],
            &TextStyle::default(),
            Some(15.0),
            true,
        );
        assert_eq!(
            wrapped,
            ["a", "b", "c", "d"]
                .into_iter()
                .map(|word| vec![word.to_string()])
                .collect::<Vec<_>>()
        );
        assert_eq!(
            measurer.calls.into_inner(),
            [
                "abcd", "abcd", "a", "ab", "bcd", "b", "bc", "cd", "c", "cd", "d"
            ]
        );
    }

    #[test]
    fn svg_word_wrapping_stops_when_the_measurement_operation_is_cancelled() {
        struct CancellingMeasurer {
            calls: std::cell::Cell<usize>,
            cancelled: std::cell::Cell<bool>,
        }

        impl TextMeasurer for CancellingMeasurer {
            fn cancellation_requested(&self) -> bool {
                self.cancelled.get()
            }

            fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
                TextMetrics {
                    width: 1.0,
                    height: 10.0,
                    line_count: 1,
                }
            }

            fn measure_svg_text_computed_length_px(&self, text: &str, _style: &TextStyle) -> f64 {
                let calls = self.calls.get().saturating_add(1);
                self.calls.set(calls);
                self.cancelled.set(true);
                text.chars().count() as f64
            }
        }

        let measurer = CancellingMeasurer {
            calls: std::cell::Cell::new(0),
            cancelled: std::cell::Cell::new(false),
        };
        let wrapped = flowchart_wrap_svg_source_word_lines(
            &measurer,
            &[vec!["a".repeat(100_000)]],
            &TextStyle::default(),
            Some(1.0),
            true,
        );

        assert!(wrapped.is_empty());
        assert_eq!(measurer.calls.get(), 1);
    }

    #[test]
    fn empty_flowchart_labels_have_zero_layout_metrics_in_both_render_modes() {
        let config = PresentationConfig::default();
        let style = TextStyle::default();
        let measurer = PreciseTextMeasurer;

        for wrap_mode in [WrapMode::HtmlLike, WrapMode::SvgLike] {
            for label_type in ["text", "string", "markdown"] {
                let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
                    measurer: &measurer,
                    raw_label: "",
                    label_type,
                    style: &style,
                    max_width_px: None,
                    wrap_mode,
                    config: &config,
                    math_renderer: None,
                });

                assert_eq!(metrics.width, 0.0, "{wrap_mode:?} {label_type}");
                assert_eq!(metrics.height, 0.0, "{wrap_mode:?} {label_type}");
                assert_eq!(metrics.line_count, 0, "{wrap_mode:?} {label_type}");
            }
        }
    }

    #[test]
    fn svg_label_metrics_apply_create_text_nbsp_rules_by_label_type() {
        let config = PresentationConfig::default();
        let style = TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let measure = |raw_label: &str, label_type: &str| {
            flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label,
                label_type,
                style: &style,
                max_width_px: Some(200.0),
                wrap_mode: WrapMode::SvgLike,
                config: &config,
                math_renderer: None,
            })
        };

        let text = measure("\u{00A0}", "text");
        assert_eq!(text.width, 0.0, "{text:?}");
        assert_eq!(text.height, 0.0, "{text:?}");
        assert_eq!(text.line_count, 0, "{text:?}");

        let markdown = measure("\u{00A0}", "markdown");
        assert!(markdown.width > 0.0, "{markdown:?}");
        assert!(markdown.height > 0.0, "{markdown:?}");
        assert_eq!(markdown.line_count, 1, "{markdown:?}");

        let entity = measure("&nbsp;", "text");
        assert!(entity.width > markdown.width, "{entity:?} {markdown:?}");
        assert!(entity.height > 0.0, "{entity:?}");
    }

    #[test]
    fn controlled_bounds_streams_geometry_and_charges_each_visited_item() {
        let nodes = vec![LayoutNode {
            id: "node".to_string(),
            x: 10.0,
            y: 20.0,
            width: 8.0,
            height: 6.0,
            is_cluster: false,
            label_width: None,
            label_height: None,
        }];
        let edges = vec![LayoutEdge {
            id: "edge".to_string(),
            from: "node".to_string(),
            to: "node".to_string(),
            from_cluster: None,
            to_cluster: None,
            points: vec![
                LayoutPoint { x: -5.0, y: 1.0 },
                LayoutPoint { x: 7.0, y: 30.0 },
                LayoutPoint { x: 14.0, y: 18.0 },
            ],
            label: Some(LayoutLabel {
                x: 4.0,
                y: 5.0,
                width: 4.0,
                height: 2.0,
            }),
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        }];
        let mut tranches = Vec::new();

        let bounds = compute_bounds_controlled(&nodes, &edges, |units| {
            tranches.push(units);
            Ok(())
        })
        .expect("the accounting callback accepts every tranche")
        .expect("the geometry is non-empty");

        assert_eq!(tranches, vec![2, 1, 3, 2]);
        assert_eq!(tranches.iter().sum::<usize>(), 8);
        assert_eq!(
            bounds,
            Bounds {
                min_x: -5.0,
                min_y: 1.0,
                max_x: 14.0,
                max_y: 30.0,
            }
        );
    }
}
