mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_timeline_svg_from_text(text: &str) -> String {
    render_timeline_svg_from_text_with_engine(text, legacy_init_theme_compat_engine())
}

fn render_timeline_svg_from_text_with_engine(text: &str, engine: MermaidEngine) -> String {
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("layout ok");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render svg")
        .svg()
        .to_owned()
}

fn try_render_timeline_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse Timeline resource-bound fixture")
        .expect("detect Timeline resource-bound fixture")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Timeline resource-bound session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn timeline_repeated_sections_expand_svg_without_duplicating_canonical_tasks() {
    use merman_model::presentation::RenderSemanticModel;

    for direction in ["LR", "TD"] {
        for second_section in ["S", "T"] {
            let source = format!(
                "timeline {direction}\nsection S\nA : X\nsection {second_section}\nB : Y\n"
            );
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let RenderSemanticModel::Timeline(model) = parsed.model() else {
                panic!("expected the canonical Timeline model");
            };
            let canonical = serde_json::to_value(model).unwrap();
            assert_eq!(model.tasks.len(), 2);
            assert_eq!(model.tasks[0].section_index, Some(0));
            assert_eq!(model.tasks[1].section_index, Some(1));
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let artifact =
                family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let labels = |class| {
                document
                    .descendants()
                    .filter(|node| node.attribute("class") == Some(class))
                    .map(|wrapper| {
                        wrapper
                            .descendants()
                            .filter(|node| node.is_text())
                            .filter_map(|node| node.text())
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
            };
            let (tasks, events): (&[&str], &[&str]) = if second_section == "S" {
                (&["A", "B", "A", "B"], &["X", "Y", "X", "Y"])
            } else {
                (&["A", "B"], &["X", "Y"])
            };
            assert_eq!(labels("taskWrapper"), tasks);
            assert_eq!(labels("eventWrapper"), events);
            assert_eq!(serde_json::to_value(model).unwrap(), canonical);
        }
    }
}

#[test]
fn timeline_repeated_sections_observe_cancellation_during_expansion_and_before_emit() {
    use merman_model::OperationControl;
    use merman_render::environment::{
        HostMeasurementResult, HostTextMeasurementRequest, HostTextMeasurer, MeasurementProfileId,
        TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfileIdentity,
    };
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CancelRepeatedTask(AtomicUsize);
    impl HostTextMeasurer for CancelRepeatedTask {
        fn measure(&self, _request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            Ok(None)
        }

        fn measure_with_control(
            &self,
            request: HostTextMeasurementRequest<'_>,
            control: &OperationControl,
        ) -> HostMeasurementResult {
            if request.phase == TextMeasurementPhase::ComputedLength
                && request.text == "A"
                && self.0.fetch_add(1, Ordering::Relaxed) == 1
            {
                control.cancel();
            }
            Ok(None)
        }
    }

    for direction in ["LR", "TD"] {
        let source = format!("timeline {direction}\nsection S\nA : X\nsection S\nB : Y\n");
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let control = OperationControl::new();
        let host = Arc::new(CancelRepeatedTask(AtomicUsize::new(0)));
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.timeline-repeated-section-cancellation").unwrap(),
            "v1",
        )
        .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                identity,
                host.clone(),
                TextMeasurementPhase::ALL,
            ))
            .begin_session_with_control(control)
            .unwrap();
        assert!(matches!(
            family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session),
            Err(merman_render::Error::Cancelled(_))
        ));
        assert_eq!(host.0.load(Ordering::Relaxed), 2);

        let control = OperationControl::new();
        let session = RenderEnvironment::deterministic()
            .begin_session_with_control(control.clone())
            .unwrap();
        let artifact =
            family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
        control.cancel();
        assert!(matches!(
            artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()),
            Err(merman_render::Error::Cancelled(_))
        ));
    }
}

#[test]
fn timeline_svg_skips_boundary_comments_and_keeps_literal_event_percent_pairs() {
    for direction in ["LR", "TD"] {
        let source = format!(
            "timeline {direction}\nsection A\nP : Seed\nx%% hidden\nsection B:%% hidden\n: literal%%kept\nNext : Ready\n"
        );
        let svg = render_timeline_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).expect("valid Timeline SVG");
        let tasks = document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("taskWrapper"))
            .count();
        let events = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("g") && node.attribute("class") == Some("eventWrapper")
            })
            .map(|event| {
                event
                    .descendants()
                    .filter(|node| node.is_text())
                    .filter_map(|node| node.text())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert_eq!(tasks, 2, "{source}");
        assert_eq!(events, ["Seed", "literal%%kept", "Ready"], "{source}");
        assert!(!svg.contains("hidden"));
    }
}

#[test]
fn timeline_svg_renders_section_suffix_events_like_following_lines() {
    for direction in ["LR", "TD"] {
        for (section_separator, event_separator) in [("", " "), ("\n", " "), ("", "\n")] {
            let source = format!(
                "timeline {direction}\nsection A\nP : Seed\nsection B{section_separator}:{event_separator}Extra\n"
            );
            let svg = render_timeline_svg_from_text(&source);
            let document = roxmltree::Document::parse(&svg).expect("valid Timeline SVG");
            let tasks = document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("g") && node.attribute("class") == Some("taskWrapper")
                })
                .count();
            let events = document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("g") && node.attribute("class") == Some("eventWrapper")
                })
                .map(|event| {
                    event
                        .descendants()
                        .filter(|node| node.is_text())
                        .filter_map(|node| node.text())
                        .collect::<String>()
                })
                .collect::<Vec<_>>();

            assert_eq!(tasks, 1, "{source}");
            assert_eq!(events, ["Seed", "Extra"], "{source}");
        }
    }
}

#[test]
fn timeline_svg_accepts_complete_accessibility_values_and_crossline_tokens() {
    let same_line = concat!(
        "timeline\n",
        "accTitle: Release; ready #stable\n",
        "accDescr: Ship; fully #accessible\n",
        "section A\nP : Seed\n",
    );
    let cross_line = concat!(
        "timeline\r\n",
        "accTitle\r\n\u{feff}:\r\nRelease; ready #stable\r\n",
        "accDescr\r\n{\r\nShip; fully #accessible\r\n}\r\n",
        "section\r\nA\r\nP :\r\nSeed\r\n",
    );
    for source in [same_line, cross_line] {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("Timeline with a11y punctuation parses")
            .expect("Timeline is detected");
        let merman_model::presentation::RenderSemanticModel::Timeline(model) = parsed.model()
        else {
            panic!("expected Timeline model");
        };
        assert_eq!(model.acc_title.as_deref(), Some("Release; ready #stable"));
        assert_eq!(model.acc_descr.as_deref(), Some("Ship; fully #accessible"));
        let compatibility = parsed
            .compatibility_json()
            .expect("Timeline compatibility JSON");
        assert_eq!(compatibility["accTitle"], "Release; ready #stable");
        assert_eq!(compatibility["accDescr"], "Ship; fully #accessible");

        let svg = render_timeline_svg_from_text(source);
        let document = roxmltree::Document::parse(&svg).expect("valid Timeline SVG");
        assert_eq!(
            document
                .descendants()
                .filter(
                    |node| node.has_tag_name("g") && node.attribute("class") == Some("taskWrapper")
                )
                .count(),
            1
        );
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.has_tag_name("g")
                    && node.attribute("class") == Some("eventWrapper"))
                .count(),
            1
        );
    }
}

#[test]
fn timeline_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = "timeline\nsection S\nA : X\nsection S\nB : Y\n";
    let baseline = try_render_timeline_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Timeline baseline");
    let exact_bytes = baseline.len();
    assert!(
        exact_bytes > 1,
        "Timeline fixture must emit a non-empty SVG"
    );

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Timeline SVG byte ceiling");
    let exact = try_render_timeline_svg_with_resource_policy(source, exact_policy)
        .expect("the exact Timeline family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Timeline SVG byte ceiling");
    let error = try_render_timeline_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the Timeline family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Timeline MaxSvgBytes rejection, got {error}");
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
fn timeline_svg_honors_mermaid_11_15_disabled_theme_colors() {
    let svg = render_timeline_svg_from_text(
        r##"%%{init: {"themeVariables": {"tertiaryColor": "#123456", "clusterBorder": "#abcdef"}}}%%
timeline
    section Release
        2026 : Ship
"##,
    );

    assert!(
        svg.contains(
            r#"#merman .disabled,#merman .disabled circle,#merman .disabled text{fill:#123456;}"#
        ),
        "expected Timeline disabled node CSS to use themeVariables.tertiaryColor: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .disabled text{fill:#abcdef;}"#),
        "expected Timeline disabled text CSS to use themeVariables.clusterBorder: {svg}"
    );
    assert!(
        !svg.contains(
            r#"#merman .disabled,#merman .disabled circle,#merman .disabled text{fill:lightgray;}"#
        ),
        "Timeline disabled node CSS should not ignore theme variables"
    );
    assert!(
        !svg.contains(r#"#merman .disabled text{fill:#efefef;}"#),
        "Timeline disabled text CSS should not ignore theme variables"
    );
}

#[test]
fn timeline_svg_uses_redux_theme_on_visible_nodes_and_lines() {
    let svg = render_timeline_svg_from_text(
        r##"%%{init: {"theme": "redux", "themeVariables": {"THEME_COLOR_LIMIT": 2, "mainBkg": "#111827", "nodeBorder": "#38bdf8", "strokeWidth": 5, "cScale0": "#ef4444", "cScaleLabel0": "#e879f9", "cScaleInv0": "#334155", "cScale1": "#172554", "cScaleLabel1": "#f8fafc", "cScaleInv1": "#334155"}}}%%
timeline
    section Release
        Plan : Build
        Ship : Done
"##,
    );

    assert!(
        svg.contains(
            r#"#merman .section--1 rect,#merman .section--1 path,#merman .section--1 circle{fill:#111827;stroke:#38bdf8;stroke-width:5;filter:url(#merman-drop-shadow);}"#
        ),
        "expected redux Timeline nodes to consume mainBkg/nodeBorder/strokeWidth on visible path DOM: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .section--1 text{fill:#38bdf8;font-weight:600;}"#),
        "expected redux Timeline labels to consume nodeBorder/fontWeight like Mermaid 11.15: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .lineWrapper line{stroke:#38bdf8;stroke-width:5;}"#),
        "expected redux Timeline lineWrapper CSS to consume nodeBorder/strokeWidth: {svg}"
    );
    assert!(
        svg.contains(r#"stroke-width="2" stroke="black" marker-end="url(#merman-arrowhead)""#),
        "expected current visible line DOM to keep Mermaid's presentational attributes while CSS overrides them: {svg}"
    );
    assert!(
        !svg.contains(r#"class="node-line--1""#),
        "redux Timeline nodes should not emit the classic bottom divider line DOM: {svg}"
    );
    assert!(
        !svg.contains(r#"q0,-5 5,-5"#),
        "redux Timeline node geometry should use sharp-corner paths instead of classic rounded corners: {svg}"
    );
    assert!(
        svg.contains(r#"transform="translate(195, 20)""#),
        "redux Timeline non-event labels should use the Mermaid 11.15 vertical offset: {svg}"
    );
    assert!(
        svg.contains(r#"transform="translate(95, 13)""#),
        "redux Timeline event labels should use the Mermaid 11.15 event vertical offset: {svg}"
    );
}

#[test]
fn timeline_neo_emits_look_scoped_nodes_and_gradient_paint() {
    let svg = render_timeline_svg_from_text(
        r##"%%{init: {"look": "neo", "theme": "default", "themeVariables": {"useGradient": true, "gradientStart": "#112233", "gradientStop": "#445566"}}}%%
timeline
    title Neo timeline
    section Release
        Plan : Build
"##,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Timeline SVG");

    assert!(
        document
            .descendants()
            .any(|node| node.attribute("data-look") == Some("neo")),
        "Neo Timeline nodes must carry the look discriminator: {svg}"
    );
    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("linearGradient") && node.attribute("id") == Some("merman-gradient")
        }),
        "Neo Timeline with useGradient must emit a scoped gradient definition: {svg}"
    );
    assert!(
        svg.contains(r#".section--1[data-look="neo"] rect"#)
            && svg.contains("stroke:url(#merman-gradient);stroke-width:2"),
        "Neo Timeline CSS must target look-scoped section terminals: {svg}"
    );
}

#[test]
fn timeline_neo_uses_neo_title_positioning() {
    let source = "timeline\n    title Neo title\n    section Release\n        Plan : Build\n";
    let classic = render_timeline_svg_from_text(source);
    let neo =
        render_timeline_svg_from_text(&format!("%%{{init: {{\"look\": \"neo\"}}}}%%\n{source}"));
    let title_x = |svg: &str| {
        roxmltree::Document::parse(svg)
            .expect("valid Timeline SVG")
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some("Neo title"))
            .and_then(|node| node.attribute("x"))
            .expect("Timeline title x")
            .parse::<f64>()
            .expect("numeric Timeline title x")
    };

    assert_ne!(title_x(&classic), title_x(&neo));
}

#[test]
fn timeline_neo_redux_emits_scoped_drop_shadow_filter() {
    let svg = render_timeline_svg_from_text(
        r##"%%{init: {"look": "neo", "theme": "redux"}}%%
timeline
    section Release
        Plan : Build
"##,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Timeline SVG");

    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("filter") && node.attribute("id") == Some("merman-drop-shadow")
        }),
        "Neo Redux Timeline must define the filter referenced by its section CSS: {svg}"
    );
}

#[test]
fn timeline_svg_honors_disabled_max_width() {
    let svg = render_timeline_svg_from_text(
        r##"%%{init: {"timeline": {"useMaxWidth": false, "padding": 12}}}%%
timeline
    section Release
        2026 : Ship
"##,
    );
    let root_open = svg.split_once('>').expect("root svg open tag").0;

    assert!(root_open.contains(r#"height=""#), "{root_open}");
    assert!(!root_open.contains("style="), "{root_open}");
    assert!(!root_open.contains("max-width"), "{root_open}");
}
