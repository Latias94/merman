use crate::XtaskError;
use crate::svgdom;
use crate::util::*;
use regex::Regex;
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub(crate) fn gen_upstream_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "er".to_string();
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut install: bool = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
            }
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--install" => install = true,
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_root =
        out_root.unwrap_or_else(|| workspace_root.join("fixtures").join("upstream-svgs"));

    let tools_root = workspace_root.join("tools").join("mermaid-cli");
    let node_modules = tools_root.join("node_modules");
    if install || !node_modules.exists() {
        let npm_cmd = if tools_root.join("package-lock.json").is_file() {
            "ci"
        } else {
            "install"
        };
        let mut cmd = if cfg!(windows) {
            let mut cmd = Command::new("cmd.exe");
            cmd.arg("/c").arg("npm").arg(npm_cmd);
            cmd
        } else {
            let mut cmd = Command::new("npm");
            cmd.arg(npm_cmd);
            cmd
        };
        let status = cmd.current_dir(&tools_root).status().map_err(|err| {
            XtaskError::UpstreamSvgFailed(format!(
                "failed to run `npm {npm_cmd}` in {}: {err}",
                tools_root.display()
            ))
        })?;
        if !status.success() {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "npm {npm_cmd} failed in {}",
                tools_root.display()
            )));
        }
    }

    let mmdc = find_mmdc(&tools_root).ok_or_else(|| {
        XtaskError::UpstreamSvgFailed(format!(
            "mmdc not found under {} (run: npm install)",
            tools_root.display()
        ))
    })?;

    fn run_one(
        workspace_root: &Path,
        out_root: &Path,
        mmdc: &Path,
        diagram: &str,
        filter: Option<&str>,
    ) -> Result<(), XtaskError> {
        let fixtures_dir = workspace_root.join("fixtures").join(diagram);
        let out_dir = out_root.join(diagram);
        let node_cwd = workspace_root.join("tools").join("mermaid-cli");
        let use_seeded_renderer = diagram == "architecture" || diagram == "gitgraph";
        let seeded_script = if use_seeded_renderer {
            Some(ensure_seeded_upstream_svg_renderer_script(workspace_root)?)
        } else {
            None
        };

        fn sanitize_svg_id(raw: &str) -> String {
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

        let mut mmd_files: Vec<PathBuf> = Vec::new();
        let Ok(entries) = fs::read_dir(&fixtures_dir) else {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "failed to list fixtures directory {}",
                fixtures_dir.display()
            )));
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().is_none_or(|e| e != "mmd") {
                continue;
            }
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
            {
                continue;
            }
            if diagram == "gantt"
                && path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    matches!(
                        n,
                        "click_loose.mmd"
                            | "click_strict.mmd"
                            | "dateformat_hash_comment_truncates.mmd"
                            | "excludes_hash_comment_truncates.mmd"
                            | "today_marker_and_axis.mmd"
                    )
                })
            {
                continue;
            }
            if diagram == "state"
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("_parser_") || n.contains("_parser_spec"))
            {
                continue;
            }
            if diagram == "class"
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("upstream_text_label_variants_spec"))
            {
                continue;
            }
            if diagram == "c4" {
                // Mermaid C4 has known render-time type assumptions that make some valid parser
                // fixtures non-renderable (e.g. kv-objects stored in `label.text` or
                // `UpdateElementStyle(..., techn="Rust")` storing `techn` as a raw string).
                //
                // Keep these fixtures for parser parity, but skip them for upstream SVG baselines.
                if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    matches!(
                        n,
                        "nesting_updates.mmd"
                            | "upstream_boundary_spec.mmd"
                            | "upstream_c4container_header_and_direction_spec.mmd"
                            | "upstream_container_spec.mmd"
                            | "upstream_person_ext_spec.mmd"
                            | "upstream_person_spec.mmd"
                            | "upstream_system_spec.mmd"
                            | "upstream_update_element_style_all_fields_spec.mmd"
                    )
                }) {
                    continue;
                }
            }
            if let Some(f) = filter {
                if !path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains(f))
                {
                    continue;
                }
            }
            mmd_files.push(path);
        }
        mmd_files.sort();

        if mmd_files.is_empty() {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "no .mmd fixtures matched under {}",
                fixtures_dir.display()
            )));
        }

        fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
            path: out_dir.display().to_string(),
            source,
        })?;

        let failures_path = out_dir.join("_failures.txt");
        let _ = fs::remove_file(&failures_path);

        let mut failures: Vec<String> = Vec::new();

        for mmd_path in mmd_files {
            let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
                failures.push(format!("invalid fixture filename {}", mmd_path.display()));
                continue;
            };
            let out_path = out_dir.join(format!("{stem}.svg"));
            let svg_id = sanitize_svg_id(stem);

            let status = if use_seeded_renderer {
                use std::io::Write;
                use std::process::Stdio;

                // Architecture layout relies on cytoscape-fcose, which uses `Math.random()` for
                // spectral initialization. To keep upstream baselines reproducible, we render via
                // a small puppeteer wrapper that seeds `Math.random()` deterministically.
                let pinned_config = node_cwd.join("mermaid-config.json");
                let seed: u64 = 1;
                let output_abs = if out_path.is_absolute() {
                    out_path.clone()
                } else {
                    workspace_root.join(&out_path)
                };

                let input_json = serde_json::json!({
                    "input_path": mmd_path.display().to_string(),
                    "output_path": output_abs.display().to_string(),
                    "config_path": pinned_config.display().to_string(),
                    "theme": "default",
                    "svg_id": svg_id,
                    "seed": seed,
                    "width": 800,
                    "height": 600,
                    "background_color": "white",
                })
                .to_string();

                let Some(script_path) = seeded_script.as_ref() else {
                    return Err(XtaskError::UpstreamSvgFailed(
                        "seeded renderer script not available".to_string(),
                    ));
                };

                let mut cmd = Command::new("node");
                cmd.arg(script_path)
                    .current_dir(&node_cwd)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit());
                let mut child = cmd.spawn().map_err(|err| {
                    XtaskError::UpstreamSvgFailed(format!(
                        "failed to spawn seeded upstream svg renderer: {err}"
                    ))
                })?;
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(input_json.as_bytes());
                }
                child.wait()
            } else {
                let mut cmd = if cfg!(windows) {
                    match mmdc.extension().and_then(|s| s.to_str()) {
                        Some(ext)
                            if ext.eq_ignore_ascii_case("cmd")
                                || ext.eq_ignore_ascii_case("bat") =>
                        {
                            let mut cmd = Command::new("cmd.exe");
                            cmd.arg("/c").arg(mmdc);
                            cmd
                        }
                        Some(ext) if ext.eq_ignore_ascii_case("ps1") => {
                            let mut cmd = Command::new("powershell.exe");
                            cmd.arg("-NoProfile")
                                .arg("-ExecutionPolicy")
                                .arg("Bypass")
                                .arg("-File")
                                .arg(mmdc);
                            cmd
                        }
                        _ => Command::new(mmdc),
                    }
                } else {
                    Command::new(mmdc)
                };
                cmd.arg("-i")
                    .arg(&mmd_path)
                    .arg("-o")
                    .arg(&out_path)
                    .arg("-t")
                    .arg("default");

                // Stabilize Rough.js output across runs. Mermaid uses Rough.js for many "classic look"
                // shapes too (often with `roughness: 0`), but the stroke control points still depend on
                // `random()` via `divergePoint`. Pin `handDrawnSeed` for reproducible upstream SVG
                // baselines.
                let pinned_config = workspace_root
                    .join("tools")
                    .join("mermaid-cli")
                    .join("mermaid-config.json");
                cmd.arg("-c").arg(pinned_config);

                // Gantt rendering depends on the page width (`parentElement.offsetWidth`). In a
                // headless Rust context we default to the Mermaid fallback width (1200) when no DOM
                // width is available. Use the same page width for upstream baselines so parity diffs
                // remain meaningful.
                if diagram == "gantt" {
                    cmd.arg("-w").arg("1200");
                }

                cmd.arg("--svgId").arg(svg_id);
                cmd.status()
            };

            match status {
                Ok(s) if s.success() => {
                    // Some upstream renderer failures surface only as console errors while still
                    // returning a successful exit code. Treat missing/empty outputs as failures so
                    // we don't silently accept a broken baseline corpus.
                    match fs::metadata(&out_path) {
                        Ok(meta) if meta.is_file() && meta.len() > 0 => {}
                        Ok(meta) => failures.push(format!(
                            "mmdc succeeded but output is empty for {} (out={}, bytes={})",
                            mmd_path.display(),
                            out_path.display(),
                            meta.len()
                        )),
                        Err(err) => failures.push(format!(
                            "mmdc succeeded but output is missing for {} (out={}, err={})",
                            mmd_path.display(),
                            out_path.display(),
                            err
                        )),
                    }
                }
                Ok(s) => failures.push(format!(
                    "mmdc failed for {} (exit={})",
                    mmd_path.display(),
                    s.code().unwrap_or(-1)
                )),
                Err(err) => failures.push(format!("mmdc failed for {}: {err}", mmd_path.display())),
            }
        }

        if failures.is_empty() {
            return Ok(());
        }

        let _ = fs::write(&failures_path, failures.join("\n"));

        Err(XtaskError::UpstreamSvgFailed(failures.join("\n")))
    }

    let filter = filter.as_deref();
    match diagram.as_str() {
        "all" => {
            let mut failures: Vec<String> = Vec::new();
            for d in [
                "er",
                "flowchart",
                "gantt",
                "architecture",
                "mindmap",
                "state",
                "class",
                "sequence",
                "info",
                "pie",
                "sankey",
                "requirement",
                "packet",
                "timeline",
                "journey",
                "kanban",
                "gitgraph",
                "quadrantchart",
                "c4",
                "block",
                "radar",
                "treemap",
                "xychart",
            ] {
                if let Err(err) = run_one(&workspace_root, &out_root, &mmdc, d, filter) {
                    failures.push(format!("{d}: {err}"));
                }
            }
            if failures.is_empty() {
                Ok(())
            } else {
                Err(XtaskError::UpstreamSvgFailed(failures.join("\n")))
            }
        }
        "er" | "flowchart" | "state" | "class" | "sequence" | "info" | "pie" | "requirement"
        | "sankey" | "packet" | "timeline" | "journey" | "kanban" | "gitgraph" | "gantt" | "c4"
        | "block" | "radar" | "quadrantchart" | "treemap" | "xychart" | "mindmap"
        | "architecture" => run_one(&workspace_root, &out_root, &mmdc, &diagram, filter),
        other => Err(XtaskError::UpstreamSvgFailed(format!(
            "unsupported diagram for upstream svg export: {other} (supported: er, flowchart, gantt, architecture, mindmap, state, class, sequence, info, pie, sankey, requirement, packet, timeline, journey, kanban, gitgraph, quadrantchart, c4, block, radar, treemap, xychart, all)"
        ))),
    }
}

pub(crate) fn check_upstream_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "er".to_string();
    let mut filter: Option<String> = None;
    let mut install: bool = false;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "strict".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--install" => install = true,
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "strict".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let baseline_root = workspace_root.join("fixtures").join("upstream-svgs");
    let out_root = workspace_root.join("target").join("upstream-svgs-check");

    let mut gen_args: Vec<String> = vec![
        "--diagram".to_string(),
        diagram.clone(),
        "--out".to_string(),
        out_root.to_string_lossy().to_string(),
    ];
    if let Some(f) = &filter {
        gen_args.push("--filter".to_string());
        gen_args.push(f.clone());
    }
    if install {
        gen_args.push("--install".to_string());
    }

    gen_upstream_svgs(gen_args)?;

    #[allow(clippy::too_many_arguments)]
    fn check_one(
        workspace_root: &Path,
        baseline_root: &Path,
        out_root: &Path,
        diagram: &str,
        filter: Option<&str>,
        check_dom: bool,
        dom_mode: svgdom::DomMode,
        dom_decimals: u32,
    ) -> Result<(), XtaskError> {
        let fixtures_dir = workspace_root.join("fixtures").join(diagram);
        let baseline_dir = baseline_root.join(diagram);
        let out_dir = out_root.join(diagram);

        let mut mmd_files: Vec<PathBuf> = Vec::new();
        let Ok(entries) = fs::read_dir(&fixtures_dir) else {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "failed to list fixtures directory {}",
                fixtures_dir.display()
            )));
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().is_none_or(|e| e != "mmd") {
                continue;
            }
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
            {
                continue;
            }
            if diagram == "gantt"
                && path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    matches!(
                        n,
                        "click_loose.mmd"
                            | "click_strict.mmd"
                            | "dateformat_hash_comment_truncates.mmd"
                            | "excludes_hash_comment_truncates.mmd"
                            | "today_marker_and_axis.mmd"
                    )
                })
            {
                continue;
            }
            if diagram == "state"
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("_parser_") || n.contains("_parser_spec"))
            {
                continue;
            }
            if diagram == "class"
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("upstream_text_label_variants_spec"))
            {
                continue;
            }
            if diagram == "c4"
                && path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    matches!(
                        n,
                        "nesting_updates.mmd"
                            | "upstream_boundary_spec.mmd"
                            | "upstream_c4container_header_and_direction_spec.mmd"
                            | "upstream_container_spec.mmd"
                            | "upstream_person_ext_spec.mmd"
                            | "upstream_person_spec.mmd"
                            | "upstream_system_spec.mmd"
                            | "upstream_update_element_style_all_fields_spec.mmd"
                    )
                })
            {
                continue;
            }
            if let Some(f) = filter {
                if !path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains(f))
                {
                    continue;
                }
            }
            mmd_files.push(path);
        }
        mmd_files.sort();

        if mmd_files.is_empty() {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "no .mmd fixtures matched under {}",
                fixtures_dir.display()
            )));
        }

        let mut mismatches: Vec<String> = Vec::new();
        for mmd_path in mmd_files {
            let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
                mismatches.push(format!("invalid fixture filename {}", mmd_path.display()));
                continue;
            };

            let baseline_path = baseline_dir.join(format!("{stem}.svg"));
            let out_path = out_dir.join(format!("{stem}.svg"));

            let baseline_svg = match fs::read_to_string(&baseline_path) {
                Ok(v) => v,
                Err(err) => {
                    mismatches.push(format!(
                        "missing baseline svg: {} ({err})",
                        baseline_path.display()
                    ));
                    continue;
                }
            };
            let out_svg = match fs::read_to_string(&out_path) {
                Ok(v) => v,
                Err(err) => {
                    mismatches.push(format!(
                        "missing generated svg: {} ({err})",
                        out_path.display()
                    ));
                    continue;
                }
            };

            let (use_dom, mode) = if check_dom {
                (true, dom_mode)
            } else if diagram == "state"
                || diagram == "gitgraph"
                || diagram == "gantt"
                || diagram == "er"
                || diagram == "class"
                || diagram == "requirement"
                || diagram == "block"
                || diagram == "mindmap"
                || diagram == "architecture"
            {
                (true, svgdom::DomMode::Structure)
            } else {
                (false, dom_mode)
            };

            if use_dom {
                let a = match svgdom::dom_signature(&baseline_svg, mode, dom_decimals) {
                    Ok(v) => v,
                    Err(err) => {
                        mismatches.push(format!(
                            "{diagram}/{stem}: baseline dom parse failed: {err}"
                        ));
                        continue;
                    }
                };
                let b = match svgdom::dom_signature(&out_svg, mode, dom_decimals) {
                    Ok(v) => v,
                    Err(err) => {
                        mismatches.push(format!(
                            "{diagram}/{stem}: generated dom parse failed: {err}"
                        ));
                        continue;
                    }
                };
                if a != b {
                    mismatches.push(format!("{diagram}/{stem}: dom differs from baseline"));
                }
            } else if baseline_svg != out_svg {
                mismatches.push(format!("{diagram}/{stem}: output differs from baseline"));
            }
        }

        if mismatches.is_empty() {
            Ok(())
        } else {
            Err(XtaskError::UpstreamSvgFailed(mismatches.join("\n")))
        }
    }

    let filter = filter.as_deref();
    let parsed_dom_mode = svgdom::DomMode::parse(&dom_mode);
    match diagram.as_str() {
        "all" => {
            let mut failures: Vec<String> = Vec::new();
            for d in [
                "er",
                "flowchart",
                "gantt",
                "architecture",
                "mindmap",
                "state",
                "class",
                "sequence",
                "info",
                "pie",
                "sankey",
                "requirement",
                "packet",
                "timeline",
                "journey",
                "kanban",
                "gitgraph",
                "quadrantchart",
                "c4",
                "block",
                "radar",
                "treemap",
            ] {
                if let Err(err) = check_one(
                    &workspace_root,
                    &baseline_root,
                    &out_root,
                    d,
                    filter,
                    check_dom,
                    parsed_dom_mode,
                    dom_decimals,
                ) {
                    failures.push(format!("{d}: {err}"));
                }
            }
            if failures.is_empty() {
                Ok(())
            } else {
                Err(XtaskError::UpstreamSvgFailed(failures.join("\n")))
            }
        }
        "er" | "flowchart" | "state" | "class" | "sequence" | "info" | "pie" | "requirement"
        | "sankey" | "packet" | "timeline" | "journey" | "kanban" | "gitgraph" | "gantt" | "c4"
        | "block" | "radar" | "quadrantchart" | "treemap" | "mindmap" | "architecture" => {
            check_one(
                &workspace_root,
                &baseline_root,
                &out_root,
                diagram.as_str(),
                filter,
                check_dom,
                parsed_dom_mode,
                dom_decimals,
            )
        }
        other => Err(XtaskError::UpstreamSvgFailed(format!(
            "unsupported diagram for upstream svg check: {other} (supported: er, flowchart, gantt, architecture, mindmap, state, class, sequence, info, pie, sankey, requirement, packet, timeline, journey, kanban, gitgraph, quadrantchart, c4, block, radar, treemap, all)"
        ))),
    }
}

pub(crate) fn find_mmdc(tools_root: &Path) -> Option<PathBuf> {
    let bin_root = tools_root.join("node_modules").join(".bin");
    for name in ["mmdc.cmd", "mmdc.ps1", "mmdc"] {
        let p = bin_root.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub(crate) fn ensure_seeded_upstream_svg_renderer_script(
    workspace_root: &Path,
) -> Result<PathBuf, XtaskError> {
    const JS: &str = r#"
const fs = require('fs');
const path = require('path');
const url = require('url');
const { createRequire } = require('module');
const requireFromCwd = createRequire(path.join(process.cwd(), 'package.json'));
const puppeteer = requireFromCwd('puppeteer');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const inputPath = String(input.input_path || '');
const outputPath = String(input.output_path || '');
const configPath = String(input.config_path || '');
const theme = String(input.theme || 'default');
const svgId = String(input.svg_id || 'diagram');
const seedStr = String((input.seed ?? 1));
const width = Number(input.width || 800);
const height = Number(input.height || 600);
const backgroundColor = String(input.background_color || 'white');
const debug = process.env.MERMAN_SEEDED_UPSTREAM_SVG_DEBUG === '1';

if (!inputPath || !outputPath || !configPath) {
  console.error('missing required input/output/config path');
  process.exit(2);
}

const cliRoot = process.cwd();
const mermaidHtmlPath = path.join(cliRoot, 'node_modules', '@mermaid-js', 'mermaid-cli', 'dist', 'index.html');
const mermaidIifePath = path.join(cliRoot, 'node_modules', 'mermaid', 'dist', 'mermaid.js');
const zenumlIifePath = path.join(cliRoot, 'node_modules', '@mermaid-js', 'mermaid-zenuml', 'dist', 'mermaid-zenuml.js');

(async () => {
  const code = fs.readFileSync(inputPath, 'utf8');
  const cfg = JSON.parse(fs.readFileSync(configPath, 'utf8'));

  const launchOpts = { headless: 'shell', args: ['--no-sandbox', '--disable-setuid-sandbox', '--allow-file-access-from-files'] };
  const browser = await puppeteer.launch(launchOpts);
  const page = await browser.newPage();
  if (process.env.MERMAN_SEEDED_UPSTREAM_SVG_DEBUG === '1') {
    page.on('console', (msg) => {
      if (!msg || typeof msg.type !== 'function') return;
      const ty = msg.type();
      if (ty === 'error' || ty === 'warning') {
        console.error(`[browser:console.${ty}] ${msg.text()}`);
      }
    });
    page.on('pageerror', (err) => {
      console.error(`[browser:pageerror] ${err && err.stack ? err.stack : String(err)}`);
    });
  }

  await page.evaluateOnNewDocument((seedStr) => {
    const mask64 = (1n << 64n) - 1n;
    let state = (BigInt(seedStr) & mask64);
    if (state === 0n) state = 1n;

    function nextU64() {
      let x = state;
      x ^= (x >> 12n);
      x ^= (x << 25n) & mask64;
      x ^= (x >> 27n);
      state = x;
      return (x * 0x2545F4914F6CDD1Dn) & mask64;
    }

    function nextF64() {
      const u = nextU64() >> 11n;
      return Number(u) / 9007199254740992; // 2^53
    }

    Math.random = nextF64;

    if (globalThis.crypto && typeof globalThis.crypto.getRandomValues === 'function') {
      const orig = globalThis.crypto.getRandomValues.bind(globalThis.crypto);
      globalThis.crypto.getRandomValues = (arr) => {
        if (!arr || typeof arr.length !== 'number') {
          return orig(arr);
        }
        // Fill the underlying bytes so behavior is consistent for Uint8/16/32 arrays.
        try {
          const bytes = new Uint8Array(arr.buffer, arr.byteOffset || 0, arr.byteLength || 0);
          for (let i = 0; i < bytes.length; i++) {
            bytes[i] = Math.floor(nextF64() * 256);
          }
          return arr;
        } catch (e) {
          // Fall back to original behavior if this isn't a typed array.
          return orig(arr);
        }
      };
    }
  }, seedStr);

  await page.setViewport({ width: Math.max(1, width), height: Math.max(1, height), deviceScaleFactor: 1 });
  await page.goto(url.pathToFileURL(mermaidHtmlPath).href);
  await Promise.all([
    page.addScriptTag({ path: mermaidIifePath }),
    page.addScriptTag({ path: zenumlIifePath }),
  ]);

  const svg = await page.evaluate(async ({ code, cfg, theme, svgId, width, debug }) => {
    const mermaid = globalThis.mermaid;
    if (!mermaid) throw new Error('global mermaid instance not found (mermaid.js)');

    if (document.fonts && typeof document.fonts[Symbol.iterator] === 'function') {
      await Promise.all(Array.from(document.fonts, (font) => font.load()));
    }

    // Match mermaid-cli behavior: register external diagrams and layout loaders.
    const zenuml = globalThis['mermaid-zenuml'];
    if (zenuml && typeof mermaid.registerExternalDiagrams === 'function') {
      await mermaid.registerExternalDiagrams([zenuml]);
    }
    const elkLayouts = globalThis.elkLayouts;
    if (elkLayouts && typeof mermaid.registerLayoutLoaders === 'function') {
      mermaid.registerLayoutLoaders(elkLayouts);
    }

    mermaid.initialize(Object.assign({ startOnLoad: false, theme }, cfg));

    const container = document.getElementById('container') || document.body;
    container.innerHTML = '';
    container.style.width = `${Math.max(1, Number(width) || 1)}px`;

    // Surface parse errors early; some Mermaid failures otherwise only manifest as a missing `svg`.
    if (typeof mermaid.parse === 'function') {
      try {
        await mermaid.parse(code);
      } catch (err) {
        if (!debug) throw err;
        return {
          ok: false,
          stage: 'parse',
          error: String(err && err.message ? err.message : err),
          stack: String(err && err.stack ? err.stack : ''),
        };
      }
    }

    async function tryRenderViaMermaidRender() {
      if (typeof mermaid.render !== 'function') return undefined;
      const rendered = await mermaid.render(svgId, code, container);
      let svg =
        typeof rendered === 'string'
          ? rendered
          : Array.isArray(rendered)
            ? rendered[0]
            : rendered && rendered.svg;
      if (typeof svg !== 'string' && rendered != null) {
        const asStr = String(rendered);
        if (asStr.trim().startsWith('<svg')) {
          svg = asStr;
        }
      }
      if (typeof svg === 'string') return svg;
      const domSvg = container.querySelector && container.querySelector('svg');
      if (domSvg && typeof domSvg.outerHTML === 'string' && domSvg.outerHTML.trim().startsWith('<svg')) {
        return domSvg.outerHTML;
      }
      return undefined;
    }

    async function tryRenderViaMermaidApi() {
      const api = mermaid.mermaidAPI;
      if (!api || typeof api.render !== 'function') return undefined;
      return await new Promise((resolve, reject) => {
        try {
          api.render(svgId, code, (svgCode) => resolve(svgCode), container);
        } catch (err) {
          reject(err);
        }
      });
    }

    const svgText = (await tryRenderViaMermaidRender()) ?? (await tryRenderViaMermaidApi());
    if (typeof svgText !== 'string') {
      if (!debug) {
        throw new Error('mermaid.render returned no svg output');
      }
      return {
        ok: false,
        stage: 'render',
        svgTextType: typeof svgText,
        containerHtmlLen: typeof container.innerHTML === 'string' ? container.innerHTML.length : -1,
      };
    }

    container.innerHTML = svgText;
    const svgEl = container.getElementsByTagName?.('svg')?.[0];
    if (!svgEl) {
      if (debug) {
        return { ok: true, stage: 'no-svg-el', svgTextLen: svgText.length };
      }
      return svgText;
    }

    // Mirror mermaid-cli SVG output shape (XMLSerializer), so outputs are valid XML.
    // eslint-disable-next-line no-undef
    const xmlSerializer = new XMLSerializer();
    const xml = xmlSerializer.serializeToString(svgEl);
    if (debug) {
      return { ok: true, stage: 'ok', svgTextLen: svgText.length, serializedLen: xml.length };
    }
    return xml;
  }, { code, cfg, theme, svgId, width, debug });

  if (debug) {
    if (typeof svg !== 'string') {
      console.error(JSON.stringify(svg, null, 2));
      process.exit(1);
    }
    console.error(`[debug] expected diagnostics object, got svg string len=${svg.length}`);
    process.exit(1);
  }

  function ensureSvgBackgroundColor(svgText, bg) {
    if (typeof svgText !== 'string') {
      throw new Error(`expected svg string from mermaid.render, got ${typeof svgText}`);
    }
    if (!bg) return svgText;
    if (svgText.includes('background-color:')) return svgText;
    const m = svgText.match(/<svg\b[^>]*\bstyle="([^"]*)"/);
    if (m) {
      const raw = m[1] || '';
      let next = raw.trim();
      if (next.length > 0 && !next.trim().endsWith(';')) {
        next += ';';
      }
      next += ` background-color: ${bg};`;
      return svgText.replace(m[0], m[0].replace(raw, next));
    }
    // Fallback: inject a style attr into the root <svg>.
    return svgText.replace(/<svg\b/, `<svg style="background-color: ${bg};"`);
  }

  const svgWithBg = ensureSvgBackgroundColor(svg, backgroundColor);
  fs.writeFileSync(outputPath, svgWithBg, 'utf8');
  await browser.close();
})().catch((err) => {
  console.error(err && err.stack ? err.stack : String(err));
  process.exit(1);
});
"#;

    let dir = workspace_root.join("target").join("xtask-js");
    fs::create_dir_all(&dir).map_err(|source| XtaskError::WriteFile {
        path: dir.display().to_string(),
        source,
    })?;
    let script_path = dir.join("seeded-upstream-svg-render.js");
    fs::write(&script_path, JS).map_err(|source| XtaskError::WriteFile {
        path: script_path.display().to_string(),
        source,
    })?;
    Ok(script_path)
}

pub(crate) fn gen_er_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_root = out_root.unwrap_or_else(|| workspace_root.join("target").join("svgs"));

    let fixtures_dir = workspace_root.join("fixtures").join("er");
    let out_dir = out_root.join("er");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({ "handDrawnSeed": 1 }),
    ));
    let layout_opts = merman_render::LayoutOptions::default();
    let mut failures: Vec<String> = Vec::new();

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
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
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::ErDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let svg = match merman_render::svg::render_er_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let out_path = out_dir.join(format!("{stem}.svg"));
        if let Err(err) = fs::write(&out_path, svg) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::DebugSvgFailed(failures.join("\n")))
}

pub(crate) fn gen_debug_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "class".to_string();
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
            }
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_root = out_root.unwrap_or_else(|| workspace_root.join("target").join("debug-svgs"));

    fn gen_one(
        workspace_root: &Path,
        out_root: &Path,
        diagram: &str,
        filter: Option<&str>,
    ) -> Result<(), XtaskError> {
        let (fixtures_dir, out_dir) = match diagram {
            "flowchart" | "flowchart-v2" | "flowchartV2" => (
                workspace_root.join("fixtures").join("flowchart"),
                out_root.join("flowchart"),
            ),
            "state" | "stateDiagram" | "stateDiagram-v2" | "stateDiagramV2" => (
                workspace_root.join("fixtures").join("state"),
                out_root.join("state"),
            ),
            "class" | "classDiagram" => (
                workspace_root.join("fixtures").join("class"),
                out_root.join("class"),
            ),
            "er" | "erDiagram" => (
                workspace_root.join("fixtures").join("er"),
                out_root.join("er"),
            ),
            "sequence" => (
                workspace_root.join("fixtures").join("sequence"),
                out_root.join("sequence"),
            ),
            "info" => (
                workspace_root.join("fixtures").join("info"),
                out_root.join("info"),
            ),
            "pie" => (
                workspace_root.join("fixtures").join("pie"),
                out_root.join("pie"),
            ),
            "packet" => (
                workspace_root.join("fixtures").join("packet"),
                out_root.join("packet"),
            ),
            other => {
                return Err(XtaskError::DebugSvgFailed(format!(
                    "unsupported diagram for debug svg export: {other} (supported: flowchart, state, class, er, sequence, info, pie, packet)"
                )));
            }
        };

        let mut mmd_files: Vec<PathBuf> = Vec::new();
        let Ok(entries) = fs::read_dir(&fixtures_dir) else {
            return Err(XtaskError::DebugSvgFailed(format!(
                "failed to list fixtures directory {}",
                fixtures_dir.display()
            )));
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().is_none_or(|e| e != "mmd") {
                continue;
            }
            if let Some(f) = filter {
                if !path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains(f))
                {
                    continue;
                }
            }
            mmd_files.push(path);
        }
        mmd_files.sort();

        if mmd_files.is_empty() {
            return Err(XtaskError::DebugSvgFailed(format!(
                "no .mmd fixtures matched under {}",
                fixtures_dir.display()
            )));
        }

        fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
            path: out_dir.display().to_string(),
            source,
        })?;

        let engine = merman::Engine::new();
        let mut failures: Vec<String> = Vec::new();

        for mmd_path in mmd_files {
            let text = match fs::read_to_string(&mmd_path) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                    continue;
                }
            };

            let parsed = match futures::executor::block_on(
                engine.parse_diagram(&text, merman::ParseOptions::default()),
            ) {
                Ok(Some(v)) => v,
                Ok(None) => {
                    failures.push(format!("no diagram detected in {}", mmd_path.display()));
                    continue;
                }
                Err(err) => {
                    failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                    continue;
                }
            };

            let layout_opts = merman_render::LayoutOptions::default();
            let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                    continue;
                }
            };

            let svg = match &layouted.layout {
                merman_render::model::LayoutDiagram::FlowchartV2(layout) => {
                    Ok(merman_render::svg::render_flowchart_v2_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    ))
                }
                merman_render::model::LayoutDiagram::StateDiagramV2(layout) => {
                    Ok(merman_render::svg::render_state_diagram_v2_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    ))
                }
                merman_render::model::LayoutDiagram::ClassDiagramV2(layout) => {
                    Ok(merman_render::svg::render_class_diagram_v2_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    ))
                }
                merman_render::model::LayoutDiagram::ErDiagram(layout) => {
                    Ok(merman_render::svg::render_er_diagram_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    ))
                }
                merman_render::model::LayoutDiagram::SequenceDiagram(layout) => {
                    Ok(merman_render::svg::render_sequence_diagram_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    ))
                }
                merman_render::model::LayoutDiagram::InfoDiagram(layout) => {
                    merman_render::svg::render_info_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "info svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                merman_render::model::LayoutDiagram::PieDiagram(layout) => {
                    merman_render::svg::render_pie_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "pie svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                merman_render::model::LayoutDiagram::PacketDiagram(layout) => {
                    merman_render::svg::render_packet_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        layouted.meta.title.as_deref(),
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "packet svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                merman_render::model::LayoutDiagram::TimelineDiagram(layout) => {
                    merman_render::svg::render_timeline_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        layouted.meta.title.as_deref(),
                        layout_opts.text_measurer.as_ref(),
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "timeline svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                merman_render::model::LayoutDiagram::JourneyDiagram(layout) => {
                    merman_render::svg::render_journey_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        layouted.meta.title.as_deref(),
                        layout_opts.text_measurer.as_ref(),
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "journey svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                merman_render::model::LayoutDiagram::KanbanDiagram(layout) => {
                    merman_render::svg::render_kanban_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "kanban svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                _ => Err(XtaskError::DebugSvgFailed(format!(
                    "unsupported layout for debug svg export: {} ({})",
                    mmd_path.display(),
                    layouted.meta.diagram_type
                ))),
            };

            let svg = match svg {
                Ok(v) => v,
                Err(err) => {
                    failures.push(err.to_string());
                    continue;
                }
            };

            let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
                failures.push(format!("invalid fixture filename {}", mmd_path.display()));
                continue;
            };
            let out_path = out_dir.join(format!("{stem}.svg"));
            if let Err(err) = fs::write(&out_path, svg) {
                failures.push(format!("failed to write {}: {err}", out_path.display()));
                continue;
            }
        }

        if failures.is_empty() {
            return Ok(());
        }

        Err(XtaskError::DebugSvgFailed(failures.join("\n")))
    }

    let filter = filter.as_deref();
    let diagrams: Vec<&str> = match diagram.as_str() {
        "all" => vec!["flowchart", "state", "class", "er"],
        other => vec![other],
    };

    let mut failures: Vec<String> = Vec::new();
    for d in diagrams {
        if let Err(err) = gen_one(&workspace_root, &out_root, d, filter) {
            failures.push(format!("{d}: {err}"));
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::DebugSvgFailed(failures.join("\n")))
}

pub(crate) fn gen_default_config(args: Vec<String>) -> Result<(), XtaskError> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Err(XtaskError::Usage);
    }

    let mut schema_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--schema" => {
                i += 1;
                schema_path = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let schema_path = schema_path.unwrap_or_else(|| {
        PathBuf::from("repo-ref/mermaid/packages/mermaid/src/schemas/config.schema.yaml")
    });
    let out_path = out_path
        .unwrap_or_else(|| PathBuf::from("crates/merman-core/src/generated/default_config.json"));

    let schema_text = fs::read_to_string(&schema_path).map_err(|source| XtaskError::ReadFile {
        path: schema_path.display().to_string(),
        source,
    })?;
    let schema_yaml: YamlValue = serde_yaml::from_str(&schema_text)?;

    let Some(root_defaults) = extract_defaults(&schema_yaml, &schema_yaml) else {
        return Err(XtaskError::InvalidRef(
            "schema produced no defaults (unexpected)".to_string(),
        ));
    };

    let pretty = serde_json::to_string_pretty(&root_defaults)?;
    let out_dir = out_path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    fs::write(&out_path, pretty).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}

pub(crate) fn gen_dompurify_defaults(args: Vec<String>) -> Result<(), XtaskError> {
    let mut src_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--src" => {
                i += 1;
                src_path = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let src_path =
        src_path.unwrap_or_else(|| PathBuf::from("repo-ref/dompurify/dist/purify.cjs.js"));
    let out_path = out_path
        .unwrap_or_else(|| PathBuf::from("crates/merman-core/src/generated/dompurify_defaults.rs"));

    let src_text = fs::read_to_string(&src_path).map_err(|source| XtaskError::ReadFile {
        path: src_path.display().to_string(),
        source,
    })?;

    let html_tags = extract_frozen_string_array(&src_text, "html$1")?;
    let svg_tags = extract_frozen_string_array(&src_text, "svg$1")?;
    let svg_filters = extract_frozen_string_array(&src_text, "svgFilters")?;
    let mathml_tags = extract_frozen_string_array(&src_text, "mathMl$1")?;

    let html_attrs = extract_frozen_string_array(&src_text, "html")?;
    let svg_attrs = extract_frozen_string_array(&src_text, "svg")?;
    let mathml_attrs = extract_frozen_string_array(&src_text, "mathMl")?;
    let xml_attrs = extract_frozen_string_array(&src_text, "xml")?;

    let default_data_uri_tags =
        extract_add_to_set_string_array(&src_text, "DEFAULT_DATA_URI_TAGS")?;
    let default_uri_safe_attrs =
        extract_add_to_set_string_array(&src_text, "DEFAULT_URI_SAFE_ATTRIBUTES")?;

    let allowed_tags = unique_sorted_lowercase(
        html_tags
            .into_iter()
            .chain(svg_tags)
            .chain(svg_filters)
            .chain(mathml_tags),
    );

    let allowed_attrs = unique_sorted_lowercase(
        html_attrs
            .into_iter()
            .chain(svg_attrs)
            .chain(mathml_attrs)
            .chain(xml_attrs),
    );

    let data_uri_tags = unique_sorted_lowercase(default_data_uri_tags);
    let uri_safe_attrs = unique_sorted_lowercase(default_uri_safe_attrs);

    let out_dir = out_path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let rust = render_dompurify_defaults_rs(
        &allowed_tags,
        &allowed_attrs,
        &uri_safe_attrs,
        &data_uri_tags,
    );
    fs::write(&out_path, rust).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}

pub(crate) fn render_dompurify_defaults_rs(
    allowed_tags: &[String],
    allowed_attrs: &[String],
    uri_safe_attrs: &[String],
    data_uri_tags: &[String],
) -> String {
    fn render_slice(name: &str, values: &[String]) -> String {
        let mut out = String::new();
        // Keep small slices compact for readability and stable diffs.
        if values.len() <= 8 {
            out.push_str(&format!("pub const {name}: &[&str] = &["));
            for (i, v) in values.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&format!("{v:?}"));
            }
            out.push_str("];\n\n");
            return out;
        }
        out.push_str(&format!("pub const {name}: &[&str] = &[\n"));
        for v in values {
            out.push_str(&format!("    {v:?},\n"));
        }
        out.push_str("];\n\n");
        out
    }

    let mut out = String::new();
    out.push_str("// This file is @generated by `cargo run -p xtask -- gen-dompurify-defaults`.\n");
    out.push_str("// Source: `repo-ref/dompurify/dist/purify.cjs.js` (DOMPurify 3.2.5)\n\n");
    out.push_str(&render_slice("DEFAULT_ALLOWED_TAGS", allowed_tags));
    out.push_str(&render_slice("DEFAULT_ALLOWED_ATTR", allowed_attrs));
    out.push_str(&render_slice("DEFAULT_URI_SAFE_ATTRIBUTES", uri_safe_attrs));
    out.push_str(&render_slice("DEFAULT_DATA_URI_TAGS", data_uri_tags));
    out
}

fn unique_sorted_lowercase<I>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut set = std::collections::BTreeSet::new();
    for v in values {
        set.insert(v.to_ascii_lowercase());
    }
    set.into_iter().collect()
}

pub(crate) fn gen_flowchart_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_root = out_root.unwrap_or_else(|| workspace_root.join("target").join("svgs"));

    let fixtures_dir = workspace_root.join("fixtures").join("flowchart");
    let out_dir = out_root.join("flowchart");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let engine = merman::Engine::new();
    let layout_opts = merman_render::LayoutOptions::default();
    let mut failures: Vec<String> = Vec::new();

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::FlowchartV2(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let svg = match merman_render::svg::render_flowchart_v2_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let out_path = out_dir.join(format!("{stem}.svg"));
        if let Err(err) = fs::write(&out_path, svg) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::DebugSvgFailed(failures.join("\n")))
}

pub(crate) fn gen_state_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_root = out_root.unwrap_or_else(|| workspace_root.join("target").join("svgs"));

    let fixtures_dir = workspace_root.join("fixtures").join("state");
    let out_dir = out_root.join("state");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let engine = merman::Engine::new();
    let layout_opts = merman_render::LayoutOptions::default();
    let mut failures: Vec<String> = Vec::new();

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::StateDiagramV2(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let svg = match merman_render::svg::render_state_diagram_v2_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let out_path = out_dir.join(format!("{stem}.svg"));
        if let Err(err) = fs::write(&out_path, svg) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::DebugSvgFailed(failures.join("\n")))
}

pub(crate) fn gen_class_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_root = out_root.unwrap_or_else(|| workspace_root.join("target").join("svgs"));

    let fixtures_dir = workspace_root.join("fixtures").join("class");
    let out_dir = out_root.join("class");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let engine = merman::Engine::new();
    let layout_opts = merman_render::LayoutOptions::default();
    let mut failures: Vec<String> = Vec::new();

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let is_classdiagram_v2_header = merman::preprocess_diagram(&text, engine.registry())
            .ok()
            .map(|p| p.code.trim_start().starts_with("classDiagram-v2"))
            .unwrap_or(false);

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::ClassDiagramV2(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            aria_roledescription: is_classdiagram_v2_header.then(|| "classDiagram".to_string()),
            ..Default::default()
        };

        let svg = match merman_render::svg::render_class_diagram_v2_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let out_path = out_dir.join(format!("{stem}.svg"));
        if let Err(err) = fs::write(&out_path, svg) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::DebugSvgFailed(failures.join("\n")))
}

pub(crate) fn gen_c4_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let out_root = out_root.unwrap_or_else(|| workspace_root.join("target").join("svgs"));

    let fixtures_dir = workspace_root.join("fixtures").join("c4");
    let out_dir = out_root.join("c4");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    // Keep this aligned with `crates/merman-render/tests/layout_snapshots_test.rs` so the
    // `update-layout-snapshots` output matches the test's computed layouts.
    let engine = merman_core::Engine::new();
    let layout_opts = merman_render::LayoutOptions::default();
    let mut failures: Vec<String> = Vec::new();

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(engine.parse_diagram(
            &text,
            merman_core::ParseOptions {
                suppress_errors: true,
            },
        )) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::C4Diagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let svg = match merman_render::svg::render_c4_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let out_path = out_dir.join(format!("{stem}.svg"));
        if let Err(err) = fs::write(&out_path, svg) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::DebugSvgFailed(failures.join("\n")))
}

pub(crate) fn gen_c4_textlength(args: Vec<String>) -> Result<(), XtaskError> {
    let mut in_dir: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--in" => {
                i += 1;
                in_dir = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    let in_dir = in_dir.unwrap_or_else(|| {
        workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join("c4")
    });
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("crates")
            .join("merman-render")
            .join("src")
            .join("generated")
            .join("c4_type_textlength_11_12_2.rs")
    });

    let mut svg_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&in_dir) else {
        return Err(XtaskError::VerifyFailed(format!(
            "failed to list C4 upstream svg directory {}",
            in_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "svg") {
            continue;
        }
        svg_files.push(path);
    }
    svg_files.sort();
    if svg_files.is_empty() {
        return Err(XtaskError::VerifyFailed(format!(
            "no C4 upstream SVG files found under {}",
            in_dir.display()
        )));
    }

    let re =
        Regex::new(r#"textLength="(?P<len>\d+(?:\.\d+)?)"[^>]*>&lt;&lt;(?P<ty>[^&]+)&gt;&gt;"#)
            .map_err(|e| XtaskError::VerifyFailed(format!("failed to build regex: {e}")))?;

    let mut map: BTreeMap<String, f64> = BTreeMap::new();
    let mut conflicts: Vec<String> = Vec::new();

    for path in svg_files {
        let svg = read_text(&path)?;
        for cap in re.captures_iter(&svg) {
            let ty = cap.name("ty").map(|m| m.as_str()).unwrap_or("").to_string();
            let len = cap
                .name("len")
                .and_then(|m| m.as_str().parse::<f64>().ok())
                .unwrap_or(0.0);
            if ty.is_empty() || len <= 0.0 {
                continue;
            }
            if let Some(prev) = map.get(&ty) {
                if (*prev - len).abs() > 0.001 {
                    conflicts.push(format!("{ty}: {prev} vs {len}"));
                }
            } else {
                map.insert(ty, len);
            }
        }
    }

    if !conflicts.is_empty() {
        conflicts.sort();
        conflicts.dedup();
        return Err(XtaskError::VerifyFailed(format!(
            "conflicting C4 type textLength values found:\n{}",
            conflicts.join("\n")
        )));
    }

    if map.is_empty() {
        return Err(XtaskError::VerifyFailed(format!(
            "no C4 type textLength values were extracted from {}",
            in_dir.display()
        )));
    }

    let mut out = String::new();
    out.push_str("// This file is @generated by `cargo run -p xtask -- gen-c4-textlength`.\n");
    out.push_str("//\n");
    out.push_str(
        "// Mermaid derives these values via DOM-backed text measurement (`getBBox`) and emits them as the\n",
    );
    out.push_str(
        "// `textLength` attribute for the C4 type line (`<<person>>`, etc). To make DOM parity reproducible\n",
    );
    out.push_str(
        "// in a headless Rust context, we vendor the observed values from the pinned Mermaid CLI baselines.\n\n",
    );
    out.push_str("pub fn c4_type_text_length_px_11_12_2(type_c4_shape: &str) -> Option<f64> {\n");
    out.push_str("    match type_c4_shape {\n");
    for (ty, len) in &map {
        let _ = writeln!(&mut out, r#"        "{}" => Some({}),"#, ty, fmt_f64(*len));
    }
    out.push_str("        _ => None,\n");
    out.push_str("    }\n");
    out.push_str("}\n");

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, out).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}
