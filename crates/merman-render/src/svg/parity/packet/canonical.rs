//! Packet DOM presentation projected exclusively from the public drawing commands.

use super::super::util::fmt;
use crate::Result;
use crate::environment::RenderSession;
use merman_core::OperationPhase;
use merman_display_list::{
    DrawingCommand, DrawingListDocument, FillRule, FontDescriptor, FontStyle, LineCap, LineJoin,
    Paint, PathStyle, TextDirection, TextRun, TextStyle,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub(in crate::svg::parity) fn packet_text_class(
    semantic_id: Option<&str>,
    text_index: Option<usize>,
) -> Option<&'static str> {
    match semantic_id {
        Some("packet.title") => Some("packetTitle"),
        Some(id) if id.starts_with("packet.block.") => match text_index {
            Some(0) => Some("packetLabel"),
            Some(1) => Some("packetByte start"),
            Some(2) => Some("packetByte end"),
            _ => None,
        },
        _ => None,
    }
}

/// Retains Mermaid's family selectors when their commands share a presentation style.
/// Heterogeneous commands use their own SVG attributes instead: a class rule must never
/// overwrite another command's paint or font size.
pub(in crate::svg::parity) struct PacketSvgStyles<'a> {
    texts: BTreeMap<&'static str, Option<&'a TextStyle>>,
    blocks: Option<&'a PathStyle>,
    font: Option<&'a FontDescriptor>,
}

impl<'a> PacketSvgStyles<'a> {
    pub(in crate::svg::parity) fn new(
        document: &'a DrawingListDocument,
        session: &RenderSession,
    ) -> Result<Self> {
        let mut groups = Vec::new();
        let mut text_index = 0;
        let mut texts: BTreeMap<&str, Option<&TextStyle>> = BTreeMap::new();
        let mut blocks: Option<Option<&PathStyle>> = None;
        let mut font: Option<Option<&FontDescriptor>> = None;
        for command in &document.commands {
            session.checkpoint(OperationPhase::Emit)?;
            match command {
                DrawingCommand::BeginSemanticGroup { semantic_id } => {
                    groups.push(semantic_id.as_str());
                    text_index = 0;
                }
                DrawingCommand::EndSemanticGroup => {
                    groups.pop();
                }
                DrawingCommand::DrawText { run } => {
                    match &mut font {
                        None => font = Some(Some(&run.style.font)),
                        Some(shared)
                            if shared.is_some_and(|previous| previous != &run.style.font) =>
                        {
                            *shared = None;
                        }
                        _ => {}
                    }
                    if let Some(class) = packet_text_class(groups.last().copied(), Some(text_index))
                    {
                        texts
                            .entry(class)
                            .and_modify(|shared| {
                                if shared.is_some_and(|style| style != &run.style) {
                                    *shared = None;
                                }
                            })
                            .or_insert(Some(&run.style));
                    }
                    text_index += 1;
                }
                DrawingCommand::DrawPath { path, style } if path.as_str().ends_with(".shape") => {
                    match &mut blocks {
                        None => blocks = Some(Some(style)),
                        Some(shared) if shared.is_some_and(|previous| previous != style) => {
                            *shared = None
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            texts,
            blocks: blocks.flatten(),
            font: font.flatten(),
        })
    }

    pub(in crate::svg::parity) fn css(&self, diagram_id: &str) -> Result<String> {
        let mut css = String::new();
        if let Some(font) = self.font.filter(|font| font.resource.is_none()) {
            let families =
                crate::portable_font::PortableFontFamilies::from_resolved(&font.families)
                    .map_err(|error| crate::Error::InvalidModel {
                        message: error.to_string(),
                    })?
                    .to_css();
            let _ = write!(css, "#{diagram_id}{{font-family:{families};}}");
        }
        for (class, style) in &self.texts {
            let Some(style) = style else { continue };
            let Some(color) = solid_css(Some(&style.fill)) else {
                continue;
            };
            let selector = class.replace(' ', ".");
            let _ = write!(css, "#{diagram_id} .{selector}{{fill:{color};");
            if !class.starts_with("packetByte") {
                let _ = write!(css, "font-size:{}px;", fmt(style.font_size));
            }
            css.push('}');
        }
        // Bit labels share the packetByte selector, so publish its size only when all present
        // start/end classes agree. Missing bit classes have no visual output to style.
        let mut bit_size = None;
        let mut uniform = true;
        for (class, style) in &self.texts {
            if !class.starts_with("packetByte") {
                continue;
            }
            let Some(style) = style else {
                uniform = false;
                break;
            };
            if bit_size.is_some_and(|size| size != style.font_size) {
                uniform = false;
                break;
            }
            bit_size = Some(style.font_size);
        }
        if uniform && let Some(size) = bit_size {
            let _ = write!(
                css,
                "#{diagram_id} .packetByte{{font-size:{}px;}}",
                fmt(size)
            );
        }
        if let Some(style) = self.blocks
            && let Some(fill) = solid_css(style.fill.as_ref())
            && let Some(stroke) = solid_css(style.stroke.as_ref().map(|stroke| &stroke.paint))
        {
            let width = style.stroke.as_ref().map_or(0.0, |stroke| stroke.width);
            let _ = write!(
                css,
                "#{diagram_id} .packetBlock{{stroke:{stroke};stroke-width:{};fill:{fill};}}",
                fmt(width)
            );
        }
        Ok(css)
    }

    /// Class CSS and root inheritance are sufficient only for this exact public style. All other
    /// commands retain explicit SVG attributes, including non-default font/stroke state and alpha.
    pub(in crate::svg::parity) fn compact_text(&self, class: &str, run: &TextRun) -> bool {
        self.texts.get(class).copied().flatten() == Some(&run.style)
            && !run.text.contains('\n')
            && (!class.starts_with("packetByte")
                || self
                    .texts
                    .iter()
                    .filter(|(class, _)| class.starts_with("packetByte"))
                    .all(|(_, style)| {
                        style.is_some_and(|style| style.font_size == run.style.font_size)
                    }))
            && self.font == Some(&run.style.font)
            && run.style.font.resource.is_none()
            && run.style.font.weight == 400
            && run.style.font.style == FontStyle::Normal
            && run.style.letter_spacing == 0.0
            && run.style.stroke.is_none()
            && opaque_solid(&run.style.fill)
            && run.direction == TextDirection::Auto
            && run.language.is_none()
    }

    pub(in crate::svg::parity) fn compact_block(&self, style: &PathStyle) -> bool {
        self.blocks == Some(style)
            && style.fill_rule == FillRule::NonZero
            && style.fill.as_ref().is_none_or(opaque_solid)
            && style.stroke.as_ref().is_none_or(|stroke| {
                opaque_solid(&stroke.paint)
                    && stroke.dash_array.is_empty()
                    && stroke.dash_offset == 0.0
                    && stroke.line_cap == LineCap::Butt
                    && stroke.line_join == LineJoin::Miter
                    && stroke.miter_limit == 4.0
            })
    }
}

fn opaque_solid(paint: &Paint) -> bool {
    matches!(paint, Paint::Solid { color } if color.alpha == 255)
}

fn solid_css(paint: Option<&Paint>) -> Option<String> {
    match paint {
        None => Some("none".to_string()),
        // Alpha is emitted once by the command's fill/stroke-opacity attribute.
        Some(Paint::Solid { color }) => Some(format!(
            "#{:02x}{:02x}{:02x}",
            color.red, color.green, color.blue
        )),
        Some(Paint::Resource { .. }) => None,
    }
}
