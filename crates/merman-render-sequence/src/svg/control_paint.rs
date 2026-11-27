use super::prelude::*;
use merman_render_core::number_format::canonicalize_number;
use std::cell::RefCell;

pub(super) struct SequenceControlPaint {
    width: Option<f32>,
    is_keyword: bool,
    pub(super) bounds: RefCell<Option<Bounds>>,
}

impl SequenceControlPaint {
    pub(super) fn new(width: Option<f32>, is_keyword: bool) -> Self {
        Self {
            width,
            is_keyword,
            bounds: RefCell::new(None),
        }
    }

    pub(super) fn include_bounds(
        &self,
        out: &mut impl SvgOutput,
        coordinates: [f64; 4],
    ) -> Result<()> {
        let Some(width) = self.width else {
            return Ok(());
        };
        let mut total = self.bounds.borrow_mut();
        include_control_stroke_bounds(&mut total, coordinates, f64::from(width), self.is_keyword);
        out.checkpoint()
    }

    pub(super) fn write_attributes(&self, out: &mut impl SvgOutput, separator: bool) {
        let dash = if separator {
            "stroke-dasharray: 3, 3;"
        } else {
            ""
        };
        if let Some(width) = self.width {
            let _ = write!(
                out,
                r#" style="{dash}stroke-width:{}px;""#,
                fmt(f64::from(width))
            );
        } else if separator {
            out.push_str(r#" style="stroke-dasharray: 3, 3;""#);
        }
    }
}

pub(super) fn include_control_stroke_bounds(
    total: &mut Option<Bounds>,
    coordinates: [f64; 4],
    width: f64,
    is_keyword: bool,
) {
    let [x1, y1, x2, y2] = coordinates.map(canonicalize_number);
    // Keyword polygons can have acute joins. Separate frame lines have only stroke caps.
    let half = width / 2.0 * if is_keyword { 4.0 } else { 1.0 };
    let bounds = Bounds {
        min_x: x1.min(x2) - half,
        min_y: y1.min(y2) - half,
        max_x: x1.max(x2) + half,
        max_y: y1.max(y2) + half,
    };
    if let Some(total) = total {
        total.min_x = total.min_x.min(bounds.min_x);
        total.min_y = total.min_y.min(bounds.min_y);
        total.max_x = total.max_x.max(bounds.max_x);
        total.max_y = total.max_y.max(bounds.max_y);
    } else {
        *total = Some(bounds);
    }
}
