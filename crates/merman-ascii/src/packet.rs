use crate::Result;
use crate::error::AsciiError;
use crate::options::AsciiRenderOptions;
use crate::resource::ResourceContext;
use crate::safe_text::BudgetedTextDocument;
use merman_core::diagrams::packet::PacketDiagramRenderModel;

pub fn render_packet_diagram(
    model: &PacketDiagramRenderModel,
    options: &AsciiRenderOptions,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);
    validate_packet_blocks(model, document.resources_mut())?;

    push_optional_framed_line(&mut document, "title", model.title.as_deref())?;
    push_optional_framed_line(&mut document, "accTitle", model.acc_title.as_deref())?;
    push_optional_framed_line(&mut document, "accDescr", model.acc_descr.as_deref())?;

    for (row_idx, row) in model.packet.iter().enumerate() {
        document.resources_mut().charge_layout_work(1)?;
        document.push_line_with(|line| line.write_fmt(format_args!("row {}:", row_idx + 1)))?;
        for block in row {
            document.resources_mut().charge_layout_work(1)?;
            document.push_line_with(|line| {
                line.write_fmt(format_args!(
                    "  - range=[{}..{}] bits={} label(bytes={})=",
                    block.start,
                    block.end,
                    block.bits,
                    block.label.len(),
                ))?;
                line.push_quoted_text(&block.label)
            })?;
        }
    }

    document.finish(options)
}

fn validate_packet_blocks(
    model: &PacketDiagramRenderModel,
    resources: &mut ResourceContext,
) -> Result<()> {
    let block_count = model
        .packet
        .iter()
        .try_fold(0usize, |total, row| total.checked_add(row.len()));
    let block_count = block_count.ok_or_else(|| resources.work_overflow())?;
    resources.charge_layout_work(block_count)?;

    for row in &model.packet {
        for block in row {
            if block.start > block.end {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "packet",
                    feature: "packet blocks with end before start",
                });
            }
            let expected_bits = block
                .end
                .checked_sub(block.start)
                .and_then(|value| value.checked_add(1))
                .ok_or(AsciiError::UnsupportedFeature {
                    diagram_type: "packet",
                    feature: "packet block range overflow",
                })?;
            if block.bits <= 0 {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "packet",
                    feature: "packet blocks must have positive bit counts",
                });
            }
            if block.bits != expected_bits {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "packet",
                    feature: "packet block bit count does not match inclusive range",
                });
            }
        }
    }
    Ok(())
}

fn push_optional_framed_line(
    document: &mut BudgetedTextDocument,
    key: &str,
    value: Option<&str>,
) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    document.push_line_with(|line| {
        line.write_fmt(format_args!("{key}(bytes={})=", value.len()))?;
        line.push_quoted_text(value)
    })
}
