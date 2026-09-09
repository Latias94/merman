//! Gantt SVG chrome; geometry and paint are owned by the public command stream.

use super::*;

/// Index only uniquely occurring, single-run task labels. Edited scopes stay generic.
pub(super) struct TextProjection<'a> {
    labels: BTreeMap<&'a str, Option<&'a TextRun>>,
}

impl<'a> TextProjection<'a> {
    pub(super) fn new(document: &'a DrawingListDocument, session: &RenderSession) -> Result<Self> {
        let mut labels = BTreeMap::new();
        for (index, command) in document.commands.iter().enumerate() {
            session.checkpoint(OperationPhase::Emit)?;
            let DrawingCommand::BeginSemanticGroup { semantic_id } = command else {
                continue;
            };
            if !semantic_id.starts_with("gantt.task.") || !semantic_id.ends_with(".label") {
                continue;
            }
            let run = match document.commands.get(index + 1..) {
                Some(
                    [
                        DrawingCommand::DrawText { run },
                        DrawingCommand::EndSemanticGroup,
                        ..,
                    ],
                ) if matches!(run.obligation, TextObligation::HostText { .. }) => {
                    Some(run.as_ref())
                }
                _ => None,
            };
            labels
                .entry(semantic_id.as_str())
                .and_modify(|entry| *entry = None)
                .or_insert(run);
        }
        Ok(Self { labels })
    }
}

impl DocumentSvgEncoder<'_> {
    fn gantt_task_text(&self, semantic_id: &str) -> Option<&TextRun> {
        let SvgStructureBody::Gantt(body) = self.svg_body else {
            return None;
        };
        let label_id = if semantic_id.ends_with(".label") {
            Cow::Borrowed(semantic_id)
        } else {
            Cow::Owned(format!("{semantic_id}.label"))
        };
        let label = self.semantics.get(label_id.as_ref())?;
        if label.role != SemanticRole::Label || !body.dom_ids.contains_key(label_id.as_ref()) {
            return None;
        }
        self.gantt_text
            .as_ref()?
            .labels
            .get(label_id.as_ref())
            .copied()
            .flatten()
    }

    /// Own one element's CSS declaration block so paint, transform and blend compose once.
    pub(super) fn write_gantt_path_presentation(
        &mut self,
        path_id: &ResourceId,
        style: &PathStyle,
        path_element: bool,
    ) -> Result<bool> {
        let SvgStructureBody::Gantt(body) = self.svg_body else {
            return Ok(false);
        };
        let raw_id = path_id.as_str();
        let primitive = raw_id.starts_with("gantt.task.") && raw_id.ends_with(".bar")
            || raw_id == "gantt.today.line";
        let origin = body.path_transform_bases.get(raw_id).copied();
        if !primitive && origin.is_none() {
            return Ok(false);
        }
        let matrix = self.state.transform;
        // Keep the public element transform M authoritative. P changes only its SVG basis:
        // T(P) [T(-P) M T(P)] T(-P) = M. Ancestor transforms remain outside this operation.
        let adjusted = if let Some(origin) = origin {
            if !origin.x.is_finite() || !origin.y.is_finite() {
                return Err(invalid(
                    "Gantt SVG transform basis exceeds finite coordinates",
                ));
            }
            Transform {
                e: matrix.e + (matrix.a - 1.0) * origin.x + matrix.c * origin.y,
                f: matrix.f + matrix.b * origin.x + (matrix.d - 1.0) * origin.y,
                ..matrix
            }
        } else {
            matrix
        };
        if !adjusted.e.is_finite() || !adjusted.f.is_finite() {
            return Err(invalid(
                "Gantt SVG transform basis exceeds finite coordinates",
            ));
        }
        if let Some(origin) = origin {
            // Round-trip both origin and matrix; independent integer snapping changes geometry.
            write!(
                self.output,
                " transform-origin=\"{}px {}px\"",
                origin.x, origin.y
            )?;
        }
        if path_element {
            write!(
                self.output,
                " fill-rule=\"{}\"",
                fill_rule_name(style.fill_rule)
            )?;
        }
        // Keep general path/resource/dash paint attributes when the compact source-shaped
        // projection cannot describe them. Transform and blend still share one CSS block.
        let paint = (primitive && !path_element && self.state.blend_mode == BlendMode::Normal)
            .then(|| gantt_solid_paint(style))
            .flatten();
        if paint.is_none() {
            self.write_fill_stroke_style(style)?;
        }
        let blend = blend_css(self.state.blend_mode);
        if paint.is_some() || matrix != Transform::IDENTITY || blend.is_some() {
            self.output.push_str(" style=\"")?;
            if let Some(paint) = paint {
                if let Some(color) = paint.fill {
                    write!(
                        self.output,
                        "fill:{};fill-opacity:{};",
                        color_css(color),
                        fmt(f64::from(color.alpha) / 255.0)
                    )?;
                } else {
                    self.output.push_str("fill:none;")?;
                }
                if let Some((color, width)) = paint.stroke {
                    write!(
                        self.output,
                        "stroke:{};stroke-opacity:{};stroke-width:{};stroke-linecap:butt;stroke-linejoin:miter;stroke-miterlimit:4;stroke-dasharray:none;stroke-dashoffset:0;",
                        color_css(color),
                        fmt(f64::from(color.alpha) / 255.0),
                        fmt(width)
                    )?;
                } else {
                    self.output.push_str("stroke:none;")?;
                }
            }
            if matrix != Transform::IDENTITY {
                // CSS matrix() requires comma separators. The pinned source also expresses
                // milestone transforms in CSS, but these coefficients come only from M.
                write!(
                    self.output,
                    "transform:matrix({},{},{},{},{},{});",
                    adjusted.a, adjusted.b, adjusted.c, adjusted.d, adjusted.e, adjusted.f
                )?;
            }
            if let Some(blend) = blend {
                write!(self.output, "mix-blend-mode:{blend};")?;
            }
            self.output.push('"')?;
        }
        if self.state.opacity != 1.0 {
            write!(self.output, " opacity=\"{}\"", fmt(self.state.opacity))?;
        }
        Ok(true)
    }

    pub(super) fn write_gantt_task_text_height(&mut self) -> Result<()> {
        if let SvgStructureBody::Gantt(body) = self.svg_body
            && let Some(height) = body.task_text_height_attribute
            && self
                .current_semantic_id()
                .is_some_and(|id| id.starts_with("gantt.task."))
        {
            // This source DOM annotation is not the run's measured or painted height.
            write!(self.output, " text-height=\"{}\"", fmt(height))?;
        }
        Ok(())
    }

    pub(super) fn write_gantt_title_name(&mut self, run: &TextRun) -> Result<()> {
        if !matches!(self.svg_body, SvgStructureBody::Gantt(_))
            || !matches!(self.groups.last(), Some(GroupKind::Semantic {
                semantic_id, emitted: false, ..
            }) if semantic_id == "gantt.title")
        {
            return Ok(());
        }
        if let Some(name) = self
            .semantics
            .get("gantt.title")
            .and_then(|s| s.title.as_deref())
            && name != run.text
        {
            // An independently authored accessible name must survive visible-text edits.
            write!(self.output, " aria-label=\"{}\"", escaped_attr(name))?;
        }
        Ok(())
    }

    pub(super) fn write_gantt_text_space_attr(
        &mut self,
        run: &TextRun,
        semantic_id: Option<&str>,
    ) -> Result<()> {
        if !matches!(self.svg_body, SvgStructureBody::Gantt(_)) {
            return Ok(());
        }
        let Some(id) = semantic_id else {
            return Ok(());
        };
        let preserve = (id.starts_with("gantt.section.")
            || id == "gantt.title"
            || id.starts_with("gantt.task."))
            && gantt_text_has_significant_whitespace(&run.text);
        if preserve {
            // Source whitespace is already resolved; paint fallback must not collapse it again.
            // Native SVG consumers also need xml:space, not just CSS white-space.
            self.output.push_str(" xml:space=\"preserve\"")?;
        }
        Ok(())
    }

    pub(super) fn emit_gantt_plain_text(
        &mut self,
        run: &TextRun,
        semantic_id: Option<&str>,
    ) -> Result<bool> {
        let SvgStructureBody::Gantt(body) = self.svg_body else {
            return Ok(false);
        };
        let Some(id) = semantic_id else {
            return Ok(false);
        };
        let title = id == "gantt.title";
        let task = id.starts_with("gantt.task.") && id.ends_with(".label");
        if !(title || task)
            || !matches!(self.groups.last(), Some(GroupKind::Semantic {
                semantic_id, emitted: false, ..
            }) if semantic_id == id)
            || task && !body.dom_ids.contains_key(id)
            || run.baseline != TextBaseline::Alphabetic
            || run.direction != TextDirection::Auto
            || run.language.is_some()
            || run.style.font.resource.is_some()
            || run.style.stroke.is_some()
            || run.text.contains(['\n', '\r', '\t'])
            || self.state.blend_mode != BlendMode::Normal
        {
            return Ok(false);
        }
        let Paint::Solid { color } = run.style.fill else {
            return Ok(false);
        };
        if color.alpha != 255 {
            return Ok(false);
        }
        let Some(class) = body.text_classes.get(id) else {
            return Ok(false);
        };
        let font = self.font_families(&run.style.font)?;
        self.output.push_str("<text")?;
        self.write_gantt_dom_id(id)?;
        self.write_gantt_title_name(run)?;
        if task {
            write!(self.output, " font-size=\"{}\"", fmt(run.style.font_size))?;
        }
        write!(
            self.output,
            " x=\"{}\" y=\"{}\" class=\"{}\"",
            fmt(run.origin.x),
            fmt(run.origin.y),
            escaped_attr(class),
        )?;
        self.write_gantt_task_text_height()?;
        // CSS is a projection of the resolved run, not a replay of source class/theme rules.
        // Preserve the public string literally; the adapter already resolved source whitespace.
        self.write_gantt_text_space_attr(run, semantic_id)?;
        write!(
            self.output,
            " style=\"font-family:{};font-weight:{};font-style:{};letter-spacing:{}px;text-anchor:{};dominant-baseline:alphabetic;fill:{};fill-opacity:1;stroke:none;",
            escaped_attr(&font),
            run.style.font.weight,
            font_style(run.style.font.style),
            fmt(run.style.letter_spacing),
            text_anchor(run.anchor),
            color_css(color),
        )?;
        if title {
            write!(self.output, "font-size:{}px;", fmt(run.style.font_size))?;
        }
        self.output.push('"')?;
        self.write_state_attrs()?;
        self.output.push('>')?;
        output::escape_xml(&mut self.output, &run.text)?;
        self.output.push_str("</text>")?;
        Ok(true)
    }

    pub(super) fn emit_gantt_row(
        &mut self,
        path_id: &ResourceId,
        style: &PathStyle,
    ) -> Result<bool> {
        if !matches!(self.svg_body, SvgStructureBody::Gantt(_))
            || !path_id.as_str().starts_with("gantt.row.")
            || style.stroke.is_some()
            || self.state.blend_mode != BlendMode::Normal
        {
            return Ok(false);
        }
        let Some(Paint::Solid { color }) = style.fill else {
            return Ok(false);
        };
        let Some(bounds) = rectangle_from_path(self.path_resource(path_id)?) else {
            return Ok(false);
        };
        let Some(class) = self.path_class(path_id) else {
            return Ok(false);
        };
        // Source rows are classed rectangles. Resolve their CSS from the public command rather
        // than replaying the theme, keeping fill alpha separate from object opacity.
        write!(
            self.output,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" class=\"{}\" style=\"fill:{};fill-opacity:{};stroke:none;opacity:{};\"",
            fmt(bounds.x),
            fmt(bounds.y),
            fmt(bounds.width),
            fmt(bounds.height),
            escaped_attr(class.as_ref()),
            color_css(color),
            fmt(f64::from(color.alpha) / 255.0),
            fmt(self.state.opacity),
        )?;
        self.write_transform_and_blend()?;
        self.output.push_str("/>")?;
        Ok(true)
    }

    /// Project D3's axis primitives only when the public stroke has SVG's simple defaults.
    /// The inline color resolves currentColor from the document, never from ambient host CSS.
    pub(super) fn emit_gantt_axis_path(
        &mut self,
        path_id: &ResourceId,
        style: &PathStyle,
    ) -> Result<bool> {
        if !matches!(self.svg_body, SvgStructureBody::Gantt(_))
            || !path_id.as_str().starts_with("gantt.axis.")
            || style.fill.is_some()
            || self.state.blend_mode != BlendMode::Normal
        {
            return Ok(false);
        }
        let Some(stroke) = &style.stroke else {
            return Ok(false);
        };
        let Paint::Solid { color } = stroke.paint else {
            return Ok(false);
        };
        if color.alpha != 255
            || !stroke.dash_array.is_empty()
            || stroke.dash_offset != 0.0
            || stroke.line_cap != merman_display_list::LineCap::Butt
            || stroke.line_join != merman_display_list::LineJoin::Miter
            || stroke.miter_limit != 4.0
        {
            return Ok(false);
        }
        let path = self.path_resource(path_id)?;
        if path_id.as_str().ends_with(".domain") && stroke.width == 0.0 {
            let [
                PathSegment::MoveTo { to: a },
                PathSegment::LineTo { to: b },
                PathSegment::LineTo { to: c },
                PathSegment::LineTo { to: d },
            ] = path.segments.as_slice()
            else {
                return Ok(false);
            };
            if a.x != b.x || b.y != c.y || c.x != d.x {
                return Ok(false);
            }
            write!(
                self.output,
                "<path class=\"domain\" stroke=\"currentColor\" d=\"M{},{}V{}H{}V{}\"",
                fmt(a.x),
                fmt(a.y),
                fmt(b.y),
                fmt(c.x),
                fmt(d.y),
            )?;
        } else if path_id.as_str().contains(".tick.") && stroke.width == 1.0 {
            let Some((start, end)) = line_from_path(path) else {
                return Ok(false);
            };
            if start != Point::new(0.0, 0.0) || end.x != 0.0 {
                return Ok(false);
            }
            write!(
                self.output,
                "<line stroke=\"currentColor\" y2=\"{}\"",
                fmt(end.y)
            )?;
        } else {
            return Ok(false);
        }
        write!(
            self.output,
            " style=\"color:{};fill:none;stroke-width:{};\"",
            color_css(color),
            fmt(stroke.width),
        )?;
        self.write_state_attrs()?;
        self.output.push_str("/>")?;
        Ok(true)
    }

    pub(super) fn begin_gantt_task(&mut self, semantic_id: &str) -> Result<bool> {
        let SvgStructureBody::Gantt(body) = self.svg_body else {
            return Ok(false);
        };
        if !semantic_id.starts_with("gantt.task.") {
            return Ok(false);
        }
        let semantic = self
            .semantics
            .get(semantic_id)
            .copied()
            .ok_or_else(|| invalid("Gantt task has no semantic annotation"))?;
        if !self.debug_visibility(semantic.role)
            || self.gantt_task_text(semantic_id).is_none()
            || semantic
                .title
                .as_deref()
                .is_none_or(|name| name.trim().is_empty())
        {
            // Deleted, duplicated or structurally edited labels no longer name a source-shaped
            // task. Unnamed scopes also stay generic so readable text is not an unnamed image.
            return Ok(false);
        }
        let Some(commands) = self.document.commands.get(self.command_index + 1..) else {
            return Ok(false);
        };
        let projected = match commands {
            [
                DrawingCommand::DrawPath { path, .. },
                DrawingCommand::EndSemanticGroup,
                ..,
            ]
            | [
                DrawingCommand::Save,
                DrawingCommand::ConcatTransform { .. },
                DrawingCommand::DrawPath { path, .. },
                DrawingCommand::Restore,
                DrawingCommand::EndSemanticGroup,
                ..,
            ] => {
                semantic.role == SemanticRole::Node
                    && path.as_str().strip_suffix(".bar") == Some(semantic_id)
                    && body.dom_ids.contains_key(path.as_str())
            }
            [
                DrawingCommand::DrawText { run },
                DrawingCommand::EndSemanticGroup,
                ..,
            ] => {
                semantic.role == SemanticRole::Label
                    && semantic_id.ends_with(".label")
                    && body.dom_ids.contains_key(semantic_id)
                    && matches!(run.obligation, TextObligation::HostText { .. })
            }
            _ => false,
        };
        if !projected {
            return Ok(false);
        }
        let security = MermaidNavigationSecurity::from_security_level_loose(
            self.effective_config
                .get("securityLevel")
                .and_then(Value::as_str)
                == Some("loose"),
        );
        let link = semantic
            .link
            .as_deref()
            .and_then(|link| prepare_mermaid_navigation_uri(link, security));
        if let Some(link) = &link {
            self.output.push_str("<a href=\"")?;
            output::escape_attr(&mut self.output, link)?;
            self.output.push_str("\">")?;
        }
        // Keep the logical scope on the stack. Its one visual element owns the semantic
        // attributes; independent anchors preserve all-bars-before-labels paint ordering.
        self.groups.push(GroupKind::Semantic {
            linked: link.is_some(),
            emitted: false,
            semantic_id: semantic_id.to_owned(),
            projected_transform: Transform::IDENTITY,
        });
        Ok(true)
    }

    pub(super) fn write_gantt_task_semantics(&mut self) -> Result<()> {
        let Some(GroupKind::Semantic {
            emitted: false,
            semantic_id,
            linked,
            ..
        }) = self.groups.last()
        else {
            return Ok(());
        };
        if !semantic_id.starts_with("gantt.task.") {
            return Ok(());
        }
        let semantic = self
            .semantics
            .get(semantic_id)
            .copied()
            .ok_or_else(|| invalid("Gantt task has no semantic annotation"))?;
        let native_name = self.gantt_task_text(semantic_id).is_some_and(|run| {
            semantic
                .title
                .as_deref()
                .is_none_or(|name| name == run.text)
        });
        // A described or linked bar remains a named graphic. A sibling text cannot name
        // its separate anchor or express its description. Ordinary labels remain native text.
        let explicit_name = !native_name
            || semantic.role == SemanticRole::Node && (semantic.description.is_some() || *linked);
        write_semantic_metadata(&mut self.output, self.debug, semantic_id)?;
        if explicit_name {
            if semantic
                .title
                .as_deref()
                .is_some_and(|name| !name.is_empty())
            {
                self.output.push_str(" role=\"img\"")?;
            }
            if let Some(title) = &semantic.title {
                write!(self.output, " aria-label=\"{}\"", escaped_attr(title))?;
            }
        }
        if let Some(description) = &semantic.description {
            write!(
                self.output,
                " aria-description=\"{}\"",
                escaped_attr(description)
            )?;
        }
        Ok(())
    }

    pub(super) fn emit_gantt_section_text(&mut self, index: usize) -> Result<Option<usize>> {
        let SvgStructureBody::Gantt(body) = self.svg_body else {
            return Ok(None);
        };
        if self.state.opacity != 1.0 || self.state.blend_mode != BlendMode::Normal {
            return Ok(None);
        }
        let document = self.document;
        let Some(
            [
                DrawingCommand::BeginSemanticGroup { semantic_id },
                DrawingCommand::DrawText { run: first },
                ..,
            ],
        ) = document.commands.get(index..)
        else {
            return Ok(None);
        };
        if !semantic_id.starts_with("gantt.section.")
            || !self.semantics.get(semantic_id).is_some_and(|semantic| {
                semantic.role == SemanticRole::Label
                    && semantic.link.is_none()
                    && semantic.description.is_none()
                    && self.debug_visibility(semantic.role)
            })
        {
            return Ok(None);
        }
        let Some(class) = body.text_classes.get(semantic_id) else {
            return Ok(None);
        };
        let Paint::Solid { color } = first.style.fill else {
            return Ok(None);
        };
        // Combining translucent or differently styled text could change overlap compositing.
        if color.alpha != 255 || first.style.stroke.is_some() || first.style.font.resource.is_some()
        {
            return Ok(None);
        }
        let mut count = 0;
        loop {
            self.session.checkpoint(OperationPhase::Emit)?;
            match document.commands.get(index + 1 + count) {
                Some(DrawingCommand::DrawText { run })
                    if run.style == first.style
                        && run.baseline == TextBaseline::Central
                        && run.anchor == TextAnchor::Start
                        && run.direction == TextDirection::Auto
                        && run.language.is_none()
                        && matches!(&run.obligation, TextObligation::HostText { .. })
                        && !run.text.contains(['\n', '\r'])
                        && run.origin.x == first.origin.x =>
                {
                    count += 1
                }
                Some(DrawingCommand::EndSemanticGroup) => break,
                _ => return Ok(None),
            }
        }
        if count == 0 {
            return Ok(None);
        }
        let dy = -(count as f64 - 1.0) / 2.0;
        let center_y = first.origin.y - dy * first.style.font_size;
        if !center_y.is_finite() || center_y + dy * first.style.font_size != first.origin.y {
            return Ok(None);
        }
        let commands = &document.commands[index + 1..index + 1 + count];
        let last_text = commands.iter().rposition(|command| {
            matches!(command,
                DrawingCommand::DrawText { run } if crate::gantt::section_line_has_text(&run.text)
            )
        });
        let mut cursor =
            crate::gantt::GanttSectionLineCursor::new(center_y, first.style.font_size, count);
        for (line_index, command) in commands.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let DrawingCommand::DrawText { run } = command else {
                return Ok(None);
            };
            let has_later_text = last_text.is_some_and(|last| line_index < last);
            if !cursor
                .normalized_parts(&run.text, has_later_text)
                .flat_map(str::bytes)
                .eq(run.text.bytes())
            {
                return Ok(None);
            }
            if cursor.next_line(&run.text, has_later_text) != run.origin.y {
                return Ok(None);
            }
        }
        let font = self.font_families(&first.style.font)?;
        self.output.push_str("<text")?;
        if let Some(name) = self
            .semantics
            .get(semantic_id)
            .and_then(|s| s.title.as_deref())
        {
            let visible_name = commands.iter().enumerate().flat_map(|(index, command)| {
                let text = match command {
                    DrawingCommand::DrawText { run } => run.text.as_str(),
                    _ => "",
                };
                (index != 0)
                    .then_some(b'\n')
                    .into_iter()
                    .chain(text.bytes())
            });
            if !name.bytes().eq(visible_name) {
                // Native text names itself only when it expresses the public semantic name.
                // An independently edited name must not disappear during tspan compaction.
                write!(self.output, " aria-label=\"{}\"", escaped_attr(name))?;
            }
        }
        // Ordinary source labels rely on SVG's default whitespace behavior. Retain a
        // preservation attribute only when the resolved public runs need it, including
        // leading/trailing spaces that remain addressable across source tspans.
        if commands.iter().any(|command| {
            matches!(command, DrawingCommand::DrawText { run }
                if gantt_text_has_significant_whitespace(&run.text))
        }) {
            self.output.push_str(" xml:space=\"preserve\"")?;
        }
        write!(
            self.output,
            " dy=\"{}em\" x=\"{}\" y=\"{}\" font-size=\"{}\" class=\"{}\" style=\"font-family:",
            fmt(dy),
            fmt(first.origin.x),
            fmt(center_y),
            fmt(first.style.font_size),
            escaped_attr(class)
        )?;
        output::escape_attr(&mut self.output, &font)?;
        write!(
            self.output,
            ";font-weight:{};font-style:{};letter-spacing:{}px;text-anchor:start;fill:{};stroke:none;\"",
            first.style.font.weight,
            font_style(first.style.font.style),
            fmt(first.style.letter_spacing),
            color_css(color)
        )?;
        self.write_state_attrs()?;
        self.output.push('>')?;
        for (line_index, command) in document.commands[index + 1..index + 1 + count]
            .iter()
            .enumerate()
        {
            self.session.checkpoint(OperationPhase::Emit)?;
            let DrawingCommand::DrawText { run } = command else {
                return Err(invalid("Gantt section projection requires text commands"));
            };
            write!(
                self.output,
                "<tspan alignment-baseline=\"central\" x=\"{}\"",
                fmt(run.origin.x)
            )?;
            if line_index != 0 {
                self.output.push_str(" dy=\"1em\"")?;
            }
            self.output.push('>')?;
            output::escape_xml(&mut self.output, &run.text)?;
            self.output.push_str("</tspan>")?;
        }
        self.output.push_str("</text>")?;
        Ok(Some(count + 2))
    }

    // Fuse only an exact semantic-owned layer. An edited sequence with more commands or scopes
    // retains the ordinary serializer rather than losing those commands during DOM projection.
    fn gantt_tick_layer_at(&self, index: usize) -> bool {
        if !matches!(self.svg_body, SvgStructureBody::Gantt(_)) {
            return false;
        }
        let Some(
            [
                DrawingCommand::BeginSemanticGroup { semantic_id },
                DrawingCommand::BeginLayer { .. },
                DrawingCommand::DrawPath { .. },
                DrawingCommand::DrawText { .. },
                DrawingCommand::EndLayer,
                DrawingCommand::EndSemanticGroup,
                ..,
            ],
        ) = self.document.commands.get(index..)
        else {
            return false;
        };
        semantic_id.starts_with("gantt.axis.")
            && semantic_id.contains(".tick.")
            && self.semantics.get(semantic_id).is_some_and(|semantic| {
                semantic.role == SemanticRole::Label
                    && semantic.link.is_none()
                    && semantic.description.is_none()
                    && self.debug_visibility(semantic.role)
            })
    }

    pub(super) fn begin_gantt_tick_layer(
        &mut self,
        opacity: f64,
        blend: BlendMode,
    ) -> Result<bool> {
        let Some(index) = self.command_index.checked_sub(1) else {
            return Ok(false);
        };
        if !self.gantt_tick_layer_at(index) {
            return Ok(false);
        }
        write!(
            self.output,
            "<g class=\"tick\" opacity=\"{}\"",
            fmt(opacity)
        )?;
        if let Some(DrawingCommand::BeginSemanticGroup { semantic_id }) =
            self.document.commands.get(index)
            && let Some(name) = self
                .semantics
                .get(semantic_id)
                .and_then(|s| s.title.as_deref())
        {
            let same_visible_name = matches!(self.document.commands.get(index + 3),
                Some(DrawingCommand::DrawText { run })
                if name == run.text);
            if !same_visible_name {
                write!(
                    self.output,
                    " role=\"group\" aria-label=\"{}\"",
                    escaped_attr(name)
                )?;
            }
        }
        self.write_gantt_group_transform()?;
        write_blend_style(&mut self.output, blend)?;
        self.output.push('>')?;
        self.groups.push(GroupKind::Layer {
            projected_transform: self.state.transform,
        });
        self.state.transform = Transform::IDENTITY;
        Ok(true)
    }

    pub(super) fn emit_gantt_tick_text(
        &mut self,
        run: &TextRun,
        semantic_id: Option<&str>,
    ) -> Result<bool> {
        let Some(semantic_id) = semantic_id else {
            return Ok(false);
        };
        if !semantic_id.contains(".tick.")
            || run.baseline != TextBaseline::Alphabetic
            || run.direction != TextDirection::Auto
            || run.language.is_some()
            || run.style.stroke.is_some()
            || run.style.font.resource.is_some()
            || run.style.font.weight != 400
            || run.style.font.style != FontStyle::Normal
            || run.style.letter_spacing != 0.0
            || run.style.font_size != 10.0
            || !matches!(&run.style.fill, Paint::Solid { color } if color.alpha == 255)
            || run.anchor != TextAnchor::Middle
            || run.origin.x != 0.0
            || !matches!(run.origin.y, -3.0 | 13.0)
            || self.state.transform != Transform::IDENTITY
            || self.state.opacity != 1.0
            || self.state.blend_mode != BlendMode::Normal
        {
            return Ok(false);
        }
        let font = self.font_families(&run.style.font)?;
        write!(
            self.output,
            "<text fill=\"{}\" y=\"{}\" dy=\"{}em\" stroke=\"none\" font-size=\"{}\" style=\"font-family:{};text-anchor: {};\"",
            color_css(match &run.style.fill {
                Paint::Solid { color } => *color,
                _ => return Ok(false),
            }),
            fmt(if run.origin.y >= 0.0 {
                run.origin.y - run.style.font_size
            } else {
                run.origin.y
            }),
            if run.origin.y >= 0.0 { "1" } else { "0" },
            fmt(run.style.font_size),
            escaped_attr(font.as_str()),
            text_anchor(run.anchor),
        )?;
        self.output.push('>')?;
        output::escape_xml(&mut self.output, run.text.as_str())?;
        self.output.push_str("</text>")?;
        Ok(true)
    }

    fn write_gantt_group_transform(&mut self) -> Result<()> {
        let transform = self.state.transform;
        if transform.a == 1.0 && transform.b == 0.0 && transform.c == 0.0 && transform.d == 1.0 {
            write!(
                self.output,
                " transform=\"translate({},{})\"",
                fmt(transform.e),
                fmt(transform.f)
            )?;
        } else {
            write!(
                self.output,
                " transform=\"matrix({})\"",
                matrix_attr(transform)
            )?;
        }
        Ok(())
    }

    pub(super) fn begin_gantt_collection(&mut self, semantic_id: &str) -> Result<bool> {
        if !matches!(self.svg_body, SvgStructureBody::Gantt(_)) {
            return Ok(false);
        }
        if semantic_id == "gantt.today" {
            let semantic = self
                .semantics
                .get(semantic_id)
                .copied()
                .ok_or_else(|| invalid("Gantt today marker has no semantic annotation"))?;
            if semantic.link.is_some()
                || semantic.role != SemanticRole::Label
                || !self.debug_visibility(semantic.role)
            {
                return Ok(false);
            }
            // Mermaid's today collection contains only its marker. Keep accessible metadata
            // as attributes, so a synthetic title does not change its direct-child contract.
            let svg_id = self.semantic_svg_id(semantic_id)?;
            write!(
                self.output,
                "<g class=\"today\" id=\"{}\" role=\"group\"",
                escaped_attr(&svg_id)
            )?;
            write_semantic_metadata(&mut self.output, self.debug, semantic_id)?;
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
            self.output.push('>')?;
            self.groups.push(GroupKind::Semantic {
                linked: false,
                emitted: true,
                semantic_id: semantic_id.to_owned(),
                projected_transform: Transform::IDENTITY,
            });
            return Ok(true);
        }
        let axis = matches!(semantic_id, "gantt.axis.bottom" | "gantt.axis.top");
        let semantic = self
            .semantics
            .get(semantic_id)
            .copied()
            .ok_or_else(|| invalid("Gantt collection has no semantic annotation"))?;
        let expected_role = match semantic_id {
            "gantt.document" => SemanticRole::Document,
            "gantt.title" => SemanticRole::Label,
            _ if semantic_id.contains(".tick.") => SemanticRole::Label,
            _ => SemanticRole::Group,
        };
        // Source collection wrappers cannot carry navigation or extra accessible descriptions.
        // Let the general semantic serializer retain those public obligations and debug roles.
        if semantic.link.is_some()
            || (semantic_id != "gantt.document" && semantic.description.is_some())
            || semantic.role != expected_role
            || !self.debug_visibility(semantic.role)
            || (matches!(
                semantic_id,
                "gantt.excludes" | "gantt.rows" | "gantt.tasks" | "gantt.sections"
            ) && semantic.title.is_some())
        {
            return Ok(false);
        }
        let emitted = match semantic_id {
            "gantt.document" | "gantt.title" => false,
            "gantt.excludes" | "gantt.rows" | "gantt.tasks" | "gantt.sections" => true,
            "gantt.axis.bottom" | "gantt.axis.top" => true,
            _ if self.gantt_tick_layer_at(self.command_index) => false,
            _ => return Ok(false),
        };
        // These public scopes define paint order; Mermaid exposes only the collection wrappers.
        // The visible title and accessibility references already have their own SVG anchors.
        let projected_transform = if axis {
            self.output.push_str(
                "<g class=\"grid\" fill=\"none\" font-size=\"10\" font-family=\"sans-serif\" text-anchor=\"middle\"",
            )?;
            if let Some(name) = &semantic.title {
                write!(
                    self.output,
                    " role=\"group\" aria-label=\"{}\"",
                    escaped_attr(name)
                )?;
            }
            self.write_gantt_group_transform()?;
            self.output.push('>')?;
            let transform = self.state.transform;
            self.state.transform = Transform::IDENTITY;
            transform
        } else if emitted {
            self.output.push_str("<g>")?;
            Transform::IDENTITY
        } else {
            Transform::IDENTITY
        };
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted,
            semantic_id: semantic_id.to_owned(),
            projected_transform,
        });
        Ok(true)
    }

    pub(super) fn write_gantt_style(&mut self) -> Result<()> {
        // Retain source cursor/rasterization hints only. Legacy paint, font, opacity and
        // milestone-transform CSS would reinterpret or double-apply resolved drawing commands.
        let id = sanitize_svg_id(self.diagram_id.as_str());
        write!(
            self.output,
            "<style>#{id} .grid .tick{{shape-rendering:crispEdges;}}#{id} .clickable{{cursor:pointer;}}"
        )?;
        for index in 0..4 {
            if index != 0 {
                self.output.push(',')?;
            }
            write!(self.output, "#{id} .doneCrit{index}")?;
        }
        self.output
            .push_str("{cursor:pointer;shape-rendering:crispEdges;}</style><g/>")?;
        Ok(())
    }
}

fn gantt_text_has_significant_whitespace(text: &str) -> bool {
    text.starts_with(' ')
        || text.ends_with(' ')
        || text.contains("  ")
        || text.contains(['\t', '\r', '\n'])
}

struct GanttSolidPaint {
    fill: Option<Color>,
    stroke: Option<(Color, f64)>,
}

fn gantt_solid_paint(style: &PathStyle) -> Option<GanttSolidPaint> {
    let fill = match style.fill {
        Some(Paint::Solid { color }) => Some(color),
        None => None,
        _ => return None,
    };
    let stroke = match &style.stroke {
        Some(stroke) => {
            let Paint::Solid { color } = stroke.paint else {
                return None;
            };
            if !stroke.dash_array.is_empty()
                || stroke.dash_offset != 0.0
                || stroke.line_cap != merman_display_list::LineCap::Butt
                || stroke.line_join != merman_display_list::LineJoin::Miter
                || stroke.miter_limit != 4.0
            {
                return None;
            }
            Some((color, stroke.width))
        }
        None => None,
    };
    Some(GanttSolidPaint { fill, stroke })
}
