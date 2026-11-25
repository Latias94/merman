use super::declaration_scan::{UNSUPPORTED_NATIVE_TEXT_STYLES, root_dependent_size, style_lists};
use crate::flowchart::{
    FlowchartConfigView, FlowchartLabelDom, FlowchartModel, FlowchartRenderContext, FlowchartShape,
    flowchart_effective_node_class_names, flowchart_insert_style_by_spelling,
    flowchart_label_is_empty_for_render, flowchart_split_mermaid_style_decls,
};
use crate::mermaid_style::{
    HtmlTextGeometryDeclarations, HtmlTextProperty, parse_style_declaration,
};
use indexmap::IndexMap;
use merman_model::presentation::PresentationConfig;
use merman_model::{OperationControl, OperationControlResult};
use quick_xml::{Reader, XmlVersion, events::Event};
use rustc_hash::FxHashMap;

#[derive(Clone, Copy)]
struct Declaration<'a> {
    property: &'static str,
    spelling: &'a str,
    value: &'a str,
    important: bool,
    raw: &'a str,
}

fn declarations<'a>(
    styles: &'a [String],
    control: &OperationControl,
) -> OperationControlResult<Vec<Declaration<'a>>> {
    let mut declarations = Vec::new();
    for group in styles {
        for _ in group.as_bytes().chunks(1024) {
            control.checkpoint()?;
        }
        for raw in flowchart_split_mermaid_style_decls(group) {
            control.checkpoint()?;
            let Some(parsed) = parse_style_declaration(raw) else {
                continue;
            };
            let Some(property) = UNSUPPORTED_NATIVE_TEXT_STYLES
                .iter()
                .copied()
                .chain(["font-size"])
                .find(|property| *property == parsed.property().trim())
            else {
                continue;
            };
            declarations.push(Declaration {
                property,
                spelling: parsed.property_source(),
                value: parsed.value(),
                important: parsed.important(),
                raw,
            });
        }
    }
    Ok(declarations)
}

type ClassDeclarations<'a> = FxHashMap<&'a str, Vec<Declaration<'a>>>;

fn class_declarations<'a, 's>(
    catalog: &'s ClassDeclarations<'a>,
    classes: impl Iterator<Item = &'a str> + 's,
) -> impl Iterator<Item = Declaration<'a>> + 's
where
    'a: 's,
{
    classes
        .filter_map(move |class| catalog.get(class))
        .flatten()
        .copied()
}

pub(super) fn unsupported_source_style(
    model: &FlowchartModel,
    context: &FlowchartRenderContext,
    config: &PresentationConfig,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    let view = FlowchartConfigView::new(config);
    let node_html = view.node_html_labels();
    let edge_html = view.effective_html_labels();
    let hand_drawn = crate::config::mermaid_config_diagram_look(config).is_hand_drawn();
    let mut catalog = ClassDeclarations::default();
    for (class, styles) in &model.class_defs {
        control.checkpoint()?;
        let styles = declarations(styles, control)?;
        if !styles.is_empty() {
            catalog.insert(class.as_str(), styles);
        }
    }
    for (ordinal, subgraph) in model.subgraphs.iter().enumerate() {
        control.checkpoint()?;
        let (classes, styles) = context.effective_subgraph_css(ordinal, subgraph);
        if let Some(property) = style_lists([styles], control)? {
            return Ok(Some(property));
        }
        if catalog.is_empty() {
            continue;
        }
        let label = context.subgraph_title_for_render(ordinal, subgraph);
        let label_type = subgraph.label_type.as_deref().unwrap_or("text");
        let shells = FlowchartLabelDom::cluster(label, label_type, edge_html).classes();
        let reached = classes
            .iter()
            .map(String::as_str)
            .chain(shells.into_iter().map(|(class, _)| class));
        if let Some(property) = unsupported_structural_classes(&catalog, reached, control)? {
            return Ok(Some(property));
        }
        if let Some(property) = unsupported_html_classes(
            &catalog,
            edge_html.then_some((label, label_type, config)),
            control,
        )? {
            return Ok(Some(property));
        }
    }
    for node in &model.nodes {
        control.checkpoint()?;
        if node.styles.is_empty() && catalog.is_empty() {
            continue;
        }
        let shape = node.layout_shape.as_deref().unwrap_or("squareRect");
        let label = context.node_label_for_render(node).unwrap_or(&node.id);
        let label_type = node.label_type.as_deref().unwrap_or("text");
        let supported_html = node_html
            && node.img.is_none()
            && node.icon.is_none()
            && matches!(
                FlowchartShape::resolve(shape),
                Some(FlowchartShape::Process | FlowchartShape::RoundedRectangle)
            );
        let inline = declarations(&node.styles, control)?;
        let classes = flowchart_effective_node_class_names(&model.class_defs, &node.classes);
        let assigned = class_declarations(&catalog, classes.clone());
        let html = node_html.then_some((label, label_type, config));
        if let Some(property) = unsupported_declarations(
            assigned.chain(inline),
            html.filter(|_| supported_html),
            control,
        )? {
            return Ok(Some(property));
        }
        if !catalog.is_empty() {
            let structural =
                FlowchartLabelDom::node(shape, label, label_type, node_html, hand_drawn).classes();
            let additional = structural
                .into_iter()
                .map(|(class, _)| class)
                .filter(|class| !classes.clone().any(|assigned| assigned == *class));
            if let Some(property) = unsupported_structural_classes(&catalog, additional, control)? {
                return Ok(Some(property));
            }
        }
        if let Some(property) = unsupported_html_classes(&catalog, html, control)? {
            return Ok(Some(property));
        }
    }
    let defaults = declarations(
        model
            .edge_defaults
            .as_ref()
            .map_or(&[], |edge| edge.style.as_slice()),
        control,
    )?;
    for (ordinal, edge) in model.edges.iter().enumerate() {
        control.checkpoint()?;
        if edge.style.is_empty() && defaults.is_empty() && catalog.is_empty() {
            continue;
        }
        let Some(label) = context
            .edge_label_for_render(ordinal, edge)
            .filter(|label| !flowchart_label_is_empty_for_render(label))
        else {
            continue;
        };
        let label_type = edge.label_type.as_deref().unwrap_or("text");
        let html = edge_html.then_some((label, label_type, config));
        let inline = declarations(&edge.style, control)?;
        let assigned = class_declarations(&catalog, edge.classes.iter().map(String::as_str));
        if let Some(property) = unsupported_declarations(
            assigned.chain(defaults.iter().copied()).chain(inline),
            html,
            control,
        )? {
            return Ok(Some(property));
        }
        if let Some(property) = unsupported_structural_classes(
            &catalog,
            ["root", "nodes", "edgeLabels", "edgeLabel", "label"]
                .into_iter()
                .chain(
                    if edge_html {
                        &["labelBkg"][..]
                    } else {
                        &["text-outer-tspan", "row"]
                    }
                    .iter()
                    .copied(),
                ),
            control,
        )? {
            return Ok(Some(property));
        }
        if let Some(property) = unsupported_html_classes(&catalog, html, control)? {
            return Ok(Some(property));
        }
    }
    Ok(None)
}

type HtmlLabel<'a> = (&'a str, &'a str, &'a PresentationConfig);

fn unsupported_declarations<'a>(
    source: impl IntoIterator<Item = Declaration<'a>>,
    html: Option<HtmlLabel<'_>>,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    // Use the Flowchart compiler's authored-key identity and priority. In particular, a
    // shadowed class/default declaration is not evidence about the resulting label.
    let mut winners = IndexMap::default();
    let mut font_sizes = Vec::new();
    for declaration in source {
        control.checkpoint()?;
        if declaration.property == "font-size" {
            font_sizes.push(declaration.raw);
            continue;
        }
        flowchart_insert_style_by_spelling(&mut winners, declaration.spelling, declaration, |d| {
            d.important
        });
    }
    let mut supported_property = None;
    for declaration in winners.values() {
        if html.is_none()
            || declaration.spelling != declaration.property
            || !HtmlTextProperty::named(declaration.property).is_some_and(|property| {
                HtmlTextGeometryDeclarations::is_source_owned(property, declaration.value)
            })
        {
            return Ok(Some(declaration.property.to_owned()));
        }
        supported_property = Some(declaration.property);
    }
    if let (Some(property), Some((label, label_type, config))) = (supported_property, html)
        && !plain_html_label(label, label_type, config, control)?
    {
        return Ok(Some(property.to_owned()));
    }
    root_dependent_size(font_sizes, control)
}

fn unsupported_structural_classes<'a>(
    catalog: &ClassDeclarations<'a>,
    classes: impl IntoIterator<Item = &'a str>,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    for class in classes {
        if let Some(styles) = catalog.get(class)
            && let Some(property) = unsupported_declarations(styles.iter().copied(), None, control)?
        {
            return Ok(Some(property));
        }
    }
    Ok(None)
}

fn unsupported_html_classes(
    catalog: &ClassDeclarations<'_>,
    html: Option<HtmlLabel<'_>>,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    let Some((label, label_type, config)) = html.filter(|(label, _, _)| {
        !catalog.is_empty() && (label.contains('<') || label.contains(":fa-"))
    }) else {
        return Ok(None);
    };
    let xhtml = crate::text::flowchart_label_xhtml_for_typography_admission(
        label, label_type, config, control,
    )?;
    let mut reader = Reader::from_str(&xhtml);
    loop {
        control.checkpoint()?;
        match reader.read_event() {
            Ok(Event::Start(tag) | Event::Empty(tag)) => {
                if let Ok(Some(attribute)) = tag.try_get_attribute("class")
                    && let Ok(value) = attribute.normalized_value(XmlVersion::Implicit1_0)
                    && let Some(property) = unsupported_structural_classes(
                        catalog,
                        value.split_ascii_whitespace(),
                        control,
                    )?
                {
                    return Ok(Some(property));
                }
            }
            Ok(Event::Eof) | Err(_) => return Ok(None),
            _ => {}
        }
    }
}

fn plain_html_label(
    label: &str,
    label_type: &str,
    config: &PresentationConfig,
    control: &OperationControl,
) -> OperationControlResult<bool> {
    if crate::math::contains_delimited_math(label) {
        return Ok(false);
    }
    let xhtml = crate::text::flowchart_label_xhtml_for_typography_admission(
        label, label_type, config, control,
    )?;
    let mut reader = Reader::from_str(&xhtml);
    loop {
        control.checkpoint()?;
        match reader.read_event() {
            Ok(Event::Start(tag))
                if tag.name().as_ref() == "p" && tag.attributes_raw().is_empty() => {}
            Ok(Event::End(tag)) if tag.name().as_ref() == "p" => {}
            Ok(Event::Empty(tag))
                if tag.name().as_ref() == "br" && tag.attributes_raw().trim_ascii().is_empty() => {}
            Ok(Event::Text(_) | Event::GeneralRef(_)) => {}
            Ok(Event::Eof) => return Ok(true),
            _ => return Ok(false),
        }
    }
}
