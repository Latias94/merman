//! Flowchart style compilation helpers.

use super::*;

#[derive(Debug, Clone)]
pub(in crate::svg::parity) struct FlowchartCompiledStyles {
    pub(super) node_style: String,
    pub(super) label_style: String,
    pub(super) label_div_decls: Vec<(String, String)>,
    pub(super) fill: Option<String>,
    pub(super) fill_declared: bool,
    fill_unverified: Option<UnverifiedSourcePaint>,
    pub(super) stroke: Option<String>,
    pub(super) stroke_declared: bool,
    stroke_unverified: Option<UnverifiedSourcePaint>,
    pub(super) stroke_width: Option<String>,
    pub(super) stroke_dasharray: Option<String>,
}

#[derive(Debug, Clone)]
enum PendingSourceProvenance {
    AssignedClass {
        class_id: String,
        assignment_ordinal: usize,
        declaration_ordinal: usize,
    },
    Inline {
        declaration_ordinal: usize,
    },
}

impl PendingSourceProvenance {
    fn bind(&self, owner_id: &str) -> crate::diagram_theme::SourceStyleProvenance {
        match self {
            Self::AssignedClass {
                class_id,
                assignment_ordinal,
                declaration_ordinal,
            } => crate::diagram_theme::SourceStyleProvenance::assigned_class(
                owner_id,
                class_id,
                crate::diagram_theme::SourceStyleChannel::Shape,
                *assignment_ordinal,
                *declaration_ordinal,
            ),
            Self::Inline {
                declaration_ordinal,
            } => crate::diagram_theme::SourceStyleProvenance::inline(
                owner_id,
                crate::diagram_theme::SourceStyleChannel::Shape,
                *declaration_ordinal,
            ),
        }
    }
}

#[derive(Debug, Clone)]
struct UnverifiedSourcePaint {
    raw: String,
    provenance: PendingSourceProvenance,
}

impl UnverifiedSourcePaint {
    fn residual(&self, owner_id: &str) -> crate::diagram_theme::SourceStyleResidual {
        let declaration = crate::diagram_theme::SourceStyleDeclaration::parse(
            &self.raw,
            self.provenance.bind(owner_id),
        )
        .expect("compiled Flowchart declarations were parsed before source admission");
        crate::diagram_theme::SourceStyleResidual::from_declaration(
            &declaration,
            crate::diagram_theme::SourceStyleResidualReason::InvalidValue,
        )
    }
}

impl FlowchartCompiledStyles {
    pub(super) fn unverified_shape_paint_residuals(
        &self,
        owner_id: &str,
    ) -> Vec<crate::diagram_theme::SourceStyleResidual> {
        self.fill_unverified
            .iter()
            .chain(self.stroke_unverified.iter())
            .map(|paint| paint.residual(owner_id))
            .collect()
    }
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
    struct OrderedDeclaration {
        property: String,
        property_css: String,
        value: String,
        raw: String,
        provenance: PendingSourceProvenance,
    }

    #[derive(Default)]
    struct OrderedMap {
        order: Vec<OrderedDeclaration>,
        idx: FxHashMap<String, usize>,
    }
    impl OrderedMap {
        fn set(
            &mut self,
            property: String,
            property_css: String,
            value: String,
            raw: String,
            provenance: PendingSourceProvenance,
        ) {
            if let Some(&index) = self.idx.get(&property) {
                self.order[index].property_css = property_css;
                self.order[index].value = value;
                self.order[index].raw = raw;
                self.order[index].provenance = provenance;
                return;
            }
            self.idx.insert(property.clone(), self.order.len());
            self.order.push(OrderedDeclaration {
                property,
                property_css,
                value,
                raw,
                provenance,
            });
        }
    }

    let mut m = OrderedMap::default();

    let mut declaration_ordinal = 0;
    for (assignment_ordinal, c) in classes.iter().enumerate() {
        let Some(decls) = class_defs.get(c) else {
            continue;
        };
        for d in decls {
            for d in crate::flowchart::flowchart_split_mermaid_style_decls(d) {
                let ordinal = declaration_ordinal;
                declaration_ordinal += 1;
                let Some(declaration) = crate::mermaid_style::parse_style_declaration(d) else {
                    continue;
                };
                m.set(
                    declaration.property().to_string(),
                    declaration.property_css().to_string(),
                    declaration.value().to_string(),
                    d.trim().to_string(),
                    PendingSourceProvenance::AssignedClass {
                        class_id: c.clone(),
                        assignment_ordinal,
                        declaration_ordinal: ordinal,
                    },
                );
            }
        }
    }

    for d in inline_styles_a.iter().chain(inline_styles_b.iter()) {
        for d in crate::flowchart::flowchart_split_mermaid_style_decls(d) {
            let ordinal = declaration_ordinal;
            declaration_ordinal += 1;
            let Some(declaration) = crate::mermaid_style::parse_style_declaration(d) else {
                continue;
            };
            m.set(
                declaration.property().to_string(),
                declaration.property_css().to_string(),
                declaration.value().to_string(),
                d.trim().to_string(),
                PendingSourceProvenance::Inline {
                    declaration_ordinal: ordinal,
                },
            );
        }
    }

    let mut node_style = String::new();
    let mut label_style = String::new();

    let mut label_div_decls: Vec<(String, String)> = Vec::new();

    let mut fill: Option<String> = None;
    let mut fill_declared = false;
    let mut fill_unverified = None;
    let mut stroke: Option<String> = None;
    let mut stroke_declared = false;
    let mut stroke_unverified = None;
    let mut stroke_width: Option<String> = None;
    let mut stroke_dasharray: Option<String> = None;

    for declaration in &m.order {
        let k = declaration.property.as_str();
        let property_css = if k.starts_with("--") {
            declaration.property_css.as_str()
        } else {
            k
        };
        let v = declaration.value.as_str();
        if is_text_style_key(k) {
            if !label_style.is_empty() {
                label_style.push(';');
            }
            let _ = write!(&mut label_style, "{property_css}:{v} !important");
            label_div_decls.push((property_css.to_string(), v.to_string()));
        } else {
            if !node_style.is_empty() {
                node_style.push(';');
            }
            let _ = write!(&mut node_style, "{property_css}:{v} !important");
        }
        match k {
            "fill" => {
                fill_declared = true;
                fill = admitted_flowchart_source_paint(v).then(|| v.to_string());
                fill_unverified = fill.is_none().then(|| UnverifiedSourcePaint {
                    raw: declaration.raw.clone(),
                    provenance: declaration.provenance.clone(),
                });
            }
            "stroke" => {
                stroke_declared = true;
                stroke = admitted_flowchart_source_paint(v).then(|| v.to_string());
                stroke_unverified = stroke.is_none().then(|| UnverifiedSourcePaint {
                    raw: declaration.raw.clone(),
                    provenance: declaration.provenance.clone(),
                });
            }
            "stroke-width" => stroke_width = Some(v.to_string()),
            "stroke-dasharray" => stroke_dasharray = Some(v.to_string()),
            _ => {}
        }
    }

    FlowchartCompiledStyles {
        node_style,
        label_style,
        label_div_decls,
        fill,
        fill_declared,
        fill_unverified,
        stroke,
        stroke_declared,
        stroke_unverified,
        stroke_width,
        stroke_dasharray,
    }
}

fn admitted_flowchart_source_paint(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("none")
        || merman_core::theme_color::ThemeColor::parse(value.trim()).is_ok()
}

pub(in crate::svg::parity) fn flowchart_compile_node_styles(
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    inline_styles_a: &[String],
    inline_styles_b: &[String],
) -> FlowchartCompiledStyles {
    let effective_classes =
        crate::flowchart::flowchart_effective_node_class_names(class_defs, classes)
            .into_iter()
            .map(|class| class.to_string())
            .collect::<Vec<_>>();
    flowchart_compile_styles(
        class_defs,
        &effective_classes,
        inline_styles_a,
        inline_styles_b,
    )
}

pub(in crate::svg::parity) fn flowchart_label_div_style_prefix(
    styles: &FlowchartCompiledStyles,
    color_as_rgb: bool,
) -> String {
    fn div_style_survives_mermaid_overrides(key: &str) -> bool {
        !matches!(key, "line-height" | "text-align" | "white-space")
    }

    let mut out = String::new();
    for (key, value) in &styles.label_div_decls {
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() || !div_style_survives_mermaid_overrides(key) {
            continue;
        }
        if key == "color" {
            if color_as_rgb {
                let color = super::super::util::cssom_color_value(value);
                let _ = write!(&mut out, "color: {color} !important; ");
            } else {
                let _ = write!(
                    &mut out,
                    "color: {} !important; ",
                    value.to_ascii_lowercase()
                );
            }
        } else {
            let _ = write!(&mut out, "{key}: {value} !important; ");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color_style(value: &str) -> FlowchartCompiledStyles {
        FlowchartCompiledStyles {
            node_style: String::new(),
            label_style: String::new(),
            label_div_decls: vec![("color".to_string(), value.to_string())],
            fill: None,
            fill_declared: false,
            fill_unverified: None,
            stroke: None,
            stroke_declared: false,
            stroke_unverified: None,
            stroke_width: None,
            stroke_dasharray: None,
        }
    }

    #[test]
    fn flowchart_html_label_color_uses_the_shared_cssom_boundary() {
        assert_eq!(
            flowchart_label_div_style_prefix(&color_style("#12345680"), true),
            "color: rgba(18, 52, 86, 0.502) !important; "
        );
        assert_eq!(
            flowchart_label_div_style_prefix(&color_style("hsl(210 50% 40%)"), true),
            "color: rgb(51, 102, 153) !important; "
        );
        assert_eq!(
            flowchart_label_div_style_prefix(&color_style("var(--LabelColor)"), true),
            "color: var(--LabelColor) !important; "
        );
    }

    #[test]
    fn flowchart_shape_style_uses_canonical_property_identity() {
        let styles = flowchart_compile_styles(
            &IndexMap::new(),
            &[],
            &[r"FILL:#ef4444".to_string()],
            &[r"f\69ll:#22c55e".to_string()],
        );

        assert_eq!(styles.fill.as_deref(), Some("#22c55e"));
        assert!(styles.node_style.contains("fill:#22c55e !important"));
        assert!(!styles.node_style.contains("#ef4444"));
    }

    #[test]
    fn unadmitted_source_paint_is_preserved_but_not_treated_as_portable() {
        for value in ["red junk", "var(--paint)", "inherit", "currentColor"] {
            let styles =
                flowchart_compile_styles(&IndexMap::new(), &[], &[format!("fill:{value}")], &[]);

            assert_eq!(styles.fill, None);
            assert!(styles.fill_declared);
            assert!(
                styles
                    .node_style
                    .contains(&format!("fill:{value} !important"))
            );
            let residuals = styles.unverified_shape_paint_residuals("A");
            assert_eq!(residuals.len(), 1);
            assert_eq!(residuals[0].property(), Some("fill"));
            assert_eq!(residuals[0].provenance().owner_id(), "A");
            assert_eq!(
                residuals[0].provenance().origin(),
                crate::diagram_theme::SourceStyleOrigin::InlineStyle
            );
            assert_eq!(
                residuals[0].reason(),
                crate::diagram_theme::SourceStyleResidualReason::InvalidValue
            );
        }
    }

    #[test]
    fn escaped_custom_property_keeps_its_css_spelling() {
        let styles = flowchart_compile_styles(
            &IndexMap::new(),
            &[],
            &[r"--brand\:accent:#22c55e".to_string()],
            &[],
        );

        assert!(
            styles
                .node_style
                .contains(r"--brand\:accent:#22c55e !important")
        );
        assert!(!styles.node_style.contains("--brand:accent:"));
    }
}
