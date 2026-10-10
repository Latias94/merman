mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, GradientStop,
    LinearGradient, OrdinalPalette, OrdinalSelector, PatternKind, PatternSpec, Specified,
    ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec, materialize_theme,
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
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use merman_theme_contract::{ThemeColorTokenV1, ThemeDefinitionV1, ThemeTokensV1};
use regex::Regex;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn edge_labels_group(svg: &str) -> &str {
    let start = svg
        .find(r#"<g class="edgeLabels">"#)
        .expect("expected edgeLabels group");
    let end = svg[start..]
        .find(r#"<g class="nodes">"#)
        .map(|idx| start + idx)
        .expect("expected nodes group after edgeLabels");
    &svg[start..end]
}

fn render_er_svg_from_text(text: &str, options: &SvgRenderOptions) -> String {
    render_er_svg_from_text_with_engine(text, options, Engine::new())
}

fn render_er_svg_from_text_with_engine(
    text: &str,
    options: &SvgRenderOptions,
    engine: Engine,
) -> String {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");

    artifact
        .render_svg(options, &SvgDebugOptions::default())
        .expect("render svg")
        .svg()
        .to_owned()
}

fn try_render_er_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin ER resource-bound session");
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ER resource-bound fixture")
        .expect("detect ER resource-bound fixture");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

fn er_entity_paint_theme(fill: CanvasPaint, stroke: Option<CanvasPaint>) -> DiagramTheme {
    let mut style = ThemeStylePatch::default().with_fill(fill);
    if let Some(stroke) = stroke {
        style = style.with_stroke(stroke);
    }
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Entity, style)),
        ))
        .expect("compile ER entity fill theme")
}

fn er_relation_stroke_theme(stroke: CanvasPaint) -> DiagramTheme {
    er_relation_rules_theme([ThemeRule::new(
        ThemeTarget::Relation,
        ThemeStylePatch::default().with_stroke(stroke),
    )])
}

fn er_relation_rules_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    let styles = rules
        .into_iter()
        .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule);
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile ER relation stroke theme")
}

fn er_visible_text_theme() -> DiagramTheme {
    let text = ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#112233").expect("valid ER text fill")),
    );
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(text)))
        .expect("compile ER visible text theme")
}

fn er_font_size_theme(font_size_px: f32) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    merman_render::DiagramFamilyId::ER,
                    ThemeTextStyle::default()
                        .with_font_size_px(font_size_px)
                        .expect("valid ER typed font size"),
                ),
            ),
        )
        .expect("compile ER FontSize theme")
}

fn er_table_row_theme() -> DiagramTheme {
    let odd_row = ThemeRule::new(
        ThemeTarget::Table,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#aabbcc").expect("valid ER odd row fill")),
    )
    .with_variant(ThemeVariant::Odd);
    let even_row = ThemeRule::new(
        ThemeTarget::Table,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#ddeeff").expect("valid ER even row fill")),
    )
    .with_variant(ThemeVariant::Even);
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                [odd_row, even_row]
                    .into_iter()
                    .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule),
            ),
        )
        .expect("compile ER table row theme")
}

fn relationship_path_tags(svg: &str) -> Vec<&str> {
    Regex::new(r#"<path[^>]*class="[^"]*relationshipLine[^"]*"[^>]*/>"#)
        .expect("ER relationship path regex")
        .find_iter(svg)
        .map(|entry| entry.as_str())
        .collect()
}

fn referenced_marker_ids(path_tags: &[&str]) -> Vec<String> {
    let marker = Regex::new(r#"url\(#([^)]+)\)"#).expect("ER marker reference regex");
    path_tags
        .iter()
        .flat_map(|path| marker.captures_iter(path))
        .map(|captures| captures[1].to_string())
        .collect()
}

fn marker_opening_tag<'a>(svg: &'a str, marker_id: &str) -> &'a str {
    let needle = format!(r#"<marker id="{marker_id}""#);
    let start = svg
        .find(&needle)
        .unwrap_or_else(|| panic!("missing ER marker {marker_id}: {svg}"));
    let end = svg[start..]
        .find('>')
        .map(|offset| start + offset + 1)
        .expect("ER marker opening tag end");
    &svg[start..end]
}

fn prepare_er_family_with_theme_and_engine(
    text: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> family::FamilyRenderArtifact {
    try_prepare_er_family_with_theme_and_engine(text, theme, engine)
        .expect("prepare themed ER artifact")
}

fn try_prepare_er_family_with_theme_and_engine(
    text: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> merman_render::Result<family::FamilyRenderArtifact> {
    try_prepare_er_family_with_theme_and_engine_requirement(
        text,
        theme,
        engine,
        ThemePortabilityRequirement::RequirePortable,
    )
}

fn try_prepare_er_family_with_theme_and_engine_requirement(
    text: &str,
    theme: &DiagramTheme,
    engine: Engine,
    requirement: ThemePortabilityRequirement,
) -> merman_render::Result<family::FamilyRenderArtifact> {
    try_prepare_er_family_with_theme_and_engine_requirement_and_environment(
        text,
        theme,
        engine,
        requirement,
        RenderEnvironment::deterministic(),
    )
}

fn try_prepare_er_family_with_theme_and_engine_requirement_and_environment(
    text: &str,
    theme: &DiagramTheme,
    engine: Engine,
    requirement: ThemePortabilityRequirement,
    environment: RenderEnvironment,
) -> merman_render::Result<family::FamilyRenderArtifact> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(text, ParseOptions::strict())
        .expect("parse themed ER diagram")
        .expect("detect themed ER diagram");
    let session = environment
        .with_theme_portability_requirement(requirement)
        .begin_session_with_theme(theme)
        .expect("begin ER session");
    family::prepare(parsed, &LayoutOptions::default(), session)
}

fn render_er_svg_from_text_with_theme(text: &str, theme: &DiagramTheme) -> String {
    prepare_er_family_with_theme_and_engine(text, theme, Engine::new())
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render themed ER SVG")
        .svg()
        .to_owned()
}

fn render_er_svg_from_text_with_theme_and_engine(
    text: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> String {
    prepare_er_family_with_theme_and_engine(text, theme, engine)
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render themed ER SVG")
        .svg()
        .to_owned()
}

#[test]
fn er_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = "erDiagram\n  CUSTOMER ||--o{ ORDER : places\n";
    let baseline = try_render_er_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded ER baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "ER fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact ER SVG byte ceiling");
    let exact = try_render_er_svg_with_resource_policy(source, exact_policy)
        .expect("the exact ER family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact ER SVG byte ceiling");
    let error = try_render_er_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the ER family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected ER MaxSvgBytes rejection, got {error}");
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

fn entity_transform(svg: &str, entity_id: &str) -> (f64, f64) {
    let re = Regex::new(&format!(
        r#"id="merman-{}"[^>]*transform="translate\(([^,]+), ([^)]+)\)""#,
        regex::escape(entity_id)
    ))
    .expect("entity transform regex");
    let captures = re
        .captures(svg)
        .unwrap_or_else(|| panic!("missing transform for {entity_id}: {svg}"));
    (
        captures[1].parse().expect("entity x"),
        captures[2].parse().expect("entity y"),
    )
}

fn root_view_box(svg: &str) -> [f64; 4] {
    let re = Regex::new(r#"\bviewBox="([^\"]+)""#).expect("viewBox regex");
    let captures = re.captures(svg).expect("root viewBox");
    let values = captures[1]
        .split_ascii_whitespace()
        .map(|value| value.parse::<f64>().expect("viewBox number"))
        .collect::<Vec<_>>();
    values.try_into().expect("four viewBox numbers")
}

#[derive(Debug)]
struct ErTypedFontSizeProbeMeasurer {
    typed_size_px: f64,
    typed_size_calls: Arc<AtomicUsize>,
    entity_size_calls: Arc<AtomicUsize>,
    attribute_size_calls: Arc<AtomicUsize>,
    relationship_text_size_calls: Arc<AtomicUsize>,
    relationship_size_calls: Arc<AtomicUsize>,
    root_size_calls: Arc<AtomicUsize>,
    unexpected_size_calls: Arc<AtomicUsize>,
}

impl TextMeasurer for ErTypedFontSizeProbeMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        let uses_typed_size = (style.font_size - self.typed_size_px).abs() < f64::EPSILON;
        let uses_relationship_size = (style.font_size - 14.0).abs() < f64::EPSILON;
        if uses_typed_size {
            self.typed_size_calls.fetch_add(1, Ordering::Relaxed);
        } else if uses_relationship_size {
            self.relationship_size_calls.fetch_add(1, Ordering::Relaxed);
        } else if (style.font_size - 10.0).abs() < f64::EPSILON {
            self.root_size_calls.fetch_add(1, Ordering::Relaxed);
        } else {
            self.unexpected_size_calls.fetch_add(1, Ordering::Relaxed);
        }
        match text {
            "CUSTOMER" | "ORDER" if uses_typed_size => {
                self.entity_size_calls.fetch_add(1, Ordering::Relaxed);
            }
            "string" | "id" | "PK" if uses_typed_size => {
                self.attribute_size_calls.fetch_add(1, Ordering::Relaxed);
            }
            "owns" if uses_relationship_size => {
                self.relationship_text_size_calls
                    .fetch_add(1, Ordering::Relaxed);
            }
            "CUSTOMER" | "ORDER" | "string" | "id" | "PK" | "owns" => {
                self.unexpected_size_calls.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        TextMetrics {
            width: (text.chars().count() as f64 * style.font_size * 0.55).max(1.0),
            height: style.font_size,
            line_count: 1,
        }
    }
}

fn render_er_typed_font_size_probe(
    engine: Engine,
    profile_id: &'static str,
) -> family::RenderedFamilySvg {
    render_er_typed_font_size_probe_with_size(engine, profile_id, 24.0)
}

fn render_er_typed_font_size_probe_with_size(
    engine: Engine,
    profile_id: &'static str,
    typed_size_px: f32,
) -> family::RenderedFamilySvg {
    let typed_size_calls = Arc::new(AtomicUsize::new(0));
    let entity_size_calls = Arc::new(AtomicUsize::new(0));
    let attribute_size_calls = Arc::new(AtomicUsize::new(0));
    let relationship_text_size_calls = Arc::new(AtomicUsize::new(0));
    let relationship_size_calls = Arc::new(AtomicUsize::new(0));
    let root_size_calls = Arc::new(AtomicUsize::new(0));
    let unexpected_size_calls = Arc::new(AtomicUsize::new(0));
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new(profile_id).expect("valid ER probe profile id"),
        "test",
    )
    .expect("valid ER probe profile identity");
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(ErTypedFontSizeProbeMeasurer {
                typed_size_px: f64::from(typed_size_px),
                typed_size_calls: Arc::clone(&typed_size_calls),
                entity_size_calls: Arc::clone(&entity_size_calls),
                attribute_size_calls: Arc::clone(&attribute_size_calls),
                relationship_text_size_calls: Arc::clone(&relationship_text_size_calls),
                relationship_size_calls: Arc::clone(&relationship_size_calls),
                root_size_calls: Arc::clone(&root_size_calls),
                unexpected_size_calls: Arc::clone(&unexpected_size_calls),
            }),
        )),
    );
    let theme = er_font_size_theme(typed_size_px);
    let source = r#"erDiagram
  CUSTOMER {
    string id PK
  }
  CUSTOMER ||--o{ ORDER : owns
"#;
    let artifact = try_prepare_er_family_with_theme_and_engine_requirement_and_environment(
        source,
        &theme,
        engine,
        ThemePortabilityRequirement::RequirePortable,
        environment,
    )
    .expect("prepare ER typed FontSize probe");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render ER typed FontSize probe");

    assert!(
        typed_size_calls.load(Ordering::Relaxed) > 0,
        "entity and attribute measurement must consume the typed size"
    );
    assert!(
        entity_size_calls.load(Ordering::Relaxed) > 0,
        "entity-name measurement must consume the typed size"
    );
    assert!(
        attribute_size_calls.load(Ordering::Relaxed) > 0,
        "attribute-cell measurement must consume the typed size"
    );
    assert!(
        relationship_size_calls.load(Ordering::Relaxed) > 0,
        "relationship-label measurement must retain Mermaid's role-local 14px size"
    );
    assert!(
        relationship_text_size_calls.load(Ordering::Relaxed) > 0,
        "the relationship label must retain the role-local 14px size"
    );
    assert_eq!(
        root_size_calls.load(Ordering::Relaxed),
        0,
        "root fontSize is ER's fallback and must not overwrite an active typed size"
    );
    assert_eq!(unexpected_size_calls.load(Ordering::Relaxed), 0);
    rendered
}

fn assert_er_typed_font_size_output(rendered: family::RenderedFamilySvg) {
    let css = rendered.svg();
    assert!(
        Regex::new(r#"#merman\{[^}]*font-size:24px;"#)
            .expect("ER root font-size regex")
            .is_match(css),
        "typed ER FontSize must reach root CSS: {css}"
    );
    assert!(
        Regex::new(r#"#merman svg\{[^}]*font-size:24px;"#)
            .expect("ER nested SVG font-size regex")
            .is_match(css),
        "typed ER FontSize must reach nested-SVG CSS: {css}"
    );
    assert!(
        Regex::new(r#"#merman \.edgeLabel \.label\{[^}]*font-size:14px;"#)
            .expect("ER relationship-label font-size regex")
            .is_match(css),
        "typed ER FontSize must not replace the relationship-label 14px owner: {css}"
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

fn first_er_entity_rect_size(svg: &str) -> (f64, f64) {
    let document = roxmltree::Document::parse(svg).expect("valid ER SVG");
    let rect = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|token| token == "label-container")
                })
        })
        .expect("ER entity shell rectangle");
    (
        rect.attribute("width")
            .expect("ER entity width")
            .parse()
            .expect("ER entity width number"),
        rect.attribute("height")
            .expect("ER entity height")
            .parse()
            .expect("ER entity height number"),
    )
}

fn er_entity_rect_size(svg: &str, entity_id: &str) -> (f64, f64) {
    let document = roxmltree::Document::parse(svg).expect("valid ER SVG");
    let entity = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("id") == Some(entity_id))
        .unwrap_or_else(|| panic!("missing ER entity {entity_id}: {svg}"));
    let rect = entity
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|token| token == "label-container")
                })
        })
        .unwrap_or_else(|| panic!("missing shell rectangle for ER entity {entity_id}: {svg}"));
    (
        rect.attribute("width")
            .expect("ER entity width")
            .parse()
            .expect("ER entity width number"),
        rect.attribute("height")
            .expect("ER entity height")
            .parse()
            .expect("ER entity height number"),
    )
}

fn er_relationship_label_background_size(svg: &str, label_text: &str) -> (f64, f64) {
    let document = roxmltree::Document::parse(svg).expect("valid ER SVG");
    let label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class") == Some("edgeLabel")
                && node
                    .descendants()
                    .filter_map(|descendant| descendant.text())
                    .any(|text| text == label_text)
        })
        .unwrap_or_else(|| panic!("missing ER relationship label {label_text}: {svg}"));
    let background = label
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("background"))
        .unwrap_or_else(|| panic!("missing ER relationship background for {label_text}: {svg}"));
    (
        background
            .attribute("width")
            .expect("ER relationship label width")
            .parse()
            .expect("ER relationship label width number"),
        background
            .attribute("height")
            .expect("ER relationship label height")
            .parse()
            .expect("ER relationship label height number"),
    )
}

#[test]
fn er_svg_renders_entities_and_relationships() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join("upstream_attributes_styles_classes.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let svg = render_er_svg_from_text(&text, &SvgRenderOptions::default());

    assert!(svg.contains(r#"id="merman-entity-BOOK-0""#));
    assert!(svg.contains(r#"data-look="neo""#));
    assert!(svg.contains(r#"id="merman-id_entity-BOOK-0_entity-PAGE-1_0""#));
    assert!(svg.contains(r#"id="merman-drop-shadow""#));
    assert!(svg.contains("relationshipLine"));
    let edge_style_pattern = if cfg!(feature = "layout-elk") {
        r#"<path[^>]*class="[^"]*relationshipLine[^"]*" style="stroke-dasharray: [^"]*; stroke-dashoffset: 0;fill:none;;;fill:none"[^>]*>"#
    } else {
        r#"<path[^>]*class="[^"]*relationshipLine[^"]*" style="stroke-dasharray: [^"]*; stroke-dashoffset: 0;undefined;;;undefined"[^>]*>"#
    };
    assert!(
        Regex::new(edge_style_pattern)
            .expect("relationship path regex")
            .is_match(&svg),
        "relationship paths should preserve the selected renderer's pathStyle serialization"
    );
    assert!(svg.contains("relationshipLabelBox"));
    assert!(
        svg.contains("marker") && svg.contains("merman_er-zeroOrMoreStart"),
        "expected Mermaid-like marker ids"
    );
    assert!(
        {
            let path_re = Regex::new(r#"<path[^>]*relationshipLine[^>]*>"#).expect("regex");
            let d_re = Regex::new(r#"\bd="([^"]*)"#).expect("regex");
            path_re.find_iter(&svg).any(|m| {
                let d = &d_re.captures(m.as_str()).expect("path data")[1];
                if cfg!(feature = "layout-elk") {
                    d.contains('L') && !d.contains('C')
                } else {
                    d.contains('C')
                }
            })
        },
        "expected ELK rounded segments or Dagre basis curves in relationship paths"
    );
    assert!(
        svg.contains("color: rgb(255, 255, 255) !important;"),
        "expected classDef text color to use the ER HTML label CSSOM path"
    );
}

#[test]
fn er_svg_recursive_relationship_is_one_logical_edge_and_one_label() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join(
            "upstream_cypress_erdiagram_spec_should_render_an_er_diagram_with_a_recursive_relationship_002.mmd",
        );
    let text = std::fs::read_to_string(path).expect("fixture");

    let svg = render_er_svg_from_text(&text, &SvgRenderOptions::default());

    assert!(svg.contains(r#"data-id="id_entity-CUSTOMER-0_entity-CUSTOMER-0_0""#));
    assert!(!svg.contains("cyclic-special"));
    assert_eq!(
        svg.matches(">refers<").count(),
        1,
        "the logical recursive relationship should own one relationship label: {svg}"
    );
}

#[test]
fn er_svg_uses_configured_look_in_dom_attributes() {
    let text = r#"%%{init: {"look": "neo"}}%%
erDiagram
  CUSTOMER ||--o{ ORDER : places
"#;

    let svg = render_er_svg_from_text_with_engine(
        text,
        &SvgRenderOptions::default(),
        legacy_init_theme_compat_engine(),
    );

    assert!(
        svg.contains(r#"data-look="neo""#),
        "expected ER SVG to propagate configured look: {svg}"
    );
    assert!(
        !svg.contains(r#"data-look="classic""#),
        "configured ER look must not leave classic DOM attributes: {svg}"
    );
}

#[test]
fn er_svg_neo_markers_use_neo_geometry_units_and_theme_values() {
    let text = r##"%%{init: {"look": "neo", "themeVariables": {"mainBkg": "#fedcba", "strokeWidth": 3.5}}}%%
erDiagram
  CUSTOMER ||--o{ ORDER : places
"##;

    let svg = render_er_svg_from_text_with_engine(
        text,
        &SvgRenderOptions::default(),
        legacy_init_theme_compat_engine(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Neo ER SVG");

    let zero_or_one_start = document
        .descendants()
        .find(|node| {
            node.has_tag_name("marker") && node.attribute("id") == Some("merman_er-zeroOrOneStart")
        })
        .expect("expected Neo zeroOrOneStart marker");
    assert_eq!(
        zero_or_one_start.attribute("markerUnits"),
        Some("userSpaceOnUse")
    );
    let circle = zero_or_one_start
        .children()
        .find(|node| node.has_tag_name("circle"))
        .expect("expected Neo zeroOrOneStart circle");
    assert_eq!(circle.attribute("fill"), Some("#fedcba"));
    assert_eq!(circle.attribute("cx"), Some("21"));
    assert_eq!(circle.attribute("stroke-width"), Some("3.5"));
    let path = zero_or_one_start
        .children()
        .find(|node| node.has_tag_name("path"))
        .expect("expected Neo zeroOrOneStart path");
    assert_eq!(path.attribute("d"), Some("M9,0 L9,18"));
    assert_eq!(path.attribute("stroke-width"), Some("3.5"));

    let zero_or_more_start = document
        .descendants()
        .find(|node| {
            node.has_tag_name("marker") && node.attribute("id") == Some("merman_er-zeroOrMoreStart")
        })
        .expect("expected Neo zeroOrMoreStart marker");
    assert_eq!(
        zero_or_more_start.attribute("markerUnits"),
        Some("userSpaceOnUse")
    );
    let circle = zero_or_more_start
        .children()
        .find(|node| node.has_tag_name("circle"))
        .expect("expected Neo zeroOrMoreStart circle");
    assert_eq!(circle.attribute("fill"), Some("#fedcba"));
    assert_eq!(circle.attribute("cx"), Some("45.5"));
    assert_eq!(circle.attribute("stroke-width"), Some("3.5"));
}

#[test]
fn er_svg_classic_markers_keep_classic_geometry_and_styling() {
    let text = r##"%%{init: {"look": "classic", "themeVariables": {"mainBkg": "#fedcba", "strokeWidth": 3.5}}}%%
erDiagram
  CUSTOMER ||--o{ ORDER : places
"##;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());
    let document = roxmltree::Document::parse(&svg).expect("valid classic ER SVG");

    let zero_or_one_start = document
        .descendants()
        .find(|node| {
            node.has_tag_name("marker") && node.attribute("id") == Some("merman_er-zeroOrOneStart")
        })
        .expect("expected classic zeroOrOneStart marker");
    assert_eq!(zero_or_one_start.attribute("markerUnits"), None);
    let circle = zero_or_one_start
        .children()
        .find(|node| node.has_tag_name("circle"))
        .expect("expected classic zeroOrOneStart circle");
    assert_eq!(circle.attribute("fill"), Some("white"));
    assert_eq!(circle.attribute("cx"), Some("21"));
    assert_eq!(circle.attribute("stroke-width"), None);
    let path = zero_or_one_start
        .children()
        .find(|node| node.has_tag_name("path"))
        .expect("expected classic zeroOrOneStart path");
    assert_eq!(path.attribute("d"), Some("M9,0 L9,18"));
    assert_eq!(path.attribute("stroke-width"), None);

    let zero_or_more_start = document
        .descendants()
        .find(|node| {
            node.has_tag_name("marker") && node.attribute("id") == Some("merman_er-zeroOrMoreStart")
        })
        .expect("expected classic zeroOrMoreStart marker");
    assert_eq!(zero_or_more_start.attribute("markerUnits"), None);
    let circle = zero_or_more_start
        .children()
        .find(|node| node.has_tag_name("circle"))
        .expect("expected classic zeroOrMoreStart circle");
    assert_eq!(circle.attribute("fill"), Some("white"));
    assert_eq!(circle.attribute("cx"), Some("48"));
    assert_eq!(circle.attribute("stroke-width"), None);
}

#[test]
fn er_svg_renders_diagram_title_and_viewbox_includes_it() {
    let text = r#"---
title: Diagram Title
---
erDiagram
  A ||--o{ B : has
"#;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());

    assert!(svg.contains(r#"class="erDiagramTitleText""#));
    assert!(svg.contains(">Diagram Title<"));
    assert!(svg.contains("viewBox="));
}

#[test]
fn er_svg_title_expands_negative_viewbox_without_rebasing_graph_content() {
    let untitled = r#"erDiagram
  A ||--o{ B : has
"#;
    let titled = r#"---
title: A deliberately wide diagram title
---
erDiagram
  A ||--o{ B : has
"#;

    let untitled_svg = render_er_svg_from_text(untitled, &SvgRenderOptions::default());
    let titled_svg = render_er_svg_from_text(titled, &SvgRenderOptions::default());

    assert_eq!(
        entity_transform(&untitled_svg, "entity-A-0"),
        entity_transform(&titled_svg, "entity-A-0")
    );
    assert!(root_view_box(&titled_svg)[1] < 0.0);
}

#[test]
fn er_svg_forest_theme_renders_root_gradient() {
    let text = r#"---
config:
  theme: forest
---
erDiagram
  A ||--|| B : owns
"#;

    let svg = render_er_svg_from_text(
        text,
        &SvgRenderOptions {
            diagram_id: Some("er_theme_gradient".to_string()),
            ..SvgRenderOptions::default()
        },
    );

    assert!(
        svg.contains(r#"<linearGradient id="er_theme_gradient-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%">"#),
        "expected Mermaid 11.15 ER root gradient element: {svg}"
    );
}

#[test]
fn er_svg_relationship_labels_follow_root_htmllabels_not_flowchart_htmllabels() {
    let text = r#"%%{init: {"htmlLabels": true, "flowchart": {"htmlLabels": false}}}%%
erDiagram
  A ||--|| B : owns
"#;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());

    let edge_labels = edge_labels_group(&svg);
    assert!(svg.contains(r#"class="nodeLabel markdown-node-label""#));
    assert!(
        edge_labels.contains(r#"class="labelBkg""#)
            && edge_labels.contains(r#"<foreignObject width=""#),
        "expected ER relationship labels to keep HTML foreignObject output when root htmlLabels=true"
    );
}

#[test]
fn er_svg_relationship_labels_follow_flowchart_htmllabels_when_root_unset() {
    let text = r#"%%{init: {"flowchart": {"htmlLabels": false}}}%%
erDiagram
  A ||--|| B : owns
"#;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());

    let edge_labels = edge_labels_group(&svg);
    assert!(
        edge_labels.contains(r#"<rect class="background""#)
            && edge_labels.contains(">owns</tspan>")
            && edge_labels.contains(r#"text-anchor="middle""#)
            && !edge_labels.contains("<foreignObject"),
        "expected ER relationship labels to switch to SVG text when flowchart htmlLabels=false and root htmlLabels is unset"
    );
}

#[test]
fn er_svg_flowchart_htmllabels_only_changes_relationship_labels() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join(
            "upstream_cypress_erdiagram_spec_should_render_edge_labels_correctly_when_flowchart_htmllabels_is_019.mmd",
        );
    let text = std::fs::read_to_string(path).expect("ER flowchart htmlLabels fixture");

    let svg = render_er_svg_from_text_with_engine(
        &text,
        &SvgRenderOptions::default(),
        Engine::new().with_site_config(MermaidConfig::from_value(
            json!({"theme":"default", "look":"classic", "layout":"dagre"}),
        )),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid ER SVG");
    let edge_labels = edge_labels_group(&svg);

    assert!(
        edge_labels.contains(">places</tspan>") && !edge_labels.contains("<foreignObject"),
        "flowchart.htmlLabels=false must switch only relationship labels: {svg}"
    );

    let relationship_labels = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("edgeLabel"))
        .collect::<Vec<_>>();
    assert_eq!(relationship_labels.len(), 5);
    assert!(relationship_labels.iter().all(|label| {
        label.descendants().any(|node| node.has_tag_name("text"))
            && !label
                .descendants()
                .any(|node| node.has_tag_name("foreignObject"))
    }));

    let (label_width, label_height) = er_relationship_label_background_size(&svg, "places");
    assert!(label_width > 0.0);
    assert!(label_height > 0.0);

    let entities = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.starts_with("merman-entity-"))
        })
        .collect::<Vec<_>>();
    assert_eq!(entities.len(), 5);
    assert!(
        entities.iter().all(|entity| entity
            .descendants()
            .any(|node| node.has_tag_name("foreignObject"))),
        "root htmlLabels unset must keep ER entity labels as foreignObject: {svg}"
    );
    for entity_id in [
        "merman-entity-CUSTOMER-0",
        "merman-entity-ORDER-1",
        "merman-entity-LINE-ITEM-2",
        "merman-entity-ADDRESS-3",
        "merman-entity-INVOICE-4",
    ] {
        let (_, height) = er_entity_rect_size(&svg, entity_id);
        assert_eq!(height, 84.0, "unexpected height for {entity_id}");
    }
}

#[test]
fn er_svg_html_labels_false_uses_svg_text_for_entity_and_attribute_labels() {
    let text = r#"%%{init: {"htmlLabels": false}}%%
erDiagram
subgraph Orders [Order Domain]
  CUSTOMER {
    string id PK
    string name
  }
end
CUSTOMER ||--|| ORDER : owns
"#;

    let html_text = r#"%%{init: {"htmlLabels": true}}%%
erDiagram
subgraph Orders [Order Domain]
  CUSTOMER {
    string id PK
    string name
  }
end
CUSTOMER ||--|| ORDER : owns
"#;

    let svg = render_er_svg_from_text_with_engine(
        text,
        &SvgRenderOptions::default(),
        Engine::new().with_site_config(MermaidConfig::from_value(
            json!({"theme":"default", "look":"classic", "layout":"dagre"}),
        )),
    );
    let html_svg = render_er_svg_from_text_with_engine(
        html_text,
        &SvgRenderOptions::default(),
        Engine::new().with_site_config(MermaidConfig::from_value(
            json!({"theme":"default", "look":"classic", "layout":"dagre"}),
        )),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid ER SVG");

    assert!(
        !document
            .descendants()
            .any(|node| node.has_tag_name("foreignObject")),
        "htmlLabels=false must not emit foreignObject labels: {svg}"
    );

    let label_text = |class_name: &str| {
        let label = document
            .descendants()
            .find(|node| node.attribute("class") == Some(class_name))
            .unwrap_or_else(|| panic!("missing ER label group {class_name}: {svg}"));
        assert!(
            label.descendants().any(|node| node.has_tag_name("text")),
            "ER label {class_name} must use an SVG text terminal: {svg}"
        );
        label
            .descendants()
            .filter_map(|node| node.text())
            .find(|text| !text.is_empty())
            .unwrap_or_default()
            .to_owned()
    };

    assert_eq!(label_text("label name"), "CUSTOMER");
    assert_eq!(label_text("label attribute-type"), "string");
    assert_eq!(label_text("label attribute-name"), "id");

    let edge_labels = edge_labels_group(&svg);
    assert!(
        edge_labels.contains(">owns</tspan>") && !edge_labels.contains("<foreignObject"),
        "root htmlLabels=false must switch relationship labels to SVG text: {svg}"
    );
    let (label_width, label_height) = er_relationship_label_background_size(&svg, "owns");
    assert!(label_width > 0.0);
    assert!(label_height > 0.0);

    let svg_entity_size = er_entity_rect_size(&svg, "merman-entity-ORDER-1");
    let html_entity_size = er_entity_rect_size(&html_svg, "merman-entity-ORDER-1");
    assert_ne!(
        svg_entity_size, html_entity_size,
        "root htmlLabels=false must feed SVG text metrics into ER entity geometry"
    );
    assert!((svg_entity_size.1 - 77.6).abs() < 1e-9);
    assert!((html_entity_size.1 - 84.0).abs() < 1e-9);

    let cluster_label = document
        .descendants()
        .find(|node| node.attribute("class") == Some("cluster-label"))
        .expect("missing ER cluster label");
    assert!(
        cluster_label
            .descendants()
            .any(|node| node.has_tag_name("text")),
        "ER cluster labels must use SVG text when htmlLabels=false: {svg}"
    );
    assert!(
        cluster_label
            .descendants()
            .filter_map(|node| node.text())
            .collect::<String>()
            .contains("Order Domain"),
        "ER cluster title must be emitted as SVG text: {svg}"
    );
}

#[test]
fn er_static_entity_paint_reaches_plain_and_attribute_shells() {
    let source = r#"---
config:
  theme: default
  look: classic
  layout: dagre
---
erDiagram
  PLAIN
  TABLE {
    string id PK
  }
  PLAIN ||--|| TABLE : owns
"#;
    let theme = er_entity_paint_theme(
        CanvasPaint::solid("#123456").expect("valid entity fill"),
        Some(CanvasPaint::solid("#654321").expect("valid entity stroke")),
    );
    let svg = render_er_svg_from_text_with_theme(source, &theme);

    let plain_start = svg
        .find(r#"id="merman-entity-PLAIN-0""#)
        .expect("plain entity group");
    let plain_end = svg[plain_start..]
        .find("</g>")
        .map(|offset| plain_start + offset)
        .expect("plain entity group end");
    let plain = &svg[plain_start..plain_end];
    assert!(plain.contains(r#"class="basic label-container""#));
    assert!(plain.contains("fill:#123456"), "{plain}");
    assert!(plain.contains("stroke:#654321"), "{plain}");

    let table_start = svg
        .find(r#"id="merman-entity-TABLE-1""#)
        .expect("attribute entity group");
    let table_end = svg[table_start..]
        .find("</g>")
        .map(|offset| table_start + offset)
        .expect("attribute entity group end");
    let table = &svg[table_start..table_end];
    assert!(table.contains(r#"class="outer-path""#), "{table}");
    assert!(table.contains("fill:#123456"), "{table}");
    assert!(table.contains("stroke:#654321"), "{table}");
}

#[test]
fn er_tokens_only_definition_reaches_the_model_owned_entity_surface() {
    let definition = ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_color(ThemeColorTokenV1::Surface, "#123456")
            .with_color(ThemeColorTokenV1::Border, "#654321"),
    );
    let materialized = materialize_theme(&definition).expect("materialize tokens-only ER theme");
    let theme = DiagramThemeCompiler::new()
        .compile_spec_wire(materialized.into_spec())
        .expect("compile tokens-only ER theme");

    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync("---\nconfig:\n  theme: default\n  look: classic\n  layout: dagre\n---\nerDiagram\n  CUSTOMER\n", ParseOptions::strict())
        .expect("parse tokens-only ER diagram")
        .expect("detect tokens-only ER diagram");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::BestEffort)
        .begin_session_with_theme(&theme)
        .expect("begin tokens-only ER session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare tokens-only ER artifact");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render tokens-only ER SVG");
    let entity_start = rendered
        .svg()
        .find(r#"id="merman-entity-CUSTOMER-0""#)
        .expect("ER entity group");
    let entity_end = rendered.svg()[entity_start..]
        .find("</g>")
        .map(|offset| entity_start + offset)
        .expect("ER entity group end");
    let entity = &rendered.svg()[entity_start..entity_end];

    assert!(entity.contains("fill:#123456"), "tokens.surface: {entity}");
    assert!(entity.contains("stroke:#654321"), "tokens.border: {entity}");
    assert!(
        merman_render::__private::family_evidence(rendered.into_completion().report())
            .applied_count()
            > 0,
        "the Entity adapter must own at least one terminal mechanism"
    );
}

#[test]
fn er_visible_text_and_relation_label_have_direct_terminal_receipts() {
    let source = r#"erDiagram
  CUSTOMER {
    string id PK
    string name
  }
  CUSTOMER ||--o{ ORDER : owns
"#;
    let theme = er_visible_text_theme();
    let artifact = prepare_er_family_with_theme_and_engine(source, &theme, Engine::new());
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render directly themed ER visible surfaces");
    let svg = rendered.svg();

    assert_eq!(
        svg.matches(r#"style="color:#112233;fill:#112233""#).count(),
        8,
        "the two entity names, five non-empty attribute cells, and relation label must own the direct text fill: {svg}"
    );
    assert!(
        svg.contains(r#"<span class="edgeLabel" style="color:#112233;fill:#112233">"#),
        "ER relationship label terminal must own its direct fill: {svg}"
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);

    let native_artifact = prepare_er_family_with_theme_and_engine(
        source,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "htmlLabels": false
        }))),
    );
    let native = native_artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render native ER relationship label with direct text fill");
    assert!(
        Regex::new(r#"<text[^>]*style="color:#112233;fill:#112233"[^>]*>.*?owns.*?</text>"#)
            .expect("native ER relationship label regex")
            .is_match(native.svg()),
        "native ER relationship label text must own its direct fill after stylesheet cascade: {}",
        native.svg()
    );
    let native_evidence =
        merman_render::__private::family_evidence(native.into_completion().report());
    assert_eq!(native_evidence.applied_count(), 1);
    assert_eq!(native_evidence.theme_residual_count(), 0);

    let resvg = prepare_er_family_with_theme_and_engine(source, &theme, Engine::new())
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render HTML ER relationship label for resvg")
        .finalize_resvg(&SvgPipeline::resvg_safe())
        .expect("finalize HTML ER relationship label for resvg");
    let native_svg = merman_render::__private::native_export_svg(resvg.svg());
    let native_document =
        roxmltree::Document::parse(native_svg).expect("valid native ER relationship label SVG");
    let relationship_label = native_document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node
                    .descendants()
                    .filter_map(|descendant| descendant.text())
                    .any(|text| text.contains("owns"))
        })
        .unwrap_or_else(|| panic!("native ER relationship label: {native_svg}"));
    assert_eq!(relationship_label.attribute("fill"), Some("#112233"));
}

#[test]
fn er_font_stack_is_measured_and_emitted_by_the_typed_family_plan() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(
                merman_render::DiagramFamilyId::ER,
                ThemeTextStyle::default().with_font_stack(
                    FontStack::single("ErTypedFont").expect("valid ER font stack"),
                ),
            ),
        ))
        .expect("compile ER typed FontStack theme");
    let source = "erDiagram\n  CUSTOMER {\n    string id PK\n  }\n";
    let artifact = prepare_er_family_with_theme_and_engine(source, &theme, Engine::new());
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render typed ER FontStack");

    assert!(
        rendered.svg().contains("font-family:ErTypedFont"),
        "typed ER FontStack must reach the final stylesheet: {}",
        rendered.svg()
    );
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn er_title_only_keeps_typed_font_stack_applicable() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(
                merman_render::DiagramFamilyId::ER,
                ThemeTextStyle::default().with_font_stack(
                    FontStack::single("ErTitleFont").expect("valid ER title font stack"),
                ),
            ),
        ))
        .expect("compile ER title-only FontStack theme");
    let source = r#"---
title: Title-only ER
---
erDiagram
"#;
    let artifact = try_prepare_er_family_with_theme_and_engine_requirement(
        source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("prepare title-only ER with strict typed FontStack");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render title-only ER with strict typed FontStack");

    assert!(
        rendered.svg().contains("font-family:ErTitleFont"),
        "typed ER FontStack must reach the title stylesheet: {}",
        rendered.svg()
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid title-only ER SVG");
    let title = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|token| token == "erDiagramTitleText")
                })
        })
        .unwrap_or_else(|| panic!("missing ER title terminal: {}", rendered.svg()));
    assert_eq!(title.text(), Some("Title-only ER"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn er_without_visible_text_keeps_typed_font_stack_not_applicable() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(
                merman_render::DiagramFamilyId::ER,
                ThemeTextStyle::default().with_font_stack(
                    FontStack::single("UnusedErFont").expect("valid ER font stack"),
                ),
            ),
        ))
        .expect("compile empty ER FontStack theme");
    let artifact = try_prepare_er_family_with_theme_and_engine_requirement(
        "erDiagram\n",
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("prepare empty ER with strict typed FontStack");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render empty ER with strict typed FontStack");

    assert!(
        rendered.svg().contains("font-family:UnusedErFont"),
        "the root stylesheet may carry the resolved font even when no visible terminal exists"
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn er_typed_font_size_reaches_dagre_measurement_layout_css_and_evidence() {
    let rendered = render_er_typed_font_size_probe(
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "fontSize": 10
        }))),
        "test.er-typed-font-size-dagre",
    );
    assert_er_typed_font_size_output(rendered);
}

#[test]
fn er_typed_font_size_changes_dagre_entity_geometry() {
    let small = render_er_typed_font_size_probe_with_size(
        Engine::new(),
        "test.er-typed-font-size-dagre-small",
        12.0,
    );
    let large = render_er_typed_font_size_probe_with_size(
        Engine::new(),
        "test.er-typed-font-size-dagre-large",
        24.0,
    );
    let (small_width, small_height) = first_er_entity_rect_size(small.svg());
    let (large_width, large_height) = first_er_entity_rect_size(large.svg());

    assert!(
        large_width > small_width,
        "larger typed text must widen the measured ER entity: {small_width} -> {large_width}"
    );
    assert!(
        large_height > small_height,
        "larger typed text must increase the measured ER entity height: {small_height} -> {large_height}"
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_typed_font_size_reaches_elk_measurement_layout_css_and_evidence() {
    let rendered = render_er_typed_font_size_probe(
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "fontSize": 10,
            "layout": "elk"
        }))),
        "test.er-typed-font-size-elk",
    );
    assert_er_typed_font_size_output(rendered);
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_typed_font_size_changes_elk_entity_geometry() {
    let elk_engine = || {
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "layout": "elk"
        })))
    };
    let small = render_er_typed_font_size_probe_with_size(
        elk_engine(),
        "test.er-typed-font-size-elk-small",
        12.0,
    );
    let large = render_er_typed_font_size_probe_with_size(
        elk_engine(),
        "test.er-typed-font-size-elk-large",
        24.0,
    );
    let (small_width, small_height) = first_er_entity_rect_size(small.svg());
    let (large_width, large_height) = first_er_entity_rect_size(large.svg());

    assert!(
        large_width > small_width,
        "larger typed text must widen the ELK ER entity: {small_width} -> {large_width}"
    );
    assert!(
        large_height > small_height,
        "larger typed text must increase the ELK ER entity height: {small_height} -> {large_height}"
    );
}

#[test]
fn er_source_owned_theme_font_size_is_not_claimed_as_typed() {
    let theme = er_font_size_theme(24.0);
    let source = r##"%%{init: {"themeVariables": {"fontSize": "30px"}}}%%
erDiagram
  CUSTOMER ||--o{ ORDER : owns
"##;
    let artifact = try_prepare_er_family_with_theme_and_engine_requirement(
        source,
        &theme,
        legacy_init_theme_compat_engine(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("prepare source-owned ER font size");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render source-owned ER font size");

    assert!(
        Regex::new(r#"#merman\{[^}]*font-size:30px;"#)
            .expect("source-owned ER root font-size regex")
            .is_match(rendered.svg()),
        "source-owned font size must reach ER CSS: {}",
        rendered.svg()
    );
    assert!(
        !Regex::new(r#"#merman\{[^}]*font-size:24px;"#)
            .expect("typed ER root font-size regex")
            .is_match(rendered.svg()),
        "typed font size must yield to source themeVariables.fontSize: {}",
        rendered.svg()
    );
    assert!(
        Regex::new(r#"#merman \.edgeLabel \.label\{[^}]*font-size:14px;"#)
            .expect("source-owned ER relationship-label font-size regex")
            .is_match(rendered.svg())
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn er_title_only_keeps_typed_font_size_applicable() {
    let theme = er_font_size_theme(24.0);
    let source = r#"---
title: Title-only ER
---
erDiagram
"#;
    let artifact = try_prepare_er_family_with_theme_and_engine_requirement(
        source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("prepare title-only ER with strict typed FontSize");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render title-only ER with strict typed FontSize");

    assert!(rendered.svg().contains("font-size:24px"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn er_without_visible_base_text_keeps_typed_font_size_not_applicable() {
    let theme = er_font_size_theme(24.0);
    let artifact = try_prepare_er_family_with_theme_and_engine_requirement(
        "erDiagram\n",
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("prepare empty ER with strict typed FontSize");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render empty ER with strict typed FontSize");

    assert!(rendered.svg().contains("font-size:24px"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn er_entity_source_font_size_remains_a_fail_closed_layout_residual() {
    let theme = er_font_size_theme(24.0);
    let source = r#"erDiagram
  CUSTOMER:::sized
  classDef sized font-size:30px
"#;
    let artifact = try_prepare_er_family_with_theme_and_engine_requirement(
        source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("prepare ER entity-local font size");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render ER entity-local font size in best-effort mode");

    assert!(rendered.svg().contains("font-size:24px"));
    assert!(
        rendered.svg().contains("font-size:30px !important"),
        "ER source class font size must remain the terminal winner: {}",
        rendered.svg()
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);
    assert_eq!(evidence.compatibility_residual_count(), 0);

    let strict = try_prepare_er_family_with_theme_and_engine_requirement(
        source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("prepare strict ER entity-local font size");
    let error = match strict.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
        Ok(_) => panic!("entity-local ER font size must fail closed in portable mode"),
        Err(error) => error,
    };
    assert!(
        matches!(
            error,
            merman_render::Error::UnverifiedFamilyTheme {
                family_id: merman_render::DiagramFamilyId::ER,
                residual_count: 1,
            }
        ),
        "unexpected ER source font-size portability error: {error:?}"
    );
}

#[test]
fn er_inherited_entity_font_size_keeps_the_typed_owner() {
    let theme = er_font_size_theme(24.0);
    for inherited_value in ["inherit", "unset"] {
        let source = format!(
            "erDiagram\n  CUSTOMER:::sized\n  classDef sized font-size:{inherited_value}\n"
        );
        let artifact = try_prepare_er_family_with_theme_and_engine_requirement(
            &source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("prepare inherited ER entity font size");
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render inherited ER entity font size");

        assert!(rendered.svg().contains("font-size:24px"));
        assert!(
            rendered
                .svg()
                .contains(&format!("font-size:{inherited_value} !important"))
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1, "value={inherited_value}");
        assert_eq!(
            evidence.theme_residual_count(),
            0,
            "value={inherited_value}"
        );
    }
}

#[test]
fn er_descendant_font_size_is_an_unverified_layout_owner() {
    let theme = er_font_size_theme(24.0);
    for alias in [
        "Before <span style='font-size:40px'>Big</span>",
        "Before <span style='font-size:/* host */40px'>Big</span>",
        "Before <span style='font:40px serif'>Big</span>",
        "Before <span style='all:initial'>Big</span>",
        "Before <span class='host'>Big</span>",
        "Before <font size='7'>Big</font>",
    ] {
        let source = format!("erDiagram\n  CUSTOMER[\"{alias}\"]\n");
        let artifact = try_prepare_er_family_with_theme_and_engine_requirement(
            &source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .expect("prepare ER descendant font-size override");
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render ER descendant font-size override in best-effort mode");
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0, "alias={alias}");
        assert_eq!(evidence.theme_residual_count(), 1, "alias={alias}");

        let strict = try_prepare_er_family_with_theme_and_engine_requirement(
            &source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("prepare strict ER descendant font-size override");
        let error =
            match strict.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("descendant ER font size must fail closed: {alias}"),
                Err(error) => error,
            };
        assert!(
            matches!(
                error,
                merman_render::Error::UnverifiedFamilyTheme {
                    family_id: merman_render::DiagramFamilyId::ER,
                    residual_count: 1,
                }
            ),
            "unexpected ER descendant font-size portability error for {alias}: {error:?}"
        );
    }
}

#[test]
fn er_descendant_inheritance_and_invalid_css_keep_the_typed_font_size_owner() {
    let theme = er_font_size_theme(24.0);
    for alias in [
        "Before <span style='font-size:inherit'>Inherited</span>",
        "Before <span style='font-size:unset'>Unset</span>",
        "Before <span style='all:red'>Invalid</span>",
    ] {
        let source = format!("erDiagram\n  CUSTOMER[\"{alias}\"]\n");
        let artifact = try_prepare_er_family_with_theme_and_engine_requirement(
            &source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("prepare inherited ER descendant font size");
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render inherited ER descendant font size");
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1, "alias={alias}");
        assert_eq!(evidence.theme_residual_count(), 0, "alias={alias}");
    }
}

#[test]
fn er_typed_font_size_uses_canonical_f32_css_spelling() {
    let theme = er_font_size_theme(14.4);
    let rendered =
        prepare_er_family_with_theme_and_engine("erDiagram\n  CUSTOMER\n", &theme, Engine::new())
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render non-integer ER font size");

    assert!(rendered.svg().contains("font-size:14.4px"));
    assert!(!rendered.svg().contains("14.399999618530273px"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn er_base_typography_is_typed_for_font_stack_and_font_size() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(FontStack::single("ErTypedFont").expect("valid ER font stack"))
        .with_font_size_px(23.0)
        .expect("valid ER typed font size");
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default()
                    .with_family_style(merman_render::DiagramFamilyId::ER, typography),
            ),
        )
        .expect("compile ER mixed typography theme");
    let source = "erDiagram\n  CUSTOMER {\n    string id PK\n  }\n";
    let artifact = try_prepare_er_family_with_theme_and_engine_requirement(
        source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("prepare ER mixed typography in best-effort mode");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render ER mixed typography");
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());

    assert!(completion.output().contains("font-family:ErTypedFont"));
    assert!(completion.output().contains("font-size:23px"));
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn er_configured_font_family_owns_the_font_stack_route() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(
                merman_render::DiagramFamilyId::ER,
                ThemeTextStyle::default().with_font_stack(
                    FontStack::single("ErTypedFont").expect("valid ER font stack"),
                ),
            ),
        ))
        .expect("compile ER configured-owner theme");
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "themeVariables": {"fontFamily": "ConfigFont, sans-serif"}
    })));
    let source = "erDiagram\n  CUSTOMER {\n    string id PK\n  }\n";
    let artifact = prepare_er_family_with_theme_and_engine(source, &theme, engine);
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render ER configured font owner");
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());

    assert!(
        completion
            .output()
            .contains("font-family:ConfigFont,sans-serif")
    );
    assert!(!completion.output().contains("font-family:ErTypedFont"));
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn er_configured_font_family_is_not_applicable_with_a_typed_font_size_sibling() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(FontStack::single("ErTypedFont").expect("valid ER font stack"))
        .with_font_size_px(23.0)
        .expect("valid ER typed font size");
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default()
                    .with_family_style(merman_render::DiagramFamilyId::ER, typography),
            ),
        )
        .expect("compile ER configured mixed typography theme");
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "themeVariables": {"fontFamily": "ConfigFont, sans-serif"}
    })));
    let source = "erDiagram\n  CUSTOMER {\n    string id PK\n  }\n";
    let artifact = try_prepare_er_family_with_theme_and_engine_requirement(
        source,
        &theme,
        engine,
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("prepare ER configured mixed typography");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render ER configured mixed typography");
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());

    assert!(
        completion
            .output()
            .contains("font-family:ConfigFont,sans-serif")
    );
    assert!(!completion.output().contains("font-family:ErTypedFont"));
    assert!(completion.output().contains("font-size:23px"));
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn er_native_relation_text_uses_text_color_owner_not_node_text_color() {
    let theme = er_visible_text_theme();
    let artifact = prepare_er_family_with_theme_and_engine(
        "erDiagram\n  CUSTOMER ||--o{ ORDER : owns\n",
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "htmlLabels": false,
            "themeVariables": {
                "nodeTextColor": "#dc2626"
            }
        }))),
    );
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("nodeTextColor must not suppress native relation text ownership");
    assert!(
        Regex::new(r#"<text[^>]*style=\"color:#112233;fill:#112233\"[^>]*>.*?owns.*?</text>"#)
            .expect("native ER relation label regex")
            .is_match(rendered.svg()),
        "the native relationship label must use the typed Text.fill winner: {}",
        rendered.svg()
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn er_mixed_html_color_owners_cannot_prove_portable_typed_text_fill() {
    let theme = er_visible_text_theme();
    for (label, source_owned_marker) in [
        (
            "Before <span style='color:red'>Red</span> After",
            "color:red",
        ),
        (
            "Before <span class='label'>CSS-owned</span> After",
            "class='label'",
        ),
    ] {
        let source = format!("erDiagram\n  CUSTOMER ||--o{{ ORDER : \"{label}\"\n");
        let artifact = try_prepare_er_family_with_theme_and_engine_requirement(
            &source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .expect("prepare mixed-color ER relationship label");
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render mixed-color ER relationship label in best-effort mode");
        assert!(
            rendered
                .svg()
                .contains(r#"style="color:#112233;fill:#112233""#),
            "the inherited runs should keep the typed browser color: {}",
            rendered.svg()
        );
        assert!(
            rendered.svg().contains(source_owned_marker),
            "the source-owned run should survive in browser SVG: {}",
            rendered.svg()
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);

        let strict = prepare_er_family_with_theme_and_engine(&source, &theme, Engine::new());
        let error =
            match strict.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("mixed XHTML color ownership must fail closed in portable mode"),
                Err(error) => error,
            };
        assert!(
            matches!(
                error,
                merman_render::Error::IncompleteFamilyTheme {
                    family_id: merman_render::DiagramFamilyId::ER,
                    required_count: 1,
                    accounted_count: 0,
                }
            ),
            "unexpected ER mixed-color portability error: {error:?}"
        );
    }
}

fn er_row_path<'a>(document: &'a roxmltree::Document<'a>, odd: bool) -> roxmltree::Node<'a, 'a> {
    let class = if odd { "row-rect-odd" } else { "row-rect-even" };
    document
        .descendants()
        .find(|node| node.attribute("class") == Some(class))
        .and_then(|row| {
            row.children()
                .find(|node| node.has_tag_name("path") && node.attribute("stroke") == Some("none"))
        })
        .expect("visible row fill path")
}

#[test]
fn er_table_odd_even_fill_reaches_each_matching_row() {
    let source = "---\nconfig:\n  theme: default\n  layout: dagre\n---\nerDiagram\n CUSTOMER {\n string id\n string name\n string email\n }\n ORDER {\n string id\n string status\n }\n";
    for look in ["classic", "neo", "handDrawn"] {
        for html in [true, false] {
            for (paint, css) in [
                (CanvasPaint::solid("#aabbcc").unwrap(), "#aabbcc"),
                (CanvasPaint::Transparent, "transparent"),
            ] {
                for variant in [ThemeVariant::Odd, ThemeVariant::Even] {
                    let theme = er_relation_rules_theme([ThemeRule::new(
                        ThemeTarget::Table,
                        ThemeStylePatch::default().with_fill(paint.clone()),
                    )
                    .with_variant(variant)]);
                    let rendered = prepare_er_family_with_theme_and_engine(
                        source,
                        &theme,
                        Engine::new().with_site_config(MermaidConfig::from_value(
                            json!({"look": look, "htmlLabels": html}),
                        )),
                    )
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .unwrap();
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    for (class, count, row_variant) in [
                        ("row-rect-odd", 3, ThemeVariant::Odd),
                        ("row-rect-even", 2, ThemeVariant::Even),
                    ] {
                        let rows = document
                            .descendants()
                            .filter(|node| node.attribute("class") == Some(class))
                            .collect::<Vec<_>>();
                        assert_eq!(rows.len(), count);
                        for row in rows {
                            let path = row
                                .children()
                                .find(|node| {
                                    node.has_tag_name("path")
                                        && node.attribute("stroke") == Some("none")
                                })
                                .unwrap();
                            assert!(path.attribute("d").is_some_and(|d| !d.is_empty()));
                            let expected = (row_variant == variant).then(|| format!("fill:{css}"));
                            assert_eq!(
                                path.attribute("style"),
                                expected.as_deref(),
                                "{look}/{html}/{variant:?}"
                            );
                        }
                    }
                    let completion = rendered.into_completion();
                    let evidence = merman_render::__private::family_evidence(completion.report());
                    assert_eq!(evidence.applied_count(), 1);
                    assert_eq!(evidence.theme_residual_count(), 0);
                    assert_eq!(evidence.compatibility_residual_count(), 0);
                }
            }
        }
    }
}

#[test]
fn er_table_row_config_ownership_is_property_local() {
    let source = "erDiagram\n CUSTOMER {\n string id\n string name\n }\n";
    let theme = er_table_row_theme();
    for odd in [true, false] {
        let key = if odd { "rowOdd" } else { "rowEven" };
        for authored_source in [false, true] {
            for color in ["#112233", if odd { "#aabbcc" } else { "#ddeeff" }] {
                let config = json!({"themeVariables": {key: color}});
                let text = if authored_source {
                    format!("%%{{init: {config}}}%%\n{source}")
                } else {
                    source.to_owned()
                };
                let engine =
                    Engine::new().with_site_config(MermaidConfig::from_value(if authored_source {
                        json!({"secure": []})
                    } else {
                        config
                    }));
                let rendered = prepare_er_family_with_theme_and_engine(&text, &theme, engine)
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .unwrap();
                assert_eq!(
                    rendered
                        .metadata()
                        .effective_config
                        .get_str(&format!("themeVariables.{key}")),
                    Some(color),
                    "{key}/source={authored_source}"
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let owned = er_row_path(&document, odd);
                assert_eq!(
                    owned.attribute("fill"),
                    Some(color),
                    "{key}/source={authored_source}"
                );
                assert_eq!(
                    owned.attribute("style"),
                    None,
                    "{key}/source={authored_source}"
                );
                let other = er_row_path(&document, !odd);
                assert_eq!(
                    other.attribute("style"),
                    Some(if odd { "fill:#ddeeff" } else { "fill:#aabbcc" })
                );
                let completion = rendered.into_completion();
                let evidence = merman_render::__private::family_evidence(completion.report());
                assert_eq!(evidence.applied_count(), 1);
                assert_eq!(evidence.not_applicable_count(), 1);
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn er_table_source_fill_owns_only_even_row_paths() {
    for source_style in [
        "style CUSTOMER fill:#112233",
        "classDef accent fill:#112233\n class CUSTOMER accent",
    ] {
        let source = format!(
            "---\nconfig:\n  theme: default\n  look: classic\n  layout: dagre\n---\nerDiagram\n CUSTOMER {{\n string id\n string name\n }}\n {source_style}\n"
        );
        let rendered =
            prepare_er_family_with_theme_and_engine(&source, &er_table_row_theme(), Engine::new())
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            er_row_path(&document, true).attribute("style"),
            Some("fill:#aabbcc")
        );
        assert_eq!(
            er_row_path(&document, false).attribute("style"),
            Some("fill:#112233 !important")
        );
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn er_table_unqualified_fill_reaches_both_row_writers() {
    let theme = er_relation_rules_theme([ThemeRule::new(
        ThemeTarget::Table,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#abcdef").unwrap()),
    )]);
    let rendered = try_prepare_er_family_with_theme_and_engine(
        "erDiagram\n CUSTOMER {\n string id\n string name\n }\n",
        &theme,
        Engine::new(),
    )
    .unwrap()
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    for odd in [true, false] {
        let path = er_row_path(&document, odd);
        assert_eq!(path.attribute("style"), Some("fill:#abcdef"));
    }
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn er_table_fill_does_not_certify_unsupported_sibling_typography() {
    let mut patch = ThemeStylePatch::default().with_fill(CanvasPaint::solid("#aabbcc").unwrap());
    patch.typography.font_size_px = Specified::Value(28.0);
    let theme = er_relation_rules_theme([
        ThemeRule::new(ThemeTarget::Table, patch).with_variant(ThemeVariant::Odd)
    ]);
    let source = "erDiagram\n CUSTOMER {\n string id\n }\n";
    let result = try_prepare_er_family_with_theme_and_engine(source, &theme, Engine::new())
        .and_then(|artifact| {
            artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        });
    assert!(
        matches!(
            result,
            Err(merman_render::Error::UnverifiedFamilyTheme { .. })
        ),
        "unsupported Table typography must not be certified by its sibling fill"
    );
}

#[test]
fn er_table_row_fill_requires_a_matching_visible_row() {
    for (source, applied) in [
        ("erDiagram\n", 0),
        ("erDiagram\n CUSTOMER\n", 0),
        ("erDiagram\n CUSTOMER {\n string id\n }\n", 1),
    ] {
        let rendered =
            prepare_er_family_with_theme_and_engine(source, &er_table_row_theme(), Engine::new())
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
        roxmltree::Document::parse(rendered.svg()).unwrap();
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), applied);
        assert_eq!(evidence.not_applicable_count(), 2 - applied);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn er_source_entity_styles_outrank_typed_entity_paint() {
    let source = r#"---
config:
  theme: default
  look: classic
  layout: dagre
---
erDiagram
  BARE
  ASSIGNED
  INLINE
  classDef accent fill:#00aa00,stroke:#00bb00
  class ASSIGNED accent
  style INLINE fill:#0000aa,stroke:#0000bb
"#;
    let theme = er_entity_paint_theme(
        CanvasPaint::solid("#123456").expect("valid entity fill"),
        Some(CanvasPaint::solid("#654321").expect("valid entity stroke")),
    );
    let svg = render_er_svg_from_text_with_theme(source, &theme);

    let entity_fragment = |id: &str| {
        let start = svg
            .find(&format!(r#"id="merman-entity-{id}-"#))
            .unwrap_or_else(|| panic!("missing ER entity {id}: {svg}"));
        let end = svg[start..]
            .find(r#"class="node"#)
            .and_then(|_| svg[start..].find("</g>"))
            .map(|offset| start + offset)
            .expect("entity fragment end");
        &svg[start..end]
    };

    let bare = entity_fragment("BARE");
    assert!(bare.contains("fill:#123456"), "{bare}");
    assert!(bare.contains("stroke:#654321"), "{bare}");
    let assigned = entity_fragment("ASSIGNED");
    assert!(assigned.contains("fill:#00aa00"), "{assigned}");
    assert!(assigned.contains("stroke:#00bb00"), "{assigned}");
    let inline = entity_fragment("INLINE");
    assert!(inline.contains("fill:#0000aa"), "{inline}");
    assert!(inline.contains("stroke:#0000bb"), "{inline}");

    let default_svg = render_er_svg_from_text_with_theme(
        "---\nconfig:\n  theme: default\n  look: classic\n  layout: dagre\n---\nerDiagram\n  DEFAULT\n  classDef default fill:#aa0000,stroke:#bb0000\n",
        &theme,
    );
    assert!(default_svg.contains("fill:#aa0000"), "{default_svg}");
    assert!(default_svg.contains("stroke:#bb0000"), "{default_svg}");
}

#[test]
fn er_site_theme_variable_ownership_controls_typed_entity_paint() {
    let source = "erDiagram\n  PLAIN\n  TABLE {\n    string id PK\n  }\n";
    let theme = er_entity_paint_theme(
        CanvasPaint::solid("#123456").expect("valid entity fill"),
        Some(CanvasPaint::solid("#654321").expect("valid entity stroke")),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "themeVariables": {
            "mainBkg": "#112233",
            "nodeBorder": "#445566"
        }
    })));
    let svg = render_er_svg_from_text_with_theme_and_engine(source, &theme, engine);

    assert!(svg.contains("#112233"), "{svg}");
    assert!(svg.contains("#445566"), "{svg}");
    assert!(
        !svg.contains("#123456"),
        "site fill must own ER entity fill: {svg}"
    );
    assert!(
        !svg.contains("#654321"),
        "site stroke must own ER entity stroke: {svg}"
    );

    let default_primary_border_svg = render_er_svg_from_text_with_theme_and_engine(
        source,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "theme": "default",
            "themeVariables": {
                "primaryBorderColor": "#abcdef"
            }
        }))),
    );
    assert!(
        default_primary_border_svg.contains("#654321"),
        "default primaryBorderColor is not the terminal ER entity stroke owner: {default_primary_border_svg}"
    );

    let base_primary_svg = render_er_svg_from_text_with_theme_and_engine(
        source,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "theme": "base",
            "themeVariables": {
                "primaryColor": "#fff4dd"
            }
        }))),
    );
    assert!(
        !base_primary_svg.contains("#654321"),
        "base primaryColor-derived nodeBorder must own ER entity stroke: {base_primary_svg}"
    );
}

#[test]
fn er_entity_ordinal_paint_fails_closed() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Entity,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").expect("valid entity fill")),
                    )
                    .with_ordinal(OrdinalSelector::exact(1).expect("valid ER entity ordinal")),
                ),
            ),
        )
        .expect("compile ordinal ER entity theme");
    let error = match prepare_er_family_with_theme_and_engine(
        "erDiagram\n  PLAIN\n",
        &theme,
        Engine::new(),
    )
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("ER entity ordinal paint must remain unverified"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((merman_render::DiagramFamilyId::ER, 1))
    );
}

#[test]
fn er_entity_ordinal_palette_is_not_applicable_when_typed_fill_wins() {
    let palette = OrdinalPalette::new([
        ThemeColorValue::parse("#abcdef").expect("valid ER entity palette color")
    ])
    .expect("non-empty ER entity palette");
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(ThemeRule::new(
                        ThemeTarget::Entity,
                        ThemeStylePatch::default().with_fill(
                            CanvasPaint::solid("#123456").expect("valid ER entity fill"),
                        ),
                    ))
                    .with_ordinal_palette(ThemeTarget::Entity, palette),
            ),
        )
        .expect("compile ER entity fill and ordinal palette theme");
    let rendered =
        prepare_er_family_with_theme_and_engine("---\nconfig:\n  theme: default\n  look: classic\n  layout: dagre\n---\nerDiagram\n  PLAIN\n", &theme, Engine::new())
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("typed ER entity fill must shadow the unsupported palette");

    assert!(
        rendered.svg().contains("fill:#123456"),
        "{}",
        rendered.svg()
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn er_entity_paint_is_not_applicable_without_entities() {
    let theme = er_entity_paint_theme(
        CanvasPaint::solid("#123456").expect("valid entity fill"),
        Some(CanvasPaint::solid("#654321").expect("valid entity stroke")),
    );
    let _ = render_er_svg_from_text_with_theme("erDiagram\n", &theme);
}

#[test]
fn er_static_relation_stroke_reaches_path_referenced_markers_and_evidence() {
    for (name, stroke, expected_css) in [
        (
            "solid",
            CanvasPaint::solid("#123456").expect("valid ER relation stroke"),
            "#123456",
        ),
        ("transparent", CanvasPaint::Transparent, "transparent"),
    ] {
        let theme = er_relation_stroke_theme(stroke);
        let artifact = prepare_er_family_with_theme_and_engine(
            "erDiagram\n  A ||--o{ B : owns\n",
            &theme,
            Engine::new(),
        );
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap_or_else(|error| panic!("render {name} ER relation stroke: {error}"));
        let paths = relationship_path_tags(rendered.svg());
        assert_eq!(paths.len(), 1, "{name}: {}", rendered.svg());
        assert!(
            paths[0].contains(&format!("stroke:{expected_css}")),
            "{name}: {}",
            paths[0]
        );

        let marker_ids = referenced_marker_ids(&paths);
        assert_eq!(marker_ids.len(), 2, "{name}: {}", paths[0]);
        for marker_id in marker_ids {
            let marker = marker_opening_tag(rendered.svg(), &marker_id);
            assert!(
                marker.contains(&format!("stroke:{expected_css} !important")),
                "{name}/{marker_id}: {marker}"
            );
        }

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 1, "{name}");
        assert_eq!(evidence.not_applicable_count(), 0, "{name}");
    }
}

#[test]
fn er_explicit_default_relation_stroke_reaches_path_referenced_markers_and_evidence() {
    for (stroke, expected_css) in [
        (
            CanvasPaint::solid("#123456").expect("valid ER relation stroke"),
            "#123456",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ] {
        let theme = er_relation_rules_theme([ThemeRule::new(
            ThemeTarget::Relation,
            ThemeStylePatch::default().with_stroke(stroke),
        )
        .with_variant(ThemeVariant::Default)]);
        let artifact = prepare_er_family_with_theme_and_engine(
            "erDiagram\n  A ||--o{ B : owns\n",
            &theme,
            Engine::new(),
        );
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render explicit default ER relation stroke");
        let paths = relationship_path_tags(rendered.svg());
        assert_eq!(paths.len(), 1, "{}", rendered.svg());
        assert!(
            paths[0].contains(&format!("stroke:{expected_css}")),
            "{}",
            paths[0]
        );
        for marker_id in referenced_marker_ids(&paths) {
            let marker = marker_opening_tag(rendered.svg(), &marker_id);
            assert!(
                marker.contains(&format!("stroke:{expected_css} !important")),
                "{marker_id}: {marker}"
            );
        }

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn er_static_relation_stroke_binds_all_recursive_segments_and_terminal_markers() {
    let theme = er_relation_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid recursive ER relation stroke"),
    );
    let svg = render_er_svg_from_text_with_theme("erDiagram\n  A ||--o{ A : refers\n", &theme);
    let paths = relationship_path_tags(&svg);
    // Dagre keeps three helper segments in the layout model, while Mermaid's common SVG painter
    // publishes one merged logical self-loop path. The layout test covers the helper segments;
    // this SVG test must assert the visible terminal instead of the internal projection.
    assert_eq!(paths.len(), 1, "recursive ER path count: {svg}");
    assert!(
        paths.iter().all(|path| path.contains("stroke:#123456")),
        "every recursive ER path segment must own the direct stroke: {paths:?}"
    );

    let marker_ids = referenced_marker_ids(&paths);
    assert_eq!(
        marker_ids.len(),
        2,
        "recursive ER marker references: {paths:?}"
    );
    for marker_id in marker_ids {
        let marker = marker_opening_tag(&svg, &marker_id);
        assert!(
            marker.contains("stroke:#123456 !important"),
            "recursive ER marker {marker_id}: {marker}"
        );
    }
}

#[test]
fn er_explicit_line_color_owns_relation_paths_and_markers() {
    let theme =
        er_relation_stroke_theme(CanvasPaint::solid("#123456").expect("valid ER relation stroke"));
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "themeVariables": { "lineColor": "#fedcba" }
    })));
    let artifact =
        prepare_er_family_with_theme_and_engine("erDiagram\n  A ||--o{ B : owns\n", &theme, engine);
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render source-owned ER relation stroke");
    let paths = relationship_path_tags(rendered.svg());
    assert_eq!(paths.len(), 1, "{}", rendered.svg());
    assert!(!paths[0].contains("#123456"), "{}", paths[0]);
    assert!(
        rendered.svg().contains("stroke:#fedcba"),
        "{}",
        rendered.svg()
    );
    for marker_id in referenced_marker_ids(&paths) {
        let marker = marker_opening_tag(rendered.svg(), &marker_id);
        assert!(!marker.contains("#123456"), "{marker_id}: {marker}");
    }

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn er_relation_stroke_is_not_applicable_without_relationships() {
    let theme =
        er_relation_stroke_theme(CanvasPaint::solid("#123456").expect("valid ER relation stroke"));
    let artifact =
        prepare_er_family_with_theme_and_engine("erDiagram\n  A\n", &theme, Engine::new());
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("ER relation stroke without relationships is not applicable");
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn er_relation_theme_rejects_unowned_facets_and_non_static_stroke_routes() {
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(
                0.0,
                ThemeColorValue::parse("#123456").expect("valid ER gradient start"),
            )
            .expect("valid ER gradient stop"),
            GradientStop::new(
                1.0,
                ThemeColorValue::parse("#abcdef").expect("valid ER gradient end"),
            )
            .expect("valid ER gradient stop"),
        ],
    )
    .expect("valid ER gradient");
    let pattern = PatternSpec::new(
        PatternKind::Grid,
        8.0,
        8.0,
        ThemeColorValue::parse("#123456").expect("valid ER pattern color"),
    )
    .expect("valid ER pattern");
    let mut clear = ThemeStylePatch::default();
    clear.stroke.paint = Specified::Clear;
    let solid = || {
        ThemeStylePatch::default()
            .with_stroke(CanvasPaint::solid("#123456").expect("valid ER relation stroke"))
    };
    let unsupported_cases = [
        ThemeRule::new(ThemeTarget::Relation, solid())
            .with_ordinal(OrdinalSelector::exact(1).expect("valid ER relation ordinal")),
        ThemeRule::new(ThemeTarget::Relation, clear),
        ThemeRule::new(
            ThemeTarget::Relation,
            ThemeStylePatch::default().with_stroke(CanvasPaint::LinearGradient(gradient)),
        ),
        ThemeRule::new(
            ThemeTarget::Relation,
            ThemeStylePatch::default().with_stroke(CanvasPaint::Pattern(pattern)),
        ),
    ];

    for rule in unsupported_cases {
        let theme = er_relation_rules_theme([rule]);
        let result = try_prepare_er_family_with_theme_and_engine(
            "erDiagram\n  A ||--o{ B : owns\n",
            &theme,
            Engine::new(),
        )
        .and_then(|artifact| {
            artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        });
        let error = match result {
            Ok(_) => panic!("unsupported ER theme route must fail closed"),
            Err(error) => error,
        };
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::ER, 1))
        );
    }
}

#[test]
fn er_title_fill_is_unsupported_and_reconciles_title_presence() {
    let theme = er_relation_rules_theme([ThemeRule::new(
        ThemeTarget::Title,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#654321").expect("valid ER title fill")),
    )]);
    let titled_source = r#"---
title: ER title
---
erDiagram
  A ||--o{ B : owns
"#;

    let best_effort = try_prepare_er_family_with_theme_and_engine_requirement(
        titled_source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("prepare ER title fill in best-effort mode")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("render ER title fill in best-effort mode");
    assert!(best_effort.svg().contains(r#"class="erDiagramTitleText""#));
    let evidence =
        merman_render::__private::family_evidence(best_effort.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);
    assert_eq!(evidence.compatibility_residual_count(), 0);

    let strict_result = try_prepare_er_family_with_theme_and_engine_requirement(
        titled_source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .and_then(|artifact| {
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    });
    let strict_error = match strict_result {
        Ok(_) => panic!("strict ER title fill must fail closed"),
        Err(error) => error,
    };
    assert_eq!(
        strict_error.unverified_family_theme(),
        Some((merman_render::DiagramFamilyId::ER, 1))
    );

    let untitled_source = "erDiagram\n  A ||--o{ B : owns\n";
    let untitled = try_prepare_er_family_with_theme_and_engine_requirement(
        untitled_source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("ER title fill without a title is not applicable")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("render ER without a title");
    let untitled_evidence =
        merman_render::__private::family_evidence(untitled.into_completion().report());
    assert_eq!(untitled_evidence.required_count(), 1);
    assert_eq!(untitled_evidence.accounted_count(), 1);
    assert_eq!(untitled_evidence.applied_count(), 0);
    assert_eq!(untitled_evidence.not_applicable_count(), 1);
    assert_eq!(untitled_evidence.theme_residual_count(), 0);
    assert_eq!(untitled_evidence.compatibility_residual_count(), 0);
}

#[test]
fn er_shadowed_unsupported_relation_selector_is_not_applicable() {
    let unsupported_ordinal = ThemeRule::new(
        ThemeTarget::Relation,
        ThemeStylePatch::default()
            .with_stroke(CanvasPaint::solid("#aa0000").expect("valid ordinal stroke")),
    )
    .with_ordinal(OrdinalSelector::exact(1).expect("valid ER relation ordinal"));
    let final_static = ThemeRule::new(
        ThemeTarget::Relation,
        ThemeStylePatch::default()
            .with_stroke(CanvasPaint::solid("#123456").expect("valid static stroke")),
    );
    let theme = er_relation_rules_theme([unsupported_ordinal, final_static]);
    let rendered = prepare_er_family_with_theme_and_engine(
        "erDiagram\n  A ||--o{ B : owns\n",
        &theme,
        Engine::new(),
    )
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("a shadowed unsupported ER selector must not block the final direct winner");

    assert!(
        relationship_path_tags(rendered.svg())
            .iter()
            .all(|path| path.contains("stroke:#123456")),
        "the final static winner must reach every visible relation: {}",
        rendered.svg()
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn er_relation_fill_fallback_reaches_visible_paths_and_referenced_markers() {
    for look in ["classic", "neo", "handDrawn"] {
        for variant in [None, Some(ThemeVariant::Default)] {
            for (paint, css) in [
                (CanvasPaint::solid("#123456").unwrap(), "#123456"),
                (CanvasPaint::Transparent, "transparent"),
            ] {
                let mut rule = ThemeRule::new(
                    ThemeTarget::Relation,
                    ThemeStylePatch::default().with_fill(paint),
                );
                if let Some(variant) = variant {
                    rule = rule.with_variant(variant);
                }
                let theme = er_relation_rules_theme([rule]);
                let rendered = prepare_er_family_with_theme_and_engine(
                    "erDiagram\n A ||--o{ B : owns\n A ||--o{ A : refers\n",
                    &theme,
                    Engine::new()
                        .with_site_config(MermaidConfig::from_value(json!({"look": look}))),
                )
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let paths = relationship_path_tags(rendered.svg());
                assert_eq!(paths.len(), 2);
                for path in &paths {
                    assert!(path.contains(&format!("stroke:{css}")), "{look}: {path}");
                }
                let ids = referenced_marker_ids(&paths);
                assert!(!ids.is_empty());
                for id in ids {
                    let marker = document
                        .descendants()
                        .find(|node| node.attribute("id") == Some(id.as_str()))
                        .unwrap();
                    assert!(marker.has_tag_name("marker"));
                    assert!(
                        marker
                            .attribute("style")
                            .unwrap()
                            .contains(&format!("stroke:{css} !important"))
                    );
                }
                let completion = rendered.into_completion();
                let evidence = merman_render::__private::family_evidence(completion.report());
                assert_eq!(evidence.applied_count(), 1);
                assert_eq!(evidence.theme_residual_count(), 0);
                assert_eq!(evidence.compatibility_residual_count(), 0);
            }
        }
    }
}

#[test]
fn er_relation_stroke_shadows_fill_independently_of_rule_order() {
    for reverse in [false, true] {
        for stroke_paint in [
            Specified::Value(CanvasPaint::solid("#abcdef").unwrap()),
            Specified::Clear,
            Specified::Value(CanvasPaint::Pattern(
                PatternSpec::new(
                    PatternKind::Grid,
                    8.0,
                    8.0,
                    ThemeColorValue::parse("#abcdef").unwrap(),
                )
                .unwrap(),
            )),
        ] {
            let unsupported = !matches!(stroke_paint, Specified::Value(CanvasPaint::Solid(_)));
            let mut stroke = ThemeStylePatch::default();
            stroke.stroke.paint = stroke_paint;
            let mut rules = vec![
                ThemeRule::new(
                    ThemeTarget::Relation,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                ),
                ThemeRule::new(ThemeTarget::Relation, stroke),
            ];
            if reverse {
                rules.reverse();
            }
            let theme = er_relation_rules_theme(rules);
            let rendered = try_prepare_er_family_with_theme_and_engine_requirement(
                "erDiagram\n A ||--o{ B : owns\n",
                &theme,
                Engine::new(),
                if unsupported {
                    ThemePortabilityRequirement::BestEffort
                } else {
                    ThemePortabilityRequirement::RequirePortable
                },
            )
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
            let paths = relationship_path_tags(rendered.svg());
            assert_eq!(paths.len(), 1);
            assert!(!paths[0].contains("#123456"));
            if !unsupported {
                assert!(paths[0].contains("stroke:#abcdef"));
            }
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.not_applicable_count(), 1);
            assert_eq!(evidence.applied_count(), usize::from(!unsupported));
            assert_eq!(evidence.theme_residual_count(), usize::from(unsupported));
        }
    }
}

#[test]
fn er_text_fill_tracks_the_visible_title_and_its_own_configuration() {
    for variant in [None, Some(ThemeVariant::Default)] {
        for config in [
            json!({}),
            json!({"themeVariables": {"nodeTextColor": "#fedcba"}}),
            json!({"themeVariables": {"textColor": "#fedcba"}}),
        ] {
            let mut rule = ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            );
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            let theme = er_relation_rules_theme([rule]);
            let owned = config.pointer("/themeVariables/textColor").is_some();
            let rendered = prepare_er_family_with_theme_and_engine(
                "---\ntitle: ER overview\n---\nerDiagram\n",
                &theme,
                Engine::new().with_site_config(MermaidConfig::from_value(config)),
            )
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let title = document
                .descendants()
                .find(|node| node.attribute("class") == Some("erDiagramTitleText"))
                .unwrap();
            assert_eq!(title.text(), Some("ER overview"));
            assert_eq!(
                title.attribute("style"),
                (!owned).then_some("color:#123456;fill:#123456")
            );
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.applied_count(), usize::from(!owned));
            assert_eq!(evidence.not_applicable_count(), usize::from(owned));
            assert_eq!(evidence.compatibility_residual_count(), 0);
        }
    }
}

#[test]
fn er_text_fill_ownership_follows_each_html_or_svg_consumer() {
    let source = "erDiagram\n CUSTOMER {\n string id\n }\n CUSTOMER ||--o{ ORDER : owns\n";
    for html in [false, true] {
        for key in ["textColor", "nodeTextColor"] {
            for from_source in [false, true] {
                let theme = er_relation_rules_theme([ThemeRule::new(
                    ThemeTarget::Text,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                )
                .with_variant(ThemeVariant::Default)]);
                let config = json!({"theme": "base", "htmlLabels": html, "themeVariables": {key: "#fedcba"}});
                let (text, engine) = if from_source {
                    (
                        format!("---\nconfig: {config}\n---\n{source}"),
                        Engine::new()
                            .with_site_config(MermaidConfig::from_value(json!({"secure": []}))),
                    )
                } else {
                    (
                        source.to_owned(),
                        Engine::new().with_site_config(MermaidConfig::from_value(config)),
                    )
                };
                let rendered = prepare_er_family_with_theme_and_engine(&text, &theme, engine)
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .unwrap();
                assert_eq!(
                    rendered
                        .metadata()
                        .effective_config
                        .get_str(&format!("themeVariables.{key}")),
                    Some("#fedcba"),
                    "{key}/source={from_source}"
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let applied = (html && key == "textColor") || (!html && key == "nodeTextColor");
                for label in ["CUSTOMER", "ORDER", "string", "id", "owns"] {
                    let terminal = document
                        .descendants()
                        .find(|node| node.is_text() && node.text() == Some(label))
                        .unwrap_or_else(|| panic!("missing visible {label}"));
                    assert_eq!(
                        terminal.ancestors().any(|node| node
                            .attribute("style")
                            .is_some_and(|style| style.contains("fill:#123456"))),
                        applied,
                        "html={html}/{key}/source={from_source}/{label}"
                    );
                }
                let completion = rendered.into_completion();
                let evidence = merman_render::__private::family_evidence(completion.report());
                assert_eq!(evidence.applied_count(), usize::from(applied));
                assert_eq!(evidence.not_applicable_count(), usize::from(!applied));
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn er_svg_distinguishes_empty_and_whitespace_relationship_labels() {
    let svg = render_er_svg_from_text(
        r#"erDiagram
BOOK }|..|{ AUTHOR : ""
BOOK }|..|{ GENRE : " "
AUTHOR }|..|{ GENRE : "  "
"#,
        &SvgRenderOptions::default(),
    );
    let labels = edge_labels_group(&svg);
    assert_eq!(labels.matches(r#"<g class="edgeLabel""#).count(), 2);
    assert!(!labels.contains("undefined"));
    assert!(!labels.contains("NaN"));
    assert_eq!(labels.matches(r#"width="0" height="0""#).count(), 2);
}

#[test]
fn er_svg_row_fills_follow_optional_theme_colors() {
    let row_re = Regex::new(r#"<g[^>]*class="row-rect-(?:odd|even)"[^>]*>(.*?)</g>"#).unwrap();
    for (variables, expected_fills) in [
        (serde_json::json!({}), vec![]),
        (
            serde_json::json!({"rowOdd": "#123456", "rowEven": "#abcdef"}),
            vec!["#123456", "#abcdef"],
        ),
        (serde_json::json!({"rowOdd": "none", "rowEven": ""}), vec![]),
    ] {
        let config = serde_json::json!({"theme": "redux", "themeVariables": variables});
        let text = format!(
            "%%{{init: {config}}}%%\nerDiagram\n BOOK {{\n string title\n int pages\n }}\n"
        );
        let svg = render_er_svg_from_text(&text, &SvgRenderOptions::default());
        let rows = row_re.captures_iter(&svg).collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(
                row[1].matches("<path ").count(),
                if expected_fills.is_empty() { 1 } else { 2 }
            );
            if let Some(fill) = expected_fills.get(index) {
                assert!(row[1].contains(&format!(r#"fill="{fill}""#)));
            }
        }
    }
}

#[test]
fn er_derived_theme_colors_do_not_claim_raw_paint_defaults() {
    let theme = er_relation_rules_theme([
        ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        )
        .with_variant(ThemeVariant::Default),
        ThemeRule::new(
            ThemeTarget::Relation,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#654321").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::Table,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#abcdef").unwrap()),
        ),
    ]);
    let rendered = prepare_er_family_with_theme_and_engine(
        "---\ntitle: Raw owners\n---\nerDiagram\n A {\n string id\n string name\n }\n A ||--o{ B : owns\n", &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({"theme": "base", "themeVariables": {"primaryColor": "#fedcba"}}))),
    ).render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()).unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert_eq!(
        er_row_path(&document, true).attribute("style"),
        Some("fill:#abcdef")
    );
    assert_eq!(
        er_row_path(&document, false).attribute("style"),
        Some("fill:#abcdef")
    );
    let paths = relationship_path_tags(rendered.svg());
    assert_eq!(paths.len(), 1);
    assert!(paths[0].contains("stroke:#654321"));
    let title = document
        .descendants()
        .find(|node| node.attribute("class") == Some("erDiagramTitleText"))
        .unwrap();
    assert_eq!(title.attribute("style"), Some("color:#123456;fill:#123456"));
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 3);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn er_redux_explicit_styles_override_palette_on_every_table_path() {
    for theme in ["redux", "redux-dark", "redux-color", "redux-dark-color"] {
        for look in ["classic", "neo"] {
            for html_labels in [false, true] {
                let source = format!(
                    r#"---
config:
  theme: {theme}
  look: {look}
  layout: dagre
  htmlLabels: {html_labels}
---
erDiagram
  BOOK["Book"]:::core {{
    string title PK
    string author FK
  }}
  classDef core fill:#f96,stroke:#456,stroke-width:3px,color:#fff
  class BOOK core
  style BOOK fill:#f9f,stroke:#333,stroke-width:2px
"#
                );
                let svg = render_er_svg_from_text(&source, &SvgRenderOptions::default());
                let document = roxmltree::Document::parse(&svg).unwrap();
                let entity = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some("merman-entity-BOOK-0"))
                    .expect("BOOK entity");
                for path in entity
                    .descendants()
                    .filter(|node| node.has_tag_name("path"))
                {
                    let style = path.attribute("style").unwrap_or_default();
                    assert!(
                        style.contains("fill:#f9f !important"),
                        "{theme}/{look}: {style}"
                    );
                    assert!(
                        style.contains("stroke:#333 !important"),
                        "{theme}/{look}: {style}"
                    );
                    assert!(
                        style.contains("stroke-width:2px !important"),
                        "{theme}/{look}: {style}"
                    );
                    assert!(
                        !style.contains("#f96"),
                        "class fill must lose to explicit style"
                    );
                }
            }
        }
    }
}

#[test]
fn er_non_redux_preserves_row_colors_and_styles_both_divider_paths() {
    let source = r#"---
config:
  theme: default
  look: neo
  layout: dagre
---
erDiagram
  BOOK {
    string title PK
    string author FK
  }
  style BOOK fill:#f9f,stroke:#333,stroke-width:2px,stroke-dasharray:4 2
"#;
    let svg = render_er_svg_from_text(source, &SvgRenderOptions::default());
    let document = roxmltree::Document::parse(&svg).unwrap();
    for group in document.descendants().filter(|node| {
        matches!(
            node.attribute("class"),
            Some("outer-path" | "row-rect-odd" | "row-rect-even" | "divider")
        )
    }) {
        for path in group.children().filter(|node| node.has_tag_name("path")) {
            let style = path.attribute("style").unwrap_or_default();
            assert!(
                style.contains("stroke:#333 !important"),
                "{}: {style}",
                group.attribute("class").unwrap()
            );
            // Mermaid erDiagram.jison skips whitespace in style blocks and joins tokens.
            assert!(style.contains("stroke-dasharray:42 !important"), "{style}");
            assert_eq!(
                style.contains("fill:#f9f !important"),
                group.attribute("class") == Some("row-rect-even")
            );
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_elk_paints_ancestors_before_descendants_in_stable_model_order() {
    let source = r#"---
config:
  layout: elk
---
erDiagram
subgraph ZOuter [Outer]
  subgraph AInner [Inner]
    subgraph Deepest [Deep]
      A
    end
    B
  end
  subgraph ZSibling [Sibling]
    C
  end
end
subgraph BPeer [Peer]
  D
end
A ||--|| C : crosses_siblings
B ||--|| D : crosses_roots
"#;
    let svg = render_er_svg_from_text(source, &SvgRenderOptions::default());
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let clusters = document
        .descendants()
        .find(|node| node.attribute("class") == Some("clusters"))
        .expect("clusters group");
    let ids: Vec<_> = clusters
        .children()
        .filter_map(|node| node.attribute("id"))
        .collect();
    // ER builds its model from reverse subgraph completion order. Stable depth ordering
    // preserves that sibling order while moving every parent before its descendants.
    assert_eq!(
        ids,
        [
            "merman-BPeer",
            "merman-ZOuter",
            "merman-ZSibling",
            "merman-AInner",
            "merman-Deepest"
        ]
    );
    let last_cluster = svg.find(r#"id="merman-Deepest""#).unwrap();
    let leaf_nodes = svg.find(r#"<g class="nodes">"#).unwrap();
    assert!(last_cluster < leaf_nodes);
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .count(),
        2
    );
    assert!(svg.contains("crosses_siblings") && svg.contains("crosses_roots"));
}

#[test]
fn er_svg_label_mode_covers_containers_entities_and_attribute_cells() {
    let body = r#"erDiagram
subgraph ZOuter [Outer Namespace]
  subgraph AInner [Inner Namespace]
    BOOK["Book **Title**"] {
      string name PK "visible comment"
      int edition FK
      type~T~ generic UK "generic value"
    }
    AUTHOR["Author Name"]
  end
end
BOOK ||--|| AUTHOR : writes
style ZOuter color:#123456
"#;
    let layouts = if cfg!(feature = "layout-elk") {
        vec!["dagre", "elk"]
    } else {
        vec!["dagre"]
    };
    for layout in layouts {
        for html_labels in [false, true] {
            let source = format!(
                "---\nconfig:\n  layout: {layout}\n  htmlLabels: {html_labels}\n---\n{body}"
            );
            let svg = render_er_svg_from_text(&source, &SvgRenderOptions::default());
            let document = roxmltree::Document::parse(&svg).expect("valid SVG");
            let foreign_objects = document
                .descendants()
                .filter(|node| node.has_tag_name("foreignObject"))
                .count();
            assert_eq!(
                foreign_objects > 0,
                html_labels,
                "{layout}/{html_labels}: {svg}"
            );
            for id in [
                "merman-ZOuter",
                "merman-AInner",
                "merman-entity-BOOK-0",
                "merman-entity-AUTHOR-1",
            ] {
                let owner = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some(id))
                    .expect("label owner");
                assert_eq!(
                    owner
                        .descendants()
                        .any(|node| node.has_tag_name("foreignObject")),
                    html_labels,
                    "{id}: {svg}"
                );
                assert_eq!(
                    owner.descendants().any(|node| node.has_tag_name("text")),
                    !html_labels,
                    "{id}: {svg}"
                );
            }
            let visible: String = document
                .descendants()
                .filter(|node| node.is_text())
                .filter_map(|node| node.text())
                .collect();
            for text in [
                "Outer Namespace",
                "Inner Namespace",
                "Book Title",
                "Author Name",
                "string",
                "name",
                "PK",
                "visible comment",
                "int",
                "edition",
                "FK",
                "type<T>",
                "generic value",
                "writes",
            ] {
                assert!(
                    visible.contains(text),
                    "missing {text:?} in {layout}/{html_labels}: {visible}"
                );
            }
            let outer = document
                .descendants()
                .find(|node| node.attribute("id") == Some("merman-ZOuter"))
                .expect("outer group");
            let title = outer
                .descendants()
                .find(|node| node.has_tag_name(if html_labels { "span" } else { "text" }))
                .expect("styled title");
            assert!(
                title
                    .attribute("style")
                    .unwrap_or_default()
                    .contains(if html_labels {
                        "color:#123456"
                    } else {
                        "fill:#123456"
                    })
            );
            for class in [
                "label name",
                "label attribute-type",
                "label attribute-name",
                "label attribute-keys",
                "label attribute-comment",
            ] {
                let label = document
                    .descendants()
                    .find(|node| node.attribute("class") == Some(class))
                    .expect("attribute label");
                assert!(
                    label
                        .descendants()
                        .any(|node| node.has_tag_name(if html_labels {
                            "foreignObject"
                        } else {
                            "text"
                        })),
                    "{class}: {svg}"
                );
            }
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_elk_line_hops_change_only_crossing_paths() {
    use std::collections::BTreeMap;
    let body = r#"erDiagram
A ||--|| X : links
A ||--|| Y : links
A ||--|| Z : links
B ||--|| X : links
B ||--|| Y : links
B ||--|| Z : links
C ||--|| X : links
C ||--|| Y : links
C ||--|| Z : links
"#;
    let render = |mode: &str| {
        render_er_svg_from_text(
            &format!("---\nconfig:\n  layout: elk\n  elk:\n    lineHops: {mode}\n---\n{body}"),
            &SvgRenderOptions::default(),
        )
    };
    let without = render("false");
    let arcs = render("true");
    let gaps = render("gap");
    let paths = |svg: &str| {
        let document = roxmltree::Document::parse(svg).expect("valid SVG");
        document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .map(|node| {
                (
                    node.attribute("id").unwrap().to_string(),
                    (
                        node.attribute("d").unwrap().to_string(),
                        node.attribute("marker-start").unwrap().to_string(),
                        node.attribute("marker-end").unwrap().to_string(),
                        node.attribute("data-points").unwrap().to_string(),
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>()
    };
    let plain_paths = paths(&without);
    let arc_paths = paths(&arcs);
    let gap_paths = paths(&gaps);
    assert_eq!(plain_paths.len(), 9);
    let mut changed = 0;
    let mut unchanged = 0;
    for (id, plain) in &plain_paths {
        let arc = &arc_paths[id];
        let gap = &gap_paths[id];
        assert_eq!((&plain.1, &plain.2, &plain.3), (&arc.1, &arc.2, &arc.3));
        assert_eq!((&plain.1, &plain.2, &plain.3), (&gap.1, &gap.2, &gap.3));
        if plain.0 != arc.0 {
            changed += 1;
            assert_ne!(plain.0, gap.0);
            assert_ne!(arc.0, gap.0);
            assert!(arc.0.contains('A'), "expected jump arc for {id}: {}", arc.0);
            assert!(
                gap.0.matches('M').count() > 1,
                "expected a discontinuity for {id}: {}",
                gap.0
            );
        } else {
            unchanged += 1;
            assert_eq!(plain.0, gap.0);
        }
    }
    assert!(changed > 0 && unchanged > 0);
    assert_eq!(edge_labels_group(&without), edge_labels_group(&arcs));
    assert_eq!(edge_labels_group(&without), edge_labels_group(&gaps));
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_elk_neo_hops_update_solid_and_dashed_masks_from_final_path_length() {
    use kurbo::Shape;
    use std::collections::BTreeMap;

    for dashed in [false, true] {
        let mut body = String::from("erDiagram\n");
        let relationship = if dashed { "||..||" } else { "||--||" };
        for source in ["A", "B", "C"] {
            for target in ["X", "Y", "Z"] {
                body.push_str(&format!("{source} {relationship} {target} : links\n"));
            }
        }
        let render = |mode: &str| {
            render_er_svg_from_text(
                &format!(
                    "---\nconfig:\n  layout: elk\n  look: neo\n  elk:\n    lineHops: {mode}\n---\n{body}"
                ),
                &SvgRenderOptions::default(),
            )
        };
        let without = render("false");
        let original = roxmltree::Document::parse(&without).expect("original SVG");
        let original_edges: BTreeMap<_, _> = original
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .map(|node| (node.attribute("id").unwrap(), node))
            .collect();

        for mode in ["true", "gap"] {
            let svg = render(mode);
            let document = roxmltree::Document::parse(&svg).expect("hopped SVG");
            let mut changed = 0;
            let mut unchanged = 0;
            for edge in document
                .descendants()
                .filter(|node| node.attribute("data-edge") == Some("true"))
            {
                let id = edge.attribute("id").unwrap();
                let before = original_edges[id];
                let d = edge.attribute("d").unwrap();
                let style = edge.attribute("style").unwrap();
                for attribute in ["marker-start", "marker-end", "data-points"] {
                    assert_eq!(
                        before.attribute(attribute),
                        edge.attribute(attribute),
                        "{id}/{mode}/{attribute}"
                    );
                }
                if d == before.attribute("d").unwrap() {
                    unchanged += 1;
                    assert_eq!(style, before.attribute("style").unwrap(), "{id}/{mode}");
                    continue;
                }
                changed += 1;
                // getTotalLength() returns an SVG DOM float before marker-offset arithmetic.
                let length =
                    f64::from(kurbo::BezPath::from_svg(d).unwrap().perimeter(1.0e-6) as f32);
                let dasharray: Vec<f64> = style
                    .split(';')
                    .find_map(|declaration| declaration.trim().strip_prefix("stroke-dasharray:"))
                    .expect("Neo mask")
                    .split_whitespace()
                    .map(|value| value.parse().expect("dash length"))
                    .collect();
                // lineJump.ts reads offsets from the first four original dash values.
                // For a repeated Neo 2/2 pattern the fourth value is 2, not its final 0.
                let end_offset = if dashed { 2.0 } else { 0.0 };
                assert_eq!(dasharray.len(), 4, "{id}/{mode}: {style}");
                assert_eq!(
                    (dasharray[0], dasharray[1], dasharray[3]),
                    (0.0, 0.0, end_offset)
                );
                assert!(
                    (dasharray[2] - (length - end_offset).max(0.0)).abs() < 0.001,
                    "{id}/{mode}: length={length}, style={style}"
                );
                assert!(style.contains("stroke-dashoffset: 0;"));
                assert!(
                    !style.contains(";;"),
                    "upstream collapses empty declarations: {style}"
                );
            }
            assert!(changed > 0 && unchanged > 0, "dashed={dashed}/{mode}");
            assert_eq!(edge_labels_group(&without), edge_labels_group(&svg));
        }
    }
}
