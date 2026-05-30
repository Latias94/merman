#[cfg(feature = "ratex-math")]
use merman_render::math::MathRenderer;
#[cfg(feature = "ratex-math")]
use merman_render::text::{
    TextMeasurer, TextMetrics, TextStyle, WrapMode, round_to_1_64_px, split_html_br_lines,
};

#[cfg(not(feature = "ratex-math"))]
fn main() {
    eprintln!("enable the `ratex-math` feature to run this audit helper");
    std::process::exit(2);
}

#[cfg(feature = "ratex-math")]
fn main() {
    use merman_core::MermaidConfig;
    use merman_render::math::{NodeKatexMathRenderer, RatexMathRenderer};
    use merman_render::text::VendoredFontMetricsTextMeasurer;
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
        r"\overbrace{a+b+c}^{\text{note}}",
        r"x(t)=c_1\begin{bmatrix}-\cos{t}+\sin{t}\\ 2\cos{t} \end{bmatrix}e^{2t}",
    ];
    let mixed_labels = [
        r"Solve $$x^2$$",
        r"Use $$\sqrt{x+3}$$ now",
        r"Matrix $$x(t)=c_1\begin{bmatrix}-\cos{t}+\sin{t}\\ 2\cos{t} \end{bmatrix}e^{2t}$$ state",
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

    let text_measurer = VendoredFontMetricsTextMeasurer::default();
    println!();
    println!(
        "Flowchart mixed prose/math samples compose text fragments with measured math fragments:\n"
    );
    println!(
        "| Label | KaTeX probe width | KaTeX probe height | RaTeX composed width | RaTeX composed height |"
    );
    println!("| --- | ---: | ---: | ---: | ---: |");
    for label in mixed_labels {
        let katex =
            node.measure_html_label(label, &config, &style, Some(200.0), WrapMode::HtmlLike);
        let ratex_metrics =
            flowchart_composed_ratex_metrics(&text_measurer, &ratex, label, &config, &style);
        print_row(label, katex, ratex_metrics);
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
fn flowchart_composed_ratex_metrics(
    measurer: &dyn TextMeasurer,
    renderer: &dyn MathRenderer,
    label: &str,
    config: &merman_core::MermaidConfig,
    style: &TextStyle,
) -> Option<TextMetrics> {
    renderer.render_html_label(label, config)?;

    let mut saw_math = false;
    let mut width = 0.0_f64;
    let mut height = 0.0_f64;
    let mut line_count = 0usize;
    for line in split_html_br_lines(label) {
        line_count += 1;
        let (line_width, line_height) = if line.contains("$$") {
            saw_math = true;
            measure_flowchart_mixed_math_line(measurer, renderer, line, config, style)?
        } else {
            let metrics = measurer.measure_wrapped(line, style, Some(200.0), WrapMode::HtmlLike);
            (metrics.width.max(0.0), metrics.height.max(0.0))
        };
        width = width.max(line_width);
        height += line_height;
    }

    saw_math.then_some(TextMetrics {
        width: round_to_1_64_px(width),
        height: round_to_1_64_px(height.max(1.0)),
        line_count: line_count.max(1),
    })
}

#[cfg(feature = "ratex-math")]
fn measure_flowchart_mixed_math_line(
    measurer: &dyn TextMeasurer,
    renderer: &dyn MathRenderer,
    line: &str,
    config: &merman_core::MermaidConfig,
    style: &TextStyle,
) -> Option<(f64, f64)> {
    let start = line.find("$$")?;
    let content_start = start + 2;
    let end_start = line[content_start..].rfind("$$")? + content_start;
    if end_start < content_start {
        return None;
    }
    let formula = &line[content_start..end_start];
    if formula.contains("$$") {
        return None;
    }

    let mut width = 0.0_f64;
    let mut height = 0.0_f64;
    for text in [&line[..start], &line[end_start + 2..]] {
        if text.is_empty() {
            continue;
        }
        let metrics = measurer.measure_wrapped(text, style, None, WrapMode::HtmlLike);
        width += metrics.width.max(0.0);
        height = height.max(metrics.height.max(0.0));
    }

    let chunk = &line[start..end_start + 2];
    let math_metrics =
        renderer.measure_html_label(chunk, config, style, Some(10_000.0), WrapMode::HtmlLike)?;
    width += math_metrics.width.max(0.0);
    height = height.max(math_metrics.height.max(0.0));

    Some((width, height.max(1.0)))
}

#[cfg(feature = "ratex-math")]
fn print_row(
    label: &str,
    katex: Option<merman_render::text::TextMetrics>,
    ratex: Option<merman_render::text::TextMetrics>,
) {
    let (katex_w, katex_h) = format_metrics(katex);
    let (ratex_w, ratex_h) = format_metrics(ratex);
    println!("| `{label}` | {katex_w} | {katex_h} | {ratex_w} | {ratex_h} |");
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
