//! Info's source-shaped DOM projection from resolved public text and paint.

use super::*;
use merman_display_list::TextStyle;

pub(super) fn shared_text_style<'a>(
    document: &'a DrawingListDocument,
    session: &RenderSession,
) -> Result<Option<&'a TextStyle>> {
    let mut shared = None;
    for command in &document.commands {
        session.checkpoint(OperationPhase::Emit)?;
        let DrawingCommand::DrawText { run } = command else {
            continue;
        };
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

impl DocumentSvgEncoder<'_> {
    pub(super) fn write_info_style(&mut self) -> Result<()> {
        self.output.push_str("<style>")?;
        if let Some(style) = self.info_text_style {
            let Paint::Solid { color, .. } = style.fill else {
                return Err(invalid("Info shared text paint must be solid"));
            };
            let font = self.font_families(&style.font)?;
            write!(
                self.output,
                "#{} .version{{font-family:{};font-weight:{};font-style:{};letter-spacing:{}px;fill:{};fill-opacity:{};}}",
                self.diagram_id,
                font,
                style.font.weight,
                font_style(style.font.style),
                fmt(style.letter_spacing),
                color_css(color),
                fmt(paint_opacity(&style.fill))
            )?;
        }
        self.output.push_str("</style>")
    }

    pub(super) fn emit_compact_info_text(&mut self, run: &TextRun) -> Result<bool> {
        if self.info_text_style != Some(&run.style)
            || run.baseline != TextBaseline::Alphabetic
            || run.direction != TextDirection::Auto
            || run.language.is_some()
            || run.text.contains(['\n', '\r'])
            || self.state.transform != Transform::IDENTITY
            || self.state.opacity != 1.0
            || self.state.blend_mode != BlendMode::Normal
        {
            return Ok(false);
        }
        write!(
            self.output,
            r#"<text x="{}" y="{}" class="version" font-size="{}" style="text-anchor: {};">"#,
            fmt(run.origin.x),
            fmt(run.origin.y),
            fmt(run.style.font_size),
            text_anchor(run.anchor)
        )?;
        output::escape_xml(&mut self.output, run.text.as_str())?;
        self.output.push_str("</text>")?;
        Ok(true)
    }
}
