//! Cynefin DOM scopes projected from canonical semantic groups and transforms.

use super::*;
use crate::render_geometry::cynefin::{MARKER_SIZE, REF_X, REF_Y, VIEW_BOX_SIZE, marker_transform};
use merman_display_list::{StrokeStyle, TextStyle};

#[derive(Clone, Copy, PartialEq)]
pub(super) struct PathCssStyle<'a> {
    stroke: Option<&'a StrokeStyle>,
    marker_fill: Option<&'a Paint>,
}

struct FillAndStroke<'a> {
    path: &'a ResourceId,
    class: &'a str,
    fill: &'a Paint,
    stroke: &'a StrokeStyle,
    opacity: f64,
    fill_rule: FillRule,
}

/// Shared by CSS collection and emission: either both see one paint operation or neither does.
fn fill_and_stroke<'a>(
    commands: &'a [DrawingCommand],
    index: usize,
    body: &'a crate::drawing_list::CynefinSvgBody,
    state: GraphicsState,
) -> Option<FillAndStroke<'a>> {
    if state.opacity != 1.0 || state.blend_mode != BlendMode::Normal {
        return None;
    }
    let [
        DrawingCommand::Save,
        DrawingCommand::SetOpacity { opacity },
        DrawingCommand::DrawPath { path, style: fill },
        DrawingCommand::Restore,
        DrawingCommand::DrawPath {
            path: stroke_path,
            style: stroke,
        },
    ] = commands.get(index..index.checked_add(5)?)?
    else {
        return None;
    };
    let class = body.path_classes.get(path.as_str())?;
    if !matches!(
        class.as_str(),
        "cynefinConfusion" | "cynefinItem" | "cynefinItemOverflow"
    ) || path != stroke_path
        || fill.fill_rule != stroke.fill_rule
        || fill.stroke.is_some()
        || stroke.fill.is_some()
        || !matches!(fill.fill, Some(Paint::Solid { .. }))
    {
        return None;
    }
    Some(FillAndStroke {
        path,
        class,
        fill: fill.fill.as_ref()?,
        stroke: stroke.stroke.as_ref()?,
        opacity: *opacity,
        fill_rule: fill.fill_rule,
    })
}

pub(super) fn shared_path_styles<'a>(
    document: &'a DrawingListDocument,
    body: &'a crate::drawing_list::CynefinSvgBody,
    session: &RenderSession,
) -> Result<BTreeMap<String, Option<PathCssStyle<'a>>>> {
    let mut styles = BTreeMap::new();
    let mut state = GraphicsState::default();
    let mut saves = Vec::new();
    let mut index = 0;
    while index < document.commands.len() {
        session.checkpoint(OperationPhase::Emit)?;
        let combined = fill_and_stroke(&document.commands, index, body, state);
        let (path, fill_rule, stroke, fill) = if let Some(combined) = combined {
            index += 5;
            (
                combined.path,
                combined.fill_rule,
                Some(combined.stroke),
                Some(combined.fill),
            )
        } else {
            let command = &document.commands[index];
            index += 1;
            match command {
                DrawingCommand::Save => {
                    saves.try_reserve(1).map_err(|_| {
                        crate::Error::DrawingListAllocationFailed {
                            collection: "SVG paint scopes",
                        }
                    })?;
                    saves.push(state);
                }
                DrawingCommand::Restore => {
                    state = saves
                        .pop()
                        .ok_or_else(|| invalid("SVG paint restore has no save"))?
                }
                DrawingCommand::SetOpacity { opacity } => state.opacity = *opacity,
                DrawingCommand::SetBlendMode { blend_mode } => state.blend_mode = *blend_mode,
                _ => {}
            }
            let DrawingCommand::DrawPath { path, style } = command else {
                continue;
            };
            (
                path,
                style.fill_rule,
                style.stroke.as_ref(),
                style.fill.as_ref(),
            )
        };
        let Some(class) = body.path_classes.get(path.as_str()) else {
            continue;
        };
        if !matches!(
            class.as_str(),
            "cynefinDomain"
                | "cynefinBoundary"
                | "cynefinCliff"
                | "cynefinConfusion"
                | "cynefinItem"
                | "cynefinItemOverflow"
                | "cynefinArrowLine"
                | "cynefinArrowHead"
        ) {
            continue;
        }
        let marker_fill = (class == "cynefinArrowHead").then_some(fill).flatten();
        let candidate = (fill_rule == FillRule::NonZero
            && stroke.is_none_or(|stroke| matches!(stroke.paint, Paint::Solid { .. }))
            && marker_fill.is_none_or(|paint| matches!(paint, Paint::Solid { .. })))
        .then_some(PathCssStyle {
            stroke,
            marker_fill,
        });
        styles
            .entry(class.clone())
            .and_modify(|shared| {
                if *shared != candidate {
                    *shared = None;
                }
            })
            .or_insert(candidate);
    }
    Ok(styles)
}

/// Only identical resolved styles may move into a shared class rule. Edited heterogeneous
/// commands retain explicit attributes, so class projection never invents a visual default.
pub(super) fn shared_text_styles<'a>(
    document: &'a DrawingListDocument,
    body: &crate::drawing_list::CynefinSvgBody,
    session: &RenderSession,
) -> Result<BTreeMap<String, Option<&'a TextStyle>>> {
    let mut styles = BTreeMap::new();
    let mut groups = Vec::new();
    for command in &document.commands {
        session.checkpoint(OperationPhase::Emit)?;
        match command {
            DrawingCommand::BeginSemanticGroup { semantic_id } => groups.push(semantic_id.as_str()),
            DrawingCommand::EndSemanticGroup => {
                groups.pop();
            }
            DrawingCommand::DrawText { run } => {
                let Some(class) = groups.last().and_then(|id| body.text_classes.get(*id)) else {
                    continue;
                };
                // Class selectors are structural metadata, not executable source CSS.
                if !matches!(
                    class.as_str(),
                    "cynefinDomainLabel"
                        | "cynefinSubtitle"
                        | "cynefinItemText"
                        | "cynefinArrowLabel"
                        | "cynefinTitle"
                ) {
                    continue;
                }
                let candidate = (run.style.font.resource.is_none()
                    && run.style.stroke.is_none()
                    && matches!(run.style.fill, Paint::Solid { .. }))
                .then_some(&run.style);
                styles
                    .entry(class.clone())
                    .and_modify(|shared| {
                        if *shared != candidate {
                            *shared = None;
                        }
                    })
                    .or_insert(candidate);
            }
            _ => {}
        }
    }
    Ok(styles)
}

impl DocumentSvgEncoder<'_> {
    pub(super) fn emit_cynefin_fill_and_stroke(&mut self, index: usize) -> Result<bool> {
        let SvgStructureBody::Cynefin(body) = self.svg_body else {
            return Ok(false);
        };
        let Some(combined) = fill_and_stroke(&self.document.commands, index, body, self.state)
        else {
            return Ok(false);
        };
        let path = self.path_resource(combined.path)?;
        let rounded = if matches!(combined.class, "cynefinItem" | "cynefinItemOverflow") {
            rounded_rectangle_from_path(path)
        } else {
            None
        };
        if let Some((bounds, radius)) = rounded {
            write!(
                self.output,
                "<rect class=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" ry=\"{}\"",
                combined.class,
                fmt(bounds.x),
                fmt(bounds.y),
                fmt(bounds.width),
                fmt(bounds.height),
                fmt(radius),
                fmt(radius)
            )?;
        } else {
            write!(
                self.output,
                "<path class=\"{}\" d=\"{}\"",
                combined.class,
                path_d(&path.segments)
            )?;
        }
        let Paint::Solid { color, opacity } = combined.fill else {
            return Err(invalid("Cynefin combined fill must be solid"));
        };
        write!(
            self.output,
            " fill=\"{}\" fill-opacity=\"{}\"",
            color_css(*color),
            fmt(combined.opacity * f64::from(color.alpha) / 255.0 * opacity)
        )?;
        let shared = self
            .cynefin_path_styles
            .get(combined.class)
            .and_then(Option::as_ref);
        if !shared.is_some_and(|style| style.stroke == Some(combined.stroke)) {
            // An edited sibling disables class sharing. Explicit stroke paint must not
            // overwrite the independently resolved fill opacity above.
            self.write_stroke_style(Some(combined.stroke))?;
        }
        if combined.fill_rule != FillRule::NonZero {
            write!(
                self.output,
                " fill-rule=\"{}\"",
                fill_rule_name(combined.fill_rule)
            )?;
        }
        self.write_state_attrs()?;
        self.output.push_str("/>")?;
        Ok(true)
    }

    pub(super) fn write_cynefin_accessibility_copies(
        &mut self,
        title: Option<&str>,
        description: Option<&str>,
    ) -> Result<()> {
        // The family renderer appends these after Mermaid's identified root metadata.
        // Both DOM copies consume the same public document semantics.
        for (tag, text) in [("title", title), ("desc", description)] {
            if let Some(text) = text {
                write!(self.output, "<{tag}>")?;
                output::escape_xml(&mut self.output, text)?;
                write!(self.output, "</{tag}>")?;
            }
        }
        Ok(())
    }

    pub(super) fn write_cynefin_styles(&mut self) -> Result<()> {
        self.output.push_str("<style>")?;
        for (class, style) in &self.cynefin_text_styles {
            let Some(style) = style else {
                continue;
            };
            let Paint::Solid { color, .. } = style.fill else {
                return Err(invalid("Cynefin shared text paint must be solid"));
            };
            let font = self.font_families(&style.font)?;
            write!(
                self.output,
                "#{} .{}{{font-family:{};font-size:{}px;font-weight:{};font-style:{};letter-spacing:{}px;fill:{};fill-opacity:{};}}",
                self.diagram_id,
                class,
                font,
                fmt(style.font_size),
                style.font.weight,
                font_style(style.font.style),
                fmt(style.letter_spacing),
                color_css(color),
                fmt(paint_opacity(&style.fill))
            )?;
        }
        for (class, style) in &self.cynefin_path_styles {
            let Some(style) = style else {
                continue;
            };
            write!(self.output, "#{} .{}{{", self.diagram_id, class)?;
            if let Some(stroke) = style.stroke {
                let Paint::Solid { color, .. } = stroke.paint else {
                    return Err(invalid("Cynefin shared stroke must be solid"));
                };
                write!(
                    self.output,
                    "stroke:{};stroke-opacity:{};stroke-width:{};stroke-linecap:{};stroke-linejoin:{};stroke-miterlimit:{};stroke-dashoffset:{};stroke-dasharray:",
                    color_css(color),
                    fmt(paint_opacity(&stroke.paint)),
                    fmt(stroke.width),
                    line_cap(stroke.line_cap),
                    line_join(stroke.line_join),
                    fmt(stroke.miter_limit),
                    fmt(stroke.dash_offset)
                )?;
                if stroke.dash_array.is_empty() {
                    self.output.push_str("none")?;
                }
                for (index, value) in stroke.dash_array.iter().enumerate() {
                    if index != 0 {
                        self.output.push(',')?;
                    }
                    write!(self.output, "{}", fmt(*value))?;
                }
                self.output.push(';')?;
            } else {
                self.output.push_str("stroke:none;")?;
            }
            if class == "cynefinArrowHead" {
                if let Some(paint @ Paint::Solid { color, .. }) = style.marker_fill {
                    write!(
                        self.output,
                        "fill:{};fill-opacity:{};",
                        color_css(*color),
                        fmt(paint_opacity(paint))
                    )?;
                } else {
                    self.output.push_str("fill:none;")?;
                }
            }
            self.output.push('}')?;
        }
        // Mermaid inserts an empty structural group between its stylesheet and content.
        self.output.push_str("</style><g/>")
    }

    fn cynefin_compact_path_class(&self, path_id: &ResourceId, style: &PathStyle) -> Option<&str> {
        let SvgStructureBody::Cynefin(body) = self.svg_body else {
            return None;
        };
        let class = body.path_classes.get(path_id.as_str())?;
        let expected = self.cynefin_path_styles.get(class)?.as_ref()?;
        (style.fill_rule == FillRule::NonZero
            && expected.stroke == style.stroke.as_ref()
            && (class != "cynefinArrowHead" || expected.marker_fill == style.fill.as_ref()))
        .then_some(class.as_str())
    }

    pub(super) fn emit_compact_cynefin_path(
        &mut self,
        path_id: &ResourceId,
        style: &PathStyle,
    ) -> Result<bool> {
        let Some(class) = self
            .cynefin_compact_path_class(path_id, style)
            .map(str::to_owned)
        else {
            return Ok(false);
        };
        let path = self.path_resource(path_id)?;
        if matches!(class.as_str(), "cynefinItem" | "cynefinItemOverflow") {
            let Some((bounds, radius)) = rounded_rectangle_from_path(path) else {
                return Ok(false);
            };
            write!(
                self.output,
                "<rect class=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" ry=\"{}\"",
                class,
                fmt(bounds.x),
                fmt(bounds.y),
                fmt(bounds.width),
                fmt(bounds.height),
                fmt(radius),
                fmt(radius)
            )?;
        } else if class == "cynefinDomain" {
            let Some(bounds) = rectangle_from_path(path) else {
                return Ok(false);
            };
            write!(
                self.output,
                "<rect class=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                class,
                fmt(bounds.x),
                fmt(bounds.y),
                fmt(bounds.width),
                fmt(bounds.height)
            )?;
        } else {
            write!(
                self.output,
                "<path class=\"{}\" d=\"{}\"",
                class,
                path_d(&path.segments)
            )?;
        }
        // With one solid fill and no stroke, element opacity and fill opacity have the same
        // composition. Keep the source attribute without rounding alpha back to a CSS default.
        if class == "cynefinDomain"
            && style.stroke.is_none()
            && self.state.blend_mode == BlendMode::Normal
            && let Some(ref paint @ Paint::Solid { color, .. }) = style.fill
        {
            write!(
                self.output,
                " fill=\"{}\" fill-opacity=\"{}\" stroke=\"none\"",
                color_css(color),
                fmt(paint_opacity(paint) * self.state.opacity)
            )?;
            self.write_transform_and_blend()?;
            self.output.push_str("/>")?;
            return Ok(true);
        }
        if class != "cynefinArrowHead" {
            if let Some(paint) = &style.fill {
                self.write_paint("fill", paint)?;
            } else {
                self.output.push_str(" fill=\"none\"")?;
            }
        }
        if class == "cynefinDomain" && style.stroke.is_none() {
            self.output.push_str(" stroke=\"none\"")?;
        }
        self.write_state_attrs()?;
        self.output.push_str("/>")?;
        Ok(true)
    }

    pub(super) fn emit_compact_cynefin_text(
        &mut self,
        run: &TextRun,
        semantic_id: Option<&str>,
    ) -> Result<bool> {
        let SvgStructureBody::Cynefin(body) = self.svg_body else {
            return Ok(false);
        };
        let Some(class) = semantic_id.and_then(|id| body.text_classes.get(id)) else {
            return Ok(false);
        };
        if self.cynefin_text_styles.get(class).copied().flatten() != Some(&run.style)
            || run.direction != TextDirection::Auto
            || run.language.is_some()
            || run.text.contains(['\n', '\r'])
            || self.state.transform != Transform::IDENTITY
            || self.state.opacity != 1.0
            || self.state.blend_mode != BlendMode::Normal
        {
            return Ok(false);
        }
        let baseline = match run.baseline {
            TextBaseline::Middle => "middle",
            TextBaseline::Alphabetic => "auto",
            other => text_baseline(other),
        };
        write!(
            self.output,
            "<text class=\"{}\" x=\"{}\" y=\"{}\" text-anchor=\"{}\" dominant-baseline=\"{}\">",
            escaped_attr(class),
            fmt(run.origin.x),
            fmt(run.origin.y),
            text_anchor(run.anchor),
            baseline
        )?;
        output::escape_xml(&mut self.output, run.text.as_str())?;
        self.output.push_str("</text>")?;
        Ok(true)
    }

    pub(super) fn emit_cynefin_marked_edge(&mut self, index: usize) -> Result<bool> {
        // Element opacity/blending composites the edge and its SVG marker together, whereas the
        // public commands paint them separately. Keep ordinary paths when that distinction matters.
        if self.state.opacity != 1.0 || self.state.blend_mode != BlendMode::Normal {
            return Ok(false);
        }
        let Some(
            [
                DrawingCommand::DrawPath {
                    path: line_id,
                    style: line_style,
                },
                DrawingCommand::Save,
                DrawingCommand::ConcatTransform { transform },
                DrawingCommand::DrawPath {
                    path: arrow_id,
                    style: arrow_style,
                },
                DrawingCommand::Restore,
            ],
        ) = self.document.commands.get(index..index.saturating_add(5))
        else {
            return Ok(false);
        };
        let Some(prefix) = line_id.as_str().strip_suffix(".line") else {
            return Ok(false);
        };
        if !prefix.starts_with("cynefin.transition.")
            || arrow_id.as_str().strip_suffix(".arrowhead") != Some(prefix)
        {
            return Ok(false);
        }
        let SvgStructureBody::Cynefin(body) = self.svg_body else {
            return Ok(false);
        };
        // ID-shaped additions are not registered source elements. Giving them the source
        // classes would let another transition's shared CSS override their public paint.
        if body.path_classes.get(line_id.as_str()).map(String::as_str) != Some("cynefinArrowLine")
            || body.path_classes.get(arrow_id.as_str()).map(String::as_str)
                != Some("cynefinArrowHead")
        {
            return Ok(false);
        }
        let Some(stroke) = line_style.stroke.as_ref() else {
            return Ok(false);
        };
        let [
            PathSegment::MoveTo { to: start },
            PathSegment::QuadTo { control, to: end },
        ] = self.path_resource(line_id)?.segments.as_slice()
        else {
            return Ok(false);
        };
        if marker_transform(*start, *control, *end, stroke.width) != Some(*transform) {
            // An edited command stream is still rendered faithfully as ordinary paths. Only
            // the exact source marker placement can be projected into marker-end syntax.
            return Ok(false);
        }
        let Some(paint @ Paint::Solid { color, .. }) = arrow_style.fill.as_ref() else {
            return Ok(false);
        };
        if arrow_style.stroke.is_some() {
            return Ok(false);
        }
        let arrow = self.path_resource(arrow_id)?;
        if !arrow.segments.iter().all(|segment| match segment {
            PathSegment::MoveTo { to } | PathSegment::LineTo { to } => {
                (0.0..=VIEW_BOX_SIZE).contains(&to.x) && (0.0..=VIEW_BOX_SIZE).contains(&to.y)
            }
            PathSegment::Close => true,
            _ => false,
        }) {
            // Do not clip arbitrary edited paths to the source marker's viewBox.
            return Ok(false);
        }
        let marker_body = if self
            .cynefin_compact_path_class(arrow_id, arrow_style)
            .is_some()
        {
            format!(
                "<path d=\"{}\" class=\"cynefinArrowHead\"/>",
                path_d(&arrow.segments).with_lowercase_close()
            )
        } else {
            format!(
                "<path d=\"{}\" class=\"cynefinArrowHead\" fill=\"{}\" fill-opacity=\"{}\" fill-rule=\"{}\" stroke=\"none\"/>",
                escaped_attr(path_d(&arrow.segments).with_lowercase_close()),
                color_css(*color),
                fmt(paint_opacity(paint)),
                fill_rule_name(arrow_style.fill_rule),
            )
        };
        let count = self.cynefin_marker_definitions.len();
        let diagram_id = &self.diagram_id;
        let marker_id = self
            .cynefin_marker_definitions
            .entry(marker_body)
            .or_insert_with(|| {
                if count == 0 {
                    format!("cynefin-arrow-{diagram_id}")
                } else {
                    format!("cynefin-arrow-{diagram_id}-{count}")
                }
            })
            .clone();
        let path_data = path_d(&self.path_resource(line_id)?.segments);
        write!(
            self.output,
            "<path class=\"cynefinArrowLine\" d=\"{}\" marker-end=\"url(#{})\"",
            escaped_attr(&path_data),
            escaped_attr(&marker_id)
        )?;
        if self
            .cynefin_compact_path_class(line_id, line_style)
            .is_some()
        {
            if let Some(paint) = &line_style.fill {
                self.write_paint("fill", paint)?;
            } else {
                self.output.push_str(" fill=\"none\"")?;
            }
        } else {
            self.write_path_style(line_style)?;
        }
        self.write_state_attrs()?;
        self.output.push_str("/>")?;
        Ok(true)
    }

    pub(super) fn write_cynefin_marker_defs(&mut self) -> Result<()> {
        if self.cynefin_marker_definitions.is_empty() {
            return Ok(());
        }
        self.output.push_str("<defs>")?;
        for (body, id) in &self.cynefin_marker_definitions {
            self.session.checkpoint(OperationPhase::Emit)?;
            write!(
                self.output,
                "<marker id=\"{}\" viewBox=\"0 0 {} {}\" refX=\"{}\" refY=\"{}\" markerWidth=\"{}\" markerHeight=\"{}\" orient=\"auto-start-reverse\">{body}</marker>",
                escaped_attr(id),
                fmt(VIEW_BOX_SIZE),
                fmt(VIEW_BOX_SIZE),
                fmt(REF_X),
                fmt(REF_Y),
                fmt(MARKER_SIZE),
                fmt(MARKER_SIZE)
            )?;
        }
        self.output.push_str("</defs>")?;
        self.session.checkpoint(OperationPhase::Emit)?;
        Ok(())
    }

    pub(super) fn begin_cynefin_semantic_group(
        &mut self,
        semantic_id: &str,
        role: SemanticRole,
    ) -> Result<()> {
        // Labels and edges have public semantic ownership but no corresponding SVG wrapper.
        // Containers and item nodes retain Mermaid's exact group classes and translations.
        let emitted = matches!(role, SemanticRole::Group | SemanticRole::Node);
        let projected_transform = if emitted {
            self.state.transform
        } else {
            Transform::IDENTITY
        };
        if emitted {
            self.output.push_str("<g")?;
            if role == SemanticRole::Group
                && let Some(class) = self.semantic_extra_class(semantic_id).map(str::to_owned)
                && !class.is_empty()
            {
                write!(self.output, " class=\"{}\"", escaped_attr(&class))?;
            }
            if projected_transform != Transform::IDENTITY {
                let t = projected_transform;
                if t.a == 1.0 && t.b == 0.0 && t.c == 0.0 && t.d == 1.0 {
                    write!(
                        self.output,
                        " transform=\"translate({}, {})\"",
                        fmt(t.e),
                        fmt(t.f)
                    )?;
                } else {
                    write!(self.output, " transform=\"matrix({})\"", matrix_attr(t))?;
                }
                self.state.transform = Transform::IDENTITY;
            }
            self.output.push('>')?;
        }
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted,
            semantic_id: semantic_id.to_string(),
            projected_transform,
        });
        Ok(())
    }
}
