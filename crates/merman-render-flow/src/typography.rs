//! Native measurement admission for the labels selected by Flow renderers.

use crate::layout_config::FlowchartConfigView;
use merman_model::presentation::PresentationConfig;
use merman_model::{OperationControl, OperationControlResult, OperationPhase};
use merman_render_core::{
    html::{HtmlOutputError, HtmlSanitizeSink},
    mermaid_style::native_typography::UNSUPPORTED_NATIVE_TEXT_STYLES,
    resources::{OperationWorkError, OperationWorkMeter, RenderResourcePolicy},
    text::create_text_html::{flowchart_label_html_source, flowchart_label_sanitize_config},
};

#[cfg(feature = "diagram-flowchart")]
mod flowchart_css;
#[cfg(feature = "diagram-flowchart")]
pub use flowchart_css::unsupported_source_style;

fn flowchart_label_xhtml_for_typography_admission(
    label: &str,
    label_type: &str,
    config: &PresentationConfig,
    control: &OperationControl,
) -> Result<String, merman_model::OperationCancelled> {
    control.checkpoint()?;
    let html = flowchart_label_html_source(label, label_type, config);
    // Sanitizer-removed attributes and encoded angle brackets cannot affect rendered typography.
    let work_meter = OperationWorkMeter::new_with_control(
        RenderResourcePolicy::unbounded_for_trusted_input(),
        control.clone(),
    );
    let parse_work = std::cell::Cell::new(0);
    let sink = HtmlSanitizeSink::new(usize::MAX, &work_meter, &parse_work, OperationPhase::Layout);
    let sanitize_config = flowchart_label_sanitize_config(config);
    match merman_markup::sanitize::sanitize_xhtml_with_sink(&html, sanitize_config.as_ref(), &sink)
    {
        Ok(html) => Ok(html),
        Err(merman_markup::sanitize::SanitizeFailure::Output(HtmlOutputError::Operation(
            OperationWorkError::Cancelled(error),
        ))) => Err(error),
        Err(_) => Ok(String::new()),
    }
}

/// Finds retained inline HTML typography that the caller's measurement policy cannot represent.
/// Admission uses the same Markdown projection and HTML normalization as SVG emission.
#[doc(hidden)]
pub fn unsupported_html_label_style(
    label: &str,
    label_type: &str,
    config: &PresentationConfig,
    unsupported_properties: &[&str],
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    use cssparser::{Delimiter, Parser};
    use quick_xml::{Reader, events::Event};

    let html = flowchart_label_xhtml_for_typography_admission(label, label_type, config, control)?;
    let mut tags = Reader::from_str(&html);
    tags.config_mut().check_end_names = false;
    tags.config_mut().allow_unmatched_ends = true;
    loop {
        control.checkpoint()?;
        let opening = match tags.read_event() {
            Ok(Event::Start(opening) | Event::Empty(opening)) => opening,
            Ok(Event::Eof) | Err(_) => break,
            _ => continue,
        };
        let Some(style) = opening
            .html_attributes()
            .with_checks(false)
            .filter_map(Result::ok)
            .find(|attr| attr.key.as_ref().eq_ignore_ascii_case("style"))
        else {
            continue;
        };
        let style = merman_model::text::decode_html_entities_to_unicode(style.value.as_ref());
        let element = opening.name().as_ref().to_ascii_lowercase();
        let mut declarations = Parser::new(&style);
        while !declarations.is_exhausted() {
            control.checkpoint()?;
            let declaration = declarations.parse_until_after(Delimiter::Semicolon, |declaration| {
                let property = declaration.expect_ident_cloned()?.to_ascii_lowercase();
                declaration.expect_colon()?;
                let first = declaration.next().ok().cloned();
                let inherits = matches!(
                    &first,
                    Some(cssparser::Token::Ident(value)) if value.eq_ignore_ascii_case("inherit")
                ) && (declaration.is_exhausted()
                    || (declaration.try_parse(cssparser::parse_important).is_ok()
                        && declaration.is_exhausted()));
                while declaration.next().is_ok() {}
                // Inheritance can override the intrinsic emphasis or code face modeled below.
                let overrides_element = match element.as_str() {
                    "strong" | "b" => matches!(property.as_str(), "font-weight" | "font"),
                    "cite" | "dfn" | "em" | "i" | "var" => {
                        matches!(property.as_str(), "font-style" | "font")
                    }
                    "code" | "kbd" | "samp" | "tt" => {
                        matches!(property.as_str(), "font-family" | "font")
                    }
                    _ => false,
                };
                Ok::<_, cssparser::ParseError<()>>((
                    property,
                    first.is_some(),
                    inherits && !overrides_element,
                ))
            });
            control.checkpoint()?;
            if let Ok((property, true, false)) = declaration
                && (matches!(
                    property.as_str(),
                    "font-size" | "font-family" | "font-weight" | "font-style"
                ) || unsupported_properties
                    .iter()
                    .any(|name| property.eq_ignore_ascii_case(name)))
            {
                return Ok(Some(property));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
#[test]
fn unsupported_html_style_checks_cancellation_during_fragment_recovery() {
    let control = OperationControl::new();
    control.cancel_after_checkpoints(8);
    let label = "<span>label</span>".repeat(64);
    let error = unsupported_html_label_style(
        &label,
        "text",
        &PresentationConfig::default(),
        &[],
        &control,
    )
    .expect_err("typography admission must cancel while recovering HTML");
    assert_eq!(control.checkpoint(), Err(error));
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub fn unsupported_flowchart_html_style(
    model: &merman_model::families::flowchart::FlowchartModel,
    context: &merman_model::families::flowchart::FlowchartRenderContext,
    config: &PresentationConfig,
    swimlane: bool,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    let view = FlowchartConfigView::new(config);
    let edge_html = view.effective_html_labels();
    let inspect = |label: &str, label_type: Option<&str>| {
        unsupported_html_label_style(
            label,
            label_type.unwrap_or("text"),
            config,
            UNSUPPORTED_NATIVE_TEXT_STYLES,
            control,
        )
    };
    if view.node_html_labels() {
        for node in &model.nodes {
            control.checkpoint()?;
            if let Some(property) = inspect(
                context.node_label_for_render(node).unwrap_or(&node.id),
                node.label_type.as_deref(),
            )? {
                return Ok(Some(property));
            }
        }
    }
    if edge_html {
        for (semantic_index, edge) in model.edges.iter().enumerate() {
            control.checkpoint()?;
            if let Some(label) = context.edge_label_for_render(semantic_index, edge)
                && let Some(property) = inspect(label, edge.label_type.as_deref())?
            {
                return Ok(Some(property));
            }
        }
    }
    let swimlane_title_html = view.swimlane_title_html_labels();
    let mut subgraph_children = std::collections::HashSet::new();
    if swimlane {
        for subgraph in &model.subgraphs {
            control.checkpoint()?;
            for child in &subgraph.nodes {
                control.checkpoint()?;
                subgraph_children.insert(child.as_str());
            }
        }
    }
    for (ordinal, subgraph) in model.subgraphs.iter().enumerate() {
        control.checkpoint()?;
        let (title_html, label_type) =
            if swimlane && !subgraph_children.contains(subgraph.id.as_str()) {
                (swimlane_title_html, Some("markdown"))
            } else {
                (edge_html, subgraph.label_type.as_deref())
            };
        if title_html
            && let Some(property) = inspect(
                context.subgraph_title_for_render(ordinal, subgraph),
                label_type,
            )?
        {
            return Ok(Some(property));
        }
    }
    Ok(None)
}

#[cfg(feature = "diagram-agentflow")]
pub fn unsupported_agentflow_html_style(
    model: &merman_model::families::agentflow::AgentflowDiagramRenderModel,
    config: &PresentationConfig,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    let view = FlowchartConfigView::new(config);
    let labels = model.render_label_sources();
    let inspect = |label, kind: Option<&str>| {
        unsupported_html_label_style(
            label,
            kind.unwrap_or("text"),
            config,
            UNSUPPORTED_NATIVE_TEXT_STYLES,
            control,
        )
    };
    if view.node_html_labels() {
        for (label, kind) in labels.node_labels() {
            control.checkpoint()?;
            if let Some(property) = inspect(label, kind)? {
                return Ok(Some(property));
            }
        }
    }
    if view.effective_html_labels() {
        for label in labels
            .edge_labels()
            .chain(labels.subgraph_labels().map(Some))
        {
            control.checkpoint()?;
            if let Some((label, kind)) = label
                && let Some(property) = inspect(label, kind)?
            {
                return Ok(Some(property));
            }
        }
    }
    Ok(None)
}
