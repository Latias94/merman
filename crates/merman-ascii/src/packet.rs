//! Packet's fixed-scale spatial view and exact database records.

mod grid;

use crate::operation::AsciiExecution;
use crate::options::AsciiRenderOptions;
use crate::resource::ResourceContext;
use crate::safe_text::{BudgetedTextDocument, push_optional_document_field};
use crate::{AsciiError, AsciiProjection, Result};
use merman_model::families::packet::{PacketDiagramModel, validate_packet_bits_per_row};
use merman_model::{MermaidConfig, OperationPhase};

#[derive(Clone, Copy)]
struct PacketSettings {
    bits_per_row: Option<usize>,
    descending: bool,
    show_bits: bool,
}

impl PacketSettings {
    fn resolve(config: Option<&MermaidConfig>) -> Result<Self> {
        let width = validate_packet_bits_per_row(
            config.and_then(|config| config.as_value().get("packet")?.get("bitsPerRow")?.as_f64()),
        )
        .map_err(|_| AsciiError::InvalidOption {
            field: "packet.bitsPerRow",
            message: "must be a finite number greater than or equal to one",
        })?;
        Ok(Self {
            bits_per_row: (width <= 64.0 && width.fract() == 0.0).then_some(width as usize),
            descending: config.and_then(|config| config.get_str("packet.bitOrder"))
                == Some("descending"),
            show_bits: config
                .and_then(|config| config.get_bool("packet.showBits"))
                .unwrap_or(true),
        })
    }
}

pub(super) fn render_packet_diagram(
    model: &PacketDiagramModel,
    options: &AsciiRenderOptions,
    config: Option<&MermaidConfig>,
    execution: AsciiExecution<'_>,
) -> Result<crate::output::RenderedPrimary> {
    let settings = PacketSettings::resolve(config)?;
    let layout_resources = execution.new_resource_context(OperationPhase::Layout);
    let last_bit = grid::last_bit(model, settings, &layout_resources)?;
    let mut document = BudgetedTextDocument::from_resources(layout_resources, options);
    let wrap_width = last_bit.map(|_| execution.max_width().unwrap_or(80));
    write_metadata(model, &mut document, wrap_width)?;
    if let Some(last_bit) = last_bit {
        grid::write_grid(model, last_bit, settings, options, &mut document, execution)?;
        document.push_line("Field records:")?;
    }
    execution.rebind_resource_context(document.resources_mut(), OperationPhase::Emit);
    write_records(model, &mut document, wrap_width, execution)?;
    let projection = if last_bit.is_some() {
        AsciiProjection::Diagrammatic
    } else {
        AsciiProjection::StructuredText
    };
    document
        .finish_with_execution(execution)
        .map(|text| crate::output::RenderedPrimary::new(text, projection))
}

fn write_metadata(
    model: &PacketDiagramModel,
    document: &mut BudgetedTextDocument,
    wrap_width: Option<usize>,
) -> Result<()> {
    for (key, value) in [
        ("title", model.title.as_deref()),
        ("accTitle", model.acc_title.as_deref()),
        ("accDescr", model.acc_descr.as_deref()),
    ] {
        if let (Some(width), Some(value)) = (wrap_width, value) {
            document.push_wrapped_prefixed_line_with("", "  ", width, |line| {
                line.write_fmt(format_args!("{key}(bytes={})=", value.len()))?;
                line.push_quoted_text(value)
            })?;
        } else {
            push_optional_document_field(document, key, value)?;
        }
    }
    Ok(())
}

fn write_records(
    model: &PacketDiagramModel,
    document: &mut BudgetedTextDocument,
    wrap_width: Option<usize>,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    for (row_idx, row) in model.packet.iter().enumerate() {
        execution.checkpoint(merman_model::OperationPhase::Emit)?;
        document.resources_mut().charge_layout_work(1)?;
        document.push_line_with(|line| line.write_fmt(format_args!("row {}:", row_idx + 1)))?;
        for block in row {
            execution.checkpoint(merman_model::OperationPhase::Emit)?;
            document.resources_mut().charge_layout_work(1)?;
            let mut start = ryu_js::Buffer::new();
            let mut end = ryu_js::Buffer::new();
            let mut bits = ryu_js::Buffer::new();
            let (start, end, bits) = (
                start.format(block.start),
                end.format(block.end),
                bits.format(block.bits),
            );
            if let Some(width) = wrap_width {
                document.push_wrapped_prefixed_line_with("  - ", "    ", width, |line| {
                    line.write_fmt(format_args!(
                        "range=[{start}..{end}] bits={bits} label(bytes={})=",
                        block.label.len(),
                    ))?;
                    line.push_quoted_text(&block.label)
                })?;
            } else {
                document.push_line_with(|line| {
                    line.write_fmt(format_args!(
                        "  - range=[{}..{}] bits={} label(bytes={})=",
                        start,
                        end,
                        bits,
                        block.label.len(),
                    ))?;
                    line.push_quoted_text(&block.label)
                })?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};

    #[test]
    fn packet_grid_work_and_document_usage_accept_exact_and_reject_one_below() {
        use merman_model::families::packet::PacketBlock;

        let model = PacketDiagramModel {
            title: Some("Header".into()),
            packet: vec![vec![PacketBlock {
                start: 0.0,
                end: 39.0,
                bits: 40.0,
                label: "请求 👩‍💻".into(),
            }]],
            ..Default::default()
        };
        let options = AsciiRenderOptions::ascii();
        let policy = AsciiResourcePolicy::default();
        let ledger = ResourceContext::new(policy);
        render_packet_diagram(
            &model,
            &options,
            None,
            AsciiExecution::for_test(&policy).with_render_ledger(&ledger),
        )
        .unwrap();
        for (limit, exact) in [
            (
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                ledger.layout_work_used(),
            ),
            (
                AsciiResourceLimitId::MaxDocumentCells,
                ledger.document_cells_used(),
            ),
            (AsciiResourceLimitId::MaxGraphemeBytes, "👩‍💻".len()),
        ] {
            let exact_policy = policy.with_limit(limit, exact).unwrap();
            render_packet_diagram(
                &model,
                &options,
                None,
                AsciiExecution::for_test(&exact_policy),
            )
            .unwrap();
            let short_policy = policy.with_limit(limit, exact - 1).unwrap();
            assert!(matches!(
                render_packet_diagram(&model, &options, None, AsciiExecution::for_test(&short_policy)),
                Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == limit
            ));
        }
    }

    #[test]
    fn canonical_packet_matches_full_row_and_metadata_output() {
        use merman_core::{MermaidEngine, ParseOptions, SemanticParser};
        use merman_model::SemanticDiagramModel;
        use merman_model::presentation::RenderSemanticModel;

        let options = AsciiRenderOptions::ascii();
        let policy = AsciiResourcePolicy::default();
        for source in [
            "packet\ntitle Header\naccTitle: Accessible\naccDescr: Complete packet\n0-7: \"Type\"\n8-39: \"Payload\"\n",
            "packet\n",
        ] {
            let canonical = SemanticParser::new().parse_diagram_sync(source).unwrap();
            let SemanticDiagramModel::Packet(model) = &canonical.model else {
                panic!("Packet model")
            };
            let full = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let RenderSemanticModel::Packet(full) = full.model() else {
                panic!("Packet model")
            };
            assert!(std::ptr::eq(full.packet.as_ptr(), full.packet.as_ptr()));
            let actual =
                render_packet_diagram(model, &options, None, AsciiExecution::for_test(&policy))
                    .unwrap();
            let expected =
                render_packet_diagram(full, &options, None, AsciiExecution::for_test(&policy))
                    .unwrap();
            assert_eq!(actual, expected, "{source}");
        }
    }
}
