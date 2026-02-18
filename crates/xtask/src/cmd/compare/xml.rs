//! SVG compare XML helpers.

use crate::XtaskError;
use crate::svgdom;
use crate::util::*;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

pub(crate) fn compare_svg_xml(args: Vec<String>) -> Result<(), XtaskError> {
    let mut check: bool = false;
    let mut dom_mode: Option<String> = None;
    let mut dom_decimals: Option<u32> = None;
    let mut filter: Option<String> = None;
    let mut text_measurer: Option<String> = None;
    let mut only_diagrams: Vec<String> = Vec::new();

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
            "--diagram" => {
                i += 1;
                let d = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
                if !d.is_empty() {
                    only_diagrams.push(d);
                }
            }
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

    let measurer: std::sync::Arc<dyn merman_render::text::TextMeasurer + Send + Sync> =
        match text_measurer.as_deref().unwrap_or("vendored") {
            "deterministic" => {
                std::sync::Arc::new(merman_render::text::DeterministicTextMeasurer::default())
            }
            _ => {
                std::sync::Arc::new(merman_render::text::VendoredFontMetricsTextMeasurer::default())
            }
        };

    // Mermaid gitGraph auto-generates commit ids using `Math.random()`. Upstream gitGraph SVGs in
    // this repo are generated with a seeded upstream renderer, so keep the local side seeded too
    // for meaningful strict XML comparisons.
    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({ "handDrawnSeed": 1, "gitGraph": { "seed": 1 } }),
    ));
    let layout_opts = merman_render::LayoutOptions {
        text_measurer: std::sync::Arc::clone(&measurer),
        ..Default::default()
    };

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let upstream_root = workspace_root.join("fixtures").join("upstream-svgs");
    let fixtures_root = workspace_root.join("fixtures");
    let out_root = workspace_root.join("target").join("compare").join("xml");

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

    let mut diagrams: Vec<String> = Vec::new();
    let Ok(entries) = fs::read_dir(&upstream_root) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list upstream svg directory {}",
            upstream_root.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !only_diagrams.is_empty() && !only_diagrams.iter().any(|d| d == name) {
            continue;
        }
        diagrams.push(name.to_string());
    }
    diagrams.sort();

    if diagrams.is_empty() {
        return Err(XtaskError::SvgCompareFailed(
            "no diagram directories matched under fixtures/upstream-svgs".to_string(),
        ));
    }

    let mut mismatches: Vec<(String, String, PathBuf, PathBuf)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    for diagram in diagrams {
        let upstream_dir = upstream_root.join(&diagram);
        let fixtures_dir = fixtures_root.join(&diagram);
        let out_dir = out_root.join(&diagram);
        fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
            path: out_dir.display().to_string(),
            source,
        })?;

        let Ok(svg_entries) = fs::read_dir(&upstream_dir) else {
            missing.push(format!(
                "{diagram}: failed to list {}",
                upstream_dir.display()
            ));
            continue;
        };

        let mut upstream_svgs: Vec<PathBuf> = Vec::new();
        for entry in svg_entries.flatten() {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            if !has_extension(&p, "svg") {
                continue;
            }
            if let Some(ref f) = filter {
                if !p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains(f))
                {
                    continue;
                }
            }
            upstream_svgs.push(p);
        }
        upstream_svgs.sort();

        for upstream_path in upstream_svgs {
            let Some(stem) = upstream_path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let fixture_path = fixtures_dir.join(format!("{stem}.mmd"));
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

            let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
                Ok(v) => v,
                Err(err) => {
                    missing.push(format!("{diagram}/{stem}: layout failed: {err}"));
                    continue;
                }
            };

            let mut svg_opts = merman_render::svg::SvgRenderOptions {
                diagram_id: Some(sanitize_svg_id(stem)),
                ..Default::default()
            };
            if diagram == "gantt" {
                if let merman_render::model::LayoutDiagram::GanttDiagram(layout) = &layouted.layout
                {
                    svg_opts.now_ms_override =
                        gantt_derive_now_ms_from_upstream_today(&upstream_svg, layout);
                }
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
