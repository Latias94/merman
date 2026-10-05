//! ELK Box's SIMPLE packing mode, the mode reachable through Mermaid 12.
//!
//! Ported from Eclipse ELK 0.9.1, EPL-2.0, `BoxLayoutProvider.sort/placeBoxes`:
//! https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.core/src/org/eclipse/elk/core/util/BoxLayoutProvider.java
//!
//! Rectangles arrive after node measurement and effective minimum-size resolution. The algorithm
//! does not route edges. Container expansion returns content translations separately: callers must
//! apply them to the expanded child's contents, not its position or ports. Grouped packing modes
//! are not exposed by Mermaid's ELK adapter and are deliberately not represented as supported modes.

use crate::work::{WorkControl, WorkError};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContentAlignment {
    #[default]
    Start,
    Center,
    End,
}

/// A measured child. Input order breaks equal-priority/equal-area ties.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rectangle {
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
    pub priority: i32,
    pub horizontal_content_alignment: ContentAlignment,
    pub vertical_content_alignment: ContentAlignment,
    /// Active child sizing constraints, after resolving their effective minimum. `None`
    /// means the child provider has already fixed its size through `ElkUtil.resizeNode`.
    pub minimum_size: Option<crate::LSize>,
    /// Size to restore during Rectpacking's second micro layout. The Box branch clears
    /// constraints instead, so Box itself ignores this value.
    pub micro_layout_size: Option<crate::LSize>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Padding {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Default for Padding {
    fn default() -> Self {
        Self {
            top: 15.0,
            right: 15.0,
            bottom: 15.0,
            left: 15.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    pub spacing: f64,
    pub padding: Padding,
    /// Nonpositive values use the source fallback of 1.3.
    pub aspect_ratio: f64,
    pub expand_nodes: bool,
    pub interactive: bool,
    /// Effective parent minimum; pass zero when MINIMUM_SIZE is not enabled.
    pub minimum_width: f64,
    pub minimum_height: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            spacing: 15.0,
            padding: Padding::default(),
            aspect_ratio: 1.3,
            expand_nodes: false,
            interactive: false,
            minimum_width: 0.0,
            minimum_height: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Placement {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub content_shift_x: f64,
    pub content_shift_y: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    /// Placements retain the input order, regardless of packing order.
    pub rectangles: Vec<Placement>,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
    #[error(transparent)]
    Work(#[from] WorkError),
    #[error("ELK Box received invalid {field} on rectangle {index}")]
    InvalidRectangle { index: usize, field: &'static str },
    #[error("ELK Box received invalid option {0}")]
    InvalidOption(&'static str),
    #[error("ELK Box geometry exceeded the finite numeric range")]
    NonFiniteGeometry,
}

/// Packs measured rectangles without mutating caller-owned geometry on interruption or failure.
/// No random seed is consumed: ELK Box is deterministic and does not use ELK's random generator.
pub fn layout(
    rectangles: &[Rectangle],
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<Layout, Error> {
    charge(work, 1)?;
    validate_options(options)?;
    for (index, rectangle) in rectangles.iter().enumerate() {
        charge(work, 1)?;
        for (field, value) in [("width", rectangle.width), ("height", rectangle.height)] {
            if !value.is_finite() || value < 0.0 {
                return Err(Error::InvalidRectangle { index, field });
            }
        }
        for (field, value) in [("x", rectangle.x), ("y", rectangle.y)] {
            if !value.is_finite() {
                return Err(Error::InvalidRectangle { index, field });
            }
        }
        if let Some(minimum) = rectangle.minimum_size {
            for (field, value) in [
                ("minimum_width", minimum.width),
                ("minimum_height", minimum.height),
            ] {
                if !value.is_finite() || value < 0.0 {
                    return Err(Error::InvalidRectangle { index, field });
                }
            }
        }
    }
    // ELK's recursive engine bypasses the layout provider for an empty node, even with padding
    // or MINIMUM_SIZE. This is the elkjs entry-point result, not placeBoxes' internal empty case.
    if rectangles.is_empty() {
        return Ok(Layout::default());
    }
    let order = sort(rectangles, options.interactive, work)?;
    // Box sorts using incoming dimensions, then ElkUtil.resizeNode resets each unfixed child
    // to its effective minimum. In particular, Radial leaves these constraints active.
    let mut resized;
    let rectangles = if rectangles.iter().any(|node| node.minimum_size.is_some()) {
        work.check(rectangles.len())?;
        resized = rectangles.to_vec();
        for node in &mut resized {
            charge(work, 1)?;
            if let Some(minimum) = node.minimum_size {
                node.width = minimum.width;
                node.height = minimum.height;
            }
        }
        &resized[..]
    } else {
        rectangles
    };
    let mut max_row_width: f64 = 0.0;
    let mut total_area = 0.0;
    for &index in &order {
        charge(work, 1)?;
        let rectangle = rectangles[index];
        max_row_width = max_row_width.max(rectangle.width);
        total_area += rectangle.width * rectangle.height;
    }
    let mean = total_area / rectangles.len() as f64;
    let mut variance = 0.0;
    for &index in &order {
        charge(work, 1)?;
        let rectangle = rectangles[index];
        variance += (rectangle.width * rectangle.height - mean).powi(2);
    }
    if !total_area.is_finite() || !variance.is_finite() {
        return Err(Error::NonFiniteGeometry);
    }
    let aspect_ratio = if options.aspect_ratio <= 0.0 {
        1.3
    } else {
        options.aspect_ratio
    };
    // Source sample variance is NaN for one rectangle. It suppresses wrapping there; explicitly
    // handle that case rather than depending on Rust/Java's different NaN max semantics.
    if rectangles.len() > 1 {
        total_area += rectangles.len() as f64 * (variance / (rectangles.len() - 1) as f64).sqrt();
        let target_area = total_area * aspect_ratio;
        if !target_area.is_finite() {
            return Err(Error::NonFiniteGeometry);
        }
        max_row_width = max_row_width.max(target_area.sqrt());
    }
    max_row_width += options.padding.left;
    work.check(rectangles.len())?;
    let mut result = Layout {
        rectangles: vec![Placement::default(); rectangles.len()],
        ..Layout::default()
    };
    let mut x = options.padding.left;
    let mut y = options.padding.top;
    let mut highest: f64 = 0.0;
    let mut broadest = options.padding.left + options.padding.right;
    let mut row_starts = vec![0];
    let mut row_heights = Vec::new();
    for (position, &index) in order.iter().enumerate() {
        charge(work, 1)?;
        let rectangle = rectangles[index];
        if rectangles.len() > 1 && x + rectangle.width > max_row_width {
            if options.expand_nodes {
                row_starts.push(position);
                row_heights.push(highest);
            }
            x = options.padding.left;
            y += highest + options.spacing;
            highest = 0.0;
            broadest = broadest.max(options.padding.left + options.padding.right + rectangle.width);
        }
        result.rectangles[index] = Placement {
            x,
            y,
            width: rectangle.width,
            height: rectangle.height,
            ..Placement::default()
        };
        broadest = broadest.max(x + rectangle.width + options.padding.right);
        highest = highest.max(rectangle.height);
        x += rectangle.width + options.spacing;
    }
    result.width = broadest.max(options.minimum_width);
    result.height = y + highest + options.padding.bottom;
    if result.height < options.minimum_height {
        highest += options.minimum_height - result.height;
        result.height = options.minimum_height;
    }
    if options.expand_nodes {
        row_starts.push(order.len());
        row_heights.push(highest);
        for (row, &height) in row_heights.iter().enumerate() {
            charge(work, 1)?;
            x = options.padding.left;
            for (offset, &index) in order[row_starts[row]..row_starts[row + 1]]
                .iter()
                .enumerate()
            {
                charge(work, 1)?;
                let placement = &mut result.rectangles[index];
                placement.height = height;
                // Upstream translates contents only for the last box in a row, including its
                // vertical change. Earlier boxes grow vertically without translating contents.
                if row_starts[row] + offset + 1 == row_starts[row + 1] {
                    placement.width = result.width - x - options.padding.right;
                    let rectangle = rectangles[index];
                    placement.content_shift_x = alignment_shift(
                        placement.width - rectangle.width,
                        rectangle.horizontal_content_alignment,
                    );
                    placement.content_shift_y = alignment_shift(
                        height - rectangle.height,
                        rectangle.vertical_content_alignment,
                    );
                }
                x += placement.width + options.spacing;
            }
        }
    }
    if !result.width.is_finite() || !result.height.is_finite() {
        return Err(Error::NonFiniteGeometry);
    }
    for placement in &result.rectangles {
        charge(work, 1)?;
        if ![
            placement.x,
            placement.y,
            placement.width,
            placement.height,
            placement.content_shift_x,
            placement.content_shift_y,
        ]
        .into_iter()
        .all(f64::is_finite)
        {
            return Err(Error::NonFiniteGeometry);
        }
    }
    Ok(result)
}

fn validate_options(options: &Options) -> Result<(), Error> {
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
    if !options.aspect_ratio.is_finite() {
        return Err(Error::InvalidOption("aspect_ratio"));
    }
    Ok(())
}

fn alignment_shift(growth: f64, alignment: ContentAlignment) -> f64 {
    let growth = growth.max(0.0);
    match alignment {
        ContentAlignment::Start => 0.0,
        ContentAlignment::Center => growth / 2.0,
        ContentAlignment::End => growth,
    }
}

fn charge(work: &mut dyn WorkControl, units: usize) -> Result<(), WorkError> {
    work.check(units)?;
    work.charge(units)
}

/// Stable merge sort preserves Java Collections.sort ties while allowing cancellation within
/// the comparison loop instead of admitting one uninterruptible input-sized sorting operation.
fn sort(
    rectangles: &[Rectangle],
    interactive: bool,
    work: &mut dyn WorkControl,
) -> Result<Vec<usize>, Error> {
    work.check(rectangles.len())?;
    let mut order: Vec<_> = (0..rectangles.len()).collect();
    let mut scratch = vec![0; rectangles.len()];
    let mut width = 1;
    while width < order.len() {
        let mut start = 0;
        while start < order.len() {
            let middle = start.saturating_add(width).min(order.len());
            let end = middle.saturating_add(width).min(order.len());
            let (mut left, mut right) = (start, middle);
            for slot in &mut scratch[start..end] {
                charge(work, 1)?;
                let take_left = right == end
                    || (left < middle && {
                        let a = rectangles[order[left]];
                        let b = rectangles[order[right]];
                        let mut comparison = b.priority.cmp(&a.priority);
                        if interactive {
                            comparison = comparison
                                .then_with(|| a.y.total_cmp(&b.y))
                                .then_with(|| a.x.total_cmp(&b.x));
                        }
                        comparison
                            .then_with(|| (a.width * a.height).total_cmp(&(b.width * b.height)))
                            .is_le()
                    });
                if take_left {
                    *slot = order[left];
                    left += 1;
                } else {
                    *slot = order[right];
                    right += 1;
                }
            }
            start = end;
        }
        std::mem::swap(&mut order, &mut scratch);
        width = width.saturating_mul(2);
    }
    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoopWorkControl;

    // Actual elkjs 0.9.3 output, using its standard layout entry point and `elk.algorithm=elk.box`.
    // Worker SHA-256: 90ab078ad34ff826ca0ece1ee338ae98071a6b3e6cfbb00ce2eb671b32eacd88.
    // Inputs below use fixed measured children, no edges, and only the specified layout options.
    fn rectangles(sizes: &[(f64, f64)]) -> Vec<Rectangle> {
        sizes
            .iter()
            .map(|&(width, height)| Rectangle {
                width,
                height,
                ..Rectangle::default()
            })
            .collect()
    }

    fn assert_oracle(
        input: &[Rectangle],
        options: Options,
        bounds: (f64, f64),
        expected: &[(f64, f64, f64, f64)],
    ) -> Layout {
        let result = layout(input, &options, &mut NoopWorkControl).unwrap();
        assert!((result.width - bounds.0).abs() < 1e-9, "width: {result:?}");
        assert!(
            (result.height - bounds.1).abs() < 1e-9,
            "height: {result:?}"
        );
        assert_eq!(result.rectangles.len(), expected.len());
        for (actual, &(x, y, width, height)) in result.rectangles.iter().zip(expected) {
            for (actual, expected) in [
                (actual.x, x),
                (actual.y, y),
                (actual.width, width),
                (actual.height, height),
            ] {
                assert!(
                    (actual - expected).abs() < 1e-9,
                    "actual {actual}, expected {expected}"
                );
            }
        }
        result
    }

    #[test]
    fn elkjs_empty_and_single() {
        assert_oracle(&[], Options::default(), (0.0, 0.0), &[]);
        assert_oracle(
            &[],
            Options {
                minimum_width: 200.0,
                minimum_height: 200.0,
                ..Options::default()
            },
            (0.0, 0.0),
            &[],
        );
        assert_oracle(
            &rectangles(&[(80.0, 40.0)]),
            Options::default(),
            (110.0, 70.0),
            &[(15.0, 15.0, 80.0, 40.0)],
        );
    }

    #[test]
    fn elkjs_varied_sizes_use_sample_deviation_row_width() {
        assert_oracle(
            &rectangles(&[
                (80.0, 20.0),
                (20.0, 80.0),
                (35.0, 30.0),
                (110.0, 15.0),
                (12.0, 12.0),
            ]),
            Options::default(),
            (140.0, 220.0),
            &[
                (15.0, 60.0, 80.0, 20.0),
                (15.0, 95.0, 20.0, 80.0),
                (42.0, 15.0, 35.0, 30.0),
                (15.0, 190.0, 110.0, 15.0),
                (15.0, 15.0, 12.0, 12.0),
            ],
        );
    }

    #[test]
    fn elkjs_equal_area_ties_retain_input_order() {
        assert_oracle(
            &rectangles(&[(40.0, 40.0), (80.0, 20.0), (20.0, 80.0), (40.0, 40.0)]),
            Options::default(),
            (110.0, 200.0),
            &[
                (15.0, 15.0, 40.0, 40.0),
                (15.0, 70.0, 80.0, 20.0),
                (15.0, 105.0, 20.0, 80.0),
                (50.0, 105.0, 40.0, 40.0),
            ],
        );
    }

    #[test]
    fn elkjs_priority_precedes_area() {
        let mut input = rectangles(&[(80.0, 20.0), (20.0, 80.0), (35.0, 30.0), (110.0, 15.0)]);
        for (rectangle, priority) in input.iter_mut().zip([0, 4, -2, 4]) {
            rectangle.priority = priority;
        }
        assert_oracle(
            &input,
            Options::default(),
            (140.0, 220.0),
            &[
                (15.0, 140.0, 80.0, 20.0),
                (15.0, 15.0, 20.0, 80.0),
                (15.0, 175.0, 35.0, 30.0),
                (15.0, 110.0, 110.0, 15.0),
            ],
        );
    }

    #[test]
    fn elkjs_interactive_sorts_by_y_then_x_before_area() {
        let mut input = rectangles(&[(80.0, 20.0), (20.0, 80.0), (35.0, 30.0), (110.0, 15.0)]);
        for (rectangle, (x, y)) in
            input
                .iter_mut()
                .zip([(100.0, 50.0), (40.0, 10.0), (10.0, 10.0), (0.0, 50.0)])
        {
            rectangle.x = x;
            rectangle.y = y;
        }
        assert_oracle(
            &input,
            Options {
                interactive: true,
                ..Options::default()
            },
            (140.0, 175.0),
            &[
                (15.0, 140.0, 80.0, 20.0),
                (65.0, 15.0, 20.0, 80.0),
                (15.0, 15.0, 35.0, 30.0),
                (15.0, 110.0, 110.0, 15.0),
            ],
        );
    }

    #[test]
    fn elkjs_container_expansion_and_title_padding() {
        let input = rectangles(&[(80.0, 20.0), (20.0, 80.0), (35.0, 30.0), (110.0, 15.0)]);
        assert_oracle(
            &input,
            Options {
                padding: Padding {
                    top: 48.0,
                    left: 24.0,
                    bottom: 24.0,
                    right: 24.0,
                },
                spacing: 50.0,
                aspect_ratio: 2.0,
                expand_nodes: true,
                minimum_width: 320.0,
                minimum_height: 280.0,
                ..Options::default()
            },
            (320.0, 367.0),
            &[
                (24.0, 128.0, 272.0, 20.0),
                (24.0, 198.0, 272.0, 80.0),
                (24.0, 48.0, 272.0, 30.0),
                (24.0, 328.0, 272.0, 15.0),
            ],
        );
    }

    #[test]
    fn elkjs_expansion_allocates_minimum_height_to_last_row() {
        assert_oracle(
            &rectangles(&[(40.0, 40.0), (40.0, 20.0), (20.0, 40.0)]),
            Options {
                aspect_ratio: 2.0,
                expand_nodes: true,
                minimum_width: 200.0,
                minimum_height: 200.0,
                ..Options::default()
            },
            (200.0, 200.0),
            &[
                (15.0, 70.0, 170.0, 115.0),
                (15.0, 15.0, 40.0, 40.0),
                (70.0, 15.0, 115.0, 40.0),
            ],
        );
    }

    #[test]
    fn elkjs_expansion_translates_contents_only_at_row_ends() {
        // The actual oracle gives each box one fixed child at (2, 3), with H_CENTER V_CENTER.
        // Child results are (67, 40.5), (2, 3), (49.5, 3): the middle box's height grows by 20
        // without translating its child, since it is not the last box in the first row.
        let mut input = rectangles(&[(40.0, 40.0), (40.0, 20.0), (20.0, 40.0)]);
        for rectangle in &mut input {
            rectangle.horizontal_content_alignment = ContentAlignment::Center;
            rectangle.vertical_content_alignment = ContentAlignment::Center;
        }
        let result = layout(
            &input,
            &Options {
                aspect_ratio: 2.0,
                expand_nodes: true,
                minimum_width: 200.0,
                minimum_height: 200.0,
                ..Options::default()
            },
            &mut NoopWorkControl,
        )
        .unwrap();
        let shifts: Vec<_> = result
            .rectangles
            .iter()
            .map(|p| (p.content_shift_x, p.content_shift_y))
            .collect();
        assert_eq!(shifts, [(65.0, 37.5), (0.0, 0.0), (47.5, 0.0)]);
    }

    #[test]
    fn elkjs_zero_dimensions_and_fractional_spacing() {
        assert_oracle(
            &rectangles(&[(0.0, 0.0), (0.0, 20.0), (20.0, 0.0)]),
            Options::default(),
            (50.0, 65.0),
            &[
                (15.0, 15.0, 0.0, 0.0),
                (30.0, 15.0, 0.0, 20.0),
                (15.0, 50.0, 20.0, 0.0),
            ],
        );
        // GWT's Java floatValue does not round to binary32; match elkjs, not native Java's cast.
        assert_oracle(
            &rectangles(&[(15.25, 12.125), (11.75, 33.625), (30.125, 15.25)]),
            Options {
                spacing: 0.123456789,
                ..Options::default()
            },
            (60.125, 78.998456789),
            &[
                (15.0, 15.0, 15.25, 12.125),
                (30.373456789000002, 15.0, 11.75, 33.625),
                (15.0, 48.748456789, 30.125, 15.25),
            ],
        );
    }

    #[test]
    fn failures_do_not_mutate_input_and_work_can_interrupt_every_phase() {
        struct Control {
            remaining: usize,
        }
        impl WorkControl for Control {
            fn check(&mut self, units: usize) -> Result<(), WorkError> {
                if units > self.remaining {
                    Err(WorkError::Interrupted)
                } else {
                    Ok(())
                }
            }
            fn charge(&mut self, units: usize) -> Result<(), WorkError> {
                self.check(units)?;
                self.remaining -= units;
                Ok(())
            }
        }
        let input = rectangles(&[(80.0, 20.0), (20.0, 80.0), (35.0, 30.0), (110.0, 15.0)]);
        let unchanged = input.clone();
        let options = Options {
            expand_nodes: true,
            ..Options::default()
        };
        let mut count = Control {
            remaining: usize::MAX,
        };
        let expected = layout(&input, &options, &mut count).unwrap();
        let cost = usize::MAX - count.remaining;
        for remaining in 0..cost {
            assert_eq!(
                layout(&input, &options, &mut Control { remaining }),
                Err(Error::Work(WorkError::Interrupted))
            );
        }
        assert_eq!(
            layout(&input, &options, &mut Control { remaining: cost }).unwrap(),
            expected
        );
        assert_eq!(input, unchanged);
    }

    #[test]
    fn invalid_and_overflowing_geometry_is_rejected() {
        for minimum in [-1.0, f64::NAN, f64::INFINITY] {
            let child = Rectangle {
                width: 40.0,
                height: 20.0,
                minimum_size: Some(crate::LSize {
                    width: minimum,
                    height: 20.0,
                }),
                ..Default::default()
            };
            assert!(matches!(
                layout(&[child], &Options::default(), &mut crate::NoopWorkControl),
                Err(Error::InvalidRectangle {
                    field: "minimum_width",
                    ..
                })
            ));
        }
        let input = rectangles(&[(-1.0, 20.0)]);
        assert_eq!(
            layout(&input, &Options::default(), &mut NoopWorkControl),
            Err(Error::InvalidRectangle {
                index: 0,
                field: "width"
            })
        );
        assert_eq!(
            layout(
                &rectangles(&[(f64::MAX, 2.0)]),
                &Options::default(),
                &mut NoopWorkControl
            ),
            Err(Error::NonFiniteGeometry)
        );
        assert_eq!(
            layout(
                &[],
                &Options {
                    spacing: f64::NAN,
                    ..Options::default()
                },
                &mut NoopWorkControl
            ),
            Err(Error::InvalidOption("spacing"))
        );
    }
}
