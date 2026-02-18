//! Mindmap debug utilities.

use crate::XtaskError;
use crate::util::*;
use regex::Regex;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

pub(crate) fn debug_mindmap_svg_positions(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<String> = None;
    let mut upstream: Option<PathBuf> = None;
    let mut local: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(|s| s.to_string());
            }
            "--upstream" => {
                i += 1;
                upstream = args.get(i).map(PathBuf::from);
            }
            "--local" => {
                i += 1;
                local = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    if let Some(f) = fixture.as_deref() {
        let upstream_default = workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join("mindmap")
            .join(format!("{f}.svg"));
        let local_default = workspace_root
            .join("target")
            .join("compare")
            .join("mindmap")
            .join(format!("{f}.svg"));
        upstream = upstream.or(Some(upstream_default));
        local = local.or(Some(local_default));
    }

    let Some(upstream_path) = upstream else {
        return Err(XtaskError::Usage);
    };
    let Some(local_path) = local else {
        return Err(XtaskError::Usage);
    };

    let upstream_svg =
        fs::read_to_string(&upstream_path).map_err(|source| XtaskError::ReadFile {
            path: upstream_path.display().to_string(),
            source,
        })?;
    let local_svg = fs::read_to_string(&local_path).map_err(|source| XtaskError::ReadFile {
        path: local_path.display().to_string(),
        source,
    })?;

    #[derive(Debug, Clone, Copy)]
    struct Translate {
        x: f64,
        y: f64,
    }

    fn parse_translate(transform: &str) -> Option<Translate> {
        let t = transform.trim();
        let t = t.strip_prefix("translate(")?;
        let t = t.strip_suffix(')')?;
        let parts = t
            .split(|ch: char| ch == ',' || ch.is_whitespace())
            .filter(|s| !s.trim().is_empty())
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .collect::<Vec<_>>();
        match parts.as_slice() {
            [x, y] => Some(Translate { x: *x, y: *y }),
            [x] => Some(Translate { x: *x, y: 0.0 }),
            _ => None,
        }
    }

    fn accumulated_translate(node: roxmltree::Node<'_, '_>) -> Translate {
        let mut x = 0.0;
        let mut y = 0.0;
        for n in node.ancestors().filter(|n| n.is_element()).skip(1) {
            if let Some(transform) = n.attribute("transform") {
                if let Some(t) = parse_translate(transform) {
                    x += t.x;
                    y += t.y;
                }
            }
        }
        Translate { x, y }
    }

    #[derive(Debug, Clone)]
    struct RootInfo {
        view_box: Option<String>,
        max_width: Option<String>,
    }

    #[derive(Debug, Clone)]
    struct NodePos {
        id: String,
        class: String,
        x: f64,
        y: f64,
    }

    fn parse_root_info(svg: &str) -> Result<RootInfo, String> {
        let doc = roxmltree::Document::parse(svg).map_err(|e| e.to_string())?;
        let root = doc.root_element();
        let view_box = root.attribute("viewBox").map(|s| s.to_string());
        let max_width = root.attribute("style").and_then(|s| {
            static RE: OnceLock<Regex> = OnceLock::new();
            let re = RE.get_or_init(|| Regex::new(r#"max-width:\s*([0-9.]+)px"#).unwrap());
            re.captures(s)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        });
        Ok(RootInfo {
            view_box,
            max_width,
        })
    }

    fn parse_node_positions(svg: &str) -> Result<Vec<NodePos>, String> {
        let doc = roxmltree::Document::parse(svg).map_err(|e| e.to_string())?;
        let mut out: Vec<NodePos> = Vec::new();

        for n in doc.descendants().filter(|n| n.is_element()) {
            if n.tag_name().name() != "g" {
                continue;
            }
            let Some(id) = n.attribute("id") else {
                continue;
            };
            if !id.starts_with("node_") {
                continue;
            }
            let Some(class) = n.attribute("class") else {
                continue;
            };
            if !class.split_whitespace().any(|t| t == "node") {
                continue;
            }
            let Some(transform) = n.attribute("transform") else {
                continue;
            };
            let Some(local) = parse_translate(transform) else {
                continue;
            };
            let abs = accumulated_translate(n);
            out.push(NodePos {
                id: id.to_string(),
                class: class.to_string(),
                x: local.x + abs.x,
                y: local.y + abs.y,
            });
        }

        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    let up_root = parse_root_info(&upstream_svg).map_err(XtaskError::DebugSvgFailed)?;
    let lo_root = parse_root_info(&local_svg).map_err(XtaskError::DebugSvgFailed)?;
    let up_nodes = parse_node_positions(&upstream_svg).map_err(XtaskError::DebugSvgFailed)?;
    let lo_nodes = parse_node_positions(&local_svg).map_err(XtaskError::DebugSvgFailed)?;

    println!("upstream: {}", upstream_path.display());
    println!("local:    {}", local_path.display());
    println!();

    println!("== Root SVG ==");
    println!(
        "upstream viewBox: {:?}",
        up_root.view_box.as_deref().unwrap_or("<missing>")
    );
    println!(
        "local    viewBox: {:?}",
        lo_root.view_box.as_deref().unwrap_or("<missing>")
    );
    println!(
        "upstream max-width(px): {:?}",
        up_root.max_width.as_deref().unwrap_or("<missing>")
    );
    println!(
        "local    max-width(px): {:?}",
        lo_root.max_width.as_deref().unwrap_or("<missing>")
    );
    println!();

    println!("== Nodes ==");
    println!("upstream nodes: {}", up_nodes.len());
    println!("local nodes:    {}", lo_nodes.len());
    println!();

    let mut up_by_id: std::collections::BTreeMap<&str, &NodePos> =
        std::collections::BTreeMap::new();
    for n in &up_nodes {
        up_by_id.insert(n.id.as_str(), n);
    }
    let mut lo_by_id: std::collections::BTreeMap<&str, &NodePos> =
        std::collections::BTreeMap::new();
    for n in &lo_nodes {
        lo_by_id.insert(n.id.as_str(), n);
    }

    for (id, up) in &up_by_id {
        let lo = lo_by_id.get(id).copied();
        match lo {
            Some(lo) => {
                if up.x != lo.x || up.y != lo.y || up.class != lo.class {
                    println!("id={id}");
                    println!("  upstream: ({:.6}, {:.6}) class={}", up.x, up.y, up.class);
                    println!("  local:    ({:.6}, {:.6}) class={}", lo.x, lo.y, lo.class);
                }
            }
            None => println!("upstream-only: {id}"),
        }
    }
    for id in lo_by_id.keys() {
        if !up_by_id.contains_key(id) {
            println!("local-only: {id}");
        }
    }

    Ok(())
}
