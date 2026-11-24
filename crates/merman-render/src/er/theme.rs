use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::resources::{OperationWorkError, OperationWorkMeter, PreparedTextRetainedReservation};

mod css_binding;
mod visual;
pub(crate) use css_binding::ErCssBinding;

#[derive(Debug, Clone)]
struct ErStyleDeclaration {
    property: Arc<str>,
    property_css: Arc<str>,
    value: Arc<str>,
    important: bool,
}

impl ErStyleDeclaration {
    fn parse(raw: &str) -> Option<Self> {
        let parsed = crate::mermaid_style::parse_style_declaration(raw)?;
        Some(Self {
            property: Arc::from(parsed.property()),
            property_css: Arc::from(parsed.property_css()),
            value: Arc::from(parsed.value()),
            important: parsed.important(),
        })
    }

    fn property(&self) -> &str {
        &self.property
    }
    fn property_css(&self) -> &str {
        &self.property_css
    }
    fn value(&self) -> &str {
        &self.value
    }
    fn important(&self) -> bool {
        self.important
    }
}

/// Mermaid source styles resolved once for an ER entity. The theme plan and SVG writer consume the
/// same declarations for measurement and SVG emission.
#[derive(Debug, Clone, Default)]
pub(crate) struct ErEntitySourceStyle {
    rect_declarations: Arc<[ErStyleDeclaration]>,
    text_declarations: Arc<[ErStyleDeclaration]>,
    pub(crate) visual: visual::ErSourceVisual,
}

impl ErEntitySourceStyle {
    pub(crate) fn retained_declaration_bytes(&self) -> usize {
        let mut bytes = 0usize;
        for declarations in [&self.rect_declarations, &self.text_declarations] {
            bytes = bytes
                .saturating_add(2 * std::mem::size_of::<usize>())
                .saturating_add(
                    declarations
                        .len()
                        .saturating_mul(std::mem::size_of::<ErStyleDeclaration>()),
                );
            for declaration in declarations.iter() {
                for value in [
                    declaration.property(),
                    declaration.property_css(),
                    declaration.value(),
                ] {
                    bytes = bytes
                        .saturating_add(value.len())
                        .saturating_add(2 * std::mem::size_of::<usize>());
                }
            }
        }
        bytes
    }
    pub(crate) fn label_style_css(&self, html_labels: bool) -> String {
        visual::label_style_css(self, html_labels)
    }

    pub(crate) fn text_style(
        &self,
        inherited: &crate::text::TextStyle,
        root_size: f64,
    ) -> crate::text::TextStyle {
        let mut cascade = crate::mermaid_style::TextStyleCascade::new(
            inherited,
            crate::mermaid_style::CssFontSizeContext::new(inherited.font_size, root_size),
        );
        for declaration in self.text_declarations.iter() {
            // styles2String emits the selected source value as important on each label layer.
            cascade.observe(declaration.property(), declaration.value(), true);
        }
        cascade.finish().into_owned()
    }

    pub(crate) fn typography_style(&self) -> String {
        self.text_declarations
            .iter()
            .filter(|declaration| {
                matches!(
                    declaration.property(),
                    "font-family" | "font-size" | "font-weight" | "font-style"
                )
            })
            .map(|declaration| {
                format!(
                    "{}:{} !important; ",
                    declaration.property_css(),
                    declaration.value()
                )
            })
            .collect()
    }

    pub(crate) fn label_style(&self, html_labels: bool) -> String {
        visual::label_style(self, html_labels)
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
}

pub(crate) fn compile_er_entity_source_style(
    entity: &merman_model::families::er::ErEntity,
    classes: &indexmap::IndexMap<String, merman_model::families::er::ErClassDef>,
) -> ErEntitySourceStyle {
    compile_er_source_style(
        entity.css_classes.split_whitespace(),
        &entity.css_styles,
        classes,
    )
}

pub(crate) fn compile_er_subgraph_source_style(
    subgraph: &merman_model::families::er::ErSubgraph,
    classes: &indexmap::IndexMap<String, merman_model::families::er::ErClassDef>,
) -> ErEntitySourceStyle {
    compile_er_source_style(
        subgraph.classes.iter().map(String::as_str),
        &subgraph.css_styles,
        classes,
    )
}

fn compile_er_source_style<'a>(
    class_names: impl DoubleEndedIterator<Item = &'a str>,
    source_styles: &[String],
    classes: &indexmap::IndexMap<String, merman_model::families::er::ErClassDef>,
) -> ErEntitySourceStyle {
    let mut seen_classes = BTreeSet::<&str>::new();
    let mut class_defs = class_names
        .rev()
        .filter(|class_name| seen_classes.insert(*class_name))
        .filter_map(|class_name| classes.get(class_name))
        .collect::<Vec<_>>();
    class_defs.reverse();
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
        visual: visual::ErSourceVisual::default(),
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
        )
        .into(),
        text_declarations: ordered_declarations(
            &text_map,
            &[
                "color",
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "opacity",
            ],
        )
        .into(),
    }
}

fn charge_er_subgraph_class_style_work(
    class_names: &[String],
    classes: &indexmap::IndexMap<String, merman_model::families::er::ErClassDef>,
    work_meter: &OperationWorkMeter,
) -> Result<(), OperationWorkError> {
    let mut seen_classes = BTreeSet::new();
    for class_name in class_names {
        work_meter.charge(1usize.saturating_add(class_name.len()))?;
        if !seen_classes.insert(class_name.as_str()) {
            continue;
        }
        if let Some(class) = classes.get(class_name) {
            for raw in class.styles.iter().chain(&class.text_styles) {
                work_meter.charge(1usize.saturating_add(raw.len()))?;
            }
        }
    }
    Ok(())
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
        .find(|declaration| declaration.property().eq_ignore_ascii_case(property))
        .map(ErStyleDeclaration::value)
}

/// Prepared HTML title and its retained text reservation.
#[derive(Debug)]
pub(crate) struct ErPreparedHtmlSubgraphLabel {
    fragment: String,
    _retained: PreparedTextRetainedReservation,
}

impl ErPreparedHtmlSubgraphLabel {
    fn prepare(
        id: &str,
        work_meter: &Arc<OperationWorkMeter>,
        project: impl FnOnce() -> String,
    ) -> Result<(String, Self), OperationWorkError> {
        // Match Class's map entry/control allowance; both String heaps are counted separately.
        let owner_bytes =
            std::mem::size_of::<(String, Self)>().saturating_add(std::mem::size_of::<usize>());
        work_meter.preflight_prepared_text_retained_bytes(owner_bytes.saturating_add(id.len()))?;
        let fragment = project();
        // The shared projector is opaque. Admit its actual capacity before copying the key
        // or allocating the map record, then retain one lease for that complete owner.
        work_meter.preflight_prepared_text_retained_bytes(
            owner_bytes
                .saturating_add(id.len())
                .saturating_add(fragment.capacity()),
        )?;
        let key = id.to_owned();
        let bytes = owner_bytes
            .saturating_add(key.capacity())
            .saturating_add(fragment.capacity());
        let retained = work_meter.reserve_prepared_text_retained_bytes(bytes)?;
        Ok((
            key,
            Self {
                fragment,
                _retained: retained,
            },
        ))
    }

    pub(crate) fn fragment(&self) -> &str {
        &self.fragment
    }
}

/// Prepared styles for ER entities and subgraphs, shared by the SVG paths.
#[derive(Debug)]
pub(crate) struct ErEntityThemePlan {
    css_binding: ErCssBinding,
    entity_indices: BTreeMap<String, usize>,
    entity_source_styles: Vec<ErEntitySourceStyle>,
    subgraph_source_styles: BTreeMap<String, ErEntitySourceStyle>,
    html_subgraph_labels: BTreeMap<String, ErPreparedHtmlSubgraphLabel>,
    // Drop shared source views before their counted prepared-label owner.
    _prepared_labels: Arc<crate::er::ErPreparedLabels>,
}

impl ErEntityThemePlan {
    pub(crate) fn resolve(
        config: &merman_model::presentation::PresentationConfig,
        native: Option<(&crate::theme::FamilyTheme, crate::er::ErNativeStyle)>,
        model: &merman_model::families::er::ErDiagramModel,
        layout: &crate::model::ErDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let entity_indices = model
            .entities
            .values()
            .enumerate()
            .map(|(index, entity)| (entity.id.clone(), index))
            .collect::<BTreeMap<_, _>>();
        let mut css_binding = ErCssBinding::resolve(config.as_value());
        if let Some((theme, native)) = native {
            css_binding.apply_native(theme, native, config);
        }
        let entity_source_styles = model
            .entities
            .values()
            .map(|entity| {
                layout
                    .prepared_labels
                    .entity(&entity.id)
                    .expect("ER preparation retains every semantic entity")
                    .source_style
                    .clone()
            })
            .collect::<Vec<_>>();
        work_meter.charge(model.subgraphs.len())?;
        let mut subgraphs = BTreeMap::new();
        for subgraph in &model.subgraphs {
            subgraphs.entry(subgraph.id.as_str()).or_insert(subgraph);
        }
        let mut subgraph_source_styles = BTreeMap::new();
        for cluster in &layout.clusters {
            let Some(subgraph) = subgraphs.get(cluster.id.as_str()) else {
                continue;
            };
            if subgraph_source_styles.contains_key(&subgraph.id) {
                continue;
            }
            work_meter.charge(1usize.saturating_add(subgraph.title.len()))?;
            for raw in &subgraph.css_styles {
                work_meter.charge(1usize.saturating_add(raw.len()))?;
            }
            charge_er_subgraph_class_style_work(&subgraph.classes, &model.classes, work_meter)?;
            subgraph_source_styles.insert(
                subgraph.id.clone(),
                compile_er_subgraph_source_style(subgraph, &model.classes),
            );
        }
        let config = super::ErConfigView::new(config.as_value());
        // Only top-level lanes use flowchart.htmlLabels and unconditional Markdown titles.
        // Nested groups retain the ordinary cluster writer's root mode and label type.
        let swimlane_lanes = layout
            .swimlane
            .iter()
            .flat_map(|paint| &paint.lanes)
            .filter(|lane| lane.parent_id.is_none())
            .map(|lane| (lane.id.as_str(), lane))
            .collect::<BTreeMap<_, _>>();
        let mut html_subgraph_labels = BTreeMap::new();
        for cluster in &layout.clusters {
            let lane = swimlane_lanes.get(cluster.id.as_str());
            if !config.subgraph_html_labels(lane.is_some())
                || html_subgraph_labels.contains_key(&cluster.id)
            {
                continue;
            }
            let (title, markdown) = if let Some(lane) = lane {
                (lane.title.as_str(), true)
            } else if let Some(subgraph) = subgraphs.get(cluster.id.as_str()) {
                (
                    subgraph.title.as_str(),
                    !matches!(subgraph.label_type.as_str(), "string" | "text"),
                )
            } else {
                continue;
            };
            let retained_meter = layout
                .prepared_labels
                .retained_work_meter
                .as_ref()
                .expect("ER prepared layout labels retain their operation ledger");
            let (key, label) =
                ErPreparedHtmlSubgraphLabel::prepare(&cluster.id, retained_meter, || {
                    if markdown {
                        crate::text::mermaid_markdown_to_xhtml_label_fragment(title, true)
                    } else {
                        crate::svg::escape_xml(title)
                    }
                })?;
            work_meter.charge(
                1usize
                    .saturating_add(key.len())
                    .saturating_add(label.fragment.len()),
            )?;
            html_subgraph_labels.insert(key, label);
        }
        Self {
            css_binding,
            entity_indices,
            entity_source_styles,
            subgraph_source_styles,
            html_subgraph_labels,
            _prepared_labels: Arc::clone(&layout.prepared_labels),
        }
        .prepare_visuals(model, work_meter)
    }

    pub(crate) fn index_for_entity_id(&self, entity_id: &str) -> Option<usize> {
        self.entity_indices.get(entity_id).copied()
    }

    pub(crate) fn css_binding(&self) -> &css_binding::ErCssBinding {
        &self.css_binding
    }

    pub(crate) fn source_style(&self, entity_index: usize) -> Option<&ErEntitySourceStyle> {
        self.entity_source_styles.get(entity_index)
    }

    pub(crate) fn subgraph_source_style(&self, id: &str) -> Option<&ErEntitySourceStyle> {
        self.subgraph_source_styles.get(id)
    }

    pub(crate) fn html_subgraph_label(&self, id: &str) -> Option<&ErPreparedHtmlSubgraphLabel> {
        self.html_subgraph_labels.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_subgraph_retained_capacity_is_shared_until_the_last_owner_drops() {
        let meter = Arc::new(OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let (key, label) = ErPreparedHtmlSubgraphLabel::prepare("Lane", &meter, || {
            let mut fragment = String::with_capacity(4096);
            fragment.push_str("<p>Visible title</p>");
            fragment
        })
        .unwrap();
        let bytes = std::mem::size_of::<(String, ErPreparedHtmlSubgraphLabel)>()
            + std::mem::size_of::<usize>()
            + key.capacity()
            + label.fragment.capacity();
        assert!(label.fragment.capacity() > label.fragment.len());
        assert_eq!(meter.prepared_text_retained_bytes(), bytes);
        let owner = Arc::new(BTreeMap::from([(key, label)]));
        let replay = Arc::clone(&owner);
        drop(owner);
        assert_eq!(meter.prepared_text_retained_bytes(), bytes);
        drop(replay);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn html_subgraph_retained_preflight_and_exact_capacity_limit() {
        use crate::resources::{RenderResourcePolicy, ResourceLimitId};
        let prepare = |meter: &Arc<OperationWorkMeter>, projected: &std::cell::Cell<bool>| {
            ErPreparedHtmlSubgraphLabel::prepare("Lane", meter, || {
                projected.set(true);
                let mut fragment = String::with_capacity(4096);
                fragment.push_str("<p>Visible title</p>");
                fragment
            })
        };
        let measuring = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let label = prepare(&measuring, &std::cell::Cell::new(false)).unwrap();
        let bytes = measuring.prepared_text_retained_bytes();
        drop(label);
        assert_eq!(measuring.prepared_text_retained_bytes(), 0);
        for ceiling in [1, bytes - 1, bytes] {
            let meter = Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, ceiling)
                    .unwrap(),
            ));
            let projected = std::cell::Cell::new(false);
            let result = prepare(&meter, &projected);
            if ceiling == bytes {
                let label = result.expect("exact retained capacity is admitted");
                assert_eq!(meter.prepared_text_retained_bytes(), bytes);
                drop(label);
            } else {
                let OperationWorkError::ResourceLimitExceeded(error) = result.unwrap_err() else {
                    panic!("expected the retained HTML-title byte ceiling");
                };
                assert_eq!(error.limit, "max_prepared_text_retained_bytes");
                assert_eq!(error.max, ceiling);
                assert!(error.actual > ceiling);
                assert_eq!(projected.get(), ceiling != 1);
            }
            assert_eq!(meter.prepared_text_retained_bytes(), 0);
        }
    }

    #[test]
    fn duplicate_subgraph_classes_charge_only_their_reference_work() {
        let classes = indexmap::IndexMap::from([(
            "accent".to_string(),
            merman_model::families::er::ErClassDef {
                id: "accent".to_string(),
                styles: vec!["fill:#123456".to_string()],
                text_styles: vec!["color:#654321".to_string()],
            },
        )]);
        let unique = vec!["accent".to_string()];
        let duplicate = vec!["accent".to_string(), "accent".to_string()];
        let unique_meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let duplicate_meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );

        charge_er_subgraph_class_style_work(&unique, &classes, &unique_meter)
            .expect("unique class work");
        charge_er_subgraph_class_style_work(&duplicate, &classes, &duplicate_meter)
            .expect("duplicate class work");

        assert_eq!(
            duplicate_meter.used() - unique_meter.used(),
            1 + "accent".len(),
            "a repeated class must not charge its declarations twice"
        );
    }
}
