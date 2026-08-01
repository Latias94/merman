use super::execute::{ExecutedMetadata, execute_graphical};
use super::prepare::PreparedGraphicalRender;
use crate::error::CliError;
use crate::markdown::{MarkdownChart, MarkdownImage};
use crate::resources::CheckedBytes;
use crate::runtime::SharedWriter;
use crate::transaction::StageSlot;
#[cfg(feature = "parallel-markdown")]
use rayon::prelude::*;
use std::sync::Mutex;

pub(crate) fn render_charts(
    renderer: &PreparedGraphicalRender,
    charts: &[MarkdownChart<'_>],
    slots: Vec<StageSlot>,
    urls: Vec<String>,
    staged_bytes: &Mutex<CheckedBytes>,
    stderr: &SharedWriter,
    #[cfg(feature = "parallel-markdown")] jobs: usize,
) -> Result<Vec<MarkdownImage>, CliError> {
    debug_assert_eq!(charts.len(), slots.len());
    debug_assert_eq!(charts.len(), urls.len());

    #[cfg(feature = "parallel-markdown")]
    if charts.len() > 1 && jobs > 1 {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(jobs)
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
                    render_chart(
                        renderer,
                        chart,
                        slot,
                        url,
                        staged_bytes,
                        stderr,
                        chart_number(index),
                    )
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
            render_chart(
                renderer,
                chart,
                slot,
                url,
                staged_bytes,
                stderr,
                chart_number(index),
            )
        })
        .collect()
}

fn render_chart(
    renderer: &PreparedGraphicalRender,
    chart: &MarkdownChart<'_>,
    slot: StageSlot,
    url: String,
    staged_bytes: &Mutex<CheckedBytes>,
    stderr: &SharedWriter,
    chart_index: u64,
) -> Result<MarkdownImage, CliError> {
    let rendered = (|| {
        let artifact = execute_graphical(renderer, chart.definition(), stderr)?;
        let ExecutedMetadata { title, desc } = artifact.stage_into(slot, staged_bytes)?;
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
