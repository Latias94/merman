#[cfg(test)]
use crate::RenderResourcePolicy;
use crate::Result;
use crate::model::{
    Bounds, LayoutPoint, RadarAxisLayout, RadarCurveLayout, RadarDiagramLayout,
    RadarGraticuleShapeLayout, RadarLegendItemLayout,
};
use crate::resources::OperationWorkMeter;
use crate::text::TextMeasurer;
use merman_model::OperationPhase;
use merman_model::families::radar::{RadarCurve, RadarDiagramModel};

mod axis_paint;
mod config;
mod css_binding;
mod paint;
pub(crate) mod point_list;
mod text_paint;
mod theme;

pub(crate) use axis_paint::{RadarAxisPaintPlan, RadarAxisPaintReceipt};
pub(crate) use config::RadarConfigView;
pub(crate) use css_binding::RadarCssBinding;
pub(crate) use text_paint::{RadarTextPaintPlan, RadarTextPaintReceipt};
pub(crate) use theme::{
    RadarSeriesPaintPlan, RadarTitleThemePlan, RadarTypographyCssEmission, RadarTypographyThemePlan,
};

pub(crate) fn effective_title<'a>(
    model: &'a RadarDiagramModel,
    diagram_title: Option<&'a str>,
) -> Option<&'a str> {
    model
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .or_else(|| {
            diagram_title
                .map(str::trim)
                .filter(|title| !title.is_empty())
        })
}

fn drawable_curves(model: &RadarDiagramModel) -> impl Iterator<Item = (usize, &RadarCurve)> {
    model
        .curves
        .iter()
        .enumerate()
        .filter(move |(_, curve)| curve.entries.len() == model.axes.len())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RadarLayoutWork {
    axes: usize,
    graticule_shapes: usize,
    graticule_points: usize,
    curve_scans: usize,
    curve_entry_scans: usize,
    curve_shapes: usize,
    curve_points: usize,
    legend_items: usize,
}

impl RadarLayoutWork {
    fn from_model(
        model: &RadarDiagramModel,
        ticks: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self> {
        let axes = model.axes.len();
        let curves = model.curves.len();
        let mut work = Self {
            axes,
            graticule_shapes: ticks,
            graticule_points: if model.options.graticule.trim() == "polygon" {
                ticks.saturating_mul(axes)
            } else {
                0
            },
            curve_scans: curves,
            curve_entry_scans: 0,
            curve_shapes: 0,
            curve_points: 0,
            legend_items: if model.options.show_legend { curves } else { 0 },
        };
        for curve in &model.curves {
            work_meter.checkpoint(OperationPhase::Layout)?;
            work.curve_entry_scans = work.curve_entry_scans.saturating_add(curve.entries.len());
            if curve.entries.len() == axes {
                work.curve_shapes = work.curve_shapes.saturating_add(1);
                work.curve_points = work.curve_points.saturating_add(axes);
            }
        }
        Ok(work)
    }

    fn total(self) -> usize {
        [
            self.axes,
            self.graticule_shapes,
            self.graticule_points,
            self.curve_scans,
            self.curve_entry_scans,
            self.curve_shapes,
            self.curve_points,
            self.legend_items,
        ]
        .into_iter()
        .fold(0usize, usize::saturating_add)
    }
}

struct RadarTicks {
    value: f64,
    iterations: usize,
}

impl RadarTicks {
    fn new(value: f64) -> Self {
        Self {
            value,
            // JavaScript's `i < ticks` performs no iterations for NaN or negatives.
            // Saturating positive infinity lets the work policy reject unbounded work
            // without changing canonical model state. Source ticks are already <= 32.
            iterations: if value > 0.0 {
                value.ceil() as usize
            } else {
                0
            },
        }
    }
}

fn fmt_number(v: f64) -> String {
    ryu_js::Buffer::new().format(v).to_owned()
}

pub(crate) fn format_points(points: &[LayoutPoint]) -> String {
    let mut out = String::new();
    let mut buffer = ryu_js::Buffer::new();
    for (index, point) in points.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        out.push_str(buffer.format(point.x));
        out.push(',');
        out.push_str(buffer.format(point.y));
    }
    out
}

fn relative_radius(value: f64, min: f64, max: f64, radius: f64) -> f64 {
    // radar/renderer.ts clips before subtracting, multiplies before dividing,
    // and lets Math.min/Math.max propagate NaN.
    if value.is_nan() || min.is_nan() || max.is_nan() {
        return f64::NAN;
    }
    let clipped = value.max(min).min(max);
    (radius * (clipped - min)) / (max - min)
}

fn polar_xy(radius: f64, angle: f64) -> LayoutPoint {
    LayoutPoint {
        x: radius * angle.cos(),
        y: radius * angle.sin(),
    }
}

fn closed_round_curve_path(points: &[LayoutPoint], tension: f64) -> String {
    if points.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let p0 = points[0].clone();
    out.push_str(&format!("M{},{}", fmt_number(p0.x), fmt_number(p0.y)));

    let n = points.len();
    for i in 0..n {
        let p0 = points[(i + n - 1) % n].clone();
        let p1 = points[i].clone();
        let p2 = points[(i + 1) % n].clone();
        let p3 = points[(i + 2) % n].clone();

        // Mermaid's radar renderer uses a simple Catmull-Rom conversion:
        // - `cp1 = p1 + (p2 - p0) * tension`
        // - `cp2 = p2 - (p3 - p1) * tension`
        let cp1 = LayoutPoint {
            x: p1.x + (p2.x - p0.x) * tension,
            y: p1.y + (p2.y - p0.y) * tension,
        };
        let cp2 = LayoutPoint {
            x: p2.x - (p3.x - p1.x) * tension,
            y: p2.y - (p3.y - p1.y) * tension,
        };

        out.push_str(&format!(
            " C{},{} {},{} {},{}",
            fmt_number(cp1.x),
            fmt_number(cp1.y),
            fmt_number(cp2.x),
            fmt_number(cp2.y),
            fmt_number(p2.x),
            fmt_number(p2.y)
        ));
    }
    out.push_str(" Z");
    out
}

#[cfg(test)]
pub(crate) fn layout_radar_diagram_typed(
    model: &RadarDiagramModel,
    effective_config: &serde_json::Value,
    measurer: &dyn TextMeasurer,
) -> Result<RadarDiagramLayout> {
    let work_meter = OperationWorkMeter::new(RenderResourcePolicy::interactive());
    layout_radar_diagram_typed_with_work_meter(model, effective_config, measurer, &work_meter)
}

/// Lays out a Radar model under the resource policy owned by the render operation.
#[cfg(test)]
pub(crate) fn layout_radar_diagram_typed_with_resource_policy(
    model: &RadarDiagramModel,
    effective_config: &serde_json::Value,
    _measurer: &dyn TextMeasurer,
    resource_limits: RenderResourcePolicy,
) -> Result<RadarDiagramLayout> {
    let work_meter = OperationWorkMeter::new(resource_limits);
    layout_radar_diagram_typed_with_work_meter(model, effective_config, _measurer, &work_meter)
}

/// Lays out a Radar model under the cumulative work meter owned by the render operation.
pub(crate) fn layout_radar_diagram_typed_with_work_meter(
    model: &RadarDiagramModel,
    effective_config: &serde_json::Value,
    _measurer: &dyn TextMeasurer,
    work_meter: &OperationWorkMeter,
) -> Result<RadarDiagramLayout> {
    work_meter.checkpoint(OperationPhase::Layout)?;
    work_meter
        .policy()
        .check_model_complexity(merman_model::resources::model_complexity_from_radar(model))?;
    let _ = (
        model.acc_title.as_deref(),
        model.acc_descr.as_deref(),
        model.title.as_deref(),
    );

    let cfg = RadarConfigView::new(effective_config).layout_settings();
    let width = cfg.width;
    let height = cfg.height;
    let margin_left = cfg.margin_left;
    let margin_right = cfg.margin_right;
    let margin_top = cfg.margin_top;
    let margin_bottom = cfg.margin_bottom;
    let axis_scale_factor = cfg.axis_scale_factor;
    let axis_label_factor = cfg.axis_label_factor;
    let curve_tension = cfg.curve_tension;

    let svg_width = width + margin_left + margin_right;
    let svg_height = height + margin_top + margin_bottom;

    let center_x = margin_left + width / 2.0;
    let center_y = margin_top + height / 2.0;
    let base_radius = (width / 2.0).min(height / 2.0);
    let radius = base_radius;

    let title_y = -center_y;

    let axis_count = model.axes.len();
    let ticks = RadarTicks::new(model.options.ticks);
    let layout_work = RadarLayoutWork::from_model(model, ticks.iterations, work_meter)?;
    work_meter.charge(layout_work.total())?;
    let mut axes: Vec<RadarAxisLayout> = Vec::new();
    if axis_count > 0 {
        for (i, axis) in model.axes.iter().enumerate() {
            work_meter.checkpoint(OperationPhase::Layout)?;
            let angle = -std::f64::consts::FRAC_PI_2
                + (i as f64) * (std::f64::consts::TAU / (axis_count as f64));
            let line = polar_xy(base_radius * axis_scale_factor, angle);
            let label = polar_xy(base_radius * axis_label_factor, angle);
            axes.push(RadarAxisLayout {
                label: axis.label.clone(),
                angle,
                line_x2: line.x,
                line_y2: line.y,
                label_x: label.x,
                label_y: label.y,
            });
        }
    }

    let mut graticules: Vec<RadarGraticuleShapeLayout> = Vec::new();
    if ticks.iterations > 0 {
        for t in 1..=ticks.iterations {
            work_meter.checkpoint(OperationPhase::Layout)?;
            let r = base_radius * (t as f64) / ticks.value;
            if model.options.graticule.trim() == "polygon" {
                let points = if axis_count == 0 {
                    Vec::new()
                } else {
                    let mut points = Vec::with_capacity(axis_count);
                    for axis_index in 0..axis_count {
                        work_meter.checkpoint(OperationPhase::Layout)?;
                        let angle = -std::f64::consts::FRAC_PI_2
                            + (axis_index as f64) * (std::f64::consts::TAU / (axis_count as f64));
                        points.push(polar_xy(r, angle));
                    }
                    points
                };
                graticules.push(RadarGraticuleShapeLayout {
                    kind: "polygon".to_string(),
                    r: None,
                    points,
                });
            } else {
                graticules.push(RadarGraticuleShapeLayout {
                    kind: "circle".to_string(),
                    r: Some(r),
                    points: Vec::new(),
                });
            }
        }
    }

    let mut inferred_max = f64::NEG_INFINITY;
    for c in &model.curves {
        work_meter.checkpoint(OperationPhase::Layout)?;
        for v in &c.entries {
            work_meter.checkpoint(OperationPhase::Layout)?;
            inferred_max = if inferred_max.is_nan() || v.is_nan() {
                f64::NAN
            } else {
                inferred_max.max(*v)
            };
        }
    }
    let max_value = model.options.max.unwrap_or(inferred_max);
    let min_value = model.options.min;

    let mut curves: Vec<RadarCurveLayout> = Vec::new();
    for (curve_idx, curve) in model.curves.iter().enumerate() {
        work_meter.checkpoint(OperationPhase::Layout)?;
        if curve.entries.len() != axis_count {
            continue;
        }
        let mut points: Vec<LayoutPoint> = Vec::new();
        if axis_count > 0 {
            for (i, &entry) in curve.entries.iter().enumerate() {
                work_meter.checkpoint(OperationPhase::Layout)?;
                let angle = -std::f64::consts::FRAC_PI_2
                    + (i as f64) * (std::f64::consts::TAU / (axis_count as f64));
                let radius = relative_radius(entry, min_value, max_value, base_radius);
                points.push(polar_xy(radius, angle));
            }
        }
        let path_d = if model.options.graticule.trim() == "polygon" {
            String::new()
        } else {
            closed_round_curve_path(&points, curve_tension)
        };
        curves.push(RadarCurveLayout {
            label: curve.label.clone(),
            class_index: curve_idx as i64,
            points,
            path_d,
        });
    }

    let mut legend_items: Vec<RadarLegendItemLayout> = Vec::new();
    if model.options.show_legend {
        let base_x = ((width / 2.0 + margin_right) * 3.0) / 4.0;
        let base_y = (-(height / 2.0 + margin_top) * 3.0) / 4.0;
        let step_y = 20.0;
        for (i, c) in model.curves.iter().enumerate() {
            work_meter.checkpoint(OperationPhase::Layout)?;
            legend_items.push(RadarLegendItemLayout {
                label: c.label.clone(),
                class_index: i as i64,
                x: base_x,
                y: base_y + (i as f64) * step_y,
            });
        }
    }

    Ok(RadarDiagramLayout {
        bounds: Some(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: svg_width,
            max_y: svg_height,
        }),
        svg_width,
        svg_height,
        center_x,
        center_y,
        radius,
        axis_label_factor,
        title_y,
        axes,
        graticules,
        curves,
        legend_items,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        RadarTicks, layout_radar_diagram_typed, layout_radar_diagram_typed_with_resource_policy,
        layout_radar_diagram_typed_with_work_meter,
    };
    use crate::resources::OperationWorkMeter;
    use crate::text::DeterministicTextMeasurer;
    use crate::{Error, RenderResourcePolicy, ResourceLimitId};
    use merman_model::families::radar::{RadarAxis, RadarCurve, RadarDiagramModel, RadarOptions};
    use merman_model::{OperationControl, OperationPhase};
    use serde_json::json;

    fn radar_model_with_entries(entries: &[&[u64]]) -> RadarDiagramModel {
        RadarDiagramModel {
            axes: ["A", "B", "C"]
                .into_iter()
                .map(|name| RadarAxis {
                    name: name.to_owned(),
                    label: name.to_owned(),
                })
                .collect(),
            curves: entries
                .iter()
                .enumerate()
                .map(|(index, values)| RadarCurve {
                    name: format!("Curve{index}"),
                    label: format!("Curve {index}"),
                    entries: values.iter().map(|value| *value as f64).collect(),
                })
                .collect(),
            options: RadarOptions {
                ticks: 0.0,
                ..RadarOptions::default()
            },
            ..RadarDiagramModel::default()
        }
    }

    #[test]
    fn radar_skips_short_and_long_curves_without_changing_scale_or_legend_indices() {
        let measurer = DeterministicTextMeasurer::default();
        for graticule in ["circle", "polygon"] {
            for show_legend in [false, true] {
                let mut model = radar_model_with_entries(&[&[100, 1], &[1, 2, 3], &[1, 2, 3, 200]]);
                model.options.graticule = graticule.to_owned();
                model.options.show_legend = show_legend;
                let layout = layout_radar_diagram_typed(&model, &json!({}), &measurer).unwrap();

                assert_eq!(layout.curves.len(), 1);
                assert_eq!(layout.curves[0].class_index, 1);
                assert_eq!(layout.curves[0].points.len(), 3);
                assert!((layout.curves[0].points[0].y + 1.5).abs() < 1e-9);
                assert_eq!(
                    layout
                        .legend_items
                        .iter()
                        .map(|item| item.class_index)
                        .collect::<Vec<_>>(),
                    if show_legend {
                        vec![0, 1, 2]
                    } else {
                        Vec::new()
                    }
                );
            }
        }
    }

    #[test]
    fn radar_keeps_legend_when_every_curve_is_skipped() {
        let model = radar_model_with_entries(&[&[1, 2], &[1, 2, 3, 4]]);
        let layout =
            layout_radar_diagram_typed(&model, &json!({}), &DeterministicTextMeasurer::default())
                .unwrap();

        assert!(layout.curves.is_empty());
        assert_eq!(layout.legend_items.len(), 2);
        assert_eq!(layout.legend_items[1].class_index, 1);
    }

    #[test]
    fn radar_programmatic_nonfinite_numbers_propagate_through_layout() {
        for (entry, min, max) in [
            (f64::NAN, 0.0, None),
            (1.0, f64::NEG_INFINITY, Some(3.0)),
            (1.0, 0.0, Some(f64::NEG_INFINITY)),
            (1.0, 0.0, Some(f64::NAN)),
        ] {
            let mut model = radar_model_with_entries(&[&[1, 2, 3]]);
            model.curves[0].entries[0] = entry;
            model.options.min = min;
            model.options.max = max;
            let layout = layout_radar_diagram_typed(
                &model,
                &json!({}),
                &DeterministicTextMeasurer::default(),
            )
            .unwrap();
            assert!(
                layout.curves[0]
                    .points
                    .iter()
                    .all(|point| point.x.is_nan() && point.y.is_nan())
            );
            assert!(layout.curves[0].path_d.starts_with("MNaN,NaN CNaN,NaN"));
        }
    }

    #[test]
    fn radar_fractional_ticks_use_ceil_iterations_and_the_original_denominator() {
        for graticule in ["circle", "polygon"] {
            for (ticks, radii) in [
                (2.0, vec![150.0, 300.0]),
                (2.5, vec![120.0, 240.0, 360.0]),
                (0.5, vec![600.0]),
                (0.0, Vec::new()),
            ] {
                let mut model = radar_model_with_entries(&[&[1, 2, 3]]);
                model.options.graticule = graticule.to_owned();
                model.options.ticks = ticks;
                let layout = layout_radar_diagram_typed(
                    &model,
                    &json!({}),
                    &DeterministicTextMeasurer::default(),
                )
                .unwrap();

                assert_eq!(layout.graticules.len(), radii.len());
                for (shape, radius) in layout.graticules.iter().zip(radii) {
                    assert_eq!(shape.kind, graticule);
                    if graticule == "polygon" {
                        assert_eq!(shape.points.len(), 3);
                        assert!((shape.points[0].y + radius).abs() < 1e-9);
                    } else {
                        assert_eq!(shape.r, Some(radius));
                    }
                }
            }
        }
    }

    #[test]
    fn radar_nonpositive_or_nan_typed_ticks_do_not_generate_iterations() {
        for value in [0.0, -0.0, -1.0, -0.5, f64::NAN, f64::NEG_INFINITY] {
            let ticks = RadarTicks::new(value);
            assert_eq!(ticks.iterations, 0, "{value}");
            assert_eq!(ticks.value.to_bits(), value.to_bits());
        }
        assert_eq!(RadarTicks::new(u64::MAX as f64).iterations, usize::MAX);
        assert_eq!(RadarTicks::new(f64::INFINITY).iterations, usize::MAX);
    }

    #[test]
    fn radar_work_budget_counts_fractional_graticules_and_only_drawable_curve_geometry() {
        for (graticule, show_legend, expected_work) in [
            ("circle", false, 22),
            ("circle", true, 25),
            ("polygon", false, 31),
            ("polygon", true, 34),
        ] {
            let mut model = radar_model_with_entries(&[&[100, 1], &[1, 2, 3], &[1, 2, 3, 200]]);
            model.options.graticule = graticule.to_owned();
            model.options.ticks = 2.5;
            model.options.show_legend = show_legend;
            let exact = RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, expected_work)
                .unwrap();
            let meter = OperationWorkMeter::new(exact);
            let layout = layout_radar_diagram_typed_with_work_meter(
                &model,
                &json!({}),
                &DeterministicTextMeasurer::default(),
                &meter,
            )
            .unwrap();
            assert_eq!(meter.used(), expected_work);
            assert_eq!(layout.graticules.len(), 3);
            assert_eq!(layout.curves.len(), 1);

            let below = RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, expected_work - 1)
                .unwrap();
            let error = layout_radar_diagram_typed_with_resource_policy(
                &model,
                &json!({}),
                &DeterministicTextMeasurer::default(),
                below,
            )
            .unwrap_err();
            let Error::ResourceLimitExceeded(error) = error else {
                panic!("expected Radar work limit");
            };
            assert_eq!(error.actual, expected_work);
        }
    }

    #[test]
    fn radar_layout_observes_cancellation_before_and_during_fractional_graticules() {
        let mut model = RadarDiagramModel::default();
        model.options.ticks = 2.5;
        for checkpoints in [0, 2] {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(checkpoints);
            let meter = OperationWorkMeter::new_with_control(
                RenderResourcePolicy::unbounded_for_trusted_input(),
                control,
            );
            let error = layout_radar_diagram_typed_with_work_meter(
                &model,
                &json!({}),
                &DeterministicTextMeasurer::default(),
                &meter,
            )
            .unwrap_err();
            assert!(matches!(
                error,
                Error::Cancelled(cancelled) if cancelled.phase == OperationPhase::Layout
            ));
            assert_eq!(meter.used(), if checkpoints == 0 { 0 } else { 3 });
        }
    }

    #[test]
    fn radar_legend_layout_uses_mermaid_step_y() {
        let model = RadarDiagramModel {
            title: Some("Radar".to_string()),
            axes: vec![
                RadarAxis {
                    name: "a".to_string(),
                    label: "A".to_string(),
                },
                RadarAxis {
                    name: "b".to_string(),
                    label: "B".to_string(),
                },
                RadarAxis {
                    name: "c".to_string(),
                    label: "C".to_string(),
                },
            ],
            curves: vec![
                RadarCurve {
                    name: "one".to_string(),
                    label: "One".to_string(),
                    entries: vec![1.0, 2.0, 3.0],
                },
                RadarCurve {
                    name: "two".to_string(),
                    label: "Two".to_string(),
                    entries: vec![3.0, 2.0, 1.0],
                },
            ],
            options: RadarOptions {
                show_legend: true,
                ticks: 3.0,
                min: 0.0,
                max: Some(3.0),
                graticule: "circle".to_string(),
            },
            ..Default::default()
        };
        let measurer = DeterministicTextMeasurer {
            char_width_factor: 8.0,
            line_height_factor: 16.0,
        };

        let layout = layout_radar_diagram_typed(&model, &json!({}), &measurer).unwrap();

        assert_eq!(layout.legend_items.len(), 2);
        assert_eq!(layout.legend_items[1].y - layout.legend_items[0].y, 20.0);
    }

    #[test]
    fn radar_work_budget_is_checked_before_the_layout_allocates_shapes() {
        let mut model = RadarDiagramModel::default();
        model.options.ticks = 3.0;
        let measurer = DeterministicTextMeasurer {
            char_width_factor: 8.0,
            line_height_factor: 16.0,
        };
        let limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 2)
            .unwrap();

        let error =
            layout_radar_diagram_typed_with_resource_policy(&model, &json!({}), &measurer, limits)
                .unwrap_err();

        let Error::ResourceLimitExceeded(error) = error else {
            panic!("expected resource limit error");
        };
        assert_eq!(error.limit, "max_layout_work_units");
        assert_eq!(error.actual, 3);
    }

    #[test]
    fn radar_polygon_work_is_aggregated_before_allocating_points() {
        let mut model = RadarDiagramModel {
            axes: vec![
                RadarAxis {
                    name: "a".to_string(),
                    label: "A".to_string(),
                },
                RadarAxis {
                    name: "b".to_string(),
                    label: "B".to_string(),
                },
            ],
            ..Default::default()
        };
        model.options.ticks = 3.0;
        model.options.graticule = "polygon".to_string();
        let measurer = DeterministicTextMeasurer {
            char_width_factor: 8.0,
            line_height_factor: 16.0,
        };
        let limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 10)
            .unwrap();

        let error =
            layout_radar_diagram_typed_with_resource_policy(&model, &json!({}), &measurer, limits)
                .unwrap_err();

        let Error::ResourceLimitExceeded(error) = error else {
            panic!("expected resource limit error");
        };
        assert_eq!(error.limit, "max_layout_work_units");
        assert_eq!(error.actual, 11);
    }

    #[test]
    fn radar_unbounded_typed_tick_counts_are_rejected_without_rewriting_the_model() {
        let measurer = DeterministicTextMeasurer {
            char_width_factor: 8.0,
            line_height_factor: 16.0,
        };
        let limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();

        for ticks in [u64::MAX as f64, f64::INFINITY] {
            let mut model = RadarDiagramModel::default();
            model.options.ticks = ticks;
            let error = layout_radar_diagram_typed_with_resource_policy(
                &model,
                &json!({}),
                &measurer,
                limits,
            )
            .unwrap_err();
            let Error::ResourceLimitExceeded(error) = error else {
                panic!("expected resource limit error");
            };
            assert_eq!(error.limit, "max_layout_work_units");
            assert_eq!(error.actual, usize::MAX);
            assert_eq!(model.options.ticks.to_bits(), ticks.to_bits());
        }
    }
}
