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
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let parsed = Engine::new()
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
    assert!(svg.contains(r#"data-look="classic""#));
    assert!(svg.contains(r#"id="merman-id_entity-BOOK-0_entity-PAGE-1_0""#));
    assert!(svg.contains(r#"id="merman-drop-shadow""#));
    assert!(svg.contains("relationshipLine"));
    assert!(
        Regex::new(
            r#"<path[^>]*class="[^"]*relationshipLine[^"]*" style="undefined;;;undefined"[^>]*>"#
        )
        .expect("relationship path regex")
        .is_match(&svg),
        "relationship paths should preserve Mermaid's exact empty pathStyle serialization"
    );
    assert!(svg.contains("relationshipLabelBox"));
    assert!(
        svg.contains("marker") && svg.contains("merman_er-zeroOrMoreStart"),
        "expected Mermaid-like marker ids"
    );
    assert!(
        {
            let path_re = Regex::new(r#"<path[^>]*relationshipLine[^>]*>"#).expect("regex");
            let d_re = Regex::new(r#"\bd="[^"]*C"#).expect("regex");
            path_re.find_iter(&svg).any(|m| d_re.is_match(m.as_str()))
        },
        "expected curveBasis cubic bezier commands in relationship paths"
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

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());

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

    let svg = render_er_svg_from_text(&text, &SvgRenderOptions::default());
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

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());
    let html_svg = render_er_svg_from_text(html_text, &SvgRenderOptions::default());
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
    let source = r#"erDiagram
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
        .parse_diagram_for_render_model_sync("erDiagram\n  CUSTOMER\n", ParseOptions::strict())
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

#[test]
fn er_table_odd_even_remain_legacy_compatibility_until_qualified_cutover() {
    let source = r#"erDiagram
  CUSTOMER {
    string id PK
    string name
  }
"#;
    let theme = er_table_row_theme();

    let error = match try_prepare_er_family_with_theme_and_engine(source, &theme, Engine::new()) {
        Ok(_) => panic!("ER table Odd/Even routes must remain compatibility residuals"),
        Err(error) => error,
    };
    match error {
        merman_render::Error::LegacyFamilyThemeCompatibility {
            family_id,
            residual_count,
        } => {
            assert_eq!(family_id, merman_render::DiagramFamilyId::ER);
            assert_eq!(residual_count, 2);
        }
        other => panic!("expected ER table compatibility residual, got {other}"),
    }

    let artifact = try_prepare_er_family_with_theme_and_engine_requirement(
        source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("BestEffort ER table rendering should retain the compatibility bridge");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render ER table compatibility bridge");
    let svg = rendered.svg();

    assert!(
        Regex::new(r##"class="row-rect-odd"[^>]*><path[^>]*fill="#aabbcc""##)
            .expect("ER odd bridge fill regex")
            .is_match(svg),
        "odd ER row must retain rowOdd compatibility projection: {svg}"
    );
    assert!(
        Regex::new(r##"class="row-rect-even"[^>]*><path[^>]*fill="#ddeeff""##)
            .expect("ER even bridge fill regex")
            .is_match(svg),
        "even ER row must retain rowEven compatibility projection: {svg}"
    );
    assert!(
        !svg.contains("style=\"fill:#aabbcc") && !svg.contains("style=\"fill:#ddeeff"),
        "ER table rows must not claim direct typed inline fills before qualified cutover: {svg}"
    );
}

#[test]
fn er_source_entity_styles_outrank_typed_entity_paint() {
    let source = r#"erDiagram
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
        "erDiagram\n  DEFAULT\n  classDef default fill:#aa0000,stroke:#bb0000\n",
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
        prepare_er_family_with_theme_and_engine("erDiagram\n  PLAIN\n", &theme, Engine::new())
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
    let fill = || {
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#654321").expect("valid ER relation fill"))
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
    let legacy_cases = [ThemeRule::new(ThemeTarget::Relation, fill())];

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

    for rule in legacy_cases {
        let theme = er_relation_rules_theme([rule]);
        let error = match try_prepare_er_family_with_theme_and_engine(
            "erDiagram\n  A ||--o{ B : owns\n",
            &theme,
            Engine::new(),
        ) {
            Ok(_) => panic!("legacy ER theme routes must remain strict compatibility residuals"),
            Err(error) => error,
        };
        match error {
            merman_render::Error::LegacyFamilyThemeCompatibility {
                family_id,
                residual_count,
            } => {
                assert_eq!(family_id, merman_render::DiagramFamilyId::ER);
                assert_eq!(residual_count, 1);
            }
            other => panic!("expected ER legacy compatibility residual, got {other}"),
        }
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
