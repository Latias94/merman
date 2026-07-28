use super::executor::RenderRequest;
use crate::error::CliError;
use crate::io::{OutputTarget, write_file};
use crate::markdown::{self, MarkdownImage};
use crate::render::plan::RenderPlan;
use crate::resources::CountLedgerKind;
#[cfg(feature = "parallel-markdown")]
use rayon::prelude::*;
use std::path::{Path, PathBuf};

struct RenderedMarkdownChart {
    output_file: PathBuf,
    bytes: Vec<u8>,
    image: MarkdownImage,
}

impl RenderPlan {
    pub(super) fn is_markdown_input(&self) -> bool {
        match self.mode {
            super::plan::RenderMode::MmdcCompat => self
                .input
                .as_deref()
                .filter(|path| *path != Path::new("-"))
                .map(markdown::is_markdown_path)
                .unwrap_or(false),
            super::plan::RenderMode::NativeBatch => true,
            super::plan::RenderMode::NativeSingle => false,
        }
    }
}

impl<'a> RenderRequest<'a> {
    pub(super) fn render_markdown(&self, text: &str) -> Result<(), CliError> {
        #[cfg(feature = "ascii")]
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

        let mut chart_counter = self
            .plan
            .resources
            .checked_count(CountLedgerKind::MarkdownCharts);
        let charts = match markdown::extract_charts_limited(text, chart_counter.max()) {
            Ok(charts) => charts,
            Err(error) => {
                let limit_error = chart_counter
                    .try_add(error.observed)
                    .expect_err("the scanner reported a count above this same policy limit");
                return Err(limit_error.into());
            }
        };
        let chart_count = u64::try_from(charts.len())
            .map_err(|_| CliError::InvalidInput("Markdown chart count overflow".to_string()))?;
        chart_counter.try_add(chart_count)?;

        if charts.is_empty() {
            self.info("No mermaid charts found in Markdown input");
        } else {
            self.info(&format!(
                "Found {} mermaid charts in Markdown input",
                charts.len()
            ));
        }

        let output_files = (1..=charts.len())
            .map(|index| {
                markdown::numbered_output_path(
                    output_path,
                    index,
                    self.plan.format,
                    self.plan.artefacts.as_deref(),
                )
            })
            .collect::<Vec<_>>();
        let writes_document = markdown::is_markdown_path(output_path);

        let rendered_charts = self.render_markdown_charts(output_path, &charts, &output_files)?;
        let images = rendered_charts
            .iter()
            .map(|rendered| rendered.image.clone())
            .collect::<Vec<_>>();
        let rewritten = if writes_document {
            let rewritten = markdown::replace_charts_with_images(text, &images);
            self.charge_staged_bytes(rewritten.len())?;
            Some(rewritten)
        } else {
            None
        };

        for rendered in &rendered_charts {
            write_file(
                &rendered.output_file,
                &rendered.bytes,
                &self.plan.publications,
            )?;
        }
        if let Some(rewritten) = rewritten.as_deref() {
            write_file(output_path, rewritten.as_bytes(), &self.plan.publications)?;
        }

        for rendered in &rendered_charts {
            self.info(&format!(" ✅ {}", rendered.image.url));
        }
        if writes_document {
            self.info(&format!(" ✅ {}", crate::error::safe_path(output_path)));
        }

        Ok(())
    }

    fn render_markdown_charts(
        &self,
        output_path: &Path,
        charts: &[markdown::MarkdownChart],
        output_files: &[PathBuf],
    ) -> Result<Vec<RenderedMarkdownChart>, CliError> {
        if charts.len() <= 1 || self.plan.markdown_jobs() == 1 {
            return charts
                .iter()
                .zip(output_files)
                .map(|(chart, output_file)| {
                    self.render_markdown_chart(output_path, output_file, chart)
                })
                .collect();
        }

        #[cfg(feature = "parallel-markdown")]
        {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(self.plan.markdown_jobs())
                .build()
                .map_err(|err| {
                    CliError::InvalidInput(format!(
                        "failed to configure Markdown render jobs: {err}"
                    ))
                })?;

            pool.install(|| {
                charts
                    .par_iter()
                    .zip(output_files.par_iter())
                    .map(|(chart, output_file)| {
                        self.render_markdown_chart(output_path, output_file, chart)
                    })
                    .collect()
            })
        }

        #[cfg(not(feature = "parallel-markdown"))]
        {
            charts
                .iter()
                .zip(output_files)
                .map(|(chart, output_file)| {
                    self.render_markdown_chart(output_path, output_file, chart)
                })
                .collect()
        }
    }

    fn render_markdown_chart(
        &self,
        output_path: &Path,
        output_file: &Path,
        chart: &markdown::MarkdownChart,
    ) -> Result<RenderedMarkdownChart, CliError> {
        let artifact = self.render_artifact(&chart.definition)?;
        self.charge_staged_bytes(artifact.bytes.len())?;

        let url = markdown::relative_markdown_url(output_path, output_file)?;
        Ok(RenderedMarkdownChart {
            output_file: output_file.to_path_buf(),
            bytes: artifact.bytes,
            image: MarkdownImage {
                url,
                title: artifact.title,
                alt: artifact.desc.unwrap_or_else(|| "diagram".to_string()),
            },
        })
    }
}
