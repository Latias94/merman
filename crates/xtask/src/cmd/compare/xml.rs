//! SVG compare XML helpers.

use crate::XtaskError;
use crate::cmd::svg_compare_engine_with_site_config;
use crate::svgdom;
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpstreamRootTrust {
    PinnedCanonical,
    UntrustedCustom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProvenanceCoverage {
    CompleteFamily,
    SelectedFixtures,
}

impl UpstreamRootTrust {
    fn provenance_coverage(self, filter: Option<&str>) -> Option<ProvenanceCoverage> {
        match (self, filter) {
            (Self::PinnedCanonical, None) => Some(ProvenanceCoverage::CompleteFamily),
            (Self::PinnedCanonical, Some(_)) => Some(ProvenanceCoverage::SelectedFixtures),
            (Self::UntrustedCustom, _) => None,
        }
    }
}

fn classify_upstream_root(upstream_root: &Path, canonical_root: &Path) -> UpstreamRootTrust {
    if upstream_root == canonical_root
        || fs::canonicalize(upstream_root)
            .ok()
            .zip(fs::canonicalize(canonical_root).ok())
            .is_some_and(|(upstream, canonical)| upstream == canonical)
    {
        UpstreamRootTrust::PinnedCanonical
    } else {
        UpstreamRootTrust::UntrustedCustom
    }
}

fn validate_svg_xml_diagram(diagram: &str) -> Result<(), XtaskError> {
    let path = Path::new(diagram);
    let mut components = path.components();
    let is_single_normal_component = matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(component)), None) if component == path.as_os_str()
    );
    let contains_separator = diagram.chars().any(|ch| matches!(ch, '/' | '\\'));
    let bytes = diagram.as_bytes();
    let has_windows_drive_prefix =
        bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';

    if path.is_absolute()
        || contains_separator
        || has_windows_drive_prefix
        || !is_single_normal_component
    {
        return Err(XtaskError::SvgCompareFailed(format!(
            "invalid --diagram {diagram:?}: expected a single normal path component"
        )));
    }

    Ok(())
}

fn svg_xml_family_dir(
    root: &Path,
    diagram: &str,
    root_description: &str,
) -> Result<PathBuf, XtaskError> {
    validate_svg_xml_diagram(diagram)?;
    let family_dir = root.join(diagram);
    if family_dir.parent() != Some(root) {
        return Err(XtaskError::SvgCompareFailed(format!(
            "refusing to resolve --diagram {diagram:?} outside the {root_description} root {}",
            root.display()
        )));
    }

    let canonical_root = match fs::canonicalize(root) {
        Ok(path) => path,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(family_dir),
        Err(err) => {
            return Err(XtaskError::SvgCompareFailed(format!(
                "failed to resolve the {root_description} root {}: {err}",
                root.display()
            )));
        }
    };
    let canonical_family = match fs::canonicalize(&family_dir) {
        Ok(path) => path,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(family_dir),
        Err(err) => {
            return Err(XtaskError::SvgCompareFailed(format!(
                "failed to resolve the {root_description} family {}: {err}",
                family_dir.display()
            )));
        }
    };
    if canonical_family.parent() != Some(canonical_root.as_path()) {
        return Err(XtaskError::SvgCompareFailed(format!(
            "refusing {root_description} family {diagram:?}: {} is not a canonical direct child of {}",
            canonical_family.display(),
            canonical_root.display()
        )));
    }
    Ok(family_dir)
}

fn svg_xml_compare_skip_reason(diagram: &str, stem: &str) -> Option<&'static str> {
    crate::cmd::upstream_svg_compare_skip_reason(diagram, stem)
}

pub(crate) fn compare_svg_xml(args: Vec<String>) -> Result<(), XtaskError> {
    let mut check: bool = false;
    let mut dom_mode: Option<String> = None;
    let mut dom_decimals: Option<u32> = None;
    let mut filter: Option<String> = None;
    let mut text_measurer: Option<String> = None;
    let mut upstream_root_arg: Option<PathBuf> = None;
    let mut fixtures_root_arg: Option<PathBuf> = None;
    let mut out_root_arg: Option<PathBuf> = None;
    let mut only_diagrams: Vec<String> = Vec::new();
    let mut include_elk_probes = false;
    let mut flowchart_elk_backend = crate::cmd::default_flowchart_elk_backend();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--check" | "--check-xml" => check = true,
            "--dom-mode" => {
                i += 1;
                dom_mode = args.get(i).map(|s| s.trim().to_string());
            }
            "--dom-decimals" | "--xml-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.trim().parse::<u32>().ok());
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--text-measurer" => {
                i += 1;
                text_measurer = args.get(i).map(|s| s.trim().to_ascii_lowercase());
            }
            "--upstream-root" => {
                i += 1;
                upstream_root_arg = args.get(i).map(|s| PathBuf::from(s.trim()));
            }
            "--fixtures-root" => {
                i += 1;
                fixtures_root_arg = args.get(i).map(|s| PathBuf::from(s.trim()));
            }
            "--out-root" => {
                i += 1;
                out_root_arg = args.get(i).map(|s| PathBuf::from(s.trim()));
            }
            "--diagram" => {
                i += 1;
                let d = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
                validate_svg_xml_diagram(&d)?;
                only_diagrams.push(d);
            }
            "--flowchart-elk-backend" => {
                i += 1;
                flowchart_elk_backend =
                    crate::cmd::parse_flowchart_elk_backend(args.get(i).map(String::as_str))?;
            }
            "--include-elk-probes" => include_elk_probes = true,
            "--help" | "-h" => {
                return Err(XtaskError::Usage);
            }
            other => {
                return Err(XtaskError::UnknownCommand(format!(
                    "compare-svg-xml: unknown arg `{other}`"
                )));
            }
        }
        i += 1;
    }

    let dom_mode = dom_mode.unwrap_or_else(|| "strict".to_string());
    let dom_decimals = dom_decimals.unwrap_or(3);
    let mode = svgdom::DomMode::parse(&dom_mode);

    let measurer: Arc<dyn merman_render::text::TextMeasurer + Send + Sync> =
        match text_measurer.as_deref().unwrap_or("vendored") {
            "deterministic" => Arc::new(merman_render::text::DeterministicTextMeasurer::default()),
            _ => Arc::new(merman_render::text::VendoredFontMetricsTextMeasurer::default()),
        };

    let workspace_root = crate::cmd::workspace_root();

    let node_math_renderer: Option<Arc<dyn merman_render::math::MathRenderer + Send + Sync>> = {
        let node_cwd = crate::cmd::mermaid_cli_root();
        if node_cwd.join("package.json").is_file() && node_cwd.join("node_modules").is_dir() {
            Some(Arc::new(merman_render::math::NodeKatexMathRenderer::new(
                node_cwd,
            )))
        } else {
            None
        }
    };

    // Mermaid gitGraph auto-generates commit ids using `Math.random()`. Upstream gitGraph SVGs in
    // this repo are generated with a seeded upstream renderer, so keep the local side seeded too
    // for meaningful strict XML comparisons.
    let engine = svg_compare_engine_with_site_config(
        serde_json::json!({ "handDrawnSeed": 1, "gitGraph": { "seed": 1 } }),
    );
    let layout_opts = merman_render::LayoutOptions {
        text_measurer: Arc::clone(&measurer),
        flowchart_elk_backend,
        ..Default::default()
    };

    fn resolve_root(workspace_root: &Path, raw: Option<PathBuf>, default: PathBuf) -> PathBuf {
        let Some(raw) = raw else {
            return default;
        };
        if raw.is_absolute() {
            return raw;
        }
        if raw.as_os_str().is_empty() {
            return default;
        }
        workspace_root.join(raw)
    }

    let canonical_upstream_root = crate::cmd::fixtures_root().join("upstream-svgs");
    let upstream_root = resolve_root(
        &workspace_root,
        upstream_root_arg,
        canonical_upstream_root.clone(),
    );
    let upstream_root_trust = classify_upstream_root(&upstream_root, &canonical_upstream_root);
    let provenance_coverage = upstream_root_trust.provenance_coverage(filter.as_deref());
    if upstream_root_trust == UpstreamRootTrust::UntrustedCustom {
        eprintln!(
            "warning: compare-svg-xml is using untrusted custom upstream SVGs from {}; pinned provenance validation is disabled",
            upstream_root.display()
        );
    }
    let fixtures_root = resolve_root(
        &workspace_root,
        fixtures_root_arg,
        crate::cmd::fixtures_root(),
    );
    let out_root = resolve_root(
        &workspace_root,
        out_root_arg,
        crate::cmd::target_root().join("compare").join("xml"),
    );

    fn sanitize_svg_id(raw: &str) -> String {
        let mut out = String::with_capacity(raw.len());
        for ch in raw.chars() {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                out.push(ch);
            } else {
                out.push('_');
            }
        }
        out
    }

    fn flowchart_fixture_diagram_id(stem: &str, upstream_svg: &str) -> String {
        let fallback = sanitize_svg_id(stem);
        if !stem.ends_with("_katex") {
            return fallback;
        }
        let Ok(doc) = roxmltree::Document::parse(upstream_svg) else {
            return fallback;
        };
        let Some(id) = doc.root_element().attribute("id") else {
            return fallback;
        };
        if id.trim().is_empty() {
            fallback
        } else {
            id.to_string()
        }
    }

    fn gantt_upstream_today_x1(svg: &str) -> Option<f64> {
        let doc = roxmltree::Document::parse(svg).ok()?;
        for n in doc.descendants().filter(|n| n.has_tag_name("line")) {
            if !n
                .attribute("class")
                .unwrap_or_default()
                .split_whitespace()
                .any(|t| t == "today")
            {
                continue;
            }
            let x1 = n.attribute("x1")?.parse::<f64>().ok()?;
            if x1.is_finite() {
                return Some(x1);
            }
        }
        None
    }

    fn gantt_derive_now_ms_from_upstream_today(
        upstream_svg: &str,
        layout: &merman_render::model::GanttDiagramLayout,
    ) -> Option<i64> {
        let x1 = gantt_upstream_today_x1(upstream_svg)?;
        let min_ms = layout.tasks.iter().map(|t| t.start_ms).min()?;
        let max_ms = layout.tasks.iter().map(|t| t.end_ms).max()?;
        if max_ms <= min_ms {
            return None;
        }
        let range = (layout.width - layout.left_padding - layout.right_padding).max(1.0);
        let target_x = x1;

        fn gantt_today_x(
            now_ms: i64,
            min_ms: i64,
            max_ms: i64,
            range: f64,
            left_padding: f64,
        ) -> f64 {
            if max_ms <= min_ms {
                return left_padding + (range / 2.0).round();
            }
            let t = (now_ms - min_ms) as f64 / (max_ms - min_ms) as f64;
            left_padding + (t * range).round()
        }

        // Start from a linear estimate, then bracket + binary-search to find a `now_ms` that
        // reproduces the exact upstream `x1` under our `round(t * range)` implementation.
        let span = (max_ms - min_ms) as f64;
        let scaled = target_x - layout.left_padding;
        if !(span.is_finite() && scaled.is_finite() && range.is_finite()) {
            return None;
        }
        let est = (min_ms as f64) + span * (scaled / range);
        if !est.is_finite() {
            return None;
        }
        let mut lo = est.round() as i64;
        let mut hi = lo;
        let mut step: i64 = 1;

        let mut guard = 0;
        while guard < 80 {
            guard += 1;
            let x_lo = gantt_today_x(lo, min_ms, max_ms, range, layout.left_padding);
            if x_lo.is_nan() {
                return None;
            }
            if x_lo <= target_x {
                break;
            }
            hi = lo;
            lo = lo.saturating_sub(step);
            step = step.saturating_mul(2);
        }
        guard = 0;
        step = 1;
        while guard < 80 {
            guard += 1;
            let x_hi = gantt_today_x(hi, min_ms, max_ms, range, layout.left_padding);
            if x_hi.is_nan() {
                return None;
            }
            if x_hi >= target_x {
                break;
            }
            lo = hi;
            hi = hi.saturating_add(step);
            step = step.saturating_mul(2);
        }

        // If we failed to bracket, bail.
        let x_lo = gantt_today_x(lo, min_ms, max_ms, range, layout.left_padding);
        let x_hi = gantt_today_x(hi, min_ms, max_ms, range, layout.left_padding);
        if !(x_lo <= target_x && target_x <= x_hi) {
            return None;
        }

        // Lower-bound search: first `now_ms` where x >= target.
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let x_mid = gantt_today_x(mid, min_ms, max_ms, range, layout.left_padding);
            if x_mid < target_x {
                lo = mid.saturating_add(1);
            } else {
                hi = mid;
            }
        }
        let x = gantt_today_x(lo, min_ms, max_ms, range, layout.left_padding);
        if x == target_x { Some(lo) } else { None }
    }

    let mut diagrams: Vec<String> = if !only_diagrams.is_empty() {
        only_diagrams
    } else if upstream_root_trust == UpstreamRootTrust::PinnedCanonical {
        crate::cmd::primary_svg_matrix_diagrams()
            .map(str::to_string)
            .collect()
    } else {
        let entries = fs::read_dir(&upstream_root).map_err(|err| {
            XtaskError::SvgCompareFailed(format!(
                "failed to list upstream svg directory {}: {err}",
                upstream_root.display()
            ))
        })?;
        entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .filter_map(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_string)
            })
            .collect()
    };
    diagrams.sort();
    diagrams.dedup();

    if diagrams.is_empty() {
        return Err(XtaskError::SvgCompareFailed(
            "no diagram directories matched under fixtures/upstream-svgs".to_string(),
        ));
    }

    let mut mismatches: Vec<(String, String, PathBuf, PathBuf)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut skipped: Vec<(String, &'static str)> = Vec::new();

    for diagram in diagrams {
        let upstream_dir = svg_xml_family_dir(&upstream_root, &diagram, "upstream SVG")?;
        let fixtures_dir = svg_xml_family_dir(&fixtures_root, &diagram, "fixtures")?;
        let out_dir = svg_xml_family_dir(&out_root, &diagram, "output")?;
        let _upstream_family_lock = super::acquire_upstream_svg_family_lock_for_compare(
            &upstream_dir,
            provenance_coverage.is_some(),
        )?;
        let provenance = if let Some(coverage) = provenance_coverage {
            Some(crate::cmd::load_upstream_svg_provenance(
                &diagram,
                &fixtures_dir,
                &upstream_dir,
                coverage == ProvenanceCoverage::CompleteFamily,
            )?)
        } else {
            None
        };
        fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
            path: out_dir.display().to_string(),
            source,
        })?;

        let fixture_paths =
            crate::cmd::list_mmd_fixtures_in_dir(&fixtures_dir, filter.as_deref(), true);
        if fixture_paths.is_empty() {
            missing.push(format!(
                "{diagram}: no fixtures matched under {}",
                fixtures_dir.display()
            ));
            continue;
        }

        for fixture_path in fixture_paths {
            let Some(stem) = fixture_path.file_stem().and_then(|s| s.to_str()) else {
                missing.push(format!(
                    "{diagram}: invalid fixture filename {}",
                    fixture_path.display()
                ));
                continue;
            };
            if let Some(reason) = svg_xml_compare_skip_reason(&diagram, stem) {
                skipped.push((format!("{diagram}/{stem}"), reason));
                continue;
            }
            let upstream_path = upstream_dir.join(format!("{stem}.svg"));
            if let Some(provenance) = &provenance {
                provenance
                    .validate_fixture(&fixture_path, &upstream_path)
                    .map_err(XtaskError::SvgCompareFailed)?;
            }
            let text = match fs::read_to_string(&fixture_path) {
                Ok(v) => v,
                Err(err) => {
                    missing.push(format!(
                        "{diagram}/{stem}: missing fixture {} ({err})",
                        fixture_path.display()
                    ));
                    continue;
                }
            };
            let upstream_svg = match fs::read_to_string(&upstream_path) {
                Ok(v) => v,
                Err(err) => {
                    missing.push(format!(
                        "{diagram}/{stem}: failed to read upstream svg {} ({err})",
                        upstream_path.display()
                    ));
                    continue;
                }
            };

            let parsed = match futures::executor::block_on(engine.parse_diagram(
                &text,
                merman::ParseOptions {
                    suppress_errors: true,
                },
            )) {
                Ok(Some(v)) => v,
                Ok(None) => {
                    missing.push(format!("{diagram}/{stem}: no diagram detected"));
                    continue;
                }
                Err(err) => {
                    missing.push(format!("{diagram}/{stem}: parse failed: {err}"));
                    continue;
                }
            };

            if diagram == "flowchart" {
                let flowchart_layout_elk = parsed.meta.diagram_type == "flowchart-elk"
                    || parsed.meta.effective_config.get_str("layout") == Some("elk")
                    || parsed
                        .meta
                        .effective_config
                        .get_str("flowchart.defaultRenderer")
                        == Some("elk");
                if flowchart_layout_elk {
                    let admitted = crate::cmd::flowchart_elk_svg_compare_admitted(
                        stem,
                        include_elk_probes,
                        flowchart_elk_backend,
                    );
                    if !admitted
                        && let Some(reason) = crate::cmd::flowchart_elk_svg_compare_skip_reason(
                            stem,
                            include_elk_probes,
                            flowchart_elk_backend,
                        )
                    {
                        skipped.push((format!("{diagram}/{stem}"), reason));
                        continue;
                    }
                }
            }

            let mut layout_opts = layout_opts.clone();
            if matches!(diagram.as_str(), "flowchart" | "sequence") {
                layout_opts.math_renderer = node_math_renderer.clone();
            }

            let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
                Ok(v) => v,
                Err(err) => {
                    missing.push(format!("{diagram}/{stem}: layout failed: {err}"));
                    continue;
                }
            };

            let is_classdiagram_v2_header = if diagram == "class" {
                {
                    merman::preprocess_diagram(&text, engine.registry())
                        .ok()
                        .map(|p| p.code.trim_start().starts_with("classDiagram-v2"))
                        .unwrap_or(false)
                }
            } else {
                false
            };

            let diagram_id = if diagram == "flowchart" {
                flowchart_fixture_diagram_id(stem, &upstream_svg)
            } else {
                sanitize_svg_id(stem)
            };
            let mut svg_opts = merman_render::svg::SvgRenderOptions {
                diagram_id: Some(diagram_id),
                aria_roledescription: is_classdiagram_v2_header.then(|| "classDiagram".to_string()),
                ..Default::default()
            };
            if matches!(diagram.as_str(), "flowchart" | "sequence") {
                svg_opts.math_renderer = node_math_renderer.clone();
            }
            if diagram == "gantt"
                && let merman_render::model::LayoutDiagram::GanttDiagram(layout) = &layouted.layout
            {
                svg_opts.now_ms_override =
                    gantt_derive_now_ms_from_upstream_today(&upstream_svg, layout);
            }
            let local_svg = match merman_render::svg::render_layouted_svg(
                &layouted,
                layout_opts.text_measurer.as_ref(),
                &svg_opts,
            ) {
                Ok(v) => v,
                Err(err) => {
                    missing.push(format!("{diagram}/{stem}: render failed: {err}"));
                    continue;
                }
            };

            let upstream_xml = match svgdom::canonical_xml(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    missing.push(format!(
                        "{diagram}/{stem}: upstream xml parse failed: {err}"
                    ));
                    continue;
                }
            };
            let local_xml = match svgdom::canonical_xml(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    missing.push(format!("{diagram}/{stem}: local xml parse failed: {err}"));
                    continue;
                }
            };

            if upstream_xml != local_xml {
                let upstream_out = out_dir.join(format!("{stem}.upstream.xml"));
                let local_out = out_dir.join(format!("{stem}.local.xml"));
                fs::write(&upstream_out, upstream_xml).map_err(|source| XtaskError::WriteFile {
                    path: upstream_out.display().to_string(),
                    source,
                })?;
                fs::write(&local_out, local_xml).map_err(|source| XtaskError::WriteFile {
                    path: local_out.display().to_string(),
                    source,
                })?;

                mismatches.push((diagram.clone(), stem.to_string(), upstream_out, local_out));
            }
        }
    }

    let report_path = out_root.join("xml_report.md");
    let mut report = String::new();
    let _ = writeln!(&mut report, "# SVG Canonical XML Compare Report");
    let _ = writeln!(&mut report);
    let _ = writeln!(
        &mut report,
        "- Mode: `{}`",
        match mode {
            svgdom::DomMode::Strict => "strict",
            svgdom::DomMode::Structure => "structure",
            svgdom::DomMode::Parity => "parity",
            svgdom::DomMode::ParityRoot => "parity-root",
        }
    );
    let _ = writeln!(&mut report, "- Decimals: `{dom_decimals}`");
    let _ = writeln!(
        &mut report,
        "- Upstream provenance: {}",
        match provenance_coverage {
            Some(ProvenanceCoverage::CompleteFamily) => {
                "`pinned canonical (complete family validated)`"
            }
            Some(ProvenanceCoverage::SelectedFixtures) => {
                "`pinned canonical (selected fixtures validated)`"
            }
            None => "`untrusted custom (debug only)`",
        }
    );
    let _ = writeln!(
        &mut report,
        "- Output: `target/compare/xml/<diagram>/<fixture>.(upstream|local).xml`"
    );
    let _ = writeln!(&mut report);
    let _ = writeln!(&mut report, "## Mismatches ({})", mismatches.len());
    let _ = writeln!(&mut report);
    for (diagram, stem, upstream_out, local_out) in &mismatches {
        let _ = writeln!(
            &mut report,
            "- `{diagram}/{stem}`: `{}` vs `{}`",
            upstream_out.display(),
            local_out.display()
        );
    }
    if !missing.is_empty() {
        let _ = writeln!(&mut report);
        let _ = writeln!(&mut report, "## Missing / Failed ({})", missing.len());
        let _ = writeln!(&mut report);
        for m in &missing {
            let _ = writeln!(&mut report, "- {m}");
        }
    }
    if !skipped.is_empty() {
        let _ = writeln!(&mut report);
        let _ = writeln!(&mut report, "## Skipped ({})", skipped.len());
        let _ = writeln!(&mut report);
        for (item, reason) in &skipped {
            let _ = writeln!(&mut report, "- {item}: {reason}");
        }
    }

    fs::write(&report_path, report).map_err(|source| XtaskError::WriteFile {
        path: report_path.display().to_string(),
        source,
    })?;

    if check && (!mismatches.is_empty() || !missing.is_empty()) {
        let mut msg = String::new();
        let _ = writeln!(
            &mut msg,
            "canonical XML mismatches: {} (mode={dom_mode}, decimals={dom_decimals})",
            mismatches.len()
        );
        if !mismatches.is_empty() {
            for (diagram, stem, upstream_out, local_out) in mismatches.iter().take(20) {
                let _ = writeln!(
                    &mut msg,
                    "- {diagram}/{stem}: {} vs {}",
                    upstream_out.display(),
                    local_out.display()
                );
            }
            if mismatches.len() > 20 {
                let _ = writeln!(&mut msg, "- … ({} more)", mismatches.len() - 20);
            }
        }
        if !missing.is_empty() {
            let _ = writeln!(&mut msg, "missing/failed cases: {}", missing.len());
            for m in missing.iter().take(20) {
                let _ = writeln!(&mut msg, "- {m}");
            }
            if missing.len() > 20 {
                let _ = writeln!(&mut msg, "- … ({} more)", missing.len() - 20);
            }
        }
        let _ = writeln!(&mut msg, "report: {}", report_path.display());
        return Err(XtaskError::SvgCompareFailed(msg));
    }

    println!("wrote report: {}", report_path.display());
    Ok(())
}

pub(crate) fn canon_svg_xml(args: Vec<String>) -> Result<(), XtaskError> {
    let mut in_path: Option<PathBuf> = None;
    let mut dom_mode: Option<String> = None;
    let mut dom_decimals: Option<u32> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--in" => {
                i += 1;
                in_path = args.get(i).map(PathBuf::from);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args.get(i).map(|s| s.trim().to_string());
            }
            "--dom-decimals" | "--xml-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.trim().parse::<u32>().ok());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let in_path = in_path.ok_or(XtaskError::Usage)?;
    let svg = fs::read_to_string(&in_path).map_err(|source| XtaskError::ReadFile {
        path: in_path.display().to_string(),
        source,
    })?;
    let mode = svgdom::DomMode::parse(dom_mode.as_deref().unwrap_or("strict"));
    let decimals = dom_decimals.unwrap_or(3);

    let xml = svgdom::canonical_xml(&svg, mode, decimals).map_err(XtaskError::SvgCompareFailed)?;
    print!("{xml}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ProvenanceCoverage, UpstreamRootTrust, classify_upstream_root, compare_svg_xml,
        svg_xml_compare_skip_reason, svg_xml_family_dir, validate_svg_xml_diagram,
    };

    #[test]
    fn svg_xml_diagram_accepts_primary_matrix_family_names() {
        for diagram in crate::cmd::primary_svg_matrix_diagrams() {
            validate_svg_xml_diagram(diagram).unwrap_or_else(|err| {
                panic!("primary matrix family {diagram:?} was rejected: {err}")
            });
        }
    }

    #[test]
    fn svg_xml_diagram_rejects_non_normal_or_nested_path_components() {
        let absolute = crate::cmd::workspace_root()
            .join("outside-family")
            .display()
            .to_string();
        let invalid = [
            "",
            ".",
            "..",
            "./flowchart",
            r".\flowchart",
            "../flowchart",
            r"..\flowchart",
            "/flowchart",
            r"\flowchart",
            "flowchart/sequence",
            r"flowchart\sequence",
            "flowchart//sequence",
            r"flowchart\\sequence",
            "flowchart/",
            r"flowchart\",
            "C:flowchart",
            r"C:\flowchart",
            r"\\server\share\flowchart",
            absolute.as_str(),
        ];

        for diagram in invalid {
            let err = validate_svg_xml_diagram(diagram)
                .expect_err("non-normal or nested diagram path must be rejected");
            assert!(
                err.to_string().contains("single normal path component"),
                "unexpected error for {diagram:?}: {err}"
            );
        }
    }

    #[test]
    fn compare_svg_xml_rejects_an_escaping_diagram_at_the_cli_boundary() {
        let err = compare_svg_xml(vec![
            "--diagram".to_string(),
            "../outside-family".to_string(),
        ])
        .expect_err("escaping --diagram must be rejected before resolving family paths");

        assert!(
            err.to_string().contains("single normal path component"),
            "{err}"
        );
    }

    #[test]
    fn pinned_family_directories_are_direct_children_of_their_roots() {
        let roots = [
            ("fixtures", crate::cmd::fixtures_root()),
            (
                "upstream SVG",
                crate::cmd::fixtures_root().join("upstream-svgs"),
            ),
            (
                "output",
                crate::cmd::target_root().join("compare").join("xml"),
            ),
        ];

        for (description, root) in roots {
            let family = svg_xml_family_dir(&root, "flowchart", description)
                .expect("pinned family path should resolve");
            assert_eq!(family, root.join("flowchart"));
            assert_eq!(family.parent(), Some(root.as_path()));
        }
    }

    #[cfg(unix)]
    #[test]
    fn existing_family_symlink_cannot_escape_its_root() {
        use std::os::unix::fs::symlink;

        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "merman-svg-xml-family-symlink-{}-{nonce}",
            std::process::id()
        ));
        let root = base.join("root");
        let outside = base.join("outside");
        fs::create_dir_all(&root).expect("create test root");
        fs::create_dir_all(&outside).expect("create outside directory");
        symlink(&outside, root.join("flowchart")).expect("create escaping family symlink");

        let error = svg_xml_family_dir(&root, "flowchart", "test")
            .expect_err("canonical family escape must be rejected");
        assert!(error.to_string().contains("canonical direct child"));

        fs::remove_dir_all(base).expect("remove isolated symlink test tree");
    }

    #[test]
    fn explicit_canonical_upstream_root_remains_pinned() {
        let canonical = crate::cmd::fixtures_root().join("upstream-svgs");
        let explicit = canonical.join(".");

        assert_eq!(
            classify_upstream_root(&explicit, &canonical),
            UpstreamRootTrust::PinnedCanonical
        );
    }

    #[test]
    fn custom_upstream_root_is_untrusted() {
        let canonical = crate::cmd::fixtures_root().join("upstream-svgs");
        let custom = crate::cmd::target_root().join("custom-upstream-svgs");

        assert_eq!(
            classify_upstream_root(&custom, &canonical),
            UpstreamRootTrust::UntrustedCustom
        );
    }

    #[test]
    fn pinned_provenance_requires_complete_coverage_unless_filtered() {
        assert_eq!(
            UpstreamRootTrust::PinnedCanonical.provenance_coverage(None),
            Some(ProvenanceCoverage::CompleteFamily)
        );
        assert_eq!(
            UpstreamRootTrust::PinnedCanonical.provenance_coverage(Some("fixture")),
            Some(ProvenanceCoverage::SelectedFixtures)
        );
        assert_eq!(
            UpstreamRootTrust::UntrustedCustom.provenance_coverage(None),
            None
        );
    }

    #[test]
    fn filtered_compare_rejects_a_matching_fixture_without_an_upstream_svg() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = crate::cmd::target_root()
            .join("compare")
            .join("xtask-tests")
            .join(format!(
                "xml-missing-upstream-{}-{nonce}",
                std::process::id()
            ));
        let fixtures_root = root.join("fixtures");
        let upstream_root = root.join("upstream");
        let out_root = root.join("out");
        std::fs::create_dir_all(fixtures_root.join("info")).expect("create fixture family");
        std::fs::create_dir_all(upstream_root.join("info")).expect("create upstream family");
        std::fs::write(
            fixtures_root.join("info").join("missing-baseline.mmd"),
            "info\nshowInfo\n",
        )
        .expect("write fixture");

        let err = compare_svg_xml(vec![
            "--diagram".to_string(),
            "info".to_string(),
            "--filter".to_string(),
            "missing-baseline".to_string(),
            "--check".to_string(),
            "--fixtures-root".to_string(),
            fixtures_root.display().to_string(),
            "--upstream-root".to_string(),
            upstream_root.display().to_string(),
            "--out-root".to_string(),
            out_root.display().to_string(),
        ])
        .expect_err("a matching fixture without an SVG must fail the compare gate");

        assert!(err.to_string().contains("upstream svg"), "{err}");
    }

    #[test]
    fn svg_xml_compare_skip_reason_keeps_known_sequence_regen_skip() {
        assert_eq!(
            svg_xml_compare_skip_reason("sequence", "stress_end_keyword_016"),
            Some("pinned Mermaid 11.16 rejects `(end)` as a participant id")
        );
        assert_eq!(
            svg_xml_compare_skip_reason("sequence", "stress_end_keyword_015"),
            None
        );
    }

    #[test]
    fn svg_xml_compare_skip_reason_defers_flowchart_elk_after_parse() {
        assert_eq!(
            svg_xml_compare_skip_reason(
                "flowchart",
                "upstream_html_demos_flowchart_elk_flowchart_elk_001",
            ),
            None
        );
        assert_eq!(
            svg_xml_compare_skip_reason(
                "flowchart",
                "upstream_cypress_flowchart_elk_spec_1_elk_should_render_a_simple_flowchart_001",
            ),
            None
        );
        assert_eq!(
            crate::cmd::flowchart_elk_svg_parity_skip_reason(
                "upstream_cypress_flowchart_elk_spec_1_elk_should_render_a_simple_flowchart_001",
            ),
            None
        );
        assert_eq!(
            crate::cmd::flowchart_elk_svg_parity_skip_reason(
                "upstream_html_demos_flowchart_elk_flowchart_elk_001",
            ),
            None
        );
        assert_eq!(
            svg_xml_compare_skip_reason("flowchart", "upstream_docs_flowchart_basic_001"),
            None
        );
    }

    #[test]
    fn svg_xml_compare_skip_reason_covers_flowchart_parser_only_svg_baseline() {
        assert_eq!(
            svg_xml_compare_skip_reason(
                "flowchart",
                "upstream_flow_text_ellipse_vertex_parser_only_spec"
            ),
            Some("pinned Mermaid 11.16 cannot render this parser-only ellipse vertex fixture")
        );
        assert_eq!(
            svg_xml_compare_skip_reason(
                "flowchart",
                "upstream_html_demos_flowchart_flowchart_040_katex"
            ),
            None
        );
    }

    #[test]
    fn svg_xml_compare_skip_reason_covers_class_prototype_key_render_artifact() {
        let reason = svg_xml_compare_skip_reason("class", "upstream_parser_class_spec")
            .expect("class prototype-key render artifact should be explicitly skipped");
        assert!(reason.contains("prototype-key class ids"));
        assert_eq!(
            svg_xml_compare_skip_reason("class", "upstream_namespaces_and_generics"),
            None
        );
    }

    #[test]
    fn compare_svg_xml_defaults_flowchart_elk_to_source_ported_backend() {
        let out_root = crate::cmd::target_root()
            .join("compare")
            .join("xtask-tests")
            .join("xml_default_flowchart_elk");

        compare_svg_xml(vec![
            "--diagram".to_string(),
            "flowchart".to_string(),
            "--filter".to_string(),
            "upstream_html_demos_flowchart_elk_flowchart_elk_001".to_string(),
            "--check".to_string(),
            "--dom-mode".to_string(),
            "parity".to_string(),
            "--dom-decimals".to_string(),
            "3".to_string(),
            "--out-root".to_string(),
            out_root.display().to_string(),
        ])
        .expect("default source-backed backend should admit and match the ELK probe fixture");
    }
}
