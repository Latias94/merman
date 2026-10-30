use crate::diagnostics::DiagnosticSink;
use crate::error::CliError;
use crate::runtime::SharedWriter;
use merman::{RenderEvidence, TargetAdmissionReceipt, ThemeEvidenceStatus};
use serde::Serialize;
#[cfg(feature = "markdown")]
use std::collections::BTreeMap;
use std::path::Path;
#[cfg(feature = "markdown")]
use std::sync::Mutex;

/// A CLI projection of completed renderer evidence; admission stays with the renderer.
#[derive(Serialize)]
pub(crate) struct ThemeResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    evidence: Option<ThemeEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<TargetResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct ThemeEvidence {
    family: &'static str,
    status: &'static str,
    output_mutated: bool,
    diagnostics: Vec<ThemeDiagnostic>,
}

#[derive(Serialize)]
struct ThemeDiagnostic {
    code: &'static str,
    subject: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    property: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_document: Option<&'static str>,
    source_paths: Vec<String>,
    generated: bool,
}

#[derive(Serialize)]
struct TargetResult {
    kind: &'static str,
    status: &'static str,
    reasons: Vec<&'static str>,
    font_source: &'static str,
}

impl From<&TargetAdmissionReceipt> for TargetResult {
    fn from(receipt: &TargetAdmissionReceipt) -> Self {
        Self {
            kind: receipt.artifact_kind().id(),
            status: receipt.status().id(),
            reasons: receipt.reasons().iter().map(|reason| reason.id()).collect(),
            font_source: receipt.font_source().id(),
        }
    }
}

impl ThemeResult {
    pub(crate) fn completed(evidence: &RenderEvidence, target: &TargetAdmissionReceipt) -> Self {
        Self {
            evidence: Some(ThemeEvidence {
                family: evidence.family_id().as_str(),
                status: status_id(evidence.theme_evidence().status()),
                output_mutated: evidence.theme_evidence().output_mutated(),
                diagnostics: evidence
                    .theme_diagnostics()
                    .iter()
                    .map(|diagnostic| ThemeDiagnostic {
                        code: diagnostic.code(),
                        subject: diagnostic.subject(),
                        target: diagnostic.target().map(|target| target.id()),
                        property: diagnostic.property(),
                        source_document: diagnostic.source_document(),
                        source_paths: diagnostic.source_paths().to_vec(),
                        generated: diagnostic.generated(),
                    })
                    .collect(),
            }),
            target: Some(target.into()),
            error: None,
        }
    }

    pub(crate) fn failed(error: &CliError) -> Self {
        let target = match error {
            CliError::Render(merman::RenderError::TargetAdmission(error)) => {
                Some(error.receipt().into())
            }
            _ => None,
        };
        Self {
            evidence: None,
            target,
            error: Some(error.to_string()),
        }
    }

    pub(crate) fn encode_single(self, output: Option<&Path>) -> Result<Vec<u8>, CliError> {
        encode_report(&[ReportEntry {
            chart: 1,
            output: output.map(|path| path.to_string_lossy().into_owned()),
            result: self,
        }])
    }
}

fn status_id(status: ThemeEvidenceStatus) -> &'static str {
    match status {
        ThemeEvidenceStatus::NotApplicable => "not_applicable",
        ThemeEvidenceStatus::Verified => "verified",
        ThemeEvidenceStatus::Residual => "residual",
        ThemeEvidenceStatus::Incomplete => "incomplete",
        _ => "unknown",
    }
}

pub(super) fn observe(
    requested: bool,
    quiet: bool,
    evidence: &RenderEvidence,
    target: &TargetAdmissionReceipt,
    stderr: &SharedWriter,
    chart: Option<u64>,
) {
    if quiet {
        return;
    }
    let requested = requested || evidence.theme_recipe_fingerprint().is_some();
    let diagnostics = evidence.theme_diagnostics();
    let actionable = diagnostics.iter().any(|diagnostic| {
        !matches!(
            diagnostic.code(),
            "family-unadapted" | "evidence-incomplete"
        )
    });
    if !requested && !actionable {
        return;
    }
    let diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| {
            requested
                || !matches!(
                    diagnostic.code(),
                    "family-unadapted" | "evidence-incomplete"
                )
        })
        .collect::<Vec<_>>();
    let mut details = diagnostics
        .iter()
        .take(3)
        .map(|diagnostic| {
            let mut detail = format!("{} {}", diagnostic.code(), diagnostic.subject());
            if let Some(target) = diagnostic.target() {
                detail.push_str(&format!(" ({})", target.id()));
            }
            if let Some(property) = diagnostic.property() {
                detail.push_str(&format!(" {property}"));
            }
            if let Some(path) = diagnostic.source_paths().first() {
                detail.push_str(&format!(": {path}"));
            }
            detail
        })
        .collect::<Vec<_>>();
    if diagnostics.len() > 3 {
        details.push("more details available with --theme-report PATH".to_owned());
    }
    let details = if details.is_empty() {
        String::new()
    } else {
        format!("; {}", details.join("; "))
    };
    let chart = chart
        .map(|chart| format!(" chart {chart}"))
        .unwrap_or_default();
    DiagnosticSink::new(false, stderr).info(merman::normalize_terminal_diagnostic(&format!(
        "theme{} ({}/{}): {}; target {}{}",
        chart,
        evidence.family_id().as_str(),
        target.artifact_kind().id(),
        status_id(evidence.theme_evidence().status()),
        target.status().id(),
        details,
    )));
}

#[derive(Serialize)]
struct ReportEntry {
    chart: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<String>,
    #[serde(flatten)]
    result: ThemeResult,
}

fn encode_report(results: &[ReportEntry]) -> Result<Vec<u8>, CliError> {
    #[derive(Serialize)]
    struct Report<'a> {
        schema_version: u32,
        stage: &'static str,
        results: &'a [ReportEntry],
    }
    let mut bytes = serde_json::to_vec_pretty(&Report {
        schema_version: 1,
        stage: "render",
        results,
    })
    .map_err(CliError::json_output)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(feature = "markdown")]
#[derive(Default)]
pub(crate) struct BatchThemeReport(Mutex<BTreeMap<u64, ReportEntry>>);

#[cfg(feature = "markdown")]
impl BatchThemeReport {
    pub(crate) fn record(&self, chart: u64, output: String, result: ThemeResult) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                chart,
                ReportEntry {
                    chart,
                    output: Some(output),
                    result,
                },
            );
    }

    pub(crate) fn encode(self) -> Result<Vec<u8>, CliError> {
        let results = self
            .0
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .into_values()
            .collect::<Vec<_>>();
        encode_report(&results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_render_reports_omit_unavailable_evidence_and_target_receipts() {
        let bytes = ThemeResult::failed(&CliError::NoDiagram)
            .encode_single(None)
            .unwrap();
        let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let result = &report["results"][0];
        assert!(result["error"].is_string());
        for unavailable in ["evidence", "target", "output"] {
            assert!(
                result.get(unavailable).is_none(),
                "{unavailable} must not be invented"
            );
        }
    }
}
