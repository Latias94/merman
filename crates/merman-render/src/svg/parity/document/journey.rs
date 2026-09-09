//! Journey DOM grouping projected from public semantic scopes and positioned text.

use super::*;

impl DocumentSvgEncoder<'_> {
    pub(super) fn write_journey_background_presentation(
        &mut self,
        path: &ResourceId,
        style: &PathStyle,
    ) -> Result<bool> {
        if !matches!(self.svg_body, SvgStructureBody::Journey(_))
            || !path.as_str().ends_with(".background")
            || !(path.as_str().starts_with("journey.task.")
                || path.as_str().starts_with("journey.section."))
        {
            return Ok(false);
        }
        // Source rectangles expose fill/stroke, with the remaining presentation inherited
        // from CSS. Both representations here come from the resolved public style, never
        // the old layout fill that the source stylesheet overwrote.
        if let Some(fill) = &style.fill {
            self.write_paint("fill", fill)?;
        }
        if let Some(stroke) = &style.stroke {
            self.write_paint("stroke", &stroke.paint)?;
        }
        self.write_journey_primitive_css(style)?;
        Ok(true)
    }

    /// Circle geometry is already verified by the primitive emitter. Keep source identity
    /// attributes while the complete public style owns the effective CSS presentation.
    pub(super) fn write_journey_circle_presentation(
        &mut self,
        path: &ResourceId,
        style: &PathStyle,
    ) -> Result<bool> {
        if !matches!(self.svg_body, SvgStructureBody::Journey(_)) {
            return Ok(false);
        }
        let id = path.as_str();
        let face = id.starts_with("journey.task.") && id.ends_with(".face");
        let eye = id.starts_with("journey.task.")
            && (id.ends_with(".left_eye") || id.ends_with(".right_eye"));
        let actor =
            id.ends_with(".circle") && (id.starts_with("journey.actor.") || id.contains(".actor."));
        if !face && !eye && !actor {
            return Ok(false);
        }
        if face {
            self.output.push_str(" overflow=\"visible\"")?;
        } else {
            if let Some(fill) = &style.fill {
                self.write_paint("fill", fill)?;
            }
            if let Some(stroke) = &style.stroke {
                self.write_paint("stroke", &stroke.paint)?;
            }
        }
        if (face || eye)
            && let Some(stroke) = &style.stroke
        {
            write!(self.output, " stroke-width=\"{}\"", fmt(stroke.width))?;
        }
        self.write_journey_primitive_css(style)?;
        Ok(true)
    }

    pub(super) fn write_journey_line_presentation(
        &mut self,
        path: &ResourceId,
        style: &PathStyle,
    ) -> Result<bool> {
        if !matches!(self.svg_body, SvgStructureBody::Journey(_)) {
            return Ok(false);
        }
        let task = path.as_str().starts_with("journey.task.") && path.as_str().ends_with(".line");
        let mouth =
            path.as_str().starts_with("journey.task.") && path.as_str().ends_with(".face.mouth");
        if !task && !mouth && path.as_str() != "journey.activity.line" {
            return Ok(false);
        }
        if let Some(stroke) = &style.stroke {
            self.write_paint("stroke", &stroke.paint)?;
            write!(
                self.output,
                " stroke-width=\"{}{}\"",
                fmt(stroke.width),
                if task || mouth { "px" } else { "" }
            )?;
            if !stroke.dash_array.is_empty() {
                self.output.push_str(" stroke-dasharray=\"")?;
                for (index, value) in stroke.dash_array.iter().enumerate() {
                    self.session.checkpoint(OperationPhase::Emit)?;
                    if index != 0 {
                        self.output.push(' ')?;
                    }
                    write!(self.output, "{}", fmt(*value))?;
                }
                self.output.push('"')?;
            }
        }
        self.write_journey_primitive_css(style)?;
        Ok(true)
    }

    /// Rebase the public two-arc mouth without recovering a task score or source layout.
    /// Non-default element transforms keep the ordinary absolute-path representation.
    pub(super) fn emit_journey_curved_mouth(
        &mut self,
        path_id: &ResourceId,
        style: &PathStyle,
    ) -> Result<bool> {
        if !matches!(self.svg_body, SvgStructureBody::Journey(_))
            || !path_id.as_str().starts_with("journey.task.")
            || !path_id.as_str().ends_with(".face.mouth")
            || self.state.transform != Transform::IDENTITY
            // Resource paint uses the element's user-space coordinate system. Rebasing just
            // the path would move a gradient/pattern, so retain its absolute representation.
            || matches!(style.fill, Some(Paint::Resource { .. }))
            || style.stroke.as_ref().is_some_and(|stroke| matches!(stroke.paint, Paint::Resource { .. }))
        {
            return Ok(false);
        }
        let path = self.path_resource(path_id)?;
        let [
            PathSegment::MoveTo { to: start },
            PathSegment::ArcTo { to: end, .. },
            PathSegment::LineTo { .. },
            PathSegment::ArcTo { .. },
            PathSegment::Close,
        ] = path.segments.as_slice()
        else {
            return Ok(false);
        };
        // This is a coordinate basis, not new geometry. Translating every public point by
        // -origin and applying T(origin) preserves edited radii, flags, and endpoints too.
        let origin = Point::new(start.x * 0.5 + end.x * 0.5, start.y * 0.5 + end.y * 0.5);
        let local = offset_path_segments(&path.segments, -origin.x, -origin.y);
        if !local.iter().all(|segment| match segment {
            PathSegment::MoveTo { to }
            | PathSegment::LineTo { to }
            | PathSegment::ArcTo { to, .. } => to.x.is_finite() && to.y.is_finite(),
            PathSegment::Close => true,
            _ => false,
        }) {
            return Ok(false);
        }
        write!(
            self.output,
            "<path d=\"{}\" transform=\"translate({}, {})\"",
            path_d(&local),
            fmt(origin.x),
            fmt(origin.y)
        )?;
        if let Some(class) = self.path_class(path_id) {
            write!(self.output, " class=\"{}\"", escaped_attr(class.as_ref()))?;
        }
        self.write_journey_primitive_css(style)?;
        write_resource_metadata(&mut self.output, self.debug, path_id.as_str())?;
        self.output.push_str("/>")?;
        Ok(true)
    }

    fn write_journey_primitive_css(&mut self, style: &PathStyle) -> Result<()> {
        self.output.push_str(" style=\"")?;
        self.write_journey_css_paint("fill", style.fill.as_ref())?;
        self.write_journey_css_paint("stroke", style.stroke.as_ref().map(|stroke| &stroke.paint))?;
        write!(
            self.output,
            "fill-rule:{};",
            fill_rule_name(style.fill_rule)
        )?;
        if let Some(stroke) = &style.stroke {
            write!(
                self.output,
                "stroke-width:{};stroke-linecap:{};stroke-linejoin:{};stroke-miterlimit:{};stroke-dashoffset:{};stroke-dasharray:",
                fmt(stroke.width),
                line_cap(stroke.line_cap),
                line_join(stroke.line_join),
                fmt(stroke.miter_limit),
                fmt(stroke.dash_offset)
            )?;
            if stroke.dash_array.is_empty() {
                self.output.push_str("none")?;
            } else {
                for (index, value) in stroke.dash_array.iter().enumerate() {
                    self.session.checkpoint(OperationPhase::Emit)?;
                    if index != 0 {
                        self.output.push(' ')?;
                    }
                    write!(self.output, "{}", fmt(*value))?;
                }
            }
            self.output.push(';')?;
        }
        // Own opacity and blend in this same CSS block; calling write_state_attrs as well
        // would create a second style attribute when the public blend is non-default.
        write!(
            self.output,
            "opacity:{};mix-blend-mode:{};\"",
            fmt(self.state.opacity),
            blend_css(self.state.blend_mode).unwrap_or("normal")
        )?;
        if self.state.transform != Transform::IDENTITY {
            write!(
                self.output,
                " transform=\"matrix({})\"",
                matrix_attr(self.state.transform)
            )?;
        }
        Ok(())
    }

    fn write_journey_css_paint(&mut self, property: &str, paint: Option<&Paint>) -> Result<()> {
        write!(self.output, "{property}:")?;
        let opacity = match paint {
            Some(Paint::Solid { color }) => {
                self.output.push_str(&color_css(*color))?;
                f64::from(color.alpha) / 255.0
            }
            Some(Paint::Resource { id }) => {
                let svg_id = self.svg_resource_id(id.as_str())?;
                write!(self.output, "url(#{})", escaped_attr(&svg_id))?;
                1.0
            }
            None => {
                self.output.push_str("none")?;
                1.0
            }
        };
        write!(self.output, ";{property}-opacity:{};", fmt(opacity))?;
        Ok(())
    }

    /// The source legend uses text > tspan with an absolute tspan x. Keep this identity
    /// spelling only for single-line host text; all paint/state still uses the common writer.
    pub(super) fn journey_legend_text(&self, run: &TextRun, semantic_id: Option<&str>) -> bool {
        matches!(self.svg_body, SvgStructureBody::Journey(body)
            if semantic_id.is_some_and(|id| id.starts_with("journey.actor.")
                && body.text_classes.get(id).is_some_and(|class| class == "legend")))
            && !run.text.contains(['\n', '\r'])
    }

    pub(super) fn begin_journey_semantic_group(
        &mut self,
        id: &str,
        semantic: &SemanticAnnotation,
    ) -> Result<bool> {
        let SvgStructureBody::Journey(body) = self.svg_body else {
            return Ok(false);
        };
        let (role, emitted) = if id == "journey.document" {
            (SemanticRole::Document, false)
        } else if id == "journey.activity" {
            (SemanticRole::Edge, false)
        } else if id.starts_with("journey.actor.") && body.semantic_classes.contains_key(id) {
            (SemanticRole::Label, false)
        } else if (id.starts_with("journey.section.") && body.semantic_classes.contains_key(id))
            || (id.starts_with("journey.task.") && id.ends_with(".expression"))
        {
            (SemanticRole::Group, true)
        } else if id.starts_with("journey.task.") && body.semantic_classes.contains_key(id) {
            (SemanticRole::Node, true)
        } else {
            return Ok(false);
        };
        // Edited metadata uses the ordinary semantic projection rather than disappearing when
        // its source scope happened not to have an SVG wrapper.
        if semantic.role != role
            || semantic.link.is_some()
            || !self.debug_visibility(role)
            || (id != "journey.document" && semantic.description.is_some())
            || (id != "journey.document"
                && semantic
                    .title
                    .as_deref()
                    .is_some_and(|name| !name.is_empty())
                && !self.journey_scope_has_native_name(semantic.title.as_deref().unwrap_or(""))?)
        {
            return Ok(false);
        }
        let emitted = emitted || self.debug.include_drawing_list_metadata;
        if emitted {
            self.output.push_str("<g")?;
            write_semantic_metadata(&mut self.output, self.debug, id)?;
            self.output.push('>')?;
        }
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted,
            semantic_id: id.to_owned(),
            projected_transform: Transform::IDENTITY,
        });
        Ok(true)
    }

    pub(super) fn emit_journey_actor(&mut self, index: usize) -> Result<Option<usize>> {
        if !matches!(self.svg_body, SvgStructureBody::Journey(_)) {
            return Ok(None);
        }
        let Some(
            [
                DrawingCommand::BeginSemanticGroup { semantic_id },
                DrawingCommand::DrawPath { path, style },
                DrawingCommand::EndSemanticGroup,
                ..,
            ],
        ) = self.document.commands.get(index..)
        else {
            return Ok(None);
        };
        if !semantic_id.starts_with("journey.task.")
            || !semantic_id.contains(".actor.")
            || path.as_str().strip_suffix(".circle") != Some(semantic_id.as_str())
        {
            return Ok(None);
        }
        let semantic = self
            .semantics
            .get(semantic_id)
            .copied()
            .ok_or_else(|| invalid("Journey actor has no semantic annotation"))?;
        if semantic.role != SemanticRole::Label
            || semantic.link.is_some()
            || !self.debug_visibility(semantic.role)
        {
            return Ok(None);
        }
        let Some((center, radius)) = circle_from_path(self.path_resource(path)?) else {
            return Ok(None);
        };
        write!(
            self.output,
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"",
            fmt(center.x),
            fmt(center.y),
            fmt(radius)
        )?;
        if let Some(class) = self.path_class(path) {
            write!(self.output, " class=\"{}\"", escaped_attr(class.as_ref()))?;
        }
        if !self.write_journey_circle_presentation(path, style)? {
            self.write_fill_stroke_style(style)?;
            self.write_state_attrs()?;
        }
        write_resource_metadata(&mut self.output, self.debug, path.as_str())?;
        write_semantic_metadata(&mut self.output, self.debug, semantic_id)?;
        self.output.push('>')?;
        if let Some(title) = semantic.title.as_deref() {
            self.output.push_str("<title>")?;
            output::escape_xml(&mut self.output, title)?;
            self.output.push_str("</title>")?;
        }
        if let Some(description) = semantic.description.as_deref() {
            self.output.push_str("<desc>")?;
            output::escape_xml(&mut self.output, description)?;
            self.output.push_str("</desc>")?;
        }
        self.output.push_str("</circle>")?;
        Ok(Some(3))
    }

    /// Preserve the source HTML identity shell around one public clipped text scope. The
    /// marked group is the complete native projection, including its authoritative clip.
    pub(super) fn emit_journey_box_text(&mut self, index: usize) -> Result<Option<usize>> {
        let SvgStructureBody::Journey(body) = self.svg_body else {
            return Ok(None);
        };
        if self.state.transform != Transform::IDENTITY
            || self.state.opacity != 1.0
            || self.state.blend_mode != BlendMode::Normal
        {
            return Ok(None);
        }
        let Some(
            [
                DrawingCommand::Save,
                DrawingCommand::ClipPath { path, .. },
                ..,
            ],
        ) = self.document.commands.get(index..)
        else {
            return Ok(None);
        };
        let Some(scope) = self.current_semantic_id() else {
            return Ok(None);
        };
        if path.as_str().strip_suffix(".label.clip") != Some(scope) {
            return Ok(None);
        }
        let Some(class) = body.text_classes.get(scope) else {
            return Ok(None);
        };
        let Some(bounds) = rectangle_from_path(self.path_resource(path)?) else {
            return Ok(None);
        };
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return Ok(None);
        }
        let mut end = index + 2;
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
            self.document.commands.get(end..),
            Some([
                DrawingCommand::Restore,
                DrawingCommand::EndSemanticGroup,
                ..
            ])
        ) {
            return Ok(None);
        }
        // Overflow is governed only by the public clip below. The HTML and nested SVG shells
        // must not add independent clipping or ask the browser to wrap this text a second time.
        write!(
            self.output,
            "<switch><foreignObject x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" overflow=\"visible\"><div xmlns=\"http://www.w3.org/1999/xhtml\" class=\"{}\" style=\"display: table; height: 100%; width: 100%; position: relative;\"><div class=\"label\" style=\"display: table-cell; text-align: center; vertical-align: middle;\"><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" overflow=\"visible\" style=\"position: absolute; left: 0; top: 0; overflow: visible;\"><g transform=\"translate({}, {})\"><g data-merman-native-text=\"v1\">",
            fmt(bounds.x),
            fmt(bounds.y),
            fmt(bounds.width),
            fmt(bounds.height),
            escaped_attr(class),
            fmt(bounds.width),
            fmt(bounds.height),
            fmt(-bounds.x),
            fmt(-bounds.y)
        )?;
        for command in &self.document.commands[index..=end] {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_command(command)?;
        }
        self.output
            .push_str("</g></g></svg></div></div></foreignObject></switch>")?;
        Ok(Some(end - index + 1))
    }

    fn journey_scope_has_native_name(&self, name: &str) -> Result<bool> {
        let mut remaining = name.as_bytes();
        let mut has_text = false;
        let mut commands = &self.document.commands[self.command_index + 1..];
        while let Some((command, rest)) = commands.split_first() {
            self.session.checkpoint(OperationPhase::Emit)?;
            match command {
                DrawingCommand::BeginSemanticGroup { .. } => {
                    // Only fixed non-text child scopes can be skipped. Nested or edited text
                    // keeps the explicit parent name, without an unbounded recursive scan.
                    let consumed = match commands {
                        [
                            DrawingCommand::BeginSemanticGroup { .. },
                            DrawingCommand::DrawPath { .. },
                            DrawingCommand::EndSemanticGroup,
                            ..,
                        ] => 3,
                        [
                            DrawingCommand::BeginSemanticGroup { .. },
                            DrawingCommand::DrawPath { .. },
                            DrawingCommand::DrawPath { .. },
                            DrawingCommand::DrawPath { .. },
                            DrawingCommand::EndSemanticGroup,
                            ..,
                        ] => 5,
                        _ => return Ok(false),
                    };
                    commands = &commands[consumed..];
                    continue;
                }
                DrawingCommand::EndSemanticGroup => return Ok(has_text && remaining.is_empty()),
                DrawingCommand::DrawText { run } => {
                    if has_text {
                        let Some(tail) = remaining.strip_prefix(b"\n") else {
                            return Ok(false);
                        };
                        remaining = tail;
                    }
                    let Some(tail) = remaining.strip_prefix(run.text.as_bytes()) else {
                        return Ok(false);
                    };
                    remaining = tail;
                    has_text = true;
                }
                _ => {}
            }
            commands = rest;
        }
        Ok(false)
    }
}
