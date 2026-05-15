//! Flowchart root-pin triage reports.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use crate::{XtaskError, cmd};

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
    DeferCourierFont,
    DeferIconFont,
    DeferFontEnv,
}

impl TriageBucket {
    const ALL: [Self; 9] = [
        Self::SharedTextCandidate,
        Self::SharedMultilineText,
        Self::LowNoiseText,
        Self::LayoutEdgeOrder,
        Self::LayoutShapeGeometry,
        Self::RootOnlyLayout,
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

    let mut buckets: BTreeMap<TriageBucket, Vec<(&RootRow, String, Vec<&LabelRow>)>> =
        BTreeMap::new();
    for row in root_rows {
        let labels = labels_by_fixture
            .get(row.fixture.as_str())
            .cloned()
            .unwrap_or_default();
        let fixture_source = read_flowchart_fixture(&row.fixture);
        let (bucket, reason) = classify_root_pin(row, &labels, fixture_source.as_deref());
        let mut top = labels;
        top.sort_by(|a, b| {
            let a_delta = a.delta_w.abs().max(a.delta_h.abs());
            let b_delta = b.delta_w.abs().max(b.delta_h.abs());
            b_delta
                .partial_cmp(&a_delta)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        top.truncate(4);
        buckets.entry(bucket).or_default().push((row, reason, top));
    }

    let mut out = String::new();
    out.push_str("# Flowchart Root Pin Triage\n\n");
    out.push_str(&format!("Source: `{}`\n\n", input.display()));
    out.push_str(
        "Policy: no fixture/glyph lookup tables; prefer shared text/layout rules; defer font environment and icon glyph parity.\n\n",
    );
    out.push_str(&format!("- root pins: {}\n", root_rows.len()));
    out.push_str(&format!("- label delta rows: {}\n\n", label_rows.len()));

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
        for (row, reason, labels) in items {
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

        let (bucket, _) = classify_root_pin(&row, &[&labels[0]], Some("graph LR\nA[fa:fa-car]"));

        assert_eq!(bucket, TriageBucket::DeferIconFont);
    }

    #[test]
    fn classify_root_pin_covers_layout_and_text_buckets() {
        let courier = root_row("plain_courier");
        assert_eq!(
            classify_root_pin(&courier, &[], Some("classDef c fontFamily: Courier")).0,
            TriageBucket::DeferCourierFont
        );

        let font_env = root_row("stress_flowchart_text_style_overrides_076");
        assert_eq!(
            classify_root_pin(&font_env, &[], Some("classDef c font-family: serif")).0,
            TriageBucket::DeferFontEnv
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
            classify_root_pin(&edge_row, &[&edge_labels[0], &edge_labels[1]], None).0,
            TriageBucket::LayoutEdgeOrder
        );

        let shape_row = root_row("stress_flowchart_shape_mix_009");
        assert_eq!(
            classify_root_pin(&shape_row, &[], None).0,
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
            classify_root_pin(&multiline_row, &[&multiline[0]], None).0,
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
            classify_root_pin(&low_noise_row, &[&low_noise[0]], None).0,
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
            classify_root_pin(&shared_row, &[&shared[0]], None).0,
            TriageBucket::SharedTextCandidate
        );

        let root_only = root_row("root_only");
        assert_eq!(
            classify_root_pin(&root_only, &[], None).0,
            TriageBucket::RootOnlyLayout
        );
    }
}
