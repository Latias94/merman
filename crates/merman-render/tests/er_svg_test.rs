use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, GradientStop,
    LinearGradient, OrdinalSelector, PatternKind, PatternSpec, Specified, ThemeColorValue,
    ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    ThemeVariant, materialize_theme,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgPipeline, SvgRenderOptions};
use merman_theme_contract::{ThemeColorTokenV1, ThemeDefinitionV1, ThemeTokensV1};
use regex::Regex;
use serde_json::json;
use std::path::PathBuf;

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
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(text, ParseOptions::strict())
        .expect("parse themed ER diagram")
        .expect("detect themed ER diagram");
    let session = RenderEnvironment::deterministic()
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
fn er_svg_recursive_relationship_keeps_three_segments_and_one_label() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join(
            "upstream_cypress_erdiagram_spec_should_render_an_er_diagram_with_a_recursive_relationship_002.mmd",
        );
    let text = std::fs::read_to_string(path).expect("fixture");

    let svg = render_er_svg_from_text(&text, &SvgRenderOptions::default());

    for edge_id in [
        "entity-CUSTOMER-0-cyclic-special-1",
        "entity-CUSTOMER-0-cyclic-special-mid",
        "entity-CUSTOMER-0-cyclic-special-2",
    ] {
        assert!(
            svg.contains(&format!(r#"data-id="{edge_id}""#)),
            "missing recursive ER edge segment {edge_id}: {svg}"
        );
    }
    assert_eq!(
        svg.matches(">refers<").count(),
        1,
        "only the middle recursive segment should own the relationship label: {svg}"
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
fn er_static_relation_stroke_binds_all_recursive_segments_and_terminal_markers() {
    let theme = er_relation_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid recursive ER relation stroke"),
    );
    let svg = render_er_svg_from_text_with_theme("erDiagram\n  A ||--o{ A : refers\n", &theme);
    let paths = relationship_path_tags(&svg);
    assert_eq!(paths.len(), 3, "recursive ER path count: {svg}");
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
    let legacy_cases = [
        ThemeRule::new(ThemeTarget::Relation, solid()).with_variant(ThemeVariant::Default),
        ThemeRule::new(ThemeTarget::Relation, fill()),
        ThemeRule::new(ThemeTarget::Title, fill()),
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
