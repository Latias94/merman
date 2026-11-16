use super::{ChartDocument, ChartDocumentPlan, ChartLine, new_chart_line};
use crate::color::AsciiColorRole;
use crate::options::TerminalWidthProfile;
use crate::resource::ResourceContext;
use crate::safe_text::visit_quoted_terminal_text;
use crate::text::display_width_with_profile;
use crate::xychart::XyChartView;
use crate::xychart::plot::{
    ChartChars, LinearDomainPlan, SeriesPlan, TerminalChartPlan, TerminalDisclosurePlan,
    format_data_number, plot_type_name,
};
use crate::{AsciiError, AsciiRenderOptions, Result};

pub(super) fn push_value_disclosure_lines(
    out: &mut ChartDocument,
    model: &XyChartView<'_>,
    plan: &TerminalChartPlan<'_>,
    chars: ChartChars,
    disclosure: TerminalDisclosurePlan,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<()> {
    if disclosure.band_domain {
        let line = band_domain_disclosure_line(model, plan, options, resources)?;
        out.push(line, resources)?;
    }
    if disclosure.x_linear_domain
        && let Some(domain) = plan.x_linear_domain
    {
        let line =
            linear_domain_disclosure_line(domain, LinearDomainOwner::XAxis, options, resources)?;
        out.push(line, resources)?;
    }
    if disclosure.y_linear_domain {
        let line = linear_domain_disclosure_line(
            plan.y_linear_domain,
            LinearDomainOwner::YAxis,
            options,
            resources,
        )?;
        out.push(line, resources)?;
    }
    if !disclosure.values {
        return Ok(());
    }
    for series in &plan.series {
        let disclosure = ValueDisclosure::new(model, series, plan, chars)?;
        for row in disclosure.rows() {
            let mut line = new_chart_line(options, resources);
            disclosure.write_row(
                &mut StyledDisclosureLine { line: &mut line },
                row,
                resources,
            )?;
            out.push(line, resources)?;
        }
    }
    Ok(())
}

/// Closed ownership tag for the injective authored-title row protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TitleOwner {
    Chart,
    XAxis,
    YAxis,
}

impl TitleOwner {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Chart => "chart",
            Self::XAxis => "xAxis",
            Self::YAxis => "yAxis",
        }
    }
}

pub(super) fn push_title_display_line(
    out: &mut ChartDocument,
    owner: TitleOwner,
    value: &str,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<()> {
    let mut line = new_chart_line(options, resources);
    write_title_display_line(
        &mut StyledDisclosureLine { line: &mut line },
        owner,
        value,
        resources,
    )?;
    out.push(line, resources)
}

pub(super) fn title_display_line_width(
    owner: TitleOwner,
    value: &str,
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
) -> Result<usize> {
    let mut line = MeasuredDisclosureLine {
        width: 0,
        width_profile: options.terminal_width_profile,
    };
    write_title_display_line(&mut line, owner, value, resources)?;
    Ok(line.width)
}

fn write_title_display_line(
    line: &mut impl DisclosureLine,
    owner: TitleOwner,
    value: &str,
    resources: &ResourceContext,
) -> Result<()> {
    line.push_text("titleDisplay: ", AsciiColorRole::Text, resources)?;
    push_disclosure_field(line, owner.as_str(), value, resources)
}

pub(super) fn measure_value_disclosure(
    document: &mut ChartDocumentPlan,
    model: &XyChartView<'_>,
    series: &SeriesPlan<'_>,
    plan: &TerminalChartPlan<'_>,
    chars: ChartChars,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<()> {
    let disclosure = ValueDisclosure::new(model, series, plan, chars)?;
    for row in disclosure.rows() {
        let mut line = MeasuredDisclosureLine {
            width: 0,
            width_profile: options.terminal_width_profile,
        };
        disclosure.write_row(&mut line, row, resources)?;
        document.include_line(line.width, resources)?;
    }
    Ok(())
}

pub(super) fn band_domain_disclosure_line_width(
    model: &XyChartView<'_>,
    plan: &TerminalChartPlan<'_>,
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
    model: &XyChartView<'_>,
    plan: &TerminalChartPlan<'_>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinearDomainOwner {
    XAxis,
    YAxis,
}

impl LinearDomainOwner {
    const fn prefix(self) -> &'static str {
        match self {
            Self::XAxis => "xDomain: linear authored=[",
            Self::YAxis => "yDomain: linear authored=[",
        }
    }
}

pub(super) fn linear_domain_disclosure_line_widths(
    plan: &TerminalChartPlan<'_>,
    disclosure: TerminalDisclosurePlan,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<(Option<usize>, Option<usize>)> {
    let x_width = if disclosure.x_linear_domain {
        let domain = plan.x_linear_domain.ok_or_else(invalid_disclosure_plan)?;
        Some(measure_linear_domain_disclosure_line(
            domain,
            LinearDomainOwner::XAxis,
            options,
            resources,
        )?)
    } else {
        None
    };
    let y_width = if disclosure.y_linear_domain {
        Some(measure_linear_domain_disclosure_line(
            plan.y_linear_domain,
            LinearDomainOwner::YAxis,
            options,
            resources,
        )?)
    } else {
        None
    };
    Ok((x_width, y_width))
}

fn measure_linear_domain_disclosure_line(
    domain: LinearDomainPlan,
    owner: LinearDomainOwner,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<usize> {
    let mut line = MeasuredDisclosureLine {
        width: 0,
        width_profile: options.terminal_width_profile,
    };
    write_linear_domain_disclosure_line(&mut line, domain, owner, resources)?;
    Ok(line.width)
}

fn linear_domain_disclosure_line(
    domain: LinearDomainPlan,
    owner: LinearDomainOwner,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<ChartLine> {
    let mut line = new_chart_line(options, resources);
    write_linear_domain_disclosure_line(
        &mut StyledDisclosureLine { line: &mut line },
        domain,
        owner,
        resources,
    )?;
    Ok(line)
}

fn write_linear_domain_disclosure_line(
    line: &mut impl DisclosureLine,
    domain: LinearDomainPlan,
    owner: LinearDomainOwner,
    resources: &mut ResourceContext,
) -> Result<()> {
    line.push_text(owner.prefix(), AsciiColorRole::Text, resources)?;
    push_optional_data_number(line, domain.authored_min, resources)?;
    line.push_text(",", AsciiColorRole::Text, resources)?;
    push_optional_data_number(line, domain.authored_max, resources)?;
    line.push_text("] resolved=[", AsciiColorRole::Text, resources)?;
    line.push_text(
        &format_data_number(domain.resolved.min),
        AsciiColorRole::Text,
        resources,
    )?;
    line.push_text(",", AsciiColorRole::Text, resources)?;
    line.push_text(
        &format_data_number(domain.resolved.max),
        AsciiColorRole::Text,
        resources,
    )?;
    line.push_char(']', AsciiColorRole::Text, resources)
}

fn push_optional_data_number(
    line: &mut impl DisclosureLine,
    value: Option<f64>,
    resources: &ResourceContext,
) -> Result<()> {
    match value {
        Some(value) => line.push_text(&format_data_number(value), AsciiColorRole::Text, resources),
        None => line.push_text("none", AsciiColorRole::Text, resources),
    }
}

fn write_band_domain_disclosure_line(
    line: &mut impl DisclosureLine,
    model: &XyChartView<'_>,
    plan: &TerminalChartPlan<'_>,
    resources: &mut ResourceContext,
) -> Result<()> {
    let crate::xychart::plot::AxisPlan::Band { .. } = &plan.x_axis else {
        return Ok(());
    };
    let merman_model::families::xy_chart::XyChartAxisModel::Band { categories, .. } = &model.x_axis
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

/// A row is a complete renderer-owned record; authored text cannot introduce another row.
/// Measuring and painting replay this same borrowed plan without retaining a wide intermediate.
enum ValueDisclosureRow {
    Header,
    Sample(usize),
    OrphanHeader,
    Orphan(usize),
    End,
}

struct ValueDisclosure<'plan, 'model> {
    series: &'plan SeriesPlan<'model>,
    plan: &'plan TerminalChartPlan<'model>,
    plot: &'plan merman_model::families::xy_chart::XyChartPlotModel,
    chars: ChartChars,
}

impl<'plan, 'model> ValueDisclosure<'plan, 'model> {
    fn new(
        model: &'plan XyChartView<'model>,
        series: &'plan SeriesPlan<'model>,
        plan: &'plan TerminalChartPlan<'model>,
        chars: ChartChars,
    ) -> Result<Self> {
        let plot = model
            .plots
            .get(series.series_index)
            .ok_or_else(invalid_disclosure_plan)?;
        Ok(Self {
            series,
            plan,
            plot,
            chars,
        })
    }

    fn rows(&self) -> impl Iterator<Item = ValueDisclosureRow> {
        let orphan_start = self.series.data.len().min(self.plot.point_labels.len());
        std::iter::once(ValueDisclosureRow::Header)
            .chain((0..self.series.data.len()).map(ValueDisclosureRow::Sample))
            .chain(std::iter::once(ValueDisclosureRow::OrphanHeader))
            .chain((orphan_start..self.plot.point_labels.len()).map(ValueDisclosureRow::Orphan))
            .chain((orphan_start < self.plot.point_labels.len()).then_some(ValueDisclosureRow::End))
    }

    fn write_row(
        &self,
        line: &mut impl DisclosureLine,
        row: ValueDisclosureRow,
        resources: &mut ResourceContext,
    ) -> Result<()> {
        resources.charge_layout_work(1)?;
        line.push_text("values: ", AsciiColorRole::Text, resources)?;
        match row {
            ValueDisclosureRow::Header => self.write_header(line, resources),
            ValueDisclosureRow::Sample(index) => self.write_sample(line, index, resources),
            ValueDisclosureRow::OrphanHeader => {
                line.push_text("] orphanPointLabels=[", AsciiColorRole::Text, resources)?;
                if self.plot.point_labels.len() <= self.series.data.len() {
                    line.push_char(']', AsciiColorRole::Text, resources)?;
                }
                Ok(())
            }
            ValueDisclosureRow::Orphan(index) => {
                line.push_text("  ", AsciiColorRole::Text, resources)?;
                push_disclosure_value(line, &self.plot.point_labels[index], resources)?;
                if index + 1 < self.plot.point_labels.len() {
                    line.push_char(',', AsciiColorRole::Text, resources)?;
                }
                Ok(())
            }
            ValueDisclosureRow::End => line.push_char(']', AsciiColorRole::Text, resources),
        }
    }

    fn write_header(
        &self,
        line: &mut impl DisclosureLine,
        resources: &ResourceContext,
    ) -> Result<()> {
        let series = self.series;
        line.push_char(
            self.chars.series_symbol(series),
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
        match series.title {
            Some(title) => {
                line.push_char(' ', AsciiColorRole::Text, resources)?;
                push_disclosure_field(line, "title", title, resources)?;
            }
            None => line.push_text(" title=none", AsciiColorRole::Text, resources)?,
        }
        line.push_text(" samples=[", AsciiColorRole::Text, resources)
    }

    fn write_sample(
        &self,
        line: &mut impl DisclosureLine,
        index: usize,
        resources: &ResourceContext,
    ) -> Result<()> {
        let datum = &self.series.data[index];
        line.push_text("  {index=", AsciiColorRole::Text, resources)?;
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
        match self.plot.point_labels.get(index) {
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
                    .is_some_and(|value| !self.plan.y_range.contains(value))
            {
                "true"
            } else {
                "false"
            },
            AsciiColorRole::Text,
            resources,
        )?;
        line.push_char('}', AsciiColorRole::Text, resources)?;
        if index + 1 < self.series.data.len() {
            line.push_char(',', AsciiColorRole::Text, resources)?;
        }
        Ok(())
    }
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
