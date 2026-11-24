//! The bounded classDef projection shared by source measurement and SVG class selectors.

use super::FlowchartSourceFacetStatus;
use crate::diagram_theme::PreparedSourceStyleDeclaration;
use crate::text::TextStyle;
use indexmap::IndexMap;
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::cell::{RefCell, RefMut};
use std::sync::Arc;

/// The authored definitions for one operation and the facets of the fixed DOM shells it visits.
pub(crate) struct FlowchartClassDefinitions<'a> {
    pub(crate) source: &'a IndexMap<String, Vec<String>>,
    shells: RefCell<FxHashMap<&'static str, FlowchartPreparedClassShell>>,
}

impl<'a> FlowchartClassDefinitions<'a> {
    pub(crate) fn new(source: &'a IndexMap<String, Vec<String>>) -> Self {
        Self {
            source,
            shells: RefCell::default(),
        }
    }

    /// Charge the original declarations for every occurrence before exposing the shared result.
    pub(crate) fn prepare_shell(
        &self,
        class: &'static str,
        charge: impl FnMut(usize) -> crate::Result<()>,
    ) -> crate::Result<Option<RefMut<'_, FlowchartPreparedClassShell>>> {
        let Some(declarations) = self.charge_shell(class, charge)? else {
            return Ok(None);
        };
        Ok(Some(RefMut::map(self.shells.borrow_mut(), |shells| {
            shells.entry(class).or_insert_with(|| {
                FlowchartPreparedClassShell::new(
                    class,
                    &FlowchartClassDefStyles::prepare(declarations),
                )
            })
        })))
    }

    /// Edge preparation needs the complete CSSOM declarations to replay its emitted CSS checks.
    /// Leave the smaller terminal projection for subsequent consumers of the same class.
    pub(crate) fn prepare_shell_cssom(
        &self,
        class: &'static str,
        charge: impl FnMut(usize) -> crate::Result<()>,
    ) -> crate::Result<Option<FlowchartClassDefStyles>> {
        let Some(declarations) = self.charge_shell(class, charge)? else {
            return Ok(None);
        };
        let styles = FlowchartClassDefStyles::prepare(declarations);
        self.shells
            .borrow_mut()
            .entry(class)
            .or_insert_with(|| FlowchartPreparedClassShell::new(class, &styles));
        Ok(Some(styles))
    }

    fn charge_shell(
        &self,
        class: &str,
        mut charge: impl FnMut(usize) -> crate::Result<()>,
    ) -> crate::Result<Option<&[String]>> {
        let Some(declarations) = self.source.get(class) else {
            return Ok(None);
        };
        for declaration in declarations {
            charge(declaration.len())?;
        }
        Ok(Some(declarations))
    }
}

#[derive(Debug)]
pub(crate) struct FlowchartPreparedClassShell {
    class_id: Arc<str>,
    shape_labels: FlowchartShellLabelStyles,
    text_labels: FlowchartShellLabelStyles,
    pub(crate) paint: FlowchartShellPaint,
}

impl FlowchartPreparedClassShell {
    fn new(class: &str, styles: &FlowchartClassDefStyles) -> Self {
        let shape = &styles.shape;
        Self {
            class_id: Arc::from(class),
            shape_labels: FlowchartShellLabelStyles::project(shape, "color"),
            text_labels: FlowchartShellLabelStyles::project(&styles.text, "fill"),
            paint: FlowchartShellPaint {
                fill: shape.contains_key("fill"),
                stroke: shape.contains_key("stroke"),
                stroke_width: shape.contains_key("stroke-width"),
                stroke_dasharray: shape.contains_key("stroke-dasharray"),
                radius: shape.contains_key("rx") || shape.contains_key("ry"),
            },
        }
    }
}

#[derive(Debug)]
pub(crate) struct FlowchartShellPaint {
    pub(crate) fill: bool,
    pub(crate) stroke: bool,
    pub(crate) stroke_width: bool,
    pub(crate) stroke_dasharray: bool,
    pub(crate) radius: bool,
}

#[derive(Debug, Default)]
struct FlowchartShellLabelStyles {
    foreground: Option<Arc<PreparedSourceStyleDeclaration>>,
    fonts: [Option<FlowchartShellFont>; 2],
}

#[derive(Debug)]
struct FlowchartShellFont {
    ordinal: usize,
    declaration: Arc<PreparedSourceStyleDeclaration>,
}

impl FlowchartShellLabelStyles {
    fn project(
        cssom: &IndexMap<String, Arc<PreparedSourceStyleDeclaration>>,
        foreground: &str,
    ) -> Self {
        let mut labels = Self {
            foreground: cssom.get(foreground).cloned(),
            ..Self::default()
        };
        for (ordinal, (property, declaration)) in cssom.iter().enumerate() {
            let affected = match property.as_str() {
                "font-family" => [true, false],
                "font-size" => [false, true],
                "font" | "all" => {
                    let raw = format!("{property}:{}", declaration.value());
                    let inherits = matches!(declaration.value().trim(), "inherit" | "unset");
                    [
                        inherits
                            || crate::mermaid_style::css_font_family_declaration_ownership([
                                raw.as_str()
                            ]) != crate::mermaid_style::CssFontFamilyOwnership::Inherited,
                        inherits
                            || crate::mermaid_style::css_font_size_declaration_ownership([
                                raw.as_str()
                            ]) != crate::mermaid_style::CssFontSizeOwnership::Inherited,
                    ]
                }
                _ => [false, false],
            };
            for (owned, winner) in affected.into_iter().zip(&mut labels.fonts) {
                if owned {
                    *winner = Some(FlowchartShellFont {
                        ordinal,
                        declaration: Arc::clone(declaration),
                    });
                }
            }
        }
        labels
    }
}

#[derive(Debug, Default)]
pub(crate) struct FlowchartClassDefStyles {
    pub(crate) shape: IndexMap<String, Arc<PreparedSourceStyleDeclaration>>,
    pub(crate) text: IndexMap<String, Arc<PreparedSourceStyleDeclaration>>,
}

impl FlowchartClassDefStyles {
    pub(crate) fn prepare(declarations: &[String]) -> Self {
        let mut styles = Self::default();
        for raw in declarations {
            Self::set(&mut styles.shape, raw);
            // Preserve FlowDB's source-sensitive textStyles transform before CSSOM identity.
            if raw.contains("color") {
                let raw = raw
                    .replacen("fill", "bgFill", 1)
                    .replacen("color", "fill", 1);
                Self::set(&mut styles.text, &raw);
            }
        }
        styles
    }

    fn set(styles: &mut IndexMap<String, Arc<PreparedSourceStyleDeclaration>>, raw: &str) {
        let Some(declaration) = PreparedSourceStyleDeclaration::parse(raw) else {
            return;
        };
        let property = declaration.property().to_owned();
        // CSSOM replacement moves a normalized property to its final declaration position.
        styles.shift_remove(&property);
        styles.insert(property, Arc::new(declaration));
    }
}

/// Fixed label shells emitted by the node and cluster writers, not arbitrary CSS selectors.
#[derive(Clone, Copy)]
pub(crate) struct FlowchartLabelDom<'a> {
    node: Option<(&'a str, bool)>,
    label: &'a str,
    label_type: &'a str,
    html: bool,
}

impl<'a> FlowchartLabelDom<'a> {
    pub(crate) fn node(
        shape: &'a str,
        label: &'a str,
        label_type: &'a str,
        html: bool,
        hand_drawn: bool,
    ) -> Self {
        Self {
            node: Some((shape, hand_drawn)),
            label,
            label_type,
            html,
        }
    }

    pub(crate) fn cluster(label: &'a str, label_type: &'a str, html: bool) -> Self {
        Self {
            node: None,
            label,
            label_type,
            html,
        }
    }

    /// The number is the fixed terminal depth, used only to distinguish direct glyph/content
    /// declarations from inherited outer-shell fonts. ClassDef order breaks ties at one depth.
    pub(crate) fn classes(self) -> Vec<(&'static str, u8)> {
        let mut classes = if let Some((shape, hand_drawn)) = self.node {
            let wrapper = if shape == "imageSquare" {
                "image-shape"
            } else if shape.starts_with("icon") {
                "icon-shape"
            } else if hand_drawn {
                "rough-node"
            } else {
                "node"
            };
            let mut classes = vec![("root", 0), ("nodes", 0), (wrapper, 0), ("label", 1)];
            if shape == "note" {
                classes.push(("noteLabel", 1));
            }
            if let Some(shape) = super::bpmn::BpmnShape::resolve(shape) {
                classes.extend(
                    shape
                        .classes()
                        .split_ascii_whitespace()
                        .map(|class| (class, 0)),
                );
            }
            if self.html && matches!(wrapper, "image-shape" | "icon-shape") {
                classes.push(("labelBkg", 0));
            }
            classes
        } else {
            vec![
                ("root", 0),
                ("clusters", 0),
                ("cluster", 0),
                ("cluster-label", 1),
            ]
        };
        if self.html {
            // The shared paragraph projection proves a real direct child of nodeLabel.
            // Unsupported raw block/Math descendants retain their existing local boundaries.
            if crate::text::mermaid_markdown_wants_paragraph_wrap(self.label) {
                classes.push(("nodeLabel", 2));
                if self.node.is_some() && self.label_type == "markdown" {
                    classes.push(("markdown-node-label", 2));
                }
            }
        } else {
            classes.extend([("text-outer-tspan", 2), ("row", 2)]);
        }
        classes
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FlowchartStructuralFontSource {
    pub(crate) declaration: Arc<PreparedSourceStyleDeclaration>,
    pub(crate) class_id: Arc<str>,
    pub(crate) ordinal: usize,
    depth: u8,
    content_owned: bool,
}

impl FlowchartStructuralFontSource {
    pub(crate) const fn owns_content(&self) -> bool {
        self.content_owned
    }

    pub(crate) fn measurable(&self) -> bool {
        match self.declaration.property() {
            "font-family" => crate::text::parse_css_font_stack(self.declaration.value()).is_some(),
            // Relative sizes may be applied at several matching spans/ancestors. Admit only
            // absolute values here rather than inventing a CSS inheritance evaluator.
            "font-size" => {
                self.declaration
                    .svg_number_or_px()
                    .is_some_and(|size| size >= 0.0)
                    && self
                        .declaration
                        .resolve_font_size_px(crate::mermaid_style::CssFontSizeContext::uniform(
                            16.0,
                        ))
                        .is_some()
            }
            _ => false,
        }
    }

    fn status(&self) -> FlowchartSourceFacetStatus {
        FlowchartSourceFacetStatus::from_parts(true, self.measurable())
    }
}

#[derive(Debug, Default)]
pub(crate) struct FlowchartStructuralLabelTypography {
    pub(crate) font_stack: Option<FlowchartStructuralFontSource>,
    pub(crate) font_size: Option<FlowchartStructuralFontSource>,
    pub(crate) foreground: Option<FlowchartStructuralFontSource>,
}

impl FlowchartStructuralLabelTypography {
    pub(crate) fn prepare(
        class_defs: &FlowchartClassDefinitions<'_>,
        css_html_labels: bool,
        dom: FlowchartLabelDom<'_>,
        mut charge: impl FnMut(usize) -> crate::Result<()>,
    ) -> crate::Result<Self> {
        let mut result = Self::default();
        if class_defs.source.is_empty() {
            return Ok(result);
        }
        charge(dom.label.len())?;
        let classes = dom.classes();
        // Inspect only the fixed DOM shells, then restore stylesheet order. This avoids a
        // complete class catalog scan for every node while charging all parsed source bytes.
        let mut matching = Vec::new();
        for (class, depth) in classes {
            charge(1)?;
            if let Some(index) = class_defs.source.get_index_of(class) {
                matching.push((index, class, depth));
            }
        }
        matching.sort_by_key(|(index, ..)| *index);
        for (_, class, shell_depth) in matching {
            let Some(shell) = class_defs.prepare_shell(class, &mut charge)? else {
                continue;
            };
            // Native .class tspan is emitted in both CSS label modes. HTML glyphs use color
            // from .class span or the proven nodeLabel direct children, never shape fill.
            let foreground = if !dom.html {
                shell.text_labels.foreground.as_ref()
            } else if css_html_labels {
                shell.shape_labels.foreground.as_ref()
            } else {
                None
            };
            let foreground_depth = if dom.html && shell_depth == 2 { 2 } else { 0 };
            if let Some(declaration) = foreground
                && result
                    .foreground
                    .as_ref()
                    .is_none_or(|current| current.depth <= foreground_depth)
            {
                result.foreground = Some(FlowchartStructuralFontSource {
                    declaration: Arc::clone(declaration),
                    class_id: Arc::clone(&shell.class_id),
                    ordinal: 0,
                    depth: foreground_depth,
                    content_owned: false,
                });
            }
            if dom.html && !css_html_labels {
                continue;
            }
            let label_styles = if css_html_labels {
                &shell.shape_labels
            } else {
                &shell.text_labels
            };
            let (depth, content_owned) = if dom.html {
                (u8::from(shell_depth == 2), shell_depth == 2)
            } else if css_html_labels {
                (shell_depth, shell_depth > 0)
            } else {
                (0, true)
            };
            // With HTML selectors but native labels, root/nodes family inheritance stops
            // at the default .label family; node/BPMN>* directly reaches that label group.
            let stack_inherited =
                !dom.html && css_html_labels && matches!(class, "root" | "nodes" | "clusters");
            let [font_stack, font_size] = &label_styles.fonts;
            for (winner, slot) in [
                (
                    font_stack.as_ref().filter(|_| !stack_inherited),
                    &mut result.font_stack,
                ),
                (font_size.as_ref(), &mut result.font_size),
            ] {
                if let Some(winner) = winner
                    && slot.as_ref().is_none_or(|current| current.depth <= depth)
                {
                    *slot = Some(FlowchartStructuralFontSource {
                        declaration: Arc::clone(&winner.declaration),
                        class_id: Arc::clone(&shell.class_id),
                        ordinal: winner.ordinal,
                        depth,
                        content_owned,
                    });
                }
            }
        }
        Ok(result)
    }

    pub(crate) fn font_stack_status(&self) -> FlowchartSourceFacetStatus {
        self.font_stack
            .as_ref()
            .map_or(FlowchartSourceFacetStatus::Absent, |source| source.status())
    }

    pub(crate) fn font_size_status(&self) -> FlowchartSourceFacetStatus {
        self.font_size
            .as_ref()
            .map_or(FlowchartSourceFacetStatus::Absent, |source| source.status())
    }

    pub(crate) fn foreground_status(&self) -> FlowchartSourceFacetStatus {
        self.foreground
            .as_ref()
            .map_or(FlowchartSourceFacetStatus::Absent, |source| {
                FlowchartSourceFacetStatus::from_parts(
                    true,
                    crate::mermaid_style::is_supported_css_color_value(source.declaration.value()),
                )
            })
    }

    pub(crate) fn apply_to_parent<'a>(&self, parent: Cow<'a, TextStyle>) -> Cow<'a, TextStyle> {
        if self.font_stack.is_none() && self.font_size.is_none() {
            return parent;
        }
        let mut cascade = crate::mermaid_style::TextStyleCascade::new(
            &parent,
            crate::mermaid_style::CssFontSizeContext::uniform(parent.font_size),
        );
        for source in self.font_stack.iter().chain(&self.font_size) {
            if source.measurable() && !source.content_owned {
                cascade.observe(
                    source.declaration.property(),
                    source.declaration.value(),
                    true,
                );
            }
        }
        Cow::Owned(cascade.finish().into_owned())
    }

    pub(crate) fn apply_to_content(&self, style: &mut Cow<'_, TextStyle>) {
        if !self
            .font_stack
            .iter()
            .chain(&self.font_size)
            .any(|source| source.measurable() && source.content_owned)
        {
            return;
        }
        let mut cascade = crate::mermaid_style::TextStyleCascade::new(
            style.as_ref(),
            crate::mermaid_style::CssFontSizeContext::uniform(style.font_size),
        );
        for source in self.font_stack.iter().chain(&self.font_size) {
            if source.measurable() && source.content_owned {
                cascade.observe(
                    source.declaration.property(),
                    source.declaration.value(),
                    true,
                );
            }
        }
        *style = Cow::Owned(cascade.finish().into_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_projection_bounds_catalog_work_and_propagates_rejection() {
        let source = "font-size:32px";
        let mut definitions = IndexMap::from([("root".to_owned(), vec![source.to_owned()])]);
        let dom = FlowchartLabelDom::node("squareRect", "WIDE", "text", true, false);
        let mut original_work = 0;
        let prepared = FlowchartStructuralLabelTypography::prepare(
            &FlowchartClassDefinitions::new(&definitions),
            true,
            dom,
            |units| {
                original_work += units;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            prepared.font_size_status(),
            FlowchartSourceFacetStatus::Admitted
        );
        assert!(original_work >= source.len() + "WIDE".len());

        for index in 0..1_000 {
            definitions.insert(
                format!("unrelated_{index}"),
                vec!["font-size:99px".to_owned()],
            );
        }
        let class_defs = FlowchartClassDefinitions::new(&definitions);
        let mut expanded_work = 0;
        let first = FlowchartStructuralLabelTypography::prepare(&class_defs, true, dom, |units| {
            expanded_work += units;
            Ok(())
        })
        .unwrap();
        assert_eq!(expanded_work, original_work);

        let mut reused_work = 0;
        let reused = FlowchartStructuralLabelTypography::prepare(&class_defs, true, dom, |units| {
            reused_work += units;
            Ok(())
        })
        .unwrap();
        assert_eq!(reused_work, expanded_work);
        assert!(Arc::ptr_eq(
            &first.font_size.unwrap().declaration,
            &reused.font_size.unwrap().declaration,
        ));

        let rejected =
            FlowchartStructuralLabelTypography::prepare(&class_defs, true, dom, |units| {
                if units == source.len() {
                    Err(crate::Error::InvalidModel {
                        message: "test work limit".to_owned(),
                    })
                } else {
                    Ok(())
                }
            });
        assert!(
            rejected.is_err(),
            "source parsing must propagate the operation's work rejection"
        );
    }
}
