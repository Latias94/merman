//! Venn's source DOM projected from public geometry, paint, and text commands.

use super::*;
use merman_display_list::StrokeStyle;

pub(super) struct TextStyles<'a> {
    pub(super) title: Option<&'a merman_display_list::TextStyle>,
    area_font_sizes: BTreeMap<&'a str, Option<f64>>,
}

impl<'a> TextStyles<'a> {
    pub(super) fn new(
        document: &'a DrawingListDocument,
        body: &crate::drawing_list::VennSvgBody,
        session: &RenderSession,
    ) -> Result<Self> {
        let mut area_font_sizes = BTreeMap::new();
        let mut depth = 0;
        let mut area = None;
        for command in &document.commands {
            session.checkpoint(OperationPhase::Emit)?;
            match command {
                DrawingCommand::BeginSemanticGroup { semantic_id } => {
                    depth += 1;
                    if body.semantic_classes.get(semantic_id).map(String::as_str)
                        == Some("venn-text-area")
                    {
                        area = Some((semantic_id.as_str(), depth));
                    }
                }
                DrawingCommand::EndSemanticGroup => {
                    if area.is_some_and(|(_, start)| start == depth) {
                        area = None;
                    }
                    depth -= 1;
                }
                DrawingCommand::DrawText { run } => {
                    if let Some((id, _)) = area {
                        area_font_sizes
                            .entry(id)
                            .and_modify(|size| {
                                if *size != Some(run.style.font_size) {
                                    *size = None;
                                }
                            })
                            .or_insert(Some(run.style.font_size));
                    }
                }
                _ => {}
            }
        }
        Ok(Self {
            title: shared_title_style(document, session)?,
            area_font_sizes,
        })
    }
}

fn shared_title_style<'a>(
    document: &'a DrawingListDocument,
    session: &RenderSession,
) -> Result<Option<&'a merman_display_list::TextStyle>> {
    let mut shared = None;
    for commands in document.commands.windows(3) {
        session.checkpoint(OperationPhase::Emit)?;
        let [
            DrawingCommand::BeginSemanticGroup { semantic_id },
            DrawingCommand::DrawText { run },
            DrawingCommand::EndSemanticGroup,
        ] = commands
        else {
            continue;
        };
        if semantic_id != "venn.title" {
            continue;
        }
        if run.style.font.resource.is_some()
            || run.style.stroke.is_some()
            || !matches!(run.style.fill, Paint::Solid { .. })
            || shared.is_some_and(|style| style != &run.style)
        {
            return Ok(None);
        }
        shared = Some(&run.style);
    }
    Ok(shared)
}

/// Preserve the upstream M/m/a/a spelling only when relative coordinates round-trip exactly.
struct AreaPathData<'a>(&'a [PathSegment]);

impl std::fmt::Display for AreaPathData<'_> {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [
            PathSegment::MoveTo { to: origin },
            PathSegment::MoveTo { to: start },
            first @ PathSegment::ArcTo { to: middle, .. },
            second @ PathSegment::ArcTo { to: end, .. },
        ] = self.0
        else {
            return write!(output, "{}", path_d(self.0));
        };
        let pairs = [(origin, start), (start, middle), (middle, end)];
        let offsets = pairs.map(|(from, to)| Point::new(to.x - from.x, to.y - from.y));
        if !pairs.iter().zip(offsets).all(|((from, to), delta)| {
            delta.x.is_finite()
                && delta.y.is_finite()
                && from.x + delta.x == to.x
                && from.y + delta.y == to.y
        }) {
            return write!(output, "{}", path_d(self.0));
        }
        write!(
            output,
            "M {} {} m {} {}",
            origin.x, origin.y, offsets[0].x, offsets[0].y
        )?;
        for (segment, delta) in [first, second].into_iter().zip(&offsets[1..]) {
            if let PathSegment::ArcTo {
                radius_x,
                radius_y,
                x_axis_rotation_degrees,
                large_arc,
                sweep_clockwise,
                ..
            } = segment
            {
                write!(
                    output,
                    " a {} {} {} {} {} {} {}",
                    radius_x,
                    radius_y,
                    x_axis_rotation_degrees,
                    u8::from(*large_arc),
                    u8::from(*sweep_clockwise),
                    delta.x,
                    delta.y
                )?;
            }
        }
        Ok(())
    }
}

/// Recognize one complete independent paint scope, without changing encoder state.
fn paint_scope(
    commands: &[DrawingCommand],
    index: usize,
) -> Option<(&ResourceId, &PathStyle, f64, usize)> {
    match commands.get(index)? {
        DrawingCommand::DrawPath { path, style } => Some((path, style, 1.0, 1)),
        DrawingCommand::Save => {
            let [
                DrawingCommand::Save,
                DrawingCommand::SetOpacity { opacity },
                DrawingCommand::DrawPath { path, style },
                DrawingCommand::Restore,
            ] = commands.get(index..index.checked_add(4)?)?
            else {
                return None;
            };
            Some((path, style, *opacity, 4))
        }
        _ => None,
    }
}

fn compact_stroke(stroke: &StrokeStyle) -> bool {
    matches!(stroke.paint, Paint::Solid { .. })
        && stroke.dash_array.is_empty()
        && stroke.dash_offset == 0.0
        && stroke.line_cap == LineCap::Butt
        && stroke.line_join == LineJoin::Miter
        && stroke.miter_limit == 4.0
}

fn rough_scope<'a>(
    commands: &'a [DrawingCommand],
    index: usize,
    area: &str,
) -> Option<(&'a ResourceId, &'a StrokeStyle, f64, usize)> {
    let (path, style, opacity, count) = paint_scope(commands, index)?;
    if style.fill.is_some()
        || style.fill_rule != FillRule::NonZero
        || ![".rough-fill", ".rough-outline"]
            .iter()
            .any(|suffix| path.as_str().strip_suffix(suffix) == Some(area))
    {
        return None;
    }
    Some((
        path,
        style
            .stroke
            .as_ref()
            .filter(|stroke| compact_stroke(stroke))?,
        opacity,
        count,
    ))
}

impl DocumentSvgEncoder<'_> {
    pub(super) fn write_venn_style(&mut self) -> Result<()> {
        self.output.push_str("<style>")?;
        if let Some(style) = self
            .venn_text_styles
            .as_ref()
            .and_then(|styles| styles.title)
        {
            let font = self.font_families(&style.font)?;
            write!(
                self.output,
                "#{} .venn-title{{font-size:{}px;font-family:{};font-weight:{};font-style:{};}}",
                self.diagram_id,
                fmt(style.font_size),
                font,
                style.font.weight,
                font_style(style.font.style)
            )?;
        }
        self.output.push_str("</style><g/>")
    }

    /// Preserve Rough.js's structural wrapper, using only the public paint scopes.
    pub(super) fn emit_venn_rough_group(&mut self, index: usize) -> Result<Option<usize>> {
        let SvgStructureBody::Venn(body) = self.svg_body else {
            return Ok(None);
        };
        if self.state.opacity != 1.0 || self.state.blend_mode != BlendMode::Normal {
            return Ok(None);
        }
        let Some(area) = self
            .current_semantic_id()
            .filter(|id| body.semantic_data_sets.contains_key(*id))
        else {
            return Ok(None);
        };
        let Some(first) = rough_scope(&self.document.commands, index, area) else {
            return Ok(None);
        };
        let second = if first.0.as_str().ends_with(".rough-fill") {
            rough_scope(&self.document.commands, index + first.3, area)
                .filter(|scope| scope.0.as_str().ends_with(".rough-outline"))
        } else {
            None
        };
        self.output.push_str("<g>")?;
        let mut consumed = 0;
        for (path, stroke, opacity, count) in [Some(first), second].into_iter().flatten() {
            self.write_venn_rough_path(path, stroke, opacity)?;
            consumed += count;
        }
        self.output.push_str("</g>")?;
        Ok(Some(consumed))
    }

    fn write_venn_rough_path(
        &mut self,
        id: &ResourceId,
        stroke: &StrokeStyle,
        opacity: f64,
    ) -> Result<()> {
        let path = self.path_resource(id)?;
        self.output.push_str("<path d=\"")?;
        for (index, segment) in path.segments.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            if index != 0 {
                self.output.push(' ')?;
            }
            // Keep Rough.js's round-trippable coordinate spelling without rebuilding OpSets.
            match segment {
                PathSegment::MoveTo { to } => write!(self.output, "M{} {}", to.x, to.y)?,
                PathSegment::LineTo { to } => write!(self.output, "L{} {}", to.x, to.y)?,
                PathSegment::CubicTo {
                    control1,
                    control2,
                    to,
                } => write!(
                    self.output,
                    "C{} {}, {} {}, {} {}",
                    control1.x, control1.y, control2.x, control2.y, to.x, to.y
                )?,
                // An edited public resource may contain other valid segment types.
                other => write!(self.output, "{}", path_d(std::slice::from_ref(other)))?,
            }
        }
        let Paint::Solid { color } = stroke.paint else {
            return Err(invalid("compact Venn rough stroke must be solid"));
        };
        write!(
            self.output,
            "\" stroke=\"rgba({}, {}, {}, {})\" stroke-width=\"{}\" fill=\"none\"",
            color.red,
            color.green,
            color.blue,
            opacity * f64::from(color.alpha) / 255.0,
            fmt(stroke.width)
        )?;
        self.write_transform_and_blend()?;
        self.output.push_str("/>")
    }

    pub(super) fn begin_venn_semantic_group(
        &mut self,
        id: &str,
        semantic: &SemanticAnnotation,
    ) -> Result<bool> {
        let SvgStructureBody::Venn(body) = self.svg_body else {
            return Ok(false);
        };
        let area = body.semantic_data_sets.contains_key(id);
        let text_group = body
            .semantic_classes
            .get(id)
            .is_some_and(|class| matches!(class.as_str(), "venn-text-nodes" | "venn-text-area"));
        if (text_group || id == "venn.content")
            && (semantic.title.is_some()
                || semantic.description.is_some()
                || semantic.role != SemanticRole::Group)
        {
            // Edited public annotations need the generic canonical wrapper, rather than a
            // source structural shell which has no annotation payload of its own.
            return Ok(false);
        }
        if (area && semantic.role != SemanticRole::Node)
            || (id == "venn.document" && semantic.role != SemanticRole::Document)
        {
            return Ok(false);
        }
        if id == "venn.title" {
            let native_name = matches!(
                self.document.commands.get(self.command_index + 1..),
                Some([
                    DrawingCommand::DrawText { run },
                    DrawingCommand::EndSemanticGroup,
                    ..
                ]) if semantic.title.as_deref().is_none_or(|name| name == run.text)
            );
            if semantic.role != SemanticRole::Label
                || semantic.description.is_some()
                || !native_name
            {
                return Ok(false);
            }
        }
        if semantic.link.is_some()
            || !(area
                || text_group
                || matches!(id, "venn.document" | "venn.content" | "venn.title"))
        {
            return Ok(false);
        }
        let emitted = area || text_group || id == "venn.content";
        let projected_transform = if emitted {
            self.state.transform
        } else {
            Transform::IDENTITY
        };
        if emitted {
            self.output.push_str("<g")?;
            if let Some(class) = body.semantic_classes.get(id) {
                write!(self.output, " class=\"{}\"", escaped_attr(class))?;
            }
            if let Some(size) = self
                .venn_text_styles
                .as_ref()
                .and_then(|styles| styles.area_font_sizes.get(id))
                .copied()
                .flatten()
            {
                write!(self.output, " font-size=\"{}px\"", fmt(size))?;
            }
            if let Some(sets) = body.semantic_data_sets.get(id) {
                write!(self.output, " data-venn-sets=\"{}\"", escaped_attr(sets))?;
            }
            if area && (semantic.title.is_some() || semantic.description.is_some()) {
                // The public area annotation is authoritative, including independently edited
                // descriptions. Keep it on the existing group without changing paint order.
                self.output.push_str(" role=\"group\"")?;
                if let Some(title) = &semantic.title {
                    write!(self.output, " aria-label=\"{}\"", escaped_attr(title))?;
                }
                if let Some(description) = &semantic.description {
                    write!(
                        self.output,
                        " aria-description=\"{}\"",
                        escaped_attr(description)
                    )?;
                }
            }
            if projected_transform != Transform::IDENTITY || id == "venn.content" {
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
            if (area || text_group) && !self.debug_visibility(semantic.role) {
                self.output.push_str(" display=\"none\"")?;
            }
            self.output.push('>')?;
        }
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted,
            semantic_id: id.to_owned(),
            projected_transform,
        });
        Ok(true)
    }

    /// Retains the HTML identity shell without giving the browser a second wrapping job.
    /// The browser and native postprocessor consume one embedded public text projection.
    pub(super) fn emit_venn_text_node(&mut self, index: usize) -> Result<Option<usize>> {
        let SvgStructureBody::Venn(body) = self.svg_body else {
            return Ok(None);
        };
        if self.state.transform != Transform::IDENTITY
            || self.state.opacity != 1.0
            || self.state.blend_mode != BlendMode::Normal
        {
            return Ok(None);
        }
        let Some(DrawingCommand::BeginSemanticGroup { semantic_id }) =
            self.document.commands.get(index)
        else {
            return Ok(None);
        };
        if body.semantic_classes.get(semantic_id).map(String::as_str) != Some("venn-text-node-fo")
            || self
                .semantics
                .get(semantic_id)
                .is_none_or(|semantic| semantic.link.is_some())
        {
            return Ok(None);
        }
        let Some(DrawingCommand::DrawPath { path, style }) = self.document.commands.get(index + 1)
        else {
            return Ok(None);
        };
        if path.as_str().strip_suffix(".container") != Some(semantic_id.as_str())
            || style.fill.is_some()
            || style.stroke.is_some()
        {
            return Ok(None);
        }
        let Some(bounds) = rectangle_from_path(self.path_resource(path)?) else {
            return Ok(None);
        };
        let first = index + 2;
        let mut end = first;
        while let Some(DrawingCommand::DrawText { run }) = self.document.commands.get(end) {
            self.session.checkpoint(OperationPhase::Emit)?;
            if !matches!(run.obligation, TextObligation::HostText { .. })
                || run.text.contains(['\n', '\r'])
            {
                return Ok(None);
            }
            end += 1;
        }
        if !matches!(
            self.document.commands.get(end),
            Some(DrawingCommand::EndSemanticGroup)
        ) {
            return Ok(None);
        }
        let semantic = self
            .semantics
            .get(semantic_id)
            .copied()
            .ok_or_else(|| invalid("Venn text node has no semantic annotation"))?;
        let svg_id = self.semantic_svg_id(semantic_id)?;
        write!(
            self.output,
            "<foreignObject class=\"venn-text-node-fo\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" overflow=\"visible\"><span xmlns=\"http://www.w3.org/1999/xhtml\" class=\"venn-text-node\" style=\"display: block; position: relative; width: 100%; height: 100%;\">",
            fmt(bounds.x),
            fmt(bounds.y),
            fmt(bounds.width),
            fmt(bounds.height)
        )?;
        // Cancel only the browser container translation. The marked inner group retains the
        // public parent-space coordinates, so native output can promote it without measurement.
        write!(
            self.output,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" overflow=\"visible\" style=\"position: absolute; left: 0; top: 0; overflow: visible;\"><g transform=\"translate({}, {})\">",
            fmt(bounds.width),
            fmt(bounds.height),
            fmt(-bounds.x),
            fmt(-bounds.y)
        )?;
        write!(
            self.output,
            "<g data-merman-native-text=\"v1\" id=\"{}\" class=\"merman-semantic {}\" role=\"group\"",
            escaped_attr(&svg_id),
            semantic_role_class(semantic.role),
        )?;
        write_semantic_metadata(&mut self.output, self.debug, semantic_id)?;
        if !self.debug_visibility(semantic.role) {
            self.output.push_str(" display=\"none\"")?;
        }
        if let Some(title) = semantic.title.as_deref() {
            write!(self.output, " aria-label=\"{}\"", escaped_attr(title))?;
        }
        if let Some(description) = semantic.description.as_deref() {
            write!(
                self.output,
                " aria-description=\"{}\"",
                escaped_attr(description)
            )?;
        }
        self.output.push('>')?;
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted: true,
            semantic_id: semantic_id.clone(),
            projected_transform: Transform::IDENTITY,
        });
        for command in &self.document.commands[first..end] {
            self.session.checkpoint(OperationPhase::Emit)?;
            if let DrawingCommand::DrawText { run } = command {
                self.emit_text(run)?;
            }
        }
        self.end_semantic_group()?;
        self.output.push_str("</g></svg></span></foreignObject>")?;
        Ok(Some(end - index + 1))
    }

    fn is_venn_area_path(&self, path: &ResourceId) -> bool {
        let SvgStructureBody::Venn(body) = self.svg_body else {
            return false;
        };
        path.as_str().strip_suffix(".shape").is_some_and(|id| {
            self.current_semantic_id() == Some(id) && body.semantic_data_sets.contains_key(id)
        })
    }

    pub(super) fn emit_venn_fill_and_stroke(&mut self, index: usize) -> Result<Option<usize>> {
        if !matches!(self.svg_body, SvgStructureBody::Venn(_))
            || self.state.opacity != 1.0
            || self.state.blend_mode != BlendMode::Normal
        {
            return Ok(None);
        }
        let Some((path, fill, fill_opacity, fill_count)) =
            paint_scope(&self.document.commands, index)
        else {
            return Ok(None);
        };
        if !self.is_venn_area_path(path)
            || fill.stroke.is_some()
            || fill.fill_rule != FillRule::NonZero
        {
            return Ok(None);
        }
        let Some(Paint::Solid { color }) = fill.fill.as_ref() else {
            return Ok(None);
        };
        let Some((stroke_path, stroke, stroke_opacity, stroke_count)) =
            paint_scope(&self.document.commands, index + fill_count)
        else {
            return Ok(None);
        };
        if path != stroke_path || stroke.fill.is_some() || stroke.fill_rule != FillRule::NonZero {
            return Ok(None);
        }
        let Some(stroke) = stroke
            .stroke
            .as_ref()
            .filter(|stroke| compact_stroke(stroke))
        else {
            return Ok(None);
        };
        self.write_venn_path(
            path,
            Some((*color, fill_opacity)),
            Some((stroke, stroke_opacity)),
        )?;
        Ok(Some(fill_count + stroke_count))
    }

    pub(super) fn emit_venn_path(&mut self, path: &ResourceId, style: &PathStyle) -> Result<bool> {
        if !self.is_venn_area_path(path)
            || style.fill_rule != FillRule::NonZero
            || self.state.blend_mode != BlendMode::Normal
            || (style.fill.is_some() && style.stroke.is_some() && self.state.opacity != 1.0)
        {
            return Ok(false);
        }
        let fill = match style.fill.as_ref() {
            Some(Paint::Solid { color }) => Some((*color, self.state.opacity)),
            None => None,
            _ => return Ok(false),
        };
        if style
            .stroke
            .as_ref()
            .is_some_and(|stroke| !compact_stroke(stroke))
        {
            return Ok(false);
        }
        self.write_venn_path(
            path,
            fill,
            style
                .stroke
                .as_ref()
                .map(|stroke| (stroke, self.state.opacity)),
        )?;
        Ok(true)
    }

    fn write_venn_path(
        &mut self,
        id: &ResourceId,
        fill: Option<(Color, f64)>,
        stroke: Option<(&StrokeStyle, f64)>,
    ) -> Result<()> {
        let path = self.path_resource(id)?;
        write!(
            self.output,
            "<path d=\"{}\" style=\"",
            AreaPathData(&path.segments)
        )?;
        if let Some((color, opacity)) = fill {
            write!(
                self.output,
                "fill-opacity: {}; fill: {};",
                fmt(f64::from(color.alpha) / 255.0 * opacity),
                color_css(color)
            )?;
        } else {
            self.output
                .push_str("fill-opacity: 0; fill: transparent;")?;
        }
        if let Some((stroke, opacity)) = stroke {
            let Paint::Solid { color } = stroke.paint else {
                return Err(invalid("compact Venn stroke must be solid"));
            };
            write!(
                self.output,
                " stroke: {}; stroke-width: {}; stroke-opacity: {};",
                color_css(color),
                fmt(stroke.width),
                fmt(f64::from(color.alpha) / 255.0 * opacity)
            )?;
        }
        self.output.push('"')?;
        self.write_transform_and_blend()?;
        self.output.push_str("/>")
    }

    pub(super) fn emit_compact_venn_text(
        &mut self,
        run: &TextRun,
        semantic_id: Option<&str>,
    ) -> Result<bool> {
        let SvgStructureBody::Venn(body) = self.svg_body else {
            return Ok(false);
        };
        let Some(class) = semantic_id.and_then(|id| body.text_classes.get(id)) else {
            return Ok(false);
        };
        let title = class == "venn-title";
        let shared_title = title
            && self
                .venn_text_styles
                .as_ref()
                .and_then(|styles| styles.title)
                == Some(&run.style);
        if (!title && class != "label")
            || run.direction != TextDirection::Auto
            || run.language.is_some()
            || run.text.contains(['\n', '\r'])
            || run.style.stroke.is_some()
            || run.style.letter_spacing != 0.0
            || self.state.blend_mode != BlendMode::Normal
            || run.anchor != TextAnchor::Middle
            || run.baseline
                != if title {
                    TextBaseline::Middle
                } else {
                    TextBaseline::Alphabetic
                }
        {
            return Ok(false);
        }
        let Paint::Solid { color } = run.style.fill else {
            return Ok(false);
        };
        let font = self.font_families(&run.style.font)?;
        let y = if title {
            run.origin.y
        } else {
            run.origin.y - 0.35 * run.style.font_size
        };
        write!(
            self.output,
            "<text class=\"{}\" text-anchor=\"middle\"",
            escaped_attr(class)
        )?;
        if title {
            self.output.push_str(" dominant-baseline=\"middle\"")?;
            // The source scales this presentation attribute, but the author rule owns the
            // visible size. Both values derive from public style/viewport, never theme config.
            let size = if shared_title {
                run.style.font_size * (self.document.viewport.bounds.width / 1600.0)
            } else {
                run.style.font_size
            };
            write!(self.output, " font-size=\"{}px\"", fmt(size))?;
        } else {
            self.output.push_str(" dy=\".35em\"")?;
        }
        // Venn's root uses the document viewport directly, without the generic padding option.
        if title
            && self.document.viewport.bounds.x == 0.0
            && run.origin.x == self.document.viewport.bounds.width / 2.0
        {
            self.output.push_str(" x=\"50%\"")?;
        } else {
            write!(self.output, " x=\"{}\"", fmt(run.origin.x))?;
        }
        write!(
            self.output,
            " y=\"{}\" style=\"fill: {}; fill-opacity: {};",
            fmt(y),
            color_css(color),
            fmt(f64::from(color.alpha) / 255.0)
        )?;
        if !shared_title {
            write!(
                self.output,
                " font-size: {}px; font-family: {}; font-weight: {}; font-style: {};",
                fmt(run.style.font_size),
                escaped_attr(&font),
                run.style.font.weight,
                font_style(run.style.font.style)
            )?;
        }
        self.output.push('"')?;
        self.write_state_attrs()?;
        self.output.push('>')?;
        if !title {
            write!(
                self.output,
                "<tspan x=\"{}\" y=\"{}\" dy=\"0.35em\">",
                fmt(run.origin.x),
                fmt(y)
            )?;
        }
        output::escape_xml(&mut self.output, &run.text)?;
        if !title {
            self.output.push_str("</tspan>")?;
        }
        self.output.push_str("</text>")?;
        Ok(true)
    }
}
