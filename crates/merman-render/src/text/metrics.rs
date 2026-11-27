//! Renderer-level label measurement and Flowchart typography admission.

#[cfg(all(
    test,
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-mindmap"
    )
))]
pub(crate) use merman_render_core::text::inline_html::style_requests_bold_font_weight;

#[cfg(test)]
mod inline_frontend_tests;

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) fn flowchart_label_xhtml_for_typography_admission(
    label: &str,
    label_type: &str,
    config: &merman_model::presentation::PresentationConfig,
    control: &merman_model::OperationControl,
) -> Result<String, merman_model::OperationCancelled> {
    control.checkpoint()?;
    let html = crate::graph_label::flowchart_label_html_source(label, label_type, config);
    // Sanitizer-removed attributes and encoded angle brackets cannot affect rendered typography.
    let work_meter = crate::resources::OperationWorkMeter::new_with_control(
        crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        control.clone(),
    );
    let parse_work = std::cell::Cell::new(0);
    let sink = merman_render_core::html::HtmlSanitizeSink::new(
        usize::MAX,
        &work_meter,
        &parse_work,
        merman_model::OperationPhase::Layout,
    );
    let sanitize_config = crate::graph_label::flowchart_label_sanitize_config(config);
    match merman_markup::sanitize::sanitize_xhtml_with_sink(&html, sanitize_config.as_ref(), &sink)
    {
        Ok(html) => Ok(html),
        Err(merman_markup::sanitize::SanitizeFailure::Output(
            merman_render_core::html::HtmlOutputError::Operation(
                crate::resources::OperationWorkError::Cancelled(error),
            ),
        )) => Err(error),
        Err(_) => Ok(String::new()),
    }
}

/// Finds retained inline HTML typography that the caller's measurement policy cannot represent.
/// Admission uses the same Markdown projection and HTML normalization as SVG emission.
#[doc(hidden)]
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub fn unsupported_html_label_style(
    label: &str,
    label_type: &str,
    config: &merman_model::presentation::PresentationConfig,
    unsupported_properties: &[&str],
    control: &merman_model::OperationControl,
) -> Result<Option<String>, merman_model::OperationCancelled> {
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

#[cfg(all(
    test,
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    )
))]
#[test]
fn unsupported_html_style_checks_cancellation_during_fragment_recovery() {
    let control = merman_model::OperationControl::new();
    control.cancel_after_checkpoints(8);
    let label = "<span>label</span>".repeat(64);
    let error = unsupported_html_label_style(
        &label,
        "text",
        &merman_model::presentation::PresentationConfig::default(),
        &[],
        &control,
    )
    .expect_err("typography admission must cancel while recovering HTML");
    assert_eq!(control.checkpoint(), Err(error));
}
