use crate::text::{PreparedTextCssTypographyOverrides, TextMetrics, TextStyle};
use indexmap::IndexMap;
use std::borrow::Cow;

fn parse_style_decl(s: &str) -> Option<(&str, &str, bool)> {
    let parsed = crate::mermaid_style::parse_style_declaration(s)?;
    Some((parsed.property_source(), parsed.value(), parsed.important()))
}

fn normalize_css_font_family(font_family: &str) -> String {
    let font_family = font_family.trim().trim_end_matches(';').trim();
    if crate::mermaid_style::is_safe_css_font_family_value(font_family) {
        font_family.to_string()
    } else {
        String::new()
    }
}

pub(crate) fn flowchart_split_mermaid_style_decls(s: &str) -> impl Iterator<Item = &str> {
    fn looks_like_key_start(s: &str) -> bool {
        let s = s.trim_start();
        let Some((k, _)) = s.split_once(':') else {
            return false;
        };
        let k = k.trim();
        !k.is_empty()
            && k.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    }

    let mut parts: Vec<&str> = Vec::new();
    let mut start = 0usize;
    for (i, ch) in s.char_indices() {
        if ch != ',' {
            continue;
        }
        if looks_like_key_start(&s[i + 1..]) {
            let p = s[start..i].trim();
            if !p.is_empty() {
                parts.push(p);
            }
            start = i + 1;
        }
    }
    let tail = s[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts.into_iter()
}

pub(crate) fn flowchart_is_source_spelled_label_style_key(key: &str) -> bool {
    matches!(
        key.trim(),
        "color"
            | "font-size"
            | "font-family"
            | "font-weight"
            | "font-style"
            | "text-decoration"
            | "text-align"
            | "text-transform"
            | "line-height"
            | "letter-spacing"
            | "word-spacing"
            | "text-shadow"
            | "text-overflow"
            | "white-space"
            | "word-wrap"
            | "word-break"
            | "overflow-wrap"
            | "hyphens"
    )
}

pub(crate) fn flowchart_apply_text_style_decl(
    style: &mut std::borrow::Cow<'_, TextStyle>,
    key: &str,
    value: &str,
) {
    match key.trim() {
        "font-size" => {
            let inherited_px = style.as_ref().font_size;
            if let Some(px) = crate::mermaid_style::resolve_mermaid_font_size_px(
                value,
                crate::mermaid_style::CssFontSizeContext::uniform(inherited_px),
            ) {
                style.to_mut().font_size = px;
            }
        }
        "font-family" => {
            let font_family = normalize_css_font_family(value);
            if !font_family.is_empty() {
                style.to_mut().font_family = Some(font_family);
            }
        }
        "font-weight" => {
            style.to_mut().font_weight = Some(value.trim().to_string());
        }
        "font-style" => {
            style.to_mut().font_style = Some(value.trim().to_string());
        }
        _ => {}
    }
}

#[derive(Debug)]
pub(crate) struct FlowchartTextStyleResolution<'a> {
    pub(crate) style: Cow<'a, TextStyle>,
    /// Source-owned typography declarations whose final values must be admitted together for
    /// prepared measurement and SVG emission.
    pub(crate) prepared_text_overrides: PreparedTextCssTypographyOverrides,
    pub(crate) terminal_foreground: Option<FlowchartTerminalForeground>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowchartTerminalForegroundProvenance {
    ThemeNode,
    ThemeTitle,
    AssignedClass,
    InlineStyle,
    EdgeLabelStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlowchartTerminalForeground {
    value: String,
    provenance: FlowchartTerminalForegroundProvenance,
    important: bool,
}

impl FlowchartTerminalForeground {
    pub(crate) fn new(value: &str, provenance: FlowchartTerminalForegroundProvenance) -> Self {
        Self {
            value: value.trim().to_string(),
            provenance,
            important: false,
        }
    }

    fn observe(
        target: &mut Option<Self>,
        value: &str,
        provenance: FlowchartTerminalForegroundProvenance,
        important: bool,
    ) {
        if target
            .as_ref()
            .is_some_and(|current| current.important && !important)
        {
            return;
        }
        *target = Some(Self {
            value: value.trim().to_string(),
            provenance,
            important,
        });
    }

    pub(crate) fn value(&self) -> &str {
        &self.value
    }

    #[cfg(test)]
    pub(crate) const fn provenance(&self) -> FlowchartTerminalForegroundProvenance {
        self.provenance
    }
}

impl AsRef<TextStyle> for FlowchartTextStyleResolution<'_> {
    fn as_ref(&self) -> &TextStyle {
        self.style.as_ref()
    }
}

impl<'a> FlowchartTextStyleResolution<'a> {
    pub(crate) fn borrowed(style: &'a TextStyle) -> Self {
        Self {
            style: Cow::Borrowed(style),
            prepared_text_overrides: PreparedTextCssTypographyOverrides::default(),
            terminal_foreground: None,
        }
    }

    pub(crate) fn terminal_foreground(&self) -> Option<&FlowchartTerminalForeground> {
        self.terminal_foreground.as_ref()
    }
}

impl std::ops::Deref for FlowchartTextStyleResolution<'_> {
    type Target = TextStyle;

    fn deref(&self) -> &Self::Target {
        self.style.as_ref()
    }
}

fn apply_text_style_decl_with_provenance(
    style: &mut Cow<'_, TextStyle>,
    key: &str,
    value: &str,
    prepared_text_overrides: &mut PreparedTextCssTypographyOverrides,
    terminal_foreground: &mut Option<FlowchartTerminalForeground>,
    provenance: FlowchartTerminalForegroundProvenance,
    important: bool,
) {
    if !flowchart_is_source_spelled_label_style_key(key) {
        return;
    }
    flowchart_apply_text_style_decl(style, key, value);
    prepared_text_overrides.observe_declaration(key, value);
    if key.trim() == "color" {
        FlowchartTerminalForeground::observe(terminal_foreground, value, provenance, important);
    }
}

fn flowchart_effective_text_style_for_class_names_with_provenance<'a, 'b>(
    base: &'a TextStyle,
    class_defs: &IndexMap<String, Vec<String>>,
    class_names: impl IntoIterator<Item = &'b str>,
    inline_styles: &[String],
) -> FlowchartTextStyleResolution<'a> {
    let mut style = Cow::Borrowed(base);
    let mut prepared_text_overrides = PreparedTextCssTypographyOverrides::default();
    let mut terminal_foreground = None;

    for class in class_names {
        let Some(decls) = class_defs.get(class) else {
            continue;
        };
        for d in decls {
            for d in flowchart_split_mermaid_style_decls(d) {
                let Some((k, v, important)) = parse_style_decl(d) else {
                    continue;
                };
                apply_text_style_decl_with_provenance(
                    &mut style,
                    k,
                    v,
                    &mut prepared_text_overrides,
                    &mut terminal_foreground,
                    FlowchartTerminalForegroundProvenance::AssignedClass,
                    important,
                );
            }
        }
    }

    for d in inline_styles {
        for d in flowchart_split_mermaid_style_decls(d) {
            let Some((k, v, important)) = parse_style_decl(d) else {
                continue;
            };
            apply_text_style_decl_with_provenance(
                &mut style,
                k,
                v,
                &mut prepared_text_overrides,
                &mut terminal_foreground,
                FlowchartTerminalForegroundProvenance::InlineStyle,
                important,
            );
        }
    }

    FlowchartTextStyleResolution {
        style,
        prepared_text_overrides,
        terminal_foreground,
    }
}

pub(crate) fn flowchart_effective_node_class_names<'a>(
    class_defs: &'a IndexMap<String, Vec<String>>,
    classes: &'a [String],
) -> Vec<&'a str> {
    let mut effective: Vec<&'a str> = Vec::with_capacity(classes.len() + 2);
    if class_defs.contains_key("default") {
        effective.push("default");
    }
    if class_defs.contains_key("node") {
        effective.push("node");
    }
    effective.extend(classes.iter().map(|class| class.as_str()));
    effective
}

pub(crate) fn flowchart_effective_text_style_for_node_classes<'a>(
    base: &'a TextStyle,
    class_defs: &'a IndexMap<String, Vec<String>>,
    classes: &'a [String],
    inline_styles: &[String],
) -> std::borrow::Cow<'a, TextStyle> {
    flowchart_effective_text_style_for_node_classes_with_provenance(
        base,
        class_defs,
        classes,
        inline_styles,
    )
    .style
}

pub(crate) fn flowchart_effective_text_style_for_node_classes_with_provenance<'a>(
    base: &'a TextStyle,
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    inline_styles: &[String],
) -> FlowchartTextStyleResolution<'a> {
    let effective_classes = flowchart_effective_node_class_names(class_defs, classes);
    if effective_classes.is_empty() && inline_styles.is_empty() {
        return FlowchartTextStyleResolution {
            style: Cow::Borrowed(base),
            prepared_text_overrides: PreparedTextCssTypographyOverrides::default(),
            terminal_foreground: None,
        };
    }
    flowchart_effective_text_style_for_class_names_with_provenance(
        base,
        class_defs,
        effective_classes,
        inline_styles,
    )
}

pub(crate) fn flowchart_effective_text_style_for_classes<'a>(
    base: &'a TextStyle,
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &'a [String],
    inline_styles: &[String],
) -> std::borrow::Cow<'a, TextStyle> {
    flowchart_effective_text_style_for_classes_with_provenance(
        base,
        class_defs,
        classes,
        inline_styles,
    )
    .style
}

pub(crate) fn flowchart_effective_text_style_for_classes_with_provenance<'a>(
    base: &'a TextStyle,
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    inline_styles: &[String],
) -> FlowchartTextStyleResolution<'a> {
    if classes.is_empty() && inline_styles.is_empty() {
        return FlowchartTextStyleResolution {
            style: Cow::Borrowed(base),
            prepared_text_overrides: PreparedTextCssTypographyOverrides::default(),
            terminal_foreground: None,
        };
    }

    flowchart_effective_text_style_for_class_names_with_provenance(
        base,
        class_defs,
        classes.iter().map(|class| class.as_str()),
        inline_styles,
    )
}

/// Mermaid first compiles edge classes and then applies the concatenated `linkStyle default`
/// and per-edge declarations. The resulting style is applied after SVG line wrapping, but it owns
/// the final text bbox used by the layout graph.
#[cfg(test)]
pub(crate) fn flowchart_effective_edge_label_text_style<'a>(
    base: &'a TextStyle,
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &'a [String],
    default_edge_styles: &[String],
    edge_styles: &[String],
) -> std::borrow::Cow<'a, TextStyle> {
    flowchart_effective_edge_label_text_style_with_provenance(
        base,
        class_defs,
        classes,
        default_edge_styles,
        edge_styles,
    )
    .style
}

#[cfg(test)]
pub(crate) fn flowchart_effective_edge_label_text_style_with_provenance<'a>(
    base: &'a TextStyle,
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    default_edge_styles: &[String],
    edge_styles: &[String],
) -> FlowchartTextStyleResolution<'a> {
    if classes.is_empty() && default_edge_styles.is_empty() && edge_styles.is_empty() {
        return FlowchartTextStyleResolution {
            style: Cow::Borrowed(base),
            prepared_text_overrides: PreparedTextCssTypographyOverrides::default(),
            terminal_foreground: None,
        };
    }

    let mut resolution = flowchart_effective_text_style_for_class_names_with_provenance(
        base,
        class_defs,
        classes.iter().map(|class| class.as_str()),
        &[],
    );
    for declaration in default_edge_styles.iter().chain(edge_styles) {
        for declaration in flowchart_split_mermaid_style_decls(declaration) {
            let Some((key, value, important)) = parse_style_decl(declaration) else {
                continue;
            };
            apply_text_style_decl_with_provenance(
                &mut resolution.style,
                key,
                value,
                &mut resolution.prepared_text_overrides,
                &mut resolution.terminal_foreground,
                FlowchartTerminalForegroundProvenance::EdgeLabelStyle,
                important,
            );
        }
    }
    resolution
}

/// Mermaid's Swimlane adapter moves an edge label onto a fresh `labelRect` node and copies only
/// the first entry from the already-concatenated default/edge `labelStyle` array. Classes and later
/// style entries remain on the original edge and must not affect the label node's measurement.
#[cfg(test)]
pub(crate) fn flowchart_swimlane_label_rect_text_style<'a>(
    base: &'a TextStyle,
    default_edge_styles: &[String],
    edge_styles: &[String],
) -> std::borrow::Cow<'a, TextStyle> {
    flowchart_swimlane_label_rect_text_style_with_provenance(base, default_edge_styles, edge_styles)
        .style
}

#[cfg(test)]
pub(crate) fn flowchart_swimlane_label_rect_text_style_with_provenance<'a>(
    base: &'a TextStyle,
    default_edge_styles: &[String],
    edge_styles: &[String],
) -> FlowchartTextStyleResolution<'a> {
    let Some(first_style) = default_edge_styles.first().or_else(|| edge_styles.first()) else {
        return FlowchartTextStyleResolution {
            style: Cow::Borrowed(base),
            prepared_text_overrides: PreparedTextCssTypographyOverrides::default(),
            terminal_foreground: None,
        };
    };

    let mut style = Cow::Borrowed(base);
    let mut prepared_text_overrides = PreparedTextCssTypographyOverrides::default();
    let mut terminal_foreground = None;
    for declaration in flowchart_split_mermaid_style_decls(first_style) {
        let Some((key, value, important)) = parse_style_decl(declaration) else {
            continue;
        };
        apply_text_style_decl_with_provenance(
            &mut style,
            key,
            value,
            &mut prepared_text_overrides,
            &mut terminal_foreground,
            FlowchartTerminalForegroundProvenance::EdgeLabelStyle,
            important,
        );
    }
    FlowchartTextStyleResolution {
        style,
        prepared_text_overrides,
        terminal_foreground,
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct CssBoxEdges {
    top: f64,
    right: f64,
    bottom: f64,
    left: f64,
}

impl CssBoxEdges {
    fn set_shorthand(&mut self, values: &[f64]) {
        match values {
            [all] => {
                self.top = *all;
                self.right = *all;
                self.bottom = *all;
                self.left = *all;
            }
            [vertical, horizontal] => {
                self.top = *vertical;
                self.right = *horizontal;
                self.bottom = *vertical;
                self.left = *horizontal;
            }
            [top, horizontal, bottom] => {
                self.top = *top;
                self.right = *horizontal;
                self.bottom = *bottom;
                self.left = *horizontal;
            }
            [top, right, bottom, left] => {
                self.top = *top;
                self.right = *right;
                self.bottom = *bottom;
                self.left = *left;
            }
            _ => {}
        }
    }

    fn horizontal(self) -> f64 {
        self.left + self.right
    }

    fn vertical(self) -> f64 {
        self.top + self.bottom
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum HtmlSpanDisplay {
    #[default]
    Inline,
    InlineBlock,
    Other,
}

#[derive(Debug, Clone, Copy)]
struct HtmlSpanBoxStyle {
    display: HtmlSpanDisplay,
    margin: CssBoxEdges,
    padding: CssBoxEdges,
    border_width: CssBoxEdges,
    border_visible: [bool; 4],
    line_height: f64,
}

impl HtmlSpanBoxStyle {
    fn new(font_size: f64) -> Self {
        Self {
            display: HtmlSpanDisplay::Inline,
            margin: CssBoxEdges::default(),
            padding: CssBoxEdges::default(),
            // CSS initializes border width to `medium`, but a `none` border style makes its
            // used width zero until a declaration enables it.
            border_width: CssBoxEdges {
                top: 3.0,
                right: 3.0,
                bottom: 3.0,
                left: 3.0,
            },
            border_visible: [false; 4],
            line_height: font_size.max(1.0) * 1.5,
        }
    }

    fn visible_border_width(self) -> CssBoxEdges {
        CssBoxEdges {
            top: if self.border_visible[0] {
                self.border_width.top
            } else {
                0.0
            },
            right: if self.border_visible[1] {
                self.border_width.right
            } else {
                0.0
            },
            bottom: if self.border_visible[2] {
                self.border_width.bottom
            } else {
                0.0
            },
            left: if self.border_visible[3] {
                self.border_width.left
            } else {
                0.0
            },
        }
    }
}

fn css_box_length_px(raw: &str, font_size: f64, allow_negative: bool) -> Option<f64> {
    let raw = raw.trim().trim_end_matches("!important").trim();
    if raw.eq_ignore_ascii_case("auto") {
        return Some(0.0);
    }
    let lower = raw.to_ascii_lowercase();
    let value = if let Some(value) = lower.strip_suffix("px") {
        value.trim().parse::<f64>().ok()?
    } else if let Some(value) = lower.strip_suffix("rem") {
        value.trim().parse::<f64>().ok()? * font_size
    } else if let Some(value) = lower.strip_suffix("em") {
        value.trim().parse::<f64>().ok()? * font_size
    } else if lower == "0" || lower == "+0" || lower == "-0" {
        0.0
    } else {
        return None;
    };
    if !value.is_finite() || (!allow_negative && value < 0.0) {
        return None;
    }
    Some(value)
}

fn css_box_shorthand_px(raw: &str, font_size: f64, allow_negative: bool) -> Option<Vec<f64>> {
    let values = raw
        .split_ascii_whitespace()
        .map(|value| css_box_length_px(value, font_size, allow_negative))
        .collect::<Option<Vec<_>>>()?;
    (1..=4).contains(&values.len()).then_some(values)
}

fn css_border_width_px(raw: &str, font_size: f64) -> Option<f64> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "thin" => Some(1.0),
        "medium" => Some(3.0),
        "thick" => Some(5.0),
        _ => css_box_length_px(raw, font_size, false),
    }
}

fn css_border_style_visible(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "none" | "hidden" => Some(false),
        "dotted" | "dashed" | "solid" | "double" | "groove" | "ridge" | "inset" | "outset" => {
            Some(true)
        }
        _ => None,
    }
}

fn css_border_shorthand(raw: &str, font_size: f64) -> (f64, bool) {
    let mut width = 3.0;
    let mut visible = false;
    for token in raw.split_ascii_whitespace() {
        if let Some(value) = css_border_width_px(token, font_size) {
            width = value;
        }
        if let Some(value) = css_border_style_visible(token) {
            visible = value;
        }
    }
    (width, visible)
}

fn set_border_width_side(edges: &mut CssBoxEdges, side: usize, width: f64) {
    match side {
        0 => edges.top = width,
        1 => edges.right = width,
        2 => edges.bottom = width,
        3 => edges.left = width,
        _ => unreachable!("CSS box side index"),
    }
}

fn apply_html_span_box_decl(
    style: &mut HtmlSpanBoxStyle,
    property: &str,
    value: &str,
    font_size: f64,
) {
    let property = property.trim().to_ascii_lowercase();
    match property.as_str() {
        "display" => {
            style.display = match value.trim().to_ascii_lowercase().as_str() {
                "inline" => HtmlSpanDisplay::Inline,
                "inline-block" => HtmlSpanDisplay::InlineBlock,
                _ => HtmlSpanDisplay::Other,
            };
        }
        "margin" => {
            if let Some(values) = css_box_shorthand_px(value, font_size, true) {
                style.margin.set_shorthand(&values);
            }
        }
        "padding" => {
            if let Some(values) = css_box_shorthand_px(value, font_size, false) {
                style.padding.set_shorthand(&values);
            }
        }
        "margin-top" | "margin-right" | "margin-bottom" | "margin-left" | "padding-top"
        | "padding-right" | "padding-bottom" | "padding-left" => {
            let is_margin = property.starts_with("margin-");
            let allow_negative = is_margin;
            let Some(length) = css_box_length_px(value, font_size, allow_negative) else {
                return;
            };
            let edges = if is_margin {
                &mut style.margin
            } else {
                &mut style.padding
            };
            match property.rsplit('-').next() {
                Some("top") => edges.top = length,
                Some("right") => edges.right = length,
                Some("bottom") => edges.bottom = length,
                Some("left") => edges.left = length,
                _ => unreachable!("matched CSS box side"),
            }
        }
        "border" => {
            let (width, visible) = css_border_shorthand(value, font_size);
            style.border_width.set_shorthand(&[width]);
            style.border_visible.fill(visible);
        }
        "border-width" => {
            let values = value
                .split_ascii_whitespace()
                .map(|token| css_border_width_px(token, font_size))
                .collect::<Option<Vec<_>>>();
            if let Some(values) = values.filter(|values| (1..=4).contains(&values.len())) {
                style.border_width.set_shorthand(&values);
            }
        }
        "border-style" => {
            let values = value
                .split_ascii_whitespace()
                .map(css_border_style_visible)
                .collect::<Option<Vec<_>>>();
            if let Some(values) = values.filter(|values| (1..=4).contains(&values.len())) {
                let mut expanded = CssBoxEdges::default();
                expanded.set_shorthand(
                    &values
                        .iter()
                        .map(|visible| f64::from(*visible))
                        .collect::<Vec<_>>(),
                );
                style.border_visible = [
                    expanded.top != 0.0,
                    expanded.right != 0.0,
                    expanded.bottom != 0.0,
                    expanded.left != 0.0,
                ];
            }
        }
        "border-top" | "border-right" | "border-bottom" | "border-left" => {
            let side = match property.as_str() {
                "border-top" => 0,
                "border-right" => 1,
                "border-bottom" => 2,
                "border-left" => 3,
                _ => unreachable!("matched CSS border side"),
            };
            let (width, visible) = css_border_shorthand(value, font_size);
            set_border_width_side(&mut style.border_width, side, width);
            style.border_visible[side] = visible;
        }
        "border-top-width" | "border-right-width" | "border-bottom-width" | "border-left-width" => {
            let Some(width) = css_border_width_px(value, font_size) else {
                return;
            };
            let side = match property.as_str() {
                "border-top-width" => 0,
                "border-right-width" => 1,
                "border-bottom-width" => 2,
                "border-left-width" => 3,
                _ => unreachable!("matched CSS border width side"),
            };
            set_border_width_side(&mut style.border_width, side, width);
        }
        "border-top-style" | "border-right-style" | "border-bottom-style" | "border-left-style" => {
            let Some(visible) = css_border_style_visible(value) else {
                return;
            };
            let side = match property.as_str() {
                "border-top-style" => 0,
                "border-right-style" => 1,
                "border-bottom-style" => 2,
                "border-left-style" => 3,
                _ => unreachable!("matched CSS border style side"),
            };
            style.border_visible[side] = visible;
        }
        "line-height" => {
            let value = value.trim().trim_end_matches("!important").trim();
            let parsed = if let Some(percent) = value.strip_suffix('%') {
                percent
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .map(|percent| font_size * percent / 100.0)
            } else if let Ok(factor) = value.parse::<f64>() {
                Some(font_size * factor)
            } else {
                css_box_length_px(value, font_size, false)
            };
            if let Some(line_height) = parsed.filter(|line_height| *line_height > 0.0) {
                style.line_height = line_height;
            }
        }
        _ => {}
    }
}

fn flowchart_html_span_box_style(
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    font_size: f64,
) -> HtmlSpanBoxStyle {
    let effective = flowchart_effective_node_class_names(class_defs, classes);
    let mut style = HtmlSpanBoxStyle::new(font_size);

    // `createCssStyles()` emits one selector per class definition in definition order. All
    // selectors have equal specificity, so stylesheet order, rather than the order in a node's
    // `class` attribute, owns the cascade for the generated `<span>`.
    for (class, declarations) in class_defs {
        if !effective.contains(&class.as_str()) {
            continue;
        }
        for declaration in declarations {
            for declaration in flowchart_split_mermaid_style_decls(declaration) {
                let Some((property, value, _important)) = parse_style_decl(declaration) else {
                    continue;
                };
                apply_html_span_box_decl(&mut style, property, value, font_size);
            }
        }
    }
    style
}

pub(crate) fn flowchart_apply_html_node_class_box_metrics(
    metrics: &mut TextMetrics,
    raw_label: &str,
    label_type: &str,
    text_style: &TextStyle,
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
) {
    if raw_label.is_empty() || class_defs.is_empty() {
        return;
    }

    let has_block_child = if label_type == "markdown" {
        crate::text::mermaid_markdown_wants_paragraph_wrap(raw_label)
    } else {
        // Mermaid's `nonMarkdownToHTML()` wraps every non-empty label in `<p>...</p>`.
        true
    };
    let box_style = flowchart_html_span_box_style(class_defs, classes, text_style.font_size);
    let border = box_style.visible_border_width();
    let horizontal =
        box_style.margin.horizontal() + box_style.padding.horizontal() + border.horizontal();
    let vertical = box_style.margin.vertical() + box_style.padding.vertical() + border.vertical();

    match box_style.display {
        HtmlSpanDisplay::Inline if has_block_child && horizontal.abs() > f64::EPSILON => {
            // An inline span containing a block `<p>` is split into an inline fragment before the
            // block, the block itself, and a fragment after it. Horizontal box edges give both
            // otherwise-empty fragments inline advance, so each occupies one inherited line box.
            metrics.width = metrics.width.max(horizontal.abs());
            metrics.height += 2.0 * box_style.line_height;
            metrics.line_count += 2;
        }
        HtmlSpanDisplay::Inline => {
            metrics.width = (metrics.width + horizontal).max(0.0);
        }
        HtmlSpanDisplay::InlineBlock => {
            metrics.width = (metrics.width + horizontal).max(0.0);
            metrics.height = (metrics.height + vertical).max(0.0);
        }
        HtmlSpanDisplay::Other => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swimlane_label_rect_uses_only_the_first_concatenated_edge_style() {
        let base = TextStyle::default();
        let default_styles = vec![
            "font-size:24px".to_string(),
            "font-weight:bold".to_string(),
            "font-size:20px".to_string(),
        ];
        let edge_styles = vec![
            "font-size:12px".to_string(),
            "font-style:italic".to_string(),
        ];

        let style = flowchart_swimlane_label_rect_text_style(&base, &default_styles, &edge_styles);

        assert_eq!(style.font_size, 24.0);
        assert_eq!(style.font_weight, None);
        assert_eq!(style.font_style, None);
    }

    #[test]
    fn swimlane_label_rect_falls_back_to_the_first_edge_style() {
        let base = TextStyle::default();
        let edge_styles = vec![
            "font-size:18px,font-style:italic".to_string(),
            "font-size:30px".to_string(),
        ];

        let style = flowchart_swimlane_label_rect_text_style(&base, &[], &edge_styles);

        assert_eq!(style.font_size, 18.0);
        assert_eq!(style.font_style.as_deref(), Some("italic"));
    }

    #[test]
    fn edge_label_text_style_applies_class_then_default_then_edge_declarations() {
        let base = TextStyle::default();
        let class_defs = IndexMap::from([(
            "accent".to_string(),
            vec!["font-size:18px,font-style:italic".to_string()],
        )]);
        let classes = vec!["accent".to_string()];
        let default_styles = vec!["font-size:24px,font-weight:bold".to_string()];
        let edge_styles = vec!["font-size:12px,font-style:normal".to_string()];

        let style = flowchart_effective_edge_label_text_style(
            &base,
            &class_defs,
            &classes,
            &default_styles,
            &edge_styles,
        );

        assert_eq!(style.font_size, 12.0);
        assert_eq!(style.font_weight.as_deref(), Some("bold"));
        assert_eq!(style.font_style.as_deref(), Some("normal"));
    }

    #[test]
    fn terminal_foreground_tracks_class_inline_and_edge_label_winners() {
        let base = TextStyle::default();
        let class_defs =
            IndexMap::from([("accent".to_string(), vec!["color:#0f172a".to_string()])]);
        let classes = vec!["accent".to_string()];
        let inline = vec!["color:#f43f5e".to_string()];
        let node = flowchart_effective_text_style_for_classes_with_provenance(
            &base,
            &class_defs,
            &classes,
            &inline,
        );
        let node_foreground = node.terminal_foreground().expect("node color winner");
        assert_eq!(node_foreground.value(), "#f43f5e");
        assert_eq!(
            node_foreground.provenance(),
            FlowchartTerminalForegroundProvenance::InlineStyle
        );

        let edge = flowchart_effective_edge_label_text_style_with_provenance(
            &base,
            &class_defs,
            &classes,
            &["color:#2563eb".to_string()],
            &["color:#16a34a".to_string()],
        );
        let edge_foreground = edge.terminal_foreground().expect("edge color winner");
        assert_eq!(edge_foreground.value(), "#16a34a");
        assert_eq!(
            edge_foreground.provenance(),
            FlowchartTerminalForegroundProvenance::EdgeLabelStyle
        );

        let important_classes = IndexMap::from([(
            "important".to_string(),
            vec!["color:#7c3aed !important".to_string()],
        )]);
        let important = flowchart_effective_text_style_for_classes_with_provenance(
            &base,
            &important_classes,
            &["important".to_string()],
            &["color:#f97316".to_string()],
        );
        let important_foreground = important
            .terminal_foreground()
            .expect("important class color winner");
        assert_eq!(important_foreground.value(), "#7c3aed");
        assert_eq!(
            important_foreground.provenance(),
            FlowchartTerminalForegroundProvenance::AssignedClass
        );
    }

    #[test]
    fn text_style_resolution_preserves_same_value_source_font_provenance() {
        let base = TextStyle {
            font_family: Some("Excalifont".to_string()),
            ..TextStyle::default()
        };
        let class_defs = IndexMap::from([(
            "accent".to_string(),
            vec!["font-family:Excalifont,font-size:18px".to_string()],
        )]);
        let classes = vec!["accent".to_string()];

        let resolution = flowchart_effective_text_style_for_classes_with_provenance(
            &base,
            &class_defs,
            &classes,
            &[],
        );

        assert_eq!(resolution.font_family.as_deref(), Some("Excalifont"));
        assert_eq!(
            resolution.prepared_text_overrides.font_family(),
            Some("Excalifont")
        );
        assert_eq!(resolution.prepared_text_overrides.font_size(), Some("18px"));
    }

    #[test]
    fn text_style_resolution_uses_source_spelled_label_property_identity() {
        let base = TextStyle {
            font_size: 26.0,
            ..TextStyle::default()
        };
        let class_defs = IndexMap::from([(
            "accent".to_string(),
            vec!["FONT-SIZE:22px,Font-Family:Xiaolai SC".to_string()],
        )]);
        let classes = vec!["accent".to_string()];

        let resolution = flowchart_effective_text_style_for_classes_with_provenance(
            &base,
            &class_defs,
            &classes,
            &[],
        );

        assert_eq!(resolution.font_size, 26.0);
        assert_eq!(resolution.font_family, None);
        assert!(resolution.prepared_text_overrides.is_empty());
    }

    #[test]
    fn text_style_resolution_carries_all_measurement_affecting_source_typography() {
        let base = TextStyle::default();
        let class_defs = IndexMap::from([(
            "wide".to_string(),
            vec![
                "line-height:2,letter-spacing:3px,word-spacing:4px,text-transform:uppercase"
                    .to_string(),
            ],
        )]);
        let resolution = flowchart_effective_text_style_for_class_names_with_provenance(
            &base,
            &class_defs,
            ["wide"],
            &[],
        );

        let mut expected = PreparedTextCssTypographyOverrides::default();
        expected.observe_declaration("line-height", "2");
        expected.observe_declaration("letter-spacing", "3px");
        expected.observe_declaration("word-spacing", "4px");
        expected.observe_declaration("text-transform", "uppercase");
        assert_eq!(resolution.prepared_text_overrides, expected);
    }

    #[test]
    fn text_style_resolution_uses_the_semantic_value_before_important() {
        let base = TextStyle::default();
        let class_defs = IndexMap::from([(
            "accent".to_string(),
            vec!["font-family:\"Xiaolai SC\" !important,font-weight:bolder !important".to_string()],
        )]);
        let classes = vec!["accent".to_string()];

        let resolution = flowchart_effective_text_style_for_classes_with_provenance(
            &base,
            &class_defs,
            &classes,
            &[],
        );

        assert_eq!(resolution.font_family.as_deref(), Some("\"Xiaolai SC\""));
        assert_eq!(
            resolution.prepared_text_overrides.font_family(),
            Some("\"Xiaolai SC\"")
        );
        assert_eq!(resolution.font_weight.as_deref(), Some("bolder"));
    }
}
