use super::{ErBoxLabel, ErConfigView, swimlane_relationship_html_source};
use merman_model::families::er::ErDiagramModel;
use merman_model::presentation::PresentationConfig;
use merman_model::{OperationControl, OperationControlResult, OperationPhase};
use merman_render_core::{
    html::{HtmlOutputError, HtmlSanitizeSink},
    mermaid_style::native_typography::unsupported_retained_html_style,
    resources::{OperationWorkError, OperationWorkMeter, RenderResourcePolicy},
    text::{WrapMode, mermaid_markdown_to_xhtml_label_fragment},
};
use std::collections::HashSet;

/// Checks the retained ER HTML labels before native typography measurement.
pub fn unsupported_html_style(
    model: &ErDiagramModel,
    config: &PresentationConfig,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    let view = ErConfigView::new(config);
    let mut subgraph_ids = HashSet::with_capacity(model.subgraphs.len());
    for group in &model.subgraphs {
        control.checkpoint()?;
        subgraph_ids.insert(group.id.as_str());
    }
    if view.entity_html_label_wrap_mode() == WrapMode::HtmlLike {
        for (name, entity) in &model.entities {
            control.checkpoint()?;
            // ErDB.getData omits an entity whose source name is a subgraph id.
            if subgraph_ids.contains(name.as_str()) {
                continue;
            }
            if let Some(property) = box_label_style(
                entity.label_source_for_render(),
                !entity.attributes.is_empty(),
                control,
            )? {
                return Ok(Some(property));
            }
            for attribute in &entity.attributes {
                for source in [&attribute.ty, &attribute.name, &attribute.comment] {
                    if let Some(property) = box_label_style(source, true, control)? {
                        return Ok(Some(property));
                    }
                }
            }
        }
    }

    // This typography probe also runs without a prepared render plan.
    let swimlane = config.get_str("layout") == Some("swimlane");
    let relationship_html_labels = view.relationship_html_labels(swimlane);
    if relationship_html_labels {
        for relationship in &model.relationships {
            control.checkpoint()?;
            let html = if swimlane {
                let source = swimlane_relationship_html_source(&relationship.role_a);
                let Some(html) = sanitize_relationship(&source, config, control)? else {
                    return Ok(Some("ER relationship HTML normalization".into()));
                };
                html
            } else {
                mermaid_markdown_to_xhtml_label_fragment(
                    relationship.role_a.trim(),
                    config.get_bool("markdownAutoWrap").unwrap_or(true),
                )
            };
            if let Some(property) = unsupported_retained_html_style(&html, control)? {
                return Ok(Some(property));
            }
        }
    }

    // Match the provider's parent lookup once, rather than searching all groups per label.
    let mut children = HashSet::new();
    if swimlane {
        for group in &model.subgraphs {
            control.checkpoint()?;
            for member in &group.nodes {
                control.checkpoint()?;
                children.insert(member.as_str());
            }
        }
    }
    for group in &model.subgraphs {
        control.checkpoint()?;
        let is_lane = swimlane && !children.contains(group.id.as_str());
        if !view.subgraph_html_labels(is_lane, relationship_html_labels)
            || (!is_lane && matches!(group.label_type.as_str(), "string" | "text"))
        {
            // Ordinary string/text titles are escaped by the cluster writer.
            continue;
        }
        let html = mermaid_markdown_to_xhtml_label_fragment(&group.title, true);
        if let Some(property) = unsupported_retained_html_style(&html, control)? {
            return Ok(Some(property));
        }
    }
    Ok(None)
}

fn box_label_style(
    source: &str,
    expand_generic_types: bool,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    control.checkpoint()?;
    let label = ErBoxLabel::from_source_with_generic_types(source, expand_generic_types);
    if label.uses_generic_workaround() {
        // erBox's textContent assignment removes all inline elements before painting.
        return Ok(None);
    }
    unsupported_retained_html_style(label.xhtml_fragment(), control)
}

fn sanitize_relationship(
    source: &str,
    config: &PresentationConfig,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    // Same policy as ErSwimlaneRelationshipLabel::prepare, with admission cancellation.
    let work = OperationWorkMeter::new_with_control(
        RenderResourcePolicy::unbounded_for_trusted_input(),
        control.clone(),
    );
    let parse_work = std::cell::Cell::new(0);
    let sink = HtmlSanitizeSink::new(usize::MAX, &work, &parse_work, OperationPhase::Semantic);
    match merman_markup::sanitize::sanitize_xhtml_with_sink(source, config.as_config(), &sink) {
        Ok(html) => Ok(Some(html)),
        Err(merman_markup::sanitize::SanitizeFailure::Output(HtmlOutputError::Operation(
            OperationWorkError::Cancelled(error),
        ))) => Err(error),
        Err(_) => Ok(None),
    }
}
