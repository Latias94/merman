use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::GanttDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

fn layout_gantt_from_text(text: &str) -> GanttDiagramLayout {
    layout_gantt_from_text_at_container_width(text, LayoutOptions::default().container_width)
}

fn layout_gantt_from_text_at_container_width(
    text: &str,
    container_width: f64,
) -> GanttDiagramLayout {
    let environment = RenderEnvironment::deterministic();
    layout_gantt_from_text_with_environment(text, container_width, &environment)
}

fn layout_gantt_from_text_with_environment(
    text: &str,
    container_width: f64,
    environment: &RenderEnvironment,
) -> GanttDiagramLayout {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let mut options = LayoutOptions::default();
    options.container_width = container_width;
    let session = environment.begin_session().expect("render session");
    let artifact = family::prepare(&parsed, &options, session).expect("layout ok");
    let projection = artifact.layout_json().expect("Gantt layout projection");
    serde_json::from_value(projection["layout"]["GanttDiagram"].clone()).expect("Gantt layout")
}

#[test]
fn gantt_layout_uses_the_operation_container_width_unless_config_overrides_it() {
    let source = "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: 2024-01-01, 1d";
    let narrow = layout_gantt_from_text_at_container_width(source, 640.0);
    let wide = layout_gantt_from_text_at_container_width(source, 960.0);

    assert_eq!(narrow.width, 640.0);
    assert_eq!(wide.width, 960.0);

    let configured = layout_gantt_from_text_at_container_width(
        "---\nconfig:\n  gantt:\n    useWidth: 420\n---\ngantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: 2024-01-01, 1d",
        960.0,
    );
    assert_eq!(configured.width, 420.0);
}

#[test]
fn gantt_axis_tick_lines_retain_current_color_fallback() {
    let svg = render_gantt_svg_from_text(
        "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nPlan: plan, 2024-01-01, 2d\n",
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Gantt SVG XML");
    let tick_lines = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("line")
                && node.ancestors().any(|ancestor| {
                    ancestor.has_tag_name("g") && ancestor.attribute("class") == Some("tick")
                })
        })
        .collect::<Vec<_>>();
    assert!(
        !tick_lines.is_empty(),
        "Gantt fixture must emit axis tick lines"
    );
    assert!(
        tick_lines
            .iter()
            .all(|line| line.attribute("stroke") == Some("currentColor")),
        "axis tick lines must retain Mermaid's currentColor fallback"
    );
    let stylesheet = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Gantt SVG must include its scoped stylesheet");
    assert!(
        stylesheet.contains(".grid .tick{stroke:"),
        "the scoped stylesheet must remain the Gantt grid stroke owner"
    );
}

#[test]
fn gantt_section_font_attributes_preserve_css() {
    for (root, raw) in [
        (24, "0em"),
        (24, "24px"),
        (24, "24pt"),
        (24, "2em"),
        (0, "2em"),
        (0, "50%"),
        (0, "inherit"),
        (0, "11px"),
        (0, ".5px"),
        (24, "-1px"),
        (0, "-1px"),
    ] {
        let config = serde_json::json!({
            "themeVariables": {"fontSize": format!("{root}px")},
            "gantt": {"fontSize": 0, "sectionFontSize": raw, "tickInterval": "1month"}
        });
        let source = format!(
            "---\nconfig: {config}\n---\ngantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Work\nShip: a, 2024-01-02, 1d\n"
        );
        let svg = render_gantt_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let section = document
            .descendants()
            .find(|node| node.attribute("class") == Some("sectionTitle sectionTitle0"))
            .unwrap();
        assert_eq!(
            section.attribute("font-size"),
            Some(raw),
            "root={root}, {raw}"
        );
    }
}

struct GanttUsedFontSizeMeasurer {
    sizes: Arc<Mutex<Vec<f64>>>,
}

impl TextMeasurer for GanttUsedFontSizeMeasurer {
    fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
        panic!("Gantt labels must use the raw SVG bbox measurement")
    }

    fn measure_svg_raw_text_bbox_width_px(&self, _text: &str, style: &TextStyle) -> f64 {
        self.sizes.lock().unwrap().push(style.font_size);
        style.font_size
    }
}

#[test]
fn gantt_task_font_attributes_share_used_sizes_with_measurement() {
    for (root, value, raw, used) in [
        (24, serde_json::json!(-1), "-1", 24.0),
        (0, serde_json::json!(-1), "-1", 0.0),
        (24, serde_json::json!("-1"), "-1", 24.0),
        (0, serde_json::json!("-1"), "-1", 0.0),
        (24, serde_json::json!(0), "0", 0.0),
        (0, serde_json::json!("13"), "13", 13.0),
        (24, serde_json::json!("-1em"), "11", 11.0),
        (0, serde_json::json!("-1em"), "11", 11.0),
        (24, serde_json::json!("0em"), "11", 11.0),
        (0, serde_json::json!(".5px"), "11", 11.0),
        (0, serde_json::json!("24pt"), "11", 11.0),
        (24, serde_json::json!("2em"), "11", 11.0),
        (0, serde_json::json!("2em"), "11", 11.0),
        (24, serde_json::json!("50%"), "11", 11.0),
        (24, serde_json::json!("1rem"), "11", 11.0),
    ] {
        let config = serde_json::json!({
            "themeVariables": {"fontSize": format!("{root}px")},
            "gantt": {"fontSize": value, "sectionFontSize": 0, "tickInterval": "1month"}
        });
        let source = format!(
            "---\nconfig: {config}\n---\ngantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nShip: a, 2024-01-02, 1d\n"
        );
        let sizes = Arc::new(Mutex::new(Vec::new()));
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.gantt-used-font-size").unwrap(),
                "v1",
            )
            .unwrap(),
            Arc::new(GanttUsedFontSizeMeasurer {
                sizes: Arc::clone(&sizes),
            }),
        );
        let environment = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
            .unwrap()
            .unwrap()
            .into_presentation();
        let rendered = family::prepare(
            &parsed,
            &LayoutOptions::default(),
            environment.begin_session().unwrap(),
        )
        .unwrap()
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("gantt-used".to_owned()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap();
        assert_eq!(*sizes.lock().unwrap(), vec![used], "root={root}, {raw}");
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let label = document
            .descendants()
            .find(|node| node.attribute("id") == Some("gantt-used-a-text"))
            .unwrap();
        assert_eq!(label.attribute("font-size"), Some(raw));
        let expected_y = match raw {
            "-1" => "57.5",
            "0" => "58",
            "13" => "64.5",
            "11" => "63.5",
            _ => unreachable!(),
        };
        assert_eq!(label.attribute("y"), Some(expected_y));
    }
}

struct RawBBoxProbeMeasurer {
    calls: Arc<AtomicUsize>,
    width: f64,
}

impl TextMeasurer for RawBBoxProbeMeasurer {
    fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
        panic!("Gantt task labels must use the raw SVG text bbox operation")
    }

    fn measure_svg_raw_text_bbox_width_px(&self, _text: &str, _style: &TextStyle) -> f64 {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.width
    }
}

#[test]
fn gantt_task_labels_route_through_raw_svg_bbox_measurement() {
    let calls = Arc::new(AtomicUsize::new(0));
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.gantt-raw-bbox").unwrap(),
            "v1",
        )
        .unwrap(),
        Arc::new(RawBBoxProbeMeasurer {
            calls: Arc::clone(&calls),
            width: 200.0,
        }),
    );
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    let layout = layout_gantt_from_text_with_environment(
        "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: task, 2024-01-01, 1d",
        1_184.0,
        &environment,
    );

    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(layout.tasks[0].label.width, 200.0);
}

#[test]
fn gantt_label_placement_uses_the_resolved_container_edges() {
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.gantt-raw-bbox-placement").unwrap(),
            "v1",
        )
        .unwrap(),
        Arc::new(RawBBoxProbeMeasurer {
            calls: Arc::new(AtomicUsize::new(0)),
            width: 200.0,
        }),
    );
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    let layout = layout_gantt_from_text_with_environment(
        "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nFull range: full, 2024-01-01, 10d\nStart label: start, 2024-01-01, 1d\nEnd label: end, 2024-01-10, 1d",
        1_184.0,
        &environment,
    );
    let start = layout
        .tasks
        .iter()
        .find(|task| task.id == "start")
        .expect("start task");
    let end = layout
        .tasks
        .iter()
        .find(|task| task.id == "end")
        .expect("end task");

    assert!(start.label.class.contains("taskTextOutsideRight"));
    assert!(start.label.x > start.bar.x + start.bar.width);
    assert!(end.label.class.contains("taskTextOutsideLeft"));
    assert_eq!(end.label.x, end.bar.x - 5.0);
}

#[test]
fn gantt_layout_stops_at_the_maximum_utc_date_without_panicking() {
    // The raw model boundary case lives beside the private layout entry point.
}

fn render_gantt_svg_from_text(text: &str) -> String {
    render_gantt_svg_from_text_with_engine(MermaidEngine::new(), text)
}

#[test]
fn gantt_tasks_after_directives_reach_layout_and_svg_in_source_order() {
    let same_line = concat!(
        "gantt\r\n",
        "dateFormat YYYY-MM-DD;Build:build,2026-01-01,1d\r\n",
        "axisFormat %d#Ship:ship,after build,1d\r\n",
        "todayMarker off;Close:close,after ship,1d\r\n",
    );
    let cross_line = concat!(
        "gantt\r\n",
        "dateFormat\r\nYYYY-MM-DD;Build:build,2026-01-01,1d\r\n",
        "\u{feff}axisFormat\r\n%d#Ship:ship,after build,1d\r\n",
        "todayMarker\r\noff;Close:close,after ship,1d\r\n",
    );
    for source in [same_line, cross_line] {
        let layout = layout_gantt_from_text(source);
        assert_eq!(layout.tasks.len(), 3);
        let svg = render_gantt_svg_from_text(source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let labels: Vec<_> = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("text")
                    && node
                        .attribute("class")
                        .is_some_and(|class| class.starts_with("taskText"))
            })
            .filter_map(|node| node.text())
            .collect();
        assert_eq!(labels, [";Build", "#Ship", ";Close"]);
        for id in ["build", "ship", "close"] {
            let terminal_id = format!("gantt-config-{id}");
            assert!(
                document.descendants().any(|node| node.has_tag_name("rect")
                    && node.attribute("id") == Some(terminal_id.as_str())),
                "{id}"
            );
        }
    }
}

fn render_gantt_svg_from_text_with_engine(engine: MermaidEngine, text: &str) -> String {
    let session = RenderEnvironment::deterministic()
        .with_runtime_policy(
            merman_model::runtime::RuntimePolicy::deterministic()
                .with_fixed_unix_millis(1_704_067_200_000),
        )
        .begin_session()
        .expect("begin render session");
    let parsed = futures::executor::block_on(
        engine.parse_diagram_for_render_model(text, ParseOptions::default()),
    )
    .expect("parse ok")
    .expect("diagram detected")
    .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).expect("layout ok");
    artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("gantt-config".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render svg")
        .svg()
        .to_owned()
}

fn try_render_gantt_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = RenderEnvironment::deterministic()
        .with_runtime_policy(
            merman_model::runtime::RuntimePolicy::deterministic()
                .with_fixed_unix_millis(1_704_067_200_000),
        )
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Gantt resource-bound session");
    let parsed = futures::executor::block_on(
        MermaidEngine::new().parse_diagram_for_render_model(text, ParseOptions::default()),
    )
    .expect("parse Gantt resource-bound fixture")
    .expect("detect Gantt resource-bound fixture")
    .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("gantt-bounded".to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn gantt_viewport_uses_its_own_max_width_setting() {
    for use_max_width in [false, true] {
        let source = format!(
            "---\nconfig:\n  state:\n    useMaxWidth: {}\n  gantt:\n    useMaxWidth: {use_max_width}\n    useWidth: 600.5\n---\ngantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nTask : task, 2026-01-01, 2d\n",
            !use_max_width,
        );
        let svg = render_gantt_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let root = document.root_element();
        let view_box = root
            .attribute("viewBox")
            .unwrap()
            .split_ascii_whitespace()
            .map(|number| number.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(view_box[2], 600.5);
        if use_max_width {
            assert_eq!(root.attribute("width"), Some("100%"));
            assert_eq!(root.attribute("height"), None);
            assert_eq!(root.attribute("style"), Some("max-width: 600.5px;"));
        } else {
            for (attribute, expected) in [("width", view_box[2]), ("height", view_box[3])] {
                assert_eq!(
                    root.attribute(attribute).unwrap().parse::<f64>().unwrap(),
                    expected,
                );
            }
            assert_eq!(root.attribute("style"), None);
        }
    }
}

#[test]
fn gantt_native_title_bounds_use_drawn_typography_without_resizing_tasks() {
    use merman_render::text::DeterministicTextMeasurer;
    use merman_render::theme::{NativeTheme, ThemeCompiler};

    const TITLE: &str = "Milestones and delivery schedule";

    struct TitleMeasurer;

    impl TextMeasurer for TitleMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            DeterministicTextMeasurer::default().measure(text, style)
        }

        // The default raw width probe delegates to the title probe.
        fn measure_svg_raw_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
            DeterministicTextMeasurer::default().measure_svg_raw_text_bbox_width_px(text, style)
        }

        fn measure_svg_title_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
            assert_eq!(text, TITLE);
            assert_eq!(
                style.font_family.as_deref(),
                Some(r#""Cousine", monospace"#)
            );
            assert_eq!(style.font_weight.as_deref(), Some("700"));
            let half_width = text.len() as f64 * style.font_size * 0.3;
            (half_width, half_width)
        }

        fn measure_svg_raw_text_bbox_y_offset_px(&self, _: &str, style: &TextStyle) -> Option<f64> {
            Some(-0.9 * style.font_size)
        }

        fn measure_svg_raw_text_bbox_height_px(&self, _: &str, style: &TextStyle) -> f64 {
            style.font_size
        }
    }

    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.gantt-native-title").unwrap(),
            "v1",
        )
        .unwrap(),
        Arc::new(TitleMeasurer),
    );
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    let source = format!(
        "gantt\ntitle {TITLE}\ndateFormat YYYY-MM-DD\ntodayMarker off\nShip : ship, 2024-01-01, 2d\n"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let mut options = LayoutOptions::default();
    options.container_width = 400.0;

    for (base_size, view_box, max_width) in [
        (None, "0 0 400 124", "max-width: 400px;"),
        (Some(14), "0 0 400 124", "max-width: 400px;"),
        (Some(24), "-88 -2 576 126", "max-width: 576px;"),
    ] {
        let session = if let Some(base_size) = base_size {
            let theme = serde_json::json!({
                "schema_version": 2,
                "extends": "studio",
                "typography": {
                    "base_size_px": base_size,
                    "mono": ["Cousine", "monospace"],
                    "title_family": "mono",
                    "title_weight": 700
                }
            });
            let theme = ThemeCompiler::new()
                .compile_json(theme.to_string().as_bytes(), None)
                .unwrap();
            environment.begin_session_with_native_theme(NativeTheme::new(theme))
        } else {
            environment.begin_session()
        }
        .unwrap();
        let artifact = family::prepare(parsed.presentation(), &options, session).unwrap();
        let svg = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(svg.svg()).unwrap();
        let root = document.root_element();
        assert_eq!(root.attribute("viewBox"), Some(view_box));
        assert_eq!(root.attribute("style"), Some(max_width));
        let css = root
            .children()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        if let Some(base_size) = base_size {
            let size = f64::from(base_size) * 1.25;
            assert!(css.contains(&format!(
                r#".titleText{{text-anchor:middle;font-family:"Cousine", monospace;font-size:{size}px;font-weight:700;"#
            )));
        } else {
            assert!(css.contains(".titleText{text-anchor:middle;font-size:18px;"));
        }
        let title = document
            .descendants()
            .find(|node| node.attribute("class") == Some("titleText"))
            .unwrap();
        assert_eq!(title.text(), Some(TITLE));
        assert_eq!(title.attribute("x"), Some("200"));
        assert_eq!(title.attribute("y"), Some("25"));
        let task = document
            .descendants()
            .find(|node| node.attribute("id") == Some("merman-ship"))
            .unwrap();
        assert_eq!(task.attribute("x"), Some("75"));
        assert_eq!(task.attribute("width"), Some("250"));
    }
}

#[test]
fn gantt_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"gantt
dateFormat YYYY-MM-DD
todayMarker off
section Delivery
Plan: plan, 2024-01-01, 2d
Ship: ship, after plan, 1d
"#;
    let baseline = try_render_gantt_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Gantt baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Gantt fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Gantt SVG byte ceiling");
    let exact = try_render_gantt_svg_with_resource_policy(source, exact_policy)
        .expect("the exact Gantt family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Gantt SVG byte ceiling");
    let error = try_render_gantt_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the Gantt family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Gantt MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
    assert!(limit.explicit_overrides.iter().any(|resource_override| {
        resource_override.id == ResourceLimitId::MaxSvgBytes
            && resource_override.value == below_exact
    }));
}

#[test]
fn gantt_section_titles_share_mermaid_break_rows_and_vertical_alignment() {
    let source = include_str!("../../../fixtures/gantt/multiline_section_title_br_tags.mmd");
    let layout = layout_gantt_from_text(source);
    let section = &layout.section_titles[0];
    let expected = ["Line1", "Line2", "Line3", "Line4", "Line5"];
    assert_eq!(section.lines, expected);
    assert_eq!(section.dy_em, -2.0);

    let svg = render_gantt_svg_from_text(source);
    let document = roxmltree::Document::parse(&svg).expect("valid Gantt SVG XML");
    let title = document
        .descendants()
        .find(|node| node.attribute("class") == Some("sectionTitle sectionTitle0"))
        .expect("section title");
    assert_eq!(title.attribute("dy"), Some("-2em"));
    let rows: Vec<_> = title
        .children()
        .filter(|node| node.has_tag_name("tspan"))
        .collect();
    assert_eq!(
        rows.iter()
            .map(|node| node.text().unwrap_or_default())
            .collect::<Vec<_>>(),
        expected
    );
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.attribute("dy"), (index > 0).then_some("1em"));
    }
}

#[test]
fn gantt_task_text_height_follows_final_svg_security_sanitization() {
    let source = r#"gantt
dateFormat YYYY-MM-DD
section Delivery
Task: task, 2024-01-01, 1d
"#;
    let strict = render_gantt_svg_from_text(source);
    let loose = render_gantt_svg_from_text_with_engine(
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "securityLevel": "loose"
            })),
        ),
        source,
    );

    let task_text_heights = |svg: &str| {
        let document = roxmltree::Document::parse(svg).expect("valid Gantt SVG XML");
        document
            .descendants()
            .filter(|node| node.has_tag_name("text") && node.attribute("id").is_some())
            .filter_map(|node| node.attribute("text-height").map(str::to_owned))
            .collect::<Vec<_>>()
    };

    assert!(task_text_heights(&strict).is_empty(), "{strict}");
    assert_eq!(task_text_heights(&loose), vec!["20"], "{loose}");
}

#[test]
fn gantt_frontmatter_title_renders_unless_the_body_overrides_it() {
    let frontmatter_svg = render_gantt_svg_from_text(
        r#"---
title: Frontmatter schedule
---
gantt
dateFormat YYYY-MM-DD
section Delivery
Task: 2024-01-01, 1d
"#,
    );
    assert!(
        frontmatter_svg.contains(r#"class="titleText">Frontmatter schedule</text>"#),
        "frontmatter title should render when the Gantt body has none: {frontmatter_svg}"
    );

    let body_svg = render_gantt_svg_from_text(
        r#"---
title: Frontmatter schedule
---
gantt
title Body schedule
dateFormat YYYY-MM-DD
section Delivery
Task: 2024-01-01, 1d
"#,
    );
    assert!(body_svg.contains(r#"class="titleText">Body schedule</text>"#));
    assert!(!body_svg.contains(">Frontmatter schedule</text>"));
}

#[test]
fn gantt_explicit_whitespace_title_overrides_frontmatter_without_trimming() {
    let svg = render_gantt_svg_from_text(concat!(
        "---\n",
        "title: Frontmatter schedule\n",
        "---\n",
        "gantt\n",
        "title  \n",
        "dateFormat YYYY-MM-DD\n",
        "section Delivery\n",
        "Task: 2024-01-01, 1d\n",
    ));

    assert!(
        svg.contains(r#"class="titleText"> </text>"#),
        "the one remaining Jison separator must be rendered exactly: {svg}"
    );
    assert!(!svg.contains(">Frontmatter schedule</text>"));
}

#[test]
fn gantt_svg_tick_interval_uses_config_fallback_and_body_precedence_in_utc() {
    for (body_interval, expected) in [
        ("", &["2024-02-01"][..]),
        (
            "tickInterval 1day\n",
            &[
                "2024-01-30",
                "2024-01-31",
                "2024-02-01",
                "2024-02-02",
                "2024-02-03",
            ][..],
        ),
    ] {
        let source = format!(
            r#"---
config:
  gantt:
    tickInterval: 1month
    topAxis: true
---
gantt
dateFormat YYYY-MM-DD
axisFormat %Y-%m-%d
{body_interval}todayMarker off
section Delivery
Task: task, 2024-01-30, 2024-02-03
"#,
        );
        let svg = render_gantt_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).expect("valid Gantt SVG XML");
        let axes = document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("grid"))
            .collect::<Vec<_>>();
        assert_eq!(axes.len(), 2, "both bottom and top axes must render");
        for (index, axis) in axes.iter().enumerate() {
            let labels = axis
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .map(|node| node.text().expect("axis tick label"))
                .collect::<Vec<_>>();
            assert_eq!(labels, expected, "axis {index}, body: {body_interval:?}");
        }
    }
}

#[test]
fn gantt_svg_frontmatter_config_fields_affect_visible_output() {
    let svg = render_gantt_svg_from_text(
        r#"---
displayMode: compact
config:
  gantt:
    useWidth: 420
    rightPadding: 10
    topAxis: true
    numberSectionStyles: 2
---
gantt
  title Config Frontmatter SVG Fields
  dateFormat YYYY-MM-DD
  axisFormat %Y-%m-%d
  tickInterval 1day
  todayMarker off
  section Alpha
  Task A :a1, 2024-01-01, 1d
  section Beta
  Task B :b1, 2024-01-02, 1d
"#,
    );

    assert!(
        svg.contains(r#"viewBox="0 0 420 "#) && svg.contains(r#"style="max-width: 420px;""#),
        "frontmatter gantt.useWidth should set rendered SVG width: {svg}"
    );
    assert_eq!(
        svg.matches(r#"<g class="grid" transform="translate(75, 50)""#)
            .count(),
        1,
        "frontmatter gantt.topAxis should add the top axis grid at top padding: {svg}"
    );
    assert_eq!(
        svg.matches(r#"<g class="grid" transform="translate(75, "#)
            .count(),
        2,
        "frontmatter gantt.topAxis should render both top and bottom axes: {svg}"
    );
    assert!(
        svg.contains(r#"width="415" height="24" class="section section0""#)
            && svg.contains(r#"width="415" height="24" class="section section1""#),
        "frontmatter gantt.rightPadding and numberSectionStyles should affect visible rows: {svg}"
    );
    assert!(
        svg.contains(r#"class="sectionTitle sectionTitle0""#)
            && svg.contains(r#"class="sectionTitle sectionTitle1""#)
            && svg.contains(r#"id="gantt-config-a1""#)
            && svg.contains(r#"id="gantt-config-b1-text""#),
        "configured Gantt SVG should expose section classes and scoped task DOM: {svg}"
    );
}

#[test]
fn gantt_vertical_markers_do_not_affect_standard_row_layout() {
    let layout = layout_gantt_from_text(
        r#"
gantt
dateFormat YYYY-MM-DD
section Delivery
Start marker: vert,marker-start,2024-01-01,0d
Task A: task-a,2024-01-02,1d
Middle marker: vert,marker-middle,2024-01-05,0d
Task B: task-b,2024-01-06,1d
Final marker: vert,marker-final,2024-01-10,0d
"#,
    );
    assert_eq!(layout.height, 148.0);
    assert_eq!(
        layout.rows.iter().map(|row| row.index).collect::<Vec<_>>(),
        vec![0, 1]
    );

    let markers = layout
        .tasks
        .iter()
        .filter(|task| task.vert)
        .collect::<Vec<_>>();
    assert_eq!(markers.len(), 3);
    assert!(markers.iter().all(|task| task.order == -1));
    assert!(markers.iter().all(|task| task.bar.height == 88.0));
    assert!(markers.iter().all(|task| task.label.y == 143.0));

    let final_marker = markers
        .iter()
        .find(|task| task.id == "marker-final")
        .expect("final marker");
    assert_eq!(
        final_marker.bar.x,
        layout.width - layout.right_padding,
        "vertical markers must remain part of the time domain"
    );
}

#[test]
fn gantt_vertical_markers_do_not_affect_compact_row_packing() {
    let layout = layout_gantt_from_text(
        r#"---
displayMode: compact
---
gantt
dateFormat YYYY-MM-DD
section Delivery
Long marker: vert,marker-long,2024-01-01,31d
Task A: task-a,2024-01-01,1d
Task B: task-b,2024-01-03,1d
"#,
    );
    assert_eq!(layout.height, 124.0);
    assert_eq!(
        layout.rows.iter().map(|row| row.index).collect::<Vec<_>>(),
        vec![0]
    );
    assert_eq!(
        layout
            .tasks
            .iter()
            .map(|task| (task.id.as_str(), task.order))
            .collect::<Vec<_>>(),
        vec![("marker-long", -1), ("task-a", 0), ("task-b", 0)]
    );
}

#[test]
fn gantt_active_done_labels_preserve_upstream_classes_and_outside_paint_ownership() {
    for critical in [false, true] {
        let source = format!(
            "gantt\ndateFormat YYYY-MM-DD\nsection Work\nAn extremely long outside task label: active, done, {}task, 2024-01-01, 1d\nLong span: span, 2024-01-01, 100d\n",
            if critical { "crit, " } else { "" }
        );
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "themeVariables": {"taskTextOutsideColor": "#123456"}
            })),
        );
        let svg = render_gantt_svg_from_text_with_engine(engine, &source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = document
            .descendants()
            .find(|node| {
                node.has_tag_name("text")
                    && node
                        .attribute("id")
                        .is_some_and(|id| id.ends_with("-task-text"))
            })
            .unwrap();
        let classes = label.attribute("class").unwrap();
        let active = if critical {
            "activeCritText0"
        } else {
            "activeText0"
        };
        let done = if critical {
            "doneCritText0"
        } else {
            "doneText0"
        };
        assert!(
            classes
                .split_ascii_whitespace()
                .any(|class| class == active),
            "{classes}"
        );
        assert!(
            classes.split_ascii_whitespace().any(|class| class == done),
            "{classes}"
        );
        assert!(classes.contains("taskTextOutside"), "{classes}");
        assert!(
            label.attribute("style").is_none(),
            "source-owned outside paint must retain its CSS winner"
        );
        assert!(svg.contains("fill:#123456;"), "{svg}");
    }
}

#[test]
fn gantt_compact_sections_use_javascript_object_key_order_without_reordering_semantics() {
    let source = concat!(
        "---\nconfig:\n  gantt:\n    displayMode: compact\n---\n",
        "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\n",
        "section 2\nA: a,2026-01-01,1d\n",
        "section 1\nB: b,2026-01-01,1d\n",
        "section 01\nC: c,2026-01-01,1d\n",
        "section Later\nD: d,2026-01-01,1d\n",
    );
    let layout = layout_gantt_from_text(source);
    assert_eq!(
        layout
            .tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "c", "d"],
    );
    assert_eq!(
        layout
            .tasks
            .iter()
            .map(|task| task.order)
            .collect::<Vec<_>>(),
        [1, 0, 2, 3],
    );
    assert_eq!(
        layout
            .section_titles
            .iter()
            .map(|title| title.section.as_str())
            .collect::<Vec<_>>(),
        ["1", "2", "01", "Later"],
    );
    let svg = render_gantt_svg_from_text(source);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let titles = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("text")
                && node
                    .attribute("class")
                    .is_some_and(|class| class.starts_with("sectionTitle "))
        })
        .map(|node| {
            node.descendants()
                .filter_map(|child| child.text())
                .next()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(titles, ["1", "2", "01", "Later"]);
}

#[test]
fn gantt_duplicate_id_labels_keep_their_semantic_occurrence_after_date_sorting() {
    let source = concat!(
        "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Work\n",
        "Later: done,dup,2026-01-02,1d\n",
        "First: active,dup,2026-01-01,1d\n",
    );
    let svg = render_gantt_svg_from_text(source);
    let document = roxmltree::Document::parse(&svg).unwrap();
    for (text, expected, other) in [
        ("First", "activeText0", "doneText0"),
        ("Later", "doneText0", "activeText0"),
    ] {
        let label = document
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some(text))
            .unwrap();
        let classes = label.attribute("class").unwrap();
        assert!(
            classes
                .split_ascii_whitespace()
                .any(|class| class == expected),
            "{classes}"
        );
        assert!(
            !classes.split_ascii_whitespace().any(|class| class == other),
            "{classes}"
        );
    }
}

#[test]
fn gantt_custom_axis_retains_all_admitted_ticks_and_falls_back_above_the_upstream_limit() {
    for (end, expected_count) in [("2026-04-11", Some(2401)), ("2027-03-01", None)] {
        let source = format!(
            concat!(
                "gantt\ndateFormat YYYY-MM-DD\naxisFormat %Y-%m-%d %H:%M\n",
                "tickInterval 1hour\ntodayMarker off\nTask: task,2026-01-01,{end}\n",
            ),
            end = end
        );
        let layout = layout_gantt_from_text(&source);
        let svg = render_gantt_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let ticks = document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("tick"))
            .count();
        assert_eq!(ticks, layout.bottom_ticks.len());
        if let Some(count) = expected_count {
            assert_eq!(ticks, count);
            assert_eq!(
                layout.bottom_ticks.first().unwrap().label,
                "2026-01-01 00:00"
            );
            assert_eq!(
                layout.bottom_ticks.last().unwrap().label,
                "2026-04-11 00:00"
            );
        } else {
            assert!(
                ticks < 100,
                "excessive custom intervals must use the automatic scale"
            );
        }
    }
}

#[test]
fn gantt_excluded_days_remain_visible_until_six_calendar_years() {
    for (end, count) in [("2025-07-01", 287), ("2026-01-01", 0)] {
        let source = format!(
            concat!(
                "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nexcludes weekends\n",
                "Task: task,2020-01-01,{end}\n",
            ),
            end = end
        );
        let layout = layout_gantt_from_text(&source);
        assert_eq!(layout.excludes.len(), count);
        assert_eq!(layout.has_excludes_layer, count > 0);
        let svg = render_gantt_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(
            document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("rect") && node.attribute("class") == Some("exclude-range")
                })
                .count(),
            count
        );
    }
}

#[test]
fn gantt_exclusion_matching_keeps_the_scan_clock_and_floors_only_rectangle_geometry() {
    use merman_model::time::{CivilDate, OffsetDateTime, UtcOffset};

    let ms = |day, hour| {
        OffsetDateTime::from_local(
            CivilDate::new(2024, 1, day)
                .unwrap()
                .at_hms_milli(hour, 0, 0, 0)
                .unwrap(),
            UtcOffset::UTC,
        )
        .unwrap()
        .timestamp_millis()
    };
    // ganttRenderer.drawExcludeDays keeps dayjs(minTime)'s clock until SVG geometry is derived.
    for (exclusion, includes, shaded) in [
        ("2024-01-02_12:00", "", true),
        ("2024-01-02", "", true),
        ("2024-01-02_00:00", "", false),
        ("2024-01-02_12:00", "includes 2024-01-02_12:00\n", false),
        ("2024-01-02_12:00", "includes 2024-01-02_00:00\n", true),
    ] {
        let source = format!(
            "gantt\ndateFormat YYYY-MM-DD_HH:mm\nexcludes {exclusion}\n{includes}todayMarker off\nTask: task,2024-01-01_12:00,2d\n"
        );
        let layout = layout_gantt_from_text(&source);
        assert_eq!(layout.tasks[0].start_ms, ms(1, 12));
        assert_eq!(layout.tasks[0].end_ms, ms(if shaded { 4 } else { 3 }, 12));
        assert_eq!(
            layout.excludes.len(),
            usize::from(shaded),
            "{exclusion}, {includes}"
        );

        let svg = render_gantt_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let rectangles = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("rect") && node.attribute("class") == Some("exclude-range")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rectangles.len(),
            usize::from(shaded),
            "{exclusion}, {includes}"
        );
        if !shaded {
            continue;
        }
        let excluded = &layout.excludes[0];
        assert_eq!(excluded.id, "exclude-2024-01-02");
        assert_eq!(excluded.start_ms, ms(2, 0));
        assert_eq!(excluded.end_ms, ms(3, 0) - 1);
        let rectangle = rectangles[0];
        assert_eq!(
            rectangle.attribute("id"),
            Some("gantt-config-exclude-2024-01-02")
        );
        for (attribute, expected) in [("x", excluded.x), ("width", excluded.width)] {
            assert_eq!(
                rectangle
                    .attribute(attribute)
                    .unwrap()
                    .parse::<f64>()
                    .unwrap(),
                expected
            );
        }
        let range = layout.width - layout.left_padding - layout.right_padding;
        let clock_x = layout.left_padding + (range / 3.0).round();
        let expected_origin = format!("{clock_x}px");
        assert_eq!(
            rectangle
                .attribute("transform-origin")
                .unwrap()
                .split_ascii_whitespace()
                .next(),
            Some(expected_origin.as_str()),
            "transform origin retains noon while the rectangle begins at midnight"
        );
    }
}

#[test]
fn gantt_field_filtered_axes_reset_fields_across_minutes_days_and_years() {
    // Mermaid 12.3.0 calls d3-time 3.1.1's every(field % step), including steps larger than a cycle.
    for (date_format, axis_format, interval, start, duration, expected) in [
        (
            "YYYY-MM-DD_HH:mm",
            "%Y-%m-%d_%H:%M",
            "5hour",
            "2024-01-01_20:00",
            "10h",
            &["2024-01-01_20:00", "2024-01-02_00:00", "2024-01-02_05:00"][..],
        ),
        (
            "YYYY-MM-DD_HH:mm",
            "%Y-%m-%d_%H:%M",
            "4hour",
            "2024-01-01_20:00",
            "10h",
            &["2024-01-01_20:00", "2024-01-02_00:00", "2024-01-02_04:00"][..],
        ),
        (
            "YYYY-MM-DD_HH:mm",
            "%Y-%m-%d_%H:%M",
            "25hour",
            "2024-01-01_20:00",
            "72h",
            &["2024-01-02_00:00", "2024-01-03_00:00", "2024-01-04_00:00"][..],
        ),
        (
            "YYYY-MM-DD_HH:mm:ss",
            "%Y-%m-%d_%H:%M:%S",
            "7second",
            "2024-01-01_00:00:56",
            "10s",
            &["2024-01-01_00:00:56", "2024-01-01_00:01:00"][..],
        ),
        (
            "YYYY-MM-DD_HH:mm:ss",
            "%Y-%m-%d_%H:%M:%S",
            "5second",
            "2024-01-01_00:00:56",
            "10s",
            &["2024-01-01_00:01:00", "2024-01-01_00:01:05"][..],
        ),
        (
            "YYYY-MM-DD",
            "%Y-%m-%d",
            "5month",
            "2024-11-01",
            "160d",
            &["2024-11-01", "2025-01-01"][..],
        ),
        (
            "YYYY-MM-DD",
            "%Y-%m-%d",
            "3month",
            "2024-11-01",
            "160d",
            &["2025-01-01", "2025-04-01"][..],
        ),
        (
            "YYYY-MM-DD",
            "%Y-%m-%d",
            "13month",
            "2024-11-01",
            "800d",
            &["2025-01-01", "2026-01-01", "2027-01-01"][..],
        ),
    ] {
        let source = format!(
            "gantt\ndateFormat {date_format}\naxisFormat {axis_format}\ntickInterval {interval}\ntodayMarker off\nTask: task,{start},{duration}\n"
        );
        let svg = render_gantt_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let axes = document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("grid"))
            .collect::<Vec<_>>();
        assert_eq!(axes.len(), 1, "{interval}");
        let labels = axes[0]
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .map(|node| node.text().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(labels, expected, "{interval}");
    }
}

#[test]
fn gantt_filtered_minute_config_and_body_precedence_reach_both_axes() {
    for (configured_interval, body_interval, expected) in [
        ("7minute", "", &["2024-01-01_00:56", "2024-01-01_01:00"][..]),
        ("5minute", "", &["2024-01-01_01:00", "2024-01-01_01:05"][..]),
        (
            "5minute",
            "tickInterval 7minute\n",
            &["2024-01-01_00:56", "2024-01-01_01:00"][..],
        ),
    ] {
        let source = format!(
            "---\nconfig:\n  gantt:\n    tickInterval: {configured_interval}\n    topAxis: true\n---\ngantt\ndateFormat YYYY-MM-DD_HH:mm\naxisFormat %Y-%m-%d_%H:%M\n{body_interval}todayMarker off\nTask: task,2024-01-01_00:56,10m\n"
        );
        let svg = render_gantt_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let axes = document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("grid"))
            .collect::<Vec<_>>();
        assert_eq!(axes.len(), 2);
        for axis in axes {
            let labels = axis
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .map(|node| node.text().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                labels, expected,
                "config {configured_interval}, body {body_interval:?}"
            );
        }
    }
}

#[allow(dead_code)]
mod common;

#[test]
fn gantt_svg_preserves_db_accessibility_text() {
    for (title, description, expected_title, expected_description) in [
        (
            "Accessible <!--a11y-->",
            "First\n<!--a11y-->",
            Some("Accessible "),
            Some("First\n"),
        ),
        (
            "<!--a11y-->\u{0085}<!--a11y-->",
            "<!--a11y-->\u{0085}<!--a11y-->",
            Some("\u{0085}"),
            Some("\u{0085}"),
        ),
        ("\u{0085}", "\u{0085}", Some("\u{0085}"), Some("\u{0085}")),
        ("<!--a11y-->", "<!--a11y-->", None, None),
    ] {
        let source = format!(
            "gantt\naccTitle: {title}\naccDescr {{\n{description}\n}}\ndateFormat YYYY-MM-DD\nTask: task,2026-01-01,1d\n"
        );
        let svg = render_gantt_svg_from_text(&source);
        common::assert_svg_accessibility(&svg, expected_title, expected_description);
    }
}
