mod common;

use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramEffectSet, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec,
    EffectBinding, EffectGraph, EffectInput, EffectPrimitive, Specified, TextStylePatch,
    ThemeColorValue, ThemeGeometryPatch, ThemePortabilityRequirement, ThemeResourceLimitId,
    ThemeResourcePolicy, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn state_edge_data_points(svg: &str, edge_id: &str) -> Vec<merman_render::model::LayoutPoint> {
    use base64::Engine as _;

    let document = roxmltree::Document::parse(svg).expect("valid State SVG XML");
    let encoded = document
        .descendants()
        .find(|node| node.has_tag_name("path") && node.attribute("data-id") == Some(edge_id))
        .and_then(|node| node.attribute("data-points"))
        .unwrap_or_else(|| panic!("missing data-points for State edge {edge_id}"));
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .expect("base64 State edge points");
    serde_json::from_slice(&decoded).expect("JSON State edge points")
}

fn state_edge_label_position(svg: &str, edge_id: &str) -> (f64, f64) {
    let document = roxmltree::Document::parse(svg).expect("valid State SVG XML");
    let label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some(edge_id)
                && node
                    .attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|class| class == "label"))
        })
        .unwrap_or_else(|| panic!("missing State edge label {edge_id}"));
    let transform = label
        .parent()
        .and_then(|node| node.attribute("transform"))
        .unwrap_or_else(|| panic!("missing outer transform for State edge label {edge_id}"));
    let components = transform
        .strip_prefix("translate(")
        .and_then(|value| value.strip_suffix(')'))
        .and_then(|value| value.split_once(','))
        .unwrap_or_else(|| panic!("invalid State edge label transform: {transform}"));
    (
        components.0.trim().parse().expect("numeric label x"),
        components.1.trim().parse().expect("numeric label y"),
    )
}

fn svg_view_box(svg: &str) -> [f64; 4] {
    let document = roxmltree::Document::parse(svg).expect("valid SVG XML");
    let values = document
        .root_element()
        .attribute("viewBox")
        .expect("root viewBox")
        .split_whitespace()
        .map(|value| value.parse::<f64>().expect("numeric viewBox component"))
        .collect::<Vec<_>>();
    values.try_into().expect("four-component viewBox")
}

fn state_diagram_title_x(svg: &str) -> f64 {
    let document = roxmltree::Document::parse(svg).expect("valid State SVG XML");
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("text") && node.attribute("class") == Some("statediagramTitleText")
        })
        .and_then(|node| node.attribute("x"))
        .expect("State diagram title x")
        .parse()
        .expect("numeric State diagram title x")
}

fn state_effect_filter_regions(svg: &str) -> Vec<(f64, f64, [f64; 4])> {
    let document = roxmltree::Document::parse(svg).expect("valid themed State SVG XML");
    let mut regions = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && node.attribute("class") == Some("basic label-container")
                && node.attribute("filter").is_some()
        })
        .map(|rect| {
            let filter_id = rect
                .attribute("filter")
                .and_then(|value| value.strip_prefix("url(#"))
                .and_then(|value| value.strip_suffix(')'))
                .expect("State rect typed filter reference");
            let filter = document
                .descendants()
                .find(|node| node.has_tag_name("filter") && node.attribute("id") == Some(filter_id))
                .expect("referenced State filter definition");
            let parse = |node: roxmltree::Node<'_, '_>, name: &str| {
                node.attribute(name)
                    .unwrap_or_else(|| panic!("missing {name} on {}", node.tag_name().name()))
                    .parse::<f64>()
                    .unwrap_or_else(|_| panic!("numeric {name} on {}", node.tag_name().name()))
            };
            (
                parse(rect, "width"),
                parse(rect, "height"),
                [
                    parse(filter, "x"),
                    parse(filter, "y"),
                    parse(filter, "width"),
                    parse(filter, "height"),
                ],
            )
        })
        .collect::<Vec<_>>();
    regions.sort_by(|left, right| left.0.total_cmp(&right.0));
    regions
}

fn state_hard_shadow_theme(offset_x: f32, offset_y: f32, stroke_width: f32) -> DiagramTheme {
    state_hard_shadow_theme_with(
        DiagramThemeCompiler::new(),
        offset_x,
        offset_y,
        Some(
            ThemeStylePatch::default()
                .with_stroke_width(stroke_width)
                .expect("valid State stroke width"),
        ),
    )
}

fn state_hard_shadow_theme_with(
    compiler: DiagramThemeCompiler,
    offset_x: f32,
    offset_y: f32,
    style: Option<ThemeStylePatch>,
) -> DiagramTheme {
    state_drop_shadow_theme_with(
        compiler,
        "state-hard-shadow",
        offset_x,
        offset_y,
        0.0,
        style,
    )
}

fn state_drop_shadow_theme_with(
    compiler: DiagramThemeCompiler,
    effect_id: &str,
    offset_x: f32,
    offset_y: f32,
    blur_radius: f32,
    style: Option<ThemeStylePatch>,
) -> DiagramTheme {
    let graph = EffectGraph::new(
        effect_id,
        [EffectPrimitive::DropShadow {
            input: EffectInput::SourceGraphic,
            offset_x,
            offset_y,
            blur_radius,
            spread: 0.0,
            color: ThemeColorValue::parse("#111827").expect("valid shadow color"),
        }],
    )
    .expect("valid bounded drop-shadow graph");
    let effects = DiagramEffectSet::default()
        .with_graph(graph)
        .expect("unique effect graph")
        .with_binding(
            EffectBinding::new(ThemeTarget::State, effect_id).expect("valid State effect binding"),
        )
        .expect("unique State effect binding");
    let mut spec = DiagramThemeSpec::new().with_effects(effects);
    if let Some(style) = style {
        spec = spec.with_styles(
            ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::State, style)),
        );
    }
    compiler
        .compile(spec)
        .expect("compile State drop-shadow theme")
}

fn assert_filter_region_matches_outsets(
    width: f64,
    height: f64,
    region: [f64; 4],
    expected: [f64; 4],
) {
    let actual = [
        -region[1] * height,
        (region[0] + region[2] - 1.0) * width,
        (region[1] + region[3] - 1.0) * height,
        -region[0] * width,
    ];
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert!(
            actual >= expected - 1.0e-5 && actual <= expected + 1.0e-3,
            "expected outset {expected}, got {actual} from region {region:?}"
        );
    }
}

fn render_state_svg_from_text(text: &str) -> String {
    render_state_svg_from_text_with_engine(Engine::new(), text)
}

fn render_state_svg_from_text_with_engine(engine: Engine, text: &str) -> String {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare State artifact");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render State artifact")
        .svg()
        .to_string()
}

fn try_render_state_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin State resource-bound session");
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse State resource-bound fixture")
        .expect("detect State resource-bound fixture");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn state_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = "stateDiagram-v2\n[*] --> Ready\nReady --> Done: advance\nDone --> [*]\n";
    let baseline = try_render_state_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded State baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "State fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact State SVG byte ceiling");
    let exact = try_render_state_svg_with_resource_policy(source, exact_policy)
        .expect("the exact State family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact State SVG byte ceiling");
    let error = try_render_state_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the State family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected State MaxSvgBytes rejection, got {error}");
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

fn render_state_svg_from_text_with_theme(text: &str, theme: &DiagramTheme) -> String {
    render_state_svg_from_text_with_theme_in_environment(
        text,
        theme,
        &RenderEnvironment::deterministic(),
    )
}

fn render_state_svg_from_text_with_theme_in_environment(
    text: &str,
    theme: &DiagramTheme,
    environment: &RenderEnvironment,
) -> String {
    let session = environment.begin_session_with_theme(theme).unwrap();
    let parsed = merman_render::__private::install_parse_compatibility(theme, Engine::new())
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare themed State artifact");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render themed State artifact")
        .svg()
        .to_string()
}

fn state_note_radius_theme(radius: Specified<f32>) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Note,
                ThemeStylePatch {
                    geometry: ThemeGeometryPatch { radius },
                    ..ThemeStylePatch::default()
                },
            ))),
        )
        .expect("compile State note radius theme")
}

fn state_note_rough_paths(svg: &str) -> Vec<String> {
    let document = roxmltree::Document::parse(svg).expect("valid State SVG XML");
    let note_label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class").is_some_and(|classes| {
                    classes.split_whitespace().any(|class| class == "noteLabel")
                })
        })
        .expect("State note label group");
    let note_node = note_label.parent().expect("State note node group");
    note_node
        .children()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|class| class == "outer-path")
                })
        })
        .expect("State note outer path group")
        .children()
        .filter(|node| node.has_tag_name("path"))
        .map(|node| {
            node.attribute("d")
                .expect("State note rough path")
                .to_string()
        })
        .collect()
}

#[test]
fn state_note_positive_radius_changes_rough_geometry() {
    let source = "stateDiagram-v2\nA\nnote right of A : rounded note\n";
    let baseline = render_state_svg_from_text(source);
    let rounded = render_state_svg_from_text_with_theme(
        source,
        &state_note_radius_theme(Specified::Value(12.0)),
    );

    assert_ne!(
        state_note_rough_paths(&rounded),
        state_note_rough_paths(&baseline)
    );
}

#[test]
fn state_note_zero_radius_preserves_square_rough_path_bytes() {
    let source = "stateDiagram-v2\nA\nnote right of A : square note\n";
    let baseline = render_state_svg_from_text(source);
    let explicit_zero = render_state_svg_from_text_with_theme(
        source,
        &state_note_radius_theme(Specified::Value(0.0)),
    );

    assert_eq!(
        state_note_rough_paths(&explicit_zero),
        state_note_rough_paths(&baseline)
    );
}

#[test]
fn state_note_radius_clamps_to_half_the_shortest_side() {
    let source = "stateDiagram-v2\nA\nnote right of A : clamped note\n";
    let large = render_state_svg_from_text_with_theme(
        source,
        &state_note_radius_theme(Specified::Value(10_000.0)),
    );
    let larger = render_state_svg_from_text_with_theme(
        source,
        &state_note_radius_theme(Specified::Value(100_000.0)),
    );

    assert_eq!(
        state_note_rough_paths(&large),
        state_note_rough_paths(&larger)
    );
}

#[test]
fn state_note_clear_radius_remains_strictly_unsupported() {
    let theme = state_note_radius_theme(Specified::Clear);
    let source = "stateDiagram-v2\nA\nnote right of A : unsupported clear\n";
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable State note session");
    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse strict State note source")
        .expect("State diagram detected");
    let error = match family::prepare(parsed, &LayoutOptions::default(), session) {
        Ok(_) => panic!("clearing State Note radius must fail closed in portable mode"),
        Err(error) => error,
    };

    assert!(
        matches!(
            error,
            merman_render::Error::UnverifiedFamilyTheme {
                family_id: merman_core::DiagramFamilyId::STATE,
                residual_count: 1,
            }
        ),
        "unexpected strict State Note radius error: {error:?}"
    );
}

#[test]
fn state_svg_derives_safe_typed_drop_shadow_regions_for_classic_state_nodes() {
    let theme = state_hard_shadow_theme(5.0, 6.0, 4.0);
    let source = "stateDiagram-v2\n[*] --> Ready\nReady --> ExtraordinarilyWideStateName\nExtraordinarilyWideStateName --> [*]\n";
    let svg = render_state_svg_from_text_with_theme(source, &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid themed State SVG XML");
    let filters = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("filter")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.ends_with("-theme-effect-state-hard-shadow"))
        })
        .collect::<Vec<_>>();
    assert_eq!(filters.len(), 2);
    let filter_ids = filters
        .iter()
        .map(|filter| {
            let primitives = filter
                .children()
                .filter(|node| node.is_element())
                .collect::<Vec<_>>();
            assert_eq!(primitives.len(), 1);
            let shadow = primitives[0];
            assert!(shadow.has_tag_name("feDropShadow"));
            assert_eq!(shadow.attribute("in"), Some("SourceGraphic"));
            assert_eq!(shadow.attribute("dx"), Some("5"));
            assert_eq!(shadow.attribute("dy"), Some("6"));
            assert_eq!(shadow.attribute("stdDeviation"), Some("0"));
            assert_eq!(shadow.attribute("flood-color"), Some("#111827"));
            filter.attribute("id").expect("filter id")
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(filter_ids.len(), 2);
    let referenced_filter_ids = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect") && node.attribute("class") == Some("basic label-container")
        })
        .map(|node| {
            node.attribute("filter")
                .and_then(|value| value.strip_prefix("url(#"))
                .and_then(|value| value.strip_suffix(')'))
                .expect("State rect must reference one typed hard-shadow filter")
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(referenced_filter_ids, filter_ids);

    let regions = state_effect_filter_regions(&svg);
    assert_eq!(regions.len(), 2);
    assert!(regions[0].0 < regions[1].0, "node widths must differ");
    for (width, height, region) in &regions {
        let expected_x = -2.0 / width;
        let expected_y = -2.0 / height;
        let expected_max_x = 1.0 + 7.0 / width;
        let expected_max_y = 1.0 + 8.0 / height;
        assert!(region[0] <= expected_x + 1.0e-6, "{region:?}");
        assert!(region[1] <= expected_y + 1.0e-6, "{region:?}");
        assert!(
            region[0] + region[2] >= expected_max_x - 1.0e-6,
            "{region:?}"
        );
        assert!(
            region[1] + region[3] >= expected_max_y - 1.0e-6,
            "{region:?}"
        );
    }
    assert_ne!(regions[0].2, regions[1].2);

    let baseline_theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::State,
                    ThemeStylePatch::default()
                        .with_stroke_width(4.0)
                        .expect("valid comparison stroke width"),
                )),
            ),
        )
        .expect("compile comparison theme");
    let baseline_view_box = svg_view_box(&render_state_svg_from_text_with_theme(
        source,
        &baseline_theme,
    ));
    let effect_view_box = svg_view_box(&svg);
    assert!(effect_view_box[2] >= baseline_view_box[2] + 9.0 - 1.0e-6);
    assert!(effect_view_box[3] >= baseline_view_box[3] + 10.0 - 1.0e-6);

    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable State session");
    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse strict State source")
        .expect("State diagram detected");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare strict portable State artifact");
    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("typed hard shadow should satisfy strict family portability");

    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable State session without edges");
    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse strict State source without edges")
        .expect("State diagram detected");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare strict portable State artifact without edges");
    let mut debug = SvgDebugOptions::default();
    debug.include_edges = false;
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &debug)
        .expect("hidden edges must not invalidate emitted State node effects");
    assert!(rendered.svg().contains("theme-effect-state-hard-shadow"));
}

#[test]
fn state_svg_soft_shadow_uses_four_sigma_regions_and_expands_the_root_viewbox() {
    let theme = state_drop_shadow_theme_with(
        DiagramThemeCompiler::new(),
        "state-soft-shadow",
        0.0,
        0.0,
        8.0,
        Some(
            ThemeStylePatch::default()
                .with_stroke_width(2.0)
                .expect("valid State stroke width"),
        ),
    );
    let source = "stateDiagram-v2\n[*] --> Ready\nReady --> ExtraordinarilyWideStateName\nExtraordinarilyWideStateName --> [*]\n";
    let svg = render_state_svg_from_text_with_theme(source, &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid soft-shadow State SVG XML");

    let filters = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("filter")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.ends_with("-theme-effect-state-soft-shadow"))
        })
        .collect::<Vec<_>>();
    assert_eq!(filters.len(), 2);
    assert!(filters.iter().all(|filter| {
        filter
            .children()
            .find(|node| node.is_element() && node.has_tag_name("feDropShadow"))
            .is_some_and(|shadow| shadow.attribute("stdDeviation") == Some("8"))
    }));

    let regions = state_effect_filter_regions(&svg);
    assert_eq!(regions.len(), 2);
    assert!(regions[0].0 < regions[1].0, "node widths must differ");
    for (width, height, region) in regions {
        assert_filter_region_matches_outsets(width, height, region, [33.0; 4]);
    }

    let baseline_theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::State,
                    ThemeStylePatch::default()
                        .with_stroke_width(2.0)
                        .expect("valid comparison stroke width"),
                )),
            ),
        )
        .expect("compile comparison theme");
    let baseline_view_box = svg_view_box(&render_state_svg_from_text_with_theme(
        source,
        &baseline_theme,
    ));
    let effect_view_box = svg_view_box(&svg);
    assert!(effect_view_box[2] >= baseline_view_box[2] + 66.0 - 1.0e-6);
    assert!(effect_view_box[3] >= baseline_view_box[3] + 66.0 - 1.0e-6);

    let strict_svg = render_state_svg_from_text_with_theme_in_environment(
        source,
        &theme,
        &RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable),
    );
    assert!(strict_svg.contains(r#"stdDeviation="8""#));
}

#[test]
fn state_svg_shadow_expands_paint_bounds_without_shifting_the_diagram_title() {
    let source = r#"---
title: State
---
stateDiagram-v2
[*] --> Ready
Ready --> [*]
"#;
    let baseline_theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::State,
                    ThemeStylePatch::default()
                        .with_stroke_width(2.0)
                        .expect("valid comparison stroke width"),
                )),
            ),
        )
        .expect("compile State comparison theme");
    let shadow_theme = state_hard_shadow_theme(48.0, 0.0, 2.0);

    let baseline_svg = render_state_svg_from_text_with_theme(source, &baseline_theme);
    let shadow_svg = render_state_svg_from_text_with_theme(source, &shadow_theme);
    assert_eq!(
        state_diagram_title_x(&shadow_svg),
        state_diagram_title_x(&baseline_svg),
        "paint-only shadow outsets must not move the geometry-centered diagram title"
    );

    let baseline_view_box = svg_view_box(&baseline_svg);
    let shadow_view_box = svg_view_box(&shadow_svg);
    assert!(
        shadow_view_box[2] > baseline_view_box[2] + 40.0,
        "the positive-x shadow must expand the root paint viewport: baseline={baseline_view_box:?}, shadow={shadow_view_box:?}"
    );
    assert!(
        shadow_view_box[0] + shadow_view_box[2]
            > baseline_view_box[0] + baseline_view_box[2] + 40.0,
        "the positive-x shadow must expand the painted right edge: baseline={baseline_view_box:?}, shadow={shadow_view_box:?}"
    );
}

#[test]
fn state_svg_derives_safe_regions_for_negative_hard_shadow_offsets() {
    let theme = state_hard_shadow_theme(-5.0, -6.0, 2.0);
    let source = "stateDiagram-v2\n[*] --> Compact\nCompact --> ASignificantlyWiderState\nASignificantlyWiderState --> [*]\n";
    let svg = render_state_svg_from_text_with_theme(source, &theme);
    let regions = state_effect_filter_regions(&svg);
    assert_eq!(regions.len(), 2);
    assert!(regions[0].0 < regions[1].0, "node widths must differ");
    for (width, height, region) in regions {
        let expected_x = -6.0 / width;
        let expected_y = -7.0 / height;
        let expected_max_x = 1.0 + 1.0 / width;
        let expected_max_y = 1.0 + 1.0 / height;
        assert!(region[0] <= expected_x + 1.0e-6, "{region:?}");
        assert!(region[1] <= expected_y + 1.0e-6, "{region:?}");
        assert!(
            region[0] + region[2] >= expected_max_x - 1.0e-6,
            "{region:?}"
        );
        assert!(
            region[1] + region[3] >= expected_max_y - 1.0e-6,
            "{region:?}"
        );
    }
}

#[test]
fn state_svg_uses_the_classic_default_one_pixel_stroke_for_effect_bounds() {
    let theme = state_hard_shadow_theme_with(DiagramThemeCompiler::new(), 5.0, 6.0, None);
    let svg = render_state_svg_from_text_with_theme(
        "stateDiagram-v2\n[*] --> Ready\nReady --> [*]\n",
        &theme,
    );
    let regions = state_effect_filter_regions(&svg);
    let [(width, height, region)] = regions.as_slice() else {
        panic!("expected one materialized State hard-shadow region");
    };

    assert_filter_region_matches_outsets(*width, *height, *region, [0.5, 5.5, 6.5, 0.5]);
}

#[test]
fn state_svg_excludes_non_painting_strokes_from_effect_bounds() {
    let theme = state_hard_shadow_theme_with(DiagramThemeCompiler::new(), 5.0, 6.0, None);
    for source_style in [
        "stroke:none,stroke-width:100px",
        "stroke:transparent,stroke-width:100px",
        "stroke:#111827,stroke-width:100px,stroke-opacity:0",
        "stroke:#111827,stroke-width:100px,stroke-opacity:-1",
        "stroke:#111827,stroke-width:100px,opacity:0%",
        "stroke:#111827,stroke-width:100px,opacity:-1%",
    ] {
        let source =
            format!("stateDiagram-v2\n[*] --> Ready\nReady --> [*]\nstyle Ready {source_style}\n");
        let svg = render_state_svg_from_text_with_theme(&source, &theme);
        let regions = state_effect_filter_regions(&svg);
        let [(width, height, region)] = regions.as_slice() else {
            panic!("expected one materialized State hard-shadow region for {source_style}");
        };
        assert_filter_region_matches_outsets(*width, *height, *region, [0.0, 5.0, 6.0, 0.0]);
    }
}

#[test]
fn state_svg_terminal_effect_region_uses_the_session_resource_intersection() {
    let theme = state_hard_shadow_theme_with(
        DiagramThemeCompiler::new()
            .with_resource_policy(ThemeResourcePolicy::unbounded_for_trusted_input()),
        4096.0,
        0.0,
        None,
    );
    let source = "stateDiagram-v2\n[*] --> Ready\nReady --> [*]\n";
    let host_ceiling = ThemeResourcePolicy::interactive()
        .with_limit(ThemeResourceLimitId::MaxEffectFilterRegionMagnitude, 2)
        .expect("valid host terminal filter-region ceiling");
    let environment = RenderEnvironment::deterministic().with_theme_resource_ceiling(host_ceiling);

    for portability in [
        ThemePortabilityRequirement::BestEffort,
        ThemePortabilityRequirement::RequirePortable,
    ] {
        let session = environment
            .clone()
            .with_theme_portability_requirement(portability)
            .begin_session_with_theme(&theme)
            .expect("begin State session");
        let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::default())
            .expect("parse State source")
            .expect("State diagram detected");
        let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
            .expect("prepare State artifact");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("terminal effect materialization must preserve resource rejection"),
                Err(error) => error,
            };
        let merman_render::Error::ThemeResourceLimitExceeded(resource) = error else {
            panic!("terminal theme limits must not degrade into portability residuals: {error:?}");
        };
        assert_eq!(
            resource.limit,
            ThemeResourceLimitId::MaxEffectFilterRegionMagnitude.as_str()
        );
        assert_eq!(resource.phase.as_str(), "effect_materialize");
        assert!(resource.actual > resource.max);
        assert_eq!(resource.max, 2);
        assert_eq!(
            resource.profile,
            Some(merman_core::resources::ResourceProfile::Interactive)
        );
        assert!(resource.explicit_overrides.iter().any(|resource_override| {
            resource_override.id == ThemeResourceLimitId::MaxEffectFilterRegionMagnitude
                && resource_override.value == 2
        }));
    }
}

fn state_edge_label_foreign_object_height(svg: &str, edge_id: &str) -> f64 {
    let document = roxmltree::Document::parse(svg).expect("valid State SVG XML");
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some(edge_id)
                && node
                    .attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|class| class == "label"))
        })
        .and_then(|label| {
            label
                .descendants()
                .find(|node| node.has_tag_name("foreignObject"))
        })
        .and_then(|node| node.attribute("height"))
        .and_then(|height| height.parse::<f64>().ok())
        .unwrap_or_else(|| panic!("missing numeric State edge label height for {edge_id}"))
}

fn state_cluster_title_geometry(svg: &str, cluster_id: &str) -> (f64, f64, f64) {
    let document = roxmltree::Document::parse(svg).expect("valid State SVG XML");
    let cluster = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("data-id") == Some(cluster_id))
        .unwrap_or_else(|| panic!("missing State cluster {cluster_id}"));
    let title_height = cluster
        .descendants()
        .find(|node| node.has_tag_name("foreignObject"))
        .and_then(|node| node.attribute("height"))
        .and_then(|height| height.parse::<f64>().ok())
        .unwrap_or_else(|| panic!("missing State cluster title height for {cluster_id}"));
    let outer_y = cluster
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("outer"))
        .and_then(|node| node.attribute("y"))
        .and_then(|y| y.parse::<f64>().ok())
        .unwrap_or_else(|| panic!("missing State cluster outer y for {cluster_id}"));
    let inner_y = cluster
        .children()
        .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("inner"))
        .and_then(|node| node.attribute("y"))
        .and_then(|y| y.parse::<f64>().ok())
        .unwrap_or_else(|| panic!("missing State cluster inner y for {cluster_id}"));
    (title_height, outer_y, inner_y)
}

#[test]
fn state_svg_compiled_theme_styles_structural_roles_and_reflows_transition_labels() {
    let mut label_typography = TextStylePatch::default();
    label_typography.font_size_px = Specified::Value(30.0);
    let mut composite_typography = TextStylePatch::default();
    composite_typography.font_size_px = Specified::Value(40.0);
    let styles = ThemeRuleSet::default()
        .with_rule(ThemeRule::new(
            ThemeTarget::TransitionMarker,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#00f2ff").unwrap())
                .with_stroke(CanvasPaint::solid("#00f2ff").unwrap()),
        ))
        .with_rule(ThemeRule::new(
            ThemeTarget::TransitionLabelBackground,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#051423").unwrap()),
        ))
        .with_rule(ThemeRule::new(
            ThemeTarget::TransitionLabel,
            ThemeStylePatch {
                typography: label_typography,
                ..ThemeStylePatch::default()
            },
        ))
        .with_rule(ThemeRule::new(
            ThemeTarget::CompositeHeader,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ff00aa").unwrap()),
        ))
        .with_rule(ThemeRule::new(
            ThemeTarget::CompositeLabel,
            ThemeStylePatch {
                typography: composite_typography,
                ..ThemeStylePatch::default()
            },
        ))
        .with_rule(
            ThemeRule::new(
                ThemeTarget::SpecialStateInner,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ffffff").unwrap()),
            )
            .with_variant(ThemeVariant::End),
        );
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile State structural theme");
    let source = r#"stateDiagram-v2
state Parent {
  [*] --> Running : a deliberately long transition label
  Running --> [*]
}"#;

    let baseline = render_state_svg_from_text(source);
    let themed = render_state_svg_from_text_with_theme(source, &theme);

    assert!(themed.contains("fill:#00f2ff !important"), "{themed}");
    assert!(
        themed.contains("background-color: #051423 !important"),
        "{themed}"
    );
    assert!(themed.contains("fill:#ff00aa !important"), "{themed}");
    assert!(themed.contains("fill:#ffffff !important"), "{themed}");
    assert!(themed.contains("font-size: 30px !important"), "{themed}");
    assert!(
        state_edge_label_foreign_object_height(&themed, "edge0")
            > state_edge_label_foreign_object_height(&baseline, "edge0")
    );
    let (baseline_title_height, _, _) = state_cluster_title_geometry(&baseline, "Parent");
    let (themed_title_height, themed_outer_y, themed_inner_y) =
        state_cluster_title_geometry(&themed, "Parent");
    assert!(themed_title_height > baseline_title_height);
    assert!((themed_inner_y - themed_outer_y - themed_title_height - 2.0).abs() < 1e-6);
}

fn render_state_svg_with_hand_drawn_seed(seed: u64) -> String {
    let site_config = MermaidConfig::from_value(serde_json::json!({
        "look": "handDrawn",
        "handDrawnSeed": seed,
        "themeVariables": {
            "stateBkg": "#101827",
            "stateBorder": "#38bdf8",
            "mainBkg": "#0f172a",
            "strokeWidth": 4,
            "specialStateColor": "#f97316",
            "innerEndBackground": "#22c55e",
            "background": "#020617",
            "noteBkgColor": "#fef3c7",
            "noteBorderColor": "#92400e"
        }
    }));
    let source = r#"stateDiagram-v2
[*] --> Idle
state Decide <<choice>>
Idle --> Decide
Decide --> Fork
state Fork <<fork>>
Fork --> Join
state Join <<join>>
Join --> [*]
note right of Idle : seeded note"#;

    render_state_svg_from_text_with_engine(Engine::new().with_site_config(site_config), source)
}

#[test]
fn state_svg_honors_mermaid_11_16_theme_css_options() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "transitionColor": "#202020",
            "lineColor": "#303030",
            "nodeBorder": "#404040",
            "stateLabelColor": "#505050",
            "mainBkg": "#606060",
            "background": "#707070",
            "altBackground": "#808080",
            "strokeWidth": 4,
            "noteBorderColor": "#909090",
            "noteBkgColor": "#a0a0a0",
            "noteTextColor": "#b0b0b0",
            "labelBackgroundColor": "#c0c0c0",
            "edgeLabelBackground": "#d0d0d0",
            "transitionLabelColor": "#e0e0e0",
            "specialStateColor": "#f0f0f0",
            "innerEndBackground": "#010101",
            "compositeBackground": "#020202",
            "stateBkg": "#030303",
            "stateBorder": "#040404",
            "compositeTitleBackground": "#050505"
        }
    })));
    let svg = render_state_svg_from_text_with_engine(
        engine,
        r#"stateDiagram-v2
[*] --> Active: start
Active --> [*]: done"#,
    );

    assert!(
        svg.contains(r#".marker{fill:#303030;stroke:#303030;}"#),
        "expected State base marker CSS to follow lineColor: {svg}"
    );
    assert!(
        svg.contains(r#"defs [id$="-barbEnd"]{fill:#202020;stroke:#202020;}"#),
        "expected State barbEnd marker CSS to follow transitionColor and the prefixed marker id: {svg}"
    );
    assert!(
        svg.contains(r##"[id$="-dependencyStart"],#merman [id$="-dependencyEnd"]{fill:#303030;stroke:#303030;stroke-width:1;}"##),
        "expected State dependency marker CSS to use Mermaid 11.16 suffix selectors: {svg}"
    );
    assert!(
        svg.contains(r#".transition{stroke:#202020;stroke-width:4;fill:none;}"#),
        "expected State transition CSS to follow transitionColor/strokeWidth: {svg}"
    );
    assert!(
        svg.contains(r#".edgeLabel .label text{fill:#e0e0e0;}"#),
        "expected State edge label CSS to follow transitionLabelColor: {svg}"
    );
    assert!(
        svg.contains(r#".node circle.state-start{fill:#f0f0f0;stroke:#f0f0f0;}"#),
        "expected State start/fork CSS to follow specialStateColor: {svg}"
    );
    assert!(
        svg.contains(r#".node rect{fill:#030303;stroke:#040404;stroke-width:4px;}"#),
        "expected State node CSS to follow stateBkg/stateBorder/strokeWidth: {svg}"
    );
    assert!(
        !svg.contains(r#"id="merman-gradient""#) && svg.contains(r#"id="merman-drop-shadow""#),
        "classic state SVG should emit 11.16 drop-shadow defs but not gradient defs unless useGradient is set: {svg}"
    );
    assert!(
        !svg.contains(r#"markerUnits="strokeWidth""#),
        "classic state SVG should keep Mermaid's classic barb marker units: {svg}"
    );
    assert!(
        svg.contains(r#"id="merman-edge0""#)
            && svg.contains(r#"data-look="classic""#)
            && svg.contains(r#"id="merman-state-Active-1""#),
        "classic state DOM should use Mermaid 11.16 scoped ids and explicit data-look: {svg}"
    );
}

#[test]
fn state_svg_neo_look_emits_neo_marker_and_cluster_theme_resources() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "look": "neo",
        "themeVariables": {
            "transitionColor": "#202020",
            "mainBkg": "#606060",
            "stateBorder": "#040404",
            "strokeWidth": 4,
            "useGradient": true,
            "gradientStart": "#112233",
            "gradientStop": "#445566",
            "dropShadow": "url(#drop-shadow)",
            "radius": 3
        }
    })));
    let svg = render_state_svg_from_text_with_engine(
        engine,
        r#"stateDiagram-v2
[*] --> Active: start
state Active {
  Idle --> Busy
}"#,
    );

    assert!(
        svg.contains(r#"<defs><linearGradient id="merman-gradient""#),
        "expected neo state SVG to emit the shared gradient resource: {svg}"
    );
    assert!(
        svg.contains(r#"<filter id="merman-drop-shadow""#),
        "expected neo state SVG to emit the shared drop-shadow resource: {svg}"
    );
    assert!(
        svg.contains(r#"markerUnits="strokeWidth""#)
            && svg.contains(r#"d="M 19,7 L11,14 L13,7 L11,0 Z""#),
        "expected neo state SVG to use Mermaid's neo barb marker geometry: {svg}"
    );
    assert!(
        svg.contains(r#"marker-end="url(#merman_stateDiagram-barbEnd)""#),
        "expected neo state transitions to keep an arrowhead marker: {svg}"
    );
    assert!(
        svg.contains(
            r##"[data-look="neo"].statediagram-cluster rect{fill:#606060;stroke:url(#merman-gradient);stroke-width:4;}"##
        ),
        "expected neo state cluster CSS to reference the scoped gradient: {svg}"
    );
    assert!(
        svg.contains(
            r##"[data-look="neo"].statediagram-cluster rect.outer{rx:3px;ry:3px;filter:url(#merman-drop-shadow);}"##
        ),
        "expected neo state cluster outer rect CSS to reference the scoped drop-shadow and radius: {svg}"
    );
}

#[test]
fn state_svg_hand_drawn_seed_controls_visible_rough_paths() {
    let seed_7 = render_state_svg_with_hand_drawn_seed(7);
    let seed_7_again = render_state_svg_with_hand_drawn_seed(7);
    let seed_8 = render_state_svg_with_hand_drawn_seed(8);

    assert_eq!(
        seed_7, seed_7_again,
        "same handDrawnSeed should keep State rough SVG deterministic"
    );
    assert_ne!(
        seed_7, seed_8,
        "different handDrawnSeed should change visible State rough paths"
    );
    assert!(
        seed_7.contains(r##"fill="#101827""##)
            && seed_7.contains(r##"stroke="#38bdf8" stroke-width="4""##),
        "seed test should exercise ordinary visible rough paths: {seed_7}"
    );
    assert!(
        seed_7.contains(r##"fill="#fef3c7""##)
            && seed_7.contains(r##"stroke="#92400e" stroke-width="1.3""##),
        "seed test should exercise note rough paths as a second visible consumer: {seed_7}"
    );
}

#[test]
fn state_svg_root_html_labels_override_deprecated_flowchart_label_dom() {
    let root_false = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
A --> B: owns
"#,
    );
    let root_true = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": true, "flowchart": {"htmlLabels": false}}}%%
stateDiagram-v2
A --> B: owns
"#,
    );

    assert!(
        root_false.contains(r#"<text y="-10.1""#)
            && root_false.contains(r#"class="text-outer-tspan row""#)
            && root_false.contains(r#"class="text-inner-tspan""#),
        "root htmlLabels=false should render State labels as SVG text: {root_false}"
    );
    assert!(
        !root_false.contains("<foreignObject"),
        "root htmlLabels=false should override deprecated flowchart.htmlLabels=true for simple State label DOM: {root_false}"
    );
    assert!(
        root_true.contains("<foreignObject")
            && root_true.contains(r#"class="nodeLabel markdown-node-label""#)
            && root_true.contains(r#"class="edgeLabel""#),
        "root htmlLabels=true should override deprecated flowchart.htmlLabels=false and keep HTML label DOM: {root_true}"
    );
}

#[test]
fn state_svg_root_html_labels_false_uses_svg_text_for_cluster_titles() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
state Parent {
  A
}
"#,
    );

    assert!(
        svg.contains(r#"class="cluster-label""#)
            && svg.contains(r#"<text y="-10.1""#)
            && svg.contains(r#"class="text-outer-tspan row""#),
        "root htmlLabels=false should render State cluster titles as SVG text: {svg}"
    );
    assert!(
        !svg.contains("<foreignObject"),
        "root htmlLabels=false should override deprecated flowchart.htmlLabels=true for simple State cluster DOM: {svg}"
    );
}

#[test]
fn state_svg_root_html_labels_false_uses_svg_text_for_notes() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
A
note right of A : Note text
"#,
    );

    assert!(
        svg.contains("statediagram-note")
            && svg.contains(
                r#"<tspan font-style="normal" class="text-inner-tspan" font-weight="normal">Note text</tspan>"#
            ),
        "root htmlLabels=false should render State notes as SVG text: {svg}"
    );
    assert!(
        !svg.contains(r#"<span class="nodeLabel"><p>Note text</p></span>"#),
        "root htmlLabels=false should not render State note text through HTML node labels: {svg}"
    );
}

#[test]
fn state_svg_serializes_sanitized_note_images_as_valid_xhtml() {
    let svg = render_state_svg_from_text(
        r#"stateDiagram-v2
A
note right of A
  <a href='https://mermaid.js.org/' target='_blank'><code>note about mermaid</code></a><br/>
  <img src=x onerror=alert(1)>
end note
"#,
    );

    let document = roxmltree::Document::parse(&svg).expect("valid State SVG XML");
    let image = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "img")
        .expect("sanitized note image");
    assert_eq!(image.attribute("src"), Some("x"));
    assert_eq!(
        image.attribute("style"),
        Some("display: flex; flex-direction: column; width: 100%;")
    );
    assert!(image.attribute("onerror").is_none());
}

#[test]
fn state_svg_root_html_labels_false_uses_svg_text_for_rect_with_title() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
Display : Ready
Display : Running
"#,
    );

    assert!(
        svg.contains(r#"title-state"#)
            && svg.contains(r#"<text y="-10.1""#)
            && svg.contains(r#"class="text-outer-tspan row""#)
            && svg.contains("Ready")
            && svg.contains("Running"),
        "root htmlLabels=false should render State rectWithTitle labels as SVG text: {svg}"
    );
    assert!(
        !svg.contains("<foreignObject"),
        "root htmlLabels=false should override deprecated flowchart.htmlLabels=true for State rectWithTitle DOM: {svg}"
    );
}

#[test]
fn state_svg_root_html_labels_false_uses_svg_text_for_empty_edge_labels() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
A --> B
"#,
    );

    assert!(
        svg.contains(r#"class="edgeLabel""#)
            && svg.contains(r#"<g class="label" data-id="edge0" transform="translate(0, 0)"></g>"#),
        "root htmlLabels=false should keep the State empty edge label container: {svg}"
    );
    assert_eq!(
        svg.matches("<foreignObject").count(),
        0,
        "root htmlLabels=false should override deprecated flowchart.htmlLabels=true for empty State edge label DOM: {svg}"
    );
}

#[test]
fn state_svg_root_html_labels_false_uses_svg_text_for_self_loop_edge_labels() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
A --> A: again
"#,
    );

    assert!(
        svg.contains(r#"data-id="edge0""#)
            && svg.contains(
                r#"<tspan font-style="normal" class="text-inner-tspan" font-weight="normal">again</tspan>"#
            ),
        "root htmlLabels=false should render State self-loop labels on the original edge id as SVG text: {svg}"
    );
    assert!(
        !svg.contains("cyclic-special"),
        "Mermaid 11.16 keeps cyclic-special helpers out of the public self-loop edge label DOM: {svg}"
    );
    assert_eq!(
        svg.matches("<foreignObject").count(),
        0,
        "root htmlLabels=false should override deprecated flowchart.htmlLabels=true for State self-loop label DOM: {svg}"
    );
}

#[test]
fn state_svg_leaf_self_loop_keeps_dagre_label_anchor_without_an_explicit_path_update() {
    let svg = render_state_svg_from_text(
        r#"stateDiagram-v2
A --> A: again
"#,
    );

    let points = state_edge_data_points(&svg, "edge0");
    let (_, label_y) = state_edge_label_position(&svg, "edge0");
    let path_max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);

    assert!(
        label_y > path_max_y,
        "a leaf self-loop has no cluster cut, so Mermaid keeps its outside Dagre label anchor: {svg}"
    );
}

#[test]
fn state_svg_composite_self_loop_uses_the_explicitly_updated_cluster_path_for_its_label() {
    let svg = render_state_svg_from_text(
        r#"stateDiagram-v2
state Active {
  Idle
}
Inactive --> Idle: ACT
Active --> Active: LOG
"#,
    );

    let points = state_edge_data_points(&svg, "edge1");
    assert_eq!(points.len(), 4, "expected one compact logical self-loop");
    assert!(
        points[0].x > points[1].x && points[3].x < points[2].x,
        "data-points must retain the endpoint-clipped self-loop geometry: {points:?}"
    );

    let (label_x, label_y) = state_edge_label_position(&svg, "edge1");
    let expected_x = (points[1].x + points[2].x) / 2.0;
    let expected_y = (points[1].y + points[2].y) / 2.0;
    assert!(
        (label_x - expected_x).abs() <= 1e-5 && (label_y - expected_y).abs() <= 1e-5,
        "a composite self-loop cluster cut must move the label to the updated path midpoint: label=({label_x}, {label_y}), expected=({expected_x}, {expected_y})"
    );
}

#[test]
fn state_svg_direct_composite_self_loop_keeps_unclipped_dagre_endpoints() {
    let svg = render_state_svg_from_text(
        r#"stateDiagram-v2
[*] --> Active
state Active {
  [*] --> Ready
  Ready --> Ready : ping
  Ready --> Working : start
  Working --> Working : progress
  Working --> Ready : done
}
Active --> Active : re-enter
Active --> [*] : stop
"#,
    );

    let points = state_edge_data_points(&svg, "edge6");
    assert_eq!(points.len(), 4, "expected one compact composite self-loop");
    assert!(
        (points[0].x - points[1].x).abs() <= 1e-9 && (points[2].x - points[3].x).abs() <= 1e-9,
        "a direct composite endpoint has no node intersect callback, so the Dagre endpoints must remain unclipped: {points:?}"
    );
}

#[test]
fn state_svg_security_level_controls_unsafe_click_href_rendering() {
    let strict = render_state_svg_from_text(
        r#"%%{init: {"securityLevel": "strict"}}%%
stateDiagram-v2
	S1
	S2
	S3
	S4
	click S1 href "javascript:alert(1)"
	click S2 href "jav&#x61;script:alert(2)"
	click S3 href ""
	click S4 href "javascript#colon;alert(4)"
"#,
    );
    assert!(
        strict.contains(r#"<a>"#),
        "expected strict mode to keep Mermaid's anchor wrapper for a declared State link: {strict}"
    );
    assert!(
        !strict.contains(r#"xlink:href="javascript:alert(1)""#),
        "expected strict mode to omit unsafe State click href from SVG: {strict}"
    );
    assert!(
        strict.contains(r#"xlink:href="jav&amp;&amp;x61;script:alert(2)""#),
        "expected strict mode to run Mermaid cleanup and preserve only the browser-safe literal entity spelling: {strict}"
    );
    assert!(
        strict.contains(r#"xlink:href="""#),
        "expected strict mode to preserve an empty DOM href like DOMPurify: {strict}"
    );
    assert!(!strict.contains(r#"target="_blank""#), "{strict}");
    assert!(
        !strict.contains("ﬂ°")
            && !strict.contains("¶ß")
            && !strict.contains(r#"xlink:href="javascript&colon;alert(4)""#),
        "expected strict mode to apply Mermaid cleanup before DOMPurify admission: {strict}"
    );

    let loose = render_state_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }))),
        r#"stateDiagram-v2
S1
click S1 href "javascript:alert(1)"
"#,
    );
    assert!(
        loose.contains(r#"xlink:href="javascript:alert(1)" target="_blank""#),
        "expected loose mode to preserve State click hrefs exactly like Mermaid's link injection path: {loose}"
    );
}

#[test]
fn state_svg_normalizes_truthy_whitespace_tooltips_after_dompurify() {
    let svg = render_state_svg_from_text(
        r#"stateDiagram-v2
S1
click S1 "https://example.test" "   "
"#,
    );

    assert!(svg.contains(r#"title="""#), "{svg}");
    assert!(!svg.contains(r#"title="   ""#), "{svg}");
}

#[test]
fn state_svg_treats_explicit_and_omitted_empty_tooltips_like_mermaid() {
    let svg = render_state_svg_from_text(
        r#"stateDiagram-v2
S1
S2
click S1 "https://example.test/one" ""
click S2 href "https://example.test/two"
"#,
    );

    let explicit = svg
        .find(r#"xlink:href="https://example.test/one""#)
        .expect("explicit-tooltip anchor");
    let explicit_tag = &svg[explicit
        ..svg[explicit..]
            .find('>')
            .map_or(svg.len(), |end| explicit + end)];
    assert!(!explicit_tag.contains(" title="), "{explicit_tag}");

    let omitted = svg
        .find(r#"xlink:href="https://example.test/two""#)
        .expect("href-form anchor");
    let omitted_tag = &svg[omitted
        ..svg[omitted..]
            .find('>')
            .map_or(svg.len(), |end| omitted + end)];
    assert!(!omitted_tag.contains(" title="), "{omitted_tag}");
}

#[test]
fn state_svg_strict_repeated_unsafe_clicks_preserve_nested_wrappers() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"securityLevel": "strict"}}%%
stateDiagram-v2
S1
click S1 "javascript:alert(1)" "JavaScript"
click S1 "data:text/html,unsafe" "Data"
"#,
    );

    let document = roxmltree::Document::parse(&svg).expect("valid State SVG XML");
    let node = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.contains("-state-S1-"))
        })
        .expect("S1 node");
    let inner = node.parent().expect("inner link wrapper");
    let outer = inner.parent().expect("outer link wrapper");

    assert!(inner.has_tag_name("a"), "{svg}");
    assert!(outer.has_tag_name("a"), "{svg}");
    assert_eq!(outer.attribute("title"), Some("JavaScript"));
    assert_eq!(inner.attribute("title"), Some("Data"));
    assert_eq!(node.attribute("title"), Some("Data"));
    for anchor in [outer, inner] {
        assert_eq!(
            anchor.attribute(("http://www.w3.org/1999/xlink", "href")),
            None,
            "{svg}"
        );
        assert_eq!(anchor.attribute("target"), None, "{svg}");
    }
}

#[test]
fn state_svg_loose_repeated_clicks_preserve_each_href_and_target() {
    let svg = render_state_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }))),
        r#"stateDiagram-v2
S1
click S1 "https://example.test/first" "First"
click S1 "https://example.test/last" "Last"
"#,
    );

    let document = roxmltree::Document::parse(&svg).expect("valid State SVG XML");
    let node = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.contains("-state-S1-"))
        })
        .expect("S1 node");
    let inner = node.parent().expect("inner link wrapper");
    let outer = inner.parent().expect("outer link wrapper");

    assert_eq!(outer.attribute("title"), Some("First"));
    assert_eq!(inner.attribute("title"), Some("Last"));
    assert_eq!(node.attribute("title"), Some("Last"));
    assert_eq!(
        outer.attribute(("http://www.w3.org/1999/xlink", "href")),
        Some("https://example.test/first")
    );
    assert_eq!(
        inner.attribute(("http://www.w3.org/1999/xlink", "href")),
        Some("https://example.test/last")
    );
    assert_eq!(outer.attribute("target"), Some("_blank"));
    assert_eq!(inner.attribute("target"), Some("_blank"));
}

#[test]
fn state_svg_empty_later_tooltip_does_not_clear_existing_node_title() {
    let svg = render_state_svg_from_text(
        r#"stateDiagram-v2
S1
click S1 "https://example.test/first" "First"
click S1 "https://example.test/last" ""
"#,
    );

    let document = roxmltree::Document::parse(&svg).expect("valid State SVG XML");
    let node = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.contains("-state-S1-"))
        })
        .expect("S1 node");
    let inner = node.parent().expect("inner link wrapper");
    let outer = inner.parent().expect("outer link wrapper");

    assert_eq!(outer.attribute("title"), Some("First"));
    assert_eq!(inner.attribute("title"), None);
    assert_eq!(node.attribute("title"), Some("First"));
}

#[test]
fn state_svg_honors_theme_options_on_visible_rough_paths() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "stateBkg": "#101827",
            "stateBorder": "#38bdf8",
            "mainBkg": "#0f172a",
            "strokeWidth": 4,
            "specialStateColor": "#f97316",
            "innerEndBackground": "#22c55e",
            "background": "#020617",
            "compositeBackground": "#111827",
            "noteBkgColor": "#fef3c7",
            "noteBorderColor": "#92400e"
        }
    })));
    let svg = render_state_svg_from_text_with_engine(
        engine,
        r#"stateDiagram-v2
[*] --> Idle
state Decide <<choice>>
Idle --> Decide
Decide --> Fork
state Fork <<fork>>
Fork --> Join
state Join <<join>>
Join --> [*]
note right of Idle : themed note"#,
    );

    assert!(
        svg.contains(r##".node rect{fill:#101827;stroke:#38bdf8;stroke-width:4px;}"##),
        "classic ordinary State rects should consume stateBkg/stateBorder/strokeWidth through CSS: {svg}"
    );
    assert!(
        svg.contains(r##"fill="#0f172a""##),
        "choice rough paths should consume mainBkg like Mermaid's State polygon rule: {svg}"
    );
    assert!(
        svg.contains(r##".node circle.state-start{fill:#f97316;stroke:#f97316;}"##),
        "start-state styling should consume specialStateColor: {svg}"
    );
    assert!(
        svg.contains(r##"fill="#22c55e""##) && svg.contains(r##"stroke="#020617""##),
        "end-state inner rough path should consume innerEndBackground/background: {svg}"
    );
    assert!(
        svg.contains(r##"fill="#fef3c7""##)
            && svg.contains(r##"stroke="#92400e" stroke-width="1.3""##),
        "note rough paths should consume noteBkgColor/noteBorderColor: {svg}"
    );
}
