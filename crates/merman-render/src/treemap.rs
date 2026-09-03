use crate::config::json_f64;
use crate::model::{TreemapDiagramLayout, TreemapLeafLayout, TreemapSectionLayout};
use crate::text::{TextMeasurer, TextStyle};
use crate::theme::TreemapTheme;
use crate::{Error, Result};
use merman_core::diagrams::treemap::{
    TreemapDiagramRenderModel, TreemapNodeRenderModel as TreemapNode,
};
use serde_json::Value;
use std::collections::HashMap;

pub(crate) const TREEMAP_SECTION_INNER_PADDING_PX: f64 = 10.0;
pub(crate) const TREEMAP_SECTION_HEADER_HEIGHT_PX: f64 = 25.0;
const TREEMAP_SECTION_LABEL_INSET_X_PX: f64 = 6.0;
const TREEMAP_SECTION_LABEL_FONT_SIZE_PX: f64 = 12.0;
const TREEMAP_SECTION_VALUE_FONT_SIZE_PX: f64 = 10.0;
const TREEMAP_SECTION_LABEL_RESERVED_VALUE_WIDTH_PX: f64 = 30.0;
const TREEMAP_SECTION_LABEL_MIN_VISIBLE_WIDTH_PX: f64 = 15.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TreemapStyleTarget {
    Node,
    Label,
}

#[derive(Debug, Clone)]
pub(crate) struct TreemapStyleDeclaration {
    pub(crate) key: String,
    pub(crate) value: String,
    pub(crate) target: TreemapStyleTarget,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TreemapCompiledStyles {
    pub(crate) label_styles: String,
    pub(crate) node_styles: String,
    pub(crate) border_styles: Vec<String>,
    pub(crate) declarations: Vec<TreemapStyleDeclaration>,
}

impl TreemapCompiledStyles {
    pub(crate) fn label_styles_as_fill(&self) -> String {
        replace_first(&self.label_styles, "color:", "fill:")
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TreemapViewport {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct TreemapTitlePresentation {
    pub(crate) text: String,
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) font_size: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct TreemapSectionPresentation {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) hidden: bool,
    pub(crate) fill: String,
    pub(crate) stroke: String,
    pub(crate) compiled: TreemapCompiledStyles,
    pub(crate) clip_width: f64,
    pub(crate) label_text: String,
    pub(crate) label_fill: String,
    pub(crate) label_x: f64,
    pub(crate) label_y: f64,
    pub(crate) label_font_size: f64,
    pub(crate) value_text: Option<String>,
    pub(crate) value_x: f64,
    pub(crate) value_y: f64,
    pub(crate) value_font_size: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct TreemapLeafPresentation {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) fill: String,
    pub(crate) compiled: TreemapCompiledStyles,
    pub(crate) clip_width: f64,
    pub(crate) clip_height: f64,
    pub(crate) label_fill: String,
    pub(crate) label_x: f64,
    pub(crate) label_y: f64,
    pub(crate) label_font_size: f64,
    pub(crate) label_hidden: bool,
    pub(crate) value_text: Option<String>,
    pub(crate) value_x: f64,
    pub(crate) value_y: f64,
    pub(crate) value_font_size: f64,
    pub(crate) value_hidden: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct TreemapPresentation {
    pub(crate) viewport: TreemapViewport,
    pub(crate) title: Option<TreemapTitlePresentation>,
    pub(crate) sections: Vec<TreemapSectionPresentation>,
    pub(crate) leaves: Vec<TreemapLeafPresentation>,
}

#[derive(Default)]
struct OrdinalScale {
    range: Vec<String>,
    domain: HashMap<String, usize>,
}

impl OrdinalScale {
    fn get(&mut self, key: &str) -> String {
        let idx = if let Some(idx) = self.domain.get(key).copied() {
            idx
        } else {
            let idx = self.domain.len();
            self.domain.insert(key.to_string(), idx);
            idx
        };
        if self.range.is_empty() {
            return String::new();
        }
        self.range[idx % self.range.len()].clone()
    }
}

#[derive(Default)]
struct OrderedMap {
    order: Vec<(String, String)>,
    index: HashMap<String, usize>,
}

impl OrderedMap {
    fn set(&mut self, key: &str, value: &str) {
        if key.is_empty() {
            return;
        }
        if let Some(&index) = self.index.get(key) {
            self.order[index].1 = value.to_string();
            return;
        }
        self.index.insert(key.to_string(), self.order.len());
        self.order.push((key.to_string(), value.to_string()));
    }
}

pub(crate) fn compile_treemap_styles(css_compiled_styles: &[String]) -> TreemapCompiledStyles {
    let mut declarations = OrderedMap::default();
    for entry in css_compiled_styles {
        for raw in entry.split(';') {
            let declaration = raw.trim();
            if declaration.is_empty() {
                continue;
            }
            let (key, value) = declaration
                .split_once(':')
                .map(|(key, value)| (key.trim(), value.trim()))
                .unwrap_or((declaration, ""));
            declarations.set(key, value);
        }
    }

    let mut compiled = TreemapCompiledStyles::default();
    let mut label_styles = Vec::new();
    let mut node_styles = Vec::new();
    for (key, value) in declarations.order {
        if value.is_empty() {
            continue;
        }
        let target = if crate::mermaid_style::is_label_style_key(&key) {
            TreemapStyleTarget::Label
        } else {
            TreemapStyleTarget::Node
        };
        let declaration = format!("{key}:{value}");
        let important = format!("{declaration} !important");
        match target {
            TreemapStyleTarget::Label => label_styles.push(important),
            TreemapStyleTarget::Node => {
                node_styles.push(important.clone());
                if key.contains("stroke") {
                    compiled.border_styles.push(important);
                }
            }
        }
        compiled
            .declarations
            .push(TreemapStyleDeclaration { key, value, target });
    }
    compiled.label_styles = label_styles.join(";");
    compiled.node_styles = node_styles.join(";");
    compiled
}

pub(crate) fn treemap_presentation(
    layout: &TreemapDiagramLayout,
    theme: &TreemapTheme,
    font_family_css: &str,
    bbox_measurer: &dyn TextMeasurer,
    computed_length_measurer: &dyn TextMeasurer,
) -> TreemapPresentation {
    let title_font_size =
        crate::mermaid_style::parse_css_font_size_px(&theme.title_font_size, 14.0).unwrap_or(14.0);
    let title = layout
        .title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
        .map(|text| TreemapTitlePresentation {
            text: text.to_string(),
            x: layout.width / 2.0,
            y: layout.title_height / 2.0,
            font_size: title_font_size,
        });
    let title_bbox = title.as_ref().map(|title| {
        let style = TextStyle {
            font_family: Some(font_family_css.to_string()),
            font_size: title.font_size,
            font_weight: None,
            font_style: None,
        };
        (
            bbox_measurer
                .measure_svg_simple_text_bbox_width_px(&title.text, &style)
                .max(0.0),
            bbox_measurer
                .measure_svg_simple_text_bbox_height_px(&title.text, &style)
                .max(0.0),
        )
    });
    let viewport = treemap_viewport(layout, title.as_ref(), title_bbox);

    let mut color_scale = OrdinalScale::default();
    color_scale.range.push("transparent".to_string());
    color_scale.range.extend(theme.color_scale.iter().cloned());
    let mut color_scale_peer = OrdinalScale::default();
    color_scale_peer.range.push("transparent".to_string());
    color_scale_peer
        .range
        .extend(theme.color_scale_peer.iter().cloned());
    let mut color_scale_label = OrdinalScale::default();
    color_scale_label
        .range
        .extend(theme.color_scale_label.iter().cloned());

    let mut sections = Vec::with_capacity(layout.sections.len());
    for section in &layout.sections {
        let width = section.x1 - section.x0;
        let height = section.y1 - section.y0;
        let hidden = section.depth == 0;
        let compiled =
            compile_treemap_styles(section.css_compiled_styles.as_deref().unwrap_or_default());
        let label_fill = if hidden {
            String::new()
        } else {
            color_scale_label.get(&section.name)
        };
        let label_text = if hidden {
            String::new()
        } else {
            fit_section_label(
                section,
                layout.show_values,
                width,
                font_family_css,
                computed_length_measurer,
            )
        };
        let value_text = layout.show_values.then(|| {
            if section.value == 0.0 {
                String::new()
            } else {
                format_treemap_value(section.value, &layout.value_format)
            }
        });
        sections.push(TreemapSectionPresentation {
            x: section.x0,
            y: section.y0,
            width,
            height,
            hidden,
            fill: color_scale.get(&section.name),
            stroke: color_scale_peer.get(&section.name),
            compiled,
            clip_width: (width - 2.0 * TREEMAP_SECTION_LABEL_INSET_X_PX).max(0.0),
            label_text,
            label_fill,
            label_x: TREEMAP_SECTION_LABEL_INSET_X_PX,
            label_y: TREEMAP_SECTION_HEADER_HEIGHT_PX / 2.0,
            label_font_size: TREEMAP_SECTION_LABEL_FONT_SIZE_PX,
            value_text,
            value_x: width - TREEMAP_SECTION_INNER_PADDING_PX,
            value_y: TREEMAP_SECTION_HEADER_HEIGHT_PX / 2.0,
            value_font_size: TREEMAP_SECTION_VALUE_FONT_SIZE_PX,
        });
    }

    let complex = layout.leaves.len() > 20;
    let base_label_font_size = if complex { 16.0 } else { 38.0 };
    let base_value_font_size = if complex { 14.0 } else { 28.0 };
    let min_label_font_size = if complex { 4.0 } else { 8.0 };
    let min_value_font_size = if complex { 4.0 } else { 6.0 };
    let label_padding = if complex { 2.0 } else { 4.0 };
    let min_display_threshold = if complex { 8.0 } else { 10.0 };
    let spacing = if complex { 1.0 } else { 2.0 };

    let mut leaves = Vec::with_capacity(layout.leaves.len());
    for leaf in &layout.leaves {
        let width = leaf.x1 - leaf.x0;
        let height = leaf.y1 - leaf.y0;
        let fill_key = leaf.parent_name.as_deref().unwrap_or(leaf.name.as_str());
        let fill = color_scale.get(fill_key);
        let compiled =
            compile_treemap_styles(leaf.css_compiled_styles.as_deref().unwrap_or_default());
        let leaf_label_fill = theme.readable_leaf_label_fill(
            &fill,
            &compiled.node_styles,
            color_scale_label.get(&leaf.name),
        );
        let available_width = width - 2.0 * label_padding;
        let available_height = height - 2.0 * label_padding;
        let mut label_font_size = base_label_font_size;
        let label_hidden = if available_width < min_display_threshold
            || available_height < min_display_threshold
        {
            true
        } else {
            let mut style = TextStyle {
                font_family: Some(font_family_css.to_string()),
                font_size: label_font_size,
                font_weight: None,
                font_style: None,
            };
            loop {
                if computed_length_measurer.measure_svg_text_computed_length_px(&leaf.name, &style)
                    <= available_width
                    || label_font_size <= min_label_font_size
                {
                    break;
                }
                label_font_size -= 1.0;
                style.font_size = label_font_size;
            }

            let mut prospective_value_font_size = (label_font_size * 0.6)
                .round()
                .min(base_value_font_size)
                .max(min_value_font_size);
            let mut combined_height = label_font_size + spacing + prospective_value_font_size;
            while combined_height > available_height && label_font_size > min_label_font_size {
                label_font_size -= 1.0;
                style.font_size = label_font_size;
                prospective_value_font_size = (label_font_size * 0.6)
                    .round()
                    .min(base_value_font_size)
                    .max(min_value_font_size);
                combined_height = label_font_size + spacing + prospective_value_font_size;
            }
            style.font_size = label_font_size;
            if complex {
                label_font_size < min_label_font_size || available_height < min_label_font_size
            } else {
                computed_length_measurer.measure_svg_text_computed_length_px(&leaf.name, &style)
                    > available_width
                    || label_font_size < min_label_font_size
                    || available_height < label_font_size
            }
        };

        let value_text = layout.show_values.then(|| {
            if leaf.value == 0.0 {
                String::new()
            } else {
                format_treemap_value(leaf.value, &layout.value_format)
            }
        });
        let mut value_font_size = base_value_font_size;
        let mut value_y = height / 2.0;
        let mut value_hidden = true;
        if value_text.is_some() && !label_hidden {
            value_font_size = (label_font_size * 0.6)
                .round()
                .min(base_value_font_size)
                .max(min_value_font_size);
            value_y = height / 2.0 + label_font_size / 2.0 + spacing;
            let style = TextStyle {
                font_family: Some(font_family_css.to_string()),
                font_size: value_font_size,
                font_weight: None,
                font_style: None,
            };
            let value_width = computed_length_measurer.measure_svg_text_computed_length_px(
                value_text.as_deref().unwrap_or_default(),
                &style,
            );
            value_hidden = value_width > available_width
                || value_y + value_font_size > height - 4.0
                || value_font_size < min_value_font_size;
        }
        leaves.push(TreemapLeafPresentation {
            x: leaf.x0,
            y: leaf.y0,
            width,
            height,
            fill,
            compiled,
            clip_width: (width - 4.0).max(0.0),
            clip_height: (height - 4.0).max(0.0),
            label_fill: leaf_label_fill,
            label_x: width / 2.0,
            label_y: height / 2.0,
            label_font_size,
            label_hidden,
            value_text,
            value_x: width / 2.0,
            value_y,
            value_font_size,
            value_hidden,
        });
    }

    TreemapPresentation {
        viewport,
        title,
        sections,
        leaves,
    }
}

fn fit_section_label(
    section: &TreemapSectionLayout,
    show_values: bool,
    width: f64,
    font_family_css: &str,
    measurer: &dyn TextMeasurer,
) -> String {
    let mut available_width =
        width - TREEMAP_SECTION_LABEL_INSET_X_PX - TREEMAP_SECTION_LABEL_INSET_X_PX;
    if show_values && section.value != 0.0 {
        let value_end = width - TREEMAP_SECTION_INNER_PADDING_PX;
        let label_end = value_end
            - TREEMAP_SECTION_LABEL_RESERVED_VALUE_WIDTH_PX
            - TREEMAP_SECTION_INNER_PADDING_PX;
        available_width = label_end - TREEMAP_SECTION_LABEL_INSET_X_PX;
    }
    available_width = available_width.max(TREEMAP_SECTION_LABEL_MIN_VISIBLE_WIDTH_PX);
    let style = TextStyle {
        font_family: Some(font_family_css.to_string()),
        font_size: TREEMAP_SECTION_LABEL_FONT_SIZE_PX,
        font_weight: Some("bold".to_string()),
        font_style: None,
    };
    if measurer.measure_svg_text_computed_length_px(&section.name, &style) <= available_width {
        return section.name.clone();
    }

    let ellipsis = "...";
    let mut current = section.name.clone();
    while !current.is_empty() {
        current.pop();
        if current.is_empty() {
            return if measurer.measure_svg_text_computed_length_px(ellipsis, &style)
                <= available_width
            {
                ellipsis.to_string()
            } else {
                String::new()
            };
        }
        let candidate = format!("{current}{ellipsis}");
        if measurer.measure_svg_text_computed_length_px(&candidate, &style) <= available_width {
            return candidate;
        }
    }
    String::new()
}

fn treemap_viewport(
    layout: &TreemapDiagramLayout,
    title: Option<&TreemapTitlePresentation>,
    title_bbox: Option<(f64, f64)>,
) -> TreemapViewport {
    let mut bounds = TreemapViewBoxBounds::empty();
    for section in &layout.sections {
        if section.depth != 0 {
            bounds.include(section.x0, section.y0, section.x1, section.y1);
        }
    }
    for leaf in &layout.leaves {
        bounds.include(leaf.x0, leaf.y0, leaf.x1, leaf.y1);
    }
    if layout.title_height > 0.0 && bounds.min_y.is_finite() && bounds.max_y.is_finite() {
        bounds.min_y += layout.title_height;
        bounds.max_y += layout.title_height;
    }
    if let (Some(title), Some((width, height))) = (title, title_bbox) {
        if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
            bounds.include(
                title.x - width / 2.0,
                title.y - height / 2.0,
                title.x + width / 2.0,
                title.y + height / 2.0,
            );
        } else if !title.text.trim().is_empty() {
            bounds.min_y = bounds.min_y.min(0.0);
            bounds.max_y = bounds.max_y.max(layout.title_height);
        }
    }

    if bounds.has_rects() {
        TreemapViewport {
            x: bounds.min_x - layout.diagram_padding,
            y: bounds.min_y - layout.diagram_padding,
            width: bounds.max_x - bounds.min_x + layout.diagram_padding * 2.0,
            height: bounds.max_y - bounds.min_y + layout.diagram_padding * 2.0,
        }
    } else {
        TreemapViewport {
            x: -layout.diagram_padding,
            y: -layout.diagram_padding,
            width: layout.diagram_padding * 2.0,
            height: layout.diagram_padding * 2.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TreemapViewBoxBounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl TreemapViewBoxBounds {
    const fn empty() -> Self {
        Self {
            min_x: f64::INFINITY,
            min_y: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            max_y: f64::NEG_INFINITY,
        }
    }

    fn include(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        let width = x1 - x0;
        let height = y1 - y0;
        if !(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0) {
            return;
        }
        self.min_x = self.min_x.min(x0);
        self.min_y = self.min_y.min(y0);
        self.max_x = self.max_x.max(x1);
        self.max_y = self.max_y.max(y1);
    }

    fn has_rects(self) -> bool {
        self.min_x.is_finite()
            && self.min_y.is_finite()
            && self.max_x.is_finite()
            && self.max_y.is_finite()
    }
}

pub(crate) fn format_treemap_value(value: f64, format: &str) -> String {
    let format = format.trim();
    if let Some(rest) = format.strip_prefix('$') {
        if format == "$0,0" {
            return format!("${}", format_d3_general(value, 12, true));
        }
        if format.contains(',') {
            let precision = format
                .split_once('.')
                .and_then(|(_, precision)| {
                    precision
                        .chars()
                        .take_while(|character| character.is_ascii_digit())
                        .collect::<String>()
                        .parse::<usize>()
                        .ok()
                })
                .unwrap_or(12);
            return format!("${}", format_d3_general(value, precision.max(1), true));
        }
        return format!("${}", format_treemap_value(value, rest));
    }
    if let Some(precision) = format_precision(format, ".", "%") {
        return d3_minus(format!("{:.precision$}%", value * 100.0));
    }
    if let Some(precision) = format_precision(format, ".", "f") {
        return d3_minus(format!("{value:.precision$}"));
    }
    if let Some(precision) = format_precision(format, ",.", "f") {
        return d3_minus(group_decimal(&format!("{value:.precision$}")));
    }
    if let Some(precision) = format_precision(format, ".", "e") {
        return format_scientific(value, precision);
    }
    if format.is_empty() || format == "," {
        return format_d3_general(value, 12, true);
    }
    format_treemap_value(value, ",")
}

pub(crate) fn treemap_value_format_is_portable(format: &str) -> bool {
    let format = format.trim();
    if format.is_empty() || format == "," || format == "$0,0" || format == "$" {
        return true;
    }
    let format = format.strip_prefix('$').unwrap_or(format);
    format_precision(format, ".", "%").is_some()
        || format_precision(format, ".", "f").is_some()
        || format_precision(format, ",.", "f").is_some()
        || format_precision(format, ".", "e").is_some()
}

fn format_precision(value: &str, prefix: &str, suffix: &str) -> Option<usize> {
    value
        .strip_prefix(prefix)?
        .strip_suffix(suffix)?
        .parse::<usize>()
        .ok()
}

fn format_d3_general(value: f64, precision: usize, grouped: bool) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    let precision = precision.clamp(1, 21);
    let exponent = value.abs().log10().floor() as i32;
    let raw = if exponent >= precision as i32 || exponent < -6 {
        trim_significand_zeros(format_scientific(value, precision - 1))
    } else {
        let decimals = (precision as i32 - 1 - exponent).max(0) as usize;
        trim_significand_zeros(format!("{value:.decimals$}"))
    };
    let result = if grouped && !raw.contains(['e', 'E']) {
        group_decimal(&raw)
    } else {
        raw
    };
    d3_minus(result)
}

fn format_scientific(value: f64, precision: usize) -> String {
    let raw = format!("{value:.precision$e}");
    let Some((significand, exponent)) = raw.split_once('e') else {
        return raw;
    };
    if exponent.starts_with(['+', '-']) {
        raw
    } else {
        format!("{significand}e+{exponent}")
    }
}

fn trim_significand_zeros(value: String) -> String {
    let (significand, exponent) = value
        .split_once('e')
        .map(|(significand, exponent)| (significand, Some(exponent)))
        .unwrap_or((value.as_str(), None));
    let significand = if significand.contains('.') {
        significand
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    } else {
        significand.to_string()
    };
    exponent
        .map(|exponent| format!("{significand}e{exponent}"))
        .unwrap_or(significand)
}

fn d3_minus(value: String) -> String {
    value
        .strip_prefix('-')
        .map(|value| format!("−{value}"))
        .unwrap_or(value)
}

fn group_decimal(value: &str) -> String {
    let (integer, fraction) = value.split_once('.').unwrap_or((value, ""));
    let (sign, digits) = integer
        .strip_prefix('-')
        .map(|digits| ("-", digits))
        .unwrap_or(("", integer));
    let mut grouped = String::with_capacity(value.len() + digits.len() / 3);
    grouped.push_str(sign);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(character);
    }
    if !fraction.is_empty() {
        grouped.push('.');
        grouped.push_str(fraction);
    }
    grouped
}

fn replace_first(haystack: &str, needle: &str, replacement: &str) -> String {
    let Some(index) = (!needle.is_empty())
        .then(|| haystack.find(needle))
        .flatten()
    else {
        return haystack.to_string();
    };
    let mut output = String::with_capacity(haystack.len() - needle.len() + replacement.len());
    output.push_str(&haystack[..index]);
    output.push_str(replacement);
    output.push_str(&haystack[index + needle.len()..]);
    output
}

mod config;

use config::TreemapConfigView;

#[derive(Debug, Clone)]
struct HierNode {
    name: String,
    own_value: f64,
    value: f64,
    class_selector: Option<String>,
    css_compiled_styles: Option<Vec<String>>,
    parent: Option<usize>,
    children: Vec<usize>,
    depth: usize,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

fn push_node(nodes: &mut Vec<HierNode>, node: &TreemapNode, parent: Option<usize>, depth: usize) {
    let mut stack = vec![(node, parent, depth)];
    while let Some((current, parent_idx, current_depth)) = stack.pop() {
        let own_value = current.value.as_ref().and_then(json_f64).unwrap_or(0.0);
        let idx = nodes.len();
        nodes.push(HierNode {
            name: current.name.clone(),
            own_value,
            value: 0.0,
            class_selector: current.class_selector.clone(),
            css_compiled_styles: current.css_compiled_styles.clone(),
            parent: parent_idx,
            children: Vec::new(),
            depth: current_depth,
            x0: 0.0,
            y0: 0.0,
            x1: 0.0,
            y1: 0.0,
        });

        if let Some(parent_idx) = parent_idx
            && let Some(parent_node) = nodes.get_mut(parent_idx)
        {
            parent_node.children.push(idx);
        }

        if let Some(children) = current.children.as_ref() {
            for child in children.iter().rev() {
                stack.push((child, Some(idx), current_depth.saturating_add(1)));
            }
        }
    }
}

fn compute_sum(nodes: &mut [HierNode], idx: usize) -> f64 {
    let mut stack = vec![(idx, false)];
    while let Some((node_idx, visited)) = stack.pop() {
        let Some(node) = nodes.get(node_idx) else {
            continue;
        };

        if visited {
            let sum = node.own_value
                + node
                    .children
                    .iter()
                    .filter_map(|&child_idx| nodes.get(child_idx).map(|child| child.value))
                    .sum::<f64>();
            if let Some(node) = nodes.get_mut(node_idx) {
                node.value = sum;
            }
        } else {
            stack.push((node_idx, true));
            for &child_idx in node.children.iter().rev() {
                stack.push((child_idx, false));
            }
        }
    }

    nodes.get(idx).map(|node| node.value).unwrap_or(0.0)
}

fn sort_children_by_value(nodes: &mut [HierNode], idx: usize) {
    let mut stack = vec![idx];
    while let Some(node_idx) = stack.pop() {
        if node_idx >= nodes.len() {
            continue;
        }

        let mut items = nodes[node_idx]
            .children
            .iter()
            .copied()
            .enumerate()
            .map(|(pos, child)| (child, pos))
            .collect::<Vec<_>>();
        items.sort_by(|(a, a_pos), (b, b_pos)| {
            let av = nodes[*a].value;
            let bv = nodes[*b].value;
            bv.partial_cmp(&av)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a_pos.cmp(b_pos))
        });
        nodes[node_idx].children = items.into_iter().map(|(child, _pos)| child).collect();

        let children = nodes[node_idx].children.clone();
        for child_idx in children.into_iter().rev() {
            stack.push(child_idx);
        }
    }
}

fn each_before(nodes: &[HierNode], root: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(idx) = stack.pop() {
        out.push(idx);
        let children = &nodes[idx].children;
        for &c in children.iter().rev() {
            stack.push(c);
        }
    }
    out
}

fn descendants_bfs(nodes: &[HierNode], root: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut next = vec![root];
    while !next.is_empty() {
        let mut current = next;
        current.reverse();
        next = Vec::new();
        while let Some(idx) = current.pop() {
            out.push(idx);
            for &c in &nodes[idx].children {
                next.push(c);
            }
        }
    }
    out
}

fn leaves_each_before(nodes: &[HierNode], root: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for idx in each_before(nodes, root) {
        if nodes[idx].children.is_empty() {
            out.push(idx);
        }
    }
    out
}

fn treemap_round_node(nodes: &mut [HierNode], idx: usize) {
    nodes[idx].x0 = nodes[idx].x0.round();
    nodes[idx].y0 = nodes[idx].y0.round();
    nodes[idx].x1 = nodes[idx].x1.round();
    nodes[idx].y1 = nodes[idx].y1.round();
}

fn treemap_dice(
    nodes: &mut [HierNode],
    children: &[usize],
    row_value: f64,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) {
    let mut x = x0;
    let k = if row_value != 0.0 {
        (x1 - x0) / row_value
    } else {
        0.0
    };
    for &child in children {
        nodes[child].y0 = y0;
        nodes[child].y1 = y1;
        nodes[child].x0 = x;
        x += nodes[child].value * k;
        nodes[child].x1 = x;
    }
}

fn treemap_slice(
    nodes: &mut [HierNode],
    children: &[usize],
    row_value: f64,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) {
    let mut y = y0;
    let k = if row_value != 0.0 {
        (y1 - y0) / row_value
    } else {
        0.0
    };
    for &child in children {
        nodes[child].x0 = x0;
        nodes[child].x1 = x1;
        nodes[child].y0 = y;
        y += nodes[child].value * k;
        nodes[child].y1 = y;
    }
}

fn squarify(nodes: &mut [HierNode], parent: usize, mut x0: f64, mut y0: f64, x1: f64, y1: f64) {
    const PHI: f64 = (1.0 + 2.23606797749979) / 2.0;
    let ratio = PHI;

    let children = nodes[parent].children.clone();
    if children.is_empty() {
        return;
    }

    let n = children.len();
    let mut i0 = 0usize;
    let mut i1 = 0usize;
    let mut value = nodes[parent].value;

    while i0 < n {
        let dx = x1 - x0;
        let dy = y1 - y0;

        let mut sum_value;
        loop {
            if i1 >= n {
                return;
            }
            sum_value = nodes[children[i1]].value;
            i1 += 1;
            if sum_value != 0.0 || i1 >= n {
                break;
            }
        }

        let mut min_value = sum_value;
        let mut max_value = sum_value;

        let alpha = (dy / dx).max(dx / dy) / (value * ratio);
        let mut beta = sum_value * sum_value * alpha;
        let mut min_ratio = (max_value / beta).max(beta / min_value);

        while i1 < n {
            let node_value = nodes[children[i1]].value;
            sum_value += node_value;
            if node_value < min_value {
                min_value = node_value;
            }
            if node_value > max_value {
                max_value = node_value;
            }
            beta = sum_value * sum_value * alpha;
            let new_ratio = (max_value / beta).max(beta / min_value);
            if new_ratio > min_ratio {
                sum_value -= node_value;
                break;
            }
            min_ratio = new_ratio;
            i1 += 1;
        }

        let dice = dx < dy;
        let row_children = &children[i0..i1];
        if dice {
            let y2 = if value != 0.0 {
                y0 + dy * sum_value / value
            } else {
                y1
            };
            treemap_dice(nodes, row_children, sum_value, x0, y0, x1, y2);
            y0 = y2;
        } else {
            let x2 = if value != 0.0 {
                x0 + dx * sum_value / value
            } else {
                x1
            };
            treemap_slice(nodes, row_children, sum_value, x0, y0, x2, y1);
            x0 = x2;
        }

        value -= sum_value;
        i0 = i1;
    }
}

fn position_node(
    nodes: &mut [HierNode],
    idx: usize,
    padding_stack: &mut Vec<f64>,
    padding_inner: f64,
) {
    let depth = nodes[idx].depth;
    if padding_stack.len() <= depth {
        padding_stack.resize(depth + 1, 0.0);
    }
    let mut p = padding_stack[depth];
    let mut x0 = nodes[idx].x0 + p;
    let mut y0 = nodes[idx].y0 + p;
    let mut x1 = nodes[idx].x1 - p;
    let mut y1 = nodes[idx].y1 - p;
    if x1 < x0 {
        x0 = (x0 + x1) / 2.0;
        x1 = x0;
    }
    if y1 < y0 {
        y0 = (y0 + y1) / 2.0;
        y1 = y0;
    }
    nodes[idx].x0 = x0;
    nodes[idx].y0 = y0;
    nodes[idx].x1 = x1;
    nodes[idx].y1 = y1;

    if nodes[idx].children.is_empty() {
        return;
    }

    p = padding_inner / 2.0;
    if padding_stack.len() <= depth + 1 {
        padding_stack.resize(depth + 2, 0.0);
    }
    padding_stack[depth + 1] = p;

    let has_children = true;
    let padding_top = if has_children {
        TREEMAP_SECTION_HEADER_HEIGHT_PX + TREEMAP_SECTION_INNER_PADDING_PX
    } else {
        0.0
    };
    let padding_lr = if has_children {
        TREEMAP_SECTION_INNER_PADDING_PX
    } else {
        0.0
    };
    let padding_bottom = if has_children {
        TREEMAP_SECTION_INNER_PADDING_PX
    } else {
        0.0
    };

    x0 += padding_lr - p;
    y0 += padding_top - p;
    x1 -= padding_lr - p;
    y1 -= padding_bottom - p;
    if x1 < x0 {
        x0 = (x0 + x1) / 2.0;
        x1 = x0;
    }
    if y1 < y0 {
        y0 = (y0 + y1) / 2.0;
        y1 = y0;
    }

    squarify(nodes, idx, x0, y0, x1, y1);
}

pub(crate) fn layout_treemap_diagram_typed(
    model: &TreemapDiagramRenderModel,
    diagram_title: Option<&str>,
    effective_config: &Value,
    _measurer: &dyn crate::text::TextMeasurer,
) -> Result<TreemapDiagramLayout> {
    let cfg = TreemapConfigView::new(effective_config).layout_settings();
    let title = model
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .or_else(|| {
            diagram_title
                .map(str::trim)
                .filter(|title| !title.is_empty())
        })
        .map(str::to_owned);

    let title_height = if title.is_some() { 30.0 } else { 0.0 };

    let width = if cfg.node_width > 0.0 {
        cfg.node_width * TREEMAP_SECTION_INNER_PADDING_PX
    } else {
        960.0
    };
    let height = if cfg.node_height > 0.0 {
        cfg.node_height * TREEMAP_SECTION_INNER_PADDING_PX
    } else {
        500.0
    };

    let mut nodes: Vec<HierNode> = Vec::new();
    push_node(&mut nodes, &model.root, None, 0);
    if nodes.is_empty() {
        return Err(Error::InvalidModel {
            message: "treemap root produced no nodes".to_string(),
        });
    }
    let root_idx = 0usize;

    compute_sum(&mut nodes, root_idx);
    sort_children_by_value(&mut nodes, root_idx);

    nodes[root_idx].x0 = 0.0;
    nodes[root_idx].y0 = 0.0;
    nodes[root_idx].x1 = width;
    nodes[root_idx].y1 = height;

    let mut padding_stack = vec![0.0];
    for idx in each_before(&nodes, root_idx) {
        position_node(&mut nodes, idx, &mut padding_stack, cfg.padding.max(0.0));
    }

    for idx in each_before(&nodes, root_idx) {
        treemap_round_node(&mut nodes, idx);
    }

    let branch_nodes = descendants_bfs(&nodes, root_idx)
        .into_iter()
        .filter(|&idx| !nodes[idx].children.is_empty())
        .collect::<Vec<_>>();

    let leaf_nodes = leaves_each_before(&nodes, root_idx);

    let mut sections = Vec::new();
    for idx in &branch_nodes {
        let n = &nodes[*idx];
        sections.push(TreemapSectionLayout {
            name: n.name.clone(),
            depth: n.depth as i64,
            value: n.value,
            x0: n.x0,
            y0: n.y0,
            x1: n.x1,
            y1: n.y1,
            class_selector: n.class_selector.clone(),
            css_compiled_styles: n.css_compiled_styles.clone(),
        });
    }

    let mut leaves = Vec::new();
    for idx in &leaf_nodes {
        let n = &nodes[*idx];
        leaves.push(TreemapLeafLayout {
            name: n.name.clone(),
            value: n.value,
            parent_name: n.parent.map(|p| nodes[p].name.clone()),
            x0: n.x0,
            y0: n.y0,
            x1: n.x1,
            y1: n.y1,
            class_selector: n.class_selector.clone(),
            css_compiled_styles: n.css_compiled_styles.clone(),
        });
    }

    Ok(TreemapDiagramLayout {
        title_height,
        width,
        height,
        use_max_width: cfg.use_max_width,
        diagram_padding: cfg.diagram_padding.max(0.0),
        show_values: cfg.show_values,
        value_format: cfg.value_format,
        acc_title: model.acc_title.clone(),
        acc_descr: model.acc_descr.clone(),
        title,
        sections,
        leaves,
    })
}

#[cfg(test)]
mod tests {
    use super::format_treemap_value;

    #[test]
    fn treemap_geometry_constants_match_mermaid() {
        assert_eq!(super::TREEMAP_SECTION_INNER_PADDING_PX, 10.0);
        assert_eq!(super::TREEMAP_SECTION_HEADER_HEIGHT_PX, 25.0);
    }

    #[test]
    fn treemap_value_formats_cover_the_upstream_fixture_set() {
        let cases = [
            (",", "1,234.5"),
            (".1%", "123450.0%"),
            (".2f", "1234.50"),
            ("$.2f", "$1234.50"),
            (",.2f", "1,234.50"),
            ("$.1%", "$123450.0%"),
            (".2e", "1.23e+3"),
            ("$,.2f", "$1.2e+3"),
            ("$,.0f", "$1e+3"),
            ("$0,0", "$1,234.5"),
        ];
        for (format, expected) in cases {
            assert_eq!(format_treemap_value(1234.5, format), expected, "{format}");
        }
    }
}
