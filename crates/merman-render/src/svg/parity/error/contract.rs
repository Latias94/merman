use super::super::util::{escape_attr_into, escape_xml_into, fmt};
use crate::drawing_list::{ERROR_ICON_PATHS, ErrorSvgBody};
use crate::portable_font::PortableFontFamilies;
use crate::{Error, Result};
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingResource, FillRule, FontStyle, LineCap, LineJoin, MeasurementProvenance, Paint,
    PathStyle, Point, Rect, ResourceId, SemanticRole, TextAnchor, TextBaseline, TextDirection,
    TextObligation, TextPaintOrder, TextRun,
};
use serde_json::{Map, Value};
use std::fmt::Write as _;

const ERROR_DOCUMENT_ID: &str = "error.document";
const ERROR_TITLE: &str = "Syntax error in text";
const ERROR_VIEWPORT: Rect = Rect::new(0.0, 0.0, 2412.0, 512.0);
const ERROR_MAX_WIDTH: f64 = 512.0;
const ERROR_TEXT_PROFILE: &str = "error-fixed-text";

/// Renderer-neutral facts that are safe to project into Error SVG presentation CSS.
pub(in crate::svg::parity) struct ErrorProjection<'a> {
    icon_color: Color,
    text_color: Color,
    font_families: &'a [String],
}

/// Verifies the complete Error-family contract before the SVG encoder writes any bytes.
pub(in crate::svg::parity) fn validate_error_projection_contract<'a>(
    document: &'a DrawingListDocument,
    body: &ErrorSvgBody,
) -> Result<ErrorProjection<'a>> {
    if document.version != DRAWING_LIST_VERSION
        || document.coordinate_system != CoordinateSystem::LogicalPixelsYDown
        || document.viewport.bounds != ERROR_VIEWPORT
    {
        return Err(invalid(
            "Error DrawingList no longer has the fixed renderer-neutral viewport contract",
        ));
    }
    if body.max_width_px != ERROR_MAX_WIDTH {
        return Err(invalid(
            "Error SVG sidecar no longer has the fixed max-width contract",
        ));
    }
    if !document.fallbacks.is_empty() || !document.extensions.is_empty() {
        return Err(invalid(
            "Error DrawingList must not contain fallbacks or extensions",
        ));
    }

    let [semantic] = document.semantics.as_slice() else {
        return Err(invalid(
            "Error DrawingList must contain exactly one document semantic",
        ));
    };
    let expected_version = format!("mermaid version {}", crate::error::UPSTREAM_MERMAID_VERSION);
    if semantic.id != ERROR_DOCUMENT_ID
        || semantic.role != SemanticRole::Document
        || semantic.title.as_deref() != Some(ERROR_TITLE)
        || semantic.description.as_deref() != Some(expected_version.as_str())
        || semantic.link.is_some()
    {
        return Err(invalid(
            "Error DrawingList document semantic no longer matches the source-backed contract",
        ));
    }

    if document.resources.len() != ERROR_ICON_PATHS.len() {
        return Err(invalid(
            "Error DrawingList must contain exactly six ordered path resources",
        ));
    }
    for (index, (resource, source)) in document.resources.iter().zip(ERROR_ICON_PATHS).enumerate() {
        let DrawingResource::Path(path) = resource else {
            return Err(invalid(format!(
                "Error resource {index} must be a path resource"
            )));
        };
        let expected_id = format!("error.icon.{index}");
        if path.id.as_str() != expected_id {
            return Err(invalid(format!(
                "Error resource {index} has unexpected id {}",
                path.id.as_str()
            )));
        }
        if path.segments != crate::drawing_list::parse_svg_path(source)? {
            return Err(invalid(format!(
                "Error resource {index} no longer matches its source-backed path geometry"
            )));
        }
    }

    if document.commands.len() != 12
        || !matches!(document.commands.first(), Some(DrawingCommand::Save))
        || !matches!(
            document.commands.get(1),
            Some(DrawingCommand::BeginSemanticGroup { semantic_id })
                if semantic_id == ERROR_DOCUMENT_ID
        )
        || !matches!(
            document.commands.get(10),
            Some(DrawingCommand::EndSemanticGroup)
        )
        || !matches!(document.commands.get(11), Some(DrawingCommand::Restore))
    {
        return Err(invalid(
            "Error DrawingList command stream no longer matches Save/semantic/body/Restore order",
        ));
    }

    let Some(DrawingCommand::DrawPath {
        style: first_path_style,
        ..
    }) = document.commands.get(2)
    else {
        return Err(invalid("Error command 2 must draw icon path 0"));
    };
    let background = solid_paint_color(
        first_path_style.fill.as_ref(),
        "Error icon fill must be a resolved solid paint",
    )?;
    let expected_path_style = PathStyle {
        fill_rule: FillRule::NonZero,
        fill: Some(Paint::solid(background)),
        stroke: None,
    };
    for index in 0..ERROR_ICON_PATHS.len() {
        let Some(DrawingCommand::DrawPath { path, style }) = document.commands.get(index + 2)
        else {
            return Err(invalid(format!(
                "Error command {} must draw icon path {index}",
                index + 2
            )));
        };
        if path.as_str() != format!("error.icon.{index}") || style != &expected_path_style {
            return Err(invalid(format!(
                "Error icon command {index} no longer matches its source-backed paint contract"
            )));
        }
    }

    let Some(DrawingCommand::DrawText { run: title_run }) = document.commands.get(8) else {
        return Err(invalid("Error command 8 must draw the syntax-error title"));
    };
    let text_color = solid_paint_color(
        Some(&title_run.style.fill),
        "Error text fill must be a resolved solid paint",
    )?;
    PortableFontFamilies::from_resolved(&title_run.style.font.families).map_err(|error| {
        invalid(format!(
            "Error host text has invalid font families: {error}"
        ))
    })?;
    validate_error_text_run(
        title_run,
        ERROR_TITLE,
        Point::new(1440.0, 250.0),
        Rect::new(0.0, 100.0, ERROR_VIEWPORT.width, 180.0),
        150.0,
        text_color,
        &title_run.style.font.families,
    )?;

    let Some(DrawingCommand::DrawText { run: version_run }) = document.commands.get(9) else {
        return Err(invalid("Error command 9 must draw the Mermaid version"));
    };
    validate_error_text_run(
        version_run,
        semantic
            .description
            .as_deref()
            .ok_or_else(|| invalid("Error semantic is missing its version description"))?,
        Point::new(1250.0, 400.0),
        Rect::new(0.0, 300.0, ERROR_VIEWPORT.width, 130.0),
        100.0,
        text_color,
        &title_run.style.font.families,
    )?;

    Ok(ErrorProjection {
        icon_color: background,
        text_color,
        font_families: &title_run.style.font.families,
    })
}

fn solid_paint_color(paint: Option<&Paint>, message: &str) -> Result<Color> {
    match paint {
        Some(Paint::Solid { color }) => Ok(*color),
        Some(Paint::Resource { .. }) | None => Err(invalid(message)),
    }
}

fn validate_error_text_run(
    run: &TextRun,
    expected_text: &str,
    expected_origin: Point,
    expected_bounds: Rect,
    expected_font_size: f64,
    expected_color: merman_display_list::Color,
    expected_families: &[String],
) -> Result<()> {
    let Some(stroke) = run.style.stroke.as_ref() else {
        return Err(invalid(
            "Error text must retain Mermaid's resolved text stroke",
        ));
    };
    if run.text != expected_text
        || run.origin != expected_origin
        || run.bounds != expected_bounds
        || run.style.font.families != expected_families
        || run.style.font.weight != 400
        || run.style.font.style != FontStyle::Normal
        || run.style.font.postscript_name.is_some()
        || run.style.font.resource.is_some()
        || run.style.font_size != expected_font_size
        || run.style.letter_spacing != 0.0
        || run.style.line_height != expected_font_size
        || run.style.fill != Paint::solid(expected_color)
        || stroke.paint != Paint::solid(expected_color)
        || stroke.width != 1.0
        || !stroke.dash_array.is_empty()
        || stroke.dash_offset != 0.0
        || stroke.line_cap != LineCap::Butt
        || stroke.line_join != LineJoin::Miter
        || stroke.miter_limit != 4.0
        || run.style.paint_order != TextPaintOrder::FillThenStroke
        || run.anchor != TextAnchor::Middle
        || run.baseline != TextBaseline::Alphabetic
        || run.direction != TextDirection::Auto
        || run.language.is_some()
        || !matches!(
            &run.obligation,
            TextObligation::HostText {
                measurement: MeasurementProvenance::DeterministicFallback { profile }
            } if profile == ERROR_TEXT_PROFILE
        )
    {
        return Err(invalid(
            "Error text no longer matches the source-backed geometry, font, paint, or host-text obligation",
        ));
    }
    Ok(())
}

/// Builds Mermaid's shared stylesheet with every Error-active token overwritten from the
/// validated public document. Every selector that can match Error DOM receives its font, fill,
/// and stroke tokens from `projection`; other theme values only feed selectors absent from Error.
pub(in crate::svg::parity) fn canonical_error_css(
    diagram_id: &str,
    effective_config: &Value,
    projection: &ErrorProjection<'_>,
) -> Result<String> {
    if !effective_config.is_object() {
        return Err(invalid("Error effective configuration must be an object"));
    }

    // Error's renderer-neutral document already carries every visual value used by its icon and
    // text. Do not copy arbitrary theme tokens into the shared stylesheet: even a shallow CSS
    // scalar such as `var(--host-color)` would introduce a second, unresolved visual source for
    // the Error DOM. Keeping this projection closed also means deeply nested host configuration
    // is never cloned or dropped by the canonical serializer.
    let mut root = Map::new();
    let mut variables = Map::new();
    variables.insert(
        "errorBkgColor".to_string(),
        Value::String(error_color_css(projection.icon_color)),
    );
    variables.insert(
        "errorTextColor".to_string(),
        Value::String(error_color_css(projection.text_color)),
    );
    variables.insert(
        "fontFamily".to_string(),
        Value::String(
            PortableFontFamilies::from_resolved(projection.font_families)
                .map_err(|error| {
                    invalid(format!(
                        "Error host text has invalid font families: {error}"
                    ))
                })?
                .to_css(),
        ),
    );
    root.insert("themeVariables".to_string(), Value::Object(variables));
    let css_config = Value::Object(root);
    Ok(super::super::info_css_with_config(diagram_id, &css_config))
}

fn error_color_css(color: Color) -> String {
    if color.alpha == u8::MAX {
        format!("#{:02x}{:02x}{:02x}", color.red, color.green, color.blue)
    } else {
        format!(
            "rgba({},{},{},{})",
            color.red,
            color.green,
            color.blue,
            fmt(f64::from(color.alpha) / 255.0)
        )
    }
}

pub(in crate::svg::parity) fn write_error_path(
    output: &mut String,
    path_id: &ResourceId,
) -> Result<()> {
    let source = error_icon_source(path_id)?;
    output.push_str(r#"<path class="error-icon" d=""#);
    escape_attr_into(output, source);
    output.push_str("\"/>");
    Ok(())
}

pub(in crate::svg::parity) fn write_error_text(output: &mut String, run: &TextRun) -> Result<()> {
    write!(
        output,
        r#"<text class="error-text" x="{}" y="{}" font-size="{}px" style="text-anchor: middle;">"#,
        fmt(run.origin.x),
        fmt(run.origin.y),
        fmt(run.style.font_size),
    )
    .map_err(|_| invalid("failed to write Error text"))?;
    escape_xml_into(output, run.text.as_str());
    output.push_str("</text>");
    Ok(())
}

fn error_icon_source(path_id: &ResourceId) -> Result<&'static str> {
    let raw_index = path_id
        .as_str()
        .strip_prefix("error.icon.")
        .ok_or_else(|| {
            invalid(format!(
                "unexpected Error path resource {}",
                path_id.as_str()
            ))
        })?;
    let index = raw_index
        .parse::<usize>()
        .ok()
        .filter(|index| *index < ERROR_ICON_PATHS.len())
        .ok_or_else(|| invalid(format!("unexpected Error icon index {raw_index}")))?;
    Ok(ERROR_ICON_PATHS[index])
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LayoutOptions;
    use crate::environment::RenderEnvironment;
    use merman_core::{Engine, OperationControl, ParseOptions};
    use merman_display_list::{DrawingListLimits, DrawingListPolicy};

    fn canonical_error_document() -> DrawingListDocument {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync("flowchart TD\nA -->\n", ParseOptions::lenient())
            .expect("Error fixture parses")
            .expect("Error fixture detects a diagram");
        let session = RenderEnvironment::deterministic()
            .begin_session_with_control(OperationControl::new())
            .expect("render session starts");
        let artifact =
            crate::family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
                .expect("Error family layout succeeds");
        let rendered = artifact
            .render_drawing_list(
                DrawingListPolicy::AllowRasterSubtree,
                DrawingListLimits::default(),
            )
            .expect("Error DrawingList builds");
        rendered.document().clone()
    }

    #[test]
    fn error_css_uses_the_validated_document_instead_of_conflicting_config() {
        let document = canonical_error_document();
        let projection = validate_error_projection_contract(&document, &ErrorSvgBody::new())
            .expect("the actual Error builder must produce a valid projection");
        let conflicting_config = serde_json::json!({
            "fontFamily": "Top Level Conflict",
            "themeVariables": {
                "errorBkgColor": "#010203",
                "errorTextColor": "#040506",
                "fontFamily": "Theme Conflict"
            }
        });

        let css = canonical_error_css("error-source-contract", &conflicting_config, &projection)
            .expect("validated Error projection must produce CSS");

        assert!(css.contains(
            r#"#error-source-contract{font-family:"trebuchet ms",verdana,arial,sans-serif;"#
        ));
        assert!(css.contains("#error-source-contract .error-icon{fill:#552222;}"));
        assert!(css.contains("#error-source-contract .error-text{fill:#552222;stroke:#552222;}"));
        assert!(!css.contains("Conflict"));
        assert!(!css.contains("#010203"));
        assert!(!css.contains("#040506"));
    }

    #[test]
    fn error_css_does_not_clone_unrelated_deep_host_config() {
        let document = canonical_error_document();
        let projection = validate_error_projection_contract(&document, &ErrorSvgBody::new())
            .expect("the actual Error builder must produce a valid projection");
        let mut nested = Value::Null;
        for _ in 0..8_192 {
            nested = Value::Array(vec![nested]);
        }
        let mut variables = Map::new();
        variables.insert("unrelated".to_string(), nested);
        let mut root = Map::new();
        root.insert("themeVariables".to_string(), Value::Object(variables));
        let config = merman_core::MermaidConfig::from_value(Value::Object(root));

        let css = canonical_error_css("error-deep-config", config.as_value(), &projection)
            .expect("deep unrelated host config must not enter Error CSS projection");

        assert!(css.contains("#error-deep-config .error-icon{fill:#552222;}"));
    }

    #[test]
    fn error_css_does_not_reintroduce_unresolved_host_tokens() {
        let document = canonical_error_document();
        let projection = validate_error_projection_contract(&document, &ErrorSvgBody::new())
            .expect("the actual Error builder must produce a valid projection");
        let config = serde_json::json!({
            "fontSize": "var(--host-size)",
            "themeVariables": {
                "textColor": "var(--host-text)",
                "lineColor": "currentColor",
                "nodeBorder": "var(--host-border)",
                "dropShadow": "var(--host-shadow)"
            }
        });

        let css = canonical_error_css("error-closed-projection", &config, &projection)
            .expect("unrelated host CSS must not affect Error canonical CSS");

        assert!(!css.contains("var(--host-"));
        assert!(!css.contains("currentColor"));
        assert!(css.contains("#error-closed-projection{font-family:"));
        assert!(css.contains("#error-closed-projection .error-icon{fill:#552222;}"));
    }

    #[test]
    fn error_projection_contract_rejects_public_document_mutations() {
        let document = canonical_error_document();
        let body = ErrorSvgBody::new();
        validate_error_projection_contract(&document, &body)
            .expect("the actual Error builder must satisfy its SVG projection contract");

        type DocumentMutation = fn(&mut DrawingListDocument);
        let mutations: [(&str, DocumentMutation); 8] = [
            ("resource order", |document| document.resources.swap(0, 1)),
            ("extra resource", |document| {
                let mut extra = document.resources[0].clone();
                let DrawingResource::Path(path) = &mut extra else {
                    unreachable!("Error resource zero is a path");
                };
                path.id = ResourceId::new("error.icon.extra");
                document.resources.push(extra);
            }),
            ("semantic payload", |document| {
                document.semantics[0].description = Some("different version".to_string());
            }),
            ("path geometry", |document| {
                let DrawingResource::Path(path) = &mut document.resources[0] else {
                    unreachable!("Error resource zero is a path");
                };
                path.segments.pop();
            }),
            ("text stroke", |document| {
                let DrawingCommand::DrawText { run } = &mut document.commands[8] else {
                    unreachable!("Error command eight is text");
                };
                run.style.stroke = None;
            }),
            ("graphics state", |document| {
                document.commands[2] = DrawingCommand::SetOpacity { opacity: 0.5 };
            }),
            ("command order", |document| document.commands.swap(2, 3)),
            ("extra extension", |document| {
                document
                    .extensions
                    .insert("unexpected".to_string(), Value::Bool(true));
            }),
        ];

        for (name, mutate) in mutations {
            let mut mutated = document.clone();
            mutate(&mut mutated);
            assert!(
                validate_error_projection_contract(&mutated, &body).is_err(),
                "{name} mutation must fail closed"
            );
        }

        let mut mutated_body = body.clone();
        mutated_body.max_width_px = 511.0;
        assert!(
            validate_error_projection_contract(&document, &mutated_body).is_err(),
            "sidecar max-width mutation must fail closed"
        );
    }
}
