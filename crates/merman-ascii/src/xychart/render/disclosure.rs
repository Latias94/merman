use super::{ChartDocument, ChartLine, new_chart_line};
use crate::color::AsciiColorRole;
use crate::options::TerminalWidthProfile;
use crate::resource::ResourceContext;
use crate::safe_text::visit_quoted_terminal_text;
use crate::text::display_width_with_profile;
use crate::xychart::plot::{
    ChartChars, SeriesPlan, TerminalChartPlan, TerminalDisclosurePlan, format_data_number,
    plot_type_name,
};
use crate::{AsciiError, AsciiRenderOptions, Result};
use merman_core::diagrams::xychart::XyChartDiagramRenderModel;

pub(super) fn push_value_disclosure_lines(
    out: &mut ChartDocument,
    model: &XyChartDiagramRenderModel,
    plan: &TerminalChartPlan,
    chars: ChartChars,
    disclosure: TerminalDisclosurePlan,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<()> {
    if disclosure.band_domain {
        let line = band_domain_disclosure_line(model, plan, options, resources)?;
        out.push(line, resources)?;
    }
    for series in &plan.series {
        resources.charge_layout_work(1)?;
        let Some(line) = value_disclosure_line(model, series, plan, chars, options, resources)?
        else {
            continue;
        };
        out.push(line, resources)?;
    }
    Ok(())
}

pub(super) fn value_disclosure_line_width(
    model: &XyChartDiagramRenderModel,
    series: &SeriesPlan,
    plan: &TerminalChartPlan,
    chars: ChartChars,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<Option<usize>> {
    let mut line = MeasuredDisclosureLine {
        width: 0,
        width_profile: options.terminal_width_profile,
    };
    write_value_disclosure_line(&mut line, model, series, plan, chars, resources)?;
    Ok(Some(line.width))
}

fn value_disclosure_line(
    model: &XyChartDiagramRenderModel,
    series: &SeriesPlan,
    plan: &TerminalChartPlan,
    chars: ChartChars,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<Option<ChartLine>> {
    let mut line = new_chart_line(options, resources);
    write_value_disclosure_line(
        &mut StyledDisclosureLine { line: &mut line },
        model,
        series,
        plan,
        chars,
        resources,
    )?;

    Ok(Some(line))
}

pub(super) fn band_domain_disclosure_line_width(
    model: &XyChartDiagramRenderModel,
    plan: &TerminalChartPlan,
    disclosure: TerminalDisclosurePlan,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<Option<usize>> {
    if !disclosure.band_domain {
        return Ok(None);
    }

    let mut line = MeasuredDisclosureLine {
        width: 0,
        width_profile: options.terminal_width_profile,
    };
    write_band_domain_disclosure_line(&mut line, model, plan, resources)?;
    Ok(Some(line.width))
}

fn band_domain_disclosure_line(
    model: &XyChartDiagramRenderModel,
    plan: &TerminalChartPlan,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<ChartLine> {
    let mut line = new_chart_line(options, resources);
    write_band_domain_disclosure_line(
        &mut StyledDisclosureLine { line: &mut line },
        model,
        plan,
        resources,
    )?;
    Ok(line)
}

fn write_band_domain_disclosure_line(
    line: &mut impl DisclosureLine,
    model: &XyChartDiagramRenderModel,
    plan: &TerminalChartPlan,
    resources: &mut ResourceContext,
) -> Result<()> {
    let crate::xychart::plot::AxisPlan::Band { .. } = &plan.x_axis else {
        return Ok(());
    };
    let merman_core::diagrams::xychart::XyChartAxisRenderModel::Band { categories, .. } =
        &model.x_axis
    else {
        return Err(invalid_disclosure_plan());
    };

    line.push_text(
        "xDomain: band categories=[",
        AsciiColorRole::Text,
        resources,
    )?;
    for (index, category) in categories.iter().enumerate() {
        resources.charge_layout_work(1)?;
        if index > 0 {
            line.push_text(", ", AsciiColorRole::Text, resources)?;
        }
        push_disclosure_value(line, category, resources)?;
    }
    line.push_char(']', AsciiColorRole::Text, resources)
}

trait DisclosureLine {
    fn push_text(
        &mut self,
        text: &str,
        role: AsciiColorRole,
        resources: &ResourceContext,
    ) -> Result<()>;

    fn push_char(
        &mut self,
        ch: char,
        role: AsciiColorRole,
        resources: &ResourceContext,
    ) -> Result<()> {
        let mut buffer = [0u8; 4];
        self.push_text(ch.encode_utf8(&mut buffer), role, resources)
    }

    fn push_quoted(
        &mut self,
        value: &str,
        role: AsciiColorRole,
        resources: &ResourceContext,
    ) -> Result<()> {
        visit_quoted_terminal_text(value, resources, |fragment| {
            self.push_text(fragment, role, resources)
        })
    }
}

struct StyledDisclosureLine<'a> {
    line: &'a mut ChartLine,
}

impl DisclosureLine for StyledDisclosureLine<'_> {
    fn push_text(
        &mut self,
        text: &str,
        role: AsciiColorRole,
        _resources: &ResourceContext,
    ) -> Result<()> {
        self.line.try_push_role_text(text, role)
    }

    fn push_quoted(
        &mut self,
        value: &str,
        role: AsciiColorRole,
        _resources: &ResourceContext,
    ) -> Result<()> {
        self.line.try_push_role_quoted_text(value, role)
    }
}

struct MeasuredDisclosureLine {
    width: usize,
    width_profile: TerminalWidthProfile,
}

impl DisclosureLine for MeasuredDisclosureLine {
    fn push_text(
        &mut self,
        text: &str,
        _role: AsciiColorRole,
        resources: &ResourceContext,
    ) -> Result<()> {
        self.width = resources.checked_grid_add(
            self.width,
            display_width_with_profile(text, self.width_profile),
        )?;
        Ok(())
    }
}

fn write_value_disclosure_line(
    line: &mut impl DisclosureLine,
    model: &XyChartDiagramRenderModel,
    series: &SeriesPlan,
    plan: &TerminalChartPlan,
    chars: ChartChars,
    resources: &mut ResourceContext,
) -> Result<()> {
    let plot = model
        .plots
        .get(series.series_index)
        .ok_or_else(invalid_disclosure_plan)?;
    line.push_text("values: ", AsciiColorRole::Text, resources)?;
    line.push_char(
        chars.legend_symbol(series.plot_type),
        AsciiColorRole::ChartSeries(series.series_index),
        resources,
    )?;
    line.push_text(" series=", AsciiColorRole::Text, resources)?;
    push_disclosure_usize(line, series.series_index, resources)?;
    line.push_text(" type=", AsciiColorRole::Text, resources)?;
    line.push_text(
        plot_type_name(series.plot_type),
        AsciiColorRole::Text,
        resources,
    )?;
    match series.title.as_deref() {
        Some(title) => {
            line.push_char(' ', AsciiColorRole::Text, resources)?;
            push_disclosure_field(line, "title", title, resources)?;
        }
        None => line.push_text(" title=none", AsciiColorRole::Text, resources)?,
    }
    line.push_text(" samples=[", AsciiColorRole::Text, resources)?;

    for (index, datum) in series.data.iter().enumerate() {
        resources.charge_layout_work(1)?;
        if index > 0 {
            line.push_text(", ", AsciiColorRole::Text, resources)?;
        }
        line.push_text("{index=", AsciiColorRole::Text, resources)?;
        push_disclosure_usize(line, index, resources)?;
        line.push_char(' ', AsciiColorRole::Text, resources)?;
        push_disclosure_field(line, "x", &datum.x, resources)?;
        line.push_text(" value=", AsciiColorRole::Text, resources)?;
        match datum.value {
            Some(value) => {
                line.push_text(&format_data_number(value), AsciiColorRole::Text, resources)?
            }
            None => line.push_text("none", AsciiColorRole::Text, resources)?,
        }
        match plot.point_labels.get(index) {
            Some(point_label) => {
                line.push_char(' ', AsciiColorRole::Text, resources)?;
                push_disclosure_field(line, "pointLabel", point_label, resources)?;
            }
            None => line.push_text(" pointLabel=none", AsciiColorRole::Text, resources)?,
        }
        line.push_text(" clipped=", AsciiColorRole::Text, resources)?;
        line.push_text(
            if datum.x_clipped
                || datum
                    .value
                    .is_some_and(|value| !plan.y_range.contains(value))
            {
                "true"
            } else {
                "false"
            },
            AsciiColorRole::Text,
            resources,
        )?;
        line.push_char('}', AsciiColorRole::Text, resources)?;
    }
    line.push_text("] orphanPointLabels=[", AsciiColorRole::Text, resources)?;

    let point_labels = plot.point_labels.as_slice();
    let orphan_start = series.data.len().min(point_labels.len());
    for (index, point_label) in point_labels[orphan_start..].iter().enumerate() {
        resources.charge_layout_work(1)?;
        if index > 0 {
            line.push_text(", ", AsciiColorRole::Text, resources)?;
        }
        push_disclosure_value(line, point_label, resources)?;
    }
    line.push_char(']', AsciiColorRole::Text, resources)
}

fn invalid_disclosure_plan() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "xychart",
        feature: "chart disclosure plan",
    }
}

fn push_disclosure_usize(
    line: &mut impl DisclosureLine,
    value: usize,
    resources: &ResourceContext,
) -> Result<()> {
    line.push_text(&value.to_string(), AsciiColorRole::Text, resources)
}

fn push_disclosure_field(
    line: &mut impl DisclosureLine,
    key: &str,
    value: &str,
    resources: &ResourceContext,
) -> Result<()> {
    line.push_text(key, AsciiColorRole::Text, resources)?;
    line.push_text("(bytes=", AsciiColorRole::Text, resources)?;
    push_disclosure_usize(line, value.len(), resources)?;
    line.push_text(")=", AsciiColorRole::Text, resources)?;
    line.push_quoted(value, AsciiColorRole::Text, resources)
}

fn push_disclosure_value(
    line: &mut impl DisclosureLine,
    value: &str,
    resources: &ResourceContext,
) -> Result<()> {
    line.push_text("bytes=", AsciiColorRole::Text, resources)?;
    push_disclosure_usize(line, value.len(), resources)?;
    line.push_char('=', AsciiColorRole::Text, resources)?;
    line.push_quoted(value, AsciiColorRole::Text, resources)
}
