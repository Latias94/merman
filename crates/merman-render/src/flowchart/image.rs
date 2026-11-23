use kurbo::Size;

use crate::model::LayoutPoint;
use crate::text::TextMetrics;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ImageSquareGeometry {
    pub(crate) image: Size,
    pub(crate) label: Size,
    pub(crate) outer: Size,
    pub(crate) label_padding: f64,
    pub(crate) top_label: bool,
}

pub(crate) struct ImageSquareSizing<'a> {
    pub(crate) href: &'a str,
    pub(crate) width: Option<f64>,
    pub(crate) height: Option<f64>,
    pub(crate) constraint_on: bool,
}

impl ImageSquareSizing<'_> {
    fn resolve(self, min_width: f64) -> Size {
        if let Some(natural) = crate::raster_data_url::intrinsic_raster_size(self.href) {
            // Mermaid imageSquare.ts gives a nonzero height priority under the constraint.
            let raw_width = self.width.unwrap_or(natural.width).max(min_width);
            let ratio = natural.width / natural.height;
            if self.constraint_on {
                let width = self
                    .height
                    .filter(|height| *height != 0.0)
                    .map_or(raw_width, |height| height * ratio);
                Size::new(width.max(0.0), (width / ratio).max(0.0))
            } else {
                Size::new(raw_width, self.height.unwrap_or(natural.height).max(0.0))
            }
        } else {
            // Preserve the headless size of references without intrinsic metadata.
            let height = self.height.unwrap_or(60.0).max(1.0);
            let width = self.width.unwrap_or(height).max(1.0);
            let raw_width = width.max(min_width);
            match (self.constraint_on, self.height) {
                (true, Some(_)) => Size::new(width, height),
                (true, None) => Size::new(raw_width, raw_width / (width / height)),
                (false, _) => Size::new(raw_width, height),
            }
        }
    }
}

impl ImageSquareGeometry {
    pub(crate) fn resolve(
        sizing: ImageSquareSizing<'_>,
        metrics: TextMetrics,
        has_label: bool,
        wrapping_width: f64,
        top_label: bool,
        mut charge_work: impl FnMut(usize) -> crate::Result<()>,
    ) -> crate::Result<Self> {
        // URL decoding and header inspection are linear in the authored URL size. Pixel decode
        // stays in the export stage under its existing byte and pixel limits.
        charge_work(1 + sizing.href.len() / 64)?;
        let min_width = if has_label {
            wrapping_width.max(0.0)
        } else {
            0.0
        };
        let image = sizing.resolve(min_width);
        charge_work(0)?;

        // The image shape's .labelBkg gives the nested label paragraph 2px of padding.
        let (label, label_padding) = if has_label {
            (Size::new(metrics.width + 4.0, metrics.height + 4.0), 8.0)
        } else {
            (Size::ZERO, 0.0)
        };
        let outer = Size::new(
            image.width.max(label.width),
            image.height + label.height + label_padding,
        );
        if !outer.width.is_finite() || !outer.height.is_finite() {
            return Err(crate::Error::InvalidModel {
                message: "Flowchart image has non-finite dimensions".into(),
            });
        }
        Ok(Self {
            image,
            label,
            outer,
            label_padding,
            top_label,
        })
    }

    pub(crate) fn image_top(self) -> f64 {
        if self.top_label {
            self.outer.height / 2.0 - self.image.height
        } else {
            -self.outer.height / 2.0
        }
    }

    pub(crate) fn labeled_outline(self) -> Option<[LayoutPoint; 8]> {
        if self.label_padding == 0.0 {
            return None;
        }
        let image = self.image.width / 2.0;
        let label = self.label.width / 2.0;
        let bottom = self.outer.height / 2.0;
        let top = -bottom;
        let points = if self.top_label {
            let split = top + self.label.height + self.label_padding;
            [
                (-label, top),
                (label, top),
                (label, split),
                (image, split),
                (image, bottom),
                (-image, bottom),
                (-image, split),
                (-label, split),
            ]
        } else {
            let split = top + self.image.height;
            // imageSquare.ts tapers the lower right corner of the bottom label.
            [
                (-image, top),
                (image, top),
                (image, split),
                (label, split),
                (label / 2.0, bottom),
                (-label, bottom),
                (-label, split),
                (-image, split),
            ]
        };
        Some(points.map(|(x, y)| LayoutPoint { x, y }))
    }
}
