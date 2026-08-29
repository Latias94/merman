use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use merman_render::environment::{RenderEnvironment, TextMeasurementPhase};
use merman_render::text::{
    DeterministicTextMeasurer, TextMeasurer, TextStyle, WrapMode, measure_html_with_inline_styles,
};
use std::hint::black_box;

const FLOWCHART_FONT_FAMILY: &str = "\"trebuchet ms\", verdana, arial, sans-serif";

const TEXT_CASES: &[(&str, &str, Option<f64>)] = &[
    ("node_label", "Node Label", Some(200.0)),
    ("edge_label", "Edge Label", Some(200.0)),
    ("subgraph_title", "Subgraph Title", Some(200.0)),
    (
        "wrapped_cluster_title",
        "A very long cluster title with punctuation: (a/b/c)",
        Some(120.0),
    ),
    ("special_characters", "special characters", Some(200.0)),
];

fn flowchart_style(font_weight: Option<&str>) -> TextStyle {
    TextStyle {
        font_family: Some(FLOWCHART_FONT_FAMILY.to_string()),
        font_size: 16.0,
        font_weight: font_weight.map(str::to_string),
        font_style: None,
    }
}

fn rich_inline_html(visible_bytes: usize, run_count: usize, break_count: usize) -> String {
    assert!(visible_bytes > 0);
    assert!((1..=visible_bytes).contains(&run_count));
    assert!(break_count < visible_bytes);

    let mut text = vec![b'a'; visible_bytes];
    for index in 0..break_count {
        let offset = (index + 1) * visible_bytes / (break_count + 1);
        text[offset] = b' ';
    }

    let mut html = String::with_capacity(visible_bytes + run_count * 16);
    let mut start = 0usize;
    for index in 0..run_count {
        let end = (index + 1) * visible_bytes / run_count;
        let fragment = std::str::from_utf8(&text[start..end]).expect("ASCII benchmark input");
        match index % 4 {
            0 => html.push_str(fragment),
            1 => {
                html.push_str("<strong>");
                html.push_str(fragment);
                html.push_str("</strong>");
            }
            2 => {
                html.push_str("<em>");
                html.push_str(fragment);
                html.push_str("</em>");
            }
            _ => {
                html.push_str("<code>");
                html.push_str(fragment);
                html.push_str("</code>");
            }
        }
        start = end;
    }
    html
}

fn bench_text_measure_stress(c: &mut Criterion) {
    let measurer = DeterministicTextMeasurer::default();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("deterministic benchmark session");
    let inline_measurer = session.text_measurer(TextMeasurementPhase::Wrap);
    let styles = [
        ("plain", flowchart_style(None)),
        ("bold", flowchart_style(Some("bold"))),
    ];

    let mut group = c.benchmark_group("text_measure_stress");
    group.sample_size(50);

    for (style_name, style) in &styles {
        for &(case_name, text, wrap_width) in TEXT_CASES {
            group.bench_function(
                BenchmarkId::new(format!("computed_length_{style_name}"), case_name),
                |b| {
                    b.iter(|| {
                        black_box(measurer.measure_svg_text_computed_length_px(
                            black_box(text),
                            black_box(style),
                        ));
                    });
                },
            );

            group.bench_function(
                BenchmarkId::new(format!("wrapped_svg_like_{style_name}"), case_name),
                |b| {
                    b.iter(|| {
                        black_box(measurer.measure_wrapped(
                            black_box(text),
                            black_box(style),
                            black_box(wrap_width),
                            WrapMode::SvgLike,
                        ));
                    });
                },
            );
        }
    }

    let style = flowchart_style(None);
    const FIXED_VISIBLE_BYTES: usize = 4096;
    for run_count in [1, 32, 256] {
        for break_count in [0, 31, 255] {
            let html = rich_inline_html(FIXED_VISIBLE_BYTES, run_count, break_count);
            group.bench_with_input(
                BenchmarkId::new(
                    "rich_inline_fixed_bytes",
                    format!("r{run_count}_k{break_count}"),
                ),
                &html,
                |b, html| {
                    b.iter(|| {
                        black_box(measure_html_with_inline_styles(
                            black_box(&measurer),
                            black_box(html),
                            black_box(&style),
                            black_box(Some(180.0)),
                            WrapMode::HtmlLike,
                        ));
                    });
                },
            );
        }
    }

    for run_count in [1, 32, 128] {
        for segment_count in [16, 32, 64, 128] {
            let html = rich_inline_html(FIXED_VISIBLE_BYTES, run_count, segment_count - 1);
            let natural_width = measure_html_with_inline_styles(
                &inline_measurer,
                &html,
                &style,
                None,
                WrapMode::HtmlLike,
            )
            .width;
            // The public width is quantized to 1/64 px. Subtract a full pixel so the internal raw
            // natural width is guaranteed to exceed the benchmark limit instead of accidentally
            // taking the no-wrap fast path after rounding.
            let max_width = (natural_width - 1.0).max(1.0);
            let active_line_probe = measure_html_with_inline_styles(
                &inline_measurer,
                &html,
                &style,
                Some(max_width),
                WrapMode::HtmlLike,
            );
            assert!(
                active_line_probe.line_count > 1,
                "active-line benchmark must exercise rollback and wrapping"
            );
            group.bench_with_input(
                BenchmarkId::new(
                    "rich_inline_active_line_matrix",
                    format!("r{run_count}_k{segment_count}"),
                ),
                &html,
                |b, html| {
                    b.iter(|| {
                        black_box(measure_html_with_inline_styles(
                            black_box(&inline_measurer),
                            black_box(html),
                            black_box(&style),
                            black_box(Some(max_width)),
                            WrapMode::HtmlLike,
                        ));
                    });
                },
            );
        }
    }

    for run_count in [1, 16, 64, 256, 1024] {
        let html = rich_inline_html(FIXED_VISIBLE_BYTES, run_count, 31);
        group.bench_with_input(
            BenchmarkId::new("rich_inline_run_curve", run_count),
            &html,
            |b, html| {
                b.iter(|| {
                    black_box(measure_html_with_inline_styles(
                        black_box(&measurer),
                        black_box(html),
                        black_box(&style),
                        black_box(Some(180.0)),
                        WrapMode::HtmlLike,
                    ));
                });
            },
        );
    }

    for break_count in [0, 15, 63, 255, 1023] {
        let html = rich_inline_html(FIXED_VISIBLE_BYTES, 32, break_count);
        group.bench_with_input(
            BenchmarkId::new("rich_inline_break_curve", break_count),
            &html,
            |b, html| {
                b.iter(|| {
                    black_box(measure_html_with_inline_styles(
                        black_box(&measurer),
                        black_box(html),
                        black_box(&style),
                        black_box(Some(180.0)),
                        WrapMode::HtmlLike,
                    ));
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_text_measure_stress);
criterion_main!(benches);
