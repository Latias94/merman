use crate::XtaskError;
use regex::Regex;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    #[allow(dead_code)]
    fn center(self) -> Point {
        Point {
            x: self.x + self.w / 2.0,
            y: self.y + self.h / 2.0,
        }
    }
}

#[derive(Debug, Clone)]
struct ClusterInfo {
    outer: Rect,
    label_translate: Option<Point>,
}

#[derive(Debug, Clone)]
struct NodeInfo {
    translate: Point,
    rect: Option<Rect>,
}

#[derive(Debug, Clone)]
struct RootScope {
    #[allow(dead_code)]
    index: usize,
    translate: Point,
    is_nested: bool,
    nested_root_id: Option<String>,
    clusters: BTreeMap<String, ClusterInfo>,
    nodes: BTreeMap<String, NodeInfo>,
}

fn parse_f64(s: &str) -> Option<f64> {
    let s = s.trim().trim_end_matches("px").trim();
    s.parse::<f64>().ok()
}

fn parse_translate_attr(transform: &str) -> Option<Point> {
    let t = transform.trim();
    let start = t.find("translate(")? + "translate(".len();
    let rest = &t[start..];
    let end = rest.find(')')?;
    let inner = &rest[..end];
    let inner = inner.replace(',', " ");
    let mut parts = inner.split_whitespace();
    let x = parse_f64(parts.next()?)?;
    let y = parts.next().and_then(parse_f64).unwrap_or(0.0);
    Some(Point { x, y })
}

fn class_has_token(class: Option<&str>, token: &str) -> bool {
    class
        .unwrap_or_default()
        .split_whitespace()
        .any(|t| t == token)
}

fn parse_svg_scopes(svg: &str) -> Result<Vec<RootScope>, XtaskError> {
    let doc = roxmltree::Document::parse(svg)
        .map_err(|e| XtaskError::SvgCompareFailed(format!("failed to parse svg xml: {e}")))?;

    let mut scopes: Vec<RootScope> = Vec::new();
    for (idx, root) in doc
        .descendants()
        .filter(|n| {
            n.is_element()
                && n.tag_name().name() == "g"
                && class_has_token(n.attribute("class"), "root")
        })
        .enumerate()
    {
        let translate = root
            .attribute("transform")
            .and_then(parse_translate_attr)
            .unwrap_or(Point { x: 0.0, y: 0.0 });
        let is_nested = root.attribute("transform").is_some();

        let mut clusters: BTreeMap<String, ClusterInfo> = BTreeMap::new();
        let mut nodes: BTreeMap<String, NodeInfo> = BTreeMap::new();

        // Clusters: `g.statediagram-cluster` contains an inner `rect.outer` and an optional
        // `g.cluster-label` translate.
        for g in root.descendants().filter(|n| {
            n.is_element()
                && n.tag_name().name() == "g"
                && n.attribute("id").is_some()
                && class_has_token(n.attribute("class"), "statediagram-cluster")
        }) {
            let Some(id) = g.attribute("id") else {
                continue;
            };

            let outer = g
                .descendants()
                .find(|n| {
                    n.is_element()
                        && n.tag_name().name() == "rect"
                        && class_has_token(n.attribute("class"), "outer")
                })
                .and_then(|r| {
                    Some(Rect {
                        x: parse_f64(r.attribute("x")?)?,
                        y: parse_f64(r.attribute("y")?)?,
                        w: parse_f64(r.attribute("width")?)?,
                        h: parse_f64(r.attribute("height")?)?,
                    })
                });

            let Some(outer) = outer else { continue };

            let label_translate = g
                .descendants()
                .find(|n| {
                    n.is_element()
                        && n.tag_name().name() == "g"
                        && class_has_token(n.attribute("class"), "cluster-label")
                })
                .and_then(|n| n.attribute("transform"))
                .and_then(parse_translate_attr);

            clusters.insert(
                id.to_string(),
                ClusterInfo {
                    outer,
                    label_translate,
                },
            );
        }

        // Nodes: `g.node` with `id` and `transform="translate(...)"`. We also record the first
        // direct/descendant rect (common for most state nodes) to estimate the node bbox.
        for g in root.descendants().filter(|n| {
            n.is_element()
                && n.tag_name().name() == "g"
                && n.attribute("id").is_some()
                && class_has_token(n.attribute("class"), "node")
                && n.attribute("transform").is_some()
        }) {
            let Some(id) = g.attribute("id") else {
                continue;
            };
            let Some(tr) = g.attribute("transform").and_then(parse_translate_attr) else {
                continue;
            };
            let rect = g
                .descendants()
                .find(|n| n.is_element() && n.tag_name().name() == "rect")
                .and_then(|r| {
                    Some(Rect {
                        x: parse_f64(r.attribute("x")?)?,
                        y: parse_f64(r.attribute("y")?)?,
                        w: parse_f64(r.attribute("width")?)?,
                        h: parse_f64(r.attribute("height")?)?,
                    })
                });
            nodes.insert(
                id.to_string(),
                NodeInfo {
                    translate: tr,
                    rect,
                },
            );
        }

        let nested_root_id = if is_nested {
            clusters.keys().next().cloned()
        } else {
            None
        };

        scopes.push(RootScope {
            index: idx,
            translate,
            is_nested,
            nested_root_id,
            clusters,
            nodes,
        });
    }

    Ok(scopes)
}

fn parse_svg_root_viewport(svg: &str) -> (Option<f64>, Option<String>) {
    static RE: OnceLock<Regex> = OnceLock::new();

    let re = RE.get_or_init(|| Regex::new(r#"max-width:\s*([0-9.]+)px"#).unwrap());
    let max_w = re
        .captures(svg)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<f64>().ok());

    let viewbox = Regex::new(r#"viewBox="([^"]+)""#)
        .ok()
        .and_then(|re| re.captures(svg))
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));

    (max_w, viewbox)
}

fn find_scope_by_nested_root_id<'a>(scopes: &'a [RootScope], id: &str) -> Option<&'a RootScope> {
    scopes
        .iter()
        .find(|s| s.is_nested && s.nested_root_id.as_deref() == Some(id))
}

pub(crate) fn analyze_state_fixture(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<String> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut root_id: Option<String> = None;
    let mut decimals: u32 = 3;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(|s| s.to_string());
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--root" => {
                i += 1;
                root_id = args.get(i).map(|s| s.to_string());
            }
            "--decimals" => {
                i += 1;
                decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let fixture = fixture
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or(XtaskError::Usage)?;

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("state");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("state");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("analyze")
            .join("state")
            .join(format!("{fixture}.md"))
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("svgs");

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let upstream_path = upstream_dir.join(format!("{fixture}.svg"));
    let upstream_svg =
        fs::read_to_string(&upstream_path).map_err(|source| XtaskError::ReadFile {
            path: upstream_path.display().to_string(),
            source,
        })?;

    let mmd_path = fixtures_dir.join(format!("{fixture}.mmd"));
    let text = fs::read_to_string(&mmd_path).map_err(|source| XtaskError::ReadFile {
        path: mmd_path.display().to_string(),
        source,
    })?;

    let engine = merman::Engine::new();
    let parsed =
        futures::executor::block_on(engine.parse_diagram(&text, merman::ParseOptions::default()))
            .map_err(|e| {
                XtaskError::SvgCompareFailed(format!(
                    "parse failed for {}: {e}",
                    mmd_path.display()
                ))
            })?
            .ok_or_else(|| {
                XtaskError::SvgCompareFailed(format!(
                    "no diagram detected in {}",
                    mmd_path.display()
                ))
            })?;

    let layout_opts = merman_render::LayoutOptions {
        text_measurer: std::sync::Arc::new(
            merman_render::text::VendoredFontMetricsTextMeasurer::default(),
        ),
        ..Default::default()
    };
    let layouted = merman_render::layout_parsed(&parsed, &layout_opts).map_err(|e| {
        XtaskError::SvgCompareFailed(format!("layout failed for {}: {e}", mmd_path.display()))
    })?;

    let merman_render::model::LayoutDiagram::StateDiagramV2(layout) = &layouted.layout else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "unexpected layout type for {}: {}",
            mmd_path.display(),
            layouted.meta.diagram_type
        )));
    };

    let svg_opts = merman_render::svg::SvgRenderOptions {
        diagram_id: Some(fixture.to_string()),
        ..Default::default()
    };
    let local_svg = merman_render::svg::render_state_diagram_v2_svg(
        layout,
        &layouted.semantic,
        &layouted.meta.effective_config,
        layouted.meta.title.as_deref(),
        layout_opts.text_measurer.as_ref(),
        &svg_opts,
    )
    .map_err(|e| {
        XtaskError::SvgCompareFailed(format!("render failed for {}: {e}", mmd_path.display()))
    })?;

    let local_svg_path = out_svg_dir.join(format!("{fixture}.local.svg"));
    let upstream_svg_path = out_svg_dir.join(format!("{fixture}.upstream.svg"));
    let _ = fs::write(&local_svg_path, &local_svg);
    let _ = fs::write(&upstream_svg_path, &upstream_svg);

    let (up_max_w, up_viewbox) = parse_svg_root_viewport(&upstream_svg);
    let (lo_max_w, lo_viewbox) = parse_svg_root_viewport(&local_svg);

    let upstream_scopes = parse_svg_scopes(&upstream_svg)?;
    let local_scopes = parse_svg_scopes(&local_svg)?;

    let root_id = root_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            // For state diagrams, the most problematic parity-root cases are typically nested.
            // Default to the first nested scope id when present.
            upstream_scopes
                .iter()
                .filter_map(|s| s.nested_root_id.clone())
                .next()
        });

    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# State Fixture Analysis\n\n- Fixture: `{}`\n- Upstream SVG: `{}`\n- Local SVG: `{}`\n- Decimals: `{}`\n",
        fixture,
        upstream_path.display(),
        local_svg_path.display(),
        decimals
    );

    let fmt = |v: f64| format!("{:.*}", decimals as usize, v);

    let _ = writeln!(&mut report, "## Root Viewport\n");
    let _ = writeln!(
        &mut report,
        "- Upstream: max-width(px)={:?}, viewBox={:?}",
        up_max_w.map(&fmt),
        up_viewbox.as_deref()
    );
    let _ = writeln!(
        &mut report,
        "- Local: max-width(px)={:?}, viewBox={:?}\n",
        lo_max_w.map(&fmt),
        lo_viewbox.as_deref()
    );

    let _ = writeln!(&mut report, "## Root Scopes\n");
    let _ = writeln!(
        &mut report,
        "| scope | nested | nestedRootId | translate(x,y) upstream | translate(x,y) local |\n|---:|:---:|---|---:|---:|"
    );

    let mut all_scope_ids: BTreeMap<String, ()> = BTreeMap::new();
    for s in &upstream_scopes {
        if let Some(id) = &s.nested_root_id {
            all_scope_ids.insert(id.clone(), ());
        }
    }
    for s in &local_scopes {
        if let Some(id) = &s.nested_root_id {
            all_scope_ids.insert(id.clone(), ());
        }
    }

    // Top-level scope row.
    let up0 = upstream_scopes
        .first()
        .map(|s| s.translate)
        .unwrap_or(Point { x: 0.0, y: 0.0 });
    let lo0 = local_scopes
        .first()
        .map(|s| s.translate)
        .unwrap_or(Point { x: 0.0, y: 0.0 });
    let _ = writeln!(
        &mut report,
        "| 0 | no | (root) | ({},{}) | ({},{}) |",
        fmt(up0.x),
        fmt(up0.y),
        fmt(lo0.x),
        fmt(lo0.y)
    );
    let mut scope_no = 1usize;
    for (id, _) in all_scope_ids {
        let up = find_scope_by_nested_root_id(&upstream_scopes, &id).map(|s| s.translate);
        let lo = find_scope_by_nested_root_id(&local_scopes, &id).map(|s| s.translate);
        let up = up.unwrap_or(Point { x: 0.0, y: 0.0 });
        let lo = lo.unwrap_or(Point { x: 0.0, y: 0.0 });
        let _ = writeln!(
            &mut report,
            "| {} | yes | `{}` | ({},{}) | ({},{}) |",
            scope_no,
            id,
            fmt(up.x),
            fmt(up.y),
            fmt(lo.x),
            fmt(lo.y)
        );
        scope_no += 1;
    }
    let _ = writeln!(&mut report);

    {
        let (scope_label, up_scope, lo_scope) = if let Some(root_id) = root_id.as_deref() {
            (
                format!("Nested Scope: `{root_id}`"),
                find_scope_by_nested_root_id(&upstream_scopes, root_id),
                find_scope_by_nested_root_id(&local_scopes, root_id),
            )
        } else {
            (
                "Root Scope: (root)".to_string(),
                upstream_scopes.first(),
                local_scopes.first(),
            )
        };

        let _ = writeln!(&mut report, "## {scope_label}\n");

        if let (Some(up), Some(lo)) = (up_scope, lo_scope) {
            let _ = writeln!(
                &mut report,
                "### Root Translate\n\n- Upstream: ({},{})\n- Local: ({},{})\n",
                fmt(up.translate.x),
                fmt(up.translate.y),
                fmt(lo.translate.x),
                fmt(lo.translate.y)
            );

            // Cluster summary.
            let _ = writeln!(&mut report, "### Clusters (outer rect)\n");
            let _ = writeln!(
                &mut report,
                "| clusterId | upstream x,y,w,h | local x,y,w,h | upstream label(x,y) | local label(x,y) | Δw | Δh |\n|---|---:|---:|---:|---:|---:|---:|"
            );

            let mut cluster_ids: BTreeMap<String, ()> = BTreeMap::new();
            for k in up.clusters.keys() {
                cluster_ids.insert(k.clone(), ());
            }
            for k in lo.clusters.keys() {
                cluster_ids.insert(k.clone(), ());
            }
            for (cid, _) in cluster_ids {
                let u = up.clusters.get(&cid).map(|c| c.outer);
                let l = lo.clusters.get(&cid).map(|c| c.outer);
                let ul = up
                    .clusters
                    .get(&cid)
                    .and_then(|c| c.label_translate)
                    .unwrap_or(Point { x: 0.0, y: 0.0 });
                let ll = lo
                    .clusters
                    .get(&cid)
                    .and_then(|c| c.label_translate)
                    .unwrap_or(Point { x: 0.0, y: 0.0 });
                if let (Some(u), Some(l)) = (u, l) {
                    let _ = writeln!(
                        &mut report,
                        "| `{}` | ({},{},{},{}) | ({},{},{},{}) | ({},{}) | ({},{}) | {} | {} |",
                        cid,
                        fmt(u.x),
                        fmt(u.y),
                        fmt(u.w),
                        fmt(u.h),
                        fmt(l.x),
                        fmt(l.y),
                        fmt(l.w),
                        fmt(l.h),
                        fmt(ul.x),
                        fmt(ul.y),
                        fmt(ll.x),
                        fmt(ll.y),
                        fmt(l.w - u.w),
                        fmt(l.h - u.h)
                    );
                } else {
                    let _ = writeln!(
                        &mut report,
                        "| `{}` | {:?} | {:?} | ({},{}) | ({},{}) |  |  |",
                        cid,
                        u,
                        l,
                        fmt(ul.x),
                        fmt(ul.y),
                        fmt(ll.x),
                        fmt(ll.y),
                    );
                }
            }
            let _ = writeln!(&mut report);

            // Node translate deltas.
            let _ = writeln!(&mut report, "### Nodes (translate)\n");

            let mut deltas: Vec<(f64, String, Point, Point)> = Vec::new();
            for (id, u) in &up.nodes {
                if let Some(l) = lo.nodes.get(id) {
                    let dx = l.translate.x - u.translate.x;
                    let dy = l.translate.y - u.translate.y;
                    let score = dx.abs().max(dy.abs());
                    deltas.push((score, id.clone(), u.translate, l.translate));
                }
            }
            deltas.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

            let _ = writeln!(
                &mut report,
                "| nodeId | upstream (x,y) | local (x,y) | upstream rect(w,h) | local rect(w,h) | Δx | Δy | Δw | Δh |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|"
            );

            for (score, id, u, l) in deltas.into_iter().take(60) {
                let _ = score;
                let ur = up.nodes.get(&id).and_then(|n| n.rect).unwrap_or(Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                });
                let lr = lo.nodes.get(&id).and_then(|n| n.rect).unwrap_or(Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                });
                let _ = writeln!(
                    &mut report,
                    "| `{}` | ({},{}) | ({},{}) | ({},{}) | ({},{}) | {} | {} | {} | {} |",
                    id,
                    fmt(u.x),
                    fmt(u.y),
                    fmt(l.x),
                    fmt(l.y),
                    fmt(ur.w),
                    fmt(ur.h),
                    fmt(lr.w),
                    fmt(lr.h),
                    fmt(l.x - u.x),
                    fmt(l.y - u.y),
                    fmt(lr.w - ur.w),
                    fmt(lr.h - ur.h)
                );
            }
            let _ = writeln!(&mut report);
        } else {
            let _ = writeln!(
                &mut report,
                "- Missing scope: upstream={} local={}\n",
                up_scope.is_some(),
                lo_scope.is_some()
            );
        }
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}
