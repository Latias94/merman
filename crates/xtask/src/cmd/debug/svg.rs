//! SVG debug utilities.

use crate::XtaskError;
use std::fs;
use std::path::PathBuf;

pub(crate) fn debug_svg_bbox(args: Vec<String>) -> Result<(), XtaskError> {
    let mut svg_path: Option<PathBuf> = None;
    let mut padding: f64 = 8.0;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--svg" => {
                i += 1;
                svg_path = args.get(i).map(PathBuf::from);
            }
            "--padding" => {
                i += 1;
                padding = args
                    .get(i)
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(8.0);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let svg_path = svg_path.ok_or(XtaskError::Usage)?;
    let svg = fs::read_to_string(&svg_path).map_err(|source| XtaskError::ReadFile {
        path: svg_path.display().to_string(),
        source,
    })?;

    let dbg = merman_render::svg::debug_svg_emitted_bounds(&svg).ok_or_else(|| {
        XtaskError::DebugSvgFailed(format!(
            "failed to compute emitted bounds for {}",
            svg_path.display()
        ))
    })?;

    let b = dbg.bounds;
    let vb_min_x = b.min_x - padding;
    let vb_min_y = b.min_y - padding;
    let vb_w = (b.max_x - b.min_x) + 2.0 * padding;
    let vb_h = (b.max_y - b.min_y) + 2.0 * padding;

    println!("svg: {}", svg_path.display());
    println!(
        "bounds: min=({:.6},{:.6}) max=({:.6},{:.6})",
        b.min_x, b.min_y, b.max_x, b.max_y
    );
    println!(
        "viewBox (padding={:.3}): {:.6} {:.6} {:.6} {:.6}",
        padding, vb_min_x, vb_min_y, vb_w, vb_h
    );
    println!("style max-width: {:.6}px", vb_w);

    fn print_contrib(label: &str, c: &Option<merman_render::svg::SvgEmittedBoundsContributor>) {
        let Some(c) = c else {
            println!("{label}: <none>");
            return;
        };
        fn clip_attr(s: &str) -> String {
            const MAX: usize = 140;
            if s.len() <= MAX {
                return s.to_string();
            }
            let mut out = s.chars().take(MAX).collect::<String>();
            out.push('…');
            out
        }

        println!(
            "{label}: <{} id={:?} class={:?}> bbox=({:.6},{:.6})-({:.6},{:.6})",
            c.tag, c.id, c.class, c.bounds.min_x, c.bounds.min_y, c.bounds.max_x, c.bounds.max_y
        );
        if let Some(d) = c.d.as_deref() {
            println!("  d={}", clip_attr(d));
        }
        if let Some(points) = c.points.as_deref() {
            println!("  points={}", clip_attr(points));
        }
        if let Some(tf) = c.transform.as_deref() {
            println!("  transform={}", clip_attr(tf));
        }
    }

    print_contrib("min_x", &dbg.min_x);
    print_contrib("min_y", &dbg.min_y);
    print_contrib("max_x", &dbg.max_x);
    print_contrib("max_y", &dbg.max_y);

    Ok(())
}

pub(crate) fn debug_svg_data_points(args: Vec<String>) -> Result<(), XtaskError> {
    #[derive(Debug, Clone, Copy, serde::Deserialize)]
    struct Point {
        x: f64,
        y: f64,
    }

    use base64::Engine as _;

    fn decode_points(svg: &str, element_id: &str) -> Result<Vec<Point>, XtaskError> {
        let doc = roxmltree::Document::parse(svg)
            .map_err(|e| XtaskError::SvgCompareFailed(format!("failed to parse svg xml: {e}")))?;
        let node = doc
            .descendants()
            .find(|n| n.is_element() && n.attribute("id") == Some(element_id))
            .ok_or_else(|| {
                XtaskError::DebugSvgFailed(format!("missing element with id={element_id:?}"))
            })?;
        let b64 = node.attribute("data-points").ok_or_else(|| {
            XtaskError::DebugSvgFailed(format!(
                "element id={element_id:?} has no `data-points` attribute"
            ))
        })?;

        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64.as_bytes())
            .map_err(|e| XtaskError::DebugSvgFailed(format!("invalid base64 data-points: {e}")))?;
        let pts: Vec<Point> = serde_json::from_slice(&bytes).map_err(|e| {
            XtaskError::DebugSvgFailed(format!("invalid JSON data-points payload: {e}"))
        })?;
        Ok(pts)
    }

    let mut svg_path: Option<PathBuf> = None;
    let mut other_svg_path: Option<PathBuf> = None;
    let mut element_id: Option<String> = None;
    let mut decimals: usize = 3;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--svg" => {
                i += 1;
                svg_path = args.get(i).map(PathBuf::from);
            }
            "--other" => {
                i += 1;
                other_svg_path = args.get(i).map(PathBuf::from);
            }
            "--id" => {
                i += 1;
                element_id = args.get(i).map(|s| s.to_string());
            }
            "--decimals" => {
                i += 1;
                decimals = args
                    .get(i)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(3);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let svg_path = svg_path.ok_or(XtaskError::Usage)?;
    let element_id = element_id.ok_or(XtaskError::Usage)?;

    let svg = fs::read_to_string(&svg_path).map_err(|source| XtaskError::ReadFile {
        path: svg_path.display().to_string(),
        source,
    })?;
    let points = decode_points(&svg, &element_id)?;

    println!("svg: {}", svg_path.display());
    println!("id: {element_id}");
    println!("points: {}", points.len());
    for (idx, p) in points.iter().enumerate() {
        println!(
            "  {idx:>3}: {x:.d$}, {y:.d$}",
            x = p.x,
            y = p.y,
            d = decimals
        );
    }

    let Some(other_svg_path) = other_svg_path else {
        return Ok(());
    };

    let other_svg = fs::read_to_string(&other_svg_path).map_err(|source| XtaskError::ReadFile {
        path: other_svg_path.display().to_string(),
        source,
    })?;
    let other_points = decode_points(&other_svg, &element_id)?;

    println!("\nother: {}", other_svg_path.display());
    println!("points: {}", other_points.len());
    if points.len() != other_points.len() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "point count mismatch: {} vs {}",
            points.len(),
            other_points.len()
        )));
    }

    println!("\ndelta (other - svg):");
    for (idx, (a, b)) in points.iter().zip(other_points.iter()).enumerate() {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        println!(
            "  {idx:>3}: dx={dx:.d$} dy={dy:.d$}",
            dx = dx,
            dy = dy,
            d = decimals
        );
    }

    Ok(())
}
