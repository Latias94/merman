use super::super::plot::format_data_number;
use super::super::plot::plot_type_name;
use super::ChartOrientation;
use crate::Result;
use crate::options::AsciiRenderOptions;
use crate::resource::ResourceContext;
use crate::safe_text::{
    BudgetedTextDocument, BudgetedTextLine, push_line_field, push_line_list,
    push_optional_document_field,
};
use merman_core::diagrams::xychart::{
    XyChartAxisDisplayPolicy, XyChartAxisRenderModel, XyChartDiagramRenderModel,
    XyChartPlotRenderModel,
};

pub(super) fn render(
    model: &XyChartDiagramRenderModel,
    orientation: ChartOrientation,
    options: &AsciiRenderOptions,
    resources: ResourceContext,
) -> Result<String> {
    render_with_probe(model, orientation, options, resources, None)
}

fn render_with_probe(
    model: &XyChartDiagramRenderModel,
    orientation: ChartOrientation,
    options: &AsciiRenderOptions,
    resources: ResourceContext,
    #[cfg(test)] retained: Option<std::rc::Rc<std::cell::Cell<usize>>>,
    #[cfg(not(test))] _retained: Option<()>,
) -> Result<String> {
    let mut document = BudgetedTextDocument::from_resources(resources, options);
    #[cfg(test)]
    if let Some(retained) = retained {
        document.set_retain_probe(retained);
    }
    document.push_line("xychart: empty")?;
    document.push_line_with(|line| {
        line.push_str("orientation: ")?;
        line.push_str(orientation.as_str())
    })?;
    push_optional_document_field(&mut document, "title", model.title.as_deref())?;
    push_axis(&mut document, "xAxis", &model.x_axis)?;
    push_axis(&mut document, "yAxis", &model.y_axis)?;
    push_display_policy(&mut document, model)?;
    push_plots(&mut document, &model.plots)?;
    document.finish()
}

fn push_axis(
    document: &mut BudgetedTextDocument,
    key: &str,
    axis: &XyChartAxisRenderModel,
) -> Result<()> {
    document.push_line_with(|line| {
        line.push_str(key)?;
        match axis {
            XyChartAxisRenderModel::Band { title, categories } => {
                line.push_str(": band ")?;
                push_line_field(line, "", "title", title)?;
                line.push_str(" ")?;
                push_line_list(
                    line,
                    "",
                    "categories",
                    categories.iter().map(String::as_str),
                )
            }
            XyChartAxisRenderModel::Linear { title, min, max } => {
                line.push_str(": linear ")?;
                push_line_field(line, "", "title", title)?;
                line.push_str(" min=")?;
                push_optional_number(line, *min)?;
                line.push_str(" max=")?;
                push_optional_number(line, *max)
            }
        }
    })
}

fn push_display_policy(
    document: &mut BudgetedTextDocument,
    model: &XyChartDiagramRenderModel,
) -> Result<()> {
    document.push_line_with(|line| {
        line.write_fmt(format_args!(
            "display: showTitle={} showDataLabel={} showDataLabelOutsideBar={} ",
            model.display.show_title,
            model.display.show_data_label,
            model.display.show_data_label_outside_bar,
        ))?;
        push_axis_display_policy(line, "xAxis", model.display.x_axis)?;
        line.push_str(" ")?;
        push_axis_display_policy(line, "yAxis", model.display.y_axis)
    })
}

fn push_axis_display_policy(
    line: &mut BudgetedTextLine<'_>,
    key: &str,
    policy: XyChartAxisDisplayPolicy,
) -> Result<()> {
    line.write_fmt(format_args!(
        "{key}={{showLabel={} showTitle={} showTick={} showAxisLine={}}}",
        policy.show_label, policy.show_title, policy.show_tick, policy.show_axis_line,
    ))
}

fn push_plots(document: &mut BudgetedTextDocument, plots: &[XyChartPlotRenderModel]) -> Result<()> {
    if plots.is_empty() {
        return document.push_line("plots: []");
    }

    document.push_line_with(|line| line.write_fmt(format_args!("plots: count={}", plots.len())))?;
    for (index, plot) in plots.iter().enumerate() {
        document.resources_mut().charge_layout_work(1)?;
        document.push_line_with(|line| {
            line.write_fmt(format_args!(
                "plot: index={index} type={} ",
                plot_type_name(plot.plot_type),
            ))?;
            match plot.title.as_deref() {
                Some(title) => push_line_field(line, "", "title", title)?,
                None => line.push_str("title=none")?,
            }
            line.push_str(" values=[] data=[] ")?;
            push_line_list(
                line,
                "",
                "pointLabels",
                plot.point_labels.iter().map(String::as_str),
            )
        })?;
    }
    Ok(())
}

fn push_optional_number(line: &mut BudgetedTextLine<'_>, value: Option<f64>) -> Result<()> {
    match value {
        Some(value) => line.push_str(&format_data_number(value)),
        None => line.push_str("none"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AsciiError;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use crate::xychart::plot::TerminalChartPlan;
    use merman_core::diagrams::xychart::{XyChartDisplayPolicy, XyChartPlotType};
    use merman_core::resources::ResourceProfile;
    use std::cell::Cell;
    use std::rc::Rc;

    fn zero_slot_model() -> XyChartDiagramRenderModel {
        XyChartDiagramRenderModel {
            orientation: "vertical".to_string(),
            title: Some("No samples".to_string()),
            acc_title: None,
            acc_descr: None,
            x_axis: XyChartAxisRenderModel::Band {
                title: "Category".to_string(),
                categories: Vec::new(),
            },
            y_axis: XyChartAxisRenderModel::Linear {
                title: "Score".to_string(),
                min: Some(0.0),
                max: Some(10.0),
            },
            plots: vec![XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Line,
                title: Some("Forecast".to_string()),
                values: Vec::new(),
                data: Vec::new(),
                point_labels: vec!["orphan".to_string()],
            }],
            display: XyChartDisplayPolicy::default(),
        }
    }

    fn measured_layout_work(model: &XyChartDiagramRenderModel) -> usize {
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let options = AsciiRenderOptions::ascii();
        let mut resources = ResourceContext::new(policy);
        let meter = resources.clone();
        assert!(
            TerminalChartPlan::measure_cardinality(model, &mut resources)
                .expect("zero-slot probe should fit the configured work budget")
                .is_empty()
        );
        render(model, ChartOrientation::Vertical, &options, resources)
            .expect("unbounded zero-slot projection should render");
        meter.layout_work_used()
    }

    #[test]
    fn empty_projection_uses_one_cumulative_layout_work_ledger() {
        let model = zero_slot_model();
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let exact_work = measured_layout_work(&model);
        assert!(exact_work > 1);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact layout-work limit should be valid");
        super::super::render_xychart_diagram_with_resources(
            &model,
            &AsciiRenderOptions::ascii(),
            exact_policy,
        )
        .expect("exact zero-slot layout-work budget should render through the public path");

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("N-1 layout-work limit should be valid");
        let error = super::super::render_xychart_diagram_with_resources(
            &model,
            &AsciiRenderOptions::ascii(),
            below_policy,
        )
        .expect_err("N-1 zero-slot layout-work budget should reject through the public path");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact_work);
        assert_eq!(details.max, exact_work - 1);
    }

    #[test]
    fn empty_projection_rejects_n_minus_one_before_retaining_overflow_fragment() {
        let model = zero_slot_model();
        let options = AsciiRenderOptions::ascii();
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(unbounded);
        let rendered = render(&model, ChartOrientation::Vertical, &options, resources)
            .expect("the unbounded empty projection should render");

        let exact = unbounded
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, rendered.len())
            .expect("the exact output limit should be valid");
        let exact_retained = Rc::new(Cell::new(0));
        render_with_probe(
            &model,
            ChartOrientation::Vertical,
            &options,
            ResourceContext::new(exact),
            Some(Rc::clone(&exact_retained)),
        )
        .expect("the exact empty projection should retain every fragment");

        let below = exact
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, rendered.len() - 1)
            .expect("the N-1 output limit should be valid");
        let below_retained = Rc::new(Cell::new(0));
        let error = render_with_probe(
            &model,
            ChartOrientation::Vertical,
            &options,
            ResourceContext::new(below),
            Some(Rc::clone(&below_retained)),
        )
        .expect_err("N-1 must reject before retaining the overflow fragment");

        assert!(below_retained.get() < exact_retained.get());
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected output resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxOutputBytes);
        assert_eq!(details.actual, rendered.len());
        assert_eq!(details.max, rendered.len() - 1);
    }
}
