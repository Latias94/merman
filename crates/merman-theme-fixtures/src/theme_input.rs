use crate::error::CatalogError;
use crate::io::unicode_range_contains;
use crate::model::{
    AssetRecord, ReferenceBorderInput, ReferenceCanvasLayer, ReferenceDiagramFamily,
    ReferenceFontBinding, ReferenceFontStack, ReferenceGradientRepetition, ReferenceGradientStop,
    ReferenceNodeStyleInput, ReferenceSemanticRule, ReferenceSemanticStylePatch,
    ReferenceSemanticTarget, ReferenceShadowInput, ReferenceSvgEffect, ReferenceThemeInput,
    ReferenceThemeMechanism, ReferenceThemeTokens, ReferenceTypographyInput, THEME_INPUT_VERSION,
};
use crate::source_compatibility::{
    ThemeCssEvidence, collect_source_compatibility_mechanisms, collect_theme_css_evidence,
    has_flowchart_node_theme_variable, has_state_node_theme_variable, is_paint_declaration,
    is_typography_declaration,
};
use crate::wire::{
    ReferenceBorderInputWire, ReferenceCanvasLayerWire, ReferenceGradientStopWire,
    ReferenceSemanticRuleWire, ReferenceSemanticStylePatchWire, ReferenceShadowInputWire,
    ReferenceSvgEffectWire, ReferenceThemeInputWire,
};
use merman_core::{Engine, MermaidConfig, ParseOptions, RenderSemanticModel};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct ParsedFixtureSource {
    pub(crate) family: ReferenceDiagramFamily,
    pub(crate) visible_text: Vec<String>,
    pub(crate) reference_mechanisms: BTreeSet<ReferenceThemeMechanism>,
    pub(crate) style_evidence_ids: BTreeSet<String>,
}

impl ReferenceThemeTokens {
    pub fn background(&self) -> &str {
        &self.background
    }

    pub fn surface(&self) -> &str {
        &self.surface
    }

    pub fn primary(&self) -> &str {
        &self.primary
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

impl ReferenceTypographyInput {
    pub fn font_stack(&self) -> Option<&ReferenceFontStack> {
        self.font_stack.as_ref()
    }

    pub const fn letter_spacing_milli_em(&self) -> Option<i16> {
        self.letter_spacing_milli_em
    }

    pub const fn text_transform(&self) -> Option<crate::ReferenceTextTransform> {
        self.text_transform
    }
}

impl ReferenceGradientStop {
    pub const fn offset_percent(&self) -> u8 {
        self.offset_percent
    }

    pub fn color(&self) -> &str {
        &self.color
    }
}

impl ReferenceBorderInput {
    pub fn color(&self) -> &str {
        &self.color
    }

    pub const fn width_px(&self) -> u16 {
        self.width_px
    }
}

impl ReferenceShadowInput {
    pub const fn offset_x_px(&self) -> i16 {
        self.offset_x_px
    }

    pub const fn offset_y_px(&self) -> i16 {
        self.offset_y_px
    }

    pub const fn blur_px(&self) -> u16 {
        self.blur_px
    }

    pub const fn spread_px(&self) -> u16 {
        self.spread_px
    }

    pub fn color(&self) -> &str {
        &self.color
    }
}

impl ReferenceNodeStyleInput {
    pub const fn border(&self) -> Option<&ReferenceBorderInput> {
        self.border.as_ref()
    }

    pub fn dash_pattern(&self) -> &[u16] {
        &self.dash_pattern
    }

    pub const fn corner_radius_px(&self) -> Option<u16> {
        self.corner_radius_px
    }

    pub const fn shadow(&self) -> Option<&ReferenceShadowInput> {
        self.shadow.as_ref()
    }
}

impl ReferenceSemanticStylePatch {
    pub fn fill(&self) -> Option<&str> {
        self.fill.as_deref()
    }

    pub const fn border(&self) -> Option<&ReferenceBorderInput> {
        self.border.as_ref()
    }

    pub const fn corner_radius_px(&self) -> Option<u16> {
        self.corner_radius_px
    }

    pub const fn shadow(&self) -> Option<&ReferenceShadowInput> {
        self.shadow.as_ref()
    }

    pub fn text_color(&self) -> Option<&str> {
        self.text_color.as_deref()
    }

    pub const fn font_weight(&self) -> Option<u16> {
        self.font_weight
    }
}

impl ReferenceThemeInput {
    pub const fn tokens(&self) -> Option<&ReferenceThemeTokens> {
        self.tokens.as_ref()
    }

    pub const fn typography(&self) -> Option<&ReferenceTypographyInput> {
        self.typography.as_ref()
    }

    pub fn canvas(&self) -> &[ReferenceCanvasLayer] {
        &self.canvas
    }

    pub const fn node_style(&self) -> Option<&ReferenceNodeStyleInput> {
        self.node_style.as_ref()
    }

    pub fn semantic_rules(&self) -> &[ReferenceSemanticRule] {
        &self.semantic_rules
    }
}

pub(crate) fn parse_fixture_source(
    fixture_id: &str,
    source: &str,
) -> Result<ParsedFixtureSource, CatalogError> {
    let engine = source_compatibility_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .map_err(|error| CatalogError::InvalidFixtureSource {
            fixture: fixture_id.to_string(),
            message: error.to_string(),
        })?
        .ok_or_else(|| CatalogError::InvalidFixtureSource {
            fixture: fixture_id.to_string(),
            message: "source does not contain a Mermaid diagram".to_string(),
        })?;
    let has_init_directive = source_has_init_directive(source);
    let has_frontmatter_config = source_has_frontmatter_visual_config(source);
    let css_evidence = collect_theme_css_evidence(parsed.metadata().effective_config.as_value());
    if css_evidence.invalid {
        return Err(CatalogError::UnmodeledThemeCss {
            fixture: fixture_id.to_string(),
            message: "every rule, selector, and declaration must use the admitted typed subset"
                .to_string(),
        });
    }
    let mut visible = BTreeSet::new();
    let mut style_evidence_ids = BTreeSet::new();
    collect_global_source_style_evidence(
        parsed.metadata().effective_config.as_value(),
        &css_evidence,
        has_init_directive,
        has_frontmatter_config,
        &mut style_evidence_ids,
    );
    let family = match parsed.model() {
        RenderSemanticModel::Flowchart(model) => {
            collect_flowchart_visible_text(model, parsed.flowchart_render_context(), &mut visible);
            collect_flowchart_style_evidence(
                model,
                parsed.metadata().config.as_value(),
                parsed.metadata().effective_config.as_value(),
                &mut style_evidence_ids,
            );
            ReferenceDiagramFamily::Flowchart
        }
        RenderSemanticModel::State(model) => {
            collect_state_visible_text(model, &mut visible);
            collect_state_style_evidence(
                model,
                parsed.metadata().config.as_value(),
                parsed.metadata().effective_config.as_value(),
                &mut style_evidence_ids,
            );
            ReferenceDiagramFamily::StateDiagram
        }
        RenderSemanticModel::Class(model) => {
            collect_class_visible_text(model, &mut visible);
            let facts = parsed.class_style_precedence_facts().ok_or_else(|| {
                CatalogError::InvalidFixtureSource {
                    fixture: fixture_id.to_string(),
                    message: "class parser did not retain style precedence evidence".to_string(),
                }
            })?;
            collect_class_style_evidence(facts, &mut style_evidence_ids);
            ReferenceDiagramFamily::ClassDiagram
        }
        RenderSemanticModel::Er(model) => {
            collect_er_visible_text(model, &mut visible);
            collect_er_style_evidence(model, &mut style_evidence_ids);
            ReferenceDiagramFamily::ErDiagram
        }
        RenderSemanticModel::Sequence(model) => {
            collect_sequence_visible_text(model, &mut visible);
            ReferenceDiagramFamily::Sequence
        }
        _ => {
            let value = parsed.metadata().diagram_type.as_str();
            return Err(CatalogError::InvalidFixtureSource {
                fixture: fixture_id.to_string(),
                message: format!("unsupported fixture diagram family `{value}`"),
            });
        }
    };
    Ok(ParsedFixtureSource {
        family,
        visible_text: visible.into_iter().collect(),
        reference_mechanisms: collect_source_compatibility_mechanisms(
            family,
            parsed.metadata().config.as_value(),
            parsed.metadata().effective_config.as_value(),
            &css_evidence,
        ),
        style_evidence_ids,
    })
}

fn source_compatibility_engine() -> Engine {
    // These licensed, hash-bound fixtures intentionally exercise Mermaid source-owned visual
    // configuration. Keep security/resource policy keys protected while admitting the visual
    // keys into effective_config so evidence proves an applied source mechanism, not just a
    // retained-but-filtered directive payload.
    Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "secure": [
            "secure",
            "securityLevel",
            "startOnLoad",
            "maxTextSize",
            "suppressErrorRendering",
            "maxEdges"
        ]
    })))
}

fn source_has_init_directive(source: &str) -> bool {
    init_directive_values(source)
        .iter()
        .any(has_visual_config_override)
}

fn source_has_frontmatter_visual_config(source: &str) -> bool {
    let Some(frontmatter) = merman_core::preprocess::split_frontmatter_block(source) else {
        return false;
    };
    let Ok(fields) =
        merman_core::preprocess::parse_frontmatter_yaml_fields(frontmatter.dedented_body.as_ref())
    else {
        return false;
    };
    has_visual_config_override(&Value::Object(fields))
}

fn has_visual_config_override(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.iter().any(|(key, value)| {
        if key == "config" {
            return has_visual_config_override(value);
        }
        if matches!(
            key.as_str(),
            "theme"
                | "themeVariables"
                | "themeCSS"
                | "look"
                | "fontFamily"
                | "fontSize"
                | "htmlLabels"
                | "flowchart"
                | "state"
                | "class"
                | "sequence"
                | "er"
        ) {
            return has_nonempty_visual_value(value);
        }
        false
    })
}

fn has_nonempty_visual_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(value) => !value.trim().is_empty(),
        Value::Array(values) => values.iter().any(has_nonempty_visual_value),
        Value::Object(values) => values.values().any(has_nonempty_visual_value),
        Value::Bool(_) | Value::Number(_) => true,
    }
}

fn init_directive_values(source: &str) -> Vec<Value> {
    let mut values = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find("%%{") {
        let start = cursor + relative;
        let body_start = start + 3;
        let Some(end) = directive_end(source, body_start) else {
            break;
        };
        let body = &source[body_start..end];
        if let Some((kind, payload)) = body.split_once(':')
            && matches!(kind.trim(), "init" | "initialize")
            && let Ok(value) = json5::from_str::<Value>(payload.trim())
        {
            values.push(value);
        }
        cursor = end + 3;
    }
    values
}

fn directive_end(source: &str, body_start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 1usize;
    let mut quote = None;
    let mut index = body_start;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active) = quote {
            if byte == b'\\' {
                index = index.checked_add(2)?;
                continue;
            }
            if byte == active {
                quote = None;
            }
            index += 1;
            continue;
        }
        match byte {
            b'\'' | b'"' => quote = Some(byte),
            b'{' => depth = depth.checked_add(1)?,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 && source.get(index + 1..index + 3) == Some("%%") {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn collect_flowchart_style_evidence(
    model: &merman_core::diagrams::flowchart::FlowchartModel,
    config: &Value,
    effective_config: &Value,
    evidence: &mut BTreeSet<String>,
) {
    if has_flowchart_node_theme_variable(config, effective_config) {
        evidence.insert("flow-node-theme".to_string());
    }
    if model
        .class_defs
        .values()
        .any(|styles| styles.iter().any(|style| is_paint_declaration(style)))
    {
        evidence.insert("global-css-classdef".to_string());
    }
    if model
        .class_defs
        .get("default")
        .is_some_and(|styles| styles.iter().any(|style| is_paint_declaration(style)))
    {
        evidence.insert("flow-node-default".to_string());
    }
    if model.class_defs.iter().any(|(name, styles)| {
        name != "default" && styles.iter().any(|style| is_paint_declaration(style))
    }) {
        evidence.insert("flow-node-named".to_string());
    }
    if model.nodes.iter().any(|node| {
        node.classes.iter().any(|class_name| {
            class_name != "default"
                && model
                    .class_defs
                    .get(class_name)
                    .is_some_and(|styles| styles.iter().any(|style| is_paint_declaration(style)))
        })
    }) {
        evidence.insert("flow-node-assigned".to_string());
    }
    if model
        .nodes
        .iter()
        .any(|node| node.styles.iter().any(|style| is_paint_declaration(style)))
    {
        evidence.insert("flow-node-inline".to_string());
    }
    if model.edge_defaults.as_ref().is_some_and(|defaults| {
        defaults
            .style
            .iter()
            .any(|style| is_paint_declaration(style))
    }) {
        evidence.insert("flow-edge-default".to_string());
    }
    if model
        .edges
        .iter()
        .any(|edge| edge.style.iter().any(|style| is_paint_declaration(style)))
    {
        evidence.insert("flow-edge-specific".to_string());
    }
    if !evidence.is_empty() {
        evidence.insert("global-css-family".to_string());
        evidence.insert("global-css-family-typography-residual".to_string());
    }
}

fn collect_class_style_evidence(
    facts: &merman_core::models::class_diagram::ClassStylePrecedenceFacts,
    evidence: &mut BTreeSet<String>,
) {
    if facts
        .assignment_before_definition_copy_witness()
        .is_some_and(valid_class_style_witness)
    {
        evidence.insert("class-assignment-before-definition-copy".to_string());
    }
    if facts
        .definition_before_assignment_no_backfill_witness()
        .is_some_and(valid_class_style_witness)
    {
        evidence.insert("class-definition-before-assignment-no-backfill".to_string());
    }
    if facts
        .inline_paint_witness()
        .is_some_and(|witness| valid_class_declaration_witness(witness, is_paint_declaration))
    {
        evidence.insert("class-node-inline".to_string());
    }
    if facts
        .classdef_typography_witness()
        .is_some_and(|witness| valid_class_declaration_witness(witness, is_typography_declaration))
    {
        evidence.insert("class-typography-classdef-residual".to_string());
    }
}

fn valid_class_declaration_witness(
    witness: &merman_core::models::class_diagram::ClassStyleDeclarationWitness,
    valid: impl Fn(&str) -> bool,
) -> bool {
    !witness.target().trim().is_empty() && witness.styles().iter().any(|style| valid(style))
}

fn collect_state_style_evidence(
    model: &merman_core::diagrams::state::StateDiagramRenderModel,
    config: &Value,
    effective_config: &Value,
    evidence: &mut BTreeSet<String>,
) {
    if has_state_node_theme_variable(config, effective_config) {
        evidence.insert("state-node-theme".to_string());
    }
    if model.states.values().any(|state| {
        state.classes.iter().any(|class_name| {
            model
                .style_classes
                .get(class_name)
                .is_some_and(|class| class.styles.iter().any(|style| is_paint_declaration(style)))
        })
    }) {
        evidence.insert("state-node-assigned".to_string());
    }
    if model
        .states
        .values()
        .any(|state| state.styles.iter().any(|style| is_paint_declaration(style)))
        || model.nodes.iter().any(|node| {
            node.css_styles
                .iter()
                .any(|style| is_paint_declaration(style))
        })
    {
        evidence.insert("state-node-inline".to_string());
    }
}

fn collect_global_source_style_evidence(
    effective_config: &Value,
    css: &ThemeCssEvidence,
    has_init_directive: bool,
    has_frontmatter_config: bool,
    evidence: &mut BTreeSet<String>,
) {
    if has_init_directive {
        evidence.insert("global-config-init".to_string());
    }
    if has_frontmatter_config {
        evidence.insert("global-config-frontmatter".to_string());
    }
    if css.has_qualified_rule
        || effective_config
            .get("themeCSS")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty() && !css.mechanisms.is_empty())
    {
        evidence.insert("global-css-theme".to_string());
    }
}

fn collect_er_style_evidence(
    model: &merman_core::diagrams::er::ErDiagramRenderModel,
    evidence: &mut BTreeSet<String>,
) {
    if has_er_origin_precedence_chain(model, is_paint_declaration) {
        evidence.insert("er-node-default".to_string());
        evidence.insert("er-node-assigned".to_string());
        evidence.insert("er-node-inline".to_string());
    }
    if has_er_origin_precedence_chain(model, is_typography_declaration) {
        evidence.insert("er-typography-default-residual".to_string());
        evidence.insert("er-typography-assigned-residual".to_string());
        evidence.insert("er-typography-inline-residual".to_string());
    }
}

fn has_er_origin_precedence_chain(
    model: &merman_core::diagrams::er::ErDiagramRenderModel,
    admitted: fn(&str) -> bool,
) -> bool {
    model.entities.values().any(|entity| {
        let classes = entity.css_classes.split_whitespace().collect::<Vec<_>>();
        let Some(default_index) = classes.iter().position(|name| *name == "default") else {
            return false;
        };
        let default_properties = model
            .classes
            .get("default")
            .map_or_else(BTreeSet::new, |class| {
                admitted_style_properties(
                    class
                        .styles
                        .iter()
                        .chain(&class.text_styles)
                        .map(String::as_str),
                    admitted,
                )
            });
        let assigned_properties = admitted_style_properties(
            classes
                .iter()
                .skip(default_index + 1)
                .filter_map(|name| model.classes.get(*name))
                .flat_map(|class| class.styles.iter().chain(&class.text_styles))
                .map(String::as_str),
            admitted,
        );
        let inline_properties =
            admitted_style_properties(entity.css_styles.iter().map(String::as_str), admitted);

        default_properties.iter().any(|property| {
            assigned_properties.contains(property) && inline_properties.contains(property)
        })
    })
}

fn admitted_style_properties<'a>(
    styles: impl IntoIterator<Item = &'a str>,
    admitted: fn(&str) -> bool,
) -> BTreeSet<String> {
    styles
        .into_iter()
        .filter(|style| admitted(style))
        .filter_map(merman_core::style::parse_safe_style_decl)
        .map(|(property, _)| property.trim().to_ascii_lowercase())
        .collect()
}

fn valid_class_style_witness(
    witness: &merman_core::models::class_diagram::ClassStylePrecedenceWitness,
) -> bool {
    !witness.class_name().trim().is_empty()
        && !witness.target().trim().is_empty()
        && witness
            .styles()
            .iter()
            .any(|style| is_paint_declaration(style))
        && witness.earlier_style_event_ordinal() < witness.later_style_event_ordinal()
}

fn collect_flowchart_visible_text(
    model: &merman_core::diagrams::flowchart::FlowchartModel,
    sources: Option<&merman_core::diagrams::flowchart::FlowchartRenderContext>,
    visible: &mut BTreeSet<String>,
) {
    for node in &model.nodes {
        let label = sources
            .and_then(|sources| sources.node_label_for_render(node))
            .or(node.label.as_deref())
            .unwrap_or(&node.id);
        insert_visible_text(visible, label);
    }
    for (semantic_index, edge) in model.edges.iter().enumerate() {
        if let Some(label) = sources
            .and_then(|sources| sources.edge_label_for_render(semantic_index, edge))
            .or(edge.label.as_deref())
        {
            insert_visible_text(visible, label);
        }
    }
    for (semantic_index, subgraph) in model.subgraphs.iter().enumerate() {
        let title = sources.map_or(subgraph.title.as_str(), |sources| {
            sources.subgraph_title_for_render(semantic_index, subgraph)
        });
        insert_visible_text(visible, title);
    }
}

fn collect_state_visible_text(
    model: &merman_core::diagrams::state::StateDiagramRenderModel,
    visible: &mut BTreeSet<String>,
) {
    for node in &model.nodes {
        if let Some(label) = &node.label {
            collect_json_strings(label, visible);
        } else {
            insert_visible_text(visible, &node.id);
        }
        if let Some(descriptions) = &node.description {
            for description in descriptions {
                insert_visible_text(visible, description);
            }
        }
    }
    for edge in &model.edges {
        insert_visible_text(visible, &edge.label);
    }
    for relation in &model.relations {
        if let Some(title) = &relation.relation_title {
            insert_visible_text(visible, title);
        }
    }
    for state in model.states.values() {
        for description in &state.descriptions {
            insert_visible_text(visible, description);
        }
        if let Some(note) = &state.note {
            insert_visible_text(visible, &note.text);
        }
    }
}

fn collect_class_visible_text(
    model: &merman_core::models::class_diagram::ClassDiagram,
    visible: &mut BTreeSet<String>,
) {
    for class in model.classes.values() {
        insert_visible_text(visible, &class.title_text_for_render());
        if let Some(annotation) = class.annotation_text_for_render() {
            insert_visible_text(visible, &annotation);
        }
        for member in class.members.iter().chain(&class.methods) {
            insert_visible_text(visible, &member.display_text_for_render());
        }
    }
    for relation in &model.relations {
        insert_class_visible_text(visible, &relation.title);
        if relation.relation_title_1.as_deref() != Some("none") {
            if let Some(title) = relation.relation_title_1.as_deref() {
                insert_class_visible_text(visible, title);
            }
        }
        if relation.relation_title_2.as_deref() != Some("none") {
            if let Some(title) = relation.relation_title_2.as_deref() {
                insert_class_visible_text(visible, title);
            }
        }
    }
    for note in &model.notes {
        insert_class_visible_text(visible, &note.text);
    }
    for interface in &model.interfaces {
        insert_class_visible_text(visible, &interface.label);
    }
    for namespace in model.namespaces.values() {
        let label = if namespace.label.trim().is_empty() {
            &namespace.id
        } else {
            &namespace.label
        };
        insert_visible_text(visible, label);
    }
}

fn insert_class_visible_text(visible: &mut BTreeSet<String>, source: &str) {
    let decoded = merman_core::entities::decode_entities_minimal(source.trim());
    insert_visible_text(visible, &decoded);
}

fn collect_er_visible_text(
    model: &merman_core::diagrams::er::ErDiagramRenderModel,
    visible: &mut BTreeSet<String>,
) {
    for entity in model.entities.values() {
        insert_er_visible_text(visible, entity.label_source_for_render());
        for attribute in &entity.attributes {
            insert_er_visible_text(visible, &attribute.ty);
            insert_er_visible_text(visible, &attribute.name);
            insert_er_visible_text(visible, &attribute.comment);
            if !attribute.keys.is_empty() {
                insert_er_visible_text(visible, &attribute.keys.join(","));
            }
        }
    }
    for relationship in &model.relationships {
        insert_er_visible_text(visible, &relationship.role_a);
    }
}

fn collect_sequence_visible_text(
    model: &merman_core::diagrams::sequence::SequenceDiagramRenderModel,
    visible: &mut BTreeSet<String>,
) {
    if let Some(title) = &model.title {
        insert_visible_text(visible, title);
    }
    if model.actor_order.is_empty() {
        for (actor_id, actor) in &model.actors {
            insert_sequence_actor_visible_text(visible, actor_id, actor);
        }
    } else {
        for actor_id in &model.actor_order {
            let Some(actor) = model.actors.get(actor_id) else {
                continue;
            };
            insert_sequence_actor_visible_text(visible, actor_id, actor);
        }
    }
    for sequence_box in &model.boxes {
        if let Some(name) = &sequence_box.name {
            insert_visible_text(visible, name);
        }
    }
    for message in &model.messages {
        insert_visible_text(visible, message.message_text());
    }
    for note in &model.notes {
        insert_visible_text(visible, &note.message);
    }
}

fn insert_sequence_actor_visible_text(
    visible: &mut BTreeSet<String>,
    actor_id: &str,
    actor: &merman_core::diagrams::sequence::SequenceActor,
) {
    let label = if !actor.description.trim().is_empty() {
        &actor.description
    } else if !actor.name.trim().is_empty() {
        &actor.name
    } else {
        actor_id
    };
    insert_visible_text(visible, label);
}

fn insert_er_visible_text(visible: &mut BTreeSet<String>, source: &str) {
    let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(source);
    let rendered = merman_core::common::parse_generic_types(decoded.as_ref().trim());
    insert_visible_text(visible, &rendered);
}

fn collect_json_strings(value: &Value, visible: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => insert_visible_text(visible, text),
        Value::Array(values) => {
            for value in values {
                collect_json_strings(value, visible);
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                collect_json_strings(value, visible);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn insert_visible_text(visible: &mut BTreeSet<String>, text: &str) {
    let text = text.trim();
    if !text.is_empty() {
        visible.insert(text.to_string());
    }
}

pub(crate) fn convert_theme_input(
    fixture_id: &str,
    wire: ReferenceThemeInputWire,
    source_family: ReferenceDiagramFamily,
) -> Result<ReferenceThemeInput, CatalogError> {
    if wire.theme_input_version != THEME_INPUT_VERSION || wire.fixture_id != fixture_id {
        return Err(CatalogError::ThemeInputMismatch(fixture_id.to_string()));
    }
    let input = ReferenceThemeInput {
        fixture_id: wire.fixture_id,
        tokens: wire.tokens.map(|tokens| ReferenceThemeTokens {
            background: tokens.background,
            surface: tokens.surface,
            primary: tokens.primary,
            text: tokens.text,
        }),
        typography: wire.typography.map(|typography| ReferenceTypographyInput {
            font_stack: typography.font_stack.map(|stack| ReferenceFontStack {
                binding: stack.binding,
                families: stack.families,
                asset_ids: stack.asset_ids,
            }),
            letter_spacing_milli_em: typography.letter_spacing_milli_em,
            text_transform: typography.text_transform,
        }),
        canvas: wire.canvas.into_iter().map(convert_canvas_layer).collect(),
        blend_mode: wire.blend_mode,
        node_style: wire.node_style.map(|style| ReferenceNodeStyleInput {
            border: style.border.map(convert_border),
            dash_pattern: style.dash_pattern,
            corner_radius_px: style.corner_radius_px,
            shadow: style.shadow.map(convert_shadow),
        }),
        semantic_rules: wire
            .semantic_rules
            .into_iter()
            .map(convert_semantic_rule)
            .collect(),
        svg_effects: wire
            .svg_effects
            .into_iter()
            .map(|effect| match effect {
                ReferenceSvgEffectWire::TurbulenceDisplacement {
                    base_frequency_milli,
                    octaves,
                    scale,
                } => ReferenceSvgEffect::TurbulenceDisplacement {
                    base_frequency_milli,
                    octaves,
                    scale,
                },
            })
            .collect(),
        residual_mechanisms: wire.residual_mechanisms,
    };
    validate_theme_input(&input, source_family)?;
    Ok(input)
}

fn convert_canvas_layer(layer: ReferenceCanvasLayerWire) -> ReferenceCanvasLayer {
    match layer {
        ReferenceCanvasLayerWire::LinearGradient {
            angle_degrees,
            repetition,
            tile_width_px,
            tile_height_px,
            stops,
        } => ReferenceCanvasLayer::LinearGradient {
            angle_degrees,
            repetition,
            tile_width_px,
            tile_height_px,
            stops: stops.into_iter().map(convert_gradient_stop).collect(),
        },
        ReferenceCanvasLayerWire::RadialGradient {
            center_x_percent,
            center_y_percent,
            repetition,
            tile_width_px,
            tile_height_px,
            stops,
        } => ReferenceCanvasLayer::RadialGradient {
            center_x_percent,
            center_y_percent,
            repetition,
            tile_width_px,
            tile_height_px,
            stops: stops.into_iter().map(convert_gradient_stop).collect(),
        },
        ReferenceCanvasLayerWire::Solid { color } => ReferenceCanvasLayer::Solid { color },
    }
}

fn convert_gradient_stop(stop: ReferenceGradientStopWire) -> ReferenceGradientStop {
    ReferenceGradientStop {
        offset_percent: stop.offset_percent,
        color: stop.color,
    }
}

fn convert_border(border: ReferenceBorderInputWire) -> ReferenceBorderInput {
    ReferenceBorderInput {
        color: border.color,
        width_px: border.width_px,
    }
}

fn convert_shadow(shadow: ReferenceShadowInputWire) -> ReferenceShadowInput {
    ReferenceShadowInput {
        offset_x_px: shadow.offset_x_px,
        offset_y_px: shadow.offset_y_px,
        blur_px: shadow.blur_px,
        spread_px: shadow.spread_px,
        color: shadow.color,
    }
}

fn convert_semantic_patch(patch: ReferenceSemanticStylePatchWire) -> ReferenceSemanticStylePatch {
    ReferenceSemanticStylePatch {
        fill: patch.fill,
        border: patch.border.map(convert_border),
        corner_radius_px: patch.corner_radius_px,
        shadow: patch.shadow.map(convert_shadow),
        text_color: patch.text_color,
        font_weight: patch.font_weight,
    }
}

fn convert_semantic_rule(rule: ReferenceSemanticRuleWire) -> ReferenceSemanticRule {
    match rule {
        ReferenceSemanticRuleWire::HasDescendant {
            family,
            target,
            descendant,
            apply,
        } => ReferenceSemanticRule::HasDescendant {
            family,
            target,
            descendant,
            apply: convert_semantic_patch(apply),
        },
        ReferenceSemanticRuleWire::NotClass {
            family,
            target,
            class_name,
            apply,
        } => ReferenceSemanticRule::NotClass {
            family,
            target,
            class_name,
            apply: convert_semantic_patch(apply),
        },
        ReferenceSemanticRuleWire::OrdinalPalette {
            family,
            target,
            colors,
        } => ReferenceSemanticRule::OrdinalPalette {
            family,
            target,
            colors,
        },
    }
}

fn validate_theme_input(
    input: &ReferenceThemeInput,
    source_family: ReferenceDiagramFamily,
) -> Result<(), CatalogError> {
    let valid_text = |value: &str| !value.trim().is_empty() && !value.chars().any(char::is_control);
    if input.mechanisms().is_empty()
        || input.tokens.as_ref().is_some_and(|tokens| {
            [
                &tokens.background,
                &tokens.surface,
                &tokens.primary,
                &tokens.text,
            ]
            .into_iter()
            .any(|value| !valid_text(value))
        })
    {
        return Err(CatalogError::ThemeInputMismatch(input.fixture_id.clone()));
    }
    if let Some(typography) = &input.typography {
        if typography.font_stack.is_none()
            && typography.letter_spacing_milli_em.is_none()
            && typography.text_transform.is_none()
        {
            return Err(CatalogError::ThemeInputMismatch(input.fixture_id.clone()));
        }
        if typography
            .letter_spacing_milli_em
            .is_some_and(|spacing| spacing == 0 || !(-1000..=1000).contains(&spacing))
        {
            return Err(CatalogError::ThemeInputMismatch(input.fixture_id.clone()));
        }
        if let Some(stack) = &typography.font_stack {
            let unique_families = stack.families.iter().collect::<BTreeSet<_>>();
            let invalid_binding = match stack.binding {
                ReferenceFontBinding::FixtureAssets => stack.asset_ids.is_empty(),
                ReferenceFontBinding::SystemFonts => !stack.asset_ids.is_empty(),
            };
            if stack.families.is_empty()
                || unique_families.len() != stack.families.len()
                || stack.families.iter().any(|family| !valid_text(family))
                || invalid_binding
            {
                return Err(CatalogError::ThemeInputMismatch(input.fixture_id.clone()));
            }
        }
    }
    for layer in &input.canvas {
        validate_canvas_layer(&input.fixture_id, layer, valid_text)?;
    }
    let image_layer_count = input
        .canvas
        .iter()
        .filter(|layer| {
            matches!(
                layer,
                ReferenceCanvasLayer::LinearGradient { .. }
                    | ReferenceCanvasLayer::RadialGradient { .. }
            )
        })
        .count();
    if input.blend_mode.is_some() && image_layer_count == 0 {
        return Err(CatalogError::ThemeInputMismatch(input.fixture_id.clone()));
    }
    if let Some(style) = &input.node_style
        && (style.border.is_none()
            && style.dash_pattern.is_empty()
            && style.corner_radius_px.is_none()
            && style.shadow.is_none()
            || style
                .border
                .as_ref()
                .is_some_and(|border| border.width_px == 0 || !valid_text(&border.color))
            || style.dash_pattern.contains(&0)
            || style.corner_radius_px == Some(0)
            || style
                .shadow
                .as_ref()
                .is_some_and(|shadow| !valid_text(&shadow.color)))
    {
        return Err(CatalogError::ThemeInputMismatch(input.fixture_id.clone()));
    }
    for rule in &input.semantic_rules {
        if !valid_semantic_rule(rule, source_family, valid_text) {
            return Err(CatalogError::InvalidSemanticRule(input.fixture_id.clone()));
        }
    }
    if input.svg_effects.iter().any(|effect| match effect {
        ReferenceSvgEffect::TurbulenceDisplacement {
            base_frequency_milli,
            octaves,
            scale,
        } => *base_frequency_milli == 0 || *octaves == 0 || *scale == 0,
    }) || !input
        .residual_mechanisms
        .iter()
        .all(|mechanism| *mechanism == ReferenceThemeMechanism::BackdropFilter)
    {
        return Err(CatalogError::ThemeInputMismatch(input.fixture_id.clone()));
    }
    Ok(())
}

fn validate_canvas_layer(
    fixture_id: &str,
    layer: &ReferenceCanvasLayer,
    valid_text: impl Fn(&str) -> bool,
) -> Result<(), CatalogError> {
    let valid_stops = |stops: &[ReferenceGradientStop]| {
        stops.len() >= 2
            && stops
                .iter()
                .all(|stop| stop.offset_percent <= 100 && valid_text(&stop.color))
            && stops
                .windows(2)
                .all(|pair| pair[0].offset_percent < pair[1].offset_percent)
    };
    let valid_repetition = |repetition, width: Option<u16>, height: Option<u16>| match repetition {
        ReferenceGradientRepetition::None => width.is_none() && height.is_none(),
        ReferenceGradientRepetition::Tiled => {
            width.is_some_and(|value| value > 0) && height.is_some_and(|value| value > 0)
        }
        ReferenceGradientRepetition::Repeating => {
            matches!((width, height), (None, None) | (Some(1..), Some(1..)))
        }
    };
    let valid = match layer {
        ReferenceCanvasLayer::LinearGradient {
            angle_degrees,
            repetition,
            tile_width_px,
            tile_height_px,
            stops,
        } => {
            *angle_degrees <= 360
                && valid_stops(stops)
                && valid_repetition(*repetition, *tile_width_px, *tile_height_px)
        }
        ReferenceCanvasLayer::RadialGradient {
            center_x_percent,
            center_y_percent,
            repetition,
            tile_width_px,
            tile_height_px,
            stops,
        } => {
            *center_x_percent <= 100
                && *center_y_percent <= 100
                && valid_stops(stops)
                && valid_repetition(*repetition, *tile_width_px, *tile_height_px)
        }
        ReferenceCanvasLayer::Solid { color } => valid_text(color),
    };
    if valid {
        Ok(())
    } else {
        Err(CatalogError::ThemeInputMismatch(fixture_id.to_string()))
    }
}

fn valid_semantic_rule(
    rule: &ReferenceSemanticRule,
    source_family: ReferenceDiagramFamily,
    valid_text: impl Fn(&str) -> bool,
) -> bool {
    match rule {
        ReferenceSemanticRule::HasDescendant {
            family,
            target,
            descendant,
            apply,
        } => {
            *family == source_family
                && target != descendant
                && !apply.is_empty()
                && valid_semantic_patch(apply, &valid_text)
                && matches!(
                    (*family, *target, *descendant),
                    (
                        ReferenceDiagramFamily::Flowchart,
                        ReferenceSemanticTarget::Cluster,
                        ReferenceSemanticTarget::Node
                    ) | (
                        ReferenceDiagramFamily::StateDiagram,
                        ReferenceSemanticTarget::Node,
                        ReferenceSemanticTarget::Label
                    ) | (
                        ReferenceDiagramFamily::ClassDiagram,
                        ReferenceSemanticTarget::Node,
                        ReferenceSemanticTarget::Attribute
                    ) | (
                        ReferenceDiagramFamily::ErDiagram,
                        ReferenceSemanticTarget::Node,
                        ReferenceSemanticTarget::Attribute
                    )
                )
        }
        ReferenceSemanticRule::NotClass {
            family,
            target,
            class_name,
            apply,
        } => {
            *family == source_family
                && *target == ReferenceSemanticTarget::Node
                && valid_text(class_name)
                && !apply.is_empty()
                && valid_semantic_patch(apply, &valid_text)
        }
        ReferenceSemanticRule::OrdinalPalette {
            family,
            target,
            colors,
        } => {
            *family == source_family
                && *target == ReferenceSemanticTarget::Node
                && colors.len() >= 2
                && colors.iter().all(|color| valid_text(color))
        }
    }
}

fn valid_semantic_patch(
    patch: &ReferenceSemanticStylePatch,
    valid_text: impl Fn(&str) -> bool,
) -> bool {
    patch.fill.as_ref().is_none_or(|value| valid_text(value))
        && patch
            .border
            .as_ref()
            .is_none_or(|border| border.width_px > 0 && valid_text(&border.color))
        && patch.corner_radius_px != Some(0)
        && patch
            .shadow
            .as_ref()
            .is_none_or(|shadow| valid_text(&shadow.color))
        && patch
            .text_color
            .as_ref()
            .is_none_or(|value| valid_text(value))
        && patch
            .font_weight
            .is_none_or(|weight| (1..=1000).contains(&weight))
}

pub(crate) fn validate_fixture_font_contract(
    fixture_id: &str,
    input: &ReferenceThemeInput,
    declared_asset_ids: &BTreeSet<String>,
    assets_by_id: &BTreeMap<String, &AssetRecord>,
    visible_text: &[String],
) -> Result<(), CatalogError> {
    let input_asset_ids = input
        .font_stack()
        .map_or_else(BTreeSet::new, |stack| stack.asset_ids.clone());
    if input_asset_ids != *declared_asset_ids {
        return Err(CatalogError::ThemeInputAssetSetMismatch {
            fixture: fixture_id.to_string(),
            expected: declared_asset_ids.clone(),
            actual: input_asset_ids,
        });
    }
    if input.typography.is_some() && visible_text.is_empty() {
        return Err(CatalogError::MissingFixtureTextSamples(
            fixture_id.to_string(),
        ));
    }
    if declared_asset_ids.is_empty() {
        return Ok(());
    }
    let stack = input
        .font_stack()
        .ok_or_else(|| CatalogError::ThemeInputAssetSetMismatch {
            fixture: fixture_id.to_string(),
            expected: declared_asset_ids.clone(),
            actual: BTreeSet::new(),
        })?;
    let generic_families = [
        "cursive",
        "emoji",
        "fangsong",
        "fantasy",
        "math",
        "monospace",
        "sans-serif",
        "serif",
        "system-ui",
        "ui-monospace",
        "ui-rounded",
        "ui-sans-serif",
        "ui-serif",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let mut first_generic = None;
    for family in &stack.families {
        let normalized = family.to_ascii_lowercase();
        if generic_families.contains(normalized.as_str()) {
            first_generic.get_or_insert_with(|| family.clone());
            continue;
        }
        let resolved = declared_asset_ids.iter().any(|asset_id| {
            let asset = assets_by_id[asset_id];
            family.eq_ignore_ascii_case(&asset.family)
                || family.eq_ignore_ascii_case(&asset.canonical_family)
        });
        if !resolved {
            return Err(CatalogError::UnresolvedFixtureFontFamily {
                fixture: fixture_id.to_string(),
                family: family.clone(),
            });
        }
        if let Some(generic) = &first_generic {
            return Err(CatalogError::FixtureFontAfterGenericFallback {
                fixture: fixture_id.to_string(),
                family: family.clone(),
                generic: generic.clone(),
            });
        }
    }
    for asset_id in declared_asset_ids {
        let asset = assets_by_id[asset_id];
        if !stack.families.iter().any(|family| {
            family.eq_ignore_ascii_case(&asset.family)
                || family.eq_ignore_ascii_case(&asset.canonical_family)
        }) {
            return Err(CatalogError::UnusedFixtureFontAsset {
                fixture: fixture_id.to_string(),
                asset: asset_id.clone(),
            });
        }
    }
    for character in visible_text.iter().flat_map(|text| text.chars()) {
        let covered = stack
            .families
            .iter()
            .take_while(|family| !generic_families.contains(family.to_ascii_lowercase().as_str()))
            .any(|family| {
                declared_asset_ids.iter().any(|asset_id| {
                    let asset = assets_by_id[asset_id];
                    (family.eq_ignore_ascii_case(&asset.family)
                        || family.eq_ignore_ascii_case(&asset.canonical_family))
                        && unicode_range_contains(&asset.unicode_range, u32::from(character))
                            .expect("asset unicode ranges are validated before fixture contracts")
                })
            });
        if !covered {
            return Err(CatalogError::UncoveredFixtureCodepoint {
                fixture: fixture_id.to_string(),
                codepoint: u32::from(character),
            });
        }
    }
    Ok(())
}
