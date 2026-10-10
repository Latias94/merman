//! Flowchart-family edge-label content padding resolved before layout.

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct FlowchartEdgeLabelPadding {
    top: f64,
    right: f64,
    bottom: f64,
    left: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FlowchartEdgeLabelContentBox {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

impl FlowchartEdgeLabelPadding {
    pub(crate) fn from_insets(insets: crate::diagram_theme::InsetsPx) -> Self {
        Self {
            top: f64::from(insets.top),
            right: f64::from(insets.right),
            bottom: f64::from(insets.bottom),
            left: f64::from(insets.left),
        }
    }

    pub(crate) const fn is_zero(self) -> bool {
        self.top == 0.0 && self.right == 0.0 && self.bottom == 0.0 && self.left == 0.0
    }

    pub(crate) const fn top(self) -> f64 {
        self.top
    }

    pub(crate) const fn left(self) -> f64 {
        self.left
    }

    pub(crate) const fn horizontal(self) -> f64 {
        self.left + self.right
    }

    pub(crate) const fn vertical(self) -> f64 {
        self.top + self.bottom
    }

    pub(crate) fn padded_size(self, width: f64, height: f64) -> (f64, f64) {
        (
            width.max(0.0) + self.horizontal(),
            height.max(0.0) + self.vertical(),
        )
    }

    pub(crate) fn content_box(
        self,
        padded_width: f64,
        padded_height: f64,
    ) -> FlowchartEdgeLabelContentBox {
        let width = (padded_width.max(0.0) - self.horizontal()).max(0.0);
        let height = (padded_height.max(0.0) - self.vertical()).max(0.0);
        FlowchartEdgeLabelContentBox {
            x: -padded_width.max(0.0) / 2.0 + self.left,
            y: -padded_height.max(0.0) / 2.0 + self.top,
            width,
            height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asymmetric_padding_keeps_the_content_box_inside_the_padded_layout_box() {
        let padding = FlowchartEdgeLabelPadding::from_insets(crate::diagram_theme::InsetsPx {
            top: 3.0,
            right: 7.0,
            bottom: 5.0,
            left: 11.0,
        });

        let (width, height) = padding.padded_size(40.0, 20.0);
        assert_eq!((width, height), (58.0, 28.0));
        assert_eq!(
            padding.content_box(width, height),
            FlowchartEdgeLabelContentBox {
                x: -18.0,
                y: -11.0,
                width: 40.0,
                height: 20.0,
            }
        );
    }
}
