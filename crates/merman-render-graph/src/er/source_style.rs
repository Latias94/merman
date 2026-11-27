use indexmap::IndexMap;
use merman_model::families::er::{ErClassDef, ErDiagramModel, ErEntity, ErSubgraph};
use merman_render_core::LayoutCluster;
use merman_render_core::mermaid_style::{
    CssFontSizeContext, TextStyleCascade, is_label_style_key, parse_style_declaration,
};
use merman_render_core::resources::{OperationWorkError, OperationWorkMeter};
use merman_render_core::text::TextStyle;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

mod visual;

pub use visual::{ErRowVisual, ErSourceVisual, prepare_er_source_visuals};

#[derive(Debug, Clone)]
struct ErStyleDeclaration {
    property: Arc<str>,
    property_css: Arc<str>,
    value: Arc<str>,
    important: bool,
}

impl ErStyleDeclaration {
    fn parse(raw: &str) -> Option<Self> {
        let parsed = parse_style_declaration(raw)?;
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
pub struct ErEntitySourceStyle {
    rect_declarations: Arc<[ErStyleDeclaration]>,
    text_declarations: Arc<[ErStyleDeclaration]>,
    visual: ErSourceVisual,
}

impl ErEntitySourceStyle {
    pub const fn visual(&self) -> &ErSourceVisual {
        &self.visual
    }

    pub fn retained_declaration_bytes(&self) -> usize {
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
    pub fn label_style_css(&self, html_labels: bool) -> String {
        visual::label_style_css(self, html_labels)
    }

    pub fn text_style(&self, inherited: &TextStyle, root_size: f64) -> TextStyle {
        let mut cascade = TextStyleCascade::new(
            inherited,
            CssFontSizeContext::new(inherited.font_size, root_size),
        );
        for declaration in self.text_declarations.iter() {
            // styles2String emits the selected source value as important on each label layer.
            cascade.observe(declaration.property(), declaration.value(), true);
        }
        cascade.finish().into_owned()
    }

    fn typography_style(&self) -> String {
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

    pub fn label_style(&self, html_labels: bool) -> String {
        visual::label_style(self, html_labels)
    }

    pub fn rect_value(&self, property: &str) -> Option<&str> {
        last_source_style_value(&self.rect_declarations, property)
    }

    fn text_value(&self, property: &str) -> Option<&str> {
        last_source_style_value(&self.text_declarations, property)
    }

    pub fn fill(&self) -> Option<&str> {
        self.rect_value("fill")
    }

    pub fn stroke(&self) -> Option<&str> {
        self.rect_value("stroke")
    }
}

pub fn compile_er_entity_source_style(
    entity: &ErEntity,
    classes: &IndexMap<String, ErClassDef>,
) -> ErEntitySourceStyle {
    compile_er_source_style(
        entity.css_classes.split_whitespace(),
        &entity.css_styles,
        classes,
    )
}

fn compile_er_subgraph_source_style(
    subgraph: &ErSubgraph,
    classes: &IndexMap<String, ErClassDef>,
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
    classes: &IndexMap<String, ErClassDef>,
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
        visual: ErSourceVisual::default(),
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

pub struct ErSubgraphSourceStyles<'model> {
    pub subgraphs: BTreeMap<&'model str, &'model ErSubgraph>,
    pub styles: BTreeMap<String, ErEntitySourceStyle>,
}

pub fn prepare_er_subgraph_source_styles<'model>(
    model: &'model ErDiagramModel,
    clusters: &[LayoutCluster],
    work_meter: &OperationWorkMeter,
) -> Result<ErSubgraphSourceStyles<'model>, OperationWorkError> {
    work_meter.charge(model.subgraphs.len())?;
    let mut subgraphs = BTreeMap::new();
    for subgraph in &model.subgraphs {
        subgraphs.entry(subgraph.id.as_str()).or_insert(subgraph);
    }
    let mut styles = BTreeMap::new();
    for cluster in clusters {
        let Some(subgraph) = subgraphs.get(cluster.id.as_str()) else {
            continue;
        };
        if styles.contains_key(&subgraph.id) {
            continue;
        }
        work_meter.charge(1usize.saturating_add(subgraph.title.len()))?;
        for raw in &subgraph.css_styles {
            work_meter.charge(1usize.saturating_add(raw.len()))?;
        }
        charge_er_subgraph_class_style_work(&subgraph.classes, &model.classes, work_meter)?;
        styles.insert(
            subgraph.id.clone(),
            compile_er_subgraph_source_style(subgraph, &model.classes),
        );
    }
    Ok(ErSubgraphSourceStyles { subgraphs, styles })
}

fn charge_er_subgraph_class_style_work(
    class_names: &[String],
    classes: &IndexMap<String, ErClassDef>,
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
    if is_label_style_key(property) {
        text_map.insert(property.to_owned(), declaration);
    }
}

fn insert_er_text_declaration(
    text_map: &mut BTreeMap<String, ErStyleDeclaration>,
    declaration: ErStyleDeclaration,
) {
    let property = declaration.property();
    if is_label_style_key(property) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use merman_render_core::resources::RenderResourcePolicy;

    #[test]
    fn duplicate_subgraph_classes_charge_only_their_reference_work() {
        let classes = IndexMap::from([(
            "accent".to_string(),
            ErClassDef {
                id: "accent".to_string(),
                styles: vec!["fill:#123456".to_string()],
                text_styles: vec!["color:#654321".to_string()],
            },
        )]);
        let unique = vec!["accent".to_string()];
        let duplicate = vec!["accent".to_string(), "accent".to_string()];
        let unique_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let duplicate_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());

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
