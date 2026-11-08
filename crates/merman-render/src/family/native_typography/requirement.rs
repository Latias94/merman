use super::{
    OperationControl, OperationControlResult, OperationPhase, declaration_scan, retained_html,
};
use crate::requirement::{RequirementConfigView, RequirementNodeTypography};
use merman_model::families::requirement::RequirementDiagramModel;
use merman_model::presentation::PresentationConfig;
use std::collections::HashSet;

pub(super) fn unsupported_text_style(
    model: &RequirementDiagramModel,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    let mut retained_ids = HashSet::new();
    let mut retained_styles = Vec::new();
    for (name, styles) in model
        .requirements
        .iter()
        .rev()
        .map(|node| (node.name.as_str(), node.css_styles.as_slice()))
        .chain(
            model
                .elements
                .iter()
                .rev()
                .map(|node| (node.name.as_str(), node.css_styles.as_slice())),
        )
    {
        control.checkpoint()?;
        if retained_ids.insert(name) {
            retained_styles.push(styles);
        }
    }
    for styles in model
        .classes
        .values()
        .flat_map(|class| [class.styles.as_slice(), class.text_styles.as_slice()])
        .chain(retained_styles.iter().copied())
    {
        for raw in styles {
            control.checkpoint()?;
            if crate::mermaid_style::parse_style_declaration(raw)
                .is_some_and(|declaration| declaration.property() == "font-size")
            {
                continue;
            }
            if let Some(property) = declaration_scan::declarations([raw.as_str()], control)? {
                return Ok(Some(property));
            }
        }
    }
    // styles2String replaces earlier values, including their source importance. Check the
    // same final size as the measurement owner, rather than rejecting a shadowed history.
    for styles in retained_styles {
        if RequirementNodeTypography::has_unmeasured_font_size(styles, control)? {
            return Ok(Some(
                "font-size (Requirement requires an absolute CSS length)".into(),
            ));
        }
    }
    Ok(None)
}

pub(super) fn unsupported_html_style(
    model: &RequirementDiagramModel,
    config: &PresentationConfig,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    control.checkpoint()?;
    // Requirement nodes use root htmlLabels independently of the shared edge-label mode.
    // Relationship labels contain only the relationship enum, not user-authored HTML.
    if !RequirementConfigView::new(config.as_value())
        .layout_settings()
        .html_labels
    {
        return Ok(None);
    }
    let mut retained_ids = HashSet::new();
    // Prepared node plans keep the last node per name; a Requirement takes precedence over
    // an Element with the same name. Inspect only the rows that reach that final plan.
    for node in model.requirements.iter().rev() {
        control.checkpoint()?;
        if !retained_ids.insert(node.name.as_str()) {
            continue;
        }
        for source in crate::requirement::requirement_node_label_sources(node) {
            if let Some(property) = label_style(&source, config, control)? {
                return Ok(Some(property));
            }
        }
    }
    for node in model.elements.iter().rev() {
        control.checkpoint()?;
        if !retained_ids.insert(node.name.as_str()) {
            continue;
        }
        for source in crate::requirement::element_node_label_sources(node) {
            if let Some(property) = label_style(&source, config, control)? {
                return Ok(Some(property));
            }
        }
    }
    Ok(None)
}

fn label_style(
    source: &str,
    config: &PresentationConfig,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    control.checkpoint()?;
    if source.trim().is_empty() {
        return Ok(None);
    }
    let work = crate::resources::OperationWorkMeter::new_with_control(
        crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        control.clone(),
    );
    let parse_work = std::cell::Cell::new(0);
    let sink = crate::html::HtmlSanitizeSink::new(
        usize::MAX,
        &work,
        &parse_work,
        OperationPhase::Semantic,
    );
    let sanitized =
        match crate::requirement::sanitize_label_with_sink(source, config.as_config(), &sink) {
            Ok(source) => source,
            Err(merman_markup::sanitize::SanitizeFailure::Output(
                crate::html::HtmlOutputError::Operation(
                    crate::resources::OperationWorkError::Cancelled(error),
                ),
            )) => return Err(error),
            Err(_) => return Ok(Some("Requirement HTML normalization".into())),
        };
    control.checkpoint()?;
    let html = crate::text::mermaid_markdown_to_xhtml_label_fragment(&sanitized, true);
    retained_html::unsupported_style(&html, control)
}
