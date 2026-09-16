mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions, ParsedDiagramRender, RenderSemanticModel};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramEffectSet, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec,
    EffectBinding, EffectGraph, EffectInput, EffectPrimitive, FontStack, GradientStop,
    LinearGradient, OrdinalPalette, OrdinalSelector, PatternKind, PatternSpec, Specified,
    ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::{RenderEnvironment, RenderSession};
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgPipeline, SvgRenderOptions};
use merman_render::{DiagramFamilyId, LayoutOptions};
use serde_json::json;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn render_class_svg_from_text(text: &str) -> String {
    render_class_svg_from_text_with_engine(Engine::new(), text)
}

fn render_class_svg_from_text_with_engine(engine: Engine, text: &str) -> String {
    render_class_svg_from_text_with_engine_and_options(
        engine,
        text,
        &LayoutOptions::headless_svg_defaults(),
        &SvgRenderOptions::default(),
    )
}

fn render_class_svg_from_text_with_engine_and_options(
    engine: Engine,
    text: &str,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
) -> String {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    render_class_svg_from_text_with_session(engine, text, layout_options, svg_options, session)
}

fn render_class_svg_from_text_with_session(
    engine: Engine,
    text: &str,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
    session: RenderSession,
) -> String {
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let artifact = family::prepare(parsed, layout_options, session).expect("layout ok");

    artifact
        .render_svg(svg_options, &SvgDebugOptions::default())
        .expect("svg render ok")
        .svg()
        .to_owned()
}

fn class_edge_rules_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    let styles = rules
        .into_iter()
        .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule);
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile Class edge theme")
}

fn class_edge_scalar_themes(paint: CanvasPaint) -> [DiagramTheme; 4] {
    [
        (false, None),
        (false, Some(ThemeVariant::Default)),
        (true, None),
        (true, Some(ThemeVariant::Default)),
    ]
    .map(|(fill, variant)| {
        let patch = if fill {
            ThemeStylePatch::default().with_fill(paint.clone())
        } else {
            ThemeStylePatch::default().with_stroke(paint.clone())
        };
        let mut rule = ThemeRule::new(ThemeTarget::Edge, patch);
        if let Some(variant) = variant {
            rule = rule.with_variant(variant);
        }
        class_edge_rules_theme([rule])
    })
}

fn class_node_surface_theme() -> DiagramTheme {
    class_edge_rules_theme([
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#112233").expect("valid Class node fill"))
                .with_stroke(CanvasPaint::solid("#445566").expect("valid Class node stroke")),
        ),
        ThemeRule::new(
            ThemeTarget::NodeLabel,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#ddeeff").expect("valid Class node label fill")),
        ),
    ])
}

fn class_node_default_surface_theme() -> DiagramTheme {
    class_edge_rules_theme([
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#112233").expect("valid Class node fill"))
                .with_stroke(CanvasPaint::solid("#445566").expect("valid Class node stroke")),
        )
        .with_variant(ThemeVariant::Default),
        ThemeRule::new(
            ThemeTarget::NodeLabel,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#ddeeff").expect("valid Class node label fill")),
        )
        .with_variant(ThemeVariant::Default),
    ])
}

fn class_node_ordinal_surface_theme() -> DiagramTheme {
    class_edge_rules_theme([
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#112233").expect("valid static Class node fill"))
                .with_stroke(
                    CanvasPaint::solid("#445566").expect("valid static Class node stroke"),
                ),
        ),
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#aa0000").expect("valid ordinal Class node fill")),
        )
        .with_ordinal(OrdinalSelector::exact(2).expect("valid second Class node ordinal")),
        ThemeRule::new(
            ThemeTarget::NodeLabel,
            ThemeStylePatch::default().with_fill(
                CanvasPaint::solid("#ddeeff").expect("valid static Class node label fill"),
            ),
        ),
        ThemeRule::new(
            ThemeTarget::NodeLabel,
            ThemeStylePatch::default().with_fill(
                CanvasPaint::solid("#00aa00").expect("valid ordinal Class node label fill"),
            ),
        )
        .with_ordinal(OrdinalSelector::exact(2).expect("valid second Class label ordinal")),
    ])
}

fn class_node_fallback_theme() -> DiagramTheme {
    let palette = OrdinalPalette::new([
        ThemeColorValue::parse("#123456").expect("valid Class node palette color")
    ])
    .expect("non-empty Class node palette");
    let effect_id = "class-node-blur";
    let effects = DiagramEffectSet::default()
        .with_graph(
            EffectGraph::new(
                effect_id,
                [EffectPrimitive::GaussianBlur {
                    input: EffectInput::SourceGraphic,
                    std_deviation: 1.0,
                }],
            )
            .expect("valid Class node effect graph"),
        )
        .expect("unique Class node effect graph")
        .with_binding(
            EffectBinding::new(ThemeTarget::Node, effect_id)
                .expect("valid Class node effect binding"),
        )
        .expect("unique Class node effect binding");
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_styles(
                    ThemeRuleSet::default()
                        .with_ordinal_palette(ThemeTarget::Node, palette)
                        .with_ordinal_palette(
                            ThemeTarget::NodeLabel,
                            OrdinalPalette::new([ThemeColorValue::parse("#abcdef")
                                .expect("valid Class label palette color")])
                            .expect("non-empty Class label palette"),
                        ),
                )
                .with_effects(effects),
        )
        .expect("compile Class node fallback theme")
}

fn class_table_palette_theme() -> DiagramTheme {
    let palette = OrdinalPalette::new([
        ThemeColorValue::parse("#123456").expect("valid Class table palette color")
    ])
    .expect("non-empty Class table palette");
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Table, palette),
            ),
        )
        .expect("compile Class table palette theme")
}

fn class_table_effect_theme() -> DiagramTheme {
    let effect_id = "class-table-blur";
    let effects = DiagramEffectSet::default()
        .with_graph(
            EffectGraph::new(
                effect_id,
                [EffectPrimitive::GaussianBlur {
                    input: EffectInput::SourceGraphic,
                    std_deviation: 1.0,
                }],
            )
            .expect("valid Class table effect graph"),
        )
        .expect("unique Class table effect graph")
        .with_binding(
            EffectBinding::new(ThemeTarget::Table, effect_id)
                .expect("valid Class table effect binding"),
        )
        .expect("unique Class table effect binding");
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_effects(effects))
        .expect("compile Class table effect theme")
}

fn class_typography_theme(typography: ThemeTextStyle) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::CLASS, typography),
        ))
        .expect("compile Class typography theme")
}

fn try_render_class_svg_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> merman_render::Result<family::RenderedFamilySvg> {
    try_render_class_svg_with_theme_requirement(
        source,
        theme,
        engine,
        ThemePortabilityRequirement::RequirePortable,
    )
}

fn try_render_class_svg_with_theme_requirement(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    try_render_class_svg_with_theme_environment(
        source,
        theme,
        engine,
        portability,
        RenderEnvironment::deterministic(),
    )
}

fn try_render_class_svg_with_theme_environment(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
    environment: RenderEnvironment,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Class diagram")
        .expect("detect themed Class diagram");
    let session = environment
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed Class session");
    family::prepare(parsed, &LayoutOptions::default(), session)?
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
}

fn render_class_svg_with_theme(source: &str, theme: &DiagramTheme, engine: Engine) -> String {
    try_render_class_svg_with_theme_and_engine(source, theme, engine)
        .expect("render themed Class SVG")
        .svg()
        .to_owned()
}

fn layout_and_render_class_svg_with_theme(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> (
    merman_render::model::ClassDiagramLayout,
    family::RenderedFamilySvg,
) {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Class diagram")
        .expect("detect themed Class diagram");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin themed Class session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare themed Class diagram");
    let layout = serde_json::from_value(
        artifact.layout_json().expect("Class layout projection")["layout"]["ClassDiagramV2"]
            .clone(),
    )
    .expect("Class layout");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render themed Class SVG");
    (layout, rendered)
}

fn try_render_class_svg_with_resource_policy(
    source: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Class resource-bound session");
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse Class resource-bound fixture")
        .expect("detect Class resource-bound fixture");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn class_typed_font_stack_reaches_scoped_css_and_strict_receipt() {
    let font_stack =
        FontStack::new(["ClassTyped", "monospace"]).expect("valid Class typed font stack");
    let theme = class_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));
    let rendered = try_render_class_svg_with_theme_and_engine(
        "classDiagram\n  class Alpha {\n    +value: String\n  }\n",
        &theme,
        Engine::new(),
    )
    .expect("render strict portable Class font stack");

    assert!(
        rendered
            .svg()
            .contains("#merman{font-family:ClassTyped,monospace;"),
        "typed Class font stack must reach root CSS: {}",
        rendered.svg()
    );
    assert!(
        rendered.svg().contains("g.classGroup text{fill:")
            && rendered
                .svg()
                .contains("font-family:ClassTyped,monospace;font-size:10px;"),
        "typed Class font stack must reach the class-group CSS seam: {}",
        rendered.svg()
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn class_mixed_typography_composes_without_legacy_overlay() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(FontStack::single("ClassMixed").expect("valid Class mixed font stack"))
        .with_font_size_px(24.0)
        .expect("valid Class mixed font size");
    let theme = class_typography_theme(typography);
    let source = "classDiagram\n  class Alpha\n";
    let metadata = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_metadata_sync(source)
        .expect("parse Class compatibility metadata");
    assert_eq!(
        metadata.effective_config.get_str("themeVariables.fontSize"),
        Some("16px")
    );
    assert!(!merman_core::__private::fallback_overlay_owns_path(
        &metadata.effective_config,
        "themeVariables.fontSize"
    ));
    assert!(!merman_core::__private::fallback_overlay_owns_path(
        &metadata.effective_config,
        "themeVariables.fontFamily"
    ));
    assert!(!merman_core::__private::fallback_overlay_owns_path(
        &metadata.effective_config,
        "fontFamily"
    ));

    let rendered = try_render_class_svg_with_theme_requirement(
        source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("BestEffort renders mixed Class typography");
    assert!(rendered.svg().contains("ClassMixed"));
    assert!(rendered.svg().contains("font-size:24px;"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);

    try_render_class_svg_with_theme_and_engine(source, &theme, Engine::new())
        .expect("RequirePortable accepts typed Class typography");
}

#[test]
fn class_typed_font_size_outranks_root_layout_fallback() {
    let typography = ThemeTextStyle::default()
        .with_font_size_px(24.0)
        .expect("valid Class font size");
    let theme = class_typography_theme(typography);
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "fontSize": 30
    })));
    let rendered =
        try_render_class_svg_with_theme_and_engine("classDiagram\n  class Alpha\n", &theme, engine)
            .expect("typed Class font size outranks the root-only layout fallback");

    assert!(
        rendered.svg().contains("#merman{font-family:")
            && rendered.svg().contains("font-size:24px;"),
        "typed Class font size must reach the scoped stylesheet: {}",
        rendered.svg()
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn class_cardinality_geometry_keeps_mermaids_fixed_font_size() {
    fn theme_with_size(font_size_px: f32) -> DiagramTheme {
        class_typography_theme(
            ThemeTextStyle::default()
                .with_font_size_px(font_size_px)
                .expect("valid Class font size"),
        )
    }

    fn cardinality_dimensions(
        layout: &merman_render::model::ClassDiagramLayout,
    ) -> Vec<(f64, f64)> {
        layout
            .edges
            .iter()
            .flat_map(|edge| {
                [
                    edge.start_label_left.as_ref(),
                    edge.start_label_right.as_ref(),
                    edge.end_label_left.as_ref(),
                    edge.end_label_right.as_ref(),
                ]
            })
            .flatten()
            .map(|label| (label.width, label.height))
            .collect()
    }

    let source = "classDiagram\n  Alpha \"one\" --> \"many\" Beta : owns\n";
    let (small_layout, small_rendered) =
        layout_and_render_class_svg_with_theme(source, &theme_with_size(12.0), Engine::new());
    let (large_layout, large_rendered) =
        layout_and_render_class_svg_with_theme(source, &theme_with_size(48.0), Engine::new());

    let small_dimensions = cardinality_dimensions(&small_layout);
    let large_dimensions = cardinality_dimensions(&large_layout);
    assert!(
        !small_dimensions.is_empty(),
        "expected Class cardinality labels"
    );
    assert_eq!(large_dimensions, small_dimensions);
    assert!(large_rendered.svg().contains("font-size:48px;"));
    assert!(
        large_rendered
            .svg()
            .contains(".edgeTerminals{font-size:11px;line-height:initial;}")
    );
    for text in ["one", "many"] {
        let needle = format!("<p>{text}</p>");
        let small_terminal = foreign_object_for_text(small_rendered.svg(), &needle);
        let large_terminal = foreign_object_for_text(large_rendered.svg(), &needle);
        assert_eq!(
            (
                attr_f64(large_terminal, "width"),
                attr_f64(large_terminal, "height"),
            ),
            (
                attr_f64(small_terminal, "width"),
                attr_f64(small_terminal, "height"),
            ),
            "Class cardinality {text:?} must remain on the fixed 11px terminal style"
        );
    }

    let evidence =
        merman_render::__private::family_evidence(large_rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn class_explicit_theme_font_size_ownership_outranks_typed_font_size() {
    let typography = ThemeTextStyle::default()
        .with_font_size_px(24.0)
        .expect("valid Class font size");
    let theme = class_typography_theme(typography);
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "themeVariables": { "fontSize": "30px" }
    })));
    let rendered =
        try_render_class_svg_with_theme_and_engine("classDiagram\n  class Alpha\n", &theme, engine)
            .expect("explicit Class theme font size owns the route");

    assert!(
        rendered.svg().contains("font-size:30px;"),
        "config-owned Class font size must reach the scoped stylesheet: {}",
        rendered.svg()
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn class_html_node_font_size_ownership_fails_closed_until_layout_consumes_it() {
    let typography = ThemeTextStyle::default()
        .with_font_size_px(24.0)
        .expect("valid Class font size");
    let theme = class_typography_theme(typography);
    let source = r#"%%{init: {"htmlLabels": true}}%%
classDiagram
  class Alpha
  style Alpha font-size:40px
"#;

    let rendered = try_render_class_svg_with_theme_requirement(
        source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("BestEffort preserves source-owned Class node font size");
    assert!(rendered.svg().contains("font-size:40px"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);
    assert_eq!(evidence.compatibility_residual_count(), 0);

    let error = try_render_class_svg_with_theme_and_engine(source, &theme, Engine::new())
        .err()
        .expect("RequirePortable rejects split Class font-size ownership");
    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::CLASS, 1))
    );
}

#[test]
fn class_unsupported_typography_is_property_local_to_the_typed_font_stack() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(FontStack::single("ClassTyped").expect("valid Class font stack"))
        .with_font_weight(700)
        .expect("valid unsupported Class font weight");
    let theme = class_typography_theme(typography);
    let source = "classDiagram\n  class Alpha\n";
    let rendered = try_render_class_svg_with_theme_requirement(
        source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("BestEffort renders Class with unsupported font weight");

    assert!(rendered.svg().contains("ClassTyped"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
    assert_eq!(evidence.compatibility_residual_count(), 0);

    let error = match try_render_class_svg_with_theme_and_engine(source, &theme, Engine::new()) {
        Ok(_) => panic!("strict Class must reject unsupported font weight"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::CLASS, 1))
    );
}

#[test]
fn class_explicit_font_family_ownership_outranks_typed_font_stack() {
    let theme = class_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("ClassTyped").expect("valid Class typed font")),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "fontFamily": "ClassConfigOwned"
    })));
    let rendered =
        try_render_class_svg_with_theme_and_engine("classDiagram\n  class Alpha\n", &theme, engine)
            .expect("render config-owned Class font stack");

    assert!(rendered.svg().contains("font-family:ClassConfigOwned;"));
    assert!(!rendered.svg().contains("ClassTyped"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn class_font_stack_without_inherited_text_is_not_applicable() {
    let theme = class_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("ClassTerminalFree").expect("valid Class terminal-free font"),
    ));
    let rendered =
        try_render_class_svg_with_theme_and_engine("classDiagram\n", &theme, Engine::new())
            .expect("render terminal-free Class diagram");

    assert!(rendered.svg().contains("ClassTerminalFree"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn class_relation_text_keeps_font_stack_applied_when_node_text_is_source_owned() {
    let theme = class_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("ClassRelationTyped").expect("valid Class relation font"),
    ));
    let rendered = try_render_class_svg_with_theme_and_engine(
        r#"%%{init: {"htmlLabels": true}}%%
classDiagram
  class Alpha
  class Beta
  Alpha "1" --> "many" Beta : relates
  style Alpha font-family:AlphaOwned
  style Beta font-family:BetaOwned
"#,
        &theme,
        Engine::new(),
    )
    .expect("render Class relation typography receipt");

    assert!(rendered.svg().contains("font-family:ClassRelationTyped;"));
    assert!(rendered.svg().contains("font-family:AlphaOwned"));
    assert!(rendered.svg().contains("font-family:BetaOwned"));
    assert!(rendered.svg().contains("<p>relates</p>"));
    assert!(rendered.svg().contains("<p>many</p>"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn class_node_font_ownership_matches_html_and_svg_writer_semantics() {
    let theme =
        class_typography_theme(ThemeTextStyle::default().with_font_stack(
            FontStack::single("ClassWriterTyped").expect("valid Class writer font"),
        ));
    for html_labels in [true, false] {
        let source = format!(
            r#"%%{{init: {{"htmlLabels": {html_labels}}}}}%%
classDiagram
  class Alpha
  style Alpha font-family:ClassSourceOwned
"#
        );
        let rendered = try_render_class_svg_with_theme_and_engine(&source, &theme, Engine::new())
            .expect("render Class node font ownership");

        assert!(rendered.svg().contains("font-family:ClassWriterTyped;"));
        if html_labels {
            assert!(
                rendered.svg().contains(
                    r#"class="nodeLabel markdown-node-label" style="font-family:ClassSourceOwned">"#
                ),
                "node-level font-family must reach the HTML label writer: {}",
                rendered.svg()
            );
        } else {
            for text_start in rendered
                .svg()
                .match_indices("<text")
                .map(|(index, _)| index)
            {
                let text_opening = &rendered.svg()[text_start..];
                let text_opening = &text_opening[..text_opening
                    .find('>')
                    .expect("complete Class SVG text opening tag")];
                assert!(
                    !text_opening.contains("font-family:ClassSourceOwned"),
                    "node-level font-family must not reach the SVG text writer: {text_opening}"
                );
            }
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "htmlLabels={html_labels}");
        assert_eq!(evidence.applied_count(), 1, "htmlLabels={html_labels}");
        assert_eq!(
            evidence.not_applicable_count(),
            0,
            "htmlLabels={html_labels}"
        );
        assert_eq!(
            evidence.theme_residual_count(),
            0,
            "htmlLabels={html_labels}"
        );
    }
}

#[test]
fn class_edge_label_layout_and_writer_share_html_labels_precedence() {
    for (config, expected_html) in [
        (
            r#"{"htmlLabels": false, "flowchart": {"htmlLabels": true}}"#,
            false,
        ),
        (
            r#"{"htmlLabels": true, "flowchart": {"htmlLabels": false}}"#,
            true,
        ),
        (r#"{"flowchart": {"htmlLabels": false}}"#, false),
    ] {
        let svg = render_class_svg_from_text(&format!(
            "%%{{init: {config}}}%%\nclassDiagram\n  Alpha --> Beta : relates\n"
        ));
        let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
        let label = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class") == Some("edgeLabel")
                    && node
                        .descendants()
                        .filter_map(|descendant| descendant.text())
                        .any(|text| text.contains("relates"))
            })
            .expect("visible Class relation label");

        assert_eq!(
            label
                .descendants()
                .any(|descendant| descendant.has_tag_name("foreignObject")),
            expected_html,
            "config={config}: {svg}"
        );
    }
}

#[test]
fn class_unverified_font_ownership_is_a_portability_residual_not_an_invalid_model() {
    let theme = class_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("ClassFallbackTyped").expect("valid Class fallback font"),
    ));
    let source = r#"%%{init: {"htmlLabels": true}}%%
classDiagram
  class Alpha
  style Alpha font-family:var(--class-font)
"#;

    let rendered = try_render_class_svg_with_theme_requirement(
        source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("BestEffort preserves browser-resolved Class font ownership");
    assert!(rendered.svg().contains("ClassFallbackTyped"));
    assert!(rendered.svg().contains("font-family:var(--class-font)"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);

    let error = try_render_class_svg_with_theme_and_engine(source, &theme, Engine::new())
        .err()
        .expect("RequirePortable rejects browser-resolved Class font ownership");
    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::CLASS, 1))
    );
}

#[test]
fn class_html_descendant_fonts_fail_closed_until_layout_measures_them() {
    let theme = class_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("ClassMeasuredTyped").expect("valid Class measured font"),
    ));
    for (case, label) in [
        (
            "static",
            "<span style='font-family:DescendantOwned'>Visible</span>",
        ),
        (
            "quoted-semicolon",
            "<span style='font-family:&quot;a;b&quot;,sans-serif'>Visible</span>",
        ),
        (
            "font-shorthand",
            "<span style='font:16px DescendantOwned'>Visible</span>",
        ),
        ("all-reset", "<span style='all:initial'>Visible</span>"),
        ("font-face", "<font face='DescendantOwned'>Visible</font>"),
    ] {
        let source = format!(
            "%%{{init: {{\"htmlLabels\": true}}}}%%\nclassDiagram\n  class Alpha[\"{label}\"]\n"
        );
        let rendered = try_render_class_svg_with_theme_requirement(
            &source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap_or_else(|error| panic!("BestEffort preserves {case} descendant font: {error}"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "case={case}");
        assert_eq!(evidence.applied_count(), 0, "case={case}");
        assert_eq!(evidence.theme_residual_count(), 1, "case={case}");

        let error = try_render_class_svg_with_theme_and_engine(&source, &theme, Engine::new())
            .err()
            .unwrap_or_else(|| panic!("RequirePortable must reject {case} descendant font"));
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::CLASS, 1)),
            "case={case}"
        );
    }
}

#[test]
fn class_html_descendant_inherit_keeps_typed_font_portable() {
    let theme = class_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("ClassInheritedTyped").expect("valid Class inherited font"),
    ));
    for label in [
        "<span style='font-family:inherit'>Visible</span>",
        "<font face='DescendantOwned' style='font-family:inherit'>Visible</font>",
        "<span class='host' style='font-family:inherit !important'>Visible</span>",
    ] {
        let source = format!(
            r#"%%{{init: {{"htmlLabels": true}}}}%%
classDiagram
  class Alpha["{label}"]
"#
        );
        let rendered = try_render_class_svg_with_theme_and_engine(&source, &theme, Engine::new())
            .unwrap_or_else(|error| panic!("render portable inherited descendant font: {error}"));

        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "label={label}");
        assert_eq!(evidence.applied_count(), 1, "label={label}");
        assert_eq!(evidence.theme_residual_count(), 0, "label={label}");
    }
}

#[test]
fn class_svg_inline_html_font_style_remains_literal_inherited_text() {
    let theme = class_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("ClassSvgLiteralTyped").expect("valid Class SVG literal font"),
    ));
    let rendered = try_render_class_svg_with_theme_and_engine(
        r#"%%{init: {"htmlLabels": false}}%%
classDiagram
  class Literal["<span style='font-family:SourceOwned'>Visible</span>"]
"#,
        &theme,
        Engine::new(),
    )
    .expect("render Class SVG literal inline HTML font");

    assert!(rendered.svg().contains("ClassSvgLiteralTyped"));
    assert!(rendered.svg().contains("&lt;span"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn class_svg_relation_inline_html_font_style_cannot_steal_writer_ownership() {
    let theme = class_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("ClassSvgEdgeTyped").expect("valid Class SVG edge font"),
    ));
    let rendered = try_render_class_svg_with_theme_and_engine(
        r#"%%{init: {"flowchart": {"htmlLabels": false}, "securityLevel": "loose"}}%%
classDiagram
  class Alpha
  class Beta
  Alpha "1" --> "many" Beta : <span style='font-family:EdgeOwned'>relates</span>
  style Alpha font-family:AlphaOwned
  style Beta font-family:BetaOwned
"#,
        &theme,
        Engine::new(),
    )
    .expect("render Class SVG relation literal inline HTML font");

    assert!(rendered.svg().contains("ClassSvgEdgeTyped"));
    assert!(rendered.svg().contains("&lt;span"));
    assert!(rendered.svg().contains("font-family:AlphaOwned"));
    assert!(rendered.svg().contains("font-family:BetaOwned"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn class_svg_note_inline_html_font_style_remains_literal_inherited_text() {
    let theme = class_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("ClassSvgNoteTyped").expect("valid Class SVG note font"),
    ));
    let rendered = try_render_class_svg_with_theme_and_engine(
        r#"%%{init: {"htmlLabels": false}}%%
classDiagram
  note "<span style='font-family:NoteOwned'>Visible</span>"
"#,
        &theme,
        Engine::new(),
    )
    .expect("render Class SVG note literal inline HTML font");

    assert!(rendered.svg().contains("ClassSvgNoteTyped"));
    assert!(rendered.svg().contains("&lt;span"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn class_attached_note_edge_is_not_a_relation_typography_terminal() {
    let theme = class_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("ClassAttachedNoteTyped").expect("valid Class attached-note font"),
    ));
    let rendered = try_render_class_svg_with_theme_and_engine(
        "classDiagram\n  class Alpha\n  note for Alpha \"attached note\"\n",
        &theme,
        Engine::new(),
    )
    .expect("render strict portable Class attached-note typography");

    assert!(rendered.svg().contains("ClassAttachedNoteTyped"));
    assert!(rendered.svg().contains("attached note"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_typed_font_stack_reaches_layout_and_terminal_receipt() {
    let theme = class_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("ClassElkTyped").expect("valid Class ELK font")),
    );
    let rendered = try_render_class_svg_with_theme_and_engine(
        r#"---
config:
  layout: elk
---
classDiagram
  class Alpha
  class Beta
  Alpha "1" --> "many" Beta : relates
"#,
        &theme,
        Engine::new(),
    )
    .expect("render strict portable Class ELK font stack");

    assert!(rendered.svg().contains("font-family:ClassElkTyped;"));
    assert!(rendered.svg().contains("<p>relates</p>"));
    assert!(rendered.svg().contains("<p>many</p>"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_svg_relation_inventory_survives_source_owned_html_nodes() {
    let theme = class_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("ClassElkEdgeTyped").expect("valid Class ELK edge font"),
    ));
    let rendered = try_render_class_svg_with_theme_and_engine(
        r#"---
config:
  layout: elk
  flowchart:
    htmlLabels: false
---
classDiagram
  class Alpha
  class Beta
  Alpha "1" --> "many" Beta : <span style='font-family:EdgeOwned'>relates</span>
  style Alpha font-family:AlphaOwned
  style Beta font-family:BetaOwned
"#,
        &theme,
        Engine::new(),
    )
    .expect("render strict portable Class ELK relation inventory");

    assert!(rendered.svg().contains("ClassElkEdgeTyped"));
    assert!(rendered.svg().contains("&lt;span"));
    assert!(rendered.svg().contains("<p>many</p>"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_attached_note_edge_is_not_a_relation_typography_terminal() {
    let theme = class_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("ClassElkAttachedNoteTyped").expect("valid Class ELK attached-note font"),
    ));
    let rendered = try_render_class_svg_with_theme_and_engine(
        r#"---
config:
  layout: elk
---
classDiagram
  class Alpha
  note for Alpha "attached ELK note"
"#,
        &theme,
        Engine::new(),
    )
    .expect("render strict portable Class ELK attached-note typography");

    assert!(rendered.svg().contains("ClassElkAttachedNoteTyped"));
    assert!(rendered.svg().contains("attached ELK note"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn class_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"classDiagram
class Alpha
class Beta
Alpha --> Beta : advance
note for Alpha "bounded family-level note"
"#;
    let baseline = try_render_class_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Class baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Class fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Class SVG byte ceiling");
    let exact = try_render_class_svg_with_resource_policy(source, exact_policy)
        .expect("the exact Class family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Class SVG byte ceiling");
    let error = try_render_class_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the Class family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Class MaxSvgBytes rejection, got {error}");
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

fn render_class_fixture(
    name: &str,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
) -> String {
    let path = workspace_root().join("fixtures").join("class").join(name);
    let text = std::fs::read_to_string(&path).expect("fixture");
    render_class_svg_from_text_with_engine_and_options(
        Engine::new(),
        &text,
        layout_options,
        svg_options,
    )
}

fn attr_f64(tag: &str, name: &str) -> f64 {
    let prefix = format!(r#"{name}=""#);
    let start = tag.find(&prefix).expect("attribute") + prefix.len();
    let end = start + tag[start..].find('"').expect("attribute end");
    tag[start..end].parse().expect("numeric attribute")
}

fn foreign_object_for_text<'a>(svg: &'a str, text: &str) -> &'a str {
    let text_start = svg.find(text).expect("text");
    let start = svg[..text_start]
        .rfind("<foreignObject ")
        .expect("foreignObject before text");
    let end = text_start
        + svg[text_start..]
            .find("</foreignObject>")
            .expect("foreignObject after text")
        + "</foreignObject>".len();
    &svg[start..end]
}

fn embedded_stylesheet(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid SVG");
    document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "style")
        .and_then(|node| node.text())
        .expect("embedded stylesheet")
        .to_owned()
}

fn class_model(parsed: &ParsedDiagramRender) -> &merman_core::models::class_diagram::ClassDiagram {
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected Class render model");
    };
    model
}

fn layout_class_with_dagre(
    parsed: &merman_core::ParsedDiagramRender,
    session: RenderSession,
) -> merman_render::model::ClassDiagramLayout {
    let artifact =
        family::prepare(parsed.clone(), &LayoutOptions::default(), session).expect("Class layout");
    serde_json::from_value(
        artifact.layout_json().expect("Class layout projection")["layout"]["ClassDiagramV2"]
            .clone(),
    )
    .expect("Class layout")
}

fn deep_class_namespace_text(depth: usize) -> String {
    let mut lines = vec!["classDiagram".to_string()];
    for i in 0..depth {
        lines.push(format!("{}namespace N{i} {{", "  ".repeat(i)));
    }
    lines.push(format!("{}class Leaf", "  ".repeat(depth)));
    for i in (0..depth).rev() {
        lines.push(format!("{}}}", "  ".repeat(i)));
    }
    lines.join("\n")
}

#[test]
fn class_svg_root_role_comes_from_the_detected_mermaid_diagram_id() {
    for (source, expected_role) in [
        ("classDiagram\nclass Animal\n", "classDiagram"),
        ("classDiagram-v2\nclass Animal\n", "classDiagram"),
        (
            "%%{init: {\"class\": {\"defaultRenderer\": \"dagre-wrapper\"}}}%%\nclassDiagram\nclass Animal\n",
            "classDiagram",
        ),
        (
            "%%{init: {\"class\": {\"defaultRenderer\": \"dagre-d3\"}}}%%\nclassDiagram\nclass Animal\n",
            "class",
        ),
        (
            "%%{init: {\"class\": {\"defaultRenderer\": \"dagre-d3\"}}}%%\nclassDiagram\nclass Animal\nnote for Animal \"classDiagram-v2 is note text\"\n",
            "class",
        ),
    ] {
        let engine = Engine::new();
        let parsed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::default())
            .expect("parse ok")
            .expect("diagram detected");
        assert_eq!(
            parsed.metadata().diagram_type,
            expected_role,
            "Class detection must follow Mermaid's renderer-aware detector contract for {source:?}"
        );
        assert_eq!(
            class_model(&parsed).diagram_type,
            parsed.metadata().diagram_type,
            "the typed Class model must preserve the detector-selected diagram id for {source:?}"
        );
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
            .expect("layout ok");
        let svg = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("svg render ok")
            .svg()
            .to_owned();
        let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");

        assert_eq!(
            document.root_element().attribute("aria-roledescription"),
            Some(expected_role),
            "Class accessibility role must preserve Mermaid's selected detector id for {source:?}"
        );
    }
}

#[test]
fn class_svg_unified_titles_inherit_root_font_size_for_both_detector_aliases() {
    for diagram_keyword in ["classDiagram", "classDiagram-v2"] {
        let source = format!(
            r##"---
title: Inherited title
---
%%{{init: {{"themeVariables": {{"fontSize": "23px"}}}} }}%%
{diagram_keyword}
class Animal
"##
        );
        let svg = render_class_svg_from_text_with_engine(
            legacy_init_theme_compat_engine(),
            source.as_str(),
        );
        let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
        let root = document.root_element();
        let css = document
            .descendants()
            .find(|node| node.is_element() && node.tag_name().name() == "style")
            .and_then(|node| node.text())
            .expect("embedded Class stylesheet");
        let title = root
            .children()
            .find(|node| {
                node.is_element()
                    && node.tag_name().name() == "text"
                    && node.attribute("class") == Some("classDiagramTitleText")
            })
            .expect("unified Class diagram title");

        let root_rule_start = css.find("#merman{").expect("root font rule");
        let root_rule_end = root_rule_start
            + css[root_rule_start..]
                .find('}')
                .expect("root font rule end");
        let root_rule = &css[root_rule_start..=root_rule_end];

        assert!(
            root_rule.contains("font-size:23px;"),
            "{diagram_keyword} title should inherit the configured root font size: {root_rule}"
        );
        assert!(
            css.contains(".classTitleText{text-anchor:middle;font-size:18px;"),
            "Class CSS should preserve Mermaid's legacy title selector"
        );
        assert!(
            !css.contains(".classDiagramTitleText"),
            "the unified title class must not be captured by the legacy 18px selector"
        );
        assert_eq!(title.text(), Some("Inherited title"));
        assert_eq!(title.attribute("font-size"), None);
        assert_eq!(title.attribute("style"), None);
    }
}

#[test]
fn class_stylesheet_matches_signed_mermaid_11_16_css_contract() {
    const FIXTURE: &str = "stress_class_many_relations_labels_020";
    let local_svg = render_class_fixture(
        &format!("{FIXTURE}.mmd"),
        &LayoutOptions::headless_svg_defaults(),
        &SvgRenderOptions::default(),
    );
    let upstream_svg = std::fs::read_to_string(
        workspace_root()
            .join("fixtures")
            .join("upstream-svgs")
            .join("class")
            .join(format!("{FIXTURE}.svg")),
    )
    .expect("signed Mermaid Class SVG");

    let local_css = embedded_stylesheet(&local_svg).replace("#merman", "#class-contract");
    let upstream_css =
        embedded_stylesheet(&upstream_svg).replace(&format!("#{FIXTURE}"), "#class-contract");

    assert_eq!(
        local_css, upstream_css,
        "Class CSS must preserve Mermaid's common prefix, family rules, icon rules, common Neo rules, and final :root order"
    );
    assert!(
        local_css.starts_with(
            r#"#class-contract{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:16px;fill:#333;}"#
        ),
        "the public root fill must use textColor rather than classText"
    );
}

#[test]
fn class_parse_for_render_model_handles_deep_namespace_chain() {
    const DEPTH: usize = 128;
    let source = deep_class_namespace_text(DEPTH);
    let handle = std::thread::Builder::new()
        .name("class-deep-namespace-parse".to_string())
        .stack_size(128 * 1024)
        .spawn(move || {
            let engine = Engine::new();
            engine
                .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
                .expect("parse ok")
                .expect("diagram detected");
        })
        .expect("spawn deep namespace parse test");
    handle
        .join()
        .expect("deep namespace parse should finish without stack overflow");
}

#[test]
fn class_layout_handles_deep_namespace_chain() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    const DEPTH: usize = 128;
    let source = deep_class_namespace_text(DEPTH);
    let handle = std::thread::Builder::new()
        .name("class-deep-namespace-layout".to_string())
        // This regression verifies that depth-128 namespace layout terminates without recursive
        // stack growth. The full debug Dagre pipeline has substantial fixed phase frames, so the
        // test does not claim that production rendering supports a 128 KiB thread stack.
        .stack_size(256 * 1024)
        .spawn(move || {
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
                .expect("parse ok")
                .expect("diagram detected");
            let layout = layout_class_with_dagre(&parsed, session);
            assert!(
                layout.nodes.iter().any(|node| node.id == "Leaf"),
                "expected deeply nested class member to remain in the layout"
            );
        })
        .expect("spawn deep namespace layout test");
    handle
        .join()
        .expect("deep namespace layout should finish without stack overflow");
}

#[test]
fn class_svg_dotted_namespace_titles_use_hierarchical_segment_labels() {
    let svg = render_class_svg_from_text(
        r#"classDiagram
namespace Company.Project.Module {
  class User
}
"#,
    );

    assert!(svg.contains(r#"id="merman-Company" data-look="classic""#));
    assert!(svg.contains(r#"id="merman-Company.Project" data-look="classic""#));
    assert!(svg.contains(r#"id="merman-Company.Project.Module" data-look="classic""#));
    assert!(
        svg.contains("<p>Company</p>")
            && svg.contains("<p>Project</p>")
            && svg.contains("<p>Module</p>"),
        "expected default hierarchical namespace labels to use path segments"
    );
    assert!(
        !svg.contains("<p>Company.Project.Module</p>"),
        "default hierarchical mode should not render the full dotted id as the leaf label"
    );
}

#[test]
fn class_svg_scopes_text_color_for_html_labels() {
    let svg = render_class_svg_from_text(
        r#"classDiagram
    class Animal {
        +String name
        +int age
        +makeSound()
    }
"#,
    );

    assert!(
        svg.contains(r#"#merman p{margin:0;}"#),
        "expected class SVG to reset HTML label paragraph margins"
    );
    assert!(
        svg.contains(r#"#merman .nodeLabel,#merman .edgeLabel{color:#131300;}"#),
        "expected class SVG to make HTML labels self-contained instead of inheriting host page color"
    );
    assert!(
        svg.contains(r#"#merman .label text{fill:#131300;}"#),
        "expected class SVG text labels to get an explicit fill color"
    );
}

#[test]
fn class_svg_honors_configured_class_text_color() {
    let svg = render_class_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"themeVariables": {"classText": "#123456"}}}%%
classDiagram
    class Animal
"##,
    );

    assert!(
        svg.contains(r#"#merman .nodeLabel,#merman .edgeLabel{color:#123456;}"#),
        "expected classText theme variable to drive HTML label color"
    );
    assert!(
        svg.contains(r#"#merman .label text{fill:#123456;}"#),
        "expected classText theme variable to drive SVG text fill"
    );
    assert!(
        svg.contains(r#"#merman .classTitleText{text-anchor:middle;font-size:18px;fill:#333;}"#),
        "expected Mermaid's distinct textColor token to remain the legacy Class title owner"
    );
}

#[test]
fn class_svg_keeps_class_text_and_generic_text_color_owners_distinct() {
    let svg = render_class_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"themeVariables": {"classText": "#123456", "textColor": "#654321"}}}%%
classDiagram
    class Animal
"##,
    );

    assert!(
        svg.contains(r#"#merman .nodeLabel,#merman .edgeLabel{color:#123456;}"#),
        "Class node and edge labels must retain classText ownership"
    );
    assert!(
        svg.contains(r#"#merman .classTitleText{text-anchor:middle;font-size:18px;fill:#654321;}"#),
        "the Class title must retain generic textColor ownership"
    );
}

#[test]
fn class_svg_uses_configured_look_in_dom_attributes() {
    let svg = render_class_svg_from_text(
        r#"%%{init: {"look": "neo"}}%%
classDiagram
namespace Zoo {
  class Animal
  class Keeper
}
Animal --> Keeper
"#,
    );

    assert!(
        svg.contains(r#"data-look="neo""#),
        "expected class SVG to propagate configured look: {svg}"
    );
    assert!(
        !svg.contains(r#"data-look="classic""#),
        "configured class look must not leave classic DOM attributes: {svg}"
    );
}

#[test]
fn class_svg_hand_drawn_basic_node_uses_rough_wrapper_and_hachure_paths() {
    let svg = render_class_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"look": "handDrawn", "handDrawnSeed": 7, "themeVariables": {"mainBkg": "#f8fafc", "nodeBorder": "#ef4444", "useGradient": true, "gradientStart": "#112233", "gradientStop": "#445566"}}}%%
classDiagram
  class Class10
"##,
    );

    assert!(
        svg.contains(
            r#"<g class="rough-node default" id="merman-classId-Class10-0" data-look="handDrawn""#
        ),
        "hand-drawn class node should use Mermaid's rough-node wrapper class: {svg}"
    );
    assert!(
        !svg.contains(r#"<g class="node default" id="merman-classId-Class10-0""#),
        "hand-drawn class node should not keep the classic node wrapper class: {svg}"
    );
    assert!(
        svg.contains(r#"<g class="basic label-container outer-path"><path d=""#)
            && svg.contains(
                r##"stroke="#f8fafc" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d=""##
            )
            && svg.contains(
                r##"stroke="#ef4444" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/>"##
            ),
        "hand-drawn class node should render RoughJS hachure fill and outline paths: {svg}"
    );
    let marker_ids = [
        "aggregationStart",
        "aggregationEnd",
        "aggregationStart-margin",
        "aggregationEnd-margin",
        "extensionStart",
        "extensionEnd",
        "extensionStart-margin",
        "extensionEnd-margin",
        "compositionStart",
        "compositionEnd",
        "compositionStart-margin",
        "compositionEnd-margin",
        "dependencyStart",
        "dependencyEnd",
        "dependencyStart-margin",
        "dependencyEnd-margin",
        "lollipopStart",
        "lollipopEnd",
        "lollipopStart-margin",
        "lollipopEnd-margin",
    ];
    let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
    let diagram_role = document
        .root_element()
        .attribute("aria-roledescription")
        .expect("Class diagram role");
    assert_eq!(diagram_role, "classDiagram");
    for marker_id in marker_ids.iter().filter(|id| !id.ends_with("-margin")) {
        let marker = document
            .descendants()
            .find(|node| {
                node.has_tag_name("marker")
                    && node.attribute("id")
                        == Some(format!("merman_{diagram_role}-{marker_id}").as_str())
            })
            .unwrap_or_else(|| panic!("missing marker element {marker_id}: {svg}"));
        assert_eq!(
            marker.attribute("markerUnits"),
            Some("userSpaceOnUse"),
            "plain Class marker {marker_id} must not scale with relation stroke width"
        );
    }
    let marker_positions = marker_ids.map(|marker_id| {
        let marker_attr = format!(r#"id="merman_{diagram_role}-{marker_id}""#);
        svg.find(&marker_attr)
            .unwrap_or_else(|| panic!("missing Mermaid 11.16 class marker {marker_id}: {svg}"))
    });
    assert!(
        marker_positions.windows(2).all(|pair| pair[0] < pair[1]),
        "hand-drawn class marker variants should preserve Mermaid's insertion order: {svg}"
    );
    let graph_end = svg
        .find(r#"</g><defs><filter id="merman-drop-shadow""#)
        .expect("hand-drawn class SVG should append shared resources after the graph wrapper");
    let small_shadow = svg
        .find(r#"<defs><filter id="merman-drop-shadow-small""#)
        .expect("hand-drawn class SVG should include the shared small shadow filter");
    let gradient = svg
        .find(r#"<linearGradient id="merman-gradient""#)
        .expect("hand-drawn class SVG should include a configured root gradient");
    assert!(
        graph_end < small_shadow && small_shadow < gradient,
        "shared shadow filters should preserve Mermaid root resource order: {svg}"
    );
}

#[test]
fn class_svg_hand_drawn_inline_styles_reach_rough_paths_and_labels() {
    let svg = render_class_svg_from_text(
        r##"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
classDiagram
  class Class10
  style Class10 fill:#f9f,stroke:#333,stroke-width:4px,color:white
"##,
    );

    assert!(
        svg.contains(r#"class="rough-node default""#)
            && svg.contains(r##"stroke="#f9f" stroke-width="4" fill="none""##)
            && svg.contains(r##"stroke="#333" stroke-width="4" fill="none" stroke-dasharray="0 0" style="fill:#f9f;stroke:#333;stroke-width:4px;color:white""##)
            && svg.contains(r##"style="fill:#f9f;stroke:#333;stroke-width:4px;color:white"><p>Class10</p>"##),
        "inline style should reach hand-drawn class rough paths and label span: {svg}"
    );
}

#[test]
fn class_svg_hand_drawn_notes_use_rough_wrapper_and_note_hachure_paths() {
    let svg = render_class_svg_from_text(
        r##"%%{init: {"look": "handDrawn", "handDrawnSeed": 7, "themeVariables": {"noteBkgColor": "#fff5ad", "noteBorderColor": "#aaaa33"}}}%%
classDiagram
  note "hello"
"##,
    );

    assert!(
        svg.contains(r#"<g class="rough-node undefined" id="merman-note0" data-look="handDrawn""#),
        "hand-drawn class note should use Mermaid's rough-node wrapper class: {svg}"
    );
    assert!(
        svg.contains(r#"<g class="basic label-container outer-path"><path d=""#)
            && svg.contains(
                r##"stroke="#fff5ad" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d=""##
            )
            && svg.contains(
                r##"stroke="#aaaa33" stroke-width="1.3" fill="none" stroke-dasharray="0 0"/>"##
            )
            && svg.contains(r#"<g class="label noteLabel""#)
            && svg.contains(r#"class="nodeLabel markdown-node-label""#),
        "hand-drawn class note should render note-colored hachure fill and outline paths: {svg}"
    );
}

#[test]
fn class_svg_hand_drawn_edges_use_rough_transition_class() {
    let svg = render_class_svg_from_text(
        r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
classDiagram
  class A
  class B
  A --> B
  note for A "hello"
"#,
    );

    assert!(
        svg.contains(r#"class="edge-thickness-normal edge-pattern-solid transition relation""#)
            && svg.contains(r##"stroke="#000" stroke-width="1" fill="none""##)
            && svg.contains(r#"id="merman-id_A_B_1""#)
            && svg.contains(r#"data-id="id_A_B_1""#)
            && svg.contains(r#"id="merman-edgeNote0""#)
            && svg.contains(r#"data-id="edgeNote0""#)
            && svg.contains(r#"data-look="handDrawn""#),
        "hand-drawn class relations should use RoughJS transition edge DOM: {svg}"
    );
}

#[test]
fn class_direct_edge_scalar_reaches_relation_paths_and_only_their_referenced_markers() {
    let source = r#"classDiagram
  A *-- B
  C --* D
  E <|-- F
  G --|> H
  I <.. J
  K ..> L
  M o-- N
  O --o P
  Q ()-- R
"#;
    for theme in class_edge_scalar_themes(
        CanvasPaint::solid("#123456").expect("valid Class relation stroke"),
    ) {
        for look in ["classic", "handDrawn"] {
            let rendered = try_render_class_svg_with_theme_and_engine(
                source,
                &theme,
                Engine::new().with_site_config(MermaidConfig::from_value(json!({ "look": look }))),
            )
            .expect("render directly themed Class relations");
            let svg = rendered.svg();
            let document = roxmltree::Document::parse(svg).expect("valid themed Class SVG");
            let relation_paths = document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("path")
                        && node.attribute("data-edge") == Some("true")
                        && node
                            .attribute("data-id")
                            .is_some_and(|id| id.starts_with("id_"))
                })
                .collect::<Vec<_>>();
            assert_eq!(relation_paths.len(), 9, "look={look}: {svg}");

            let mut referenced_marker_ids = Vec::new();
            for path in relation_paths {
                assert!(
                    path.attribute("style")
                        .is_some_and(|style| style.contains("stroke:#123456 !important")),
                    "look={look}: typed stroke must reach the final relation path: {svg}"
                );
                if look == "handDrawn" {
                    assert_eq!(path.attribute("stroke"), Some("#123456"), "{svg}");
                }
                for marker_attribute in ["marker-start", "marker-end"] {
                    let Some(reference) = path.attribute(marker_attribute) else {
                        continue;
                    };
                    let marker_id = reference
                        .strip_prefix("url(#")
                        .and_then(|value| value.strip_suffix(')'))
                        .expect("Class marker URL");
                    referenced_marker_ids.push(marker_id.to_string());
                }
            }
            referenced_marker_ids.sort();
            referenced_marker_ids.dedup();
            assert!(!referenced_marker_ids.is_empty(), "look={look}: {svg}");
            for marker_name in [
                "aggregationStart",
                "aggregationEnd",
                "extensionStart",
                "extensionEnd",
                "compositionStart",
                "compositionEnd",
                "dependencyStart",
                "dependencyEnd",
            ] {
                let marker_suffix = format!("-{marker_name}");
                assert!(
                    referenced_marker_ids
                        .iter()
                        .any(|marker_id| marker_id.ends_with(&marker_suffix)),
                    "look={look}: relation fixture must reference {marker_name}: {svg}"
                );
            }

            for marker_id in &referenced_marker_ids {
                let marker = document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("marker") && node.attribute("id") == Some(marker_id)
                    })
                    .unwrap_or_else(|| panic!("missing referenced marker {marker_id}: {svg}"));
                let terminal = marker
                    .children()
                    .find(roxmltree::Node::is_element)
                    .expect("Class marker terminal shape");
                assert!(
                    terminal
                        .attribute("style")
                        .is_some_and(|style| style.contains("stroke:#123456 !important")),
                    "look={look}: referenced marker {marker_id} must share the typed stroke: {svg}"
                );
                if marker_id.contains("aggregation") || marker_id.contains("extension") {
                    assert!(
                        terminal.has_tag_name("path")
                            && terminal
                                .attribute("style")
                                .is_some_and(|style| style.contains("fill:transparent !important")),
                        "hollow marker child path {marker_id} must remain transparent: {svg}"
                    );
                } else if marker_id.contains("composition") || marker_id.contains("dependency") {
                    assert!(
                        terminal.has_tag_name("path")
                            && terminal
                                .attribute("style")
                                .is_some_and(|style| style.contains("fill:#123456 !important")),
                        "filled marker {marker_id} must share the typed line color: {svg}"
                    );
                }
            }

            let unused_marker = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("marker")
                        && node
                            .attribute("id")
                            .is_some_and(|id| !id.ends_with("-margin"))
                        && node
                            .attribute("id")
                            .is_some_and(|id| !referenced_marker_ids.iter().any(|used| used == id))
                })
                .expect("unused Class relation marker");
            assert!(
                unused_marker
                    .children()
                    .find(roxmltree::Node::is_element)
                    .and_then(|node| node.attribute("style"))
                    .is_none_or(|style| !style.contains("#123456")),
                "an unreferenced marker must not become typed evidence: {svg}"
            );

            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 1, "look={look}");
            assert_eq!(evidence.theme_residual_count(), 0, "look={look}");
        }
    }
}

#[test]
fn class_direct_node_surfaces_reach_html_and_native_terminals() {
    let source = r#"classDiagram
  class Account {
    +String owner
    +close()
  }
"#;
    let theme = class_node_surface_theme();

    for html_labels in [true, false] {
        let rendered = try_render_class_svg_with_theme_and_engine(
            source,
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "htmlLabels": html_labels,
            }))),
        )
        .expect("render directly themed Class node surfaces");
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid themed Class node SVG");
        let node = document
            .descendants()
            .find(|node| {
                node.is_element()
                    && node.tag_name().name() == "g"
                    && node
                        .attribute("id")
                        .is_some_and(|id| id.starts_with("merman-classId-Account-"))
            })
            .expect("Class Account terminal node");
        let terminal_paths = node
            .descendants()
            .filter(|terminal| terminal.has_tag_name("path"))
            .collect::<Vec<_>>();
        assert!(
            terminal_paths
                .iter()
                .any(|terminal| terminal.attribute("fill") == Some("#112233")),
            "typed Node.fill must reach the terminal Class shell: {}",
            rendered.svg(),
        );
        assert!(
            terminal_paths
                .iter()
                .any(|terminal| terminal.attribute("stroke") == Some("#445566")),
            "typed Node.stroke must reach the terminal Class shell: {}",
            rendered.svg(),
        );

        let themed_label = if html_labels {
            node.descendants().find(|terminal| {
                terminal.has_tag_name("span")
                    && terminal.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "nodeLabel")
                    })
                    && terminal.attribute("style").is_some_and(|style| {
                        style.contains("color:#ddeeff") && style.contains("fill:#ddeeff")
                    })
            })
        } else {
            node.descendants().find(|terminal| {
                terminal.has_tag_name("text")
                    && terminal.attribute("style").is_some_and(|style| {
                        style.contains("color:#ddeeff") && style.contains("fill:#ddeeff")
                    })
            })
        };
        assert!(
            themed_label.is_some(),
            "typed NodeLabel.fill must reach the {html_labels:?} terminal label shell: {}",
            rendered.svg(),
        );

        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 2, "html_labels={html_labels}");
        assert_eq!(
            evidence.theme_residual_count(),
            0,
            "html_labels={html_labels}"
        );
    }

    let resvg = try_render_class_svg_with_theme_and_engine(
        source,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "htmlLabels": true,
        }))),
    )
    .expect("render directly themed HTML Class labels for resvg")
    .finalize_resvg(&SvgPipeline::resvg_safe())
    .expect("finalize directly themed HTML Class labels for resvg");
    let native_svg = merman_render::__private::native_export_svg(resvg.svg());
    let native_document =
        roxmltree::Document::parse(native_svg).expect("valid native Class label SVG");
    let account_label = native_document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node
                    .descendants()
                    .filter_map(|descendant| descendant.text())
                    .any(|text| text.contains("Account"))
        })
        .unwrap_or_else(|| panic!("native Class Account label: {native_svg}"));
    assert_eq!(account_label.attribute("fill"), Some("#ddeeff"));
}

#[test]
fn class_explicit_default_node_surfaces_are_directly_verified() {
    let rendered = try_render_class_svg_with_theme_and_engine(
        "classDiagram\n  class Account\n",
        &class_node_default_surface_theme(),
        Engine::new(),
    )
    .expect("explicit Default Class node surfaces should use the typed writer");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Class SVG");
    let account = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node
                    .attribute("id")
                    .is_some_and(|id| id.starts_with("merman-classId-Account-"))
        })
        .expect("Class Account terminal node");
    assert!(
        account
            .descendants()
            .any(|node| node.has_tag_name("path") && node.attribute("fill") == Some("#112233")),
        "typed Default Node.fill must reach the Class shell: {}",
        rendered.svg(),
    );
    assert!(
        account
            .descendants()
            .any(|node| node.has_tag_name("path") && node.attribute("stroke") == Some("#445566")),
        "typed Default Node.stroke must reach the Class shell: {}",
        rendered.svg(),
    );
    assert!(
        account.descendants().any(|node| {
            node.has_tag_name("span")
                && node.attribute("class").is_some_and(|classes| {
                    classes.split_whitespace().any(|class| class == "nodeLabel")
                })
                && node.attribute("style").is_some_and(|style| {
                    style.contains("color:#ddeeff") && style.contains("fill:#ddeeff")
                })
        }),
        "typed Default NodeLabel.fill must reach the Class label shell: {}",
        rendered.svg(),
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn class_direct_node_surfaces_preserve_source_owned_occurrences() {
    let rendered = try_render_class_svg_with_theme_and_engine(
        r#"classDiagram
  class SourceOwned
  class ThemeOwned
  style SourceOwned fill:#aa0000,stroke:#bb0000,color:#cc0000
"#,
        &class_node_surface_theme(),
        Engine::new(),
    )
    .expect("source-owned Class node surfaces must coexist with direct typed occurrences");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Class SVG");
    let terminal_node = |semantic_id: &str| {
        document
            .descendants()
            .find(|node| {
                node.is_element()
                    && node.tag_name().name() == "g"
                    && node.attribute("id").is_some_and(|id| {
                        id.starts_with(format!("merman-classId-{semantic_id}-").as_str())
                    })
            })
            .unwrap_or_else(|| panic!("missing Class node {semantic_id}"))
    };
    let source_owned = terminal_node("SourceOwned");
    let theme_owned = terminal_node("ThemeOwned");

    assert!(
        source_owned
            .descendants()
            .any(|node| node.has_tag_name("path") && node.attribute("fill") == Some("#aa0000")),
        "source Node.fill must remain the terminal winner: {}",
        rendered.svg(),
    );
    assert!(
        source_owned
            .descendants()
            .any(|node| node.has_tag_name("path") && node.attribute("stroke") == Some("#bb0000")),
        "source Node.stroke must remain the terminal winner: {}",
        rendered.svg(),
    );
    assert!(
        source_owned.descendants().any(|node| {
            node.has_tag_name("span")
                && node
                    .attribute("style")
                    .is_some_and(|style| style.contains("color:#cc0000"))
        }),
        "source label paint must remain the terminal winner: {}",
        rendered.svg(),
    );
    assert!(
        theme_owned
            .descendants()
            .any(|node| node.has_tag_name("path") && node.attribute("fill") == Some("#112233")),
        "unowned Node.fill must retain the typed winner: {}",
        rendered.svg(),
    );
    assert!(
        theme_owned.descendants().any(|node| {
            node.has_tag_name("span")
                && node.attribute("style").is_some_and(|style| {
                    style.contains("color:#ddeeff") && style.contains("fill:#ddeeff")
                })
        }),
        "unowned NodeLabel.fill must retain the typed winner: {}",
        rendered.svg(),
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn class_explicit_default_node_surfaces_preserve_source_owned_occurrences() {
    let rendered = try_render_class_svg_with_theme_and_engine(
        r#"classDiagram
  class SourceOwned
  class ThemeOwned
  style SourceOwned fill:#aa0000,stroke:#bb0000,color:#cc0000
"#,
        &class_node_default_surface_theme(),
        Engine::new(),
    )
    .expect("source-owned Class node surfaces must coexist with typed Default occurrences");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Class SVG");
    let terminal_node = |semantic_id: &str| {
        document
            .descendants()
            .find(|node| {
                node.is_element()
                    && node.tag_name().name() == "g"
                    && node.attribute("id").is_some_and(|id| {
                        id.starts_with(format!("merman-classId-{semantic_id}-").as_str())
                    })
            })
            .unwrap_or_else(|| panic!("missing Class node {semantic_id}"))
    };
    let source_owned = terminal_node("SourceOwned");
    let theme_owned = terminal_node("ThemeOwned");

    assert!(
        source_owned
            .descendants()
            .any(|node| node.has_tag_name("path") && node.attribute("fill") == Some("#aa0000")),
        "source Node.fill must remain the terminal winner: {}",
        rendered.svg(),
    );
    assert!(
        source_owned
            .descendants()
            .any(|node| node.has_tag_name("path") && node.attribute("stroke") == Some("#bb0000")),
        "source Node.stroke must remain the terminal winner: {}",
        rendered.svg(),
    );
    assert!(
        source_owned.descendants().any(|node| {
            node.has_tag_name("span")
                && node
                    .attribute("style")
                    .is_some_and(|style| style.contains("color:#cc0000"))
        }),
        "source label paint must remain the terminal winner: {}",
        rendered.svg(),
    );
    assert!(
        theme_owned
            .descendants()
            .any(|node| node.has_tag_name("path") && node.attribute("fill") == Some("#112233")),
        "unowned Node.fill must retain the typed Default winner: {}",
        rendered.svg(),
    );
    assert!(
        theme_owned
            .descendants()
            .any(|node| node.has_tag_name("path") && node.attribute("stroke") == Some("#445566")),
        "unowned Node.stroke must retain the typed Default winner: {}",
        rendered.svg(),
    );
    assert!(
        theme_owned.descendants().any(|node| {
            node.has_tag_name("span")
                && node.attribute("style").is_some_and(|style| {
                    style.contains("color:#ddeeff") && style.contains("fill:#ddeeff")
                })
        }),
        "unowned NodeLabel.fill must retain the typed Default winner: {}",
        rendered.svg(),
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn class_invalid_source_css_cannot_swallow_typed_node_paint_or_forge_its_receipt() {
    let source = r#"classDiagram
  class Account
  style Account color:red!important;fill:#00ff00!important/*
"#;
    let rendered = try_render_class_svg_with_theme_and_engine(
        source,
        &class_node_surface_theme(),
        Engine::new(),
    )
    .expect("structurally invalid source CSS must not override direct Class terminal paint");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Class SVG");
    let account = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node
                    .attribute("id")
                    .is_some_and(|id| id.starts_with("merman-classId-Account-"))
        })
        .expect("Class Account terminal node");

    assert!(
        account
            .descendants()
            .any(|node| node.has_tag_name("path") && node.attribute("fill") == Some("#112233")),
        "typed Node.fill must remain the terminal value: {}",
        rendered.svg(),
    );
    assert!(
        account.descendants().any(|node| {
            node.has_tag_name("span")
                && node.attribute("style").is_some_and(|style| {
                    style.contains("color:#ddeeff") && style.contains("fill:#ddeeff")
                })
        }),
        "typed NodeLabel.fill must remain the terminal value: {}",
        rendered.svg(),
    );
    assert!(!rendered.svg().contains("#00ff00"), "{}", rendered.svg());
    assert!(!rendered.svg().contains("/*"), "{}", rendered.svg());

    let resvg = rendered
        .finalize_resvg(&SvgPipeline::resvg_safe())
        .expect("finalize adversarial Class source CSS for ResvgSafe");
    {
        let native_svg = merman_render::__private::native_export_svg(resvg.svg());
        let native_document =
            roxmltree::Document::parse(native_svg).expect("valid native Class SVG");
        assert!(
            native_document.descendants().any(|node| {
                node.has_tag_name("path") && node.attribute("fill") == Some("#112233")
            }),
            "native Class shell must retain typed Node.fill: {native_svg}",
        );
    }

    let completion = resvg.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn class_unsupported_node_ordinals_only_suppress_static_winners_on_matching_occurrences() {
    let rendered = try_render_class_svg_with_theme_requirement(
        "classDiagram\n  class First\n  class Second\n  class Third\n",
        &class_node_ordinal_surface_theme(),
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("resolve Class static and ordinal node winners per occurrence");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Class SVG");

    for semantic_id in ["First", "Third"] {
        let node = document
            .descendants()
            .find(|node| {
                node.is_element()
                    && node.tag_name().name() == "g"
                    && node.attribute("id").is_some_and(|id| {
                        id.starts_with(format!("merman-classId-{semantic_id}-").as_str())
                    })
            })
            .unwrap_or_else(|| panic!("missing Class node {semantic_id}"));
        assert!(
            node.descendants()
                .any(|terminal| terminal.has_tag_name("path")
                    && terminal.attribute("fill") == Some("#112233")),
            "Class node {semantic_id} must retain its occurrence-local static fill winner: {}",
            rendered.svg(),
        );
        assert!(
            node.descendants().any(|terminal| {
                terminal.has_tag_name("span")
                    && terminal.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "nodeLabel")
                    })
                    && terminal.attribute("style").is_some_and(|style| {
                        style.contains("color:#ddeeff") && style.contains("fill:#ddeeff")
                    })
            }),
            "Class node {semantic_id} must retain its occurrence-local static label winner: {}",
            rendered.svg(),
        );
    }

    let second = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node
                    .attribute("id")
                    .is_some_and(|id| id.starts_with("merman-classId-Second-"))
        })
        .expect("Class Second node");
    assert!(
        !second
            .descendants()
            .any(|terminal| terminal.has_tag_name("path")
                && terminal.attribute("fill") == Some("#112233")),
        "the unsupported ordinal winner must suppress static Node.fill only for its own occurrence: {}",
        rendered.svg(),
    );
    assert!(
        !second.descendants().any(|terminal| {
            terminal.has_tag_name("span")
                && terminal.attribute("style").is_some_and(|style| {
                    style.contains("color:#ddeeff") || style.contains("fill:#ddeeff")
                })
        }),
        "the unsupported ordinal winner must suppress static NodeLabel.fill only for its own occurrence: {}",
        rendered.svg(),
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 2);
}

#[test]
fn class_lollipop_interfaces_are_node_and_label_terminal_occurrences() {
    let rendered = try_render_class_svg_with_theme_and_engine(
        "classDiagram\n  IService ()-- Service\n",
        &class_node_surface_theme(),
        Engine::new(),
    )
    .expect("theme the Class node and lollipop interface occurrences");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Class SVG");

    let service = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node
                    .attribute("id")
                    .is_some_and(|id| id.starts_with("merman-classId-Service-"))
        })
        .expect("Class Service node");
    assert!(
        service
            .descendants()
            .any(|node| node.has_tag_name("path") && node.attribute("fill") == Some("#112233")),
        "the ordinary Class node must retain the static occurrence winner: {}",
        rendered.svg(),
    );

    let interface = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node.attribute("id") == Some("merman-interface0")
        })
        .expect("Class lollipop interface node");
    let container = interface
        .children()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|class| class == "label-container")
                })
        })
        .expect("Class interface terminal container");
    assert!(
        container
            .attribute("style")
            .is_some_and(|style| style.contains("opacity:0"))
            && container.attribute("fill") != Some("#112233")
            && container.attribute("stroke") != Some("#445566")
            && container.attribute("style").is_none_or(|style| {
                !style.contains("fill:#112233") && !style.contains("stroke:#445566")
            }),
        "the invisible lollipop hitbox must not impersonate a visible Node paint terminal: {}",
        rendered.svg(),
    );
    assert!(
        interface.descendants().any(|node| {
            node.has_tag_name("span")
                && node.attribute("class").is_some_and(|classes| {
                    classes.split_whitespace().any(|class| class == "nodeLabel")
                })
                && node.attribute("style").is_some_and(|style| {
                    style.contains("color:#ddeeff") && style.contains("fill:#ddeeff")
                })
        }),
        "the lollipop label must carry its NodeLabel terminal paint: {}",
        rendered.svg(),
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn class_lollipop_hitbox_cannot_be_the_only_node_paint_proof() {
    let theme = class_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Node,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#112233").expect("valid Class node fill"))
            .with_stroke(CanvasPaint::solid("#445566").expect("valid Class node stroke")),
    )]);
    let rendered = try_render_class_svg_with_theme_and_engine(
        "classDiagram\n  IService ()-- Service\n  style Service fill:#aa0000,stroke:#bb0000\n",
        &theme,
        Engine::new(),
    )
    .expect("an invisible lollipop hitbox makes Node paint not applicable");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Class SVG");
    let interface_hitbox = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|class| class == "label-container")
                })
                && node.parent().and_then(|parent| parent.attribute("id"))
                    == Some("merman-interface0")
        })
        .expect("Class interface hitbox");
    assert_ne!(interface_hitbox.attribute("fill"), Some("#112233"));
    assert_ne!(interface_hitbox.attribute("stroke"), Some("#445566"));

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn class_svg_labels_emit_the_structured_source_color_they_claim_owns_paint() {
    let theme = class_edge_rules_theme([ThemeRule::new(
        ThemeTarget::NodeLabel,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#ddeeff").expect("valid Class label fill")),
    )]);
    let rendered = try_render_class_svg_with_theme_and_engine(
        "classDiagram\n  class SourceOwned\n  style SourceOwned color:#cc0000\n",
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "htmlLabels": false,
        }))),
    )
    .expect("source-owned SVG Class label paint is terminally emitted");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Class SVG");
    let source_label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("style").is_some_and(|style| {
                    style.contains("color:#cc0000") && style.contains("fill:#cc0000")
                })
        })
        .unwrap_or_else(|| panic!("source color must reach SVG text: {}", rendered.svg()));
    assert!(
        source_label
            .descendants()
            .filter_map(|node| node.text())
            .any(|text| text.contains("SourceOwned")),
        "the source-colored terminal must own the visible title: {}",
        rendered.svg(),
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[cfg(feature = "math")]
#[test]
fn class_mixed_math_cannot_prove_an_inherited_typed_label_color() {
    let theme = class_edge_rules_theme([ThemeRule::new(
        ThemeTarget::NodeLabel,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#22cc88").expect("valid Class label fill")),
    )]);
    let error = match try_render_class_svg_with_theme_environment(
        "classDiagram\n  class Formula[\"value: $$x^2$$\"]\n",
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "htmlLabels": true,
        }))),
        ThemePortabilityRequirement::RequirePortable,
        RenderEnvironment::deterministic().with_compiled_math_renderer(),
    ) {
        Ok(_) => panic!("formula glyph paint is not proven by the inherited XHTML shell color"),
        Err(error) => error,
    };
    assert_eq!(
        error.incomplete_family_theme(),
        Some((merman_render::DiagramFamilyId::CLASS, 1, 0)),
    );
}

#[test]
fn class_html_label_fill_ownership_is_property_local_and_visible_run_local() {
    let theme = class_edge_rules_theme([ThemeRule::new(
        ThemeTarget::NodeLabel,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#ddeeff").expect("valid Class label fill")),
    )]);
    let error = match try_render_class_svg_with_theme_and_engine(
        r#"classDiagram
  class Mixed["<span style='color:#aa0000'>source-owned</span> inherited"]
"#,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "htmlLabels": true,
            "securityLevel": "loose",
        }))),
    ) {
        Ok(_) => {
            panic!("mixed descendant and inherited paint cannot be certified across native export")
        }
        Err(error) => error,
    };
    assert_eq!(
        error.incomplete_family_theme(),
        Some((merman_render::DiagramFamilyId::CLASS, 1, 0)),
    );

    let rendered = try_render_class_svg_with_theme_requirement(
        r#"classDiagram
  class Mixed["<span style='color:#aa0000'>source-owned</span> inherited"]
"#,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "htmlLabels": true,
            "securityLevel": "loose",
        }))),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("best-effort browser SVG may preserve mixed descendant paint");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Class SVG");

    let node = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node
                    .attribute("id")
                    .is_some_and(|id| id.starts_with("merman-classId-Mixed-"))
        })
        .expect("mixed Class node");
    assert!(
        node.descendants().any(|terminal| {
            terminal.has_tag_name("span")
                && terminal.attribute("class").is_some_and(|classes| {
                    classes.split_whitespace().any(|class| class == "nodeLabel")
                })
                && terminal.attribute("style").is_some_and(|style| {
                    style.contains("color:#ddeeff") && style.contains("fill:#ddeeff")
                })
        }),
        "best-effort browser SVG must still paint inherited runs: {}",
        rendered.svg(),
    );
    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("span")
                && node
                    .attribute("style")
                    .is_some_and(|style| style.contains("color:#aa0000"))
                && node.text() == Some("source-owned")
        }),
        "the explicitly colored child run must remain source-owned: {}",
        rendered.svg(),
    );
}

#[test]
fn class_non_intersecting_ordinal_stroke_does_not_suppress_static_stroke() {
    let theme = class_edge_rules_theme([
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(
                CanvasPaint::solid("#123456").expect("valid static Class relation stroke"),
            ),
        ),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(
                CanvasPaint::solid("#ef4444").expect("valid ordinal Class relation stroke"),
            ),
        )
        .with_ordinal(OrdinalSelector::exact(99).expect("valid non-intersecting Class ordinal")),
    ]);

    let rendered = try_render_class_svg_with_theme_and_engine(
        "classDiagram\n  A *-- B\n",
        &theme,
        Engine::new(),
    )
    .expect("a non-intersecting unsupported ordinal must not suppress the static direct route");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Class SVG");
    let relation = document
        .descendants()
        .find(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .expect("Class relation path");
    assert!(
        relation
            .attribute("style")
            .is_some_and(|style| style.contains("stroke:#123456 !important")),
        "the static Class stroke must remain the terminal winner: {}",
        rendered.svg(),
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn class_unsupported_ordinal_stroke_does_not_suppress_static_stroke_on_other_relations() {
    let theme = class_edge_rules_theme([
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(
                CanvasPaint::solid("#123456").expect("valid static Class relation stroke"),
            ),
        ),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(
                CanvasPaint::solid("#ef4444").expect("valid ordinal Class relation stroke"),
            ),
        )
        .with_ordinal(OrdinalSelector::exact(1).expect("valid Class ordinal")),
    ]);

    let rendered = try_render_class_svg_with_theme_requirement(
        "classDiagram\n  A *-- B\n  B *-- C\n",
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    let rendered =
        rendered.expect("an unsupported ordinal must not suppress the static direct route");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Class SVG");
    let relation_paths = document
        .descendants()
        .filter(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .collect::<Vec<_>>();
    assert_eq!(relation_paths.len(), 2, "{}", rendered.svg());
    assert!(
        relation_paths.iter().all(|relation| relation
            .attribute("style")
            .is_some_and(|style| style.contains("stroke:#123456 !important"))),
        "unsupported ordinal must not clear the static stroke on any relation: {}",
        rendered.svg(),
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn class_direct_edge_stroke_supports_transparent_terminals() {
    for theme in class_edge_scalar_themes(CanvasPaint::Transparent) {
        let svg = render_class_svg_with_theme("classDiagram\n  A *-- B\n", &theme, Engine::new());
        let document = roxmltree::Document::parse(&svg).expect("valid transparent Class SVG");
        let relation = document
            .descendants()
            .find(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
            .expect("Class relation path");
        assert!(
            relation
                .attribute("style")
                .is_some_and(|style| style.contains("stroke:transparent !important")),
            "{svg}"
        );
        let marker_id = relation
            .attribute("marker-start")
            .expect("composition marker reference")
            .strip_prefix("url(#")
            .and_then(|value| value.strip_suffix(')'))
            .expect("composition marker id");
        let marker_terminal = document
            .descendants()
            .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(marker_id))
            .and_then(|marker| marker.children().find(roxmltree::Node::is_element))
            .expect("composition marker terminal");
        let style = marker_terminal
            .attribute("style")
            .expect("typed marker style");
        assert!(style.contains("stroke:transparent !important"), "{svg}");
        assert!(style.contains("fill:transparent !important"), "{svg}");
    }
}

#[test]
fn class_explicit_site_and_secure_source_line_color_outrank_typed_edge_stroke() {
    for theme in class_edge_scalar_themes(
        CanvasPaint::solid("#123456").expect("valid Class relation stroke"),
    ) {
        let cases = [
            (
                "site",
                "classDiagram\n  A *-- B\n  note for A \"memo\"\n",
                Engine::new().with_site_config(MermaidConfig::from_value(json!({
                    "themeVariables": { "lineColor": "#abcdef" }
                }))),
                "#abcdef",
            ),
            (
                "source",
                "%%{init: {\"themeVariables\": {\"lineColor\": \"#fedcba\"}}}%%\nclassDiagram\n  A *-- B\n  note for A \"memo\"\n",
                legacy_init_theme_compat_engine(),
                "#fedcba",
            ),
        ];

        for (owner, source, engine, mermaid_stroke) in cases {
            let rendered = try_render_class_svg_with_theme_and_engine(source, &theme, engine)
                .expect("render source-owned Class relation stroke");
            let svg = rendered.svg().to_owned();
            {
                let document = roxmltree::Document::parse(&svg).expect("valid owned Class SVG");
                let paths = document
                    .descendants()
                    .filter(|node| {
                        node.has_tag_name("path") && node.attribute("data-edge") == Some("true")
                    })
                    .collect::<Vec<_>>();
                assert_eq!(paths.len(), 2);
                assert!(svg.contains(mermaid_stroke), "owner={owner}: {svg}");
                assert!(
                    paths.iter().all(|path| path
                        .attribute("style")
                        .is_none_or(|style| !style.contains("#123456"))),
                    "owner={owner}: explicit Mermaid lineColor must suppress typed stroke: {svg}"
                );
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 0, "owner={owner}");
            assert_eq!(evidence.not_applicable_count(), 1, "owner={owner}");
            assert_eq!(evidence.theme_residual_count(), 0, "owner={owner}");
        }
    }
}

#[test]
fn class_edge_scalar_reaches_attached_note_connectors() {
    for (source, note_id) in [
        ("classDiagram\nclass A\nnote for A \"memo\"\n", "edgeNote0"),
        ("classDiagram\nA *-- B\nnote for A \"memo\"\n", "edgeNote0"),
        (
            "classDiagram\nclass A\nnote \"unattached\"\nnote for A \"memo\"\n",
            "edgeNote1",
        ),
    ] {
        for paint in [
            CanvasPaint::solid("#123456").unwrap(),
            CanvasPaint::Transparent,
        ] {
            let css = if paint == CanvasPaint::Transparent {
                "transparent"
            } else {
                "#123456"
            };
            for theme in class_edge_scalar_themes(paint) {
                for look in ["classic", "handDrawn"] {
                    let rendered = try_render_class_svg_with_theme_and_engine(
                        source,
                        &theme,
                        Engine::new()
                            .with_site_config(MermaidConfig::from_value(json!({"look": look}))),
                    )
                    .expect("typed Class note connector");
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    let connector = document
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("path") && node.attribute("data-id") == Some(note_id)
                        })
                        .expect("attached note connector");
                    assert!(
                        connector.attribute("style").is_some_and(|style| {
                            style.contains(&format!("stroke:{css} !important"))
                        }),
                        "look={look}: {}",
                        rendered.svg()
                    );
                    if look == "handDrawn" {
                        assert_eq!(connector.attribute("stroke"), Some(css));
                    }
                    let evidence = merman_render::__private::family_evidence(
                        rendered.into_completion().report(),
                    );
                    assert_eq!(evidence.applied_count(), 1);
                    assert_eq!(evidence.theme_residual_count(), 0);
                }
            }
        }
    }
}

#[test]
fn class_note_paint_matches_source_identity_across_namespace_writer_order() {
    let source = r#"classDiagram
namespace Internal {
    class A
    note for A "inside"
}
class X
note for X "outside"
"#;
    let baseline = render_class_svg_from_text(source);
    for theme in class_edge_scalar_themes(CanvasPaint::solid("#123456").unwrap()) {
        let rendered = try_render_class_svg_with_theme_and_engine(source, &theme, Engine::new())
            .expect("nested note paint does not depend on global writer order");
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let ids = document
            .descendants()
            .filter(|node| node.has_tag_name("path"))
            .filter_map(|node| node.attribute("data-id"))
            .filter(|id| id.starts_with("edgeNote"))
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            ["edgeNote1", "edgeNote0"],
            "fixture must reverse source declaration order: {baseline}"
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
    }
}

#[test]
fn class_note_connectors_preserve_legacy_line_color_consumption() {
    let svg = render_class_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "themeVariables": { "lineColor": "#123456" }
        }))),
        "classDiagram\nclass A\nnote for A \"memo\"\n",
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let connector = document
        .descendants()
        .find(|node| node.has_tag_name("path") && node.attribute("data-id") == Some("edgeNote0"))
        .expect("legacy note connector");
    assert!(
        connector
            .attribute("class")
            .unwrap()
            .split_whitespace()
            .any(|class| class == "relation")
    );
    assert!(svg.contains(".relation{stroke:#123456;"), "{svg}");
    assert!(
        !connector
            .attribute("style")
            .unwrap_or_default()
            .contains("stroke:")
    );
}

#[test]
fn class_note_connectors_do_not_extend_relation_ordinals_or_width() {
    let note_only = "classDiagram\nclass A\nnote for A \"memo\"\n";
    let mixed = "classDiagram\nA --> B\nnote for A \"memo\"\n";
    for (source, ordinal, applies) in [(note_only, 1, false), (mixed, 1, true), (mixed, 2, false)] {
        let theme = class_edge_rules_theme([ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        )
        .with_ordinal(OrdinalSelector::exact(ordinal).unwrap())]);
        let result = try_render_class_svg_with_theme_and_engine(source, &theme, Engine::new());
        if applies {
            assert_eq!(
                result
                    .err()
                    .expect("unsupported relation ordinal")
                    .unverified_family_theme(),
                Some((DiagramFamilyId::CLASS, 1))
            );
        } else {
            let rendered = result.expect("notes do not introduce relation ordinal occurrences");
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.not_applicable_count(), 1);
        }
    }
    let theme = class_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").unwrap())
            .with_stroke_width(6.0)
            .unwrap(),
    )]);
    let rendered = try_render_class_svg_with_theme_and_engine(note_only, &theme, Engine::new())
        .expect("note paint is applicable while relation width is absent");
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let connector = document
        .descendants()
        .find(|node| node.attribute("data-id") == Some("edgeNote0"))
        .unwrap();
    assert!(
        !connector
            .attribute("style")
            .unwrap_or_default()
            .contains("stroke-width:6")
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
}

#[test]
fn class_edge_stroke_is_not_applicable_without_relations() {
    for theme in class_edge_scalar_themes(
        CanvasPaint::solid("#123456").expect("valid Class relation stroke"),
    ) {
        let rendered = try_render_class_svg_with_theme_and_engine(
            "classDiagram\n  class A\n  note \"unattached\"\n",
            &theme,
            Engine::new(),
        )
        .expect("Class edge stroke without relations is not applicable");
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn class_edge_stroke_rejects_ordinal_clear_gradient_and_pattern_routes() {
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(
                0.0,
                ThemeColorValue::parse("#123456").expect("valid Class gradient start"),
            )
            .expect("valid Class gradient stop"),
            GradientStop::new(
                1.0,
                ThemeColorValue::parse("#abcdef").expect("valid Class gradient end"),
            )
            .expect("valid Class gradient stop"),
        ],
    )
    .expect("valid Class gradient");
    let pattern = PatternSpec::new(
        PatternKind::Grid,
        8.0,
        8.0,
        ThemeColorValue::parse("#123456").expect("valid Class pattern color"),
    )
    .expect("valid Class pattern");
    let mut clear = ThemeStylePatch::default();
    clear.stroke.paint = Specified::Clear;
    let solid = || {
        ThemeStylePatch::default()
            .with_stroke(CanvasPaint::solid("#123456").expect("valid Class stroke"))
    };
    let unsupported_cases = [
        ThemeRule::new(ThemeTarget::Edge, solid())
            .with_ordinal(OrdinalSelector::exact(1).expect("valid Class edge ordinal")),
        ThemeRule::new(ThemeTarget::Edge, clear),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::LinearGradient(gradient)),
        ),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::Pattern(pattern)),
        ),
    ];

    for rule in unsupported_cases {
        let error = match try_render_class_svg_with_theme_and_engine(
            "classDiagram\n  A --> B\n",
            &class_edge_rules_theme([rule]),
            Engine::new(),
        ) {
            Ok(_) => panic!("unsupported Class edge stroke route must fail closed"),
            Err(error) => error,
        };
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::CLASS, 1))
        );
    }
}

#[test]
fn class_table_palette_and_effect_require_real_row_occurrences() {
    let visible_rows = "classDiagram\nclass A {\n  +String name\n  +run()\n}\n";
    let empty_table = "classDiagram\nclass A\n";

    for theme in [class_table_palette_theme(), class_table_effect_theme()] {
        let error =
            match try_render_class_svg_with_theme_and_engine(visible_rows, &theme, Engine::new()) {
                Ok(_) => {
                    panic!("visible Class rows must retain unsupported Table theme mechanisms")
                }
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::CLASS, 1)),
        );

        let rendered =
            try_render_class_svg_with_theme_and_engine(empty_table, &theme, Engine::new())
                .expect("an empty Class table makes the unsupported mechanism not applicable");
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn class_node_and_label_fallbacks_are_reconciled_against_visible_nodes() {
    let theme = class_node_fallback_theme();
    let visible = try_render_class_svg_with_theme_requirement(
        "classDiagram\n  class A\n",
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("best-effort Class node fallback render");
    let evidence = merman_render::__private::family_evidence(visible.into_completion().report());
    assert_eq!(evidence.required_count(), 3);
    assert_eq!(evidence.accounted_count(), 3);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 3);

    let empty = try_render_class_svg_with_theme_requirement(
        "classDiagram\n",
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("an empty Class makes node fallbacks not applicable");
    let evidence = merman_render::__private::family_evidence(empty.into_completion().report());
    assert_eq!(evidence.required_count(), 3);
    assert_eq!(evidence.accounted_count(), 3);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 3);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn class_svg_security_level_controls_unsafe_click_href_rendering() {
    let strict = render_class_svg_from_text(
        r#"%%{init: {"securityLevel": "strict"}}%%
classDiagram
class Class1
click Class1 href "javascript:alert(1)" "tip" _self
"#,
    );
    assert!(
        strict.contains(r#"<a data-look=""#),
        "expected strict mode to keep Mermaid's anchor wrapper for a declared Class link: {strict}"
    );
    assert!(
        !strict.contains(r#"xlink:href="javascript:alert(1)""#),
        "expected strict mode to omit unsafe Class click href from SVG: {strict}"
    );
    assert!(
        !strict.contains(r#"xlink:href="about:blank""#),
        "expected Mermaid-compatible strict Class SVG to omit sanitized about:blank href: {strict}"
    );

    let loose = render_class_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }))),
        r#"classDiagram
class Class1
click Class1 href "notes://do-your-thing/id" "tip" _self
"#,
    );
    assert!(
        loose.contains(r#"xlink:href="notes://do-your-thing/id""#),
        "expected loose mode to preserve Mermaid formatUrl-compatible Class custom protocols: {loose}"
    );
    assert!(
        loose.contains(r#"target="_self""#),
        "expected loose class parity to preserve the Mermaid link target: {loose}"
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_svg_elk_layout_preserves_existing_renderer_semantics() {
    let svg = render_class_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }))),
        r##"---
config:
  layout: elk
---
classDiagram
direction LR
namespace Platform {
  class Service:::critical {
    +start()
  }
}
class Client {
  +request()
}
Client "1" --> "many" Service : calls
note for Service "ELK note"
click Service href "https://example.com/service" "Open Service" _blank
classDef critical fill:#ffdddd,stroke:#aa0000,stroke-width:2px,color:#111111
style Client fill:#ddffdd,stroke:#00aa00,stroke-width:2px
"##,
    );

    assert!(
        svg.contains(r#"id="merman-Platform" data-look="classic""#)
            && svg.contains(r#"data-look="classic" xlink:href="https://example.com/service""#)
            && svg.contains(r#"id="merman-classId-Service-0""#)
            && svg.contains(r#"id="merman-classId-Client-"#),
        "Class ELK layout should still render namespaces and class nodes through the Class SVG renderer: {svg}"
    );
    assert!(
        svg.contains(r#"xlink:href="https://example.com/service""#)
            && svg.contains(r#"title="Open Service""#),
        "Class ELK layout should preserve Class click/link SVG semantics: {svg}"
    );
    assert!(
        svg.contains(r#"style="fill:#ffdddd;stroke:#aa0000;stroke-width:2px;color:#111111""#)
            && svg.contains(r#"style="fill:#ddffdd;stroke:#00aa00;stroke-width:2px""#),
        "Class ELK layout should preserve classDef and inline style SVG semantics: {svg}"
    );
    assert!(
        svg.contains("<p>ELK note</p>")
            && svg.contains(r#"<span class="edgeLabel"><p>calls</p></span>"#)
            && svg.contains(r#"<span class="edgeLabel"><p>many</p></span>"#),
        "Class ELK layout should preserve notes, relation labels, and cardinality terminals: {svg}"
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_svg_elk_layout_uses_common_painter_dom() {
    let svg = render_class_svg_from_text(
        r#"---
config:
  layout: elk
---
classDiagram
direction LR
class Animal
class Duck
Animal <|-- Duck
        "#,
    );

    let root = svg
        .find(r#"<g class="root""#)
        .expect("Class ELK common root group");
    let nodes = svg
        .find(r#"<g class="nodes""#)
        .expect("Class ELK nodes group");
    let edges = svg
        .find(r#"<g class="edges edgePath""#)
        .expect("Class ELK edge paths group");
    let labels = svg
        .find(r#"<g class="edgeLabels""#)
        .expect("Class ELK edge labels group");
    let clusters = svg
        .find(r#"<g class="clusters""#)
        .expect("Class ELK clusters group");
    assert!(root < edges && edges < clusters && clusters < labels && labels < nodes);
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_svg_elk_layout_uses_layout_elk_023_marker_profile() {
    let svg = render_class_svg_from_text(
        r#"---
config:
  layout: elk
---
classDiagram
class C1["One"]
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Class ELK SVG");
    let marker_units = |name: &str| {
        document
            .descendants()
            .find(|node| {
                node.has_tag_name("marker")
                    && node.attribute("id").is_some_and(|id| id.ends_with(name))
            })
            .and_then(|node| node.attribute("markerUnits"))
    };
    for marker in [
        "aggregationStart",
        "aggregationEnd",
        "extensionEnd",
        "compositionStart",
        "compositionEnd",
        "dependencyStart",
        "dependencyEnd",
        "lollipopStart",
        "lollipopEnd",
    ] {
        assert_eq!(
            marker_units(marker),
            None,
            "ELK 0.2.3 ordinary marker {marker}"
        );
    }
    assert_eq!(marker_units("extensionStart"), Some("userSpaceOnUse"));
    assert_eq!(
        marker_units("aggregationStart-margin"),
        Some("userSpaceOnUse")
    );
}

#[test]
fn class_svg_namespace_clusters_keep_theme_fill() {
    let svg = render_class_svg_from_text(
        r#"classDiagram
namespace Platform {
  class Api
}
namespace Platform.FFI {
  class Bridge
}
namespace Platform.Core {
  class Engine
}
"#,
    );

    assert!(
        svg.contains(r#"#merman .cluster rect{fill:#ffffde;stroke:#aaaa33;stroke-width:1px;}"#),
        "expected class namespace cluster CSS to provide the upstream yellow fill: {svg}"
    );
    assert!(
        !svg.contains(r#"style="fill:none !important;stroke:black !important""#),
        "namespace cluster rects must not override the theme fill with transparent inline CSS: {svg}"
    );
}

#[test]
fn class_svg_honors_numeric_stroke_width_theme_css() {
    let svg = render_class_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"themeVariables": {"mainBkg": "#112233", "nodeBorder": "#445566", "lineColor": "#778899", "strokeWidth": 7}}}%%
classDiagram
    Animal <|-- Dog
    class Animal
    class Dog
"##,
    );

    assert!(
        svg.contains(
            r#"#merman .node rect,#merman .node circle,#merman .node ellipse,#merman .node polygon,#merman .node path{fill:#112233;stroke:#445566;stroke-width:7;}"#
        ),
        "expected numeric strokeWidth to drive Class node shape CSS: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .edge-thickness-normal{stroke-width:7px;}"#),
        "expected numeric strokeWidth to drive Mermaid's common edge CSS: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .divider{stroke:#445566;stroke-width:1;}"#),
        "expected nodeBorder to drive Class divider CSS: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .relation{stroke:#778899;stroke-width:7;fill:none;}"#),
        "expected numeric strokeWidth to drive Class relation CSS: {svg}"
    );
    assert!(
        !svg.contains(r#"#merman .relation{stroke:#778899;stroke-width:1;fill:none;}"#),
        "Class relation CSS must not drop numeric strokeWidth overrides: {svg}"
    );
}

#[test]
fn class_svg_honors_configured_note_theme_colors() {
    for html_labels in [true, false] {
        let svg = render_class_svg_from_text_with_engine(
            legacy_init_theme_compat_engine(),
            &format!(
                r##"%%{{init: {{"htmlLabels": {html_labels}, "themeVariables": {{"noteBkgColor": "#112233", "noteBorderColor": "#445566", "noteTextColor": "#778899"}}}}}}%%
classDiagram
    class Animal
    note for Animal "hello"
"##
            ),
        );

        assert!(
            svg.contains(
                r##"fill="#112233" style="fill:#112233 !important;stroke:#445566 !important""##
            ),
            "expected configured noteBkgColor/noteBorderColor in note body for htmlLabels={html_labels}: {svg}"
        );
        assert!(
            svg.contains(r##"stroke="#445566" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style="fill:#112233 !important;stroke:#445566 !important""##),
            "expected configured noteBorderColor in note rough stroke for htmlLabels={html_labels}: {svg}"
        );
        assert!(
            svg.contains(
                r#"#merman .noteLabel .nodeLabel,#merman .noteLabel .edgeLabel{color:#778899;}"#
            ),
            "expected noteTextColor CSS for htmlLabels={html_labels}: {svg}"
        );
        assert!(
            !svg.contains(
                r##"fill="#fff5ad" style="fill:#fff5ad !important;stroke:#aaaa33 !important""##
            ),
            "note shape must not ignore configured colors for htmlLabels={html_labels}: {svg}"
        );
    }
}

#[test]
fn class_svg_namespaces_use_11_15_hierarchical_labels_and_keep_relation_label() {
    let svg = render_class_fixture(
        "upstream_namespaces_and_generics.mmd",
        &LayoutOptions::default(),
        &SvgRenderOptions::default(),
    );

    assert!(svg.contains(r#"id="merman-Company" data-look="classic""#));
    assert!(svg.contains(r#"id="merman-Company.Project" data-look="classic""#));
    assert!(svg.contains(r#"id="merman-Company.Project.Module" data-look="classic""#));
    assert!(
        svg.contains("<p>Company</p>")
            && svg.contains("<p>Project</p>")
            && svg.contains("<p>Module</p>"),
        "expected dotted namespace labels to use Mermaid 11.15 path segments"
    );
    let company_pos = svg
        .find(r#"id="merman-Company""#)
        .expect("Company namespace cluster");
    let project_pos = svg
        .find(r#"id="merman-Company.Project""#)
        .expect("Project namespace cluster");
    let module_pos = svg
        .find(r#"id="merman-Company.Project.Module""#)
        .expect("Module namespace cluster");
    assert!(
        company_pos < project_pos && project_pos < module_pos,
        "namespace clusters must be emitted parent-first so nested frames remain visible"
    );
    assert!(
        svg.contains("<p>manages</p>"),
        "expected relation label text to survive hierarchical namespace rendering"
    );
}

#[test]
fn class_svg_nested_namespace_relation_endpoints_share_node_coordinate_frame() {
    use base64::Engine as _;

    let svg = render_class_fixture(
        "upstream_namespaces_and_generics.mmd",
        &LayoutOptions::default(),
        &SvgRenderOptions::default(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
    let edge = document
        .descendants()
        .find(|node| {
            node.has_tag_name("path") && node.attribute("data-id") == Some("id_Admin_User_1")
        })
        .expect("Admin to User relation path");
    let encoded = edge.attribute("data-points").expect("relation data-points");
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .expect("base64 Class edge points");
    let points: Vec<merman_render::model::LayoutPoint> =
        serde_json::from_slice(&decoded).expect("JSON Class edge points");

    let owning_root = |node: roxmltree::Node<'_, '_>| {
        node.ancestors()
            .find(|ancestor| {
                ancestor.has_tag_name("g")
                    && ancestor.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "root")
                    })
            })
            .expect("recursive Class root")
            .id()
    };
    let node_bounds = |class_id: &str| {
        let node = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node
                        .attribute("id")
                        .is_some_and(|id| id.contains(&format!("classId-{class_id}-")))
            })
            .unwrap_or_else(|| panic!("missing {class_id} node"));
        let transform = node.attribute("transform").expect("node transform");
        let center_x = transform
            .strip_prefix("translate(")
            .and_then(|value| value.strip_suffix(')'))
            .and_then(|value| value.split_once(','))
            .map(|(x, _)| x.trim().parse::<f64>().expect("node x"))
            .expect("translate(x, y)");
        let outer = node
            .descendants()
            .find(|descendant| {
                descendant.has_tag_name("g")
                    && descendant.attribute("class").is_some_and(|classes| {
                        classes
                            .split_whitespace()
                            .any(|class| class == "outer-path")
                    })
            })
            .and_then(|group| group.children().find(|child| child.has_tag_name("path")))
            .expect("class outer path");
        let xs = outer
            .attribute("d")
            .expect("class outer path data")
            .split_whitespace()
            .filter_map(|component| {
                component
                    .strip_prefix('M')
                    .or_else(|| component.strip_prefix('L'))
            })
            .map(|x| x.parse::<f64>().expect("class outer path x"))
            .collect::<Vec<_>>();
        let min_x = xs.iter().copied().fold(f64::INFINITY, f64::min);
        let max_x = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (node, center_x + min_x, center_x + max_x)
    };
    let (admin, _admin_left, admin_right) = node_bounds("Admin");
    let (user, user_left, _user_right) = node_bounds("User");

    assert_eq!(owning_root(edge), owning_root(admin));
    assert_eq!(owning_root(edge), owning_root(user));
    assert!(
        (points.first().expect("source endpoint").x - admin_right).abs() <= 0.01,
        "Admin relation endpoint must touch the rendered node boundary"
    );
    assert!(
        (points.last().expect("target endpoint").x - user_left).abs() <= 0.01,
        "User relation endpoint must touch the rendered node boundary before marker shortening"
    );
}

#[test]
fn class_svg_nested_namespace_subgraphs_keep_mermaid_wrapper_structure() {
    let svg = render_class_fixture(
        "stress_class_comments_inside_namespaces_024.mmd",
        &LayoutOptions::default(),
        &SvgRenderOptions {
            diagram_id: Some("stress_class_comments_inside_namespaces_024".to_string()),
            ..Default::default()
        },
    );

    assert!(
        svg.contains(r#"<g class="root" transform="translate("#)
            && svg.contains(r#"><g class="clusters">"#),
        "expected nested namespace wrapper around the cluster group"
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
    let wrapper = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node.attribute("class") == Some("root")
                && node.attribute("transform").is_some()
        })
        .expect("nested namespace root wrapper");
    let child_classes = wrapper
        .children()
        .filter(|node| node.is_element())
        .filter_map(|node| node.attribute("class"))
        .collect::<Vec<_>>();
    assert_eq!(
        child_classes.get(..4),
        Some(["clusters", "edgePaths", "edgeLabels", "nodes"].as_slice()),
        "nested namespace wrapper must keep Mermaid's direct-child order"
    );
    assert!(svg.contains("<p>Outer.Foo</p>"));
}

#[test]
fn class_svg_namespace_extraction_depends_on_cross_boundary_edges() {
    let extracted = render_class_svg_from_text(
        r#"classDiagram
namespace Internal {
  class A
  note for A "inside"
}
class X
class Y
X --> Y
"#,
    );

    let document = roxmltree::Document::parse(&extracted).expect("valid extracted Class SVG");
    let outer_edge = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "path"
                && node
                    .attribute("data-id")
                    .is_some_and(|id| id.starts_with("id_X_Y_"))
        })
        .expect("outer X-to-Y edge path");
    let outer_edge_roots = outer_edge
        .ancestors()
        .filter(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node.attribute("class") == Some("root")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        outer_edge_roots.len(),
        1,
        "an unrelated outer relation belongs only to the top-level Dagre render root"
    );

    let note_edge = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "path"
                && node.attribute("data-id") == Some("edgeNote0")
        })
        .expect("internal note edge path");
    let note_edge_roots = note_edge
        .ancestors()
        .filter(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node.attribute("class") == Some("root")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        note_edge_roots.len(),
        2,
        "the internal note edge belongs to the extracted namespace root nested in the top-level root"
    );

    for root in [note_edge_roots[1], note_edge_roots[0]] {
        let child_classes = root
            .children()
            .filter(|node| node.is_element())
            .filter_map(|node| node.attribute("class"))
            .collect::<Vec<_>>();
        assert_eq!(
            child_classes,
            ["clusters", "edgePaths", "edgeLabels", "nodes"],
            "each layout-owned render root must preserve Mermaid's direct-child group order"
        );
    }

    let retained = render_class_svg_from_text(
        r#"classDiagram
namespace Internal {
  class A
}
class Outside
A --> Outside
"#,
    );

    let document = roxmltree::Document::parse(&retained).expect("valid retained Class SVG");
    let crossing_edge = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "path"
                && node
                    .attribute("data-id")
                    .is_some_and(|id| id.starts_with("id_A_Outside_"))
        })
        .expect("namespace-crossing edge path");
    let crossing_edge_root_count = crossing_edge
        .ancestors()
        .filter(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node.attribute("class") == Some("root")
        })
        .count();
    assert_eq!(
        crossing_edge_root_count, 1,
        "a boundary-crossing relation must remain owned by the parent Dagre render root"
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| {
                node.is_element()
                    && node.tag_name().name() == "g"
                    && node.attribute("class") == Some("root")
                    && node.attribute("transform").is_some()
            })
            .count(),
        0,
        "a boundary-crossing relation prevents extraction of its namespace cluster"
    );
}

#[test]
fn class_svg_multiple_dotted_namespace_subgraphs_use_segment_labels() {
    let svg = render_class_fixture(
        "stress_class_nested_namespaces_many_levels_021.mmd",
        &LayoutOptions::headless_svg_defaults(),
        &SvgRenderOptions {
            diagram_id: Some("stress_class_nested_namespaces_many_levels_021".to_string()),
            ..Default::default()
        },
    );

    assert!(svg.contains(
        r#"id="stress_class_nested_namespaces_many_levels_021-Root.A" data-look="classic""#
    ));
    assert!(svg.contains(
        r#"id="stress_class_nested_namespaces_many_levels_021-Root.B.B1" data-look="classic""#
    ));
    assert!(
        svg.contains("<p>A</p>") && svg.contains("<p>B1</p>"),
        "expected rendered dotted namespace clusters to use path-segment labels"
    );
    assert!(
        svg.contains("<p>Root.A.A1</p>") && svg.contains("<p>Root.B.B1.B1a</p>"),
        "expected qualified relation facade class labels to remain visible"
    );
}

#[test]
fn class_svg_handles_deep_namespace_subgraph_chain() {
    const DEPTH: usize = 128;
    let source = deep_class_namespace_text(DEPTH);
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let handle = std::thread::Builder::new()
        .name("class-deep-namespace-svg".to_string())
        // Keep the SVG path under a constrained stack while leaving room for its fixed debug
        // render frames; depth remains the recursion-safety signal exercised by this test.
        .stack_size(256 * 1024)
        .spawn(move || {
            render_class_svg_from_text_with_session(
                Engine::new(),
                &source,
                &LayoutOptions::headless_svg_defaults(),
                &SvgRenderOptions::default(),
                session,
            )
        })
        .expect("spawn deep namespace SVG test");
    let svg = handle
        .join()
        .expect("deep namespace SVG render should finish without stack overflow");

    assert!(
        svg.contains("Leaf"),
        "expected deeply nested class member to remain visible"
    );
    assert!(
        svg.contains(r#"id="merman-N0""#),
        "expected outer namespace cluster to be rendered"
    );
    assert!(
        svg.contains("N127"),
        "expected deepest namespace cluster to be rendered"
    );
}

#[test]
fn class_svg_long_relation_labels_wrap_to_mermaid_html_cap() {
    let svg = render_class_fixture(
        "stress_class_long_labels_wrapping_002.mmd",
        &LayoutOptions::headless_svg_defaults(),
        &SvgRenderOptions::default(),
    );

    let label = foreign_object_for_text(
        &svg,
        "<p>edge label with spaces, punctuation, and unicode → αβγ</p>",
    );
    assert!(attr_f64(label, "width") > 0.0 && attr_f64(label, "width") <= 200.0);
    assert!(
        attr_f64(label, "height") > 16.0,
        "long label should wrap: {label}"
    );
    assert!(label.contains("max-width: 200px"));
}

#[test]
fn class_svg_cardinality_terminals_emit_positive_measured_bounds() {
    let svg = render_class_fixture(
        "upstream_relation_types_and_cardinalities_spec.mmd",
        &LayoutOptions::headless_svg_defaults(),
        &SvgRenderOptions::default(),
    );

    let terminal = foreign_object_for_text(&svg, "<p>many</p>");
    assert!(svg.contains(r#"<span class="edgeLabel"><p>many</p></span>"#));
    assert!(attr_f64(terminal, "width") > 0.0);
    assert!(attr_f64(terminal, "height") > 0.0);
}

#[test]
fn class_svg_authored_none_cardinalities_are_not_missing_labels() {
    for diagram_type in ["classDiagram", "classDiagram-v2"] {
        let svg = render_class_svg_from_text(&format!(
            "{diagram_type}\nclass A\nclass B\nA \"none\" --> \"NONE\" B"
        ));

        for label in ["none", "NONE"] {
            assert!(
                svg.contains(&format!("<p>{label}</p>")),
                "{diagram_type} should preserve authored endpoint label {label:?}: {svg}"
            );
        }
    }
}

#[test]
fn class_svg_hand_drawn_cardinality_terminals_keep_xhtml_and_measured_bounds() {
    let svg = render_class_svg_from_text(
        r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
classDiagram
  A "1" --> "many" B
"#,
    );

    let terminal = foreign_object_for_text(&svg, "<p>1</p>");
    assert!(svg.contains(r#"<span class="edgeLabel"><p>1</p></span>"#));
    assert!(attr_f64(terminal, "width") > 0.0);
    assert!(attr_f64(terminal, "height") > 0.0);
}

#[test]
fn class_svg_edge_labels_precede_terminals_in_edge_labels_group() {
    let svg = render_class_fixture(
        "stress_class_parallel_edges_and_cardinality_004.mmd",
        &LayoutOptions::headless_svg_defaults(),
        &SvgRenderOptions::default(),
    );

    let edge_labels_start = svg
        .find(r#"<g class="edgeLabels">"#)
        .expect("edgeLabels group");
    let nodes_start = svg[edge_labels_start..]
        .find(r#"<g class="nodes">"#)
        .map(|idx| edge_labels_start + idx)
        .expect("nodes group after edge labels");
    let section = &svg[edge_labels_start..nodes_start];
    let last_label = section
        .rfind(r#"<g class="edgeLabel""#)
        .expect("edgeLabel group present");
    let first_terminal = section
        .find(r#"<g class="edgeTerminals""#)
        .expect("edge terminal group present");

    assert!(
        last_label < first_terminal,
        "expected Mermaid-style edgeLabels ordering: all edgeLabel groups before edgeTerminals"
    );
}

#[test]
fn class_svg_relation_titles_decode_entities_once() {
    let svg = render_class_fixture(
        "upstream_relation_types_and_cardinalities_spec.mmd",
        &LayoutOptions::default(),
        &SvgRenderOptions::default(),
    );

    assert!(
        svg.contains(r#"<p>&lt; owns</p>"#),
        "expected relation title entities to render exactly once"
    );
    assert!(
        !svg.contains("&amp;lt; owns"),
        "expected relation title entities to avoid double escaping"
    );
}

#[test]
fn class_svg_relation_only_generic_nodes_keep_type_suffix() {
    let svg = render_class_fixture(
        "upstream_cypress_classdiagram_v3_spec_8_should_render_a_simple_class_diagram_with_generic_class_and_re_016.mmd",
        &LayoutOptions::default(),
        &SvgRenderOptions::default(),
    );

    assert!(
        svg.contains("Class01&lt;T")
            && svg.contains("Class03&lt;T")
            && svg.contains("Class04&lt;T"),
        "expected relation-only generic classes to keep Mermaid-matching type suffixes"
    );
}

#[test]
fn class_svg_preserves_numeric_theme_font_size_css_spelling() {
    let svg = render_class_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"fontSize": 10, "themeVariables": {"fontSize": 24}, "htmlLabels": false} }%%
classDiagram
  class FontSizeSvgProbe {
    +veryLongMethodNameToForceMeasurement()
  }
"##,
    );

    assert!(
        svg.contains(
            r#"#merman{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:24;fill:"#
        ),
        "numeric themeVariables.fontSize should be emitted like Mermaid's raw CSS value"
    );
    assert!(
        !svg.contains(
            r#"#merman{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:24px;fill:"#
        ),
        "numeric themeVariables.fontSize must not be rewritten as a px string"
    );
}

#[test]
fn class_svg_px_string_theme_font_size_drives_svg_label_wrapping_without_losing_text() {
    let svg = render_class_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"theme": "base", "fontSize": 10, "themeVariables": {"fontSize": "24px"}, "htmlLabels": false} }%%
classDiagram
  class Foo {
    +veryLongMemberNameToWrapTheLayoutProbe: String
    +anotherVeryLongMemberNameToWrapTheLayoutProbe: String
    +thirdVeryLongMemberNameToWrapTheLayoutProbe: String
  }
"##,
    );

    let document = roxmltree::Document::parse(&svg).expect("valid Class SVG");
    let labels = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|class| class == "label"))
        })
        .map(|node| {
            let rows = node
                .descendants()
                .filter(|descendant| {
                    descendant.has_tag_name("tspan")
                        && descendant.attribute("class").is_some_and(|classes| {
                            classes
                                .split_whitespace()
                                .any(|class| class == "text-outer-tspan")
                        })
                })
                .map(|row| {
                    row.descendants()
                        .filter_map(|descendant| descendant.text().filter(|_| descendant.is_text()))
                        .collect::<String>()
                })
                .collect::<Vec<_>>();
            (rows.join(" "), rows.len())
        })
        .collect::<Vec<_>>();

    for expected in [
        "+veryLongMemberNameToWrapTheLayoutProbe: String",
        "+anotherVeryLongMemberNameToWrapTheLayoutProbe: String",
        "+thirdVeryLongMemberNameToWrapTheLayoutProbe: String",
    ] {
        let expected_compact = expected.split_whitespace().collect::<String>();
        let (_, rows) = labels
            .iter()
            .find(|(text, _)| text.split_whitespace().collect::<String>() == expected_compact)
            .unwrap_or_else(|| panic!("missing complete wrapped member {expected:?}: {labels:?}"));
        assert!(
            *rows >= 2,
            "24px theme text should wrap the long member without depending on a font-specific row boundary: {labels:?}"
        );
    }
}

#[test]
fn class_theme_background_reaches_both_html_and_svg_relation_labels() {
    for look in ["classic", "neo", "handDrawn"] {
        for html_labels in [true, false] {
            let theme = class_edge_rules_theme([ThemeRule::new(
                ThemeTarget::EdgeLabelBackground,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#020617").unwrap()),
            )]);
            let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "look": look, "htmlLabels": html_labels,
            })));
            let rendered = try_render_class_svg_with_theme_requirement(
                "classDiagram\nA --> B : owns\n",
                &theme,
                engine,
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let backgrounds = document
                .descendants()
                .filter(|node| {
                    if html_labels {
                        node.has_tag_name("div") && node.attribute("class") == Some("labelBkg")
                            || node.has_tag_name("span")
                                && node.attribute("class") == Some("edgeLabel")
                    } else {
                        node.has_tag_name("rect")
                            && node.attribute("class") == Some("background")
                            && node.attribute("width").is_some()
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(backgrounds.len(), if html_labels { 2 } else { 1 });
            for node in backgrounds {
                assert!(
                    node.attribute("style").unwrap_or("").contains("#020617"),
                    "{look}/html={html_labels}: theme background missing from {:?}",
                    node
                );
            }
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn class_background_transparent_clear_and_mixed_facets_keep_their_own_outcomes() {
    let source = "classDiagram\nA --> B : owns\n";
    let rule = |patch| ThemeRule::new(ThemeTarget::EdgeLabelBackground, patch);
    let solid = rule(ThemeStylePatch::default().with_fill(CanvasPaint::solid("#020617").unwrap()));
    let transparent = rule(ThemeStylePatch::default().with_fill(CanvasPaint::Transparent));
    let mut clear_patch = ThemeStylePatch::default();
    clear_patch.paint.fill = Specified::Clear;
    let clear = rule(clear_patch);
    let mixed = rule(
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#020617").unwrap())
            .with_stroke(CanvasPaint::solid("#fedcba").unwrap()),
    );
    for (rules, applied, absent, residual) in [
        (vec![solid.clone(), transparent], 1, 1, 0),
        (vec![solid.clone(), clear], 0, 1, 1),
        (vec![mixed], 0, 0, 1),
        (
            vec![solid.clone(), solid.with_variant(ThemeVariant::Active)],
            1,
            1,
            0,
        ),
    ] {
        let theme = class_edge_rules_theme(rules);
        let rendered = try_render_class_svg_with_theme_requirement(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), applied);
        assert_eq!(evidence.not_applicable_count(), absent);
        assert_eq!(evidence.theme_residual_count(), residual);
        assert_eq!(
            try_render_class_svg_with_theme_and_engine(source, &theme, Engine::new()).is_err(),
            residual != 0
        );
    }
}

#[test]
fn class_edge_label_background_selector_has_no_runtime_label_group_consumer() {
    let source = r#"classDiagram
  namespace Alpha {
    class A
  }
  namespace Beta {
    class B
  }
  A --> B : relates
"#;
    for look in ["classic", "neo", "handDrawn"] {
        for html_labels in [true, false] {
            let source = format!("%%{{init: {{\"htmlLabels\": {html_labels}}}}}%%\n{source}");
            let rendered = render_class_svg_from_text_with_engine(
                Engine::new().with_site_config(MermaidConfig::from_value(json!({
                    "look": look,
                    "htmlLabels": html_labels,
                }))),
                &source,
            );
            let document = roxmltree::Document::parse(&rendered).expect("valid Class SVG");
            let groups = document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("g") && node.attribute("class") == Some("edgeLabel")
                })
                .collect::<Vec<_>>();
            assert!(!groups.is_empty(), "look={look}, htmlLabels={html_labels}");
            assert!(
                groups
                    .iter()
                    .all(|node| node.attribute("data-look").is_none())
            );
            let css = document
                .descendants()
                .filter(|node| node.has_tag_name("style"))
                .filter_map(|node| node.text())
                .collect::<String>();
            assert!(css.contains(".edgeLabel[data-look=\"neo\"]"));
            assert!(css.contains(".labelBkg"));
        }
    }
}

#[test]
fn class_background_accounts_for_visible_empty_shadowed_and_source_owned_rules() {
    use merman_render::diagram_theme::OrdinalSelector;
    let source = "classDiagram\nA --> B : first\nB --> C\nC --> D : second\n";
    let rule = |ordinal| {
        let rule = ThemeRule::new(
            ThemeTarget::EdgeLabelBackground,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
        )
        .for_family(DiagramFamilyId::CLASS);
        if let Some(ordinal) = ordinal {
            rule.with_ordinal(OrdinalSelector::Exact(ordinal))
        } else {
            rule
        }
    };
    let compile = |rules: Vec<ThemeRule>| {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    rules
                        .into_iter()
                        .fold(ThemeRuleSet::default(), |set, rule| set.with_rule(rule)),
                ),
            )
            .unwrap()
    };
    for (ordinal, residual, not_applicable) in [(None, 0, 0), (Some(2), 1, 0), (Some(3), 0, 1)] {
        let theme = compile(vec![rule(ordinal)]);
        let rendered = try_render_class_svg_with_theme_requirement(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), usize::from(ordinal.is_none()));
        assert_eq!(evidence.theme_residual_count(), residual);
        assert_eq!(evidence.not_applicable_count(), not_applicable);
        assert_eq!(
            try_render_class_svg_with_theme_and_engine(source, &theme, Engine::new()).is_err(),
            residual != 0
        );
    }
    let theme = compile(vec![rule(None), rule(None)]);
    let rendered = try_render_class_svg_with_theme_requirement(
        source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap();
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.applied_count(), 1);

    let theme = compile(vec![rule(None)]);
    for config in [
        json!({"themeVariables": {"mainBkg": "#654321"}}),
        json!({"themeVariables": {"primaryColor": "#654321"}}),
    ] {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(config));
        let rendered = try_render_class_svg_with_theme_and_engine(source, &theme, engine).unwrap();
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
    // Source ownership of fill must not erase a winning sibling stroke facet.
    let mixed = compile(vec![
        ThemeRule::new(
            ThemeTarget::EdgeLabelBackground,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#b316cd").unwrap())
                .with_stroke(CanvasPaint::solid("#123456").unwrap()),
        )
        .for_family(DiagramFamilyId::CLASS),
    ]);
    let source_owned = Engine::new().with_site_config(MermaidConfig::from_value(
        json!({"themeVariables": {"mainBkg": "#654321"}}),
    ));
    assert!(try_render_class_svg_with_theme_and_engine(source, &mixed, source_owned).is_err());

    #[cfg(feature = "layout-elk")]
    for (ordinal, rejected) in [(2, true), (3, false)] {
        let elk_source = format!("---\nconfig:\n  layout: elk\n---\n{source}");
        let theme = compile(vec![rule(Some(ordinal))]);
        assert_eq!(
            try_render_class_svg_with_theme_and_engine(&elk_source, &theme, Engine::new()).is_err(),
            rejected,
        );
    }
    for source in ["classDiagram\n", "classDiagram\nA --> B\n"] {
        let rendered =
            try_render_class_svg_with_theme_and_engine(source, &theme, Engine::new()).unwrap();
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn class_edge_scalar_evidence_tracks_the_winning_facet_across_rule_grouping() {
    let fill = CanvasPaint::solid("#bb1122").unwrap();
    let stroke = CanvasPaint::solid("#2244bb").unwrap();
    for split in [false, true] {
        for variant in [None, Some(ThemeVariant::Default)] {
            let patch = ThemeStylePatch::default().with_fill(fill.clone());
            let patches = if split {
                vec![
                    patch,
                    ThemeStylePatch::default().with_stroke(stroke.clone()),
                ]
            } else {
                vec![patch.with_stroke(stroke.clone())]
            };
            let theme = class_edge_rules_theme(patches.into_iter().map(|patch| {
                let rule = ThemeRule::new(ThemeTarget::Edge, patch);
                if let Some(variant) = variant {
                    rule.with_variant(variant)
                } else {
                    rule
                }
            }));
            let rendered = try_render_class_svg_with_theme_and_engine(
                "classDiagram\nA *-- B\n",
                &theme,
                Engine::new(),
            )
            .expect("only the winning scalar facet should require a receipt");
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            for path in document
                .descendants()
                .filter(|node| node.attribute("data-edge") == Some("true"))
            {
                let style = path.attribute("style").unwrap();
                assert!(style.contains("stroke:#2244bb !important"));
                assert!(!style.contains("#bb1122"));
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.not_applicable_count(), usize::from(split));
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn class_edge_clear_or_gradient_stroke_blocks_fill_fallback() {
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(0.0, ThemeColorValue::parse("#112233").unwrap()).unwrap(),
            GradientStop::new(1.0, ThemeColorValue::parse("#ddeeff").unwrap()).unwrap(),
        ],
    )
    .unwrap();
    for blocked_stroke in [
        Specified::Clear,
        Specified::Value(CanvasPaint::LinearGradient(gradient)),
    ] {
        let mut stroke = ThemeStylePatch::default();
        stroke.stroke.paint = blocked_stroke;
        let theme = class_edge_rules_theme([
            ThemeRule::new(
                ThemeTarget::Edge,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#bb1122").unwrap()),
            ),
            ThemeRule::new(ThemeTarget::Edge, stroke),
        ]);
        for source in [
            "classDiagram\nA *-- B\nnote for A \"memo\"\n",
            "classDiagram\nclass A\nnote for A \"memo\"\n",
        ] {
            let rendered = try_render_class_svg_with_theme_requirement(
                source,
                &theme,
                Engine::new(),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert!(
                document
                    .descendants()
                    .filter(|node| node.attribute("data-edge") == Some("true"))
                    .all(|node| {
                        node.attribute("style")
                            .is_none_or(|style| !style.contains("#bb1122"))
                    })
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 0);
            assert_eq!(evidence.not_applicable_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 1);
            let error = try_render_class_svg_with_theme_and_engine(source, &theme, Engine::new())
                .err()
                .expect("unsupported winning stroke must fail strict rendering");
            assert_eq!(
                error.unverified_family_theme(),
                Some((DiagramFamilyId::CLASS, 1))
            );
        }
    }
}

#[test]
fn class_edge_ordinal_fill_is_residual_only_when_stroke_does_not_mask_it() {
    for stroke_masks_fill in [false, true] {
        let static_patch = if stroke_masks_fill {
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#123456").unwrap())
        } else {
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap())
        };
        for ordinal in [1, 99] {
            let theme = class_edge_rules_theme([
                ThemeRule::new(ThemeTarget::Edge, static_patch.clone()),
                ThemeRule::new(
                    ThemeTarget::Edge,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#bb1122").unwrap()),
                )
                .with_ordinal(OrdinalSelector::Exact(ordinal)),
            ]);
            let rendered = try_render_class_svg_with_theme_requirement(
                "classDiagram\nA *-- B\nB *-- C\n",
                &theme,
                Engine::new(),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            let residual = usize::from(!stroke_masks_fill && ordinal == 1);
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.theme_residual_count(), residual);
            assert_eq!(evidence.not_applicable_count(), 1 - residual);
        }
    }
}

#[test]
fn class_cluster_scalar_reaches_namespace_rectangles() {
    for (source, extracted) in [
        (
            "classDiagram\nnamespace Internal {\nclass A\nclass B\n}\nA --> B\nclass X\nX --> A\n",
            false,
        ),
        (
            "classDiagram\nnamespace Internal {\nclass A\nclass B\n}\nA --> B\nclass X\n",
            true,
        ),
        (
            "classDiagram\nnamespace Internal {\nclass A\nclass B\n}\nA --> B\nclass X\nclass Y\nX --> Y\n",
            true,
        ),
    ] {
        for fill in [false, true] {
            for variant in [None, Some(ThemeVariant::Default)] {
                for paint in [
                    CanvasPaint::solid("#123456").unwrap(),
                    CanvasPaint::Transparent,
                ] {
                    let css = if paint == CanvasPaint::Transparent {
                        "transparent"
                    } else {
                        "#123456"
                    };
                    let property = if fill { "fill" } else { "stroke" };
                    let patch = if fill {
                        ThemeStylePatch::default().with_fill(paint)
                    } else {
                        ThemeStylePatch::default().with_stroke(paint)
                    };
                    let mut rule = ThemeRule::new(ThemeTarget::Cluster, patch);
                    if let Some(variant) = variant {
                        rule = rule.with_variant(variant);
                    }
                    let theme = class_edge_rules_theme([rule]);
                    for look in ["classic", "handDrawn"] {
                        let rendered = try_render_class_svg_with_theme_and_engine(
                            source,
                            &theme,
                            Engine::new()
                                .with_site_config(MermaidConfig::from_value(json!({"look": look}))),
                        )
                        .expect("typed namespace scalar paint");
                        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                        let rectangles = document
                            .descendants()
                            .filter(|node| {
                                node.has_tag_name("rect")
                                    && node
                                        .parent_element()
                                        .and_then(|parent| parent.attribute("class"))
                                        == Some("cluster undefined")
                            })
                            .collect::<Vec<_>>();
                        assert_eq!(rectangles.len(), 1);
                        assert_eq!(
                            rectangles[0]
                                .ancestors()
                                .any(|ancestor| ancestor.attribute("class") == Some("root")
                                    && ancestor.attribute("transform").is_some()),
                            extracted
                        );
                        assert!(rectangles[0].attribute("style").is_some_and(|style| {
                            style.contains(&format!("{property}:{css} !important"))
                        }));
                        let evidence = merman_render::__private::family_evidence(
                            rendered.into_completion().report(),
                        );
                        assert_eq!(evidence.applied_count(), 1);
                        assert_eq!(evidence.theme_residual_count(), 0);
                    }
                }
            }
        }
    }
}

#[test]
fn class_cluster_scalar_preserves_independent_and_derived_source_ownership() {
    let source = "classDiagram\nnamespace Group {\nclass A\nclass B\n}\nA --> B : relation\nnote for A \"memo\"\n";
    let theme = class_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Cluster,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").unwrap())
            .with_stroke(CanvasPaint::solid("#abcdef").unwrap()),
    )]);
    for (selected, variable, fill_owned, stroke_owned) in [
        ("default", "clusterBkg", true, false),
        ("default", "clusterBorder", false, true),
        ("base", "tertiaryColor", true, true),
        ("base", "tertiaryBorderColor", false, true),
        ("base", "primaryColor", true, true),
        ("base", "secondaryColor", false, false),
        ("base", "mainBkg", false, false),
        ("base", "primaryBorderColor", false, false),
        ("dark", "secondBkg", false, false),
        ("dark", "mainBkg", true, false),
        ("dark", "border2", false, true),
    ] {
        let config = json!({"theme": selected, "themeVariables": { variable: "#456789" }});
        let rendered = try_render_class_svg_with_theme_and_engine(
            source,
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(config)),
        )
        .expect("source-owned namespace paint");
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let rectangle = document
            .descendants()
            .find(|node| {
                node.has_tag_name("rect")
                    && node
                        .parent_element()
                        .and_then(|parent| parent.attribute("class"))
                        == Some("cluster undefined")
            })
            .unwrap();
        let style = rectangle.attribute("style").unwrap_or_default();
        assert_eq!(
            style.contains("fill:#123456 !important"),
            !fill_owned,
            "{selected}/{variable}: {style}"
        );
        assert_eq!(
            style.contains("stroke:#abcdef !important"),
            !stroke_owned,
            "{selected}/{variable}: {style}"
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(
            evidence.applied_count(),
            usize::from(!(fill_owned && stroke_owned))
        );
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn class_cluster_unsupported_winners_require_real_namespace_occurrences() {
    let source = "classDiagram\nnamespace Group {\nclass A\n}\n";
    let mut clear = ThemeStylePatch::default();
    clear.paint.fill = Specified::Clear;
    for rule in [
        ThemeRule::new(ThemeTarget::Cluster, clear),
        ThemeRule::new(
            ThemeTarget::Cluster,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        )
        .with_ordinal(OrdinalSelector::exact(1).unwrap()),
    ] {
        let theme = class_edge_rules_theme([rule]);
        assert!(try_render_class_svg_with_theme_and_engine(source, &theme, Engine::new()).is_err());
        let rendered = try_render_class_svg_with_theme_and_engine(
            "classDiagram\nclass A\n",
            &theme,
            Engine::new(),
        )
        .expect("absent cluster makes the route not applicable");
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.not_applicable_count(), 1);
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_cluster_scalar_reaches_the_shared_namespace_writer() {
    let source = "---\nconfig:\n  layout: elk\n---\nclassDiagram\nnamespace Group {\nclass A\nclass B\n}\nA --> B\n";
    let theme = class_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Cluster,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").unwrap())
            .with_stroke(CanvasPaint::Transparent),
    )]);
    let rendered =
        try_render_class_svg_with_theme_and_engine(source, &theme, Engine::new()).unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let rect = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node
                    .parent_element()
                    .and_then(|parent| parent.attribute("class"))
                    == Some("cluster undefined")
        })
        .unwrap();
    assert_eq!(
        rect.attribute("style"),
        Some("fill:#123456 !important;stroke:transparent !important;")
    );
}

#[test]
fn class_cluster_source_selected_theme_and_mixed_facets_keep_their_own_outcomes() {
    let source = "%%{init: {\"theme\": \"base\", \"themeVariables\": {\"tertiaryColor\": \"#456789\"}}}%%\nclassDiagram\nnamespace Group {\nclass A\n}\n";
    let paint = class_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Cluster,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
    )]);
    let rendered = try_render_class_svg_with_theme_and_engine(
        source,
        &paint,
        legacy_init_theme_compat_engine(),
    )
    .unwrap();
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.not_applicable_count(), 1);

    let source = "classDiagram\nnamespace Group {\nclass A\n}\n";
    let gradient = LinearGradient::new(
        0.0,
        [
            GradientStop::new(0.0, ThemeColorValue::parse("red").unwrap()).unwrap(),
            GradientStop::new(1.0, ThemeColorValue::parse("blue").unwrap()).unwrap(),
        ],
    )
    .unwrap();
    let mut patch = ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
    patch.stroke.paint = Specified::Value(CanvasPaint::LinearGradient(gradient));
    let theme = class_edge_rules_theme([ThemeRule::new(ThemeTarget::Cluster, patch)]);
    let rendered = try_render_class_svg_with_theme_requirement(
        source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap();
    assert!(rendered.svg().contains("fill:#123456 !important;"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);
    assert!(try_render_class_svg_with_theme_and_engine(source, &theme, Engine::new()).is_err());
}

#[test]
fn class_extracted_namespace_receipts_follow_source_relations() {
    let source = "classDiagram\nnamespace Internal {\nclass A\nclass B\n}\nA *-- B : inside\nclass X\nclass Y\nX <|-- Y : outside\n";
    for look in ["classic", "handDrawn"] {
        let engine =
            || Engine::new().with_site_config(MermaidConfig::from_value(json!({"look": look})));
        let theme = class_edge_rules_theme([ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#123456").unwrap()),
        )]);
        let rendered = try_render_class_svg_with_theme_and_engine(source, &theme, engine())
            .expect("both namespace roots must complete relation receipts");
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let relations = document
            .descendants()
            .filter(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
            .collect::<Vec<_>>();
        assert_eq!(relations.len(), 2);
        assert!(relations[0].attribute("data-id").unwrap().contains("X"));
        assert!(relations[1].attribute("data-id").unwrap().contains("A"));
        for relation in &relations {
            assert!(
                relation
                    .attribute("style")
                    .unwrap()
                    .contains("stroke:#123456 !important")
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);

        // Direct label backgrounds still reconcile against all visible source relations.
        for (ordinal, residual) in [(1, 1), (2, 1), (3, 0)] {
            let background = class_edge_rules_theme([ThemeRule::new(
                ThemeTarget::EdgeLabelBackground,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            )
            .with_ordinal(OrdinalSelector::Exact(ordinal))]);
            let rendered = try_render_class_svg_with_theme_requirement(
                source,
                &background,
                engine(),
                ThemePortabilityRequirement::BestEffort,
            )
            .expect("background checkpoints must permit namespace traversal order");
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.theme_residual_count(), residual);
            assert_eq!(evidence.not_applicable_count(), 1 - residual);
            assert_eq!(
                try_render_class_svg_with_theme_and_engine(source, &background, engine()).is_err(),
                residual != 0
            );
        }
    }
}

#[test]
fn class_namespace_title_scalar_has_a_direct_terminal() {
    for source in [
        "classDiagram\nnamespace Internal {\nclass A\nclass B\n}\nA --> B\nclass X\nX --> A\n",
        "classDiagram\nnamespace Internal {\nclass A\nclass B\n}\nA --> B\nclass X\nclass Y\nX --> Y\n",
    ] {
        for variant in [None, Some(ThemeVariant::Default)] {
            for (paint, css) in [
                (CanvasPaint::solid("#123456").unwrap(), "#123456"),
                (CanvasPaint::Transparent, "transparent"),
            ] {
                let mut rule = ThemeRule::new(
                    ThemeTarget::Title,
                    ThemeStylePatch::default().with_fill(paint),
                );
                if let Some(variant) = variant {
                    rule = rule.with_variant(variant);
                }
                let theme = class_edge_rules_theme([rule]);
                for look in ["classic", "neo", "handDrawn"] {
                    let rendered = try_render_class_svg_with_theme_and_engine(
                        source,
                        &theme,
                        Engine::new()
                            .with_site_config(MermaidConfig::from_value(json!({"look":look}))),
                    )
                    .expect("typed Class namespace title scalar paint");
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    let labels = document
                        .descendants()
                        .filter(|node| {
                            node.has_tag_name("span")
                                && node.attribute("class") == Some("nodeLabel")
                                && node.ancestors().any(|parent| {
                                    parent.attribute("class") == Some("cluster-label")
                                })
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(labels.len(), 1);
                    assert_eq!(
                        labels[0].attribute("style"),
                        Some(format!("color:{css} !important;fill:{css} !important;").as_str())
                    );
                    let evidence = merman_render::__private::family_evidence(
                        rendered.into_completion().report(),
                    );
                    assert_eq!(evidence.applied_count(), 1);
                    assert_eq!(evidence.theme_residual_count(), 0);
                    assert_eq!(evidence.compatibility_residual_count(), 0);
                }
            }
        }
    }
}

#[test]
fn class_namespace_title_reconciles_absence_ownership_and_unsupported_siblings() {
    let rule = || {
        ThemeRule::new(
            ThemeTarget::Title,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        )
    };
    let source = "classDiagram\nnamespace Internal {\nclass A\n}\n";
    let theme = class_edge_rules_theme([rule()]);
    for (source, engine) in [
        (
            "---\ntitle: Diagram title\n---\nclassDiagram\nclass A\n",
            Engine::new(),
        ),
        (
            source,
            Engine::new().with_site_config(MermaidConfig::from_value(
                json!({"themeVariables":{"titleColor":"#abcdef"}}),
            )),
        ),
    ] {
        let rendered = try_render_class_svg_with_theme_and_engine(source, &theme, engine).unwrap();
        assert!(!rendered.svg().contains("color:#123456 !important;"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
    for ordinal in [1, 2] {
        let theme = class_edge_rules_theme([
            rule(),
            ThemeRule::new(
                ThemeTarget::Title,
                ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
            )
            .with_ordinal(OrdinalSelector::exact(ordinal).unwrap()),
        ]);
        let rendered = try_render_class_svg_with_theme_requirement(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), usize::from(ordinal == 1));
        assert_eq!(evidence.not_applicable_count(), usize::from(ordinal == 2));
    }
    let theme = class_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Title,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").unwrap())
            .with_stroke(CanvasPaint::solid("#456789").unwrap()),
    )]);
    let rendered = try_render_class_svg_with_theme_requirement(
        source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap();
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn class_namespace_title_keeps_generic_text_author_order() {
    for title_last in [false, true] {
        let title = ThemeRule::new(
            ThemeTarget::Title,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        );
        let text = ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#456789").unwrap()),
        );
        let theme = class_edge_rules_theme(if title_last {
            [text, title]
        } else {
            [title, text]
        });
        let rendered = try_render_class_svg_with_theme_requirement(
            "classDiagram\nnamespace Internal {\nclass A\n}\n",
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        assert_eq!(
            rendered
                .svg()
                .contains("color:#123456 !important;fill:#123456 !important;"),
            title_last
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), usize::from(title_last) + 1);
        assert_eq!(evidence.not_applicable_count(), usize::from(!title_last));
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_namespace_title_uses_the_same_terminal_under_elk() {
    let theme = class_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Title,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
    )]);
    let rendered = try_render_class_svg_with_theme_and_engine(
        "---\nconfig:\n  layout: elk\n---\nclassDiagram\nnamespace Internal {\nclass A\nclass B\n}\nA --> B\n", &theme, Engine::new()).unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(document.descendants().any(|node| node.has_tag_name("span")
        && node.attribute("class") == Some("nodeLabel")
        && node.attribute("style") == Some("color:#123456 !important;fill:#123456 !important;")));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn class_namespace_title_preserves_derived_source_ownership() {
    let source = "classDiagram\nnamespace Internal {\nclass A\n}\n";
    let theme = class_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Title,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
    )]);
    for (selected, variable, owned) in [
        ("default", "textColor", true),
        ("base", "tertiaryTextColor", true),
        ("base", "primaryBorderColor", false),
    ] {
        let rendered = try_render_class_svg_with_theme_and_engine(
            source,
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "theme": selected, "themeVariables": {variable: "#456789"}
            }))),
        )
        .unwrap();
        assert_eq!(rendered.svg().contains("color:#123456 !important;"), !owned);
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), usize::from(!owned));
        assert_eq!(evidence.not_applicable_count(), usize::from(owned));
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn class_namespace_title_clear_retains_only_winning_residuals() {
    let source = "classDiagram\nnamespace Internal {\nclass A\n}\n";
    for title_last in [false, true] {
        let mut clear = ThemeStylePatch::default();
        clear.paint.fill = Specified::Clear;
        let title = ThemeRule::new(ThemeTarget::Title, clear);
        let text = ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#456789").unwrap()),
        );
        let theme = class_edge_rules_theme(if title_last {
            [text, title]
        } else {
            [title, text]
        });
        let rendered = try_render_class_svg_with_theme_requirement(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), usize::from(title_last));
        assert_eq!(evidence.not_applicable_count(), usize::from(!title_last));
        assert_eq!(
            try_render_class_svg_with_theme_and_engine(source, &theme, Engine::new()).is_err(),
            title_last,
        );
    }
}

#[cfg(feature = "math")]
#[test]
fn class_namespace_math_title_cannot_prove_inherited_paint() {
    let theme = class_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Title,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
    )]);
    let source = "classDiagram\nnamespace Formula[\"value: $$x^2$$\"] {\nclass A\n}\n";
    let engine =
        || Engine::new().with_site_config(MermaidConfig::from_value(json!({"htmlLabels":true})));
    let environment = || RenderEnvironment::deterministic().with_compiled_math_renderer();
    let rendered = try_render_class_svg_with_theme_environment(
        source,
        &theme,
        engine(),
        ThemePortabilityRequirement::BestEffort,
        environment(),
    )
    .expect("valid mathematical namespace title");
    assert!(
        rendered
            .svg()
            .contains(r#"data-merman-math-native="unavailable""#)
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 0);
    assert_eq!(
        evidence.status(),
        merman_render::__private::FamilyEvidenceStatus::Incomplete
    );
    let error = match try_render_class_svg_with_theme_environment(
        source,
        &theme,
        engine(),
        ThemePortabilityRequirement::RequirePortable,
        environment(),
    ) {
        Ok(_) => panic!("formula glyph paint is not proven by inherited namespace shell color"),
        Err(error) => error,
    };
    assert_eq!(
        error.incomplete_family_theme(),
        Some((merman_render::DiagramFamilyId::CLASS, 1, 0))
    );
}

#[test]
fn class_node_label_keeps_generic_text_author_order() {
    for html_labels in [false, true] {
        for variant in [None, Some(ThemeVariant::Default)] {
            for label_last in [false, true] {
                let mut label = ThemeRule::new(
                    ThemeTarget::NodeLabel,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                );
                let mut text = ThemeRule::new(
                    ThemeTarget::Text,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#456789").unwrap()),
                );
                if let Some(variant) = variant {
                    label = label.with_variant(variant);
                    text = text.with_variant(variant);
                }
                let theme = class_edge_rules_theme(if label_last {
                    [text, label]
                } else {
                    [label, text]
                });
                let rendered = try_render_class_svg_with_theme_requirement(
                    "classDiagram\nclass Account {\n+String owner\n+close()\n}\n",
                    &theme,
                    Engine::new().with_site_config(MermaidConfig::from_value(json!({
                        "htmlLabels": html_labels,
                    }))),
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let node = document
                    .descendants()
                    .find(|node| {
                        node.attribute("id")
                            .is_some_and(|id| id.starts_with("merman-classId-Account-"))
                    })
                    .unwrap();
                assert_eq!(
                    node.descendants().any(|node| node
                        .attribute("style")
                        .is_some_and(|style| style.contains("#123456"))),
                    label_last,
                    "NodeLabel and Text must share author order: html={html_labels}, variant={variant:?}, label_last={label_last}"
                );
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.applied_count(), 1);
                assert_eq!(evidence.not_applicable_count(), 1);
            }
        }
    }
}

#[test]
fn class_node_label_generic_text_ordinal_blocks_only_its_occurrence() {
    let theme = class_edge_rules_theme([
        ThemeRule::new(
            ThemeTarget::NodeLabel,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#456789").unwrap()),
        )
        .with_ordinal(OrdinalSelector::exact(1).unwrap()),
    ]);
    let source = "classDiagram\nclass Alpha\nclass Beta\n";
    for html_labels in [false, true] {
        let engine = Engine::new()
            .with_site_config(MermaidConfig::from_value(json!({"htmlLabels":html_labels})));
        let rendered = try_render_class_svg_with_theme_requirement(
            source,
            &theme,
            engine,
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for (id, expected) in [("Alpha", false), ("Beta", true)] {
            let prefix = format!("merman-classId-{id}-");
            let node = document
                .descendants()
                .find(|node| {
                    node.attribute("id")
                        .is_some_and(|id| id.starts_with(&prefix))
                })
                .unwrap();
            assert_eq!(
                node.descendants().any(|node| node
                    .attribute("style")
                    .is_some_and(|style| style.contains("#123456"))),
                expected,
                "generic Text ordinal must mask NodeLabel at its own occurrence: {id}, html={html_labels}"
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        // Generic Text ordinal is not yet covered by the Class terminal ledger.
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(
            evidence.status(),
            merman_render::__private::FamilyEvidenceStatus::Incomplete
        );
        let error = try_render_class_svg_with_theme_requirement(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::RequirePortable,
        )
        .err()
        .expect("unaccounted Text ordinal must reject strict rendering");
        assert_eq!(
            error.incomplete_family_theme(),
            Some((DiagramFamilyId::CLASS, 2, 1))
        );
    }
}

#[test]
fn class_node_label_generic_text_clear_and_transparent_mask_older_fill() {
    for fill in [Specified::Clear, Specified::Value(CanvasPaint::Transparent)] {
        for html_labels in [false, true] {
            let mut patch = ThemeStylePatch::default();
            patch.paint.fill = fill.clone();
            let theme = class_edge_rules_theme([
                ThemeRule::new(
                    ThemeTarget::NodeLabel,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                ),
                ThemeRule::new(ThemeTarget::Text, patch),
            ]);
            let rendered = try_render_class_svg_with_theme_requirement(
                "classDiagram\nclass Account\n",
                &theme,
                Engine::new()
                    .with_site_config(MermaidConfig::from_value(json!({"htmlLabels":html_labels}))),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let node = document
                .descendants()
                .find(|node| {
                    node.attribute("id")
                        .is_some_and(|id| id.starts_with("merman-classId-Account-"))
                })
                .unwrap();
            assert!(
                !node.descendants().any(|node| node
                    .attribute("style")
                    .is_some_and(|style| style.contains("#123456"))),
                "Text clear/transparent must mask an older NodeLabel fill: {fill:?}, html={html_labels}"
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(
                evidence.applied_count(),
                usize::from(matches!(fill, Specified::Value(CanvasPaint::Transparent))),
            );
            assert_eq!(evidence.not_applicable_count(), 1);
        }
    }
}

#[test]
fn class_node_label_generic_text_precedence_preserves_source_paint() {
    for html_labels in [false, true] {
        for label_last in [false, true] {
            let label = ThemeRule::new(
                ThemeTarget::NodeLabel,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            );
            let text = ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#456789").unwrap()),
            );
            let theme = class_edge_rules_theme(if label_last {
                [text, label]
            } else {
                [label, text]
            });
            let rendered = try_render_class_svg_with_theme_requirement(
                "classDiagram\nclass Account\nstyle Account color:#cc0000\n",
                &theme,
                Engine::new()
                    .with_site_config(MermaidConfig::from_value(json!({"htmlLabels":html_labels}))),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let node = document
                .descendants()
                .find(|node| {
                    node.attribute("id")
                        .is_some_and(|id| id.starts_with("merman-classId-Account-"))
                })
                .unwrap();
            assert!(node.descendants().any(|node| {
                node.attribute("style")
                    .is_some_and(|style| style.contains("#cc0000"))
            }));
            assert!(!node.descendants().any(|node| {
                node.attribute("style")
                    .is_some_and(|style| style.contains("#123456") || style.contains("#456789"))
            }));
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 0);
            assert_eq!(evidence.not_applicable_count(), 2);
        }
    }
}

#[test]
fn class_generic_text_fill_has_a_terminal_in_every_text_channel() {
    let source = r#"---
title: Account registry
---
classDiagram
namespace Internal {
  class Account {
    +String owner
    +close()
  }
  class Ledger
}
Account "1" --> "many" Ledger : owns
note for Account "Audit note"
"#;
    for html_labels in [false, true] {
        let theme = class_edge_rules_theme([ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        )]);
        let rendered = try_render_class_svg_with_theme_requirement(
            source,
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "htmlLabels": html_labels,
            }))),
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("generic Class Text must bind every actual text writer");
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let has_paint = |node: roxmltree::Node<'_, '_>| {
            node.descendants()
                .filter(|terminal| {
                    terminal.is_text() && !terminal.text().unwrap().trim().is_empty()
                })
                .all(|terminal| {
                    ["color", "fill"].into_iter().all(|property| {
                        terminal
                            .ancestors()
                            .take_while(|ancestor| Some(*ancestor) != node.parent())
                            .filter_map(|ancestor| ancestor.attribute("style"))
                            .find_map(|style| {
                                style
                                    .split(';')
                                    .filter_map(|declaration| {
                                        let (name, value) = declaration.split_once(':')?;
                                        (name.trim() == property).then_some(
                                            value.trim().trim_end_matches("!important").trim(),
                                        )
                                    })
                                    .next_back()
                            })
                            == Some("#123456")
                    })
                })
        };
        for id in ["Account", "Ledger"] {
            let prefix = format!("merman-classId-{id}-");
            let node = document
                .descendants()
                .find(|node| {
                    node.attribute("id")
                        .is_some_and(|id| id.starts_with(&prefix))
                })
                .unwrap();
            assert!(
                has_paint(node),
                "missing Text paint for {id}, html={html_labels}"
            );
        }
        for class in [
            "cluster-label",
            "edgeLabel",
            "edgeTerminals",
            "noteLabel",
            "classDiagramTitleText",
        ] {
            let terminals = document
                .descendants()
                .filter(|node| {
                    node.is_element()
                        && node.attribute("class").is_some_and(|classes| {
                            classes.split_whitespace().any(|value| value == class)
                        })
                })
                .collect::<Vec<_>>();
            assert!(!terminals.is_empty(), "missing {class} terminal");
            assert!(
                terminals.iter().copied().all(has_paint),
                "Text paint must reach each {class} terminal, html={html_labels}"
            );
        }
        let account = document
            .descendants()
            .find(|node| {
                node.attribute("id")
                    .is_some_and(|id| id.starts_with("merman-classId-Account-"))
            })
            .unwrap();
        let paint_owners = account
            .descendants()
            .filter(|node| {
                node.attribute("style")
                    .is_some_and(|style| style.contains("color:#123456"))
            })
            .collect::<Vec<_>>();
        assert!(
            paint_owners.len() >= 3,
            "title, member and method paint owners"
        );
        for paint_owner in paint_owners {
            for replacement in ["", "color:#123456;fill:#000000"] {
                let original = &rendered.svg()[paint_owner.range()];
                let corrupted =
                    original.replacen(paint_owner.attribute("style").unwrap(), replacement, 1);
                let mut mutated = rendered.svg().to_string();
                mutated.replace_range(paint_owner.range(), &corrupted);
                let mutated = roxmltree::Document::parse(&mutated).unwrap();
                let account = mutated
                    .descendants()
                    .find(|node| {
                        node.attribute("id")
                            .is_some_and(|id| id.starts_with("merman-classId-Account-"))
                    })
                    .unwrap();
                assert!(
                    !has_paint(account),
                    "a missing or wrong terminal paint must fail, html={html_labels}"
                );
            }
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.accounted_count(), evidence.required_count());
        assert_eq!(evidence.compatibility_residual_count(), 0);
    }
}

#[test]
fn class_generic_text_shadowed_clear_does_not_block_portability() {
    for variant in [None, Some(ThemeVariant::Default)] {
        let mut clear = ThemeStylePatch::default();
        clear.paint.fill = Specified::Clear;
        let mut earlier = ThemeRule::new(ThemeTarget::Text, clear);
        let mut winner = ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        );
        if let Some(variant) = variant {
            earlier = earlier.with_variant(variant);
            winner = winner.with_variant(variant);
        }
        let theme = class_edge_rules_theme([earlier, winner]);
        let rendered = try_render_class_svg_with_theme_and_engine(
            "classDiagram\nclass Account\n",
            &theme,
            Engine::new(),
        )
        .expect("a fully shadowed Text Clear has no residual consumer");
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.accounted_count(), evidence.required_count());
    }
}

#[test]
fn class_generic_text_shadowing_preserves_winning_sibling_facets() {
    for replace_stroke in [false, true] {
        let mut earlier = ThemeStylePatch::default();
        earlier.paint.fill = Specified::Clear;
        earlier.stroke.paint = Specified::Clear;
        let mut later =
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
        if replace_stroke {
            later.stroke.paint = Specified::Value(CanvasPaint::solid("#654321").unwrap());
        }
        let theme = class_edge_rules_theme([
            ThemeRule::new(ThemeTarget::Text, earlier),
            ThemeRule::new(ThemeTarget::Text, later),
        ]);
        let source = "classDiagram\nclass Account\n";
        let rendered = try_render_class_svg_with_theme_requirement(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        // A winning Text stroke remains unsupported, whichever rule owns it.
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.not_applicable_count(), usize::from(replace_stroke));
        assert_eq!(evidence.applied_count(), usize::from(!replace_stroke));
        assert!(try_render_class_svg_with_theme_and_engine(source, &theme, Engine::new()).is_err());
    }
}

#[test]
fn class_typed_text_output_failure_cannot_return_a_completed_receipt() {
    let source = "---\ntitle: Registry\n---\nclassDiagram\nA \"1\" --> \"many\" B : owns\nnote for A \"Audit\"\n";
    let theme = class_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
    )]);
    let render = |limit| {
        let mut policy = RenderResourcePolicy::unbounded_for_trusted_input();
        if let Some(limit) = limit {
            policy = policy
                .with_limit(ResourceLimitId::MaxSvgBytes, limit)
                .unwrap();
        }
        try_render_class_svg_with_theme_environment(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::RequirePortable,
            RenderEnvironment::deterministic().with_resource_policy(policy),
        )
    };
    let baseline = render(None).unwrap();
    let exact = baseline.svg().len();
    assert_eq!(render(Some(exact)).unwrap().svg(), baseline.svg());
    for limit in [1, exact - 1] {
        let error = match render(Some(limit)) {
            Ok(_) => panic!("failed output cannot expose an Applied receipt"),
            Err(error) => error,
        };
        assert!(
            matches!(error, merman_render::Error::ResourceLimitExceeded(_)),
            "{error}"
        );
    }
}

#[test]
fn class_generic_text_partially_shadowed_rule_keeps_its_winning_fill() {
    for variant in [None, Some(ThemeVariant::Default)] {
        for html_labels in [false, true] {
            for source_owned in [false, true] {
                let mut mixed =
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
                mixed.stroke.paint = Specified::Clear;
                let mut stroke = ThemeStylePatch::default();
                stroke.stroke.paint = Specified::Value(CanvasPaint::solid("#654321").unwrap());
                let rule = |patch| {
                    let rule = ThemeRule::new(ThemeTarget::Text, patch);
                    match variant {
                        Some(variant) => rule.with_variant(variant),
                        None => rule,
                    }
                };
                let theme = class_edge_rules_theme([rule(mixed), rule(stroke)]);
                let source = if source_owned {
                    "classDiagram\nclass Account\nstyle Account color:#cc0000\n"
                } else {
                    "classDiagram\nclass Account\n"
                };
                let engine = || {
                    Engine::new().with_site_config(MermaidConfig::from_value(
                        json!({"htmlLabels": html_labels}),
                    ))
                };
                let rendered = try_render_class_svg_with_theme_requirement(
                    source,
                    &theme,
                    engine(),
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let account = document
                    .descendants()
                    .find(|node| {
                        node.attribute("id")
                            .is_some_and(|id| id.starts_with("merman-classId-Account-"))
                    })
                    .unwrap();
                let has_color = |color: &str| {
                    account.descendants().any(|node| {
                        node.attribute("style")
                            .is_some_and(|style| style.contains(color))
                    })
                };
                assert_eq!(has_color("#123456"), !source_owned);
                assert_eq!(has_color("#cc0000"), source_owned);
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.required_count(), 2);
                assert_eq!(
                    evidence.accounted_count(),
                    1,
                    "variant={variant:?}, html={html_labels}, source={source_owned}"
                );
                assert_eq!(evidence.applied_count(), usize::from(!source_owned));
                assert_eq!(evidence.not_applicable_count(), usize::from(source_owned));
                let error = try_render_class_svg_with_theme_and_engine(source, &theme, engine())
                    .err()
                    .unwrap();
                assert_eq!(
                    error.incomplete_family_theme(),
                    Some((DiagramFamilyId::CLASS, 2, 1))
                );
            }
        }
    }
}
