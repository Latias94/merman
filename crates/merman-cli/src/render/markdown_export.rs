use super::execute::{ExecutedMetadata, execute_graphical};
use super::prepare::PreparedGraphicalRender;
use crate::error::CliError;
use crate::markdown::{MarkdownChart, MarkdownImage};
use crate::resources::CheckedBytes;
use crate::runtime::SharedWriter;
use crate::transaction::StageSlot;
use merman::OperationControl;
#[cfg(feature = "parallel-markdown")]
use rayon::prelude::*;
use std::sync::Mutex;

pub(crate) struct MarkdownRenderContext<'a> {
    renderer: &'a PreparedGraphicalRender,
    control: &'a OperationControl,
    staged_bytes: &'a Mutex<CheckedBytes>,
    stderr: &'a SharedWriter,
    #[cfg(feature = "parallel-markdown")]
    jobs: usize,
}

impl<'a> MarkdownRenderContext<'a> {
    pub(crate) fn new(
        renderer: &'a PreparedGraphicalRender,
        control: &'a OperationControl,
        staged_bytes: &'a Mutex<CheckedBytes>,
        stderr: &'a SharedWriter,
        #[cfg(feature = "parallel-markdown")] jobs: usize,
    ) -> Self {
        Self {
            renderer,
            control,
            staged_bytes,
            stderr,
            #[cfg(feature = "parallel-markdown")]
            jobs,
        }
    }
}

pub(crate) fn render_charts(
    context: &MarkdownRenderContext<'_>,
    charts: &[MarkdownChart<'_>],
    slots: Vec<StageSlot>,
    urls: Vec<String>,
) -> Result<Vec<MarkdownImage>, CliError> {
    debug_assert_eq!(charts.len(), slots.len());
    debug_assert_eq!(charts.len(), urls.len());

    #[cfg(feature = "parallel-markdown")]
    if charts.len() > 1 && context.jobs > 1 {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(context.jobs)
            .build()
            .map_err(|error| {
                CliError::InvalidInput(format!("failed to configure Markdown render jobs: {error}"))
            })?;
        let rendered = pool.install(|| {
            charts
                .par_iter()
                .zip(slots.into_par_iter())
                .zip(urls.into_par_iter())
                .enumerate()
                .map(|(index, ((chart, slot), url))| {
                    render_chart(context, chart, slot, url, chart_number(index))
                })
                .collect::<Vec<_>>()
        });
        return rendered.into_iter().collect();
    }

    charts
        .iter()
        .zip(slots)
        .zip(urls)
        .enumerate()
        .map(|(index, ((chart, slot), url))| {
            render_chart(context, chart, slot, url, chart_number(index))
        })
        .collect()
}

fn render_chart(
    context: &MarkdownRenderContext<'_>,
    chart: &MarkdownChart<'_>,
    slot: StageSlot,
    url: String,
    chart_index: u64,
) -> Result<MarkdownImage, CliError> {
    let rendered = (|| {
        crate::operation::checkpoint(context.control, merman::OperationPhase::Layout)?;
        let artifact = execute_graphical(
            context.renderer,
            chart.definition(),
            context.control,
            context.stderr,
        )?;
        crate::operation::checkpoint(context.control, merman::OperationPhase::Emit)?;
        let ExecutedMetadata { title, desc } =
            artifact.stage_into(slot, context.staged_bytes, context.control)?;
        Ok(MarkdownImage {
            url,
            title,
            alt: desc.unwrap_or_else(|| "diagram".to_string()),
        })
    })();
    rendered.map_err(|error| CliError::markdown_chart(chart_index, chart.location(), error))
}

fn chart_number(index: usize) -> u64 {
    u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1)
}
