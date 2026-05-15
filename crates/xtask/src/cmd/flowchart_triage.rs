//! Flowchart root-pin triage reports.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::{XtaskError, cmd};
use regex::Regex;

#[derive(Debug, Clone)]
struct RootRow {
    fixture: String,
    upstream_max_width: String,
    local_max_width: String,
    delta: f64,
    upstream_viewbox: String,
    local_viewbox: String,
}

#[derive(Debug, Clone)]
struct LabelRow {
    fixture: String,
    idx: String,
    class_name: String,
    upstream_size: String,
    local_size: String,
    delta_w: f64,
    delta_h: f64,
    text: String,
    markup: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TriageBucket {
    SharedTextCandidate,
    SharedMultilineText,
    LowNoiseText,
    LayoutEdgeOrder,
    LayoutShapeGeometry,
    RootOnlyLayout,
    LayoutTextAccumulation,
    DeferMojibakeFontFallback,
    DeferCourierFont,
    DeferIconFont,
    DeferFontEnv,
}

impl TriageBucket {
    const ALL: [Self; 11] = [
        Self::SharedTextCandidate,
        Self::SharedMultilineText,
        Self::LowNoiseText,
        Self::LayoutEdgeOrder,
        Self::LayoutShapeGeometry,
        Self::RootOnlyLayout,
        Self::LayoutTextAccumulation,
        Self::DeferMojibakeFontFallback,
        Self::DeferCourierFont,
        Self::DeferIconFont,
        Self::DeferFontEnv,
    ];

    fn title(self) -> &'static str {
        match self {
            Self::SharedTextCandidate => "shared-text-candidate",
            Self::SharedMultilineText => "shared-multiline-text",
            Self::LowNoiseText => "low-noise-text",
            Self::LayoutEdgeOrder => "layout-edge-order",
            Self::LayoutShapeGeometry => "layout-shape-geometry",
            Self::RootOnlyLayout => "root-only-layout",
            Self::LayoutTextAccumulation => "layout-text-accumulation",
            Self::DeferMojibakeFontFallback => "defer-mojibake-font-fallback",
            Self::DeferCourierFont => "defer-courier-font",
            Self::DeferIconFont => "defer-icon-font",
            Self::DeferFontEnv => "defer-font-env",
        }
    }
}

pub(crate) fn triage_flowchart_root_pins(args: Vec<String>) -> Result<(), XtaskError> {
    let mut input = cmd::target_root()
        .join("compare")
        .join("flowchart_root_pin_label_audit_current.md");
    let mut output = cmd::target_root()
        .join("compare")
        .join("flowchart_root_pin_triage_current.md");

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--in" => {
                i += 1;
                input = args.get(i).map(PathBuf::from).ok_or(XtaskError::Usage)?;
            }
            "--out" => {
                i += 1;
                output = args.get(i).map(PathBuf::from).ok_or(XtaskError::Usage)?;
            }
            "--help" | "-h" => {
                println!(
                    "usage: xtask triage-flowchart-root-pins [--in <audit.md>] [--out <triage.md>]"
                );
                return Ok(());
            }
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let audit = fs::read_to_string(&input).map_err(|source| XtaskError::ReadFile {
        path: input.display().to_string(),
        source,
    })?;
    let (root_rows, label_rows) = parse_flowchart_audit(&audit);
    let report = render_triage_report(&input, &root_rows, &label_rows);

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&output, report).map_err(|source| XtaskError::WriteFile {
        path: output.display().to_string(),
        source,
    })?;
    println!("wrote report: {}", output.display());
    Ok(())
}

fn parse_flowchart_audit(audit: &str) -> (Vec<RootRow>, Vec<LabelRow>) {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Section {
        Root,
        Label,
    }

    let mut section: Option<Section> = None;
    let mut root_rows = Vec::new();
    let mut label_rows = Vec::new();

    for line in audit.lines() {
        if line.starts_with("## Root Viewport Deltas") {
            section = Some(Section::Root);
            continue;
        }
        if line.starts_with("## Label Metric Deltas") {
            section = Some(Section::Label);
            continue;
        }
        if !line.starts_with("| `") {
            continue;
        }

        let parts = split_markdown_row(line);
        match section {
            Some(Section::Root) if parts.len() >= 6 => {
                if let Ok(delta) = parts[3].parse::<f64>() {
                    root_rows.push(RootRow {
                        fixture: parts[0].trim_matches('`').to_string(),
                        upstream_max_width: parts[1].to_string(),
                        local_max_width: parts[2].to_string(),
                        delta,
                        upstream_viewbox: parts[4].to_string(),
                        local_viewbox: parts[5].to_string(),
                    });
                }
            }
            Some(Section::Label) if parts.len() >= 10 => {
                if let (Ok(delta_w), Ok(delta_h)) =
                    (parts[6].parse::<f64>(), parts[7].parse::<f64>())
                {
                    label_rows.push(LabelRow {
                        fixture: parts[0].trim_matches('`').to_string(),
                        idx: parts[2].to_string(),
                        class_name: parts[3].trim_matches('`').to_string(),
                        upstream_size: parts[4].to_string(),
                        local_size: parts[5].to_string(),
                        delta_w,
                        delta_h,
                        text: parts[8].to_string(),
                        markup: parts[9].to_string(),
                    });
                }
            }
            _ => {}
        }
    }

    (root_rows, label_rows)
}

fn split_markdown_row(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut chars = line.trim().trim_matches('|').chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.peek().copied() {
                Some('|') | Some('\\') => {
                    cell.push(chars.next().expect("peeked escaped markdown char"));
                }
                Some(_) => {
                    cell.push(ch);
                    cell.push(chars.next().expect("peeked markdown char"));
                }
                None => cell.push(ch),
            },
            '|' => {
                cells.push(cell.trim().to_string());
                cell.clear();
            }
            _ => cell.push(ch),
        }
    }

    cells.push(cell.trim().to_string());
    cells
}

fn render_triage_report(input: &Path, root_rows: &[RootRow], label_rows: &[LabelRow]) -> String {
    let mut labels_by_fixture: BTreeMap<&str, Vec<&LabelRow>> = BTreeMap::new();
    for label in label_rows {
        labels_by_fixture
            .entry(label.fixture.as_str())
            .or_default()
            .push(label);
    }

    let mut buckets: BTreeMap<
        TriageBucket,
        Vec<(
            &RootRow,
            String,
            Vec<&LabelRow>,
            Option<RootBoundarySummary>,
        )>,
    > = BTreeMap::new();
    for row in root_rows {
        let labels = labels_by_fixture
            .get(row.fixture.as_str())
            .cloned()
            .unwrap_or_default();
        let fixture_source = read_flowchart_fixture(&row.fixture);
        let boundary = read_flowchart_root_boundary_summary(&row.fixture);
        let (bucket, reason) =
            classify_root_pin(row, &labels, fixture_source.as_deref(), boundary.as_ref());
        let mut top = labels;
        top.sort_by(|a, b| {
            let a_delta = a.delta_w.abs().max(a.delta_h.abs());
            let b_delta = b.delta_w.abs().max(b.delta_h.abs());
            b_delta
                .partial_cmp(&a_delta)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        top.truncate(4);
        buckets
            .entry(bucket)
            .or_default()
            .push((row, reason, top, boundary));
    }

    let mut out = String::new();
    out.push_str("# Flowchart Root Pin Triage\n\n");
    out.push_str(&format!("Source: `{}`\n\n", input.display()));
    out.push_str(
        "Policy: no fixture/glyph lookup tables; prefer shared text/layout rules; defer font environment and icon glyph parity.\n\n",
    );
    out.push_str(&format!("- root pins: {}\n", root_rows.len()));
    out.push_str(&format!("- label delta rows: {}\n\n", label_rows.len()));
    push_removal_candidates_section(&mut out, root_rows);

    for bucket in TriageBucket::ALL {
        let Some(items) = buckets.get_mut(&bucket) else {
            continue;
        };
        items.sort_by(|a, b| {
            b.0.delta
                .abs()
                .partial_cmp(&a.0.delta.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        out.push_str(&format!("## {} ({})\n\n", bucket.title(), items.len()));
        for (row, reason, labels, boundary) in items {
            out.push_str(&format!(
                "- `{}` root Δ {:+.3}px (max-width {} -> {}; viewBox {} -> {})\n",
                row.fixture,
                row.delta,
                row.upstream_max_width,
                row.local_max_width,
                row.upstream_viewbox,
                row.local_viewbox
            ));
            out.push_str(&format!("  - reason: {reason}\n"));
            if let Some(summary) = boundary
                .as_ref()
                .and_then(RootBoundarySummary::dominant_horizontal)
            {
                out.push_str(&format!("  - boundary: {}\n", summary.markdown_summary()));
            }
            if let Some(summary) = boundary
                .as_ref()
                .and_then(RootBoundarySummary::dominant_vertical)
                .filter(|summary| summary.delta.abs() >= 0.25)
            {
                out.push_str(&format!(
                    "  - vertical boundary: {}\n",
                    summary.markdown_summary()
                ));
            }
            if labels.is_empty() {
                out.push_str("  - labels: no paired label delta rows\n");
            } else {
                for label in labels {
                    out.push_str(&format!(
                        "  - label #{} {}: {} -> {}, Δw {:+.3}, Δh {:+.3}, text `{}`, markup `{}`\n",
                        label.idx,
                        label.class_name,
                        label.upstream_size,
                        label.local_size,
                        label.delta_w,
                        label.delta_h,
                        label.text,
                        label.markup
                    ));
                }
            }
        }
        out.push('\n');
    }

    out
}

fn push_removal_candidates_section(out: &mut String, root_rows: &[RootRow]) {
    let mut removable = root_rows
        .iter()
        .filter(|row| root_viewport_matches_upstream(row))
        .collect::<Vec<_>>();
    removable.sort_by(|a, b| a.fixture.cmp(&b.fixture));

    out.push_str("## root-pin-removal-candidates\n\n");
    out.push_str(
        "Candidates require no-overrides root parity: upstream/local max-width and viewBox must both match in the audit report.\n\n",
    );
    if removable.is_empty() {
        out.push_str(
            "- none; keep all current flowchart root pins until shared text/layout fixes remove the remaining root drift.\n\n",
        );
        return;
    }

    for row in removable {
        out.push_str(&format!(
            "- `{}` max-width {}; viewBox {}\n",
            row.fixture, row.upstream_max_width, row.upstream_viewbox
        ));
    }
    out.push('\n');
}

fn root_viewport_matches_upstream(row: &RootRow) -> bool {
    row.upstream_max_width == row.local_max_width && row.upstream_viewbox == row.local_viewbox
}

fn read_flowchart_fixture(fixture: &str) -> Option<String> {
    fs::read_to_string(
        cmd::fixtures_root()
            .join("flowchart")
            .join(format!("{fixture}.mmd")),
    )
    .ok()
}

fn classify_root_pin(
    row: &RootRow,
    labels: &[&LabelRow],
    fixture_source: Option<&str>,
    boundary: Option<&RootBoundarySummary>,
) -> (TriageBucket, String) {
    let source = fixture_source.unwrap_or_default().to_ascii_lowercase();
    let fixture = row.fixture.to_ascii_lowercase();
    let label_text = labels
        .iter()
        .map(|label| format!("{} {}", label.text, label.markup))
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    let max_label_delta = labels
        .iter()
        .map(|label| label.delta_w.abs().max(label.delta_h.abs()))
        .fold(0.0, f64::max);
    let edge_pairing_deltas = labels
        .iter()
        .filter(|label| {
            label.class_name == "edgeLabel"
                && (label.upstream_size.starts_with("0.000")
                    || label.local_size.starts_with("0.000"))
        })
        .count();

    if fixture.contains("fontawesome")
        || fixture.contains("icon")
        || label_text.contains("fa.")
        || source.contains("fa:")
    {
        return (
            TriageBucket::DeferIconFont,
            "contains FontAwesome/icon labels; avoid glyph lookup table".to_string(),
        );
    }
    if source.contains("fontfamily: courier")
        || source.contains("fontfamily:courier")
        || source.contains("courier")
    {
        return (
            TriageBucket::DeferCourierFont,
            "courier metrics/environment; many cases need exact browser/font behavior".to_string(),
        );
    }
    if source.contains("serif")
        || source.contains("arial")
        || source.contains("font-size:22")
        || fixture.contains("text_style_overrides")
    {
        return (
            TriageBucket::DeferFontEnv,
            "custom font/family/size environment; likely baseline font difference".to_string(),
        );
    }
    if fixture_source.is_some_and(contains_c1_control_chars)
        || labels
            .iter()
            .any(|label| contains_c1_control_chars(&label.text))
        || boundary.is_some_and(|summary| summary_contains_c1_control_chars(summary))
    {
        return (
            TriageBucket::DeferMojibakeFontFallback,
            "contains mojibake C1 control bytes; residual default-stack fallback is browser/font-environment dependent, so keep the pin instead of adding glyph lookup data".to_string(),
        );
    }
    if edge_pairing_deltas >= 2 {
        return (
            TriageBucket::LayoutEdgeOrder,
            format!("{edge_pairing_deltas} edgeLabel pair/order deltas; not primarily text width"),
        );
    }
    if fixture.contains("newshapes")
        || fixture.contains("oldshapes")
        || fixture.contains("shape_alias")
        || fixture.contains("shape_mix")
        || fixture.contains("stadium_shape")
        || label_text.contains("trapezoid")
        || label_text.contains("subroutine")
        || label_text.contains("shape test")
    {
        return (
            TriageBucket::LayoutShapeGeometry,
            "shape/cluster geometry or emitted bounds likely dominates".to_string(),
        );
    }
    if let Some(edge) = boundary.and_then(RootBoundarySummary::dominant_horizontal) {
        let root_delta = row.delta.abs().max(0.001);
        let boundary_delta = edge.delta.abs();
        let owner_has_reported_label_delta = boundary_edge_matches_label_delta(edge, labels);
        if !labels.is_empty()
            && boundary_delta >= root_delta * 0.5
            && max_label_delta > root_delta * 4.0
            && !owner_has_reported_label_delta
        {
            if has_mixed_sign_label_width_drift(labels, 1.0) {
                return (
                    TriageBucket::DeferFontEnv,
                    format!(
                        "root {} boundary is `{}` and has no reported label drift, while other default-stack labels have mixed positive/negative width drift; likely accumulated browser/font shaping rather than a clean shared metric rule",
                        edge.side.title(),
                        edge.owner_summary()
                    ),
                );
            }
            return (
                TriageBucket::LayoutTextAccumulation,
                format!(
                    "root {} boundary is `{}` but largest label deltas are elsewhere; likely cumulative Dagre spacing from shared text metrics",
                    edge.side.title(),
                    edge.owner_summary()
                ),
            );
        }
    }
    if labels
        .iter()
        .any(|label| label.text.contains("\\n") || label.markup.contains("br"))
    {
        return (
            TriageBucket::SharedMultilineText,
            "plain multiline/html br text metrics candidate".to_string(),
        );
    }
    if !labels.is_empty() && max_label_delta <= 0.25 {
        return (
            TriageBucket::LowNoiseText,
            "only subpixel plain text deltas; validate before changing metrics".to_string(),
        );
    }
    if !labels.is_empty() {
        return (
            TriageBucket::SharedTextCandidate,
            "plain/default text deltas may be shared metrics".to_string(),
        );
    }
    (
        TriageBucket::RootOnlyLayout,
        "no label deltas in report; layout/emitted bounds candidate".to_string(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoundarySide {
    Left,
    Right,
    Top,
    Bottom,
}

impl BoundarySide {
    fn title(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }
}

#[derive(Debug, Clone)]
struct BoundaryContributor {
    element: String,
    owner: String,
    class_name: String,
    text: String,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl BoundaryContributor {
    fn edge_value(&self, side: BoundarySide) -> f64 {
        match side {
            BoundarySide::Left => self.left,
            BoundarySide::Right => self.right,
            BoundarySide::Top => self.top,
            BoundarySide::Bottom => self.bottom,
        }
    }

    fn short_label(&self) -> String {
        let owner = if self.owner.is_empty() {
            "<anonymous>"
        } else {
            self.owner.as_str()
        };
        let class = if self.class_name.is_empty() {
            String::new()
        } else {
            format!(".{}", self.class_name.replace(' ', "."))
        };
        let text = if self.text.is_empty() {
            String::new()
        } else {
            format!(" `{}`", markdown_cell(&self.text))
        };
        format!("{}{} `{}`{}", self.element, class, owner, text)
    }
}

#[derive(Debug, Clone)]
struct BoundarySideDelta {
    side: BoundarySide,
    upstream: BoundaryContributor,
    local: BoundaryContributor,
    delta: f64,
}

impl BoundarySideDelta {
    fn owner_summary(&self) -> String {
        if !self.local.owner.is_empty() {
            self.local.owner.clone()
        } else {
            self.upstream.owner.clone()
        }
    }

    fn markdown_summary(&self) -> String {
        format!(
            "{} edge {} -> {}, Δ {:+.3}; upstream {}, local {}",
            self.side.title(),
            format_edge(self.upstream.edge_value(self.side)),
            format_edge(self.local.edge_value(self.side)),
            self.delta,
            self.upstream.short_label(),
            self.local.short_label()
        )
    }
}

#[derive(Debug, Clone)]
struct RootBoundarySummary {
    left: Option<BoundarySideDelta>,
    right: Option<BoundarySideDelta>,
    top: Option<BoundarySideDelta>,
    bottom: Option<BoundarySideDelta>,
}

impl RootBoundarySummary {
    fn dominant_horizontal(&self) -> Option<&BoundarySideDelta> {
        match (self.left.as_ref(), self.right.as_ref()) {
            (Some(left), Some(right)) => {
                if left.delta.abs() > right.delta.abs() {
                    Some(left)
                } else {
                    Some(right)
                }
            }
            (Some(left), None) => Some(left),
            (None, Some(right)) => Some(right),
            (None, None) => None,
        }
    }

    fn dominant_vertical(&self) -> Option<&BoundarySideDelta> {
        match (self.top.as_ref(), self.bottom.as_ref()) {
            (Some(top), Some(bottom)) => {
                if top.delta.abs() > bottom.delta.abs() {
                    Some(top)
                } else {
                    Some(bottom)
                }
            }
            (Some(top), None) => Some(top),
            (None, Some(bottom)) => Some(bottom),
            (None, None) => None,
        }
    }
}

#[derive(Debug, Clone)]
struct SvgBoundaryContributors {
    left: BoundaryContributor,
    right: BoundaryContributor,
    top: BoundaryContributor,
    bottom: BoundaryContributor,
}

fn read_flowchart_root_boundary_summary(fixture: &str) -> Option<RootBoundarySummary> {
    let upstream_path = cmd::fixtures_root()
        .join("upstream-svgs")
        .join("flowchart")
        .join(format!("{fixture}.svg"));
    let local_path = cmd::target_root()
        .join("compare")
        .join("flowchart")
        .join(format!("{fixture}.svg"));
    let upstream = fs::read_to_string(upstream_path).ok()?;
    let local = fs::read_to_string(local_path).ok()?;
    collect_root_boundary_summary(&upstream, &local).ok()
}

fn collect_root_boundary_summary(
    upstream_svg: &str,
    local_svg: &str,
) -> Result<RootBoundarySummary, String> {
    let upstream = extract_svg_boundary_contributors(upstream_svg)?;
    let local = extract_svg_boundary_contributors(local_svg)?;

    let side = |side: BoundarySide,
                upstream: BoundaryContributor,
                local: BoundaryContributor|
     -> Option<BoundarySideDelta> {
        let delta = local.edge_value(side) - upstream.edge_value(side);
        Some(BoundarySideDelta {
            side,
            upstream,
            local,
            delta,
        })
    };

    Ok(RootBoundarySummary {
        left: side(BoundarySide::Left, upstream.left, local.left),
        right: side(BoundarySide::Right, upstream.right, local.right),
        top: side(BoundarySide::Top, upstream.top, local.top),
        bottom: side(BoundarySide::Bottom, upstream.bottom, local.bottom),
    })
}

fn extract_svg_boundary_contributors(svg: &str) -> Result<SvgBoundaryContributors, String> {
    let svg = crate::svgdom::normalize_xml_entities(svg);
    let doc = roxmltree::Document::parse(svg.as_ref()).map_err(|e| e.to_string())?;
    let has_root_group = doc.descendants().any(|n| {
        n.is_element()
            && n.has_tag_name("g")
            && n.attribute("class")
                .unwrap_or_default()
                .split_whitespace()
                .any(|t| t == "root")
    });

    let mut contributors: Vec<BoundaryContributor> = Vec::new();
    for n in doc.descendants().filter(|n| n.is_element()) {
        if has_root_group && !is_inside_root_group(n) {
            continue;
        }
        let Some((min_x, min_y, max_x, max_y)) = element_local_bbox(n) else {
            continue;
        };
        if (max_x - min_x).abs() < 1e-9 && (max_y - min_y).abs() < 1e-9 {
            continue;
        }
        let (tx, ty) = accumulated_translate(n);
        let (owner, class_name) = owner_and_class(n);
        contributors.push(BoundaryContributor {
            element: n.tag_name().name().to_string(),
            owner,
            class_name,
            text: owner_label_text(n),
            left: tx + min_x,
            top: ty + min_y,
            right: tx + max_x,
            bottom: ty + max_y,
        });
    }

    let Some(first) = contributors.first().cloned() else {
        return Err("no root boundary contributors found".to_string());
    };
    let mut left = first.clone();
    let mut right = first.clone();
    let mut top = first.clone();
    let mut bottom = first;
    for c in contributors.into_iter().skip(1) {
        if c.left < left.left {
            left = c.clone();
        }
        if c.right > right.right {
            right = c.clone();
        }
        if c.top < top.top {
            top = c.clone();
        }
        if c.bottom > bottom.bottom {
            bottom = c;
        }
    }

    Ok(SvgBoundaryContributors {
        left,
        right,
        top,
        bottom,
    })
}

fn is_inside_root_group(node: roxmltree::Node<'_, '_>) -> bool {
    node.ancestors().filter(|n| n.is_element()).any(|n| {
        n.has_tag_name("g")
            && n.attribute("class")
                .unwrap_or_default()
                .split_whitespace()
                .any(|t| t == "root")
    })
}

fn element_local_bbox(node: roxmltree::Node<'_, '_>) -> Option<(f64, f64, f64, f64)> {
    match node.tag_name().name() {
        "rect" | "foreignObject" | "image" => {
            let x = parse_attr_f64(node, "x").unwrap_or(0.0);
            let y = parse_attr_f64(node, "y").unwrap_or(0.0);
            let w = parse_attr_f64(node, "width").unwrap_or(0.0);
            let h = parse_attr_f64(node, "height").unwrap_or(0.0);
            Some((x, y, x + w, y + h))
        }
        "circle" => {
            let cx = parse_attr_f64(node, "cx").unwrap_or(0.0);
            let cy = parse_attr_f64(node, "cy").unwrap_or(0.0);
            let r = parse_attr_f64(node, "r").unwrap_or(0.0);
            Some((cx - r, cy - r, cx + r, cy + r))
        }
        "ellipse" => {
            let cx = parse_attr_f64(node, "cx").unwrap_or(0.0);
            let cy = parse_attr_f64(node, "cy").unwrap_or(0.0);
            let rx = parse_attr_f64(node, "rx").unwrap_or(0.0);
            let ry = parse_attr_f64(node, "ry").unwrap_or(0.0);
            Some((cx - rx, cy - ry, cx + rx, cy + ry))
        }
        "line" => {
            let x1 = parse_attr_f64(node, "x1").unwrap_or(0.0);
            let y1 = parse_attr_f64(node, "y1").unwrap_or(0.0);
            let x2 = parse_attr_f64(node, "x2").unwrap_or(0.0);
            let y2 = parse_attr_f64(node, "y2").unwrap_or(0.0);
            Some((x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2)))
        }
        "polygon" | "polyline" => {
            let nums = extract_numbers(node.attribute("points").unwrap_or_default());
            bbox_from_number_pairs(&nums)
        }
        "path" => {
            let nums = extract_numbers(node.attribute("d").unwrap_or_default());
            bbox_from_number_pairs(&nums)
        }
        _ => None,
    }
}

fn parse_attr_f64(node: roxmltree::Node<'_, '_>, attr: &str) -> Option<f64> {
    node.attribute(attr)?.parse::<f64>().ok()
}

fn bbox_from_number_pairs(nums: &[f64]) -> Option<(f64, f64, f64, f64)> {
    let mut chunks = nums.chunks_exact(2);
    let first = chunks.next()?;
    let mut min_x = first[0];
    let mut max_x = first[0];
    let mut min_y = first[1];
    let mut max_y = first[1];
    for pair in chunks {
        min_x = min_x.min(pair[0]);
        max_x = max_x.max(pair[0]);
        min_y = min_y.min(pair[1]);
        max_y = max_y.max(pair[1]);
    }
    Some((min_x, min_y, max_x, max_y))
}

fn extract_numbers(s: &str) -> Vec<f64> {
    static NUMBER_RE: OnceLock<Regex> = OnceLock::new();
    let re = NUMBER_RE.get_or_init(|| {
        Regex::new(r#"-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?"#).expect("valid number regex")
    });
    re.find_iter(s)
        .filter_map(|m| m.as_str().parse::<f64>().ok())
        .collect()
}

fn accumulated_translate(node: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let mut x = 0.0;
    let mut y = 0.0;
    for n in node.ancestors().filter(|n| n.is_element()) {
        if let Some((tx, ty)) = parse_translate(n.attribute("transform").unwrap_or_default()) {
            x += tx;
            y += ty;
        }
    }
    (x, y)
}

fn parse_translate(transform: &str) -> Option<(f64, f64)> {
    static TRANSLATE_RE: OnceLock<Regex> = OnceLock::new();
    let re = TRANSLATE_RE
        .get_or_init(|| Regex::new(r#"translate\(([^)]*)\)"#).expect("valid translate regex"));
    let caps = re.captures(transform)?;
    let nums = extract_numbers(caps.get(1)?.as_str());
    match nums.as_slice() {
        [x, y, ..] => Some((*x, *y)),
        [x] => Some((*x, 0.0)),
        _ => None,
    }
}

fn owner_and_class(node: roxmltree::Node<'_, '_>) -> (String, String) {
    for n in node.ancestors().filter(|n| n.is_element()) {
        if let Some(id) = n.attribute("id") {
            return (
                id.to_string(),
                n.attribute("class").unwrap_or_default().to_string(),
            );
        }
    }
    (
        String::new(),
        node.attribute("class").unwrap_or_default().to_string(),
    )
}

fn owner_label_text(node: roxmltree::Node<'_, '_>) -> String {
    if node.has_tag_name("foreignObject") {
        return foreignobject_text(node).replace("\\n", "\n");
    }
    for n in node.ancestors().filter(|n| n.is_element()) {
        if n.attribute("id").is_none() {
            continue;
        }
        if let Some(fo) = n.descendants().find(|d| d.has_tag_name("foreignObject")) {
            return foreignobject_text(fo).replace("\\n", "\n");
        }
        break;
    }
    String::new()
}

fn foreignobject_text(fo: roxmltree::Node<'_, '_>) -> String {
    let mut raw = String::new();
    for n in fo.descendants() {
        if n.is_element() {
            match n.tag_name().name() {
                "br" => raw.push('\n'),
                "p" => {
                    if !raw.is_empty() && !raw.ends_with('\n') {
                        raw.push('\n');
                    }
                }
                _ => {}
            }
        }
        if n.is_text() {
            if let Some(t) = n.text() {
                raw.push_str(t);
            }
        }
    }
    raw.split('\n')
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\\n")
}

fn boundary_edge_matches_label_delta(edge: &BoundarySideDelta, labels: &[&LabelRow]) -> bool {
    let boundary_text = normalize_report_text(if !edge.local.text.is_empty() {
        &edge.local.text
    } else {
        &edge.upstream.text
    });
    if boundary_text.is_empty() {
        return false;
    }
    labels
        .iter()
        .any(|label| normalize_report_text(&label.text) == boundary_text)
}

fn has_mixed_sign_label_width_drift(labels: &[&LabelRow], threshold: f64) -> bool {
    let has_negative = labels.iter().any(|label| label.delta_w <= -threshold);
    let has_positive = labels.iter().any(|label| label.delta_w >= threshold);
    has_negative && has_positive
}

fn normalize_report_text(s: &str) -> String {
    s.replace("\\n", "\n")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_c1_control_chars(s: &str) -> bool {
    s.chars().any(|ch| ('\u{80}'..='\u{9f}').contains(&ch))
}

fn summary_contains_c1_control_chars(summary: &RootBoundarySummary) -> bool {
    [
        summary.left.as_ref(),
        summary.right.as_ref(),
        summary.top.as_ref(),
        summary.bottom.as_ref(),
    ]
    .into_iter()
    .flatten()
    .any(|side| {
        contains_c1_control_chars(&side.upstream.text)
            || contains_c1_control_chars(&side.local.text)
    })
}

fn format_edge(v: f64) -> String {
    format!("{v:.3}")
}

fn markdown_cell(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\r', "")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_row(fixture: &str) -> RootRow {
        RootRow {
            fixture: fixture.to_string(),
            upstream_max_width: "100.000".to_string(),
            local_max_width: "101.000".to_string(),
            delta: 1.0,
            upstream_viewbox: "100.000x40.000".to_string(),
            local_viewbox: "101.000x40.000".to_string(),
        }
    }

    fn exact_root_row(fixture: &str) -> RootRow {
        RootRow {
            fixture: fixture.to_string(),
            upstream_max_width: "100.000".to_string(),
            local_max_width: "100.000".to_string(),
            delta: 0.0,
            upstream_viewbox: "100.000x40.000".to_string(),
            local_viewbox: "100.000x40.000".to_string(),
        }
    }

    fn label_row(
        class_name: &str,
        upstream_size: &str,
        local_size: &str,
        delta_w: f64,
        delta_h: f64,
        text: &str,
        markup: &str,
    ) -> LabelRow {
        LabelRow {
            fixture: "fixture".to_string(),
            idx: "0".to_string(),
            class_name: class_name.to_string(),
            upstream_size: upstream_size.to_string(),
            local_size: local_size.to_string(),
            delta_w,
            delta_h,
            text: text.to_string(),
            markup: markup.to_string(),
        }
    }

    fn boundary_edge(owner: &str, text: &str, delta: f64) -> BoundarySideDelta {
        let upstream = BoundaryContributor {
            element: "rect".to_string(),
            owner: owner.to_string(),
            class_name: "node default".to_string(),
            text: text.to_string(),
            left: 0.0,
            top: 0.0,
            right: 100.0,
            bottom: 24.0,
        };
        let mut local = upstream.clone();
        local.right += delta;
        BoundarySideDelta {
            side: BoundarySide::Right,
            upstream,
            local,
            delta,
        }
    }

    fn right_boundary_summary(edge: BoundarySideDelta) -> RootBoundarySummary {
        RootBoundarySummary {
            left: None,
            right: Some(edge),
            top: None,
            bottom: None,
        }
    }

    #[test]
    fn parse_flowchart_audit_reads_root_and_label_rows() {
        let audit = r#"
# Flowchart SVG Comparison

## Root Viewport Deltas (max-width/viewBox)

| Fixture | upstream max-width(px) | local max-width(px) | Δ | upstream viewBox(w×h) | local viewBox(w×h) |
|---|---:|---:|---:|---:|---:|
| `plain_pipe` | 100.000 | 101.250 | +1.250 | 100.000×50.000 | 101.250×50.000 |

## Label Metric Deltas

| Fixture | root pin | # | class | upstream w×h | local w×h | Δw | Δh | text | markup |
|---|---:|---:|---|---:|---:|---:|---:|---|---|
| `plain_pipe` | yes | 0 | `nodeLabel` | 40.000×24.000 | 41.500×24.000 | +1.500 | +0.000 | left\|right | code\\value |
"#;

        let (roots, labels) = parse_flowchart_audit(audit);

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].fixture, "plain_pipe");
        assert_eq!(roots[0].delta, 1.25);
        assert_eq!(roots[0].upstream_viewbox, "100.000×50.000");
        assert_eq!(roots[0].local_viewbox, "101.250×50.000");

        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].fixture, "plain_pipe");
        assert_eq!(labels[0].class_name, "nodeLabel");
        assert_eq!(labels[0].delta_w, 1.5);
        assert_eq!(labels[0].delta_h, 0.0);
        assert_eq!(labels[0].text, "left|right");
        assert_eq!(labels[0].markup, r"code\value");
    }

    #[test]
    fn classify_root_pin_defers_icon_font_before_other_buckets() {
        let row = root_row("stress_flowchart_icons_basic_051");
        let labels = [label_row(
            "nodeLabel",
            "40.000x24.000",
            "42.000x24.000",
            2.0,
            0.0,
            "for peace",
            "i.fa.fa-twitter",
        )];

        let (bucket, _) =
            classify_root_pin(&row, &[&labels[0]], Some("graph LR\nA[fa:fa-car]"), None);

        assert_eq!(bucket, TriageBucket::DeferIconFont);
    }

    #[test]
    fn classify_root_pin_covers_layout_and_text_buckets() {
        let courier = root_row("plain_courier");
        assert_eq!(
            classify_root_pin(&courier, &[], Some("classDef c fontFamily: Courier"), None).0,
            TriageBucket::DeferCourierFont
        );

        let font_env = root_row("stress_flowchart_text_style_overrides_076");
        assert_eq!(
            classify_root_pin(&font_env, &[], Some("classDef c font-family: serif"), None).0,
            TriageBucket::DeferFontEnv
        );

        let mojibake = [label_row(
            "nodeLabel",
            "78.281x24.000",
            "72.594x24.000",
            -5.688,
            0.0,
            "ç»\u{93}æ\u{9d}\u{9f}",
            "br",
        )];
        assert_eq!(
            classify_root_pin(&root_row("mojibake"), &[&mojibake[0]], None, None).0,
            TriageBucket::DeferMojibakeFontFallback
        );

        let edge_row = root_row("edge_order");
        let edge_labels = [
            label_row(
                "edgeLabel",
                "0.000x0.000",
                "48.000x24.000",
                48.0,
                24.0,
                "a",
                "",
            ),
            label_row(
                "edgeLabel",
                "52.000x24.000",
                "0.000x0.000",
                -52.0,
                -24.0,
                "b",
                "",
            ),
        ];
        assert_eq!(
            classify_root_pin(&edge_row, &[&edge_labels[0], &edge_labels[1]], None, None).0,
            TriageBucket::LayoutEdgeOrder
        );

        let shape_row = root_row("stress_flowchart_shape_mix_009");
        assert_eq!(
            classify_root_pin(&shape_row, &[], None, None).0,
            TriageBucket::LayoutShapeGeometry
        );

        let multiline_row = root_row("multiline");
        let multiline = [label_row(
            "nodeLabel",
            "40.000x24.000",
            "41.000x24.000",
            1.0,
            0.0,
            r"Hello\nWorld",
            "br",
        )];
        assert_eq!(
            classify_root_pin(&multiline_row, &[&multiline[0]], None, None).0,
            TriageBucket::SharedMultilineText
        );

        let low_noise_row = root_row("low_noise");
        let low_noise = [label_row(
            "nodeLabel",
            "40.000x24.000",
            "40.125x24.000",
            0.125,
            0.0,
            "plain",
            "",
        )];
        assert_eq!(
            classify_root_pin(&low_noise_row, &[&low_noise[0]], None, None).0,
            TriageBucket::LowNoiseText
        );

        let shared_row = root_row("shared_text");
        let shared = [label_row(
            "nodeLabel",
            "40.000x24.000",
            "41.000x24.000",
            1.0,
            0.0,
            "plain",
            "",
        )];
        assert_eq!(
            classify_root_pin(&shared_row, &[&shared[0]], None, None).0,
            TriageBucket::SharedTextCandidate
        );

        let root_only = root_row("root_only");
        assert_eq!(
            classify_root_pin(&root_only, &[], None, None).0,
            TriageBucket::RootOnlyLayout
        );
    }

    #[test]
    fn mixed_sign_accumulated_default_text_drift_is_deferred_font_env() {
        let row = root_row("default_font_accumulation");
        let labels = [
            label_row(
                "nodeLabel",
                "210.000x48.000",
                "205.000x48.000",
                -5.0,
                0.0,
                "large negative drift",
                "",
            ),
            label_row(
                "nodeLabel",
                "150.000x48.000",
                "153.000x48.000",
                3.0,
                0.0,
                "large positive drift",
                "br",
            ),
        ];
        let boundary = right_boundary_summary(boundary_edge(
            "flowchart-boundary",
            "boundary label with no reported drift",
            -0.6,
        ));

        let (bucket, reason) =
            classify_root_pin(&row, &[&labels[0], &labels[1]], None, Some(&boundary));

        assert_eq!(bucket, TriageBucket::DeferFontEnv);
        assert!(reason.contains("mixed positive/negative width drift"));
    }

    #[test]
    fn one_direction_accumulated_text_drift_stays_layout_candidate() {
        let row = root_row("shared_text_accumulation");
        let labels = [
            label_row(
                "nodeLabel",
                "210.000x48.000",
                "205.000x48.000",
                -5.0,
                0.0,
                "large negative drift",
                "",
            ),
            label_row(
                "nodeLabel",
                "150.000x48.000",
                "148.000x48.000",
                -2.0,
                0.0,
                "another negative drift",
                "br",
            ),
        ];
        let boundary = right_boundary_summary(boundary_edge(
            "flowchart-boundary",
            "boundary label with no reported drift",
            -0.6,
        ));

        let (bucket, reason) =
            classify_root_pin(&row, &[&labels[0], &labels[1]], None, Some(&boundary));

        assert_eq!(bucket, TriageBucket::LayoutTextAccumulation);
        assert!(reason.contains("largest label deltas are elsewhere"));
    }

    #[test]
    fn triage_report_lists_only_exact_root_viewport_removal_candidates() {
        let exact = exact_root_row("ready_to_delete");
        let width_only = RootRow {
            fixture: "width_only".to_string(),
            upstream_max_width: "100.000".to_string(),
            local_max_width: "100.000".to_string(),
            delta: 0.0,
            upstream_viewbox: "100.000x40.000".to_string(),
            local_viewbox: "100.000x39.000".to_string(),
        };
        let report = render_triage_report(
            Path::new("target/compare/audit.md"),
            &[exact, width_only],
            &[],
        );

        assert!(report.contains("## root-pin-removal-candidates"));
        assert!(report.contains("`ready_to_delete` max-width 100.000; viewBox 100.000x40.000"));
        assert!(!report.contains("`width_only` max-width"));
    }

    #[test]
    fn triage_report_says_none_when_no_root_pin_is_exactly_matched() {
        let report = render_triage_report(
            Path::new("target/compare/audit.md"),
            &[root_row("drift")],
            &[],
        );

        assert!(report.contains("- none; keep all current flowchart root pins"));
    }
}
