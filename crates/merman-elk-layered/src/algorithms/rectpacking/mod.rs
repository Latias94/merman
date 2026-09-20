//! ELK Rectangle Packing for measured children, including Mermaid 12's compaction preset.
//!
//! Modified Rust translation of Eclipse ELK 0.9.1, EPL-2.0:
//! `plugins/org.eclipse.elk.alg.rectpacking/src/org/eclipse/elk/alg/rectpacking/`
//! at commit `62d5909f96fad541bc101ad52dabaece6b7eab7e` (Kiel University, 2018–2022).
//! The provider does not route edges. Measurement, micro layout, hierarchy traversal, and
//! interpreting option strings belong to the caller. In elkjs 0.9.3 Mermaid's `SCANLINE` string
//! is not an enum member: it resolves to the GREEDY default implemented here.

mod approximation;
mod packing;
#[cfg(test)]
mod tests;

pub use super::box_layout::{ContentAlignment, Layout, Padding, Placement, Rectangle};
use crate::work::{WorkControl, WorkError};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    pub spacing: f64,
    pub padding: Padding,
    pub aspect_ratio: f64,
    pub try_box: bool,
    /// Used by the source provider's optional Box branch only.
    pub expand_nodes: bool,
    pub row_height_reevaluation: bool,
    pub compaction_iterations: u32,
    /// EQUAL_BETWEEN_STRUCTURES; the upstream provider default has no elimination phase.
    pub eliminate_whitespace: bool,
    /// Effective parent minimum, resolved before entering the provider.
    pub minimum_width: f64,
    pub minimum_height: f64,
    pub horizontal_content_alignment: ContentAlignment,
    pub vertical_content_alignment: ContentAlignment,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            spacing: 15.0,
            padding: Padding::default(),
            aspect_ratio: 1.3,
            try_box: false,
            expand_nodes: false,
            row_height_reevaluation: false,
            compaction_iterations: 1,
            eliminate_whitespace: false,
            minimum_width: 0.0,
            minimum_height: 0.0,
            horizontal_content_alignment: ContentAlignment::Start,
            vertical_content_alignment: ContentAlignment::Start,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
    #[error(transparent)]
    Work(#[from] WorkError),
    #[error(transparent)]
    Box(#[from] super::box_layout::Error),
    #[error("ELK Rectangle Packing received invalid {field} on rectangle {index}")]
    InvalidRectangle { index: usize, field: &'static str },
    #[error("ELK Rectangle Packing received invalid option {0}")]
    InvalidOption(&'static str),
    #[error("ELK Rectangle Packing geometry exceeded the finite numeric range")]
    NonFiniteGeometry,
    #[error("ELK Rectangle Packing cannot distinguish further numeric search steps")]
    NumericStagnation,
}

pub fn layout(
    rectangles: &[Rectangle],
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<Layout, Error> {
    charge(work, 1)?;
    for (name, value) in [
        ("spacing", options.spacing),
        ("padding.top", options.padding.top),
        ("padding.right", options.padding.right),
        ("padding.bottom", options.padding.bottom),
        ("padding.left", options.padding.left),
        ("minimum_width", options.minimum_width),
        ("minimum_height", options.minimum_height),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(Error::InvalidOption(name));
        }
    }
    if !options.aspect_ratio.is_finite() || options.aspect_ratio <= 0.0 {
        return Err(Error::InvalidOption("aspect_ratio"));
    }
    if options.compaction_iterations == 0 {
        return Err(Error::InvalidOption("compaction_iterations"));
    }
    for (index, rectangle) in rectangles.iter().enumerate() {
        charge(work, 1)?;
        for (field, value) in [("width", rectangle.width), ("height", rectangle.height)] {
            if !value.is_finite() || value < 0.0 {
                return Err(Error::InvalidRectangle { index, field });
            }
        }
    }
    // The recursive ELK engine bypasses providers for leaves.
    if rectangles.is_empty() {
        return Ok(Layout::default());
    }
    let mut stackable = !options.try_box || rectangles.len() < 3;
    if options.try_box && rectangles.len() >= 3 {
        for three in rectangles.windows(3) {
            charge(work, 1)?;
            if three[0].height >= three[1].height + three[2].height + options.spacing
                || three[2].height >= three[0].height + three[1].height + options.spacing
            {
                stackable = true;
                break;
            }
        }
    }
    if !stackable {
        // This is RectPackingLayoutProvider's explicit trybox branch, not an algorithm substitute.
        charge(work, rectangles.len())?;
        let priority_base =
            i32::try_from(rectangles.len()).map_err(|_| WorkError::ArithmeticOverflow)?;
        let nodes: Vec<_> = rectangles
            .iter()
            .enumerate()
            .map(|(index, rectangle)| Rectangle {
                priority: priority_base - index as i32,
                ..*rectangle
            })
            .collect();
        return Ok(super::box_layout::layout(
            &nodes,
            &super::box_layout::Options {
                spacing: options.spacing,
                padding: options.padding,
                aspect_ratio: options.aspect_ratio,
                expand_nodes: options.expand_nodes,
                minimum_width: options.minimum_width,
                minimum_height: options.minimum_height,
                ..Default::default()
            },
            work,
        )?);
    }
    let target_width =
        approximation::target_width(rectangles, options, work)?.max(options.minimum_width);
    let mut original = packing::pack(rectangles, options, target_width, work)?;
    let mut drawing_width = original.width;
    let mut drawing_height = original.height;
    let mut changes = original.changes;
    for _ in 1..options.compaction_iterations {
        charge(work, 1)?;
        let ratio = (drawing_width + options.padding.left + options.padding.right)
            / (drawing_height + options.padding.top + options.padding.bottom);
        let next_width = if rectangles.len() > 1
            && changes.increase_min != f64::INFINITY
            && ratio < options.aspect_ratio
        {
            target_width + changes.increase_min
        } else if rectangles.len() > 1
            && changes.decrease_min != f64::INFINITY
            && ratio > options.aspect_ratio
        {
            (target_width - changes.decrease_min).max(options.minimum_width)
        } else {
            target_width
        };
        let candidate = packing::pack(rectangles, options, next_width, work)?;
        if scale(candidate.width, candidate.height, options.aspect_ratio)
            >= scale(drawing_width, drawing_height, options.aspect_ratio)
        {
            // ELK 0.9.1 copies child positions, dimensions, and width-change metrics, but does not
            // replace the original ROWS or ADDITIONAL_HEIGHT properties. The expansion phase
            // therefore still operates on the original block membership and cached row sizes.
            original.nodes = candidate.nodes;
            changes = candidate.changes;
            drawing_width = candidate.width;
            drawing_height = candidate.height;
        }
    }
    if options.eliminate_whitespace {
        packing::expand(&mut original, drawing_width, options, work)?;
    }
    let real_width = original
        .nodes
        .iter()
        .map(|node| node.x + node.width)
        .fold(0.0_f64, f64::max);
    let real_height = original
        .nodes
        .iter()
        .map(|node| node.y + node.height)
        .fold(0.0_f64, f64::max);
    let dx = alignment_shift(
        drawing_width - real_width,
        options.horizontal_content_alignment,
    );
    let dy = alignment_shift(
        drawing_height - real_height,
        options.vertical_content_alignment,
    );
    for (node, rectangle) in original.nodes.iter_mut().zip(rectangles) {
        charge(work, 1)?;
        node.x += dx + options.padding.left;
        node.y += dy + options.padding.top;
        node.content_shift_x = alignment_shift(
            node.width - rectangle.width,
            rectangle.horizontal_content_alignment,
        );
        node.content_shift_y = alignment_shift(
            node.height - rectangle.height,
            rectangle.vertical_content_alignment,
        );
    }
    let result = Layout {
        rectangles: original.nodes,
        width: drawing_width + options.padding.left + options.padding.right,
        height: drawing_height + options.padding.top + options.padding.bottom,
    };
    if !result.width.is_finite()
        || !result.height.is_finite()
        || result.rectangles.iter().any(|r| {
            [
                r.x,
                r.y,
                r.width,
                r.height,
                r.content_shift_x,
                r.content_shift_y,
            ]
            .iter()
            .any(|v| !v.is_finite())
        })
    {
        return Err(Error::NonFiniteGeometry);
    }
    Ok(result)
}

fn alignment_shift(extra: f64, alignment: ContentAlignment) -> f64 {
    // ElkUtil.translate only moves child contents when a dimension grows.
    if extra <= 0.0 {
        return 0.0;
    }
    match alignment {
        ContentAlignment::Start => 0.0,
        ContentAlignment::Center => extra / 2.0,
        ContentAlignment::End => extra,
    }
}

fn scale(width: f64, height: f64, aspect: f64) -> f64 {
    if width > 0.0 && height > 0.0 {
        (aspect / width).min(1.0 / height)
    } else {
        0.0
    }
}

fn charge(work: &mut dyn WorkControl, units: usize) -> Result<(), Error> {
    work.check(units)?;
    work.charge(units)?;
    Ok(())
}
