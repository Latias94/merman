//! Pie typography projected from the public document, never from a second theme input.

use super::super::util::fmt;
use crate::drawing_list::PieSvgBody;
use crate::environment::RenderSession;
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_display_list::{Color, DrawingCommand, DrawingListDocument, Paint, TextStyle};
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub(in crate::svg::parity) struct PieSvgStyles<'a> {
    pub(in crate::svg::parity) texts: BTreeMap<&'a str, Option<&'a TextStyle>>,
}

impl<'a> PieSvgStyles<'a> {
    pub(in crate::svg::parity) fn new(
        document: &'a DrawingListDocument,
        body: &'a PieSvgBody,
        session: &RenderSession,
    ) -> Result<Self> {
        let mut groups = Vec::new();
        let mut texts: BTreeMap<&str, Option<&TextStyle>> = BTreeMap::new();
        for command in &document.commands {
            session.checkpoint(OperationPhase::Emit)?;
            match command {
                DrawingCommand::BeginSemanticGroup { semantic_id } => {
                    groups.push(semantic_id.as_str())
                }
                DrawingCommand::EndSemanticGroup => {
                    groups.pop();
                }
                DrawingCommand::DrawText { run } => {
                    let class = groups.last().and_then(|id| {
                        body.text_classes
                            .get(*id)
                            .map(String::as_str)
                            .or_else(|| id.starts_with("pie.legend.").then_some("legend text"))
                    });
                    if let Some(class) = class {
                        texts
                            .entry(class)
                            .and_modify(|style| {
                                if style.is_some_and(|previous| previous != &run.style) {
                                    *style = None;
                                }
                            })
                            .or_insert(Some(&run.style));
                    }
                }
                _ => {}
            }
        }
        Ok(Self { texts })
    }

    pub(in crate::svg::parity) fn css(&self, diagram_id: &str) -> Result<String> {
        let mut css = String::new();
        for (class, style) in &self.texts {
            let Some(style) = style else { continue };
            let Paint::Solid { color, opacity } = style.fill else {
                continue;
            };
            if style.font.resource.is_some() {
                continue;
            }
            let font =
                crate::portable_font::PortableFontFamilies::from_resolved(&style.font.families)
                    .map_err(|error| Error::InvalidModel {
                        message: error.to_string(),
                    })?
                    .to_css();
            write!(
                css,
                "#{diagram_id} .{class}{{font-family:{font};font-size:{}px;fill:{};fill-opacity:{};}}",
                fmt(style.font_size),
                css_color(color),
                fmt(f64::from(color.alpha) / 255.0 * opacity)
            )
            .map_err(|_| Error::InvalidModel {
                message: "failed to write Pie styles".to_string(),
            })?;
        }
        Ok(css)
    }
}

fn css_color(color: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.red, color.green, color.blue)
}
