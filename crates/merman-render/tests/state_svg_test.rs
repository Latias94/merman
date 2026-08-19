mod common;

use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramEffectSet, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec,
    EffectBinding, EffectGraph, EffectInput, EffectPrimitive, FontAssetSpec, FontCatalogSpec,
    FontEmbeddingRequirement, FontStack, GenericFontFamily, OrdinalSelector, Specified,
    TextStylePatch, ThemeAssets, ThemeColorValue, ThemeGeometryPatch, ThemePortabilityRequirement,
    ThemeResourceLimitId, ThemeResourcePolicy, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    ThemeTarget, ThemeVariant,
};
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgPipeline, SvgRenderOptions};
use merman_render::text::{
    TextMeasurer, TextMetrics, TextStyle, VendoredFontMetricsTextMeasurer, WrapMode,
};

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

fn assert_state_rough_path_pair(
    container: roxmltree::Node<'_, '_>,
    pair_class: Option<&str>,
    expected_fill: &str,
    expected_stroke: &str,
    expected_stroke_width: &str,
) {
    let pair = pair_class.map_or_else(
        || {
            if container
                .children()
                .filter(|node| node.has_tag_name("path"))
                .count()
                == 2
            {
                container
            } else {
                container
                    .children()
                    .find(|node| {
                        node.has_tag_name("g")
                            && node
                                .children()
                                .filter(|child| child.has_tag_name("path"))
                                .count()
                                == 2
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "missing direct State rough path pair under id={:?} class={:?}",
                            container.attribute("id"),
                            container.attribute("class")
                        )
                    })
            }
        },
        |class| {
            container
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("class").is_some_and(|classes| {
                            classes
                                .split_ascii_whitespace()
                                .any(|candidate| candidate == class)
                        })
                })
                .unwrap_or_else(|| {
                    panic!(
                        "missing State rough path pair {class} under id={:?} class={:?}",
                        container.attribute("id"),
                        container.attribute("class")
                    )
                })
        },
    );
    let paths = pair
        .children()
        .filter(|node| node.has_tag_name("path"))
        .collect::<Vec<_>>();
    let [fill_path, stroke_path] = paths.as_slice() else {
        panic!("expected exactly two direct State rough paths under {pair_class:?}");
    };

    assert_eq!(fill_path.attribute("fill"), Some(expected_fill));
    assert_eq!(fill_path.attribute("stroke"), Some("none"));
    assert_eq!(stroke_path.attribute("fill"), Some("none"));
    assert_eq!(stroke_path.attribute("stroke"), Some(expected_stroke));
    assert_eq!(
        stroke_path.attribute("stroke-width"),
        Some(expected_stroke_width)
    );
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

fn render_state_layout_and_svg_from_text_in_environment(
    text: &str,
    environment: &RenderEnvironment,
) -> (merman_render::model::StateDiagramLayout, String) {
    let session = environment.begin_session().expect("begin State session");
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse State source")
        .expect("detect State diagram");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare State artifact");
    let projection = artifact.layout_json().expect("State layout projection");
    let layout = serde_json::from_value(projection["layout"]["StateDiagramV2"].clone())
        .expect("typed State layout");
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render State artifact")
        .svg()
        .to_string();
    (layout, svg)
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
    render_state_layout_and_svg_from_text_with_theme_in_environment(text, theme, environment).1
}

fn render_state_layout_and_svg_from_text_with_theme_in_environment(
    text: &str,
    theme: &DiagramTheme,
    environment: &RenderEnvironment,
) -> (merman_render::model::StateDiagramLayout, String) {
    let session = environment.begin_session_with_theme(theme).unwrap();
    let parsed = merman_render::__private::install_parse_compatibility(theme, Engine::new())
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare themed State artifact");
    let projection = artifact.layout_json().expect("State layout projection");
    let layout = serde_json::from_value(projection["layout"]["StateDiagramV2"].clone())
        .expect("typed State layout");
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render themed State artifact")
        .svg()
        .to_string();
    (layout, svg)
}

fn render_state_public_and_native_svg_from_text_with_theme_in_environment(
    text: &str,
    theme: &DiagramTheme,
    environment: &RenderEnvironment,
) -> (String, String, usize, bool) {
    let session = environment.begin_session_with_theme(theme).unwrap();
    let parsed = merman_render::__private::install_parse_compatibility(theme, Engine::new())
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare themed State artifact")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render themed State artifact");
    let public_svg = rendered.svg().to_owned();
    let rendered = rendered
        .finalize_resvg(&SvgPipeline::resvg_safe())
        .expect("seal themed State SVG");
    let native_svg = merman_render::__private::native_export_svg(rendered.svg()).to_owned();
    let prepared_label_count = merman_render::__private::prepared_text_label_count(rendered.svg());
    let receipt_matches = merman_render::__private::prepared_text_terminal_receipt(rendered.svg())
        .is_some_and(|receipt| receipt.artifact_matches(&native_svg));

    (
        public_svg,
        native_svg,
        prepared_label_count,
        receipt_matches,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StateThemeEvidenceCounts {
    required: usize,
    accounted: usize,
    applied: usize,
    not_applicable: usize,
    residual: usize,
}

fn render_strict_state_svg_with_theme_and_config(
    theme: &DiagramTheme,
    source: &str,
    config: serde_json::Value,
    origin: &str,
) -> (String, StateThemeEvidenceCounts) {
    let (engine, source) = match origin {
        "site" => (
            Engine::new().with_site_config(MermaidConfig::from_value(config)),
            source.to_string(),
        ),
        "source" => (
            Engine::new()
                .with_site_config(MermaidConfig::from_value(serde_json::json!({"secure": []}))),
            {
                let directive = format!(
                    "%%{{init: {}}}%%\n",
                    serde_json::to_string(&config).expect("serialize State source config")
                );
                source.strip_prefix("---\n").map_or_else(
                    || format!("{directive}{source}"),
                    |frontmatter_body| {
                        frontmatter_body.find("\n---\n").map_or_else(
                            || format!("{directive}{source}"),
                            |closing_offset| {
                                let split = 4 + closing_offset + "\n---\n".len();
                                format!("{}{directive}{}", &source[..split], &source[split..])
                            },
                        )
                    },
                )
            },
        ),
        other => panic!("unsupported State config origin {other}"),
    };
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap_or_else(|error| panic!("parse strict {origin} State source: {error}"))
        .unwrap_or_else(|| panic!("detect strict {origin} State source"));
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .unwrap_or_else(|error| panic!("begin strict {origin} State session: {error}"));
    let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
        .unwrap_or_else(|error| panic!("prepare strict {origin} State artifact: {error}"))
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap_or_else(|error| panic!("render strict {origin} State artifact: {error}"));
    let svg = rendered.svg().to_string();
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    let counts = StateThemeEvidenceCounts {
        required: evidence.required_count(),
        accounted: evidence.accounted_count(),
        applied: evidence.applied_count(),
        not_applicable: evidence.not_applicable_count(),
        residual: evidence.theme_residual_count(),
    };
    (svg, counts)
}

fn state_terminal_owner_config_for_origin(
    mut config: serde_json::Value,
    origin: &str,
) -> serde_json::Value {
    if origin != "source" {
        return config;
    }

    // `theme-base` does not admit `stateBorder` as a source-authored variable. The State writer
    // resolves that terminal through `stateBorder -> nodeBorder`, so the source witness must own
    // the surviving fallback while the site witness continues to exercise the preferred path.
    let theme = config
        .get("theme")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let Some(theme_variables) = config
        .get_mut("themeVariables")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return config;
    };
    let Some(state_border) = theme_variables.remove("stateBorder") else {
        return config;
    };

    assert_eq!(
        theme.as_deref(),
        Some("base"),
        "the State source-owner fallback witness is specific to theme-base"
    );
    assert!(
        theme_variables
            .insert("nodeBorder".to_string(), state_border)
            .is_none(),
        "the State source-owner fixture must select exactly one border fallback"
    );
    config
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

fn state_prepared_transition_label_theme(font_size_px: f32) -> DiagramTheme {
    let font_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_assets(
                    ThemeAssets::default().with_font_catalog(
                        FontCatalogSpec::new([FontAssetSpec::new("excalifont", font_bytes)])
                            .with_generic_family(GenericFontFamily::SansSerif, "Excalifont")
                            .with_embedding_requirement(FontEmbeddingRequirement::FullFont),
                    ),
                )
                .with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::TransitionLabel,
                        ThemeStylePatch {
                            typography: TextStylePatch {
                                font_stack: Specified::Value(
                                    FontStack::single("Excalifont")
                                        .expect("fixture font stack should be valid"),
                                ),
                                font_size_px: Specified::Value(font_size_px),
                                ..TextStylePatch::default()
                            },
                            ..ThemeStylePatch::default()
                        },
                    )),
                ),
        )
        .expect("compile prepared State transition label theme")
}

fn state_prepared_native_label_theme(font_size_px: f32) -> DiagramTheme {
    let font_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    let typography = || TextStylePatch {
        font_stack: Specified::Value(
            FontStack::single("Excalifont").expect("fixture font stack should be valid"),
        ),
        font_size_px: Specified::Value(font_size_px),
        ..TextStylePatch::default()
    };
    let styles = [
        ThemeTarget::StateLabel,
        ThemeTarget::NoteLabel,
        ThemeTarget::CompositeLabel,
        ThemeTarget::TransitionLabel,
    ]
    .into_iter()
    .fold(ThemeRuleSet::default(), |styles, target| {
        styles.with_rule(ThemeRule::new(
            target,
            ThemeStylePatch {
                typography: typography(),
                ..ThemeStylePatch::default()
            },
        ))
    });

    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_assets(
                    ThemeAssets::default().with_font_catalog(
                        FontCatalogSpec::new([FontAssetSpec::new("excalifont", font_bytes)])
                            .with_generic_family(GenericFontFamily::SansSerif, "Excalifont")
                            .with_embedding_requirement(FontEmbeddingRequirement::FullFont),
                    ),
                )
                .with_styles(styles),
        )
        .expect("compile prepared State native-label theme")
}

fn state_transition_label_font_size_theme(font_size_px: f32) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::TransitionLabel,
                ThemeStylePatch {
                    typography: TextStylePatch {
                        font_size_px: Specified::Value(font_size_px),
                        ..TextStylePatch::default()
                    },
                    ..ThemeStylePatch::default()
                },
            ))),
        )
        .expect("compile State transition-label font-size theme")
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

#[test]
fn state_direct_paints_yield_to_terminal_mermaid_owners_from_site_and_source() {
    const MERMAID_COLOR: &str = "#22c55e";
    const TYPED_COLOR: &str = "#ec4899";

    #[derive(Clone, Copy)]
    enum PaintFacet {
        Fill,
        Stroke,
    }

    let cases = [
        (
            "state-fill",
            ThemeTarget::State,
            PaintFacet::Fill,
            "stateBkg",
            "stateDiagram-v2\nReady\n",
            ".node rect{fill:#22c55e;stroke:",
        ),
        (
            "state-stroke",
            ThemeTarget::State,
            PaintFacet::Stroke,
            "stateBorder",
            "stateDiagram-v2\nReady\n",
            "stroke:#22c55e;stroke-width:",
        ),
        (
            "state-label-fill",
            ThemeTarget::StateLabel,
            PaintFacet::Fill,
            "stateLabelColor",
            "stateDiagram-v2\nReady\n",
            ".nodeLabel{color:#22c55e;}",
        ),
        (
            "note-fill",
            ThemeTarget::Note,
            PaintFacet::Fill,
            "noteBkgColor",
            "stateDiagram-v2\nReady\nnote right of Ready : terminal note\n",
            ".statediagram-note rect{fill:#22c55e;stroke:",
        ),
        (
            "note-stroke",
            ThemeTarget::Note,
            PaintFacet::Stroke,
            "noteBorderColor",
            "stateDiagram-v2\nReady\nnote right of Ready : terminal note\n",
            ";stroke:#22c55e;stroke-width:1px;rx:0;ry:0;}",
        ),
        (
            "note-label-fill",
            ThemeTarget::NoteLabel,
            PaintFacet::Fill,
            "noteTextColor",
            "stateDiagram-v2\nReady\nnote right of Ready : terminal note\n",
            ".statediagram-note .nodeLabel{color:#22c55e;}",
        ),
        (
            "transition-label-fill",
            ThemeTarget::TransitionLabel,
            PaintFacet::Fill,
            "transitionLabelColor",
            "stateDiagram-v2\nReady --> Done: Finish\n",
            ".edgeLabel .label text{fill:#22c55e;}",
        ),
        (
            "title-fill",
            ThemeTarget::Title,
            PaintFacet::Fill,
            "textColor",
            "---\ntitle: Terminal ownership\n---\nstateDiagram-v2\nReady\n",
            ".statediagramTitleText{text-anchor:middle;font-size:18px;fill:#22c55e;}",
        ),
    ];

    for (case, target, facet, config_key, source, terminal_fragment) in cases {
        let style = match facet {
            PaintFacet::Fill => ThemeStylePatch::default().with_fill(
                CanvasPaint::solid(TYPED_COLOR).expect("valid typed State terminal fill"),
            ),
            PaintFacet::Stroke => ThemeStylePatch::default().with_stroke(
                CanvasPaint::solid(TYPED_COLOR).expect("valid typed State terminal stroke"),
            ),
        };
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(target, style))),
            )
            .unwrap_or_else(|error| panic!("compile {case} State ownership theme: {error}"));

        for origin in ["site", "source"] {
            let config = state_terminal_owner_config_for_origin(
                serde_json::json!({
                    "theme": "base",
                    "themeVariables": {(config_key): MERMAID_COLOR},
                }),
                origin,
            );
            let (svg, evidence) =
                render_strict_state_svg_with_theme_and_config(&theme, source, config, origin);

            assert!(
                svg.contains(terminal_fragment),
                "Mermaid terminal owner must reach the exact {origin} {case} declaration: {svg}"
            );
            assert!(
                !svg.contains(TYPED_COLOR),
                "typed paint must yield to {origin} {case}: {svg}"
            );
            assert_eq!(
                evidence,
                StateThemeEvidenceCounts {
                    required: 1,
                    accounted: 1,
                    applied: 0,
                    not_applicable: 1,
                    residual: 0,
                },
                "{origin} {case}"
            );
        }
    }
}

#[test]
fn state_transition_marker_paints_use_transition_color_before_line_color() {
    const SOURCE: &str = "stateDiagram-v2\nReady --> Done: Finish\n";
    const TYPED_COLOR: &str = "#ec4899";
    const MERMAID_COLOR: &str = "#22c55e";

    for (property, style, expected_typed_declaration) in [
        (
            "fill",
            ThemeStylePatch::default().with_fill(
                CanvasPaint::solid(TYPED_COLOR).expect("valid typed State transition marker fill"),
            ),
            "fill:#ec4899 !important",
        ),
        (
            "stroke",
            ThemeStylePatch::default().with_stroke(
                CanvasPaint::solid(TYPED_COLOR)
                    .expect("valid typed State transition marker stroke"),
            ),
            "stroke:#ec4899 !important",
        ),
    ] {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(ThemeTarget::TransitionMarker, style)),
                ),
            )
            .unwrap_or_else(|error| {
                panic!("compile State transition marker {property} ownership theme: {error}")
            });

        for origin in ["site", "source"] {
            let (owned_svg, owned_evidence) = render_strict_state_svg_with_theme_and_config(
                &theme,
                SOURCE,
                serde_json::json!({
                    "theme": "base",
                    "themeVariables": { "transitionColor": MERMAID_COLOR },
                }),
                origin,
            );
            let owned_document =
                roxmltree::Document::parse(&owned_svg).expect("valid owned State marker SVG XML");
            let owned_marker_path = owned_document
                .descendants()
                .find(|node| {
                    node.has_tag_name("marker")
                        && node
                            .attribute("id")
                            .is_some_and(|id| id.ends_with("_stateDiagram-barbEnd"))
                })
                .and_then(|marker| marker.children().find(|node| node.has_tag_name("path")))
                .expect("base State barbEnd marker path");

            assert!(
                owned_svg.contains("defs [id$=\"-barbEnd\"]{fill:#22c55e;stroke:#22c55e;}"),
                "transitionColor must own both terminal marker paints for {origin}: {owned_svg}"
            );
            assert!(
                !owned_marker_path
                    .attribute("style")
                    .unwrap_or_default()
                    .contains(TYPED_COLOR),
                "typed marker {property} must yield to explicit transitionColor for {origin}: {owned_svg}"
            );
            assert_eq!(
                owned_evidence,
                StateThemeEvidenceCounts {
                    required: 1,
                    accounted: 1,
                    applied: 0,
                    not_applicable: 1,
                    residual: 0,
                },
                "owned transitionColor {property} for {origin}"
            );

            let (line_svg, line_evidence) = render_strict_state_svg_with_theme_and_config(
                &theme,
                SOURCE,
                serde_json::json!({
                    "theme": "default",
                    "themeVariables": { "lineColor": MERMAID_COLOR },
                }),
                origin,
            );
            let line_document = roxmltree::Document::parse(&line_svg)
                .expect("valid lineColor State marker SVG XML");
            let typed_marker_path = line_document
                .descendants()
                .find(|node| {
                    node.has_tag_name("marker")
                        && node
                            .attribute("id")
                            .is_some_and(|id| id.ends_with("_stateDiagram-barbEnd-1"))
                })
                .and_then(|marker| marker.children().find(|node| node.has_tag_name("path")))
                .expect("typed State barbEnd marker path");

            assert!(
                typed_marker_path
                    .attribute("style")
                    .is_some_and(|style| style.contains(expected_typed_declaration)),
                "a late Default lineColor replay must not own the already-selected transitionColor {property} terminal for {origin}: {line_svg}"
            );
            assert_eq!(
                line_evidence,
                StateThemeEvidenceCounts {
                    required: 1,
                    accounted: 1,
                    applied: 1,
                    not_applicable: 0,
                    residual: 0,
                },
                "non-owning Default lineColor {property} for {origin}"
            );
        }
    }
}

#[test]
fn state_neo_note_stroke_yields_to_the_shared_node_border_owner() {
    const SOURCE: &str = "stateDiagram-v2\nReady\nnote right of Ready : Neo terminal note\n";
    const TYPED_COLOR: &str = "#ec4899";
    const MERMAID_COLOR: &str = "#22c55e";

    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Note,
                ThemeStylePatch::default().with_stroke(
                    CanvasPaint::solid(TYPED_COLOR).expect("valid typed Neo State note stroke"),
                ),
            ))),
        )
        .expect("compile Neo State note ownership theme");

    for origin in ["site", "source"] {
        let (svg, evidence) = render_strict_state_svg_with_theme_and_config(
            &theme,
            SOURCE,
            state_terminal_owner_config_for_origin(
                serde_json::json!({
                    "theme": "base",
                    "look": "neo",
                    "themeVariables": {
                        "nodeBorder": MERMAID_COLOR,
                        "useGradient": false,
                    },
                }),
                origin,
            ),
            origin,
        );
        let document =
            roxmltree::Document::parse(&svg).expect("valid Neo State note ownership SVG XML");
        let note = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class").is_some_and(|classes| {
                        classes
                            .split_whitespace()
                            .any(|class| class == "statediagram-note")
                    })
            })
            .expect("Neo State note group");
        let stroke_path = note
            .descendants()
            .filter(|node| node.has_tag_name("path"))
            .nth(1)
            .expect("Neo State note stroke path");

        assert!(
            !stroke_path
                .attribute("style")
                .unwrap_or_default()
                .contains(TYPED_COLOR),
            "typed Note.stroke must yield to the Neo path-level owner for {origin}: {svg}"
        );
        assert!(
            svg.contains("[data-look=\"neo\"].node path{stroke:#22c55e;stroke-width:"),
            "the exact Neo path selector must carry nodeBorder rather than stateBorder for {origin}: {svg}"
        );
        assert_eq!(
            evidence,
            StateThemeEvidenceCounts {
                required: 1,
                accounted: 1,
                applied: 0,
                not_applicable: 1,
                residual: 0,
            },
            "Neo Note.stroke selected border owner for {origin}"
        );
    }
}

#[test]
fn state_neo_note_stroke_does_not_yield_to_state_border() {
    const SOURCE: &str = "stateDiagram-v2\nReady\nnote right of Ready : Neo terminal note\n";
    const TYPED_COLOR: &str = "#ec4899";
    const NON_OWNER_COLOR: &str = "#22c55e";

    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Note,
                ThemeStylePatch::default().with_stroke(
                    CanvasPaint::solid(TYPED_COLOR).expect("valid typed Neo State note stroke"),
                ),
            ))),
        )
        .expect("compile Neo State note non-owner theme");
    let (svg, evidence) = render_strict_state_svg_with_theme_and_config(
        &theme,
        SOURCE,
        serde_json::json!({
            "theme": "base",
            "look": "neo",
            "themeVariables": {
                "stateBorder": NON_OWNER_COLOR,
                "useGradient": false,
            },
        }),
        "site",
    );
    let document =
        roxmltree::Document::parse(&svg).expect("valid Neo State note non-owner SVG XML");
    let stroke_path = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|class| class == "statediagram-note")
                })
        })
        .and_then(|note| {
            note.descendants()
                .filter(|node| node.has_tag_name("path"))
                .nth(1)
        })
        .expect("Neo State note stroke path");

    assert!(
        stroke_path
            .attribute("style")
            .is_some_and(|style| style.contains("stroke:#ec4899 !important")),
        "stateBorder must not suppress typed Neo Note.stroke: {svg}"
    );
    assert!(
        !svg.contains("[data-look=\"neo\"].node path{stroke:#22c55e;"),
        "Neo node stroke must not read stateBorder: {svg}"
    );
    assert_eq!(
        evidence,
        StateThemeEvidenceCounts {
            required: 1,
            accounted: 1,
            applied: 1,
            not_applicable: 0,
            residual: 0,
        }
    );
}

#[test]
fn state_neo_shared_node_stroke_uses_the_scoped_gradient_for_state_note_and_start() {
    const SOURCE: &str =
        "stateDiagram-v2\n[*] --> Ready\nnote right of Ready : Neo terminal note\n";
    const TYPED_COLOR: &str = "#ec4899";
    const GRADIENT_START: &str = "#22c55e";
    let stroke = || {
        ThemeStylePatch::default().with_stroke(
            CanvasPaint::solid(TYPED_COLOR).expect("valid typed Neo State shared stroke"),
        )
    };
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(ThemeRule::new(ThemeTarget::State, stroke()))
                    .with_rule(ThemeRule::new(ThemeTarget::Note, stroke()))
                    .with_rule(
                        ThemeRule::new(ThemeTarget::SpecialState, stroke())
                            .with_variant(ThemeVariant::Start),
                    ),
            ),
        )
        .expect("compile Neo State shared-gradient theme");

    for origin in ["site", "source"] {
        let (svg, evidence) = render_strict_state_svg_with_theme_and_config(
            &theme,
            SOURCE,
            serde_json::json!({
                "theme": "base",
                "look": "neo",
                "themeVariables": {
                    "useGradient": true,
                    "gradientStart": GRADIENT_START,
                    "gradientStop": "#16a34a",
                    "nodeBorder": "#0f172a",
                },
            }),
            origin,
        );

        for selector in [
            "[data-look=\"neo\"].node rect,",
            "[data-look=\"neo\"].node path{stroke:url(#merman-gradient);",
            "[data-look=\"neo\"].node circle{stroke:url(#merman-gradient);",
        ] {
            assert!(
                svg.contains(selector),
                "the scoped Neo gradient must own {origin} selector {selector}: {svg}"
            );
        }
        assert!(
            svg.contains("stop-color=\"#22c55e\"") && svg.contains("stop-color=\"#16a34a\""),
            "the scoped Neo gradient must retain both {origin} stops: {svg}"
        );
        assert!(
            !svg.contains(TYPED_COLOR),
            "{origin} shared gradient: {svg}"
        );
        assert_eq!(
            evidence,
            StateThemeEvidenceCounts {
                required: 3,
                accounted: 3,
                applied: 0,
                not_applicable: 3,
                residual: 0,
            },
            "{origin} shared Neo gradient"
        );
    }
}

#[test]
fn state_composite_terminal_owners_follow_shape_look_and_gradient_branch() {
    const TYPED_COLOR: &str = "#ec4899";
    const MERMAID_COLOR: &str = "#22c55e";
    const COMPOSITE: &str = "stateDiagram-v2\nstate Parent {\n  [*] --> Running\n}\n";
    const NESTED_COMPOSITE: &str =
        "stateDiagram-v2\nstate Outer {\n  state Inner {\n    [*] --> Running\n  }\n}\n";
    const DIVIDER: &str =
        "stateDiagram-v2\nstate Parallel {\n  [*] --> First\n  --\n  [*] --> Second\n}\n";

    #[derive(Clone, Copy)]
    enum PaintFacet {
        Fill,
        Stroke,
    }

    #[derive(Clone, Copy)]
    enum ClusterPart {
        Body,
        Header,
        Label,
        Divider,
    }

    struct Case {
        name: &'static str,
        target: ThemeTarget,
        facet: PaintFacet,
        source: &'static str,
        cluster_id: &'static str,
        part: ClusterPart,
        ordinal: Option<usize>,
        config: serde_json::Value,
        terminal_fragment: &'static str,
    }

    let cases = vec![
        Case {
            name: "classic-body-fill",
            target: ThemeTarget::Composite,
            facet: PaintFacet::Fill,
            source: COMPOSITE,
            cluster_id: "Parent",
            part: ClusterPart::Body,
            ordinal: None,
            config: serde_json::json!({
                "theme": "base",
                "htmlLabels": false,
                "themeVariables": { "compositeBackground": MERMAID_COLOR },
            }),
            terminal_fragment: ".statediagram-cluster.statediagram-cluster .inner{fill:#22c55e;}",
        },
        Case {
            name: "classic-body-stroke",
            target: ThemeTarget::Composite,
            facet: PaintFacet::Stroke,
            source: COMPOSITE,
            cluster_id: "Parent",
            part: ClusterPart::Body,
            ordinal: None,
            config: serde_json::json!({
                "theme": "base",
                "htmlLabels": false,
                "themeVariables": { "stateBorder": MERMAID_COLOR },
            }),
            terminal_fragment: ";stroke:#22c55e;stroke-width:",
        },
        Case {
            name: "classic-header-fill",
            target: ThemeTarget::CompositeHeader,
            facet: PaintFacet::Fill,
            source: COMPOSITE,
            cluster_id: "Parent",
            part: ClusterPart::Header,
            ordinal: None,
            config: serde_json::json!({
                "theme": "base",
                "htmlLabels": false,
                "themeVariables": { "compositeTitleBackground": MERMAID_COLOR },
            }),
            terminal_fragment: ".statediagram-cluster rect{fill:#22c55e;stroke:",
        },
        Case {
            name: "classic-header-stroke",
            target: ThemeTarget::CompositeHeader,
            facet: PaintFacet::Stroke,
            source: COMPOSITE,
            cluster_id: "Parent",
            part: ClusterPart::Header,
            ordinal: None,
            config: serde_json::json!({
                "theme": "base",
                "htmlLabels": false,
                "themeVariables": { "stateBorder": MERMAID_COLOR },
            }),
            terminal_fragment: ";stroke:#22c55e;stroke-width:",
        },
        Case {
            name: "classic-label-fill",
            target: ThemeTarget::CompositeLabel,
            facet: PaintFacet::Fill,
            source: COMPOSITE,
            cluster_id: "Parent",
            part: ClusterPart::Label,
            ordinal: None,
            config: serde_json::json!({
                "theme": "base",
                "htmlLabels": false,
                "themeVariables": { "stateLabelColor": MERMAID_COLOR },
            }),
            terminal_fragment: ".cluster-label,#merman .nodeLabel{color:#22c55e;}",
        },
        Case {
            name: "classic-alt-body-fill",
            target: ThemeTarget::Composite,
            facet: PaintFacet::Fill,
            source: NESTED_COMPOSITE,
            cluster_id: "Inner",
            part: ClusterPart::Body,
            ordinal: Some(2),
            config: serde_json::json!({
                "theme": "base",
                "htmlLabels": false,
                "themeVariables": { "altBackground": MERMAID_COLOR },
            }),
            terminal_fragment: ".statediagram-cluster.statediagram-cluster-alt .inner{fill:#22c55e;}",
        },
        Case {
            name: "classic-divider-fill",
            target: ThemeTarget::Composite,
            facet: PaintFacet::Fill,
            source: DIVIDER,
            cluster_id: "divider-id-1",
            part: ClusterPart::Divider,
            ordinal: Some(2),
            config: serde_json::json!({
                "theme": "base",
                "htmlLabels": false,
                "themeVariables": { "altBackground": MERMAID_COLOR },
            }),
            terminal_fragment: ".statediagram-state rect.divider{stroke-dasharray:10,10;fill:#22c55e;}",
        },
        Case {
            name: "neo-body-fill",
            target: ThemeTarget::Composite,
            facet: PaintFacet::Fill,
            source: COMPOSITE,
            cluster_id: "Parent",
            part: ClusterPart::Body,
            ordinal: None,
            config: serde_json::json!({
                "theme": "base",
                "look": "neo",
                "htmlLabels": false,
                "themeVariables": { "mainBkg": MERMAID_COLOR },
            }),
            terminal_fragment: "[data-look=\"neo\"].statediagram-cluster rect{fill:#22c55e;stroke:",
        },
        Case {
            name: "neo-header-fill",
            target: ThemeTarget::CompositeHeader,
            facet: PaintFacet::Fill,
            source: COMPOSITE,
            cluster_id: "Parent",
            part: ClusterPart::Header,
            ordinal: None,
            config: serde_json::json!({
                "theme": "base",
                "look": "neo",
                "htmlLabels": false,
                "themeVariables": { "mainBkg": MERMAID_COLOR },
            }),
            terminal_fragment: "[data-look=\"neo\"].statediagram-cluster rect{fill:#22c55e;stroke:",
        },
        Case {
            name: "neo-body-stroke",
            target: ThemeTarget::Composite,
            facet: PaintFacet::Stroke,
            source: COMPOSITE,
            cluster_id: "Parent",
            part: ClusterPart::Body,
            ordinal: None,
            config: serde_json::json!({
                "theme": "base",
                "look": "neo",
                "htmlLabels": false,
                "themeVariables": { "stateBorder": MERMAID_COLOR, "useGradient": false },
            }),
            terminal_fragment: ";stroke:#22c55e;stroke-width:",
        },
        Case {
            name: "neo-header-gradient-stroke",
            target: ThemeTarget::CompositeHeader,
            facet: PaintFacet::Stroke,
            source: COMPOSITE,
            cluster_id: "Parent",
            part: ClusterPart::Header,
            ordinal: None,
            config: serde_json::json!({
                "theme": "base",
                "look": "neo",
                "htmlLabels": false,
                "themeVariables": {
                    "useGradient": true,
                    "gradientStart": MERMAID_COLOR,
                    "gradientStop": "#16a34a"
                },
            }),
            terminal_fragment: "stroke:url(#merman-gradient);stroke-width:",
        },
    ];

    for case in cases {
        let style = match case.facet {
            PaintFacet::Fill => ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid(TYPED_COLOR).expect("valid typed composite fill")),
            PaintFacet::Stroke => ThemeStylePatch::default().with_stroke(
                CanvasPaint::solid(TYPED_COLOR).expect("valid typed composite stroke"),
            ),
        };
        let mut rule = ThemeRule::new(case.target, style);
        if let Some(ordinal) = case.ordinal {
            rule = rule.with_ordinal(
                OrdinalSelector::exact(ordinal).expect("valid composite terminal ordinal"),
            );
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
            .unwrap_or_else(|error| panic!("compile {} State theme: {error}", case.name));

        for origin in ["site", "source"] {
            let (svg, evidence) = render_strict_state_svg_with_theme_and_config(
                &theme,
                case.source,
                state_terminal_owner_config_for_origin(case.config.clone(), origin),
                origin,
            );
            let document = roxmltree::Document::parse(&svg)
                .unwrap_or_else(|error| panic!("valid {} State SVG XML: {error}", case.name));
            let cluster = document
                .descendants()
                .find(|node| {
                    if !node.has_tag_name("g") {
                        return false;
                    }
                    if matches!(case.part, ClusterPart::Divider) {
                        return node.attribute("id").is_some_and(|id| {
                            id.contains(&format!("-state-{}-", case.cluster_id))
                        }) && node.descendants().any(|child| {
                            child.has_tag_name("rect")
                                && child.attribute("class") == Some("divider")
                        });
                    }
                    node.attribute("data-id") == Some(case.cluster_id)
                })
                .unwrap_or_else(|| panic!("missing {} cluster: {svg}", case.cluster_id));
            let terminal = match case.part {
                ClusterPart::Body => cluster.children().find(|node| {
                    node.has_tag_name("rect") && node.attribute("class") == Some("inner")
                }),
                ClusterPart::Header => cluster.descendants().find(|node| {
                    node.has_tag_name("rect") && node.attribute("class") == Some("outer")
                }),
                ClusterPart::Label => cluster.children().find(|node| {
                    node.has_tag_name("g") && node.attribute("class") == Some("cluster-label")
                }),
                ClusterPart::Divider => cluster.descendants().find(|node| {
                    node.has_tag_name("rect") && node.attribute("class") == Some("divider")
                }),
            }
            .unwrap_or_else(|| panic!("missing {} terminal element: {svg}", case.name));

            assert!(
                !terminal
                    .attribute("style")
                    .unwrap_or_default()
                    .contains(TYPED_COLOR),
                "typed paint must not reach the owned {origin} {} terminal: {svg}",
                case.name
            );
            assert!(
                svg.contains(case.terminal_fragment),
                "exact Mermaid {origin} {} terminal declaration is missing: {svg}",
                case.name
            );
            if case.name == "neo-header-gradient-stroke" {
                assert!(
                    svg.contains("stop-color=\"#22c55e\"")
                        && svg.contains("stop-color=\"#16a34a\""),
                    "the selected Neo gradient stops must reach the terminal definition: {svg}"
                );
            }
            assert_eq!(
                evidence,
                StateThemeEvidenceCounts {
                    required: 1,
                    accounted: 1,
                    applied: 0,
                    not_applicable: 1,
                    residual: 0,
                },
                "{origin} {}",
                case.name
            );
        }
    }
}

#[test]
fn state_special_terminal_owners_follow_shape_look_and_inner_outer_branch() {
    const TYPED_COLOR: &str = "#ec4899";
    const MERMAID_COLOR: &str = "#22c55e";
    const START: &str = "stateDiagram-v2\n[*] --> Ready\n";
    const END: &str = "stateDiagram-v2\nReady --> [*]\n";
    const FORK: &str = "stateDiagram-v2\nstate Fork <<fork>>\n";
    const CHOICE: &str = "stateDiagram-v2\nstate Decide <<choice>>\n";

    #[derive(Clone, Copy)]
    enum PaintFacet {
        Fill,
        Stroke,
    }

    #[derive(Clone, Copy)]
    enum SpecialPart {
        Start,
        EndOuterFill,
        EndOuterStroke,
        EndInnerFill,
        EndInnerStroke,
        NamedFill(&'static str),
        NamedStroke(&'static str),
    }

    struct Case {
        name: &'static str,
        target: ThemeTarget,
        variant: ThemeVariant,
        facet: PaintFacet,
        source: &'static str,
        part: SpecialPart,
        config: serde_json::Value,
        expected_attr: Option<(&'static str, &'static str)>,
        terminal_fragment: Option<&'static str>,
    }

    let special_config = |look: &str| {
        serde_json::json!({
            "theme": "base",
            "look": look,
            "themeVariables": { "specialStateColor": MERMAID_COLOR },
        })
    };
    let line_config = |look: &str| {
        serde_json::json!({
            "theme": "base",
            "look": look,
            "themeVariables": { "lineColor": MERMAID_COLOR },
        })
    };
    let node_border_config = |look: &str| {
        serde_json::json!({
            "theme": "base",
            "look": look,
            "themeVariables": {
                "nodeBorder": MERMAID_COLOR,
                "useGradient": false,
            },
        })
    };
    let inner_border_config = |look: &str| {
        serde_json::json!({
            "theme": "base",
            "look": look,
            "themeVariables": { "stateBorder": MERMAID_COLOR },
        })
    };
    let cases = vec![
        Case {
            name: "classic-start-fill",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Start,
            facet: PaintFacet::Fill,
            source: START,
            part: SpecialPart::Start,
            config: special_config("classic"),
            expected_attr: None,
            terminal_fragment: Some(".node circle.state-start{fill:#22c55e;stroke:#22c55e;}"),
        },
        Case {
            name: "classic-start-stroke",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Start,
            facet: PaintFacet::Stroke,
            source: START,
            part: SpecialPart::Start,
            config: special_config("classic"),
            expected_attr: None,
            terminal_fragment: Some(".node circle.state-start{fill:#22c55e;stroke:#22c55e;}"),
        },
        Case {
            name: "classic-end-outer-fill",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::End,
            facet: PaintFacet::Fill,
            source: END,
            part: SpecialPart::EndOuterFill,
            config: serde_json::json!({
                "theme": "base",
                "look": "classic",
                "themeVariables": { "mainBkg": MERMAID_COLOR },
            }),
            expected_attr: Some(("fill", MERMAID_COLOR)),
            terminal_fragment: None,
        },
        Case {
            name: "classic-end-outer-stroke",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::End,
            facet: PaintFacet::Stroke,
            source: END,
            part: SpecialPart::EndOuterStroke,
            config: line_config("classic"),
            expected_attr: Some(("stroke", MERMAID_COLOR)),
            terminal_fragment: None,
        },
        Case {
            name: "classic-fork-fill",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Special,
            facet: PaintFacet::Fill,
            source: FORK,
            part: SpecialPart::NamedFill("Fork"),
            config: line_config("classic"),
            expected_attr: Some(("fill", MERMAID_COLOR)),
            terminal_fragment: None,
        },
        Case {
            name: "classic-fork-stroke",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Special,
            facet: PaintFacet::Stroke,
            source: FORK,
            part: SpecialPart::NamedStroke("Fork"),
            config: line_config("classic"),
            expected_attr: Some(("stroke", MERMAID_COLOR)),
            terminal_fragment: None,
        },
        Case {
            name: "classic-choice-fill",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Special,
            facet: PaintFacet::Fill,
            source: CHOICE,
            part: SpecialPart::NamedFill("Decide"),
            config: serde_json::json!({
                "theme": "base",
                "look": "classic",
                "themeVariables": { "mainBkg": MERMAID_COLOR },
            }),
            expected_attr: Some(("fill", MERMAID_COLOR)),
            terminal_fragment: None,
        },
        Case {
            name: "classic-choice-stroke",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Special,
            facet: PaintFacet::Stroke,
            source: CHOICE,
            part: SpecialPart::NamedStroke("Decide"),
            config: node_border_config("classic"),
            expected_attr: Some(("stroke", MERMAID_COLOR)),
            terminal_fragment: None,
        },
        Case {
            name: "classic-end-inner-fill",
            target: ThemeTarget::SpecialStateInner,
            variant: ThemeVariant::End,
            facet: PaintFacet::Fill,
            source: END,
            part: SpecialPart::EndInnerFill,
            config: inner_border_config("classic"),
            expected_attr: Some(("fill", MERMAID_COLOR)),
            terminal_fragment: None,
        },
        Case {
            name: "classic-end-inner-stroke",
            target: ThemeTarget::SpecialStateInner,
            variant: ThemeVariant::End,
            facet: PaintFacet::Stroke,
            source: END,
            part: SpecialPart::EndInnerStroke,
            config: inner_border_config("classic"),
            expected_attr: Some(("stroke", MERMAID_COLOR)),
            terminal_fragment: None,
        },
        Case {
            name: "neo-start-fill",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Start,
            facet: PaintFacet::Fill,
            source: START,
            part: SpecialPart::Start,
            config: special_config("neo"),
            expected_attr: None,
            terminal_fragment: Some(".node circle.state-start{fill:#22c55e;stroke:#22c55e;}"),
        },
        Case {
            name: "neo-start-stroke",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Start,
            facet: PaintFacet::Stroke,
            source: START,
            part: SpecialPart::Start,
            config: node_border_config("neo"),
            expected_attr: None,
            terminal_fragment: Some("[data-look=\"neo\"].node circle{stroke:#22c55e;filter:"),
        },
        Case {
            name: "neo-end-outer-fill",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::End,
            facet: PaintFacet::Fill,
            source: END,
            part: SpecialPart::EndOuterFill,
            config: serde_json::json!({
                "theme": "base",
                "look": "neo",
                "themeVariables": { "mainBkg": MERMAID_COLOR },
            }),
            expected_attr: Some(("fill", MERMAID_COLOR)),
            terminal_fragment: None,
        },
        Case {
            name: "neo-end-outer-stroke",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::End,
            facet: PaintFacet::Stroke,
            source: END,
            part: SpecialPart::EndOuterStroke,
            config: node_border_config("neo"),
            expected_attr: Some(("stroke", MERMAID_COLOR)),
            terminal_fragment: None,
        },
        Case {
            name: "neo-fork-fill",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Special,
            facet: PaintFacet::Fill,
            source: FORK,
            part: SpecialPart::NamedFill("Fork"),
            config: line_config("neo"),
            expected_attr: Some(("fill", MERMAID_COLOR)),
            terminal_fragment: None,
        },
        Case {
            name: "neo-fork-stroke",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Special,
            facet: PaintFacet::Stroke,
            source: FORK,
            part: SpecialPart::NamedStroke("Fork"),
            config: node_border_config("neo"),
            expected_attr: Some(("stroke", MERMAID_COLOR)),
            terminal_fragment: None,
        },
        Case {
            name: "neo-choice-fill",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Special,
            facet: PaintFacet::Fill,
            source: CHOICE,
            part: SpecialPart::NamedFill("Decide"),
            config: serde_json::json!({
                "theme": "base",
                "look": "neo",
                "themeVariables": { "mainBkg": MERMAID_COLOR },
            }),
            expected_attr: Some(("fill", MERMAID_COLOR)),
            terminal_fragment: None,
        },
        Case {
            name: "neo-choice-stroke",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Special,
            facet: PaintFacet::Stroke,
            source: CHOICE,
            part: SpecialPart::NamedStroke("Decide"),
            config: node_border_config("neo"),
            expected_attr: Some(("stroke", MERMAID_COLOR)),
            terminal_fragment: None,
        },
        Case {
            name: "neo-end-inner-fill",
            target: ThemeTarget::SpecialStateInner,
            variant: ThemeVariant::End,
            facet: PaintFacet::Fill,
            source: END,
            part: SpecialPart::EndInnerFill,
            config: inner_border_config("neo"),
            expected_attr: Some(("fill", MERMAID_COLOR)),
            terminal_fragment: None,
        },
        Case {
            name: "neo-end-inner-stroke",
            target: ThemeTarget::SpecialStateInner,
            variant: ThemeVariant::End,
            facet: PaintFacet::Stroke,
            source: END,
            part: SpecialPart::EndInnerStroke,
            config: node_border_config("neo"),
            expected_attr: Some(("stroke", MERMAID_COLOR)),
            terminal_fragment: None,
        },
    ];

    for case in cases {
        let style = match case.facet {
            PaintFacet::Fill => ThemeStylePatch::default().with_fill(
                CanvasPaint::solid(TYPED_COLOR).expect("valid typed special-state fill"),
            ),
            PaintFacet::Stroke => ThemeStylePatch::default().with_stroke(
                CanvasPaint::solid(TYPED_COLOR).expect("valid typed special-state stroke"),
            ),
        };
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(case.target, style).with_variant(case.variant)),
                ),
            )
            .unwrap_or_else(|error| panic!("compile {} State theme: {error}", case.name));

        for origin in ["site", "source"] {
            let (svg, evidence) = render_strict_state_svg_with_theme_and_config(
                &theme,
                case.source,
                state_terminal_owner_config_for_origin(case.config.clone(), origin),
                origin,
            );
            let document = roxmltree::Document::parse(&svg)
                .unwrap_or_else(|error| panic!("valid {} State SVG XML: {error}", case.name));
            let terminal = match case.part {
                SpecialPart::Start => document.descendants().find(|node| {
                    node.has_tag_name("circle") && node.attribute("class") == Some("state-start")
                }),
                SpecialPart::EndOuterFill => document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("g") && node.attribute("class") == Some("outer-path")
                    })
                    .and_then(|outer| outer.children().find(|node| node.has_tag_name("path"))),
                SpecialPart::EndOuterStroke => document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("g") && node.attribute("class") == Some("outer-path")
                    })
                    .and_then(|outer| {
                        outer
                            .children()
                            .filter(|node| node.has_tag_name("path"))
                            .nth(1)
                    }),
                SpecialPart::EndInnerFill => document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("g") && node.attribute("class") == Some("outer-path")
                    })
                    .and_then(|outer| outer.children().find(|node| node.has_tag_name("g")))
                    .and_then(|inner| inner.children().find(|node| node.has_tag_name("path"))),
                SpecialPart::EndInnerStroke => document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("g") && node.attribute("class") == Some("outer-path")
                    })
                    .and_then(|outer| outer.children().find(|node| node.has_tag_name("g")))
                    .and_then(|inner| {
                        inner
                            .children()
                            .filter(|node| node.has_tag_name("path"))
                            .nth(1)
                    }),
                SpecialPart::NamedFill(id) => document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("g")
                            && node
                                .attribute("id")
                                .is_some_and(|value| value.contains(&format!("-state-{id}-")))
                    })
                    .and_then(|group| group.descendants().find(|node| node.has_tag_name("path"))),
                SpecialPart::NamedStroke(id) => document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("g")
                            && node
                                .attribute("id")
                                .is_some_and(|value| value.contains(&format!("-state-{id}-")))
                    })
                    .and_then(|group| {
                        group
                            .descendants()
                            .filter(|node| node.has_tag_name("path"))
                            .nth(1)
                    }),
            }
            .unwrap_or_else(|| panic!("missing {} terminal element: {svg}", case.name));

            assert!(
                !terminal
                    .attribute("style")
                    .unwrap_or_default()
                    .contains(TYPED_COLOR),
                "typed paint must not reach the owned {origin} {} terminal: {svg}",
                case.name
            );
            if let Some((property, value)) = case.expected_attr {
                assert_eq!(
                    terminal.attribute(property),
                    Some(value),
                    "exact {origin} {} terminal attribute: {svg}",
                    case.name
                );
                assert!(
                    terminal
                        .attribute("style")
                        .is_some_and(|style| style.contains(&format!("{property}:{value};"))),
                    "the {origin} {} RoughJS terminal must retain its compatibility winner in inline style: {svg}",
                    case.name
                );
            }
            if let Some(fragment) = case.terminal_fragment {
                assert!(
                    svg.contains(fragment),
                    "exact Mermaid {origin} {} terminal declaration is missing: {svg}",
                    case.name
                );
            }
            assert_eq!(
                evidence,
                StateThemeEvidenceCounts {
                    required: 1,
                    accounted: 1,
                    applied: 0,
                    not_applicable: 1,
                    residual: 0,
                },
                "{origin} {}",
                case.name
            );
        }
    }
}

#[test]
fn state_neo_global_node_gradient_owns_rough_special_path_strokes() {
    const TYPED_COLOR: &str = "#ec4899";
    const END: &str = "stateDiagram-v2\nReady --> [*]\n";
    const FORK: &str = "stateDiagram-v2\nstate Fork <<fork>>\n";
    const CHOICE: &str = "stateDiagram-v2\nstate Decide <<choice>>\n";

    #[derive(Clone, Copy)]
    enum Terminal {
        EndOuter,
        EndInner,
        Named(&'static str),
    }

    let cases = [
        (
            "end-outer",
            ThemeTarget::SpecialState,
            ThemeVariant::End,
            END,
            Terminal::EndOuter,
        ),
        (
            "end-inner",
            ThemeTarget::SpecialStateInner,
            ThemeVariant::End,
            END,
            Terminal::EndInner,
        ),
        (
            "fork",
            ThemeTarget::SpecialState,
            ThemeVariant::Special,
            FORK,
            Terminal::Named("Fork"),
        ),
        (
            "choice",
            ThemeTarget::SpecialState,
            ThemeVariant::Special,
            CHOICE,
            Terminal::Named("Decide"),
        ),
    ];

    for (case, target, variant, source, terminal) in cases {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            target,
                            ThemeStylePatch::default().with_stroke(
                                CanvasPaint::solid(TYPED_COLOR)
                                    .expect("valid typed Neo special-state stroke"),
                            ),
                        )
                        .with_variant(variant),
                    ),
                ),
            )
            .unwrap_or_else(|error| panic!("compile {case} gradient-owner theme: {error}"));

        for origin in ["site", "source"] {
            let (svg, evidence) = render_strict_state_svg_with_theme_and_config(
                &theme,
                source,
                serde_json::json!({
                    "theme": "base",
                    "look": "neo",
                    "themeVariables": {
                        "useGradient": true,
                        "gradientStart": "#22c55e",
                        "gradientStop": "#16a34a"
                    }
                }),
                origin,
            );
            let document = roxmltree::Document::parse(&svg)
                .unwrap_or_else(|error| panic!("valid {case} gradient-owner SVG: {error}"));
            let path = match terminal {
                Terminal::EndOuter => document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("g") && node.attribute("class") == Some("outer-path")
                    })
                    .and_then(|outer| {
                        outer
                            .children()
                            .filter(|node| node.has_tag_name("path"))
                            .nth(1)
                    }),
                Terminal::EndInner => document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("g") && node.attribute("class") == Some("outer-path")
                    })
                    .and_then(|outer| outer.children().find(|node| node.has_tag_name("g")))
                    .and_then(|inner| {
                        inner
                            .children()
                            .filter(|node| node.has_tag_name("path"))
                            .nth(1)
                    }),
                Terminal::Named(id) => document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("g")
                            && node
                                .attribute("id")
                                .is_some_and(|value| value.contains(&format!("-state-{id}-")))
                    })
                    .and_then(|group| {
                        group
                            .descendants()
                            .filter(|node| node.has_tag_name("path"))
                            .nth(1)
                    }),
            }
            .unwrap_or_else(|| panic!("missing {case} gradient-owner terminal: {svg}"));

            assert!(
                svg.contains("[data-look=\"neo\"].node path{stroke:url(#merman-gradient);"),
                "the global Neo gradient setup must be active for {origin} {case}: {svg}"
            );
            assert!(
                path.attribute("style")
                    .is_none_or(|style| !style.contains(TYPED_COLOR)),
                "the explicit Neo gradient must suppress the typed {origin} {case} stroke: {svg}"
            );
            assert_eq!(
                path.attribute("stroke"),
                Some("url(#merman-gradient)"),
                "the writer must consume the same scoped gradient selected by the compatibility plan for {origin} {case}: {svg}"
            );
            assert_eq!(
                evidence,
                StateThemeEvidenceCounts {
                    required: 1,
                    accounted: 1,
                    applied: 0,
                    not_applicable: 1,
                    residual: 0,
                },
                "{origin} {case} gradient owner"
            );
        }
    }
}

#[test]
fn state_structural_direct_paints_remain_applied_without_terminal_provenance() {
    const TYPED_COLOR: &str = "#ec4899";
    const COMPOSITE: &str = "stateDiagram-v2\nstate Parent {\n  [*] --> Running\n}\n";
    const START: &str = "stateDiagram-v2\n[*] --> Ready\n";
    const END: &str = "stateDiagram-v2\nReady --> [*]\n";
    const CHOICE: &str = "stateDiagram-v2\nstate Decide <<choice>>\n";

    #[derive(Clone, Copy)]
    enum PaintFacet {
        Fill,
        Stroke,
    }

    #[derive(Clone, Copy)]
    enum Terminal {
        CompositeBody,
        CompositeHeader,
        CompositeLabel,
        Start,
        EndOuterFill,
        EndOuterStroke,
        EndInnerFill,
        NamedFill(&'static str),
    }

    struct Case {
        name: &'static str,
        target: ThemeTarget,
        variant: ThemeVariant,
        facet: PaintFacet,
        source: &'static str,
        terminal: Terminal,
        config: serde_json::Value,
        expected_style: &'static str,
    }

    let classic = || serde_json::json!({"theme": "base", "htmlLabels": false});
    let neo = || {
        serde_json::json!({
            "theme": "base",
            "look": "neo",
            "htmlLabels": false,
        })
    };
    let cases = vec![
        Case {
            name: "composite-body",
            target: ThemeTarget::Composite,
            variant: ThemeVariant::Default,
            facet: PaintFacet::Fill,
            source: COMPOSITE,
            terminal: Terminal::CompositeBody,
            config: classic(),
            expected_style: "fill:#ec4899 !important",
        },
        Case {
            name: "composite-header",
            target: ThemeTarget::CompositeHeader,
            variant: ThemeVariant::Default,
            facet: PaintFacet::Fill,
            source: COMPOSITE,
            terminal: Terminal::CompositeHeader,
            config: classic(),
            expected_style: "fill:#ec4899 !important",
        },
        Case {
            name: "composite-label",
            target: ThemeTarget::CompositeLabel,
            variant: ThemeVariant::Default,
            facet: PaintFacet::Fill,
            source: COMPOSITE,
            terminal: Terminal::CompositeLabel,
            config: classic(),
            expected_style: "color:#ec4899 !important",
        },
        Case {
            name: "special-start",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Start,
            facet: PaintFacet::Fill,
            source: START,
            terminal: Terminal::Start,
            config: classic(),
            expected_style: "fill:#ec4899 !important",
        },
        Case {
            name: "special-end-outer",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::End,
            facet: PaintFacet::Fill,
            source: END,
            terminal: Terminal::EndOuterFill,
            config: classic(),
            expected_style: "fill:#ec4899 !important",
        },
        Case {
            name: "special-choice",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Special,
            facet: PaintFacet::Fill,
            source: CHOICE,
            terminal: Terminal::NamedFill("Decide"),
            config: classic(),
            expected_style: "fill:#ec4899 !important",
        },
        Case {
            name: "special-end-inner",
            target: ThemeTarget::SpecialStateInner,
            variant: ThemeVariant::End,
            facet: PaintFacet::Fill,
            source: END,
            terminal: Terminal::EndInnerFill,
            config: classic(),
            expected_style: "fill:#ec4899 !important",
        },
        Case {
            name: "neo-composite-header-stroke",
            target: ThemeTarget::CompositeHeader,
            variant: ThemeVariant::Default,
            facet: PaintFacet::Stroke,
            source: COMPOSITE,
            terminal: Terminal::CompositeHeader,
            config: neo(),
            expected_style: "stroke:#ec4899 !important",
        },
        Case {
            name: "neo-special-end-outer-stroke",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::End,
            facet: PaintFacet::Stroke,
            source: END,
            terminal: Terminal::EndOuterStroke,
            config: neo(),
            expected_style: "stroke:#ec4899 !important",
        },
    ];

    for case in cases {
        let style = match case.facet {
            PaintFacet::Fill => ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid(TYPED_COLOR).expect("valid structural direct fill")),
            PaintFacet::Stroke => ThemeStylePatch::default().with_stroke(
                CanvasPaint::solid(TYPED_COLOR).expect("valid structural direct stroke"),
            ),
        };
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(case.target, style).with_variant(case.variant)),
                ),
            )
            .unwrap_or_else(|error| panic!("compile {} State theme: {error}", case.name));
        let (svg, evidence) =
            render_strict_state_svg_with_theme_and_config(&theme, case.source, case.config, "site");
        let document = roxmltree::Document::parse(&svg)
            .unwrap_or_else(|error| panic!("valid {} State SVG XML: {error}", case.name));
        let terminal = match case.terminal {
            Terminal::CompositeBody => document
                .descendants()
                .find(|node| node.has_tag_name("g") && node.attribute("data-id") == Some("Parent"))
                .and_then(|cluster| {
                    cluster.children().find(|node| {
                        node.has_tag_name("rect") && node.attribute("class") == Some("inner")
                    })
                }),
            Terminal::CompositeHeader => document
                .descendants()
                .find(|node| node.has_tag_name("g") && node.attribute("data-id") == Some("Parent"))
                .and_then(|cluster| {
                    cluster.descendants().find(|node| {
                        node.has_tag_name("rect") && node.attribute("class") == Some("outer")
                    })
                }),
            Terminal::CompositeLabel => document
                .descendants()
                .find(|node| node.has_tag_name("g") && node.attribute("data-id") == Some("Parent"))
                .and_then(|cluster| {
                    cluster.children().find(|node| {
                        node.has_tag_name("g") && node.attribute("class") == Some("cluster-label")
                    })
                }),
            Terminal::Start => document.descendants().find(|node| {
                node.has_tag_name("circle") && node.attribute("class") == Some("state-start")
            }),
            Terminal::EndOuterFill => document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g") && node.attribute("class") == Some("outer-path")
                })
                .and_then(|outer| outer.children().find(|node| node.has_tag_name("path"))),
            Terminal::EndOuterStroke => document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g") && node.attribute("class") == Some("outer-path")
                })
                .and_then(|outer| {
                    outer
                        .children()
                        .filter(|node| node.has_tag_name("path"))
                        .nth(1)
                }),
            Terminal::EndInnerFill => document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g") && node.attribute("class") == Some("outer-path")
                })
                .and_then(|outer| outer.children().find(|node| node.has_tag_name("g")))
                .and_then(|inner| inner.children().find(|node| node.has_tag_name("path"))),
            Terminal::NamedFill(id) => document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node
                            .attribute("id")
                            .is_some_and(|value| value.contains(&format!("-state-{id}-")))
                })
                .and_then(|group| group.descendants().find(|node| node.has_tag_name("path"))),
        }
        .unwrap_or_else(|| panic!("missing {} terminal element: {svg}", case.name));

        assert!(
            terminal
                .attribute("style")
                .is_some_and(|style| style.contains(case.expected_style)),
            "typed paint must reach the exact unowned {} terminal: {svg}",
            case.name
        );
        assert_eq!(
            evidence,
            StateThemeEvidenceCounts {
                required: 1,
                accounted: 1,
                applied: 1,
                not_applicable: 0,
                residual: 0,
            },
            "unowned {}",
            case.name
        );
    }
}

#[test]
fn state_note_stroke_yields_to_derived_note_background_ownership() {
    const SOURCE: &str = "stateDiagram-v2\nReady\nnote right of Ready : derived note border\n";
    const TYPED_COLOR: &str = "#ec4899";

    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Note,
                ThemeStylePatch::default().with_stroke(
                    CanvasPaint::solid(TYPED_COLOR).expect("valid typed State note stroke"),
                ),
            ))),
        )
        .expect("compile derived State note-border ownership theme");

    for theme_id in ["base", "neo", "neo-dark"] {
        for origin in ["site", "source"] {
            let config = serde_json::json!({
                "theme": theme_id,
                "themeVariables": { "noteBkgColor": "#fef3c7" },
            });
            let (svg, evidence) =
                render_strict_state_svg_with_theme_and_config(&theme, SOURCE, config, origin);

            assert!(
                !svg.contains(TYPED_COLOR),
                "typed State note stroke must yield to derived {origin} {theme_id} noteBorderColor: {svg}"
            );
            assert_eq!(
                evidence,
                StateThemeEvidenceCounts {
                    required: 1,
                    accounted: 1,
                    applied: 0,
                    not_applicable: 1,
                    residual: 0,
                },
                "{origin} {theme_id}"
            );
        }
    }
}

#[test]
fn state_transition_label_background_uses_distinct_native_and_html_terminal_owners() {
    const MERMAID_COLOR: &str = "#22c55e";
    const TYPED_COLOR: &str = "#ec4899";
    const SOURCE: &str = "stateDiagram-v2\nReady --> Done: Finish\n";

    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::TransitionLabelBackground,
                    ThemeStylePatch::default().with_fill(
                        CanvasPaint::solid(TYPED_COLOR)
                            .expect("valid typed State transition-label background"),
                    ),
                )),
            ),
        )
        .expect("compile State transition-label background ownership theme");

    let cases = [
        ("native-owner", false, "labelBackgroundColor", true),
        ("native-non-owner", false, "edgeLabelBackground", false),
        ("html-owner", true, "edgeLabelBackground", true),
        ("html-non-owner", true, "labelBackgroundColor", false),
    ];

    for (case, html_labels, configured_path, mermaid_owns_terminal) in cases {
        for origin in ["site", "source"] {
            let config = serde_json::json!({
                "theme": "base",
                "htmlLabels": html_labels,
                "themeVariables": {(configured_path): MERMAID_COLOR},
            });
            let (svg, evidence) =
                render_strict_state_svg_with_theme_and_config(&theme, SOURCE, config, origin);

            assert_eq!(
                svg.contains(TYPED_COLOR),
                !mermaid_owns_terminal,
                "typed background terminal result for {origin} {case}: {svg}"
            );
            if mermaid_owns_terminal {
                let terminal_fragment = if html_labels {
                    ".edgeLabel{background-color:#22c55e;text-align:center;}"
                } else {
                    ".edgeLabel .label rect{fill:#22c55e;opacity:0.5;}"
                };
                assert!(
                    svg.contains(terminal_fragment),
                    "Mermaid background owner must reach the exact {origin} {case} declaration: {svg}"
                );
            }
            assert_eq!(
                evidence,
                StateThemeEvidenceCounts {
                    required: 1,
                    accounted: 1,
                    applied: usize::from(!mermaid_owns_terminal),
                    not_applicable: usize::from(mermaid_owns_terminal),
                    residual: 0,
                },
                "{origin} {case}"
            );
        }
    }
}

#[test]
fn state_transition_stroke_yields_to_extended_theme_background_ownership() {
    const SOURCE: &str = "stateDiagram-v2\nReady --> Done: Finish\n";
    const TYPED_COLOR: &str = "#ec4899";

    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Transition,
                ThemeStylePatch::default().with_stroke(
                    CanvasPaint::solid(TYPED_COLOR).expect("valid typed State transition stroke"),
                ),
            ))),
        )
        .expect("compile State transition ownership theme");

    for theme_id in [
        "neo",
        "neo-dark",
        "redux",
        "redux-dark",
        "redux-color",
        "redux-dark-color",
    ] {
        for (config_key, mermaid_owns_transition) in [("background", true), ("primaryColor", false)]
        {
            for origin in ["site", "source"] {
                let config = serde_json::json!({
                    "theme": theme_id,
                    "themeVariables": {(config_key): "#ffffff"},
                });
                let (engine, source) = if origin == "site" {
                    (
                        Engine::new().with_site_config(MermaidConfig::from_value(config)),
                        SOURCE.to_string(),
                    )
                } else {
                    (
                        Engine::new().with_site_config(MermaidConfig::from_value(
                            serde_json::json!({"secure": []}),
                        )),
                        format!("%%{{init: {config}}}%%\n{SOURCE}"),
                    )
                };
                let parsed = merman_render::__private::install_parse_compatibility(&theme, engine)
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .unwrap_or_else(|error| {
                        panic!(
                            "parse strict {origin} {theme_id} {config_key} State source: {error}"
                        )
                    })
                    .unwrap_or_else(|| {
                        panic!("detect strict {origin} {theme_id} {config_key} State source")
                    });
                let session = RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .begin_session_with_theme(&theme)
                    .unwrap_or_else(|error| {
                        panic!(
                            "begin strict {origin} {theme_id} {config_key} State session: {error}"
                        )
                    });
                let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
                    .unwrap_or_else(|error| {
                        panic!("prepare strict {origin} {theme_id} {config_key} State: {error}")
                    })
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .unwrap_or_else(|error| {
                        panic!("render strict {origin} {theme_id} {config_key} State: {error}")
                    });

                if mermaid_owns_transition {
                    assert!(
                        rendered.svg().contains("stroke:#000000"),
                        "derived transitionColor must reach {origin} {theme_id} State SVG: {}",
                        rendered.svg()
                    );
                    assert!(
                        !rendered.svg().contains(TYPED_COLOR),
                        "typed transition stroke must yield to {origin} {theme_id} background: {}",
                        rendered.svg()
                    );
                } else {
                    assert!(
                        rendered.svg().contains(TYPED_COLOR),
                        "typed transition stroke must retain the unowned {origin} {theme_id} primaryColor route: {}",
                        rendered.svg()
                    );
                }

                let completion = rendered.into_completion();
                let evidence = merman_render::__private::family_evidence(completion.report());
                assert_eq!(
                    evidence.required_count(),
                    1,
                    "{origin} {theme_id} {config_key}"
                );
                assert_eq!(
                    evidence.accounted_count(),
                    1,
                    "{origin} {theme_id} {config_key}"
                );
                assert_eq!(
                    evidence.applied_count(),
                    usize::from(!mermaid_owns_transition),
                    "{origin} {theme_id} {config_key}"
                );
                assert_eq!(
                    evidence.not_applicable_count(),
                    usize::from(mermaid_owns_transition),
                    "{origin} {theme_id} {config_key}"
                );
                assert_eq!(
                    evidence.theme_residual_count(),
                    0,
                    "{origin} {theme_id} {config_key}"
                );
            }
        }
    }
}

#[test]
fn state_transition_stroke_follows_source_backed_theme_ownership() {
    const SOURCE: &str = "stateDiagram-v2\nReady --> Done: Finish\n";
    const MERMAID_COLOR: &str = "#22c55e";
    const TYPED_COLOR: &str = "#ec4899";

    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Transition,
                ThemeStylePatch::default().with_stroke(
                    CanvasPaint::solid(TYPED_COLOR).expect("valid typed State transition stroke"),
                ),
            ))),
        )
        .expect("compile State transition ownership theme");

    for (theme_id, config_key, mermaid_owns_transition) in [
        ("base", "lineColor", true),
        ("forest", "lineColor", true),
        ("default", "lineColor", false),
        ("dark", "lineColor", false),
        ("neutral", "lineColor", false),
    ] {
        for origin in ["site", "source"] {
            let config = serde_json::json!({
                "theme": theme_id,
                "themeVariables": {(config_key): MERMAID_COLOR},
            });
            let (engine, source) = if origin == "site" {
                (
                    Engine::new().with_site_config(MermaidConfig::from_value(config)),
                    SOURCE.to_string(),
                )
            } else {
                (
                    Engine::new().with_site_config(MermaidConfig::from_value(
                        serde_json::json!({"secure": []}),
                    )),
                    format!("%%{{init: {config}}}%%\n{SOURCE}"),
                )
            };
            let parsed = merman_render::__private::install_parse_compatibility(&theme, engine)
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap_or_else(|error| {
                    panic!("parse strict {origin} {theme_id} {config_key} State source: {error}")
                })
                .unwrap_or_else(|| panic!("detect strict {origin} {theme_id} {config_key} State"));
            let session = RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .unwrap_or_else(|error| {
                    panic!("begin strict {origin} {theme_id} {config_key} State session: {error}")
                });
            let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
                .unwrap_or_else(|error| {
                    panic!("prepare strict {origin} {theme_id} {config_key} State: {error}")
                })
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap_or_else(|error| {
                    panic!("render strict {origin} {theme_id} {config_key} State: {error}")
                });

            if mermaid_owns_transition {
                assert!(
                    rendered.svg().contains("stroke:#22c55e"),
                    "{theme_id} {config_key} must reach the {origin} State transition: {}",
                    rendered.svg()
                );
                assert!(
                    !rendered.svg().contains(TYPED_COLOR),
                    "typed stroke must yield to {origin} {theme_id} {config_key}: {}",
                    rendered.svg()
                );
            } else {
                assert!(
                    rendered.svg().contains(TYPED_COLOR),
                    "typed stroke must remain Applied for {origin} {theme_id} {config_key}: {}",
                    rendered.svg()
                );
            }

            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(
                evidence.required_count(),
                1,
                "{origin} {theme_id} {config_key}"
            );
            assert_eq!(
                evidence.accounted_count(),
                1,
                "{origin} {theme_id} {config_key}"
            );
            assert_eq!(
                evidence.applied_count(),
                usize::from(!mermaid_owns_transition),
                "{origin} {theme_id} {config_key}"
            );
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(mermaid_owns_transition),
                "{origin} {theme_id} {config_key}"
            );
            assert_eq!(
                evidence.theme_residual_count(),
                0,
                "{origin} {theme_id} {config_key}"
            );
        }
    }

    let config = serde_json::json!({
        "theme": "dark",
        "themeVariables": { "mainContrastColor": MERMAID_COLOR },
    });
    let (svg, evidence) =
        render_strict_state_svg_with_theme_and_config(&theme, SOURCE, config, "site");
    assert!(
        svg.contains("stroke:#22c55e"),
        "Dark site mainContrastColor must reach the State transition: {svg}"
    );
    assert!(
        !svg.contains(TYPED_COLOR),
        "typed State transition stroke must yield to Dark site mainContrastColor: {svg}"
    );
    assert_eq!(
        evidence,
        StateThemeEvidenceCounts {
            required: 1,
            accounted: 1,
            applied: 0,
            not_applicable: 1,
            residual: 0,
        },
        "Dark site mainContrastColor"
    );

    let source_config = serde_json::json!({
        "theme": "dark",
        "themeVariables": { "mainContrastColor": MERMAID_COLOR },
    });
    let (source_svg, source_evidence) =
        render_strict_state_svg_with_theme_and_config(&theme, SOURCE, source_config, "source");
    assert!(
        source_svg.contains(TYPED_COLOR),
        "source directives cannot own the non-schema mainContrastColor route: {source_svg}"
    );
    assert_eq!(
        source_evidence,
        StateThemeEvidenceCounts {
            required: 1,
            accounted: 1,
            applied: 1,
            not_applicable: 0,
            residual: 0,
        },
        "Dark source mainContrastColor"
    );
}

#[test]
fn prepared_state_transition_labels_share_layout_background_and_baseline_geometry() {
    let source = "%%{init: {\"htmlLabels\": false}}%%\nstateDiagram-v2\nA --> B: Agjp";
    let mut heights = Vec::new();

    for font_size_px in [12.0, 28.0] {
        let (layout, svg) = render_state_layout_and_svg_from_text_with_theme_in_environment(
            source,
            &state_prepared_transition_label_theme(font_size_px),
            &RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable),
        );
        let layout_label = layout
            .edges
            .iter()
            .find(|edge| edge.id == "edge0")
            .and_then(|edge| edge.label.as_ref())
            .expect("prepared State edge layout label");
        let document = roxmltree::Document::parse(&svg).expect("valid prepared State SVG");
        let label = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("data-id") == Some("edge0")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "label")
                    })
            })
            .expect("prepared State edge label");
        let background = label
            .descendants()
            .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("background"))
            .expect("prepared State edge background");
        let text = label
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .expect("prepared State edge text");
        let row = text
            .children()
            .find(|node| {
                node.has_tag_name("tspan")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "row")
                    })
            })
            .expect("prepared State edge row");
        let width = background
            .attribute("width")
            .expect("background width")
            .parse::<f64>()
            .expect("numeric background width");
        let height = background
            .attribute("height")
            .expect("background height")
            .parse::<f64>()
            .expect("numeric background height");
        let baseline_y = row
            .attribute("y")
            .expect("explicit prepared baseline")
            .parse::<f64>()
            .expect("numeric prepared baseline");
        let label_origin = label
            .attribute("transform")
            .expect("layout-sized prepared edge label transform");
        let (label_origin_x, label_origin_y) = label_origin
            .strip_prefix("translate(")
            .and_then(|value| value.strip_suffix(')'))
            .and_then(|value| value.split_once(','))
            .map(|(x, y)| {
                (
                    x.trim().parse::<f64>().expect("numeric label origin x"),
                    y.trim().parse::<f64>().expect("numeric label origin y"),
                )
            })
            .expect("two-component prepared edge label transform");
        let text_origin = text
            .parent()
            .and_then(|node| node.attribute("transform"))
            .expect("prepared edge text origin");
        let (text_origin_x, text_origin_y) = text_origin
            .strip_prefix("translate(")
            .and_then(|value| value.strip_suffix(')'))
            .and_then(|value| value.split_once(','))
            .map(|(x, y)| {
                (
                    x.trim().parse::<f64>().expect("numeric text origin x"),
                    y.trim().parse::<f64>().expect("numeric text origin y"),
                )
            })
            .expect("two-component prepared edge text transform");

        assert!(
            text.attribute("style")
                .is_some_and(|style| style.contains("font-family:\"Excalifont\"")),
            "{svg}"
        );
        assert!(
            text.attribute("style").is_some_and(
                |style| style.contains(&format!("font-size:{font_size_px}px !important"))
            ),
            "{svg}"
        );
        assert_eq!(text.attribute("y"), None, "{svg}");
        assert_eq!(row.attribute("dy"), None, "{svg}");
        assert_eq!(
            row.descendants()
                .filter(|node| node.is_text())
                .filter_map(|node| node.text())
                .collect::<String>(),
            "Agjp",
            "{svg}"
        );
        assert!((layout_label.width - width).abs() <= 1e-9, "{svg}");
        assert!((layout_label.height - height).abs() <= 1e-9, "{svg}");
        assert!((label_origin_x + width / 2.0).abs() <= 1e-9, "{svg}");
        assert!((label_origin_y + height / 2.0).abs() <= 1e-9, "{svg}");
        let absolute_baseline_y = text_origin_y + baseline_y;
        assert!(
            absolute_baseline_y > 0.0 && absolute_baseline_y < height,
            "{svg}"
        );
        assert!((text_origin_x - width / 2.0).abs() <= 1e-9, "{svg}");
        assert!(text_origin_y.abs() <= 1e-9, "{svg}");
        heights.push(height);
    }

    assert!(heights[1] > heights[0] * 1.5, "{heights:?}");
}

#[test]
fn prepared_state_native_labels_share_one_explicit_baseline_contract() {
    const SOURCE: &str = r#"%%{init: {"htmlLabels": false}}%%
stateDiagram-v2
state "Ordinary Agjp" as Ordinary
Display : Ready
Display : Running
note right of Ordinary : Note Agjp
state Parent {
  Child
}
Ordinary --> Display: Edge Agjp
"#;
    let expected_rows = [
        vec!["Ordinary Agjp"],
        vec!["Note Agjp"],
        vec!["Ready"],
        vec!["Running"],
        vec!["Parent"],
        vec!["Child"],
        vec!["Edge Agjp"],
    ];
    let mut maximum_baselines = Vec::new();

    for font_size_px in [12.0, 28.0] {
        let (public_svg, svg, prepared_label_count, receipt_matches) =
            render_state_public_and_native_svg_from_text_with_theme_in_environment(
                SOURCE,
                &state_prepared_native_label_theme(font_size_px),
                &RenderEnvironment::deterministic().with_theme_portability_requirement(
                    ThemePortabilityRequirement::RequirePortable,
                ),
            );
        assert!(
            !public_svg.contains("merman-prepared-state-"),
            "public SVG must not expose renderer-owned prepared-label locators: {public_svg}"
        );
        assert_eq!(prepared_label_count, expected_rows.len(), "{svg}");
        assert!(
            receipt_matches,
            "terminal receipt must bind the native SVG: {svg}"
        );
        assert!(!svg.contains(r#"<text y="-10.1""#), "{svg}");
        assert!(!svg.contains(r#"dy="1.1em""#), "{svg}");

        let document = roxmltree::Document::parse(&svg).expect("valid prepared State SVG");
        let all_prepared_ids = document
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .filter_map(|node| node.attribute("id"))
            .filter(|id| id.starts_with("merman-prepared-state-"))
            .collect::<Vec<_>>();
        assert_eq!(all_prepared_ids.len(), expected_rows.len(), "{svg}");
        assert_eq!(
            all_prepared_ids
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            all_prepared_ids.len(),
            "every prepared State occurrence must own a unique terminal id: {svg}"
        );
        let mut label_ids = std::collections::BTreeSet::new();
        let mut maximum_baseline = 0.0_f64;

        for expected in &expected_rows {
            let text = document
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .find(|text| {
                    let rows = text
                        .children()
                        .filter(|node| {
                            node.has_tag_name("tspan")
                                && node.attribute("class").is_some_and(|classes| {
                                    classes.split_ascii_whitespace().any(|class| class == "row")
                                })
                        })
                        .map(|row| {
                            row.descendants()
                                .filter(|node| node.is_text())
                                .filter_map(|node| node.text())
                                .collect::<String>()
                        })
                        .collect::<Vec<_>>();
                    rows.iter().map(String::as_str).eq(expected.iter().copied())
                })
                .unwrap_or_else(|| panic!("missing prepared State rows {expected:?}: {svg}"));
            let label_id = text
                .attribute("id")
                .filter(|id| id.starts_with("merman-prepared-state-"))
                .unwrap_or_else(|| panic!("missing prepared State label id for {expected:?}"));
            assert!(
                label_ids.insert(label_id),
                "prepared State label ids must be unique: {label_id}"
            );
            assert_eq!(text.attribute("y"), None, "{expected:?}: {svg}");
            assert!(
                text.attribute("style").is_some_and(|style| {
                    style.contains("font-family:\"Excalifont\"")
                        && style.contains(&format!("font-size:{font_size_px}px !important"))
                }),
                "{expected:?}: {svg}"
            );

            let mut previous_baseline = None;
            let mut line_count = 0usize;
            for row in text.children().filter(|node| {
                node.has_tag_name("tspan")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_ascii_whitespace().any(|class| class == "row")
                    })
            }) {
                assert_eq!(row.attribute("dy"), None, "{expected:?}: {svg}");
                let baseline = row
                    .attribute("y")
                    .expect("prepared State row has an explicit baseline")
                    .parse::<f64>()
                    .expect("prepared State baseline is expressed in SVG user-space pixels");
                assert!(baseline > 0.0, "{expected:?}: {svg}");
                if let Some(previous) = previous_baseline {
                    assert!(baseline > previous, "{expected:?}: {svg}");
                }
                previous_baseline = Some(baseline);
                maximum_baseline = maximum_baseline.max(baseline);
                line_count += 1;
            }
            assert_eq!(line_count, expected.len(), "{expected:?}: {svg}");
        }

        assert_eq!(label_ids.len(), expected_rows.len(), "{svg}");
        maximum_baselines.push(maximum_baseline);
    }

    assert!(
        maximum_baselines[1] > maximum_baselines[0] * 1.5,
        "{maximum_baselines:?}"
    );
}

#[test]
fn native_state_transition_labels_share_layout_background_and_measured_text_origin() {
    fn parse_translate(value: &str) -> (f64, f64) {
        value
            .strip_prefix("translate(")
            .and_then(|value| value.strip_suffix(')'))
            .and_then(|value| value.split_once(','))
            .map(|(x, y)| {
                (
                    x.trim().parse::<f64>().expect("numeric translate x"),
                    y.trim().parse::<f64>().expect("numeric translate y"),
                )
            })
            .unwrap_or_else(|| panic!("two-component translate: {value}"))
    }

    let cases = [
        (
            "ordinary-single-default",
            "%%{init: {\"htmlLabels\": false}}%%\nstateDiagram-v2\nA --> B: Agjp",
            None,
            1,
        ),
        (
            "ordinary-multiline-large",
            "%%{init: {\"htmlLabels\": false}}%%\nstateDiagram-v2\nA --> B: Agjp<br/>second",
            Some(32.0),
            2,
        ),
        (
            "self-loop-single-large",
            "%%{init: {\"htmlLabels\": false}}%%\nstateDiagram-v2\nA --> A: Agjp",
            Some(32.0),
            1,
        ),
        (
            "self-loop-multiline-default",
            "%%{init: {\"htmlLabels\": false}}%%\nstateDiagram-v2\nA --> A: Agjp<br/>second",
            None,
            2,
        ),
    ];

    for (case, source, font_size_px, expected_lines) in cases {
        let svg = font_size_px.map_or_else(
            || render_state_svg_from_text(source),
            |font_size_px| {
                render_state_svg_from_text_with_theme(
                    source,
                    &state_transition_label_font_size_theme(font_size_px),
                )
            },
        );
        let document = roxmltree::Document::parse(&svg).expect("valid native State SVG");
        let label = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("data-id") == Some("edge0")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "label")
                    })
            })
            .unwrap_or_else(|| panic!("native State edge label for {case}: {svg}"));
        let background = label
            .descendants()
            .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("background"))
            .unwrap_or_else(|| panic!("native State edge background for {case}: {svg}"));
        let text = label
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap_or_else(|| panic!("native State edge text for {case}: {svg}"));
        let rows = text
            .children()
            .filter(|node| {
                node.has_tag_name("tspan")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "row")
                    })
            })
            .collect::<Vec<_>>();
        let width = background
            .attribute("width")
            .expect("background width")
            .parse::<f64>()
            .expect("numeric background width");
        let height = background
            .attribute("height")
            .expect("background height")
            .parse::<f64>()
            .expect("numeric background height");
        let (label_x, label_y) = parse_translate(
            label
                .attribute("transform")
                .expect("layout-sized label transform"),
        );
        let text_parent = text.parent().expect("native edge text parent");
        let (text_x, text_y) = parse_translate(
            text_parent
                .attribute("transform")
                .expect("measured text origin"),
        );

        assert_eq!(rows.len(), expected_lines, "{case}: {svg}");
        assert_eq!(text.attribute("id"), None, "{case} must stay non-prepared");
        assert!(!svg.contains("@font-face"), "{case} must stay non-prepared");
        assert!((label_x + width / 2.0).abs() <= 1e-9, "{case}: {svg}");
        assert!((label_y + height / 2.0).abs() <= 1e-9, "{case}: {svg}");
        assert!((text_x - width / 2.0).abs() <= 1e-9, "{case}: {svg}");
        let font_size_px = font_size_px.unwrap_or(16.0);
        let text_style = TextStyle {
            font_size: f64::from(font_size_px),
            ..TextStyle::default()
        };
        let measurer = VendoredFontMetricsTextMeasurer::default();
        let measured_text = if expected_lines == 1 {
            "Agjp"
        } else {
            "Agjp\nsecond"
        };
        let metrics =
            measurer.measure_wrapped(measured_text, &text_style, Some(200.0), WrapMode::SvgLike);
        let bbox_y = measurer.measure_svg_create_text_bbox_y_offset_px("Agjp", &text_style);
        let first_baseline_y = rows[0]
            .attribute("y")
            .expect("native State row has an explicit baseline")
            .parse::<f64>()
            .expect("numeric native State baseline");
        let expected_first_baseline_y = 2.0 + f64::from(font_size_px) - bbox_y;
        assert_eq!(metrics.line_count, expected_lines, "{case}: {svg}");
        assert!(
            (height - (metrics.height + 4.0)).abs() <= 1e-9,
            "{case}: {svg}"
        );
        assert!(
            text_y.abs() <= 1e-9,
            "{case}: explicit baselines must be relative to the layout box: {svg}"
        );
        assert!(
            (first_baseline_y - expected_first_baseline_y).abs() <= 1e-9,
            "{case}: measured baseline {expected_first_baseline_y}, actual {first_baseline_y}: {svg}"
        );
        assert!(
            (first_baseline_y - height / 2.0).abs() > 1e-9,
            "{case}: terminal baseline must not be independently re-centered by label height: {svg}"
        );
        let ink_top_y = first_baseline_y + bbox_y - f64::from(font_size_px);
        assert!((ink_top_y - 2.0).abs() <= 1e-9, "{case}: {svg}");
        assert!(
            (ink_top_y + metrics.height - (height - 2.0)).abs() <= 1e-9,
            "{case}: {svg}"
        );
    }
}

#[test]
fn native_state_transition_asymmetric_bbox_shares_text_background_and_layout_width() {
    #[derive(Default)]
    struct AsymmetricStateTextMeasurer {
        fallback: VendoredFontMetricsTextMeasurer,
    }

    impl TextMeasurer for AsymmetricStateTextMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            if text == "Skew" {
                let mut metrics = self.fallback.measure(text, style);
                metrics.width = 10.0;
                metrics
            } else {
                self.fallback.measure(text, style)
            }
        }

        fn measure_wrapped(
            &self,
            text: &str,
            style: &TextStyle,
            max_width: Option<f64>,
            wrap_mode: WrapMode,
        ) -> TextMetrics {
            if text == "Skew" {
                self.measure(text, style)
            } else {
                self.fallback
                    .measure_wrapped(text, style, max_width, wrap_mode)
            }
        }

        fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
            if text == "Skew" {
                10.0
            } else {
                self.fallback
                    .measure_svg_text_computed_length_px(text, style)
            }
        }

        fn measure_svg_text_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
            if text == "Skew" {
                (0.0, 20.0)
            } else {
                self.fallback.measure_svg_text_bbox_x(text, style)
            }
        }

        fn measure_svg_create_text_bbox_y_offset_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.fallback
                .measure_svg_create_text_bbox_y_offset_px(text, style)
        }
    }

    fn parse_translate(value: &str) -> (f64, f64) {
        value
            .strip_prefix("translate(")
            .and_then(|value| value.strip_suffix(')'))
            .and_then(|value| value.split_once(','))
            .map(|(x, y)| {
                (
                    x.trim().parse::<f64>().expect("numeric translate x"),
                    y.trim().parse::<f64>().expect("numeric translate y"),
                )
            })
            .unwrap_or_else(|| panic!("two-component translate: {value}"))
    }

    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.state-asymmetric-bbox").expect("valid profile id"),
        "test",
    )
    .expect("valid profile identity");
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            std::sync::Arc::new(AsymmetricStateTextMeasurer::default()),
        )),
    );
    let (layout, svg) = render_state_layout_and_svg_from_text_in_environment(
        "%%{init: {\"htmlLabels\": false}}%%\nstateDiagram-v2\nA --> B: Skew",
        &environment,
    );
    let layout_label = layout
        .edges
        .iter()
        .find(|edge| edge.id == "edge0")
        .and_then(|edge| edge.label.as_ref())
        .expect("State edge layout label");
    let document = roxmltree::Document::parse(&svg).expect("valid State SVG");
    let label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some("edge0")
                && node
                    .attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|class| class == "label"))
        })
        .expect("State edge SVG label");
    let background = label
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("background"))
        .expect("State edge label background");
    let text = label
        .descendants()
        .find(|node| node.has_tag_name("text"))
        .expect("State edge label text");
    let width = background
        .attribute("width")
        .expect("background width")
        .parse::<f64>()
        .expect("numeric background width");
    let (label_x, _) = parse_translate(
        label
            .attribute("transform")
            .expect("layout-sized label transform"),
    );
    let (text_x, _) = parse_translate(
        text.parent()
            .and_then(|node| node.attribute("transform"))
            .expect("centered text anchor transform"),
    );

    assert_eq!(
        text.descendants()
            .find(|node| node.has_tag_name("tspan") && node.text() == Some("Skew"))
            .and_then(|node| node.text()),
        Some("Skew"),
        "{svg}"
    );
    assert_eq!(width, 44.0, "{svg}");
    assert_eq!(layout_label.width, width, "{svg}");
    assert_eq!(label_x, -width / 2.0, "{svg}");
    assert_eq!(text_x, width / 2.0, "{svg}");
    assert!(text_x >= 2.0, "{svg}");
    assert_eq!(text_x + 20.0, width - 2.0, "{svg}");
}

#[test]
fn native_state_transition_auto_wrap_shares_tspans_background_and_layout_bounds() {
    const EDGE_WRAP_WIDTH_PX: f64 = 200.0;

    fn parse_translate(value: &str) -> (f64, f64) {
        value
            .strip_prefix("translate(")
            .and_then(|value| value.strip_suffix(')'))
            .and_then(|value| value.split_once(','))
            .map(|(x, y)| {
                (
                    x.trim().parse::<f64>().expect("numeric translate x"),
                    y.trim().parse::<f64>().expect("numeric translate y"),
                )
            })
            .unwrap_or_else(|| panic!("two-component translate: {value}"))
    }

    let long_word = "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW";
    let spaced_words = "MMMMMMMMMM MMMMMMMMMM";
    let cases = [
        (
            "ordinary-long-word",
            format!(
                "%%{{init: {{\"htmlLabels\": false}}}}%%\nstateDiagram-v2\nA --> B: {long_word}"
            ),
            long_word,
            false,
        ),
        (
            "ordinary-spaced-words",
            format!(
                "%%{{init: {{\"htmlLabels\": false}}}}%%\nstateDiagram-v2\nA --> B: {spaced_words}"
            ),
            spaced_words,
            true,
        ),
        (
            "self-loop-long-word",
            format!(
                "%%{{init: {{\"htmlLabels\": false}}}}%%\nstateDiagram-v2\nA --> A: {long_word}"
            ),
            long_word,
            false,
        ),
        (
            "self-loop-spaced-words",
            format!(
                "%%{{init: {{\"htmlLabels\": false}}}}%%\nstateDiagram-v2\nA --> A: {spaced_words}"
            ),
            spaced_words,
            true,
        ),
    ];
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let text_style = TextStyle::default();

    for (case, source, label_text, contains_space) in cases {
        let svg = render_state_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).expect("valid native State SVG");
        let label = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("data-id") == Some("edge0")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "label")
                    })
            })
            .unwrap_or_else(|| panic!("native State edge label for {case}: {svg}"));
        let background = label
            .descendants()
            .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("background"))
            .unwrap_or_else(|| panic!("native State edge background for {case}: {svg}"));
        let text = label
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap_or_else(|| panic!("native State edge text for {case}: {svg}"));
        let rows = text
            .children()
            .filter(|node| {
                node.has_tag_name("tspan")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "row")
                    })
            })
            .collect::<Vec<_>>();
        let emitted_lines = rows
            .iter()
            .map(|row| {
                row.descendants()
                    .filter(|node| node.is_text())
                    .filter_map(|node| node.text())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        let metrics = measurer.measure_wrapped(
            label_text,
            &text_style,
            Some(EDGE_WRAP_WIDTH_PX),
            WrapMode::SvgLike,
        );
        let width = background
            .attribute("width")
            .expect("background width")
            .parse::<f64>()
            .expect("numeric background width");
        let height = background
            .attribute("height")
            .expect("background height")
            .parse::<f64>()
            .expect("numeric background height");
        let (label_x, label_y) = parse_translate(
            label
                .attribute("transform")
                .expect("layout-sized label transform"),
        );
        let text_parent = text.parent().expect("native edge text parent");
        let (text_x, text_y) = parse_translate(
            text_parent
                .attribute("transform")
                .expect("measured text origin"),
        );
        assert!(rows.len() > 1, "{case} must auto-wrap: {svg}");
        assert_eq!(rows.len(), metrics.line_count, "{case}: {svg}");
        if contains_space {
            assert_eq!(emitted_lines.join(" "), label_text, "{case}: {svg}");
        } else {
            assert_eq!(emitted_lines.concat(), label_text, "{case}: {svg}");
        }
        let mut previous_baseline = None;
        for (row, line) in rows.iter().zip(&emitted_lines) {
            let line_width = measurer
                .measure_wrapped(line, &text_style, None, WrapMode::SvgLike)
                .width;
            assert!(line_width <= EDGE_WRAP_WIDTH_PX, "{case}: {line}: {svg}");
            let baseline_y = row
                .attribute("y")
                .expect("native row has an explicit user-space baseline")
                .parse::<f64>()
                .expect("numeric native row baseline");
            assert!(baseline_y > 0.0 && baseline_y < height, "{case}: {svg}");
            if let Some(previous) = previous_baseline {
                assert!(baseline_y > previous, "{case}: {svg}");
            }
            previous_baseline = Some(baseline_y);
            assert_eq!(row.attribute("dy"), None, "{case}: {svg}");
        }

        assert_eq!(text.attribute("y"), None, "{case}: {svg}");
        assert_eq!(text.attribute("id"), None, "{case} must stay non-prepared");
        assert!(!svg.contains("@font-face"), "{case} must stay non-prepared");
        let (ink_left, ink_right) =
            emitted_lines
                .iter()
                .fold((0.0_f64, 0.0_f64), |(maximum_left, maximum_right), line| {
                    let (left, right) = measurer.measure_svg_text_bbox_x(line, &text_style);
                    (maximum_left.max(left), maximum_right.max(right))
                });
        let expected_width = 2.0 * (ink_left.max(ink_right).max(metrics.width / 2.0) + 2.0);
        assert!((width - expected_width).abs() <= 1e-9, "{case}: {svg}");
        assert!(
            (height - (metrics.height + 4.0)).abs() <= 1e-9,
            "{case}: {svg}"
        );
        assert!((label_x + width / 2.0).abs() <= 1e-9, "{case}: {svg}");
        assert!((label_y + height / 2.0).abs() <= 1e-9, "{case}: {svg}");
        assert!((text_x - width / 2.0).abs() <= 1e-9, "{case}: {svg}");
        assert_eq!(text_y, 0.0, "{case}: {svg}");
    }
}

#[test]
fn state_note_compatibility_css_does_not_override_terminal_fallback_text() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(ThemeRule::new(
                        ThemeTarget::Note,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#422006").expect("valid note fill")),
                    ))
                    .with_rule(ThemeRule::new(
                        ThemeTarget::NoteLabel,
                        ThemeStylePatch::default().with_fill(
                            CanvasPaint::solid("#fef3c7").expect("valid note-label fill"),
                        ),
                    )),
            ),
        )
        .expect("compile State note text theme");
    let svg = render_state_svg_from_text_with_theme(
        r#"%%{init: {"htmlLabels": true}}%%
stateDiagram-v2
A
note right of A : terminal note text
"#,
        &theme,
    );

    assert!(
        svg.contains(".statediagram-note .noteLabel text{fill:"),
        "the compatibility color must be scoped to the native note-label subtree: {svg}"
    );
    assert!(
        !svg.contains(".statediagram-note text{fill:"),
        "a broad descendant selector would also capture postprocessed fallback text: {svg}"
    );
    assert!(
        svg.contains("color:#fef3c7 !important"),
        "the typed NoteLabel paint must remain attached to the HTML label before fallback conversion: {svg}"
    );

    let fallback = merman_render::svg::foreign_object_label_fallback_svg_text(
        &svg,
        &VendoredFontMetricsTextMeasurer::default(),
    );
    let document = roxmltree::Document::parse(&fallback).expect("valid fallback State SVG XML");
    let fallback_text = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.text() == Some("terminal note text")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|class| class == "merman-foreignobject-fallback-text")
                })
        })
        .expect("terminal State note fallback text");
    assert_eq!(fallback_text.attribute("fill"), Some("#fef3c7"));
}

#[test]
fn state_svg_title_theme_fill_reaches_the_terminal_svg_text_paint() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::Title,
                    ThemeStylePatch::default()
                        .with_fill(CanvasPaint::solid("#f43f5e").expect("valid title paint")),
                )),
            ),
        )
        .expect("compile State title theme");
    let svg = render_state_svg_from_text_with_theme(
        r#"---
title: Terminal title
---
stateDiagram-v2
[*] --> Ready
Ready --> [*]
"#,
        &theme,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid themed State SVG XML");
    let title = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text") && node.attribute("class") == Some("statediagramTitleText")
        })
        .expect("terminal State diagram title");
    let style = title.attribute("style").expect("typed title style");

    assert!(style.contains("fill:#f43f5e !important"), "{style}");
    assert!(!style.contains("color:#f43f5e"), "{style}");
}

#[test]
fn state_title_terminal_ownership_uses_text_color_not_title_color() {
    const TYPED_COLOR: &str = "#f43f5e";
    const NON_OWNER_COLOR: &str = "#22c55e";
    const SOURCE: &str = r#"---
title: Terminal title
---
stateDiagram-v2
Ready
"#;

    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Title,
                    ThemeStylePatch::default().with_fill(
                        CanvasPaint::solid(TYPED_COLOR).expect("valid State title paint"),
                    ),
                ),
            ),
        ))
        .expect("compile State title ownership theme");
    let (svg, evidence) = render_strict_state_svg_with_theme_and_config(
        &theme,
        SOURCE,
        serde_json::json!({
            "theme": "base",
            "themeVariables": { "titleColor": NON_OWNER_COLOR },
        }),
        "site",
    );
    let document = roxmltree::Document::parse(&svg).expect("valid State title ownership SVG XML");
    let title = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text") && node.attribute("class") == Some("statediagramTitleText")
        })
        .expect("terminal State title text");

    assert!(
        title
            .attribute("style")
            .is_some_and(|style| style.contains("fill:#f43f5e !important")),
        "titleColor must not suppress the typed State Title fill: {svg}"
    );
    assert!(
        !svg.contains(NON_OWNER_COLOR),
        "State title compatibility paint must come from textColor, not titleColor: {svg}"
    );
    assert_eq!(
        evidence,
        StateThemeEvidenceCounts {
            required: 1,
            accounted: 1,
            applied: 1,
            not_applicable: 0,
            residual: 0,
        }
    );
}

#[test]
fn state_svg_split_rough_surfaces_keep_fill_and_stroke_styles_on_their_own_paths() {
    let split_paint = || {
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#f43f5e").expect("valid split-surface fill"))
            .with_stroke(CanvasPaint::solid("#2563eb").expect("valid split-surface stroke"))
            .with_stroke_width(6.0)
            .expect("valid split-surface stroke width")
    };
    let styles = ThemeRuleSet::default()
        .with_rule(ThemeRule::new(ThemeTarget::State, split_paint()))
        .with_rule(ThemeRule::new(ThemeTarget::Note, split_paint()))
        .with_rule(
            ThemeRule::new(ThemeTarget::SpecialState, split_paint())
                .with_variant(ThemeVariant::Start),
        )
        .with_rule(
            ThemeRule::new(ThemeTarget::SpecialState, split_paint())
                .with_variant(ThemeVariant::End),
        )
        .with_rule(
            ThemeRule::new(ThemeTarget::SpecialStateInner, split_paint())
                .with_variant(ThemeVariant::End),
        )
        .with_rule(
            ThemeRule::new(ThemeTarget::SpecialState, split_paint())
                .with_variant(ThemeVariant::Special),
        );
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile split-surface State theme");
    let svg = render_state_svg_from_text_with_theme(
        r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
stateDiagram-v2
[*] --> Idle
state Decide <<choice>>
state Fork <<fork>>
state Join <<join>>
Idle --> Decide
Decide --> Fork
Fork --> Join
Join --> [*]
note right of Idle : split surface
"#,
        &theme,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid themed State SVG XML");
    let split_path_pairs = document
        .descendants()
        .filter(|node| node.has_tag_name("g"))
        .filter_map(|group| {
            let paths = group
                .children()
                .filter(|child| child.has_tag_name("path"))
                .collect::<Vec<_>>();
            (paths.len() == 2).then(|| (paths[0], paths[1]))
        })
        .collect::<Vec<_>>();

    assert_eq!(
        split_path_pairs.len(),
        8,
        "expected start, ordinary, choice, fork, join, note, and both end-state surfaces: {svg}"
    );
    for (fill_path, stroke_path) in split_path_pairs {
        assert_eq!(fill_path.attribute("stroke"), Some("none"));
        assert_eq!(stroke_path.attribute("fill"), Some("none"));

        let fill_style = fill_path.attribute("style").unwrap_or_default();
        let stroke_style = stroke_path.attribute("style").unwrap_or_default();
        assert!(
            fill_style.contains("fill:#f43f5e !important"),
            "fill path must retain the typed fill: {fill_style}"
        );
        assert!(
            !fill_style
                .split(';')
                .any(|declaration| declaration.trim_start().starts_with("stroke")),
            "fill path must not acquire stroke declarations: {fill_style}"
        );
        assert!(
            stroke_style.contains("stroke:#2563eb !important")
                && stroke_style.contains("stroke-width:6px !important"),
            "stroke path must retain the typed stroke: {stroke_style}"
        );
        assert!(
            !stroke_style
                .split(';')
                .any(|declaration| declaration.trim_start().starts_with("fill")),
            "stroke path must not acquire fill declarations: {stroke_style}"
        );
    }
}

fn render_state_svg_with_hand_drawn_seed(seed: u64) -> String {
    let site_config = MermaidConfig::from_value(serde_json::json!({
        "look": "handDrawn",
        "handDrawnSeed": seed,
        "themeVariables": {
            "stateBkg": "#101827",
            "stateBorder": "#38bdf8",
            "nodeBorder": "#a855f7",
            "mainBkg": "#0f172a",
            "lineColor": "#e11d48",
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
        seed_7.contains(r##"fill="#0f172a""##)
            && seed_7.contains(r##"stroke="#a855f7" stroke-width="1.3""##),
        "seed test should exercise ordinary visible rough paths with Mermaid's hand-drawn mainBkg/nodeBorder/1.3 fallback: {seed_7}"
    );
    assert!(
        seed_7.contains(r##"fill="#fef3c7""##)
            && seed_7.contains(r##"stroke="#92400e" stroke-width="1.3""##),
        "seed test should exercise note rough paths as a second visible consumer: {seed_7}"
    );
}

#[test]
fn state_hand_drawn_terminals_follow_shape_specific_mermaid_fallbacks() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "look": "handDrawn",
        "handDrawnSeed": 7,
        "htmlLabels": false,
        "themeVariables": {
            "mainBkg": "#110011",
            "nodeBorder": "#220022",
            "stateBkg": "#330033",
            "stateBorder": "#440044",
            "strokeWidth": 9,
            "lineColor": "#550055",
            "specialStateColor": "#660066",
            "noteBkgColor": "#770077",
            "noteBorderColor": "#880088",
            "compositeTitleBackground": "#990099",
            "compositeBackground": "#aa00aa",
            "altBackground": "#bb00bb",
            "background": "#cc00cc"
        }
    })));
    let svg = render_state_svg_from_text_with_engine(
        engine,
        r#"stateDiagram-v2
[*] --> Idle
state Decide <<choice>>
state Fork <<fork>>
state Join <<join>>
Idle --> Decide
Decide --> Fork
Fork --> Join
Join --> [*]
note right of Idle : terminal note
state Parent {
  [*] --> Child
}
state Outer {
  state Inner {
    [*] --> Nested
  }
}
state Parallel {
  [*] --> First
  --
  [*] --> Second
}
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid HandDrawn State SVG XML");
    let group = |id_fragment: &str| {
        document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class") != Some("note-cluster")
                    && node
                        .attribute("id")
                        .is_some_and(|id| id.contains(id_fragment))
            })
            .unwrap_or_else(|| panic!("missing State group {id_fragment}: {svg}"))
    };

    assert_state_rough_path_pair(
        group("-state-Idle-"),
        Some("outer-path"),
        "#110011",
        "#220022",
        "1.3",
    );
    assert_state_rough_path_pair(
        group("-state-root_start-"),
        Some("outer-path"),
        "#550055",
        "#550055",
        "1",
    );
    assert_state_rough_path_pair(group("-state-Decide-"), None, "#110011", "#220022", "1.3");
    assert_state_rough_path_pair(group("-state-Fork-"), None, "#550055", "#550055", "1.3");
    assert_state_rough_path_pair(group("-state-Join-"), None, "#550055", "#550055", "1.3");

    let end = group("-state-root_end-");
    assert_state_rough_path_pair(end, Some("outer-path"), "#110011", "#550055", "2");
    let end_inner = end
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.parent().is_some_and(|parent| {
                    parent.has_tag_name("g") && parent.attribute("class") == Some("outer-path")
                })
        })
        .expect("HandDrawn State end inner path pair");
    assert_state_rough_path_pair(end_inner, None, "#440044", "#440044", "2");

    let note = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|class| class == "statediagram-note")
                })
        })
        .expect("HandDrawn State note group");
    assert_state_rough_path_pair(note, Some("outer-path"), "#770077", "#880088", "1.3");

    for (cluster_id, body_fill) in [("Parent", "#aa00aa"), ("Inner", "#bb00bb")] {
        let cluster = document
            .descendants()
            .find(|node| node.has_tag_name("g") && node.attribute("data-id") == Some(cluster_id))
            .unwrap_or_else(|| panic!("missing HandDrawn State cluster {cluster_id}: {svg}"));
        assert_state_rough_path_pair(cluster, Some("outer"), "#990099", "#220022", "1");
        assert_state_rough_path_pair(cluster, Some("inner"), body_fill, "#220022", "1");
    }

    let divider = group("-state-divider-id-1-");
    assert_state_rough_path_pair(divider, Some("divider"), "lightgrey", "#220022", "1");
    let divider_stroke = divider
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("divider"))
        .and_then(|pair| {
            pair.children()
                .filter(|node| node.has_tag_name("path"))
                .nth(1)
        })
        .expect("HandDrawn State divider stroke path");
    assert_eq!(divider_stroke.attribute("stroke-dasharray"), Some("5"));
}

#[test]
fn state_hand_drawn_terminal_owners_follow_distinct_surfaces_from_site_and_source() {
    const TYPED_COLOR: &str = "#ec4899";
    const MERMAID_COLOR: &str = "#22c55e";
    const ORDINARY: &str = "stateDiagram-v2\nReady\n";
    const NOTE: &str = "stateDiagram-v2\nReady\nnote right of Ready : terminal note\n";
    const START: &str = "stateDiagram-v2\n[*] --> Ready\n";
    const END: &str = "stateDiagram-v2\nReady --> [*]\n";
    const FORK: &str = "stateDiagram-v2\nstate Fork <<fork>>\n";
    const JOIN: &str = "stateDiagram-v2\nstate Join <<join>>\n";
    const CHOICE: &str = "stateDiagram-v2\nstate Decide <<choice>>\n";
    const COMPOSITE: &str = "stateDiagram-v2\nstate Parent {\n  [*] --> Ready\n}\n";
    const NESTED_COMPOSITE: &str =
        "stateDiagram-v2\nstate Outer {\n  state Inner {\n    [*] --> Ready\n  }\n}\n";
    const DIVIDER: &str =
        "stateDiagram-v2\nstate Parallel {\n  [*] --> First\n  --\n  [*] --> Second\n}\n";

    #[derive(Clone, Copy)]
    enum PaintFacet {
        Fill,
        Stroke,
    }

    struct Case {
        name: &'static str,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
        facet: PaintFacet,
        source: &'static str,
        config_key: &'static str,
    }

    let cases = [
        Case {
            name: "ordinary-fill",
            target: ThemeTarget::State,
            variant: ThemeVariant::Default,
            ordinal: None,
            facet: PaintFacet::Fill,
            source: ORDINARY,
            config_key: "mainBkg",
        },
        Case {
            name: "ordinary-stroke",
            target: ThemeTarget::State,
            variant: ThemeVariant::Default,
            ordinal: None,
            facet: PaintFacet::Stroke,
            source: ORDINARY,
            config_key: "nodeBorder",
        },
        Case {
            name: "note-fill",
            target: ThemeTarget::Note,
            variant: ThemeVariant::Default,
            ordinal: None,
            facet: PaintFacet::Fill,
            source: NOTE,
            config_key: "noteBkgColor",
        },
        Case {
            name: "note-stroke",
            target: ThemeTarget::Note,
            variant: ThemeVariant::Default,
            ordinal: None,
            facet: PaintFacet::Stroke,
            source: NOTE,
            config_key: "noteBorderColor",
        },
        Case {
            name: "start-fill",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Start,
            ordinal: None,
            facet: PaintFacet::Fill,
            source: START,
            config_key: "lineColor",
        },
        Case {
            name: "start-stroke",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Start,
            ordinal: None,
            facet: PaintFacet::Stroke,
            source: START,
            config_key: "lineColor",
        },
        Case {
            name: "composite-header-fill",
            target: ThemeTarget::CompositeHeader,
            variant: ThemeVariant::Default,
            ordinal: None,
            facet: PaintFacet::Fill,
            source: COMPOSITE,
            config_key: "compositeTitleBackground",
        },
        Case {
            name: "composite-header-stroke",
            target: ThemeTarget::CompositeHeader,
            variant: ThemeVariant::Default,
            ordinal: None,
            facet: PaintFacet::Stroke,
            source: COMPOSITE,
            config_key: "nodeBorder",
        },
        Case {
            name: "composite-body-fill",
            target: ThemeTarget::Composite,
            variant: ThemeVariant::Default,
            ordinal: None,
            facet: PaintFacet::Fill,
            source: COMPOSITE,
            config_key: "compositeBackground",
        },
        Case {
            name: "composite-body-stroke",
            target: ThemeTarget::Composite,
            variant: ThemeVariant::Default,
            ordinal: None,
            facet: PaintFacet::Stroke,
            source: COMPOSITE,
            config_key: "nodeBorder",
        },
        Case {
            name: "composite-alt-fill",
            target: ThemeTarget::Composite,
            variant: ThemeVariant::Default,
            ordinal: Some(2),
            facet: PaintFacet::Fill,
            source: NESTED_COMPOSITE,
            config_key: "altBackground",
        },
        Case {
            name: "divider-stroke",
            target: ThemeTarget::Composite,
            variant: ThemeVariant::Default,
            ordinal: Some(2),
            facet: PaintFacet::Stroke,
            source: DIVIDER,
            config_key: "nodeBorder",
        },
        Case {
            name: "end-outer-fill",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::End,
            ordinal: None,
            facet: PaintFacet::Fill,
            source: END,
            config_key: "mainBkg",
        },
        Case {
            name: "end-outer-stroke",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::End,
            ordinal: None,
            facet: PaintFacet::Stroke,
            source: END,
            config_key: "lineColor",
        },
        Case {
            name: "end-inner-fill",
            target: ThemeTarget::SpecialStateInner,
            variant: ThemeVariant::End,
            ordinal: None,
            facet: PaintFacet::Fill,
            source: END,
            config_key: "nodeBorder",
        },
        Case {
            name: "end-inner-stroke",
            target: ThemeTarget::SpecialStateInner,
            variant: ThemeVariant::End,
            ordinal: None,
            facet: PaintFacet::Stroke,
            source: END,
            config_key: "nodeBorder",
        },
        Case {
            name: "fork-fill",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Special,
            ordinal: None,
            facet: PaintFacet::Fill,
            source: FORK,
            config_key: "lineColor",
        },
        Case {
            name: "join-stroke",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Special,
            ordinal: None,
            facet: PaintFacet::Stroke,
            source: JOIN,
            config_key: "lineColor",
        },
        Case {
            name: "choice-fill",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Special,
            ordinal: None,
            facet: PaintFacet::Fill,
            source: CHOICE,
            config_key: "mainBkg",
        },
        Case {
            name: "choice-stroke",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Special,
            ordinal: None,
            facet: PaintFacet::Stroke,
            source: CHOICE,
            config_key: "nodeBorder",
        },
    ];

    for case in cases {
        let style = match case.facet {
            PaintFacet::Fill => ThemeStylePatch::default().with_fill(
                CanvasPaint::solid(TYPED_COLOR).expect("valid typed HandDrawn terminal fill"),
            ),
            PaintFacet::Stroke => ThemeStylePatch::default().with_stroke(
                CanvasPaint::solid(TYPED_COLOR).expect("valid typed HandDrawn terminal stroke"),
            ),
        };
        let mut rule = ThemeRule::new(case.target, style).with_variant(case.variant);
        if let Some(ordinal) = case.ordinal {
            rule = rule.with_ordinal(
                OrdinalSelector::exact(ordinal).expect("valid HandDrawn terminal ordinal"),
            );
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
            .unwrap_or_else(|error| panic!("compile {} theme: {error}", case.name));

        for origin in ["site", "source"] {
            let (svg, evidence) = render_strict_state_svg_with_theme_and_config(
                &theme,
                case.source,
                serde_json::json!({
                    "theme": "base",
                    "look": "handDrawn",
                    "handDrawnSeed": 7,
                    "htmlLabels": false,
                    "themeVariables": { (case.config_key): MERMAID_COLOR },
                }),
                origin,
            );
            let expected_attribute = match case.facet {
                PaintFacet::Fill => format!("fill=\"{MERMAID_COLOR}\""),
                PaintFacet::Stroke => format!("stroke=\"{MERMAID_COLOR}\""),
            };

            assert!(
                svg.contains(&expected_attribute),
                "explicit {origin} {} must reach a HandDrawn terminal: {svg}",
                case.name
            );
            assert!(
                !svg.contains(TYPED_COLOR),
                "typed paint must yield to explicit {origin} {}: {svg}",
                case.name
            );
            assert_eq!(
                evidence,
                StateThemeEvidenceCounts {
                    required: 1,
                    accounted: 1,
                    applied: 0,
                    not_applicable: 1,
                    residual: 0,
                },
                "{origin} {}",
                case.name
            );
        }
    }

    let divider_fill_theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Composite,
                        ThemeStylePatch::default().with_fill(
                            CanvasPaint::solid(TYPED_COLOR)
                                .expect("valid typed HandDrawn divider fill"),
                        ),
                    )
                    .with_ordinal(OrdinalSelector::exact(2).expect("valid divider ordinal")),
                ),
            ),
        )
        .expect("compile HandDrawn divider fill theme");
    for origin in ["site", "source"] {
        let (svg, evidence) = render_strict_state_svg_with_theme_and_config(
            &divider_fill_theme,
            DIVIDER,
            serde_json::json!({
                "theme": "base",
                "look": "handDrawn",
                "handDrawnSeed": 7,
                "htmlLabels": false,
                "themeVariables": { "altBackground": MERMAID_COLOR },
            }),
            origin,
        );
        assert!(
            svg.contains("fill:#ec4899 !important"),
            "HandDrawn divider fill must remain typed because its Mermaid fallback is constant lightgrey, not {origin} altBackground: {svg}"
        );
        assert_eq!(
            evidence,
            StateThemeEvidenceCounts {
                required: 1,
                accounted: 1,
                applied: 1,
                not_applicable: 0,
                residual: 0,
            },
            "{origin} divider fill"
        );
    }
}

#[test]
fn state_hand_drawn_rect_with_title_shares_typed_stroke_with_its_divider() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::State,
                    ThemeStylePatch::default()
                        .with_stroke(
                            CanvasPaint::solid("#ec4899")
                                .expect("valid HandDrawn rectWithTitle stroke"),
                        )
                        .with_stroke_width(3.0)
                        .expect("valid HandDrawn rectWithTitle stroke width"),
                )),
            ),
        )
        .expect("compile HandDrawn rectWithTitle theme");
    let (svg, evidence) = render_strict_state_svg_with_theme_and_config(
        &theme,
        "stateDiagram-v2\nDisplay : Ready\nDisplay : Running\n",
        serde_json::json!({
            "theme": "base",
            "look": "handDrawn",
            "handDrawnSeed": 7,
        }),
        "site",
    );
    let document = roxmltree::Document::parse(&svg).expect("valid HandDrawn State SVG");
    let container = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-look") == Some("handDrawn")
                && node.descendants().any(|child| {
                    child.has_tag_name("rect")
                        && child.attribute("class") == Some("outer title-state")
                })
        })
        .expect("HandDrawn rectWithTitle container");
    let outer = container
        .descendants()
        .find(|node| {
            node.has_tag_name("rect") && node.attribute("class") == Some("outer title-state")
        })
        .expect("HandDrawn rectWithTitle outer rect");
    let divider = container
        .descendants()
        .find(|node| node.has_tag_name("line") && node.attribute("class") == Some("divider"))
        .expect("HandDrawn rectWithTitle divider");
    for (surface, node) in [("outer", outer), ("divider", divider)] {
        let style = node.attribute("style").unwrap_or_default();
        assert!(
            style.contains("stroke:#ec4899 !important"),
            "typed State stroke must reach the HandDrawn rectWithTitle {surface}: {svg}"
        );
        assert!(
            style.contains("stroke-width:3px !important"),
            "typed State stroke width must reach the HandDrawn rectWithTitle {surface}: {svg}"
        );
    }
    assert_eq!(
        evidence,
        StateThemeEvidenceCounts {
            required: 1,
            accounted: 1,
            applied: 1,
            not_applicable: 0,
            residual: 0,
        }
    );
}

#[test]
fn state_neo_zero_stroke_width_owns_composite_and_special_path_terminals() {
    let width = || {
        ThemeStylePatch::default()
            .with_stroke_width(3.0)
            .expect("valid typed State terminal stroke width")
    };
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(ThemeRule::new(ThemeTarget::Composite, width()))
                    .with_rule(
                        ThemeRule::new(ThemeTarget::SpecialState, width())
                            .with_variant(ThemeVariant::Special),
                    ),
            ),
        )
        .expect("compile Neo zero-width ownership theme");
    let source = "stateDiagram-v2\nstate Parent {\n  [*] --> Running\n}\nstate Decide <<choice>>\n";

    for origin in ["site", "source"] {
        let (svg, evidence) = render_strict_state_svg_with_theme_and_config(
            &theme,
            source,
            serde_json::json!({
                "theme": "base",
                "look": "neo",
                "themeVariables": { "strokeWidth": 0 },
            }),
            origin,
        );

        assert!(
            svg.contains("[data-look=\"neo\"].statediagram-cluster rect{")
                && svg.contains("stroke-width:0;}"),
            "explicit {origin} strokeWidth=0 must reach the Neo composite terminal: {svg}"
        );
        assert!(
            svg.contains("[data-look=\"neo\"].node path{") && svg.contains("stroke-width:0px;}"),
            "explicit {origin} strokeWidth=0 must reach the Neo special path terminal: {svg}"
        );
        assert!(
            !svg.contains("stroke-width:3px !important"),
            "typed stroke width must yield to explicit {origin} strokeWidth=0: {svg}"
        );
        assert_eq!(
            evidence,
            StateThemeEvidenceCounts {
                required: 2,
                accounted: 2,
                applied: 0,
                not_applicable: 2,
                residual: 0,
            },
            "{origin} Neo zero-width ownership"
        );
    }
}

#[test]
fn state_stroke_width_terminal_ownership_follows_surface_and_look() {
    const TYPED_WIDTH: &str = "stroke-width:3px !important";
    const SOURCE_STATE: &str = "stateDiagram-v2\nReady\n";
    const SOURCE_TRANSITION: &str = "stateDiagram-v2\nReady --> Done\n";
    const SOURCE_COMPOSITE: &str = "stateDiagram-v2\nstate Parent {\n  [*] --> Ready\n}\n";
    const SOURCE_DIVIDER: &str =
        "stateDiagram-v2\nstate Parallel {\n  [*] --> First\n  --\n  [*] --> Second\n}\n";
    const SOURCE_NOTE: &str = "stateDiagram-v2\nReady\nnote right of Ready : terminal note\n";
    const SOURCE_START: &str = "stateDiagram-v2\n[*] --> Ready\n";
    const SOURCE_END: &str = "stateDiagram-v2\nReady --> [*]\n";
    const SOURCE_FORK: &str = "stateDiagram-v2\nstate Fork <<fork>>\n";
    const SOURCE_CHOICE: &str = "stateDiagram-v2\nstate Decide <<choice>>\n";

    struct OwnedCase {
        name: &'static str,
        target: ThemeTarget,
        variant: ThemeVariant,
        source: &'static str,
        look: &'static str,
        terminal_fragment: &'static str,
    }

    let owned_cases = [
        OwnedCase {
            name: "classic-state",
            target: ThemeTarget::State,
            variant: ThemeVariant::Default,
            source: SOURCE_STATE,
            look: "classic",
            terminal_fragment: "stroke-width:7px;}",
        },
        OwnedCase {
            name: "neo-state",
            target: ThemeTarget::State,
            variant: ThemeVariant::Default,
            source: SOURCE_STATE,
            look: "neo",
            terminal_fragment: "stroke-width:7px;}",
        },
        OwnedCase {
            name: "transition",
            target: ThemeTarget::Transition,
            variant: ThemeVariant::Default,
            source: SOURCE_TRANSITION,
            look: "classic",
            terminal_fragment: "stroke-width:7;fill:none;}",
        },
        OwnedCase {
            name: "classic-composite",
            target: ThemeTarget::Composite,
            variant: ThemeVariant::Default,
            source: SOURCE_COMPOSITE,
            look: "classic",
            terminal_fragment: "stroke-width:7px;}",
        },
        OwnedCase {
            name: "neo-composite",
            target: ThemeTarget::Composite,
            variant: ThemeVariant::Default,
            source: SOURCE_COMPOSITE,
            look: "neo",
            terminal_fragment: "stroke-width:7;}",
        },
        OwnedCase {
            name: "neo-path",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Special,
            source: SOURCE_CHOICE,
            look: "neo",
            terminal_fragment: "stroke-width:7px;}",
        },
    ];

    for case in owned_cases {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            case.target,
                            ThemeStylePatch::default()
                                .with_stroke_width(3.0)
                                .expect("valid State terminal stroke width"),
                        )
                        .with_variant(case.variant),
                    ),
                ),
            )
            .unwrap_or_else(|error| panic!("compile {} width theme: {error}", case.name));

        for origin in ["site", "source"] {
            let (svg, evidence) = render_strict_state_svg_with_theme_and_config(
                &theme,
                case.source,
                serde_json::json!({
                    "theme": "base",
                    "look": case.look,
                    "themeVariables": { "strokeWidth": 7 },
                }),
                origin,
            );

            assert!(
                svg.contains(case.terminal_fragment),
                "explicit {origin} strokeWidth must reach the {} terminal: {svg}",
                case.name
            );
            assert!(
                !svg.contains(TYPED_WIDTH),
                "typed width must yield to explicit {origin} strokeWidth on {}: {svg}",
                case.name
            );
            assert_eq!(
                evidence,
                StateThemeEvidenceCounts {
                    required: 1,
                    accounted: 1,
                    applied: 0,
                    not_applicable: 1,
                    residual: 0,
                },
                "{origin} {}",
                case.name
            );
        }
    }

    struct UnownedCase {
        name: &'static str,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
        source: &'static str,
        look: &'static str,
    }

    let unowned_cases = [
        UnownedCase {
            name: "hand-drawn-state",
            target: ThemeTarget::State,
            variant: ThemeVariant::Default,
            ordinal: None,
            source: SOURCE_STATE,
            look: "handDrawn",
        },
        UnownedCase {
            name: "hand-drawn-note",
            target: ThemeTarget::Note,
            variant: ThemeVariant::Default,
            ordinal: None,
            source: SOURCE_NOTE,
            look: "handDrawn",
        },
        UnownedCase {
            name: "hand-drawn-start",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Start,
            ordinal: None,
            source: SOURCE_START,
            look: "handDrawn",
        },
        UnownedCase {
            name: "hand-drawn-composite-body",
            target: ThemeTarget::Composite,
            variant: ThemeVariant::Default,
            ordinal: None,
            source: SOURCE_COMPOSITE,
            look: "handDrawn",
        },
        UnownedCase {
            name: "hand-drawn-composite-header",
            target: ThemeTarget::CompositeHeader,
            variant: ThemeVariant::Default,
            ordinal: None,
            source: SOURCE_COMPOSITE,
            look: "handDrawn",
        },
        UnownedCase {
            name: "hand-drawn-divider",
            target: ThemeTarget::Composite,
            variant: ThemeVariant::Default,
            ordinal: Some(2),
            source: SOURCE_DIVIDER,
            look: "handDrawn",
        },
        UnownedCase {
            name: "hand-drawn-end-outer",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::End,
            ordinal: None,
            source: SOURCE_END,
            look: "handDrawn",
        },
        UnownedCase {
            name: "hand-drawn-end-inner",
            target: ThemeTarget::SpecialStateInner,
            variant: ThemeVariant::End,
            ordinal: None,
            source: SOURCE_END,
            look: "handDrawn",
        },
        UnownedCase {
            name: "hand-drawn-fork",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Special,
            ordinal: None,
            source: SOURCE_FORK,
            look: "handDrawn",
        },
        UnownedCase {
            name: "hand-drawn-choice",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Special,
            ordinal: None,
            source: SOURCE_CHOICE,
            look: "handDrawn",
        },
        UnownedCase {
            name: "classic-choice",
            target: ThemeTarget::SpecialState,
            variant: ThemeVariant::Special,
            ordinal: None,
            source: SOURCE_CHOICE,
            look: "classic",
        },
    ];

    for case in unowned_cases {
        let mut rule = ThemeRule::new(
            case.target,
            ThemeStylePatch::default()
                .with_stroke_width(3.0)
                .expect("valid HandDrawn State terminal stroke width"),
        )
        .with_variant(case.variant);
        if let Some(ordinal) = case.ordinal {
            rule = rule.with_ordinal(
                OrdinalSelector::exact(ordinal).expect("valid HandDrawn width ordinal"),
            );
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
            .unwrap_or_else(|error| panic!("compile {} width theme: {error}", case.name));

        for origin in ["site", "source"] {
            let (svg, evidence) = render_strict_state_svg_with_theme_and_config(
                &theme,
                case.source,
                serde_json::json!({
                    "theme": "base",
                    "look": case.look,
                    "handDrawnSeed": 7,
                    "themeVariables": { "strokeWidth": 7 },
                }),
                origin,
            );
            assert!(
                svg.contains(TYPED_WIDTH),
                "global {origin} strokeWidth must not own the {} terminal: {svg}",
                case.name
            );
            assert_eq!(
                evidence,
                StateThemeEvidenceCounts {
                    required: 1,
                    accounted: 1,
                    applied: 1,
                    not_applicable: 0,
                    residual: 0,
                },
                "{origin} {}",
                case.name
            );
        }
    }
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
        root_false.contains(r#"class="text-outer-tspan row""#)
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
        svg.contains(r#"class="cluster-label""#) && svg.contains(r#"class="text-outer-tspan row""#),
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
            "nodeBorder": "#a855f7",
            "mainBkg": "#0f172a",
            "lineColor": "#e11d48",
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
state Fork <<fork>>
state Join <<join>>
Idle --> Decide
Decide --> Fork
Fork --> Join
Join --> [*]
note right of Idle : themed note"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid themed State rough SVG XML");
    let special_paths = |id: &str| {
        let group = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node
                        .attribute("id")
                        .is_some_and(|value| value.contains(&format!("-state-{id}-")))
            })
            .unwrap_or_else(|| panic!("missing {id} State special node: {svg}"));
        let mut paths = group.descendants().filter(|node| node.has_tag_name("path"));
        (
            paths.next().expect("special-state fill path"),
            paths.next().expect("special-state stroke path"),
        )
    };
    let (choice_fill, choice_stroke) = special_paths("Decide");
    let (fork_fill, fork_stroke) = special_paths("Fork");
    let end_outer = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class") == Some("outer-path")
                && node.children().any(|child| child.has_tag_name("g"))
        })
        .expect("State end outer-path group");
    let mut end_outer_paths = end_outer
        .children()
        .filter(|node| node.has_tag_name("path"));
    let end_outer_fill = end_outer_paths.next().expect("State end outer fill path");
    let end_outer_stroke = end_outer_paths.next().expect("State end outer stroke path");
    let end_inner = end_outer
        .children()
        .find(|node| node.has_tag_name("g"))
        .expect("State end inner group");
    let mut end_inner_paths = end_inner
        .children()
        .filter(|node| node.has_tag_name("path"));
    let end_inner_fill = end_inner_paths.next().expect("State end inner fill path");
    let end_inner_stroke = end_inner_paths.next().expect("State end inner stroke path");

    assert!(
        svg.contains(r##".node rect{fill:#101827;stroke:#38bdf8;stroke-width:4px;}"##),
        "classic ordinary State rects should consume stateBkg/stateBorder/strokeWidth through CSS: {svg}"
    );
    assert_eq!(choice_fill.attribute("fill"), Some("#0f172a"), "{svg}");
    assert_eq!(choice_stroke.attribute("stroke"), Some("#a855f7"), "{svg}");
    assert_eq!(fork_fill.attribute("fill"), Some("#e11d48"), "{svg}");
    assert_eq!(fork_stroke.attribute("stroke"), Some("#e11d48"), "{svg}");
    assert!(
        svg.contains(r##".node circle.state-start{fill:#f97316;stroke:#f97316;}"##),
        "start-state styling should consume specialStateColor: {svg}"
    );
    assert_eq!(end_outer_fill.attribute("fill"), Some("#0f172a"), "{svg}");
    assert_eq!(
        end_outer_stroke.attribute("stroke"),
        Some("#e11d48"),
        "{svg}"
    );
    assert_eq!(end_inner_fill.attribute("fill"), Some("#38bdf8"), "{svg}");
    assert_eq!(
        end_inner_stroke.attribute("stroke"),
        Some("#38bdf8"),
        "{svg}"
    );
    assert_ne!(end_inner_fill.attribute("fill"), Some("#22c55e"), "{svg}");
    assert_ne!(
        end_outer_stroke.attribute("stroke"),
        Some("#020617"),
        "{svg}"
    );
    assert!(
        svg.contains(r##"fill="#fef3c7""##)
            && svg.contains(r##"stroke="#92400e" stroke-width="1.3""##),
        "note rough paths should consume noteBkgColor/noteBorderColor: {svg}"
    );
}
