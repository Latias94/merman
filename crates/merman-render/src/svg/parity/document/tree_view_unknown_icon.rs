//! The unknown-icon DOM is a representation of a public clipped rectangle and text run.
//! No source icon body, theme paint, or private geometry is consulted here.

use super::*;

pub(super) const COMMAND_COUNT: usize = 7;

#[derive(Clone, Copy)]
pub(super) struct UnknownIcon<'a> {
    translation: Transform,
    viewport: Rect,
    view_box: Rect,
    background_id: &'a ResourceId,
    background: Rect,
    style: &'a PathStyle,
    run: &'a TextRun,
}

pub(super) fn projections<'a>(
    document: &'a DrawingListDocument,
    resources: &BTreeMap<String, &'a DrawingResource>,
    session: &RenderSession,
) -> Result<BTreeMap<usize, UnknownIcon<'a>>> {
    let mut result = BTreeMap::new();
    for (index, commands) in document.commands.windows(COMMAND_COUNT).enumerate() {
        session.checkpoint(OperationPhase::Emit)?;
        let [
            DrawingCommand::Save,
            DrawingCommand::ConcatTransform {
                transform: translation,
            },
            DrawingCommand::ClipPath { path: clip, .. },
            DrawingCommand::ConcatTransform { transform: scale },
            DrawingCommand::DrawPath { path, style },
            DrawingCommand::DrawText { run },
            DrawingCommand::Restore,
        ] = commands
        else {
            continue;
        };
        if !clip.as_str().ends_with(".asset.clip")
            || !path.as_str().ends_with(".asset.unknown.background")
            || translation.a != 1.0
            || translation.d != 1.0
            || translation.b != 0.0
            || translation.c != 0.0
            || scale.a <= 0.0
            || scale.a != scale.d
            || scale.b != 0.0
            || scale.c != 0.0
            || scale.e != 0.0
            || scale.f != 0.0
            || !matches!(run.obligation, TextObligation::HostText { .. })
            || run.style.font.resource.is_some()
            || run.style.stroke.is_some()
            || !matches!(run.style.fill, Paint::Solid { .. })
            || run.anchor != TextAnchor::Start
            || run.baseline != TextBaseline::Alphabetic
            || run.text.contains(['\r', '\n'])
            || style.stroke.is_some()
            || !style
                .fill
                .as_ref()
                .is_none_or(|paint| matches!(paint, Paint::Solid { .. }))
        {
            continue;
        }
        let (Some(DrawingResource::Path(clip)), Some(DrawingResource::Path(background))) = (
            resources.get(clip.as_str()).copied(),
            resources.get(path.as_str()).copied(),
        ) else {
            continue;
        };
        let (Some(viewport), Some(background)) =
            (rectangle_from_path(clip), rectangle_from_path(background))
        else {
            continue;
        };
        if viewport.x != 0.0 || viewport.y != 0.0 || viewport.width <= 0.0 || viewport.height <= 0.0
        {
            continue;
        }
        let view_box = Rect::new(
            0.0,
            0.0,
            viewport.width / scale.a,
            viewport.height / scale.a,
        );
        if !view_box.width.is_finite()
            || !view_box.height.is_finite()
            || view_box.width <= 0.0
            || view_box.height <= 0.0
        {
            continue;
        }
        result.insert(
            index,
            UnknownIcon {
                translation: *translation,
                viewport,
                view_box,
                background_id: path,
                background,
                style,
                run,
            },
        );
    }
    Ok(result)
}

impl DocumentSvgEncoder<'_> {
    pub(super) fn emit_tree_view_unknown_icon(&mut self, index: usize) -> Result<bool> {
        let Some(icon) = self.tree_view_unknown_icons.get(&index).copied() else {
            return Ok(false);
        };
        let matrix = multiply_transform(self.state.transform, icon.translation);
        self.output
            .push_str("<g class=\"treeView-node-icon\" transform=\"")?;
        if matrix.a == 1.0 && matrix.d == 1.0 && matrix.b == 0.0 && matrix.c == 0.0 {
            write!(
                self.output,
                "translate({},{})",
                fmt(matrix.e),
                fmt(matrix.f)
            )?;
        } else {
            write!(self.output, "matrix({})", matrix_attr(matrix))?;
        }
        write!(
            self.output,
            "\"><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\"><g><rect width=\"{}\" height=\"{}\"",
            fmt(icon.viewport.width),
            fmt(icon.viewport.height),
            fmt(icon.view_box.width),
            fmt(icon.view_box.height),
            fmt(icon.background.width),
            fmt(icon.background.height)
        )?;
        if icon.background.x != 0.0 || icon.background.y != 0.0 {
            write!(
                self.output,
                " x=\"{}\" y=\"{}\"",
                fmt(icon.background.x),
                fmt(icon.background.y)
            )?;
        }
        self.output.push_str(" style=\"")?;
        match &icon.style.fill {
            Some(paint @ Paint::Solid { color, .. }) => write!(
                self.output,
                "fill:{};fill-opacity:{};",
                color_css(*color),
                fmt(paint_opacity(paint))
            )?,
            None => self.output.push_str("fill:none;")?,
            Some(Paint::Resource { .. }) => {
                return Err(invalid("TreeView unknown icon requires solid paint"));
            }
        }
        write!(
            self.output,
            "fill-rule:{};stroke:none;opacity:{};",
            fill_rule_name(icon.style.fill_rule),
            fmt(self.state.opacity)
        )?;
        if let Some(blend) = blend_css(self.state.blend_mode) {
            write!(self.output, "mix-blend-mode:{blend};")?;
        }
        self.output.push('"')?;
        self.write_path_metadata(icon.background_id.as_str())?;
        self.output.push_str("/>")?;

        let run = icon.run;
        let Paint::Solid { color, .. } = run.style.fill else {
            return Err(invalid("TreeView unknown icon requires solid text paint"));
        };
        let font = self.font_families(&run.style.font)?;
        write!(
            self.output,
            "<text transform=\"translate({} {})\" style=\"fill:{};fill-opacity:{};font-family:{};font-size:{}px;font-weight:{};font-style:{};letter-spacing:{}px;line-height:{}px;white-space:pre;text-anchor:start;stroke:none;opacity:{};",
            fmt(run.origin.x),
            fmt(run.origin.y),
            color_css(color),
            fmt(paint_opacity(&run.style.fill)),
            escaped_attr(&font),
            fmt(run.style.font_size),
            run.style.font.weight,
            font_style(run.style.font.style),
            fmt(run.style.letter_spacing),
            fmt(run.style.line_height),
            fmt(self.state.opacity)
        )?;
        if let Some(blend) = blend_css(self.state.blend_mode) {
            write!(self.output, "mix-blend-mode:{blend};")?;
        }
        self.output.push('"')?;
        if run.direction != TextDirection::Auto {
            write!(
                self.output,
                " direction=\"{}\"",
                text_direction(run.direction)
            )?;
        }
        if let Some(language) = &run.language {
            write!(self.output, " xml:lang=\"{}\"", escaped_attr(language))?;
        }
        write_text_metadata(&mut self.output, self.debug, run)?;
        self.write_tree_view_leaf_metadata()?;
        self.output.push_str("><tspan x=\"0\" y=\"0\">")?;
        output::escape_xml(&mut self.output, &run.text)?;
        self.output.push_str("</tspan></text></g></svg></g>")?;
        Ok(true)
    }
}
