use super::executor::RenderRequest;
use crate::error::CliError;
use crate::io::OutputTarget;
use crate::io::write_file;
use crate::markdown::{self, MarkdownImage};
use crate::render::plan::RenderPlan;
use rayon::prelude::*;
use std::path::Path;

impl RenderPlan {
    pub(super) fn is_mmdc_markdown_input(&self) -> bool {
        matches!(self.mode, super::plan::RenderMode::MmdcCompat)
            && self
                .input
                .as_deref()
                .filter(|path| *path != "-")
                .map(|path| markdown::is_markdown_path(Path::new(path)))
                .unwrap_or(false)
    }
}

impl<'a> RenderRequest<'a> {
    pub(super) fn render_markdown(&self, text: &str) -> Result<(), CliError> {
        if self.plan.format.is_text() {
            return Err(CliError::InvalidOutput(
                "Markdown input does not support ASCII/Unicode output".to_string(),
            ));
        }

        let output_path = match self.plan.output.as_ref() {
            Some(OutputTarget::File(path)) => path.as_path(),
            None | Some(OutputTarget::Stdout) => {
                return Err(CliError::InvalidOutput(
                    "Cannot use `stdout` with markdown input".to_string(),
                ));
            }
        };

        let charts = markdown::extract_charts(text);

        if charts.is_empty() {
            self.info("No mermaid charts found in Markdown input");
        } else {
            self.info(&format!(
                "Found {} mermaid charts in Markdown input",
                charts.len()
            ));
        }

        let images = self.render_markdown_charts(output_path, &charts)?;

        if markdown::is_markdown_path(output_path) {
            let rewritten = markdown::replace_charts_with_images(text, &images);
            write_file(output_path, rewritten.as_bytes())?;
            self.info(&format!(" ✅ {}", output_path.display()));
        }

        Ok(())
    }

    fn render_markdown_charts(
        &self,
        output_path: &Path,
        charts: &[markdown::MarkdownChart],
    ) -> Result<Vec<MarkdownImage>, CliError> {
        if charts.len() <= 1 || self.plan.jobs == 1 {
            return charts
                .iter()
                .enumerate()
                .map(|(index, chart)| self.render_markdown_chart(output_path, index, chart))
                .collect();
        }

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.plan.jobs)
            .build()
            .map_err(|err| {
                CliError::InvalidInput(format!("failed to configure Markdown render jobs: {err}"))
            })?;

        pool.install(|| {
            charts
                .par_iter()
                .enumerate()
                .map(|(index, chart)| self.render_markdown_chart(output_path, index, chart))
                .collect()
        })
    }

    fn render_markdown_chart(
        &self,
        output_path: &Path,
        index: usize,
        chart: &markdown::MarkdownChart,
    ) -> Result<MarkdownImage, CliError> {
        let output_file = markdown::numbered_output_path(
            output_path,
            index + 1,
            self.plan.format,
            self.plan.artefacts.as_deref(),
        );
        let artifact = self.render_artifact(&chart.definition)?;
        write_file(&output_file, &artifact.bytes)?;

        let url = markdown::relative_markdown_url(output_path, &output_file)?;
        self.info(&format!(" ✅ {url}"));
        Ok(MarkdownImage {
            url,
            title: artifact.title,
            alt: artifact.desc.unwrap_or_else(|| "diagram".to_string()),
        })
    }
}
