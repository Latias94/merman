#[cfg(not(feature = "ratex-math"))]
fn main() {
    eprintln!("enable the `ratex-math` feature to run this audit helper");
    std::process::exit(2);
}

#[cfg(feature = "ratex-math")]
fn main() {
    use merman_core::MermaidConfig;
    use merman_render::math::{MathRenderer, NodeKatexMathRenderer, RatexMathRenderer};
    use merman_render::text::{TextStyle, WrapMode};
    use std::path::Path;

    let node_cwd = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("mermaid-cli");
    let node = NodeKatexMathRenderer::new(node_cwd);
    let ratex = RatexMathRenderer;
    let config = MermaidConfig::default();
    let style = TextStyle::default();
    let formulas = [
        r"x^2",
        r"\frac{1}{2}",
        r"\sqrt{x+3}",
        r"\pi r^2",
        r"\alpha",
        r"\sqrt{2+2}=\sqrt{4}=2",
    ];

    println!("# RaTeX Math Audit\n");
    println!("Flowchart samples use Mermaid's flowchart HTML-label shell at 16px:\n");
    println!("| Formula | KaTeX probe width | KaTeX probe height | RaTeX width | RaTeX height |");
    println!("| --- | ---: | ---: | ---: | ---: |");
    for formula in formulas {
        let label = format!("$${formula}$$");
        let katex =
            node.measure_html_label(&label, &config, &style, Some(200.0), WrapMode::HtmlLike);
        let ratex_metrics =
            ratex.measure_html_label(&label, &config, &style, Some(200.0), WrapMode::HtmlLike);
        print_row(formula, katex, ratex_metrics);
    }

    println!();
    println!("Sequence samples use Mermaid's `width: fit-content` math shell:\n");
    println!(
        "| Formula | KaTeX probe width | KaTeX probe height | RaTeX layout label width | RaTeX layout label height |"
    );
    println!("| --- | ---: | ---: | ---: | ---: |");
    for formula in formulas {
        let label = format!("$${formula}$$");
        let katex = node.measure_sequence_html_label(&label, &config);
        let ratex_metrics =
            sequence_layout_metrics(ratex.measure_sequence_html_label(&label, &config));
        print_row(formula, katex, ratex_metrics);
    }
}

#[cfg(feature = "ratex-math")]
fn print_row(
    formula: &str,
    katex: Option<merman_render::text::TextMetrics>,
    ratex: Option<merman_render::text::TextMetrics>,
) {
    let (katex_w, katex_h) = format_metrics(katex);
    let (ratex_w, ratex_h) = format_metrics(ratex);
    println!("| `{formula}` | {katex_w} | {katex_h} | {ratex_w} | {ratex_h} |");
}

#[cfg(feature = "ratex-math")]
fn format_metrics(metrics: Option<merman_render::text::TextMetrics>) -> (String, String) {
    metrics
        .map(|m| (format_num(m.width), format_num(m.height)))
        .unwrap_or_else(|| ("n/a".to_string(), "n/a".to_string()))
}

#[cfg(feature = "ratex-math")]
fn sequence_layout_metrics(
    metrics: Option<merman_render::text::TextMetrics>,
) -> Option<merman_render::text::TextMetrics> {
    metrics.map(|m| merman_render::text::TextMetrics {
        width: m.width.round().max(1.0),
        height: 19.0_f64.max(m.height.round() + 2.0),
        line_count: m.line_count,
    })
}

#[cfg(feature = "ratex-math")]
fn format_num(value: f64) -> String {
    let s = format!("{value:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}
