//! Flowchart style compilation helpers.

use super::*;

#[allow(dead_code)]
pub(in crate::svg::parity) fn flowchart_inline_style_for_classes(
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
) -> String {
    let mut out = String::new();
    for c in classes {
        let Some(decls) = class_defs.get(c) else {
            continue;
        };
        for d in decls {
            let Some((k, v)) = parse_style_decl(d) else {
                continue;
            };
            let _ = write!(&mut out, "{k}:{v} !important;");
        }
    }
    out.trim_end_matches(';').to_string()
}

#[derive(Debug, Clone)]
pub(in crate::svg::parity) struct FlowchartCompiledStyles {
    pub(super) node_style: String,
    pub(super) label_style: String,
    pub(super) label_color: Option<String>,
    pub(super) label_font_family: Option<String>,
    pub(super) label_font_size: Option<String>,
    pub(super) label_font_weight: Option<String>,
    pub(super) label_opacity: Option<String>,
    pub(super) fill: Option<String>,
    pub(super) stroke: Option<String>,
    pub(super) stroke_width: Option<String>,
    pub(super) stroke_dasharray: Option<String>,
}

pub(in crate::svg::parity) fn flowchart_compile_styles(
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    inline_styles_a: &[String],
    inline_styles_b: &[String],
) -> FlowchartCompiledStyles {
    // Ported from Mermaid `handDrawnShapeStyles.compileStyles()` / `styles2String()`:
    // - preserve insertion order of the first occurrence of a key
    // - later occurrences override values, without changing order
    #[derive(Default)]
    struct OrderedMap<'a> {
        order: Vec<(&'a str, &'a str)>,
        idx: FxHashMap<&'a str, usize>,
    }
    impl<'a> OrderedMap<'a> {
        fn set(&mut self, k: &'a str, v: &'a str) {
            if let Some(&i) = self.idx.get(k) {
                self.order[i].1 = v;
                return;
            }
            self.idx.insert(k, self.order.len());
            self.order.push((k, v));
        }
    }

    let mut m: OrderedMap<'_> = OrderedMap::default();

    for c in classes {
        let Some(decls) = class_defs.get(c) else {
            continue;
        };
        for d in decls {
            let Some((k, v)) = parse_style_decl(d) else {
                continue;
            };
            m.set(k, v);
        }
    }

    for d in inline_styles_a.iter().chain(inline_styles_b.iter()) {
        let Some((k, v)) = parse_style_decl(d) else {
            continue;
        };
        m.set(k, v);
    }

    let mut node_style = String::new();
    let mut label_style = String::new();

    let mut label_color: Option<String> = None;
    let mut label_font_family: Option<String> = None;
    let mut label_font_size: Option<String> = None;
    let mut label_font_weight: Option<String> = None;
    let mut label_opacity: Option<String> = None;

    let mut fill: Option<String> = None;
    let mut stroke: Option<String> = None;
    let mut stroke_width: Option<String> = None;
    let mut stroke_dasharray: Option<String> = None;

    for (k, v) in &m.order {
        let k = *k;
        let v = *v;
        if is_text_style_key(k) {
            if !label_style.is_empty() {
                label_style.push(';');
            }
            let _ = write!(&mut label_style, "{k}:{v} !important");
            match k {
                "color" => label_color = Some(v.to_string()),
                "font-family" => label_font_family = Some(v.to_string()),
                "font-size" => label_font_size = Some(v.to_string()),
                "font-weight" => label_font_weight = Some(v.to_string()),
                "opacity" => label_opacity = Some(v.to_string()),
                _ => {}
            }
        } else {
            if !node_style.is_empty() {
                node_style.push(';');
            }
            let _ = write!(&mut node_style, "{k}:{v} !important");
        }
        match k {
            "fill" => fill = Some(v.to_string()),
            "stroke" => stroke = Some(v.to_string()),
            "stroke-width" => stroke_width = Some(v.to_string()),
            "stroke-dasharray" => stroke_dasharray = Some(v.to_string()),
            _ => {}
        }
    }

    FlowchartCompiledStyles {
        node_style,
        label_style,
        label_color,
        label_font_family,
        label_font_size,
        label_font_weight,
        label_opacity,
        fill,
        stroke,
        stroke_width,
        stroke_dasharray,
    }
}
