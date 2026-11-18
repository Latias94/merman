//! Pie values on a shared percentage scale, with complete keyed labels and numeric records.

use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use crate::safe_text::{
    BudgetedTextDocument, terminal_line_display_width, visit_quoted_terminal_text,
};
use crate::{
    AsciiCharset, AsciiColorMode, AsciiDiagram, AsciiProjection, AsciiRenderOptions,
    AsciiSourceContext, Result, TerminalWidthProfile,
};
use merman_model::OperationPhase;
use merman_model::families::pie::PieDiagramModel;
use std::fmt;

const BAR_CELLS: usize = 50;
const RECORD_WIDTH: usize = 80;

// A finite binary64 value needs at most 2098 bits in units of 2^-1074. Summing at
// most usize::MAX sections and multiplying by at most 2000 needs these extra bits.
const SUM_WORDS: usize = (2098 + usize::BITS as usize + 11).div_ceil(64);

#[derive(Clone, Debug)]
struct ExactSum([u64; SUM_WORDS]);

impl ExactSum {
    fn zero() -> Self {
        Self([0; SUM_WORDS])
    }

    fn add(&mut self, value: f64) {
        let bits = value.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as usize;
        let fraction = bits & ((1u64 << 52) - 1);
        let (significand, shift) = if exponent == 0 {
            (fraction, 0)
        } else {
            (fraction | (1u64 << 52), exponent - 1)
        };
        let mut carry = (significand as u128) << (shift % 64);
        for word in &mut self.0[shift / 64..] {
            let sum = *word as u128 + (carry & u64::MAX as u128);
            *word = sum as u64;
            carry = (carry >> 64) + (sum >> 64);
            if carry == 0 {
                break;
            }
        }
        debug_assert_eq!(carry, 0);
    }

    fn times(&self, multiplier: u64) -> Self {
        let mut product = Self::zero();
        let mut carry = 0u128;
        for (output, word) in product.0.iter_mut().zip(&self.0) {
            let value = *word as u128 * multiplier as u128 + carry;
            *output = value as u64;
            carry = value >> 64;
        }
        debug_assert_eq!(carry, 0);
        product
    }

    fn at_least(&self, other: &Self) -> bool {
        self.0.iter().rev().cmp(other.0.iter().rev()).is_ge()
    }
}

#[derive(Clone, Debug)]
struct Proportions {
    exact_total: ExactSum,
}

impl Proportions {
    fn measure(
        model: &PieDiagramModel,
        resources: &ResourceContext,
    ) -> Result<std::result::Result<Self, &'static str>> {
        if model.sections.is_empty() {
            return Ok(Err("no sections"));
        }
        let mut positive = false;
        let mut exact_total = ExactSum::zero();
        for section in &model.sections {
            resources.checkpoint()?;
            resources.charge_layout_work(SUM_WORDS)?;
            if !section.value.is_finite() || section.value < 0.0 {
                return Ok(Err("values must be finite and nonnegative"));
            }
            positive |= section.value > 0.0;
            exact_total.add(section.value);
        }
        if !positive {
            return Ok(Err(
                "all section values are zero; percentages are undefined",
            ));
        }
        Ok(Ok(Self { exact_total }))
    }

    fn rounded_units(
        &self,
        value: f64,
        scale: usize,
        resources: &ResourceContext,
    ) -> Result<usize> {
        // Bars use 50 units; percentages use 1000 tenths of a percent. Compare exact
        // thresholds instead of rounding an approximate quotient. This also avoids total
        // overflow, subnormal underflow, and division errors near ties. Ties round up.
        debug_assert!(scale <= 1000);
        resources.charge_layout_work(SUM_WORDS * 2)?;
        let mut numerator = ExactSum::zero();
        numerator.add(value);
        let numerator = numerator.times(2 * scale as u64);
        let (mut low, mut high) = (0, scale);
        while low < high {
            resources.checkpoint()?;
            resources.charge_layout_work(SUM_WORDS * 2)?;
            let middle = (low + high).div_ceil(2);
            if numerator.at_least(&self.exact_total.times(2 * middle as u64 - 1)) {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        Ok(low)
    }
}

/// The canonical f64 value, rather than JSON's null replacement for non-finite numbers.
/// Keep negative zero explicit; NaN payload bits are disclosed independently in the records.
pub(crate) struct ExactValue(pub(crate) f64);

impl fmt::Display for ExactValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0.0 && self.0.is_sign_negative() {
            formatter.write_str("-0")
        } else {
            formatter.write_str(ryu_js::Buffer::new().format(self.0))
        }
    }
}

pub(crate) fn render(
    model: &PieDiagramModel,
    options: &AsciiRenderOptions,
    source: AsciiSourceContext<'_>,
    execution: AsciiExecution<'_>,
) -> Result<crate::output::RenderedPrimary> {
    let resources = execution
        .new_resource_context(OperationPhase::Layout)
        .scoped();
    let proportions = Proportions::measure(model, &resources)?;
    let projection = if proportions.is_ok() {
        AsciiProjection::Diagrammatic
    } else {
        AsciiProjection::StructuredText
    };
    let mut chart = BudgetedTextDocument::from_resources(
        resources,
        &options.with_color_mode(AsciiColorMode::Plain),
    );
    if let Some(title) = model.title.as_deref().or(source.title) {
        chart.push_wrapped_prefixed_line_with("", "  ", RECORD_WIDTH, |line| {
            line.push_quoted_text(title)
        })?;
        chart.push_line("")?;
    }
    match proportions {
        Ok(proportions) => write_bars(model, proportions, options, &mut chart)?,
        Err(reason) => chart.push_line_with(|line| {
            line.write_fmt(format_args!("Proportions unavailable: {reason}."))
        })?,
    }
    let chart = chart.finish()?;
    crate::output::render_reading_primary(
        &AsciiDiagram::Pie(model),
        source,
        options,
        execution,
        Some(&chart),
    )
    .map(|text| crate::output::RenderedPrimary::records(text, projection))
}

fn write_bars(
    model: &PieDiagramModel,
    proportions: Proportions,
    options: &AsciiRenderOptions,
    document: &mut BudgetedTextDocument,
) -> Result<()> {
    let key_width = 1 + model.sections.len().to_string().len();
    document
        .resources_mut()
        .grid_extent(BAR_CELLS, model.sections.len())?;
    document.push_line("Pie proportions (50 cells = 100%)")?;
    document.push_line("Bars round to 2% cells. Full labels and exact values follow by key.")?;
    document.push_line_with(|line| {
        line.write_fmt(format_args!(
            "{:>width$}0%                       50%                      100%",
            "",
            width = key_width + 2
        ))
    })?;
    let glyph = match options.structural_charset() {
        AsciiCharset::Ascii => '#',
        AsciiCharset::Unicode => '█',
    };
    let mut has_tiny = false;
    let mut has_keyed_label = false;
    for (index, section) in model.sections.iter().enumerate() {
        document.resources_mut().checkpoint()?;
        document.resources_mut().charge_layout_work(BAR_CELLS)?;
        let cells =
            proportions.rounded_units(section.value, BAR_CELLS, document.resources_mut())?;
        let key = format!("P{}", index + 1);
        let tiny = cells == 0 && section.value > 0.0;
        has_tiny |= tiny;
        let percentage =
            proportions.rounded_units(section.value, 1000, document.resources_mut())?;
        let percentage = if percentage == 0 && section.value > 0.0 {
            "<0.1%".to_string()
        } else {
            format!("{}.{}%", percentage / 10, percentage % 10)
        };
        let exact = ExactValue(section.value).to_string();
        let inline_limit = RECORD_WIDTH.saturating_sub(
            key_width
                + 2
                + BAR_CELLS
                + 2
                + 6
                + 1
                + if model.show_data { exact.len() + 3 } else { 0 },
        );
        let inline = quoted_width(
            &section.label,
            options.terminal_width_profile,
            document.resources_mut(),
        )? <= inline_limit;
        has_keyed_label |= !inline;
        document.push_line_with(|line| {
            line.write_fmt(format_args!("{key:>key_width$} |"))?;
            for cell in 0..BAR_CELLS {
                line.write_fmt(format_args!(
                    "{}",
                    if cell < cells {
                        glyph
                    } else if cell == 0 && tiny {
                        '*'
                    } else {
                        ' '
                    }
                ))?;
            }
            line.write_fmt(format_args!("| {percentage:>6} "))?;
            if inline {
                line.push_quoted_text(&section.label)?;
            } else {
                line.write_fmt(format_args!("[P{}]", index + 1))?;
            }
            if model.show_data {
                line.write_fmt(format_args!(" [{exact}]"))?;
            }
            Ok(())
        })?;
    }
    if has_tiny {
        document.push_line("* = a positive share below one rounded cell; it is not a 2% bar.")?;
    }
    if has_keyed_label {
        document.push_line("[P#] refers to the complete label in Pie data.")?;
    }
    Ok(())
}

fn quoted_width(
    text: &str,
    profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<usize> {
    let mut width = 0;
    visit_quoted_terminal_text(text, resources, |fragment| {
        resources.charge_layout_work(fragment.len())?;
        width =
            resources.checked_grid_add(width, terminal_line_display_width(fragment, profile))?;
        Ok(())
    })?;
    Ok(width)
}
