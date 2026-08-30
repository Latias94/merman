mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, InsetsPx,
    MermaidThemeCompatibility, OrdinalPalette, OrdinalSelector, Specified, ThemeColorValue,
    ThemeGeometryPatch, ThemePaintPatch, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurementRequest, HostTextMeasurer, MeasurementProfileId,
    RenderEnvironment, TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use std::sync::{Arc, Mutex};

fn render_timeline_svg_from_text(text: &str) -> String {
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("layout ok");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render svg")
        .svg()
        .to_owned()
}

fn timeline_event_rules_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    timeline_event_rules_theme_with_mermaid(MermaidThemeCompatibility::default(), rules)
}

fn timeline_event_rules_theme_with_mermaid(
    mermaid: MermaidThemeCompatibility,
    rules: impl IntoIterator<Item = ThemeRule>,
) -> DiagramTheme {
    let styles = rules
        .into_iter()
        .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule);
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_mermaid_compatibility(mermaid)
                .with_styles(styles),
        )
        .expect("compile Timeline event theme")
}

fn timeline_event_opacity_style(opacity: Specified<f32>) -> ThemeStylePatch {
    ThemeStylePatch {
        paint: ThemePaintPatch {
            opacity,
            ..ThemePaintPatch::default()
        },
        ..ThemeStylePatch::default()
    }
}

fn timeline_event_opacity_theme(opacity: Specified<f32>) -> DiagramTheme {
    timeline_event_rules_theme([ThemeRule::new(
        ThemeTarget::TimelineEvent,
        timeline_event_opacity_style(opacity),
    )])
}

fn timeline_event_radius_style(radius: Specified<f32>) -> ThemeStylePatch {
    ThemeStylePatch {
        geometry: ThemeGeometryPatch { radius },
        ..ThemeStylePatch::default()
    }
}

fn timeline_event_radius_theme(radius: Specified<f32>) -> DiagramTheme {
    timeline_event_rules_theme([ThemeRule::new(
        ThemeTarget::TimelineEvent,
        timeline_event_radius_style(radius),
    )])
}

fn timeline_event_palette_theme(colors: &[&str]) -> DiagramTheme {
    let palette =
        OrdinalPalette::new(colors.iter().map(|color| {
            ThemeColorValue::parse(color).expect("valid Timeline event palette color")
        }))
        .expect("non-empty Timeline event palette");
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::TimelineEvent, palette),
        ))
        .expect("compile Timeline event palette theme")
}

fn timeline_typography_theme(font_stack: FontStack) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(
                merman_render::DiagramFamilyId::TIMELINE,
                ThemeTextStyle::default().with_font_stack(font_stack),
            ),
        ))
        .expect("compile Timeline typography theme")
}

fn try_render_timeline_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> merman_render::Result<family::RenderedFamilySvg> {
    try_render_timeline_with_theme_engine_requirement(
        source,
        theme,
        engine,
        ThemePortabilityRequirement::RequirePortable,
    )
}

fn try_render_timeline_with_theme_engine_requirement(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Timeline")
        .expect("detect themed Timeline");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed Timeline session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?;
    artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
}

fn try_render_timeline_with_theme(
    source: &str,
    theme: &DiagramTheme,
) -> merman_render::Result<family::RenderedFamilySvg> {
    try_render_timeline_with_theme_requirement(
        source,
        theme,
        ThemePortabilityRequirement::RequirePortable,
    )
}

fn try_render_timeline_with_theme_requirement(
    source: &str,
    theme: &DiagramTheme,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Timeline")
        .expect("detect themed Timeline");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed Timeline session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?;
    artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
}

fn try_render_timeline_svg_with_theme(
    source: &str,
    theme: &DiagramTheme,
) -> merman_render::Result<String> {
    Ok(try_render_timeline_with_theme(source, theme)?
        .svg()
        .to_owned())
}

fn render_timeline_svg_with_theme(source: &str, theme: &DiagramTheme) -> String {
    try_render_timeline_svg_with_theme(source, theme).expect("render themed Timeline SVG")
}

fn try_render_timeline_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse Timeline resource-bound fixture")
        .expect("detect Timeline resource-bound fixture");
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Timeline resource-bound session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn timeline_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = "timeline\n    section Release\n        2026 : Ship\n";
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
fn timeline_event_palette_reaches_section_task_and_event_terminals() {
    let theme = timeline_event_palette_theme(&["#123456", "#654321"]);
    let rendered = try_render_timeline_with_theme(
        "timeline\n    section Release\n        Plan : Build\n    section Verify\n        Ship : Done\n",
        &theme,
    )
    .expect("render direct Timeline event palette");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Timeline palette SVG");
    let path_styles = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path") && node.attribute("class") == Some("node-bkg node-undefined")
        })
        .map(|node| node.attribute("style").unwrap_or_default().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        path_styles,
        [
            "fill:#123456;",
            "fill:#123456;",
            "fill:#123456;",
            "fill:#654321;",
            "fill:#654321;",
            "fill:#654321;",
        ]
    );
    let line_styles = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("line")
                && node
                    .attribute("class")
                    .is_some_and(|class| class.starts_with("node-line-"))
        })
        .map(|node| node.attribute("style").unwrap_or_default().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        line_styles,
        [
            "stroke:#edcba9;",
            "stroke:#edcba9;",
            "stroke:#edcba9;",
            "stroke:#9abcde;",
            "stroke:#9abcde;",
            "stroke:#9abcde;",
        ]
    );
    let stylesheet = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Timeline stylesheet");
    assert!(!stylesheet.contains("#123456") && !stylesheet.contains("#654321"));

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedTimelineMeasurement {
    text: String,
    font_family: Option<String>,
}

#[derive(Default)]
struct RecordingTimelineHost {
    requests: Mutex<Vec<RecordedTimelineMeasurement>>,
}

impl RecordingTimelineHost {
    fn snapshot(&self) -> Vec<RecordedTimelineMeasurement> {
        self.requests
            .lock()
            .expect("Timeline host requests lock")
            .clone()
    }
}

impl HostTextMeasurer for RecordingTimelineHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        self.requests
            .lock()
            .expect("Timeline host requests lock")
            .push(RecordedTimelineMeasurement {
                text: request.text.to_string(),
                font_family: request.style.font_family.clone(),
            });
        Ok(None)
    }
}

#[test]
fn timeline_typed_font_stack_reaches_layout_css_and_terminal_receipt() {
    let font_stack =
        FontStack::new(["Timeline Typed", "sans-serif"]).expect("valid Timeline font stack");
    let expected_font = font_stack.as_css();
    let theme = timeline_typography_theme(font_stack);
    for (direction, profile_id, source) in [
        (
            "LR",
            "test.timeline-typed-font-lr",
            "timeline\n    title LR typography proof\n    section Plan\n        Task : Event\n",
        ),
        (
            "TD",
            "test.timeline-typed-font-td",
            "timeline TD\n    title TD typography proof\n    section Plan\n        Task : Event\n",
        ),
    ] {
        let host = Arc::new(RecordingTimelineHost::default());
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new(profile_id).expect("valid Timeline profile id"),
            "1",
        )
        .expect("valid Timeline measurement profile identity");
        let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
            TextMeasurementPolicy::host_display(identity, host.clone(), TextMeasurementPhase::ALL),
        );
        let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap_or_else(|error| panic!("parse typed {direction} Timeline: {error}"))
            .unwrap_or_else(|| panic!("detect typed {direction} Timeline"));
        let session = environment
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .unwrap_or_else(|error| panic!("begin typed {direction} Timeline session: {error}"));
        let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap_or_else(|error| panic!("prepare typed {direction} Timeline: {error}"));
        let requests = host.snapshot();
        for label in ["Plan", "Task", "Event"] {
            let matching = requests
                .iter()
                .filter(|request| request.text == label)
                .collect::<Vec<_>>();
            assert!(
                !matching.is_empty(),
                "{direction} Timeline layout must measure {label:?}"
            );
            assert!(
                matching.iter().all(|request| {
                    request.font_family.as_deref() == Some(expected_font.as_str())
                }),
                "{direction} Timeline layout must measure {label:?} with the typed font stack: {matching:#?}"
            );
        }

        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap_or_else(|error| panic!("render typed {direction} Timeline: {error}"));
        let document = roxmltree::Document::parse(rendered.svg())
            .unwrap_or_else(|error| panic!("valid {direction} Timeline SVG: {error}"));
        let stylesheet = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("Timeline stylesheet");

        assert!(
            stylesheet.contains(&format!("#merman{{font-family:{expected_font};")),
            "typed {direction} Timeline FontStack must reach the scoped root CSS"
        );
        assert!(
            stylesheet.contains(&format!(
                "#merman :root{{--mermaid-font-family:{expected_font};}}"
            )),
            "typed {direction} Timeline FontStack must reach the Mermaid root variable"
        );

        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "{direction}");
        assert_eq!(evidence.accounted_count(), 1, "{direction}");
        assert_eq!(evidence.applied_count(), 1, "{direction}");
        assert_eq!(evidence.not_applicable_count(), 0, "{direction}");
        assert_eq!(evidence.theme_residual_count(), 0, "{direction}");
        assert_eq!(evidence.compatibility_residual_count(), 0, "{direction}");
    }
}

#[test]
fn timeline_event_palette_preserves_source_owned_color_scale_slots() {
    let theme = timeline_event_palette_theme(&["#123456", "#654321"]);
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "secure": []
    })));
    let rendered = try_render_timeline_with_theme_and_engine(
        r##"%%{init: {"themeVariables": {"cScale0": "#f59e0b", "cScaleInv0": "#010203"}}}%%
timeline
    section Release
        Plan : Build
    section Verify
        Ship : Done
"##,
        &theme,
        engine,
    )
    .expect("render mixed source-owned Timeline palette");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Timeline palette SVG");
    let path_styles = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path") && node.attribute("class") == Some("node-bkg node-undefined")
        })
        .map(|node| node.attribute("style").unwrap_or_default().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        path_styles,
        [
            "fill:#f59e0b;",
            "fill:#f59e0b;",
            "fill:#f59e0b;",
            "fill:#654321;",
            "fill:#654321;",
            "fill:#654321;",
        ]
    );

    let line_styles = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("line")
                && node
                    .attribute("class")
                    .is_some_and(|class| class.starts_with("node-line-"))
        })
        .map(|node| node.attribute("style").unwrap_or_default().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        line_styles,
        [
            "stroke:#010203;",
            "stroke:#010203;",
            "stroke:#010203;",
            "stroke:#9abcde;",
            "stroke:#9abcde;",
            "stroke:#9abcde;",
        ]
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn timeline_event_palette_wraps_at_a_reduced_theme_color_limit() {
    let theme = timeline_event_palette_theme(&["#123456", "#654321", "#abcdef"]);
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "secure": [],
        "themeVariables": { "THEME_COLOR_LIMIT": 2 }
    })));
    let rendered = try_render_timeline_with_theme_engine_requirement(
        "timeline\n    section Release\n        Plan : Build\n    section Verify\n        Ship : Done\n    section Archive\n        Store : Done\n",
        &theme,
        engine,
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("render reduced Timeline color limit");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid limited Timeline SVG");
    let path_styles = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path") && node.attribute("class") == Some("node-bkg node-undefined")
        })
        .map(|node| node.attribute("style"))
        .collect::<Vec<_>>();
    assert_eq!(
        path_styles,
        [
            Some("fill:#123456;"),
            Some("fill:#123456;"),
            Some("fill:#123456;"),
            Some("fill:#654321;"),
            Some("fill:#654321;"),
            Some("fill:#654321;"),
            Some("fill:#123456;"),
            Some("fill:#123456;"),
            Some("fill:#123456;"),
        ]
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn timeline_event_palette_wraps_the_default_twelve_slot_ring_for_orphan_tasks() {
    let colors = [
        "#100000", "#200000", "#300000", "#400000", "#500000", "#600000", "#700000", "#800000",
        "#900000", "#a00000", "#b00000", "#c00000",
    ];
    let theme = timeline_event_palette_theme(&colors);
    let mut source = String::from("timeline\n");
    for index in 0..13 {
        source.push_str(&format!("    Task {index} : Event {index}\n"));
    }

    let rendered = try_render_timeline_with_theme(&source, &theme)
        .expect("render orphan Timeline palette cycle");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Timeline palette SVG");
    let fills = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path") && node.attribute("class") == Some("node-bkg node-undefined")
        })
        .map(|node| node.attribute("style").unwrap_or_default().to_owned())
        .collect::<Vec<_>>();

    let line_strokes = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("line")
                && node
                    .attribute("class")
                    .is_some_and(|class| class.starts_with("node-line-"))
        })
        .map(|node| node.attribute("style").unwrap_or_default().to_owned())
        .collect::<Vec<_>>();

    assert_eq!(fills.len(), 26, "one task and one event per orphan task");
    assert_eq!(fills[0], "fill:#100000;");
    assert_eq!(fills[22], "fill:#c00000;", "twelfth task must use slot 11");
    assert_eq!(
        fills[24], "fill:#100000;",
        "thirteenth task must wrap to slot 0"
    );
    assert_eq!(
        fills[25], "fill:#100000;",
        "thirteenth event must wrap to slot 0"
    );
    assert_eq!(
        line_strokes.len(),
        26,
        "one divider line per classic task/event"
    );
    assert_eq!(line_strokes[0], "stroke:#efffff;");
    assert_eq!(
        line_strokes[22], "stroke:#3fffff;",
        "twelfth task must use slot 11 inverse"
    );
    assert_eq!(
        line_strokes[24], "stroke:#efffff;",
        "thirteenth task must wrap to slot 0 inverse"
    );
    assert_eq!(
        line_strokes[25], "stroke:#efffff;",
        "thirteenth event must wrap to slot 0 inverse"
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn timeline_event_palette_respects_disabled_multicolor_for_orphan_tasks() {
    let theme = timeline_event_palette_theme(&["#123456", "#654321"]);
    let rendered = try_render_timeline_with_theme(
        r##"%%{init: {"timeline": {"disableMulticolor": true}}}%%
timeline
    Task A : Event A
    Task B : Event B
"##,
        &theme,
    )
    .expect("render disabled-multicolor Timeline palette");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Timeline palette SVG");
    let fills = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path") && node.attribute("class") == Some("node-bkg node-undefined")
        })
        .map(|node| node.attribute("style").unwrap_or_default().to_owned())
        .collect::<Vec<_>>();

    let line_strokes = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("line")
                && node
                    .attribute("class")
                    .is_some_and(|class| class.starts_with("node-line-"))
        })
        .map(|node| node.attribute("style").unwrap_or_default().to_owned())
        .collect::<Vec<_>>();

    assert_eq!(fills, ["fill:#123456;"; 4]);
    assert_eq!(line_strokes, ["stroke:#edcba9;"; 4]);
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn timeline_event_palette_reaches_top_down_task_and_event_terminals() {
    let theme = timeline_event_palette_theme(&["#123456", "#654321"]);
    let rendered =
        try_render_timeline_with_theme("timeline TD\n    2026 : Ship\n    2027 : Done\n", &theme)
            .expect("render TopDown Timeline palette");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid TopDown Timeline SVG");
    let fills = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path") && node.attribute("class") == Some("node-bkg node-undefined")
        })
        .map(|node| node.attribute("style").unwrap_or_default().to_owned())
        .collect::<Vec<_>>();
    let line_strokes = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("line")
                && node
                    .attribute("class")
                    .is_some_and(|class| class.starts_with("node-line-"))
        })
        .map(|node| node.attribute("style").unwrap_or_default().to_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        fills,
        [
            "fill:#123456;",
            "fill:#123456;",
            "fill:#654321;",
            "fill:#654321;",
        ]
    );
    assert_eq!(
        line_strokes,
        [
            "stroke:#edcba9;",
            "stroke:#edcba9;",
            "stroke:#9abcde;",
            "stroke:#9abcde;",
        ]
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn timeline_event_transparent_palette_reaches_classic_terminals() {
    let theme = timeline_event_palette_theme(&["transparent"]);
    let rendered = try_render_timeline_with_theme(
        "timeline\n    section Release\n        Plan : Build\n",
        &theme,
    )
    .expect("render transparent Timeline event palette");
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid transparent Timeline SVG");
    let path_styles = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path") && node.attribute("class") == Some("node-bkg node-undefined")
        })
        .map(|node| node.attribute("style"))
        .collect::<Vec<_>>();
    assert_eq!(path_styles, [Some("fill:#00000000;"); 3]);
    let line_styles = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("line")
                && node
                    .attribute("class")
                    .is_some_and(|class| class.starts_with("node-line-"))
        })
        .map(|node| node.attribute("style"))
        .collect::<Vec<_>>();
    assert_eq!(line_styles, [Some("stroke:rgba(255, 255, 255, 0);"); 3]);

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn timeline_event_palette_is_not_applicable_to_redux_node_fill() {
    let palette = OrdinalPalette::new([
        ThemeColorValue::parse("#123456").expect("valid Redux Timeline palette color")
    ])
    .expect("non-empty Redux Timeline palette");
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_mermaid_compatibility(
                    MermaidThemeCompatibility::default()
                        .with_theme("redux")
                        .expect("valid Redux Mermaid theme"),
                )
                .with_styles(
                    ThemeRuleSet::default()
                        .with_ordinal_palette(ThemeTarget::TimelineEvent, palette),
                ),
        )
        .expect("compile Redux Timeline palette theme");
    let rendered = try_render_timeline_with_theme_requirement(
        "timeline\n    section Release\n        Plan : Build\n",
        &theme,
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("render Redux Timeline palette");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Redux Timeline SVG");
    let path_styles = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path") && node.attribute("class") == Some("node-bkg node-undefined")
        })
        .map(|node| node.attribute("style"))
        .collect::<Vec<_>>();
    assert_eq!(path_styles, [None; 3]);
    assert!(!document.descendants().any(|node| {
        node.has_tag_name("line")
            && node
                .attribute("class")
                .is_some_and(|class| class.starts_with("node-line-"))
    }));

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
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
    assert!(
        root_open.contains(r#"style="background-color: white;""#),
        "{root_open}"
    );
    assert!(!root_open.contains("max-width"), "{root_open}");
}

#[test]
fn timeline_static_event_opacity_reaches_each_terminal_wrapper() {
    let source = concat!(
        "timeline\n",
        "    section Release\n",
        "        2026 : Plan\n",
        "             : Ship\n",
    );
    let svg = render_timeline_svg_with_theme(
        source,
        &timeline_event_opacity_theme(Specified::Value(0.5)),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid themed Timeline SVG XML");
    let wrappers = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("eventWrapper"))
        .collect::<Vec<_>>();
    assert_eq!(wrappers.len(), 2);
    assert!(
        wrappers
            .iter()
            .all(|wrapper| wrapper.attribute("opacity") == Some("0.5"))
    );
}

#[test]
fn timeline_event_opacity_preserves_the_authored_round_trip_token() {
    let svg = render_timeline_svg_with_theme(
        "timeline\n    2026 : Ship\n",
        &timeline_event_opacity_theme(Specified::Value(0.000_000_000_5)),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid themed Timeline SVG XML");
    let wrapper = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("eventWrapper"))
        .expect("Timeline event wrapper");

    assert_eq!(wrapper.attribute("opacity"), Some("0.0000000005"));
}

#[test]
fn timeline_event_opacity_clear_restores_the_absent_attribute() {
    let svg = render_timeline_svg_with_theme(
        "timeline\n    2026 : Ship\n",
        &timeline_event_opacity_theme(Specified::Clear),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid cleared Timeline SVG XML");
    let wrapper = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("eventWrapper"))
        .expect("Timeline event wrapper");
    assert_eq!(wrapper.attribute("opacity"), None);
}

#[test]
fn timeline_static_event_radius_reaches_each_terminal_node_path() {
    let source = concat!(
        "timeline\n",
        "    section Release\n",
        "        2026 : Plan\n",
        "             : Ship\n",
    );
    let rendered = try_render_timeline_with_theme(
        source,
        &timeline_event_radius_theme(Specified::Value(12.0)),
    )
    .expect("render themed Timeline SVG");
    let svg = rendered.svg().to_owned();
    let document = roxmltree::Document::parse(&svg).expect("valid themed Timeline SVG XML");
    let event_paths = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("eventWrapper"))
        .flat_map(|wrapper| wrapper.descendants())
        .filter(|node| {
            node.has_tag_name("path") && node.attribute("class") == Some("node-bkg node-undefined")
        })
        .collect::<Vec<_>>();
    assert_eq!(event_paths.len(), 2);
    assert!(
        event_paths[0]
            .attribute("d")
            .expect("first event path")
            .contains("q0,-12 12,-12")
    );
    assert!(
        event_paths[1]
            .attribute("d")
            .expect("second event path")
            .contains("q12,0 12,12")
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn timeline_static_event_radius_clear_preserves_baseline_geometry() {
    let svg = render_timeline_svg_with_theme(
        "timeline\n    2026 : Ship\n",
        &timeline_event_radius_theme(Specified::Clear),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid cleared Timeline SVG XML");
    let event_path = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path") && node.attribute("class") == Some("node-bkg node-undefined")
        })
        .nth(1)
        .expect("Timeline event path");
    assert!(
        event_path
            .attribute("d")
            .expect("event path data")
            .contains("q0,-5 5,-5")
    );
}

#[test]
fn timeline_static_event_radius_overrides_redux_sharp_baseline() {
    let mermaid = MermaidThemeCompatibility::default()
        .with_theme("redux")
        .expect("valid Redux Mermaid theme");
    let theme = timeline_event_rules_theme_with_mermaid(
        mermaid,
        [ThemeRule::new(
            ThemeTarget::TimelineEvent,
            timeline_event_radius_style(Specified::Value(12.0)),
        )],
    );
    let svg = try_render_timeline_with_theme_requirement(
        "timeline\n    section Release\n        2026 : Ship\n",
        &theme,
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("Redux compatibility may remain a best-effort residual")
    .svg()
    .to_owned();

    assert!(
        svg.contains("q0,-12 12,-12"),
        "explicit Redux radius missing: {svg}"
    );
    assert!(
        !svg.contains("q0,-5 5,-5"),
        "Redux baseline radius must not leak into explicit geometry: {svg}"
    );
}

#[test]
fn timeline_redux_event_radius_clear_preserves_sharp_geometry_and_terminal_evidence() {
    let mermaid = MermaidThemeCompatibility::default()
        .with_theme("redux")
        .expect("valid Redux Mermaid theme");
    let theme = timeline_event_rules_theme_with_mermaid(
        mermaid,
        [ThemeRule::new(
            ThemeTarget::TimelineEvent,
            timeline_event_radius_style(Specified::Clear),
        )],
    );
    let rendered = try_render_timeline_with_theme_requirement(
        "timeline\n    section Release\n        2026 : Ship\n",
        &theme,
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("Redux Clear radius must render in best-effort mode");
    let svg = rendered.svg().to_owned();
    let document = roxmltree::Document::parse(&svg).expect("valid Redux Timeline SVG XML");
    let event_path = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path") && node.attribute("class") == Some("node-bkg node-undefined")
        })
        .nth(1)
        .expect("Redux Timeline event path");
    let path = event_path.attribute("d").expect("Redux event path data");
    assert!(
        !path.split_whitespace().any(|token| token.starts_with('q')),
        "Redux Clear radius must preserve sharp geometry: {path}"
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn timeline_static_event_radius_preserves_f32_token_and_normalizes_negative_zero() {
    let fractional = render_timeline_svg_with_theme(
        "timeline\n    2026 : Ship\n",
        &timeline_event_radius_theme(Specified::Value(0.1)),
    );
    assert!(
        fractional.contains("q0,-0.1 0.1,-0.1"),
        "fractional radius should keep its authored f32 token: {fractional}"
    );
    assert!(
        !fractional.contains("0.100000001"),
        "fractional radius geometry must use the authored canonical token: {fractional}"
    );

    let negative_zero = render_timeline_svg_with_theme(
        "timeline\n    2026 : Ship\n",
        &timeline_event_radius_theme(Specified::Value(-0.0)),
    );
    let negative_zero_document =
        roxmltree::Document::parse(&negative_zero).expect("valid negative-zero Timeline SVG");
    let negative_zero_path = negative_zero_document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("eventWrapper"))
        .and_then(|wrapper| {
            wrapper.descendants().find(|node| {
                node.has_tag_name("path")
                    && node.attribute("class") == Some("node-bkg node-undefined")
            })
        })
        .and_then(|path| path.attribute("d"))
        .expect("Timeline event path for negative zero");
    assert!(
        !negative_zero_path.contains("--0") && !negative_zero_path.contains("-0"),
        "negative zero must be normalized throughout the SVG path: {negative_zero_path}"
    );
}

#[test]
fn timeline_oversized_event_radius_is_clamped_in_horizontal_geometry() {
    let svg = render_timeline_svg_with_theme(
        "timeline\n    section Release\n        2026 : Ship\n",
        &timeline_event_radius_theme(Specified::Value(1_000.0)),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid oversized-radius Timeline SVG");
    let event_path = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("eventWrapper"))
        .and_then(|wrapper| {
            wrapper.descendants().find(|node| {
                node.has_tag_name("path")
                    && node.attribute("class") == Some("node-bkg node-undefined")
            })
        })
        .expect("Timeline event path");
    let path = event_path.attribute("d").expect("event path data");

    assert!(!path.contains("1000"), "radius must be clamped: {path}");
    assert!(
        !path.contains("h-"),
        "clamped geometry must not reverse horizontally: {path}"
    );
    assert!(
        path.contains("q0,-"),
        "clamped geometry should retain a rounded corner: {path}"
    );
}

#[test]
fn timeline_oversized_event_radius_is_clamped_in_vertical_geometry() {
    let svg = render_timeline_svg_with_theme(
        "timeline TD\n    section Release\n        2026 : Ship\n",
        &timeline_event_radius_theme(Specified::Value(1_000.0)),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid oversized-radius Timeline SVG");
    let event_path = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("eventWrapper"))
        .and_then(|wrapper| {
            wrapper.descendants().find(|node| {
                node.has_tag_name("path")
                    && node.attribute("class") == Some("node-bkg node-undefined")
            })
        })
        .expect("Timeline event path");
    let path = event_path.attribute("d").expect("event path data");

    assert!(!path.contains("1000"), "radius must be clamped: {path}");
    assert!(
        !path.contains("h-"),
        "clamped geometry must not reverse horizontally: {path}"
    );
    assert!(
        !path.contains("q0,-1000 1000,-1000"),
        "vertical event radius must clamp instead of using the authored value: {path}"
    );
}

#[test]
fn timeline_static_event_radius_and_opacity_share_one_terminal_receipt() {
    let source = "timeline\n    section Release\n        2026 : Ship\n";
    let theme = timeline_event_rules_theme([ThemeRule::new(
        ThemeTarget::TimelineEvent,
        ThemeStylePatch {
            geometry: ThemeGeometryPatch {
                radius: Specified::Value(12.0),
            },
            paint: ThemePaintPatch {
                opacity: Specified::Value(0.5),
                ..ThemePaintPatch::default()
            },
            ..ThemeStylePatch::default()
        },
    )]);
    let rendered = try_render_timeline_with_theme(source, &theme)
        .expect("mixed Timeline event terminal theme must render");
    let svg = rendered.svg().to_owned();
    assert!(
        svg.contains("q0,-12 12,-12"),
        "radius was not emitted: {svg}"
    );
    assert!(
        svg.contains(r#"class="eventWrapper" opacity="0.5""#),
        "opacity was not emitted: {svg}"
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn timeline_event_opacity_rejects_qualified_and_mixed_rules() {
    let source = "timeline\n    2026 : Ship\n";
    let opacity_style = timeline_event_opacity_style(Specified::Value(0.5));
    let unsupported_rules = [
        ThemeRule::new(ThemeTarget::TimelineEvent, opacity_style.clone())
            .with_variant(ThemeVariant::Default),
        ThemeRule::new(ThemeTarget::TimelineEvent, opacity_style.clone())
            .with_variant(ThemeVariant::Active),
        ThemeRule::new(ThemeTarget::TimelineEvent, opacity_style.clone())
            .with_ordinal(OrdinalSelector::exact(1).expect("valid Timeline event ordinal")),
        ThemeRule::new(
            ThemeTarget::TimelineEvent,
            ThemeStylePatch {
                paint: ThemePaintPatch {
                    fill_opacity: Specified::Value(0.5),
                    ..ThemePaintPatch::default()
                },
                ..ThemeStylePatch::default()
            },
        ),
        ThemeRule::new(
            ThemeTarget::TimelineEvent,
            opacity_style.with_padding(InsetsPx::all(4.0)),
        ),
    ];

    for rule in unsupported_rules {
        let error = try_render_timeline_svg_with_theme(source, &timeline_event_rules_theme([rule]))
            .expect_err("unsupported Timeline event routes must fail closed");
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::TIMELINE, 1))
        );
    }

    let theme = timeline_event_rules_theme([
        ThemeRule::new(
            ThemeTarget::TimelineEvent,
            timeline_event_opacity_style(Specified::Value(0.5)),
        ),
        ThemeRule::new(
            ThemeTarget::TimelineEvent,
            timeline_event_opacity_style(Specified::Value(0.9)),
        )
        .with_variant(ThemeVariant::Warning)
        .with_ordinal(
            OrdinalSelector::exact(999).expect("valid out-of-range Timeline event ordinal"),
        ),
    ]);
    let svg = render_timeline_svg_with_theme(source, &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid themed Timeline SVG XML");
    let wrapper = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("eventWrapper"))
        .expect("Timeline event wrapper");
    assert_eq!(wrapper.attribute("opacity"), Some("0.5"));
}

#[test]
fn timeline_event_radius_rejects_qualified_ordinal_and_mixed_rules() {
    let source = "timeline\n    2026 : Ship\n";
    let radius_style = timeline_event_radius_style(Specified::Value(12.0));
    let unsupported_rules = [
        ThemeRule::new(ThemeTarget::TimelineEvent, radius_style.clone())
            .with_variant(ThemeVariant::Default),
        ThemeRule::new(ThemeTarget::TimelineEvent, radius_style.clone())
            .with_ordinal(OrdinalSelector::exact(1).expect("valid Timeline event ordinal")),
        ThemeRule::new(
            ThemeTarget::TimelineEvent,
            ThemeStylePatch {
                geometry: ThemeGeometryPatch {
                    radius: Specified::Value(12.0),
                },
                paint: ThemePaintPatch {
                    fill_opacity: Specified::Value(0.5),
                    ..ThemePaintPatch::default()
                },
                ..ThemeStylePatch::default()
            },
        ),
        ThemeRule::new(
            ThemeTarget::TimelineEvent,
            radius_style.with_padding(InsetsPx::all(4.0)),
        ),
    ];

    for rule in unsupported_rules {
        let error = try_render_timeline_svg_with_theme(source, &timeline_event_rules_theme([rule]))
            .expect_err("unsupported Timeline radius routes must fail closed");
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::TIMELINE, 1))
        );
    }
}

#[test]
fn timeline_event_opacity_is_not_applicable_without_events() {
    let svg = render_timeline_svg_with_theme(
        "timeline\n    section Release\n        2026\n",
        &timeline_event_opacity_theme(Specified::Value(0.5)),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid event-free Timeline SVG XML");
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("eventWrapper"))
            .count(),
        0
    );
}
