//! Shared execution harness for per-diagram SVG compare commands.

use crate::XtaskError;
use crate::svgdom;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct CompareRunOptions<'a> {
    pub(crate) diagram: &'a str,
    pub(crate) out_path: Option<PathBuf>,
    pub(crate) filter: Option<&'a str>,
    pub(crate) check_dom: bool,
    pub(crate) dom_mode: &'a str,
    pub(crate) dom_decimals: u32,
}

pub(crate) type CompareRunPaths = super::CompareDiagramPaths;

#[derive(Debug, Clone, Copy)]
pub(crate) struct CompareFixtureInput<'a> {
    pub(crate) stem: &'a str,
    pub(crate) fixture_path: &'a Path,
    pub(crate) upstream_svg: &'a str,
    pub(crate) text: &'a str,
    pub(crate) check_dom: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum CompareFixtureResult {
    Skipped {
        reason: String,
    },
    Rendered {
        local_svg: String,
        compare_dom: bool,
        issues: Vec<String>,
        notes: Vec<String>,
    },
    RenderedWithPolicy {
        local_svg: String,
        compare_dom: bool,
        compare_svg_when_dom_disabled: bool,
        issues: Vec<String>,
        notes: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CompareFixtureReportInput<'a> {
    pub(crate) stem: &'a str,
    pub(crate) fixture_path: &'a Path,
    pub(crate) upstream_path: &'a Path,
    pub(crate) local_out_path: &'a Path,
    pub(crate) failed: bool,
}

fn is_pinned_upstream_dir(diagram: &str, upstream_dir: &Path) -> bool {
    let pinned = crate::cmd::fixtures_root()
        .join("upstream-svgs")
        .join(diagram);
    match (fs::canonicalize(upstream_dir), fs::canonicalize(&pinned)) {
        (Ok(actual), Ok(expected)) => actual == expected,
        _ => upstream_dir == pinned,
    }
}

pub(crate) fn run_svg_compare<S, Header, Skip, Render, Report>(
    options: CompareRunOptions<'_>,
    state: &mut S,
    write_header: Header,
    skip_fixture: Skip,
    render_fixture: Render,
    write_report: Report,
) -> Result<(), XtaskError>
where
    Header: FnMut(&mut S, &mut String, &CompareRunPaths, &CompareRunOptions<'_>),
    Skip: FnMut(&mut S, &str, &CompareRunPaths) -> Option<String>,
    Render: FnMut(&mut S, &CompareFixtureInput<'_>) -> Result<CompareFixtureResult, String>,
    Report:
        FnMut(&mut S, &mut String, &CompareRunPaths, &CompareRunOptions<'_>, &[String], &[String]),
{
    run_svg_compare_with_fixture_reports(
        options,
        state,
        write_header,
        skip_fixture,
        render_fixture,
        |_, _, _| {},
        write_report,
    )
}

// These internal adapters intentionally expose each harness callback separately. Bundling five
// unrelated generic closures only to satisfy the argument-count heuristic would obscure callers.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_svg_compare_with_roots<S, Header, Skip, Render, Report>(
    options: CompareRunOptions<'_>,
    fixtures_root: Option<PathBuf>,
    upstream_root: Option<PathBuf>,
    state: &mut S,
    write_header: Header,
    skip_fixture: Skip,
    render_fixture: Render,
    write_report: Report,
) -> Result<(), XtaskError>
where
    Header: FnMut(&mut S, &mut String, &CompareRunPaths, &CompareRunOptions<'_>),
    Skip: FnMut(&mut S, &str, &CompareRunPaths) -> Option<String>,
    Render: FnMut(&mut S, &CompareFixtureInput<'_>) -> Result<CompareFixtureResult, String>,
    Report:
        FnMut(&mut S, &mut String, &CompareRunPaths, &CompareRunOptions<'_>, &[String], &[String]),
{
    run_svg_compare_with_roots_and_fixture_reports(
        options,
        fixtures_root,
        upstream_root,
        state,
        write_header,
        skip_fixture,
        render_fixture,
        |_, _, _| {},
        write_report,
    )
}

pub(crate) fn run_svg_compare_with_fixture_reports<S, Header, Skip, Render, FixtureReport, Report>(
    options: CompareRunOptions<'_>,
    state: &mut S,
    write_header: Header,
    skip_fixture: Skip,
    render_fixture: Render,
    write_fixture_report: FixtureReport,
    write_report: Report,
) -> Result<(), XtaskError>
where
    Header: FnMut(&mut S, &mut String, &CompareRunPaths, &CompareRunOptions<'_>),
    Skip: FnMut(&mut S, &str, &CompareRunPaths) -> Option<String>,
    Render: FnMut(&mut S, &CompareFixtureInput<'_>) -> Result<CompareFixtureResult, String>,
    FixtureReport: FnMut(&mut S, &mut String, &CompareFixtureReportInput<'_>),
    Report:
        FnMut(&mut S, &mut String, &CompareRunPaths, &CompareRunOptions<'_>, &[String], &[String]),
{
    run_svg_compare_with_roots_and_fixture_reports(
        options,
        None,
        None,
        state,
        write_header,
        skip_fixture,
        render_fixture,
        write_fixture_report,
        write_report,
    )
}

// See `run_svg_compare_with_roots`: this is the complete callback-oriented harness entry point.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_svg_compare_with_roots_and_fixture_reports<
    S,
    Header,
    Skip,
    Render,
    FixtureReport,
    Report,
>(
    mut options: CompareRunOptions<'_>,
    fixtures_root: Option<PathBuf>,
    upstream_root: Option<PathBuf>,
    state: &mut S,
    mut write_header: Header,
    mut skip_fixture: Skip,
    mut render_fixture: Render,
    mut write_fixture_report: FixtureReport,
    mut write_report: Report,
) -> Result<(), XtaskError>
where
    Header: FnMut(&mut S, &mut String, &CompareRunPaths, &CompareRunOptions<'_>),
    Skip: FnMut(&mut S, &str, &CompareRunPaths) -> Option<String>,
    Render: FnMut(&mut S, &CompareFixtureInput<'_>) -> Result<CompareFixtureResult, String>,
    FixtureReport: FnMut(&mut S, &mut String, &CompareFixtureReportInput<'_>),
    Report:
        FnMut(&mut S, &mut String, &CompareRunPaths, &CompareRunOptions<'_>, &[String], &[String]),
{
    let compare_paths = crate::cmd::compare_diagram_paths_with_roots(
        options.diagram,
        options.out_path.take(),
        fixtures_root,
        upstream_root,
    );
    let fixtures_dir = compare_paths.fixtures_dir.clone();
    let upstream_dir = compare_paths.upstream_dir.clone();
    let validate_pinned_upstream = is_pinned_upstream_dir(options.diagram, &upstream_dir);
    let out_svg_dir = compare_paths.out_svg_dir.clone();
    let _upstream_family_lock = super::acquire_upstream_svg_family_lock_for_compare(
        &upstream_dir,
        validate_pinned_upstream,
    )?;
    let mmd_files = crate::cmd::list_mmd_fixtures_in_dir(&fixtures_dir, options.filter, true);
    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }
    let provenance = if validate_pinned_upstream {
        Some(crate::cmd::load_upstream_svg_provenance(
            options.diagram,
            &fixtures_dir,
            &upstream_dir,
            options.filter.is_none(),
        )?)
    } else {
        None
    };

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(options.dom_mode);
    let mut report = String::new();
    write_header(state, &mut report, &compare_paths, &options);

    let mut failures: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        if let Some(reason) = skip_fixture(state, stem, &compare_paths) {
            notes.push(format!("skipped {stem}: {reason}"));
            continue;
        }

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        if let Some(provenance) = &provenance
            && let Err(err) = provenance.validate_fixture(&mmd_path, &upstream_path)
        {
            failures.push(err);
            continue;
        }
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {stem}: {} ({err})",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let input = CompareFixtureInput {
            stem,
            fixture_path: &mmd_path,
            upstream_svg: &upstream_svg,
            text: &text,
            check_dom: options.check_dom,
        };

        let outcome = match render_fixture(state, &input) {
            Ok(v) => v,
            Err(err) => {
                failures.push(err);
                continue;
            }
        };

        let failure_start = failures.len();
        match outcome {
            CompareFixtureResult::Skipped { reason } => {
                notes.push(format!("skipped {stem}: {reason}"));
                continue;
            }
            CompareFixtureResult::Rendered {
                local_svg,
                compare_dom,
                issues,
                notes: fixture_notes,
            } => {
                write_rendered_fixture(
                    &local_out_path,
                    &local_svg,
                    &mut failures,
                    &mut notes,
                    issues,
                    fixture_notes,
                    false,
                    options.check_dom,
                    compare_dom,
                    stem,
                    &upstream_svg,
                    &upstream_path,
                    mode,
                    options.dom_decimals,
                )?;
            }
            CompareFixtureResult::RenderedWithPolicy {
                local_svg,
                compare_dom,
                compare_svg_when_dom_disabled,
                issues,
                notes: fixture_notes,
            } => {
                write_rendered_fixture(
                    &local_out_path,
                    &local_svg,
                    &mut failures,
                    &mut notes,
                    issues,
                    fixture_notes,
                    compare_svg_when_dom_disabled,
                    options.check_dom,
                    compare_dom,
                    stem,
                    &upstream_svg,
                    &upstream_path,
                    mode,
                    options.dom_decimals,
                )?;
            }
        }

        write_fixture_report(
            state,
            &mut report,
            &CompareFixtureReportInput {
                stem,
                fixture_path: &mmd_path,
                upstream_path: &upstream_path,
                local_out_path: &local_out_path,
                failed: failures.len() > failure_start,
            },
        );
    }

    write_report(
        state,
        &mut report,
        &compare_paths,
        &options,
        &failures,
        &notes,
    );

    if let Some(parent) = compare_paths.out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&compare_paths.out_path, report).map_err(|source| XtaskError::WriteFile {
        path: compare_paths.out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

#[allow(clippy::too_many_arguments)]
fn write_rendered_fixture(
    local_out_path: &Path,
    local_svg: &str,
    failures: &mut Vec<String>,
    notes: &mut Vec<String>,
    issues: Vec<String>,
    fixture_notes: Vec<String>,
    compare_svg_when_dom_disabled: bool,
    check_dom: bool,
    compare_dom: bool,
    stem: &str,
    upstream_svg: &str,
    upstream_path: &Path,
    mode: svgdom::DomMode,
    dom_decimals: u32,
) -> Result<(), XtaskError> {
    fs::write(local_out_path, local_svg).map_err(|source| XtaskError::WriteFile {
        path: local_out_path.display().to_string(),
        source,
    })?;

    if check_dom && compare_dom {
        if let Err(err) = compare_dom_signatures(
            stem,
            upstream_svg,
            local_svg,
            upstream_path,
            local_out_path,
            mode,
            dom_decimals,
        ) {
            failures.push(err);
        }
    } else if !check_dom && compare_svg_when_dom_disabled && upstream_svg != local_svg {
        failures.push(format!("svg mismatch for {stem}"));
    }

    failures.extend(issues);
    notes.extend(fixture_notes);
    Ok(())
}

pub(crate) fn sanitize_svg_id(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "diagram".to_string()
    } else {
        out
    }
}

fn compare_dom_signatures(
    stem: &str,
    upstream_svg: &str,
    local_svg: &str,
    upstream_path: &Path,
    local_out_path: &Path,
    mode: svgdom::DomMode,
    dom_decimals: u32,
) -> Result<(), String> {
    let upstream = svgdom::dom_signature(upstream_svg, mode, dom_decimals)
        .map_err(|err| format!("upstream dom parse failed for {stem}: {err}"))?;
    let local = svgdom::dom_signature(local_svg, mode, dom_decimals)
        .map_err(|err| format!("local dom parse failed for {stem}: {err}"))?;

    if upstream != local {
        let detail = if mode == svgdom::DomMode::ParityRoot {
            let mismatch = svgdom::diagnose_parity_root_mismatch(
                upstream_svg,
                local_svg,
                &upstream,
                &local,
                dom_decimals,
            )
            .map_err(|err| format!("parity-root diagnosis failed for {stem}: {err}"))?
            .ok_or_else(|| format!("parity-root diagnosis unexpectedly matched for {stem}"))?;
            format!(" ({mismatch})")
        } else {
            svgdom::dom_diff(&upstream, &local)
                .map(|d| format!(" ({d})"))
                .unwrap_or_default()
        };
        return Err(format!(
            "dom mismatch for {stem}: upstream={} local={}{}",
            upstream_path.display(),
            local_out_path.display(),
            detail
        ));
    }

    Ok(())
}

pub(crate) fn write_compare_result_section(
    report: &mut String,
    check_dom: bool,
    failures: &[String],
    out_svg_dir: &Path,
) {
    if !check_dom {
        let _ = writeln!(
            report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(report, "\n## Mismatches\n");
        for failure in failures {
            let _ = writeln!(report, "- {failure}");
        }
        let _ = writeln!(report, "\nLocal SVG outputs: `{}`\n", out_svg_dir.display());
    }
}

pub(crate) fn write_notes_section(report: &mut String, notes: &[String]) {
    if notes.is_empty() {
        return;
    }

    let _ = writeln!(
        report,
        "\n## Skipped\n\nThese fixtures are intentionally skipped (feature gaps / deferred parity).\n"
    );
    for note in notes {
        let _ = writeln!(report, "- {note}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parity_root_failure_marks_normalized_descendant_match() {
        let upstream = r#"<svg width="100%" viewBox="0 0 100 100" style="max-width: 100px; background-color: white;"><g transform="translate(10,20)"/></svg>"#;
        let local = r#"<svg width="100%" viewBox="0 0 120 100" style="max-width: 120px; background-color: white;"><g transform="translate(10,20)"/></svg>"#;

        let failure = compare_dom_signatures(
            "root-only",
            upstream,
            local,
            Path::new("upstream.svg"),
            Path::new("local.svg"),
            svgdom::DomMode::ParityRoot,
            3,
        )
        .expect_err("root viewport mismatch should fail");

        assert!(failure.contains(svgdom::PARITY_NORMALIZED_DESCENDANTS_MATCH_MARKER));
        assert!(failure.contains("svg: attr `style` mismatch"));
        assert_eq!(failure.lines().count(), 1);
    }

    #[test]
    fn parity_root_failure_prioritizes_hidden_parity_visible_mismatch() {
        let upstream = r#"<svg width="100%" viewBox="0 0 100 100" style="max-width: 100px; background-color: white;"><g transform="translate(10,20)"/></svg>"#;
        let local = r#"<svg width="100%" viewBox="0 0 120 100" style="max-width: 120px; background-color: white;"><g transform="scale(10,20)"/></svg>"#;

        let failure = compare_dom_signatures(
            "root-and-subtree",
            upstream,
            local,
            Path::new("upstream.svg"),
            Path::new("local.svg"),
            svgdom::DomMode::ParityRoot,
            3,
        )
        .expect_err("parity-visible subtree mismatch should fail");

        assert!(failure.contains(svgdom::PARITY_NORMALIZED_DESCENDANTS_DIFFER_MARKER));
        assert!(failure.contains("root-viewport-also-differs=true"));
        assert!(failure.contains("svg/g[0]: attr `transform` mismatch"));
        assert!(!failure.contains("max-width: 100px"));
        assert_eq!(failure.lines().count(), 1);
    }

    fn unique_test_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        crate::cmd::target_root()
            .join("compare")
            .join("xtask-harness-tests")
            .join(format!("{name}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn explicit_canonical_upstream_path_still_requires_pinned_provenance() {
        let pinned = crate::cmd::fixtures_root().join("upstream-svgs").join("er");
        assert!(is_pinned_upstream_dir("er", &pinned));
        assert!(is_pinned_upstream_dir("er", &pinned.join(".")));
    }

    #[test]
    fn svg_compare_harness_supports_custom_roots_and_render_level_skips() {
        let root = unique_test_root("roots-and-skips");
        let fixtures_root = root.join("fixtures");
        let upstream_root = root.join("upstream");
        let fixture_dir = fixtures_root.join("harness_probe");
        let upstream_dir = upstream_root.join("harness_probe");
        fs::create_dir_all(&fixture_dir).expect("fixture dir should be created");
        fs::create_dir_all(&upstream_dir).expect("upstream dir should be created");
        fs::write(fixture_dir.join("rendered.mmd"), "rendered")
            .expect("rendered fixture should be written");
        fs::write(fixture_dir.join("skipped.mmd"), "skipped")
            .expect("skipped fixture should be written");
        fs::write(upstream_dir.join("rendered.svg"), r#"<svg id="rendered"/>"#)
            .expect("rendered upstream should be written");
        fs::write(upstream_dir.join("skipped.svg"), r#"<svg id="skipped"/>"#)
            .expect("skipped upstream should be written");

        let out_path = root.join("report.md");
        let mut seen = Vec::new();
        run_svg_compare_with_roots(
            CompareRunOptions {
                diagram: "harness_probe",
                out_path: Some(out_path.clone()),
                filter: None,
                check_dom: true,
                dom_mode: "parity",
                dom_decimals: 3,
            },
            Some(fixtures_root),
            Some(upstream_root),
            &mut seen,
            |_, report, _paths, _options| {
                let _ = writeln!(report, "# Harness Probe");
            },
            |_, _, _| None,
            |seen, input| {
                seen.push(input.stem.to_string());
                if input.stem == "skipped" {
                    return Ok(CompareFixtureResult::Skipped {
                        reason: "parse-time admission policy".to_string(),
                    });
                }
                Ok(CompareFixtureResult::Rendered {
                    local_svg: input.upstream_svg.to_string(),
                    compare_dom: true,
                    issues: Vec::new(),
                    notes: Vec::new(),
                })
            },
            |_, report, paths, options, failures, notes| {
                write_compare_result_section(
                    report,
                    options.check_dom,
                    failures,
                    &paths.out_svg_dir,
                );
                write_notes_section(report, notes);
            },
        )
        .expect("custom-root harness run should succeed");

        assert_eq!(seen, ["rendered", "skipped"]);
        let report = fs::read_to_string(&out_path).expect("report should be written");
        assert!(report.contains("All fixtures matched."));
        assert!(report.contains("skipped skipped: parse-time admission policy"));
        let out_svg_dir = out_path
            .parent()
            .expect("out path should have parent")
            .join("harness_probe");
        assert!(out_svg_dir.join("rendered.svg").is_file());
        assert!(!out_svg_dir.join("skipped.svg").is_file());
    }
}
