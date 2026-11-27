use super::super::labels::{
    SvgLine, architecture_svg_text_terminal_bounds, svg_lines_have_visible_text,
    svg_service_title_bounds, wrap_svg_words_to_lines,
};
use crate::Result;
use crate::graph_layout::ir::{Point, Size};
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{Bounds, LayoutLabel};
use crate::text::{TextMeasurer, TextStyle};

pub(super) const TEXT_CLEARANCE: f64 = 6.0;

#[derive(Debug)]
pub(crate) struct ArchitectureShoalText {
    pub(crate) lines: Vec<SvgLine>,
    pub(crate) bounds: Bounds,
}

impl ArchitectureShoalText {
    pub(super) fn centered(
        source: Option<&str>,
        wrap: f64,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Option<Self>> {
        Self::measure(source, wrap, measurer, style, work, |lines| {
            svg_service_title_bounds(lines, measurer, style)
        })
    }

    pub(super) fn start(
        source: Option<&str>,
        wrap: f64,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Option<Self>> {
        Self::measure(source, wrap, measurer, style, work, |lines| {
            svg_lines_have_visible_text(lines, style.font_size)
                .then(|| architecture_svg_text_terminal_bounds(lines, 0.0, 0.0, measurer, style))
        })
    }

    fn measure(
        source: Option<&str>,
        wrap: f64,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        work: &mut OperationLayoutWorkControl,
        ink_bounds: impl FnOnce(&[SvgLine]) -> Option<Bounds>,
    ) -> Result<Option<Self>> {
        let Some(source) = source.map(str::trim).filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        work.charge_adapter(source.len())?;
        let lines = wrap_svg_words_to_lines(source, wrap, measurer, style);
        Ok(ink_bounds(&lines).map(|bounds| Self { lines, bounds }))
    }

    pub(super) fn size(&self) -> Size {
        Size {
            width: self.bounds.max_x - self.bounds.min_x,
            height: self.bounds.max_y - self.bounds.min_y,
        }
    }

    pub(crate) fn bounds_at(&self, anchor: Point) -> Bounds {
        Bounds {
            min_x: anchor.x + self.bounds.min_x,
            max_x: anchor.x + self.bounds.max_x,
            min_y: anchor.y + self.bounds.min_y,
            max_y: anchor.y + self.bounds.max_y,
        }
    }

    pub(crate) fn centered_at(&self, label: &LayoutLabel) -> Point {
        Point {
            x: label.x - (self.bounds.min_x + self.bounds.max_x) * 0.5,
            y: label.y - (self.bounds.min_y + self.bounds.max_y) * 0.5,
        }
    }

    pub(super) fn checked_retained_bytes(
        &self,
        work: &OperationLayoutWorkControl,
    ) -> Result<usize> {
        let mut bytes = work.checked_mul(self.lines.capacity(), size_of::<SvgLine>())?;
        for line in &self.lines {
            bytes = work.checked_add(
                bytes,
                work.checked_mul(line.capacity(), size_of::<super::super::labels::SvgWord>())?,
            )?;
            for word in line {
                bytes = work.checked_add(bytes, word.content.capacity())?;
            }
        }
        Ok(bytes)
    }
}
