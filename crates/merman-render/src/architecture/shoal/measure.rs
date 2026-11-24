use super::super::labels::{
    SvgLine, architecture_svg_text_terminal_bounds, svg_lines_have_visible_text,
    svg_service_title_bounds, wrap_svg_words_to_lines,
};
use crate::Result;
use crate::graph_layout::ir::{Point, Size, SolvedEdge};
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

    pub(super) fn edge_space(&self, clearance: f64) -> Size {
        let ink = self.size();
        // Either axis may carry the straight segment near the label, particularly for a fixed
        // port on a perpendicular face. Reserve a full ink box on either side.
        Size {
            width: 2.0 * ink.width + 4.0 * clearance,
            height: 2.0 * ink.height + 4.0 * clearance,
        }
    }

    pub(super) fn place_edge(
        &self,
        edge: &mut SolvedEdge,
        clearance: f64,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<()> {
        let Some(label) = &mut edge.label else {
            return Ok(());
        };
        let ink = self.size();
        let dx = ink.width * 0.5 + clearance;
        let dy = ink.height * 0.5 + clearance;
        let fits = |x, y| {
            let guard = kurbo::Rect::from_center_size((x, y), (ink.width, ink.height))
                .inflate(clearance, clearance);
            !edge.points.windows(2).any(|leg| {
                let point = |index: usize| Point {
                    x: leg[index].x,
                    y: leg[index].y,
                };
                enters(guard, point(0), point(1))
            })
        };
        for (x, y) in [
            (0.0, 0.0),
            (0.0, dy),
            (dx, 0.0),
            (0.0, -dy),
            (-dx, 0.0),
            (dx, -dy),
            (-dx, -dy),
            (dx, dy),
            (-dx, dy),
        ] {
            work.charge_adapter(edge.points.len())?;
            if fits(label.x + x, label.y + y) {
                label.x += x;
                label.y += y;
                break;
            }
        }
        label.width = ink.width;
        label.height = ink.height;
        Ok(())
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

pub(super) fn enters(rect: kurbo::Rect, from: Point, to: Point) -> bool {
    let mut low = 0.0_f64;
    let mut high = 1.0_f64;
    for (start, end, min, max) in [
        (from.x, to.x, rect.x0, rect.x1),
        (from.y, to.y, rect.y0, rect.y1),
    ] {
        if start == end {
            if start <= min || start >= max {
                return false;
            }
        } else {
            let a = (min - start) / (end - start);
            let b = (max - start) / (end - start);
            low = low.max(a.min(b));
            high = high.min(a.max(b));
            if low >= high {
                return false;
            }
        }
    }
    true
}
