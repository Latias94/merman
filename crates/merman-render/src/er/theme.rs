use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeStyle,
    Specified, ThemeCapability, ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackPlan, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

mod terminal;

use terminal::{
    EntityExpectation, ErTableRowTerminalId, ErTextTerminalId, ExpectedPaint,
    TextTerminalExpectation,
};
pub(crate) use terminal::{
    ErAttributeTextRole, ErEntityThemeReceipt, ErRelationTerminalExpectation,
};

type ErStyleDeclaration = crate::diagram_theme::PreparedSourceStyleDeclaration;

/// ER's resolved base-size owner, shared unchanged by preparation, layout, SVG, and evidence.
#[derive(Debug)]
pub(crate) struct ErBaseFontSizePlan {
    font_size_css: Box<str>,
    font_size_px: f64,
    typed_requested: bool,
    typed_active: bool,
}

impl ErBaseFontSizePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
    ) -> Self {
        let configured_font_size_px =
            crate::config::config_theme_or_root_font_size_px_opt(effective_config.as_value())
                .or_else(|| {
                    crate::config::config_f64_css_px(
                        effective_config.as_value(),
                        &["er", "fontSize"],
                    )
                })
                .unwrap_or(16.0)
                .max(1.0);
        let typed_requested = theme.is_some_and(|theme| {
            theme.family_mechanism_routes().iter().any(|route| {
                route.mechanism()
                    == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    && route.disposition() == FamilyThemeDisposition::TypedAdapter
            })
        });
        let config_owns = typed_requested
            && merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "themeVariables.fontSize",
            );
        let typed_active = typed_requested && !config_owns;
        let (font_size_px, font_size_css) = match theme {
            Some(theme) if typed_active => {
                let typed_font_size = theme.typography().font_size_px().max(1.0);
                (
                    f64::from(typed_font_size),
                    format!("{typed_font_size}px").into_boxed_str(),
                )
            }
            _ => (
                configured_font_size_px,
                format!("{configured_font_size_px}px").into_boxed_str(),
            ),
        };
        Self {
            font_size_css,
            font_size_px,
            typed_requested,
            typed_active,
        }
    }

    pub(crate) fn layout_override_px(&self) -> Option<f64> {
        self.typed_active.then_some(self.font_size_px)
    }

    fn font_size_css(&self) -> &str {
        &self.font_size_css
    }

    const fn font_size_px(&self) -> f64 {
        self.font_size_px
    }

    const fn typed_requested(&self) -> bool {
        self.typed_requested
    }

    const fn typed_active(&self) -> bool {
        self.typed_active
    }
}

/// Mermaid source styles resolved once for an ER entity. The theme plan and SVG writer consume the
/// same declaration set so source ownership cannot drift between admission and emission.
#[derive(Debug, Clone, Default)]
pub(crate) struct ErEntitySourceStyle {
    rect_declarations: Vec<ErStyleDeclaration>,
    text_declarations: Vec<ErStyleDeclaration>,
}

impl ErEntitySourceStyle {
    pub(crate) fn rect_declarations(&self) -> &[ErStyleDeclaration] {
        &self.rect_declarations
    }

    pub(crate) fn text_declarations(&self) -> &[ErStyleDeclaration] {
        &self.text_declarations
    }

    pub(crate) fn rect_value(&self, property: &str) -> Option<&str> {
        last_source_style_value(&self.rect_declarations, property)
    }

    pub(crate) fn text_value(&self, property: &str) -> Option<&str> {
        last_source_style_value(&self.text_declarations, property)
    }

    pub(crate) fn fill(&self) -> Option<&str> {
        self.rect_value("fill")
    }

    pub(crate) fn stroke(&self) -> Option<&str> {
        self.rect_value("stroke")
    }

    fn owns_text_color(&self) -> bool {
        self.text_value("color").is_some()
    }

    fn owns_text_font_size(&self) -> bool {
        self.text_declarations
            .iter()
            .rev()
            .find(|declaration| declaration.property_matches("font-size"))
            .is_some_and(|declaration| !declaration.inherits_property_value())
    }
}

pub(crate) fn compile_er_entity_source_style(
    entity: &merman_core::diagrams::er::ErEntityRenderModel,
    classes: &indexmap::IndexMap<String, merman_core::diagrams::er::ErClassDefRenderModel>,
) -> ErEntitySourceStyle {
    compile_er_source_style(
        entity.css_classes.split_whitespace(),
        &entity.css_styles,
        classes,
    )
}

pub(crate) fn compile_er_subgraph_source_style(
    subgraph: &merman_core::diagrams::er::ErSubgraphRenderModel,
    classes: &indexmap::IndexMap<String, merman_core::diagrams::er::ErClassDefRenderModel>,
) -> ErEntitySourceStyle {
    compile_er_source_style(
        subgraph.classes.iter().map(String::as_str),
        &subgraph.css_styles,
        classes,
    )
}

fn compile_er_source_style<'a>(
    class_names: impl Iterator<Item = &'a str>,
    source_styles: &[String],
    classes: &indexmap::IndexMap<String, merman_core::diagrams::er::ErClassDefRenderModel>,
) -> ErEntitySourceStyle {
    let mut seen_classes = BTreeSet::<&str>::new();
    let class_defs = class_names
        .filter(|class_name| seen_classes.insert(*class_name))
        .filter_map(|class_name| classes.get(class_name))
        .collect::<Vec<_>>();
    let mut rect_map = BTreeMap::<String, ErStyleDeclaration>::new();
    let mut text_map = BTreeMap::<String, ErStyleDeclaration>::new();

    for raw in class_defs.iter().flat_map(|class_def| &class_def.styles) {
        let Some(declaration) = ErStyleDeclaration::parse(raw) else {
            continue;
        };
        insert_er_box_declaration(&mut rect_map, &mut text_map, declaration);
    }
    for raw in class_defs
        .iter()
        .flat_map(|class_def| &class_def.text_styles)
    {
        let Some(declaration) = ErStyleDeclaration::parse(raw) else {
            continue;
        };
        insert_er_text_declaration(&mut text_map, declaration);
    }
    for raw in source_styles {
        let Some(declaration) = ErStyleDeclaration::parse(raw) else {
            continue;
        };
        insert_er_box_declaration(&mut rect_map, &mut text_map, declaration.clone());
        insert_er_text_declaration(&mut text_map, declaration);
    }

    ErEntitySourceStyle {
        rect_declarations: ordered_declarations(
            &rect_map,
            &[
                "fill",
                "stroke",
                "stroke-width",
                "stroke-dasharray",
                "opacity",
                "fill-opacity",
                "stroke-opacity",
            ],
        ),
        text_declarations: ordered_declarations(
            &text_map,
            &[
                "color",
                "font-family",
                "font-size",
                "font-weight",
                "opacity",
            ],
        ),
    }
}

fn insert_er_box_declaration(
    rect_map: &mut BTreeMap<String, ErStyleDeclaration>,
    text_map: &mut BTreeMap<String, ErStyleDeclaration>,
    declaration: ErStyleDeclaration,
) {
    let property = declaration.property();
    if matches!(
        property,
        "fill"
            | "stroke"
            | "stroke-width"
            | "stroke-dasharray"
            | "opacity"
            | "fill-opacity"
            | "stroke-opacity"
    ) {
        rect_map.insert(property.to_owned(), declaration.clone());
    }
    if crate::mermaid_style::is_label_style_key(property) {
        text_map.insert(property.to_owned(), declaration);
    }
}

fn insert_er_text_declaration(
    text_map: &mut BTreeMap<String, ErStyleDeclaration>,
    declaration: ErStyleDeclaration,
) {
    let property = declaration.property();
    if crate::mermaid_style::is_label_style_key(property) {
        text_map.insert(property.to_owned(), declaration);
    }
}

fn ordered_declarations(
    declarations: &BTreeMap<String, ErStyleDeclaration>,
    properties: &[&str],
) -> Vec<ErStyleDeclaration> {
    properties
        .iter()
        .filter_map(|property| declarations.get(*property).cloned())
        .collect()
}

fn last_source_style_value<'a>(
    declarations: &'a [ErStyleDeclaration],
    property: &str,
) -> Option<&'a str> {
    declarations
        .iter()
        .rev()
        .find(|declaration| declaration.property_matches(property))
        .map(ErStyleDeclaration::value)
}

/// ER terminal paint resolved once for the semantic model and shared by layout, SVG, and terminal
/// theme evidence.
#[derive(Debug)]
pub(crate) struct ErEntityThemePlan {
    entity_indices: BTreeMap<String, usize>,
    entity_source_styles: Vec<ErEntitySourceStyle>,
    inherited_font_stack: InheritedFontStackPlan,
    base_font_size: ErBaseFontSizePlan,
    has_visible_typography: bool,
    font_stack_pending: bool,
    font_stack_residual: bool,
    font_stack_not_applicable: bool,
    font_size_pending: bool,
    font_size_residual: bool,
    font_size_not_applicable: bool,
    font_size_layout_verified: bool,
    typography_title: Option<Box<str>>,
    expectations: Vec<EntityExpectation>,
    text_terminal_evidence_enabled: bool,
    text_terminals: BTreeMap<ErTextTerminalId, TextTerminalExpectation>,
    table_rows: BTreeMap<ErTableRowTerminalId, Option<ExpectedPaint>>,
    relation_strokes: Vec<Option<ExpectedPaint>>,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, BTreeSet<ThemeCapability>>,
    terminal_receipt: OnceLock<ErEntityThemeReceipt>,
}

pub(crate) const ER_PAINT_DEFAULT_PATHS: [&str; 5] = [
    "themeVariables.textColor",
    "themeVariables.nodeTextColor",
    "themeVariables.lineColor",
    "themeVariables.rowOdd",
    "themeVariables.rowEven",
];

impl ErEntityThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
        inherited_font_stack: InheritedFontStackPlan,
        base_font_size: ErBaseFontSizePlan,
        relationship_html_labels: bool,
        diagram_title: Option<&str>,
        model: &merman_core::diagrams::er::ErDiagramRenderModel,
        layout: &crate::model::ErDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let entity_indices = model
            .entities
            .values()
            .enumerate()
            .map(|(index, entity)| (entity.id.clone(), index))
            .collect::<BTreeMap<_, _>>();
        let entity_count = model.entities.len();
        let entity_source_styles = model
            .entities
            .values()
            .map(|entity| compile_er_entity_source_style(entity, &model.classes))
            .collect::<Vec<_>>();
        let Some(theme) = theme else {
            return Ok(Self {
                entity_indices,
                entity_source_styles,
                inherited_font_stack,
                base_font_size,
                has_visible_typography: false,
                font_stack_pending: false,
                font_stack_residual: false,
                font_stack_not_applicable: false,
                font_size_pending: false,
                font_size_residual: false,
                font_size_not_applicable: false,
                font_size_layout_verified: false,
                typography_title: None,
                expectations: Vec::new(),
                text_terminal_evidence_enabled: false,
                text_terminals: BTreeMap::new(),
                table_rows: BTreeMap::new(),
                relation_strokes: Vec::new(),
                evidence: FamilyThemeEvidence::default(),
                pending: BTreeMap::new(),
                terminal_receipt: OnceLock::new(),
            });
        };
        let expectations = vec![EntityExpectation::default(); entity_count];
        let needs_text_terminal_evidence = theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Text,
                    ..
                } | FamilyThemeMechanism::BaseTypography(_)
            )
        });
        let mut text_terminals = if needs_text_terminal_evidence {
            collect_text_terminals(model, layout)
        } else {
            BTreeMap::new()
        };
        if needs_text_terminal_evidence
            && let Some(title) = diagram_title
                .map(str::trim)
                .filter(|title| !title.is_empty())
        {
            insert_text_terminal(
                &mut text_terminals,
                ErTextTerminalId::DiagramTitle,
                &crate::text::VisibleTextStyleFacts::plain_text(title),
            );
        }
        let table_rows = collect_table_row_terminals(model);

        let mermaid_owns_fill = mermaid_owns_entity_fill(effective_config);
        let mermaid_owns_stroke = mermaid_owns_entity_stroke(effective_config);
        let entity_html_labels = super::ErConfigView::new(effective_config.as_value())
            .entity_html_label_wrap_mode()
            == crate::text::WrapMode::HtmlLike;
        let svg_subgraph_labels = !entity_html_labels;
        let mermaid_owns_entity_text = mermaid_owns_text_fill(effective_config, entity_html_labels);
        let mermaid_owns_relation_text =
            mermaid_owns_text_fill(effective_config, relationship_html_labels);
        let relation_stroke_source_owned = mermaid_owns_relation_stroke(effective_config);
        let mut visible_subgraph_styles = BTreeMap::<String, ErEntitySourceStyle>::new();
        if svg_subgraph_labels && needs_text_terminal_evidence {
            let subgraphs = model
                .subgraphs
                .iter()
                .map(|subgraph| (subgraph.id.as_str(), subgraph))
                .collect::<BTreeMap<_, _>>();
            for cluster in &layout.clusters {
                let Some(subgraph) = subgraphs.get(cluster.id.as_str()) else {
                    continue;
                };
                visible_subgraph_styles.insert(
                    subgraph.id.clone(),
                    compile_er_subgraph_source_style(subgraph, &model.classes),
                );
            }
        }
        let source_owns_font_family = entity_source_styles
            .iter()
            .any(|style| style.text_value("font-family").is_some())
            || visible_subgraph_styles
                .values()
                .any(|style| style.text_value("font-family").is_some());
        let source_owns_font_size = text_terminals.keys().any(|terminal_id| {
            !terminal_id.is_relation_label()
                && text_terminal_font_size_source_owned(
                    terminal_id,
                    &entity_indices,
                    &entity_source_styles,
                )
        }) || visible_subgraph_styles
            .values()
            .any(ErEntitySourceStyle::owns_text_font_size);
        let diagram_title = diagram_title
            .map(str::trim)
            .filter(|title| !title.is_empty());
        let has_visible_typography = !text_terminals.is_empty() || diagram_title.is_some();
        let has_visible_base_typography = text_terminals.iter().any(|(terminal_id, terminal)| {
            !terminal_id.is_relation_label() && terminal.visible_run_count > 0
        }) || diagram_title.is_some();
        let font_stack_requested = inherited_font_stack.typed_font_stack_requested();
        let font_stack_pending = has_visible_typography
            && font_stack_requested
            && inherited_font_stack.typed_font_stack_active()
            && !source_owns_font_family;
        let font_stack_not_applicable = font_stack_requested
            && (!has_visible_typography || !inherited_font_stack.typed_font_stack_active());
        let font_stack_residual = font_stack_requested
            && has_visible_typography
            && !font_stack_pending
            && !font_stack_not_applicable;
        let font_size_pending = has_visible_base_typography
            && base_font_size.typed_requested()
            && base_font_size.typed_active()
            && !source_owns_font_size;
        let font_size_not_applicable = base_font_size.typed_requested()
            && (!has_visible_base_typography || !base_font_size.typed_active());
        let font_size_residual = base_font_size.typed_requested()
            && has_visible_base_typography
            && !font_size_pending
            && !font_size_not_applicable;
        let font_size_layout_verified = !font_size_pending
            || ((layout.prepared_labels.entity_font_size_px() - base_font_size.font_size_px())
                .abs()
                < 1e-6
                && (layout.prepared_labels.attribute_font_size_px()
                    - base_font_size.font_size_px())
                .abs()
                    < 1e-6
                && (layout.prepared_labels.relationship_font_size_px() - 14.0).abs() < 1e-6);
        let typography_title = if font_stack_pending || font_size_pending {
            diagram_title.map(Into::into)
        } else {
            None
        };
        let mut expectations = expectations;
        let mut winner_properties = BTreeSet::<(usize, ThemeTarget, ResolvedStyleProperty)>::new();
        let mut expected_capabilities =
            BTreeMap::<(usize, ThemeTarget, ResolvedStyleProperty), ThemeCapability>::new();

        for (entity_index, _) in model.entities.values().enumerate() {
            let style = theme.style_with_work_meter(
                ThemeTarget::Entity,
                ThemeVariant::Default,
                Some(entity_index + 1),
                work_meter,
            )?;
            for (property, origin) in style.winner_rule_properties() {
                winner_properties.insert((origin.rule_index(), ThemeTarget::Entity, property));
            }

            let source_style = &entity_source_styles[entity_index];
            if let Some(expected) = typed_fill_expectation(
                theme,
                &style,
                mermaid_owns_fill || source_style.fill().is_some(),
            ) {
                expected_capabilities.insert(
                    (
                        expected.rule_index,
                        ThemeTarget::Entity,
                        ResolvedStyleProperty::Fill,
                    ),
                    paint_capability_from_css(&expected.css),
                );
                expectations[entity_index].fill = Some(expected);
            }
            if let Some(expected) = typed_stroke_expectation(
                theme,
                &style,
                mermaid_owns_stroke || source_style.stroke().is_some(),
            ) {
                expected_capabilities.insert(
                    (
                        expected.rule_index,
                        ThemeTarget::Entity,
                        ResolvedStyleProperty::Stroke,
                    ),
                    paint_capability_from_css(&expected.css),
                );
                expectations[entity_index].stroke = Some(expected);
            }
        }

        if svg_subgraph_labels && needs_text_terminal_evidence {
            work_meter.charge(model.subgraphs.len())?;
            let subgraphs: BTreeMap<_, _> = model
                .subgraphs
                .iter()
                .map(|subgraph| (subgraph.id.as_str(), subgraph))
                .collect();
            for cluster in &layout.clusters {
                let Some(subgraph) = subgraphs.get(cluster.id.as_str()) else {
                    continue;
                };
                work_meter.charge(1usize.saturating_add(subgraph.title.len()))?;
                for raw in &subgraph.css_styles {
                    work_meter.charge(1usize.saturating_add(raw.len()))?;
                }
                let mut seen_classes = BTreeSet::new();
                for class in &subgraph.classes {
                    work_meter.charge(1usize.saturating_add(class.len()))?;
                    if !seen_classes.insert(class.as_str()) {
                        continue;
                    }
                    if let Some(class) = model.classes.get(class) {
                        for raw in class.styles.iter().chain(&class.text_styles) {
                            work_meter.charge(1usize.saturating_add(raw.len()))?;
                        }
                    }
                }
                let source_style = visible_subgraph_styles
                    .get(&subgraph.id)
                    .expect("visible ER subgraph styles are indexed with layout clusters");
                let facts = subgraph_svg_text_facts(subgraph, source_style);
                insert_text_terminal(
                    &mut text_terminals,
                    ErTextTerminalId::SubgraphLabel(subgraph.id.clone().into()),
                    &facts,
                );
            }
        }
        for (text_index, (terminal_id, terminal)) in text_terminals.iter_mut().enumerate() {
            let text_style = theme.text_style_with_work_meter(
                ThemeTarget::Text,
                ThemeVariant::Default,
                Some(text_index + 1),
                work_meter,
            )?;
            for (property, origin) in text_style.winner_rule_properties() {
                winner_properties.insert((origin.rule_index(), ThemeTarget::Text, property));
            }
            if terminal.inherited_color_run_count == 0
                || text_terminal_source_owned(terminal_id, &entity_indices, &entity_source_styles)
            {
                continue;
            }
            let mermaid_owns_text = match terminal_id {
                ErTextTerminalId::DiagramTitle => {
                    mermaid_owns_default_path(effective_config, "themeVariables.textColor")
                }
                ErTextTerminalId::SubgraphLabel(id) => {
                    visible_subgraph_styles
                        .get(id.as_ref())
                        .is_some_and(ErEntitySourceStyle::owns_text_color)
                        || effective_config
                            .get_str("themeVariables.titleColor")
                            .is_some()
                        || mermaid_owns_default_path(effective_config, "themeVariables.textColor")
                }
                ErTextTerminalId::RelationLabel(_) => mermaid_owns_relation_text,
                ErTextTerminalId::EntityName(_) | ErTextTerminalId::Attribute { .. } => {
                    mermaid_owns_entity_text
                }
            };
            if let Some(expected) = typed_fill_expectation(theme, &text_style, mermaid_owns_text) {
                expected_capabilities.insert(
                    (
                        expected.rule_index,
                        ThemeTarget::Text,
                        ResolvedStyleProperty::Fill,
                    ),
                    paint_capability_from_css(&expected.css),
                );
                terminal.paint = Some(expected);
            }
        }

        let mut table_rows = table_rows;
        for (table_index, (terminal_id, terminal)) in table_rows.iter_mut().enumerate() {
            let variant = terminal_id.variant();
            let style = theme.style_with_work_meter(
                ThemeTarget::Table,
                variant,
                Some(table_index + 1),
                work_meter,
            )?;
            for (property, origin) in style.winner_rule_properties() {
                winner_properties.insert((origin.rule_index(), ThemeTarget::Table, property));
            }
            if table_terminal_source_owned(terminal_id, &entity_indices, &entity_source_styles) {
                continue;
            }
            let Some(expected) = typed_fill_expectation(
                theme,
                &style,
                mermaid_owns_table_fill(effective_config, variant),
            ) else {
                continue;
            };
            expected_capabilities.insert(
                (
                    expected.rule_index,
                    ThemeTarget::Table,
                    ResolvedStyleProperty::Fill,
                ),
                paint_capability_from_css(&expected.css),
            );
            *terminal = Some(expected);
        }

        let mut relation_strokes = Vec::with_capacity(model.relationships.len());
        for relationship_index in 0..model.relationships.len() {
            let relation_style = theme.style_with_work_meter(
                ThemeTarget::Relation,
                ThemeVariant::Default,
                Some(relationship_index + 1),
                work_meter,
            )?;
            // Relation.fill is the historical fallback for lineColor. A specified
            // stroke (including Clear or unsupported paint) always owns that lane.
            let fill_fallback = matches!(
                relation_style.stroke_resolution().specified(),
                Specified::Unspecified
            );
            for (property, origin) in relation_style.winner_rule_properties() {
                if property != ResolvedStyleProperty::Fill || fill_fallback {
                    winner_properties.insert((
                        origin.rule_index(),
                        ThemeTarget::Relation,
                        property,
                    ));
                }
            }
            let (property, expected) = if fill_fallback {
                (
                    ResolvedStyleProperty::Fill,
                    typed_fill_expectation(theme, &relation_style, relation_stroke_source_owned),
                )
            } else {
                (
                    ResolvedStyleProperty::Stroke,
                    typed_stroke_expectation(theme, &relation_style, relation_stroke_source_owned),
                )
            };
            if let Some(expected) = &expected {
                expected_capabilities.insert(
                    (expected.rule_index, ThemeTarget::Relation, property),
                    paint_capability_from_css(&expected.css),
                );
            }
            relation_strokes.push(expected);
        }
        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<(usize, ThemeTarget), ErRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target:
                        target @ (ThemeTarget::Entity
                        | ThemeTarget::Relation
                        | ThemeTarget::Text
                        | ThemeTarget::Table),
                    selector,
                    facet,
                } => {
                    let occurrence_count = match target {
                        ThemeTarget::Entity => entity_count,
                        ThemeTarget::Relation => model.relationships.len(),
                        ThemeTarget::Text => text_terminals.len(),
                        ThemeTarget::Table => table_occurrence_count(&table_rows, selector),
                        _ => unreachable!("guarded ER theme target"),
                    };
                    let observation = observations.entry((rule_index, target)).or_default();
                    if occurrence_count == 0
                        || !selector.ordinal_domain_intersects_occurrence_count(occurrence_count)
                    {
                        continue;
                    }
                    let property = resolved_style_property_for_facet(facet);
                    let route_won = winner_properties.contains(&(rule_index, target, property));
                    if !route_won {
                        continue;
                    }
                    observation.applicable = true;
                    match route.disposition() {
                        FamilyThemeDisposition::TypedAdapter => {
                            if let Some(capability) =
                                expected_capabilities.get(&(rule_index, target, property))
                            {
                                observation.capabilities.insert(*capability);
                            }
                        }
                        FamilyThemeDisposition::Unsupported => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        FamilyThemeDisposition::LegacyCompatibility => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Entity,
                } => {
                    // Reconciled below from each entity's final fill winner.
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Entity,
                    ..
                } => {
                    // Reconciled below from each entity's final style.
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Relation,
                } => {
                    // Reconciled below from each relation's final fill winner.
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Relation,
                    ..
                } => {
                    // Reconciled below from each relation's final style.
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Entity,
                    TerminalVariantDomain::uniform(entity_count, ThemeVariant::Default),
                ),
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Relation,
                    TerminalVariantDomain::uniform(
                        model.relationships.len(),
                        ThemeVariant::Default,
                    ),
                ),
                UnsupportedTerminalDomain::direct(
                    ThemeTarget::Title,
                    TerminalVariantDomain::uniform(
                        usize::from(diagram_title.is_some()),
                        ThemeVariant::Default,
                    ),
                ),
            ],
            work_meter,
        )?;

        let mut pending = BTreeMap::new();
        for ((rule_index, target), observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Strict completion remains fail-closed until the writer proves every winner.
            } else if !observation.capabilities.is_empty() {
                pending.insert(key, observation.capabilities);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            entity_indices,
            entity_source_styles,
            inherited_font_stack,
            base_font_size,
            has_visible_typography,
            font_stack_pending,
            font_stack_residual,
            font_stack_not_applicable,
            font_size_pending,
            font_size_residual,
            font_size_not_applicable,
            font_size_layout_verified,
            typography_title,
            expectations,
            text_terminal_evidence_enabled: needs_text_terminal_evidence,
            text_terminals,
            table_rows,
            relation_strokes,
            evidence,
            pending,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn index_for_entity_id(&self, entity_id: &str) -> Option<usize> {
        self.entity_indices.get(entity_id).copied()
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn font_size_css(&self) -> &str {
        self.base_font_size.font_size_css()
    }

    pub(crate) fn font_size_override(&self) -> Option<f64> {
        self.base_font_size.layout_override_px()
    }

    pub(crate) fn font_size_override_css(&self) -> Option<&str> {
        self.base_font_size
            .typed_active()
            .then_some(self.font_size_css())
    }

    pub(crate) fn source_style(&self, entity_index: usize) -> Option<&ErEntitySourceStyle> {
        self.entity_source_styles.get(entity_index)
    }

    pub(crate) fn typed_fill(&self, entity_index: usize) -> Option<(usize, &str)> {
        self.expectations
            .get(entity_index)?
            .fill
            .as_ref()
            .map(|expected| (expected.rule_index, expected.css.as_str()))
    }

    pub(crate) fn typed_stroke(&self, entity_index: usize) -> Option<(usize, &str)> {
        self.expectations
            .get(entity_index)?
            .stroke
            .as_ref()
            .map(|expected| (expected.rule_index, expected.css.as_str()))
    }

    pub(crate) fn typed_entity_name(&self, entity_id: &str) -> Option<(usize, &str)> {
        self.typed_text_terminal(&ErTextTerminalId::entity_name(entity_id))
    }

    pub(crate) fn typed_attribute_text(
        &self,
        entity_id: &str,
        row_index: usize,
        role: ErAttributeTextRole,
    ) -> Option<(usize, &str)> {
        self.typed_text_terminal(&ErTextTerminalId::attribute(entity_id, row_index, role))
    }

    pub(crate) fn typed_subgraph_label(&self, id: &str) -> Option<(usize, &str)> {
        self.typed_text_terminal(&ErTextTerminalId::SubgraphLabel(id.into()))
    }

    pub(crate) fn records_subgraph_labels(&self) -> bool {
        self.text_terminals
            .keys()
            .any(|id| matches!(id, ErTextTerminalId::SubgraphLabel(_)))
    }

    pub(crate) fn typed_diagram_title(&self) -> Option<(usize, &str)> {
        self.typed_text_terminal(&ErTextTerminalId::DiagramTitle)
    }

    pub(crate) fn typed_relation_label(&self, relationship_index: usize) -> Option<(usize, &str)> {
        self.typed_text_terminal(&ErTextTerminalId::relation_label(relationship_index))
    }

    fn typed_text_terminal(&self, id: &ErTextTerminalId) -> Option<(usize, &str)> {
        self.text_terminals
            .get(id)?
            .paint
            .as_ref()
            .map(|expected| (expected.rule_index, expected.css.as_str()))
    }

    pub(crate) fn typed_table_row(
        &self,
        entity_id: &str,
        row_index: usize,
    ) -> Option<(usize, &str)> {
        self.table_rows
            .get(&ErTableRowTerminalId::new(entity_id, row_index))?
            .as_ref()
            .map(|expected| (expected.rule_index, expected.css.as_str()))
    }

    pub(crate) fn typed_relation_stroke(&self, relationship_index: usize) -> Option<(usize, &str)> {
        self.relation_strokes
            .get(relationship_index)?
            .as_ref()
            .map(|expected| (expected.rule_index, expected.css.as_str()))
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        relation_terminals: Vec<ErRelationTerminalExpectation>,
        include_relation_labels: bool,
    ) -> ErEntityThemeReceipt {
        let text_terminals = self
            .text_terminals
            .iter()
            .filter(|(id, _)| include_relation_labels || !id.is_relation_label())
            .map(|(id, expectation)| (id.clone(), expectation.clone()))
            .collect();
        let receipt = ErEntityThemeReceipt::new(
            self.expectations.clone(),
            text_terminals,
            self.table_rows.clone(),
            self.font_stack_pending
                .then(|| self.font_family_css().into()),
        )
        .with_typography_font_size(self.font_size_pending.then(|| {
            (
                self.font_size_css().into(),
                self.base_font_size.font_size_px(),
                self.font_size_layout_verified,
            )
        }))
        .with_typography_title(self.typography_title.clone());
        let receipt = if self.text_terminal_evidence_enabled {
            receipt
        } else {
            receipt.without_text_terminals()
        };
        receipt.with_relation_terminals(relation_terminals)
    }

    pub(crate) fn record_terminal(&self, receipt: ErEntityThemeReceipt) -> bool {
        // Terminal completeness protects all renderer-owned paint/geometry receipts. Typography
        // ownership is deliberately evaluated later: an unmeasured source font is a legitimate
        // BestEffort residual and must not abort SVG emission. RequirePortable rejects that
        // residual through the frozen family report instead.
        if self.pending.is_empty() && !self.font_stack_pending && !self.font_size_pending {
            return true;
        }
        receipt.proves_complete() && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, self.has_visible_typography);
        let font_stack_key =
            FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
        if self.font_stack_pending {
            if let Some(receipt) = self.terminal_receipt.get()
                && receipt.proves_typography()
            {
                evidence
                    .mark_applied_with_capabilities(font_stack_key, [ThemeCapability::Typography]);
            } else {
                evidence.mark_residual(
                    font_stack_key,
                    FamilyThemeResidualReason::UnsupportedTypography,
                );
            }
        } else if self.font_stack_not_applicable {
            evidence.mark_not_applicable(font_stack_key);
        } else if self.font_stack_residual {
            evidence.mark_residual(
                font_stack_key,
                FamilyThemeResidualReason::UnsupportedTypography,
            );
        }
        let font_size_key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontSize);
        if self.font_size_pending {
            if let Some(receipt) = self.terminal_receipt.get()
                && receipt.proves_font_size()
            {
                evidence
                    .mark_applied_with_capabilities(font_size_key, [ThemeCapability::Typography]);
            } else {
                evidence.mark_residual(
                    font_size_key,
                    FamilyThemeResidualReason::UnsupportedTypography,
                );
            }
        } else if self.font_size_not_applicable {
            evidence.mark_not_applicable(font_size_key);
        } else if self.font_size_residual {
            evidence.mark_residual(
                font_size_key,
                FamilyThemeResidualReason::UnsupportedTypography,
            );
        }
        let Some(receipt) = self.terminal_receipt.get() else {
            return evidence;
        };
        for (key, capabilities) in &self.pending {
            let rule_index = match key {
                FamilyThemeMechanismKey::Rule { index, .. } => *index,
                FamilyThemeMechanismKey::Typography(_)
                | FamilyThemeMechanismKey::OrdinalPalette { .. }
                | FamilyThemeMechanismKey::EffectBinding { .. } => continue,
            };
            if receipt.proves_rule(rule_index) {
                evidence.mark_applied_with_capabilities(key.clone(), capabilities.iter().copied());
            } else if receipt.proves_complete() && !receipt.has_effective_rule(rule_index) {
                evidence.mark_not_applicable(key.clone());
            }
        }
        evidence
    }
}

fn typed_fill_expectation(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns: bool,
) -> Option<ExpectedPaint> {
    if mermaid_owns {
        return None;
    }
    let origin = style.fill_resolution().winner()?;
    let facet = FamilyThemeRuleFacet::fill(style.fill_resolution().specified())?;
    if theme.rule_facet_disposition(origin.rule_index(), facet)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    let css = match style.fill_resolution().specified() {
        Specified::Value(crate::diagram_theme::CanvasPaint::Transparent) => {
            "transparent".to_string()
        }
        Specified::Value(crate::diagram_theme::CanvasPaint::Solid(color)) => color.as_css(),
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(crate::diagram_theme::CanvasPaint::LinearGradient(_))
        | Specified::Value(crate::diagram_theme::CanvasPaint::RadialGradient(_))
        | Specified::Value(crate::diagram_theme::CanvasPaint::Pattern(_)) => return None,
    };
    Some(ExpectedPaint {
        rule_index: origin.rule_index(),
        css,
    })
}

fn typed_stroke_expectation(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns: bool,
) -> Option<ExpectedPaint> {
    if mermaid_owns {
        return None;
    }
    let origin = style.stroke_resolution().winner()?;
    let facet = FamilyThemeRuleFacet::stroke(style.stroke_resolution().specified())?;
    if theme.rule_facet_disposition(origin.rule_index(), facet)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    let css = match style.stroke_resolution().specified() {
        Specified::Value(crate::diagram_theme::CanvasPaint::Transparent) => "transparent".into(),
        Specified::Value(crate::diagram_theme::CanvasPaint::Solid(color)) => color.as_css(),
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(crate::diagram_theme::CanvasPaint::LinearGradient(_))
        | Specified::Value(crate::diagram_theme::CanvasPaint::RadialGradient(_))
        | Specified::Value(crate::diagram_theme::CanvasPaint::Pattern(_)) => return None,
    };
    Some(ExpectedPaint {
        rule_index: origin.rule_index(),
        css,
    })
}

pub(crate) fn subgraph_svg_text_facts(
    subgraph: &merman_core::diagrams::er::ErSubgraphRenderModel,
    source_style: &ErEntitySourceStyle,
) -> crate::text::VisibleTextStyleFacts {
    let facts = if matches!(subgraph.label_type.as_str(), "string" | "text") {
        crate::text::VisibleTextStyleFacts::plain_text(&subgraph.title)
    } else {
        crate::text::VisibleTextStyleFacts::from_svg_markdown_projection(&subgraph.title)
    };
    facts.with_unmeasured_typography(
        source_style.text_value("font-family").is_some(),
        source_style.owns_text_font_size(),
    )
}

fn collect_text_terminals(
    model: &merman_core::diagrams::er::ErDiagramRenderModel,
    layout: &crate::model::ErDiagramLayout,
) -> BTreeMap<ErTextTerminalId, TextTerminalExpectation> {
    let mut terminals = BTreeMap::new();
    for entity in model.entities.values() {
        let Some(measure) = layout.prepared_labels.entity(&entity.id) else {
            continue;
        };
        insert_text_terminal(
            &mut terminals,
            ErTextTerminalId::entity_name(&entity.id),
            measure.label.visible_style_facts(),
        );
        for (row_index, row) in measure.rows.iter().enumerate() {
            for (role, label) in [
                (ErAttributeTextRole::Type, &row.type_label),
                (ErAttributeTextRole::Name, &row.name_label),
                (ErAttributeTextRole::Keys, &row.key_label),
                (ErAttributeTextRole::Comment, &row.comment_label),
            ] {
                insert_text_terminal(
                    &mut terminals,
                    ErTextTerminalId::attribute(&entity.id, row_index, role),
                    label.visible_style_facts(),
                );
            }
        }
    }

    for relationship_index in 0..model.relationships.len() {
        let Some(label) = layout.prepared_labels.relationship(relationship_index) else {
            continue;
        };
        insert_text_terminal(
            &mut terminals,
            ErTextTerminalId::relation_label(relationship_index),
            label.visible_style_facts(),
        );
    }
    terminals
}

fn insert_text_terminal(
    terminals: &mut BTreeMap<ErTextTerminalId, TextTerminalExpectation>,
    id: ErTextTerminalId,
    facts: &crate::text::VisibleTextStyleFacts,
) {
    if facts.parse_valid() && facts.has_visible_runs() {
        terminals.insert(
            id,
            TextTerminalExpectation {
                paint: None,
                visible_run_count: facts.visible_run_count(),
                inherited_color_run_count: facts.inherited_color_run_count(),
                inherited_font_family_run_count: facts.inherited_font_family_run_count(),
                unverified_font_family_run_count: facts.unverified_font_family_run_count(),
                inherited_font_size_run_count: facts.inherited_font_size_run_count(),
                unverified_font_size_run_count: facts.unverified_font_size_run_count(),
            },
        );
    }
}

fn collect_table_row_terminals(
    model: &merman_core::diagrams::er::ErDiagramRenderModel,
) -> BTreeMap<ErTableRowTerminalId, Option<ExpectedPaint>> {
    model
        .entities
        .values()
        .flat_map(|entity| {
            (0..entity.attributes.len())
                .map(|row_index| (ErTableRowTerminalId::new(&entity.id, row_index), None))
        })
        .collect()
}

fn text_terminal_source_owned(
    terminal_id: &ErTextTerminalId,
    entity_indices: &BTreeMap<String, usize>,
    entity_source_styles: &[ErEntitySourceStyle],
) -> bool {
    terminal_id
        .entity_id()
        .and_then(|entity_id| entity_indices.get(entity_id))
        .and_then(|index| entity_source_styles.get(*index))
        .is_some_and(ErEntitySourceStyle::owns_text_color)
}

fn text_terminal_font_size_source_owned(
    terminal_id: &ErTextTerminalId,
    entity_indices: &BTreeMap<String, usize>,
    entity_source_styles: &[ErEntitySourceStyle],
) -> bool {
    terminal_id
        .entity_id()
        .and_then(|entity_id| entity_indices.get(entity_id))
        .and_then(|index| entity_source_styles.get(*index))
        .is_some_and(ErEntitySourceStyle::owns_text_font_size)
}

fn table_terminal_source_owned(
    terminal_id: &ErTableRowTerminalId,
    entity_indices: &BTreeMap<String, usize>,
    entity_source_styles: &[ErEntitySourceStyle],
) -> bool {
    // The row writer applies entity fill only to even row paths. Odd rows retain
    // their own fill, independently of the enclosing entity's source style.
    if terminal_id.variant() != ThemeVariant::Even {
        return false;
    }
    entity_indices
        .get(terminal_id.entity_id())
        .and_then(|index| entity_source_styles.get(*index))
        .is_some_and(|source_style| source_style.fill().is_some())
}

fn table_occurrence_count(
    table_rows: &BTreeMap<ErTableRowTerminalId, Option<ExpectedPaint>>,
    selector: FamilyThemeSelectorShape,
) -> usize {
    match selector {
        FamilyThemeSelectorShape::Static { variant: None } => table_rows.len(),
        FamilyThemeSelectorShape::Static {
            variant: Some(variant @ (ThemeVariant::Odd | ThemeVariant::Even)),
        } => table_rows
            .keys()
            .filter(|terminal| terminal.variant() == variant)
            .count(),
        FamilyThemeSelectorShape::Static { variant: Some(_) }
        | FamilyThemeSelectorShape::Ordinal { .. } => table_rows.len(),
    }
}

fn mermaid_owns_entity_fill(config: &merman_core::MermaidConfig) -> bool {
    merman_core::__private::config_path_overrides_typed_default(config, "themeVariables.mainBkg")
        || matches!(
            config.get_str("theme"),
            Some("redux-color" | "redux-dark-color")
        )
}

fn mermaid_owns_entity_stroke(config: &merman_core::MermaidConfig) -> bool {
    merman_core::__private::config_path_overrides_typed_default(config, "themeVariables.nodeBorder")
        || matches!(
            config.get_str("theme"),
            Some("redux-color" | "redux-dark-color")
        )
}

fn mermaid_owns_default_path(config: &merman_core::MermaidConfig, path: &str) -> bool {
    // Preserve the pre-projection ownership decision; calculated theme colors are not raw owners.
    merman_core::__private::config_post_detection_default_blocked(config, path) != Some(false)
}

fn mermaid_owns_relation_stroke(config: &merman_core::MermaidConfig) -> bool {
    mermaid_owns_default_path(config, "themeVariables.lineColor")
}

fn mermaid_owns_text_fill(config: &merman_core::MermaidConfig, html_labels: bool) -> bool {
    let node_text_owned = mermaid_owns_default_path(config, "themeVariables.nodeTextColor");
    if html_labels && (!node_text_owned || config.get_str("themeVariables.nodeTextColor").is_some())
    {
        node_text_owned
    } else {
        mermaid_owns_default_path(config, "themeVariables.textColor")
    }
}

fn mermaid_owns_table_fill(config: &merman_core::MermaidConfig, variant: ThemeVariant) -> bool {
    let path = match variant {
        ThemeVariant::Odd => "themeVariables.rowOdd",
        ThemeVariant::Even => "themeVariables.rowEven",
        _ => return false,
    };
    mermaid_owns_default_path(config, path)
}

#[derive(Debug, Default)]
struct ErRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeSet<ThemeCapability>,
}

fn paint_capability_from_css(css: &str) -> ThemeCapability {
    if css == "transparent" {
        ThemeCapability::TransparentPaint
    } else {
        ThemeCapability::SolidPaint
    }
}
