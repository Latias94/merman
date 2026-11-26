//! Borrowed vector paint for the NativeCompact renderer.
//!
//! Family adapters own preparation, placement, text measurement, and any retained storage.
//! This module accepts only already resolved paint and never decodes or transforms SVG.

use std::fmt::{self, Write};

use merman_model::OperationPhase;

use crate::number_format::{CanonicalNumber, canonical_number, canonicalize_number};
use crate::resources::OperationWorkMeter;
use crate::svg_css::SvgId;
use crate::svg_output::{BoundedSvgOutput, SvgOutput, XmlTextOutput};
use crate::text::{FontStyle, GenericFontFamily};
use crate::xml::{escape_attr_display, is_xml_1_0_char};
use crate::{Error, Result};

pub use merman_markup::theme_color::RgbaChannels as SceneColor;

/// A view of already positioned paint in painter order. No part is raw SVG or CSS.
#[derive(Debug)]
pub struct PaintScene<'a> {
    pub viewport: SceneViewport,
    pub metadata: SceneMetadata<'a>,
    pub markers: &'a [SceneMarker<'a>],
    pub groups: &'a [SceneGroup<'a>],
}

/// A box in diagram coordinates, including a possible negative origin.
#[derive(Debug, Clone, Copy)]
pub struct SceneBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct SceneViewport {
    pub view_box: SceneBox,
    pub width: f64,
    pub height: f64,
    pub canvas: Option<SceneColor>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SceneMetadata<'a> {
    pub title: Option<&'a str>,
    pub description: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub enum SceneRole {
    Node,
    Edge,
    EdgeLabel,
    Cluster,
    Note,
    Activation,
    Annotation,
}

impl SceneRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Edge => "edge",
            Self::EdgeLabel => "edge-label",
            Self::Cluster => "cluster",
            Self::Note => "note",
            Self::Activation => "activation",
            Self::Annotation => "annotation",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SceneGroup<'a> {
    /// Optional local identity. The SVG writer prefixes it with the root ID and group ordinal.
    pub id: Option<SvgId<'a>>,
    pub role: SceneRole,
    pub items: &'a [SceneItem<'a>],
}

#[derive(Debug, Clone, Copy)]
pub enum SceneItem<'a> {
    Rect(SceneRect),
    Path(ScenePath<'a>),
    Text(SceneText<'a>),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ScenePaint {
    pub fill: Option<SceneColor>,
    pub stroke: Option<SceneStroke>,
}

#[derive(Debug, Clone, Copy)]
pub struct SceneStroke {
    pub color: SceneColor,
    pub width: f64,
    pub dash: Option<[f64; 2]>,
}

#[derive(Debug, Clone, Copy)]
pub struct SceneRect {
    pub bounds: SceneBox,
    pub radius: f64,
    pub paint: ScenePaint,
}

#[derive(Debug, Clone, Copy)]
pub enum ScenePathCommand {
    MoveTo([f64; 2]),
    LineTo([f64; 2]),
    CubicTo {
        control1: [f64; 2],
        control2: [f64; 2],
        end: [f64; 2],
    },
    Close,
}

/// An index into the scene's marker table, not an authored fragment URL.
#[derive(Debug, Clone, Copy)]
pub struct SceneMarkerRef(pub usize);

#[derive(Debug, Clone, Copy)]
pub struct ScenePath<'a> {
    pub commands: &'a [ScenePathCommand],
    pub paint: ScenePaint,
    pub start_marker: Option<SceneMarkerRef>,
    pub end_marker: Option<SceneMarkerRef>,
}

#[derive(Debug, Clone, Copy)]
pub enum SceneMarkerUnits {
    UserSpaceOnUse,
    StrokeWidth,
}

#[derive(Debug, Clone, Copy)]
pub enum SceneMarkerOrient {
    Auto,
    AutoStartReverse,
}

#[derive(Debug, Clone, Copy)]
pub struct SceneMarker<'a> {
    pub view_box: Option<SceneBox>,
    pub reference: [f64; 2],
    pub width: f64,
    pub height: f64,
    pub units: SceneMarkerUnits,
    pub orient: SceneMarkerOrient,
    pub commands: &'a [ScenePathCommand],
    pub paint: ScenePaint,
}

#[derive(Debug, Clone, Copy)]
pub enum SceneTextAnchor {
    Start,
    Middle,
    End,
}

#[derive(Debug, Clone, Copy)]
pub enum SceneTextBaseline {
    Alphabetic,
    Central,
    Middle,
}

#[derive(Debug, Clone, Copy)]
pub enum SceneFontFamily<'a> {
    Generic(GenericFontFamily),
    /// A logical family name, never an already-serialized CSS expression.
    Named(&'a str),
}

#[derive(Debug, Clone, Copy)]
pub struct SceneTextStyle<'a> {
    pub families: &'a [SceneFontFamily<'a>],
    pub size: f64,
    pub weight: Option<u16>,
    pub style: FontStyle,
    pub fill: SceneColor,
}

/// One positioned plain-text run. A family adapter owns line breaks and baseline placement.
#[derive(Debug, Clone, Copy)]
pub struct SceneText<'a> {
    pub text: &'a str,
    pub position: [f64; 2],
    pub dy: f64,
    pub anchor: SceneTextAnchor,
    pub baseline: SceneTextBaseline,
    pub style: SceneTextStyle<'a>,
}

/// Emits the complete SVG through the existing exact-byte sink under the caller's operation.
///
/// Dimensions and the resolved canvas must already reflect the family's root policy. The root,
/// optional metadata, and optional canvas are each written here once. The result is not a
/// standalone portability or native export receipt. The caller still owns terminal admission.
pub fn write_svg(
    scene: &PaintScene<'_>,
    diagram_id: SvgId<'_>,
    meter: &OperationWorkMeter,
) -> Result<String> {
    let mut writer = SceneSvgWriter {
        out: BoundedSvgOutput::new(meter),
        meter,
        scope: diagram_id.semantic_str(),
        marker_count: scene.markers.len(),
        elements: 0,
    };
    writer.out.observe_operation()?;
    writer.root(scene)?;
    writer.out.finish()
}

struct SceneSvgWriter<'a, 'id> {
    out: BoundedSvgOutput<'a>,
    meter: &'a OperationWorkMeter,
    scope: &'id str,
    marker_count: usize,
    elements: usize,
}

impl SceneSvgWriter<'_, '_> {
    fn output_error(&mut self) -> Error {
        self.out
            .checkpoint()
            .err()
            .unwrap_or_else(|| invalid("formatting failed"))
    }

    fn emit(&mut self, arguments: fmt::Arguments<'_>) -> Result<()> {
        self.out
            .write_fmt(arguments)
            .map_err(|_| self.output_error())
    }

    fn elements(&mut self, count: usize) -> Result<()> {
        self.meter.charge_emit_work(count)?;
        self.elements = self
            .elements
            .checked_add(count)
            .ok_or_else(|| invalid("element count overflow"))?;
        self.meter
            .preflight_svg_element_count(self.elements, OperationPhase::Emit)?;
        Ok(())
    }

    fn text_content(&mut self, text: &str) -> Result<()> {
        self.meter.charge_emit_work(text.len().div_ceil(64))?;
        let written = XmlTextOutput::new(&mut self.out).write_str(text);
        written.map_err(|_| self.output_error())
    }

    fn root(&mut self, scene: &PaintScene<'_>) -> Result<()> {
        let viewport = scene.viewport;
        let scope = self.scope;
        let view_box = ViewBox::new(viewport.view_box)?;
        let [width, height] = positive_size(viewport.width, viewport.height)?;
        self.elements(1)?;
        self.emit(format_args!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" id="{scope}" viewBox="{view_box}" width="{width}" height="{height}""#
        ))?;
        if scene.metadata.title.is_some() {
            self.emit(format_args!(r#" aria-labelledby="{scope}-scene-title""#))?;
        }
        if scene.metadata.description.is_some() {
            self.emit(format_args!(
                r#" aria-describedby="{scope}-scene-description""#
            ))?;
        }
        self.emit(format_args!(">"))?;
        if let Some(title) = scene.metadata.title {
            self.elements(1)?;
            self.emit(format_args!(r#"<title id="{scope}-scene-title">"#))?;
            self.text_content(title)?;
            self.emit(format_args!("</title>"))?;
        }
        if let Some(description) = scene.metadata.description {
            self.elements(1)?;
            self.emit(format_args!(r#"<desc id="{scope}-scene-description">"#))?;
            self.text_content(description)?;
            self.emit(format_args!("</desc>"))?;
        }
        if let Some(canvas) = viewport.canvas {
            let fill = checked_color(canvas)?;
            let rect = RectAttributes(viewport.view_box);
            self.elements(1)?;
            self.emit(format_args!(
                r#"<rect {rect} aria-hidden="true" pointer-events="none" data-scene-role="canvas""#
            ))?;
            self.color("fill", fill)?;
            self.emit(format_args!("/>"))?;
        }
        if !scene.markers.is_empty() {
            self.elements(1)?;
            self.emit(format_args!("<defs>"))?;
            for (index, marker) in scene.markers.iter().enumerate() {
                self.marker(index, *marker)?;
            }
            self.emit(format_args!("</defs>"))?;
        }
        for (index, group) in scene.groups.iter().enumerate() {
            self.group(index, *group)?;
        }
        self.emit(format_args!("</svg>"))
    }

    fn group(&mut self, index: usize, group: SceneGroup<'_>) -> Result<()> {
        if group.items.is_empty() {
            self.meter.charge_emit_work(1)?;
            return Ok(());
        }
        self.elements(1)?;
        let scope = self.scope;
        let role = group.role.as_str();
        self.emit(format_args!(r#"<g data-scene-role="{role}""#))?;
        if let Some(id) = group.id {
            let id = id.semantic_str();
            self.emit(format_args!(r#" id="{scope}-scene-group-{index}-{id}""#))?;
        }
        self.emit(format_args!(">"))?;
        for item in group.items {
            self.elements(1)?;
            match *item {
                SceneItem::Rect(rect) => self.rect(rect)?,
                SceneItem::Path(path) => self.path(path)?,
                SceneItem::Text(text) => self.text(text)?,
            }
        }
        self.emit(format_args!("</g>"))
    }

    fn rect(&mut self, rect: SceneRect) -> Result<()> {
        validate_box(rect.bounds)?;
        nonnegative(rect.radius)?;
        let bounds = RectAttributes(rect.bounds);
        self.emit(format_args!("<rect {bounds}"))?;
        if rect.radius > 0.0 {
            let radius = canonical_number(rect.radius);
            self.emit(format_args!(r#" rx="{radius}" ry="{radius}""#))?;
        }
        self.paint(rect.paint)?;
        self.emit(format_args!("/>"))
    }

    fn path(&mut self, path: ScenePath<'_>) -> Result<()> {
        self.path_open(path.commands)?;
        self.paint(path.paint)?;
        let scope = self.scope;
        for (attribute, reference) in [
            ("marker-start", path.start_marker),
            ("marker-end", path.end_marker),
        ] {
            if let Some(SceneMarkerRef(index)) = reference {
                if index >= self.marker_count {
                    return Err(invalid("path refers to an absent marker"));
                }
                self.emit(format_args!(
                    r##" {attribute}="url(#{scope}-scene-marker-{index})""##
                ))?;
            }
        }
        self.emit(format_args!("/>"))
    }

    fn marker(&mut self, index: usize, marker: SceneMarker<'_>) -> Result<()> {
        self.elements(2)?;
        let scope = self.scope;
        let [width, height] = positive_size(marker.width, marker.height)?;
        let [reference_x, reference_y] = point(marker.reference)?;
        let units = match marker.units {
            SceneMarkerUnits::UserSpaceOnUse => "userSpaceOnUse",
            SceneMarkerUnits::StrokeWidth => "strokeWidth",
        };
        let orient = match marker.orient {
            SceneMarkerOrient::Auto => "auto",
            SceneMarkerOrient::AutoStartReverse => "auto-start-reverse",
        };
        self.emit(format_args!(
            r#"<marker id="{scope}-scene-marker-{index}" markerWidth="{width}" markerHeight="{height}" refX="{reference_x}" refY="{reference_y}" markerUnits="{units}" orient="{orient}""#
        ))?;
        if let Some(view_box) = marker.view_box {
            let view_box = ViewBox::new(view_box)?;
            self.emit(format_args!(r#" viewBox="{view_box}""#))?;
        }
        self.emit(format_args!(">"))?;
        self.path_open(marker.commands)?;
        self.paint(marker.paint)?;
        self.emit(format_args!("/></marker>"))
    }

    fn path_open(&mut self, commands: &[ScenePathCommand]) -> Result<()> {
        self.meter.charge_emit_work(commands.len())?;
        if !matches!(commands.first(), Some(ScenePathCommand::MoveTo(_))) {
            return Err(invalid("path must start with MoveTo"));
        }
        self.emit(format_args!(r#"<path d=""#))?;
        for (index, command) in commands.iter().enumerate() {
            if index.is_multiple_of(64) {
                self.out.observe_operation()?;
            }
            let separator = if index == 0 { "" } else { " " };
            match *command {
                ScenePathCommand::MoveTo(position) => {
                    let [x, y] = point(position)?;
                    self.emit(format_args!("{separator}M{x} {y}"))?;
                }
                ScenePathCommand::LineTo(position) => {
                    let [x, y] = point(position)?;
                    self.emit(format_args!("{separator}L{x} {y}"))?;
                }
                ScenePathCommand::CubicTo {
                    control1,
                    control2,
                    end,
                } => {
                    let [x1, y1] = point(control1)?;
                    let [x2, y2] = point(control2)?;
                    let [x, y] = point(end)?;
                    self.emit(format_args!("{separator}C{x1} {y1} {x2} {y2} {x} {y}"))?;
                }
                ScenePathCommand::Close => self.emit(format_args!("{separator}Z"))?,
            }
        }
        self.emit(format_args!(r#"""#))
    }

    fn paint(&mut self, paint: ScenePaint) -> Result<()> {
        if let Some(fill) = paint.fill {
            self.color("fill", checked_color(fill)?)?;
        } else {
            self.emit(format_args!(r#" fill="none""#))?;
        }
        if let Some(stroke) = paint.stroke {
            nonnegative(stroke.width)?;
            self.color("stroke", checked_color(stroke.color)?)?;
            let width = canonical_number(stroke.width);
            self.emit(format_args!(r#" stroke-width="{width}""#))?;
            if let Some([on, off]) = stroke.dash {
                nonnegative(on)?;
                nonnegative(off)?;
                let [on, off] = [on, off].map(canonical_number);
                self.emit(format_args!(r#" stroke-dasharray="{on} {off}""#))?;
            }
        }
        Ok(())
    }

    fn color(&mut self, attribute: &'static str, color: SceneColor) -> Result<()> {
        let rgb = [color.red, color.green, color.blue];
        if rgb.iter().all(|channel| channel.fract() == 0.0) {
            let [red, green, blue] = rgb.map(|channel| channel as u8);
            self.emit(format_args!(
                r##" {attribute}="#{red:02X}{green:02X}{blue:02X}""##
            ))?;
        } else {
            let [red, green, blue] = rgb.map(canonical_number);
            self.emit(format_args!(r#" {attribute}="rgb({red},{green},{blue})""#))?;
        }
        if color.alpha != 1.0 {
            let alpha = canonical_number(color.alpha);
            self.emit(format_args!(r#" {attribute}-opacity="{alpha}""#))?;
        }
        Ok(())
    }

    fn text(&mut self, text: SceneText<'_>) -> Result<()> {
        let [x, y] = point(text.position)?;
        let dy = finite(text.dy)?;
        let size = positive(text.style.size)?;
        let families = FontFamilies::new(text.style.families)?;
        let families = escape_attr_display(families);
        let anchor = match text.anchor {
            SceneTextAnchor::Start => "start",
            SceneTextAnchor::Middle => "middle",
            SceneTextAnchor::End => "end",
        };
        let baseline = match text.baseline {
            SceneTextBaseline::Alphabetic => "alphabetic",
            SceneTextBaseline::Central => "central",
            SceneTextBaseline::Middle => "middle",
        };
        let style = text.style.style.id();
        self.emit(format_args!(
            r#"<text x="{x}" y="{y}" dy="{dy}" text-anchor="{anchor}" dominant-baseline="{baseline}" alignment-baseline="{baseline}" font-family="{families}" font-size="{size}px" font-style="{style}""#
        ))?;
        if let Some(weight) = text.style.weight {
            if !(1..=1000).contains(&weight) {
                return Err(invalid("invalid resolved font weight"));
            }
            self.emit(format_args!(r#" font-weight="{weight}""#))?;
        }
        self.color("fill", checked_color(text.style.fill)?)?;
        self.emit(format_args!(">"))?;
        self.text_content(text.text)?;
        self.emit(format_args!("</text>"))
    }
}

fn invalid(message: &'static str) -> Error {
    Error::InvalidModel {
        message: format!("compact paint scene: {message}"),
    }
}

fn checked_color(color: SceneColor) -> Result<SceneColor> {
    if [color.red, color.green, color.blue]
        .into_iter()
        .all(|channel| (0.0..=255.0).contains(&channel))
        && (0.0..=1.0).contains(&color.alpha)
    {
        Ok(color)
    } else {
        Err(invalid("invalid resolved color"))
    }
}

fn finite(value: f64) -> Result<CanonicalNumber> {
    if value.is_finite() {
        Ok(canonical_number(value))
    } else {
        Err(invalid("non-finite coordinate"))
    }
}

fn nonnegative(value: f64) -> Result<()> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(invalid("negative or non-finite length"))
    }
}

fn positive(value: f64) -> Result<CanonicalNumber> {
    if value.is_finite() && canonicalize_number(value) > 0.0 {
        Ok(canonical_number(value))
    } else {
        Err(invalid("nonpositive or non-finite dimension"))
    }
}

fn point([x, y]: [f64; 2]) -> Result<[CanonicalNumber; 2]> {
    Ok([finite(x)?, finite(y)?])
}

fn positive_size(width: f64, height: f64) -> Result<[CanonicalNumber; 2]> {
    Ok([positive(width)?, positive(height)?])
}

fn validate_box(bounds: SceneBox) -> Result<()> {
    nonnegative(bounds.width)?;
    nonnegative(bounds.height)?;
    finite(bounds.x + bounds.width)?;
    finite(bounds.y + bounds.height)?;
    Ok(())
}

struct ViewBox([CanonicalNumber; 4]);

impl ViewBox {
    fn new(bounds: SceneBox) -> Result<Self> {
        validate_box(bounds)?;
        let [width, height] = positive_size(bounds.width, bounds.height)?;
        Ok(Self([finite(bounds.x)?, finite(bounds.y)?, width, height]))
    }
}

impl fmt::Display for ViewBox {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [x, y, width, height] = self.0;
        write!(out, "{x} {y} {width} {height}")
    }
}

struct RectAttributes(SceneBox);

impl fmt::Display for RectAttributes {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let SceneBox {
            x,
            y,
            width,
            height,
        } = self.0;
        let [x, y, width, height] = [x, y, width, height].map(canonical_number);
        write!(out, r#"x="{x}" y="{y}" width="{width}" height="{height}""#)
    }
}

struct FontFamilies<'a>(&'a [SceneFontFamily<'a>]);

impl<'a> FontFamilies<'a> {
    fn new(families: &'a [SceneFontFamily<'a>]) -> Result<Self> {
        if families.is_empty()
            || families.len() > 32
            || families.iter().any(|family| match family {
                SceneFontFamily::Generic(_) => false,
                SceneFontFamily::Named(name) => {
                    name.len() > 256
                        || name.trim().is_empty()
                        || name
                            .chars()
                            .any(|ch| ch.is_control() || !is_xml_1_0_char(ch))
                }
            })
        {
            return Err(invalid("invalid resolved font stack"));
        }
        Ok(Self(families))
    }
}

impl fmt::Display for FontFamilies<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, family) in self.0.iter().enumerate() {
            if index > 0 {
                out.write_str(", ")?;
            }
            match family {
                SceneFontFamily::Generic(family) => out.write_str(family.id())?,
                SceneFontFamily::Named(name) => cssparser::serialize_string(name, out)?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
