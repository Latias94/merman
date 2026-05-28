use crate::{Error, Result};
use regex::{Captures, Regex};
use std::borrow::Cow;
use std::fmt;
use std::sync::{Arc, OnceLock};

use super::parity::foreign_object_label_fallback_svg_text;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SvgPipelinePreset {
    #[default]
    Parity,
    Readable,
    ResvgSafe,
}

#[derive(Debug, Clone, Copy)]
pub struct SvgPostprocessContext<'a> {
    preset: SvgPipelinePreset,
    pass_index: usize,
    pass_name: &'a str,
}

impl<'a> SvgPostprocessContext<'a> {
    pub fn preset(&self) -> SvgPipelinePreset {
        self.preset
    }

    pub fn pass_index(&self) -> usize {
        self.pass_index
    }

    pub fn pass_name(&self) -> &'a str {
        self.pass_name
    }
}

pub trait SvgPostprocessor: Send + Sync {
    fn name(&self) -> &'static str;

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>>;
}

#[derive(Clone)]
pub struct SvgPipeline {
    preset: SvgPipelinePreset,
    postprocessors: Vec<Arc<dyn SvgPostprocessor>>,
}

impl fmt::Debug for SvgPipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = self
            .postprocessors
            .iter()
            .map(|pass| pass.name())
            .collect::<Vec<_>>();

        f.debug_struct("SvgPipeline")
            .field("preset", &self.preset)
            .field("postprocessors", &names)
            .finish()
    }
}

impl Default for SvgPipeline {
    fn default() -> Self {
        Self::parity()
    }
}

impl SvgPipeline {
    pub fn parity() -> Self {
        Self::from_preset(SvgPipelinePreset::Parity)
    }

    pub fn readable() -> Self {
        Self::from_preset(SvgPipelinePreset::Readable)
    }

    pub fn resvg_safe() -> Self {
        Self::from_preset(SvgPipelinePreset::ResvgSafe)
    }

    pub fn from_preset(preset: SvgPipelinePreset) -> Self {
        Self {
            preset,
            postprocessors: Vec::new(),
        }
    }

    pub fn preset(&self) -> SvgPipelinePreset {
        self.preset
    }

    pub fn with_postprocessor<P>(mut self, postprocessor: P) -> Self
    where
        P: SvgPostprocessor + 'static,
    {
        self.postprocessors.push(Arc::new(postprocessor));
        self
    }

    pub fn with_shared_postprocessor(mut self, postprocessor: Arc<dyn SvgPostprocessor>) -> Self {
        self.postprocessors.push(postprocessor);
        self
    }

    pub fn push_postprocessor<P>(&mut self, postprocessor: P)
    where
        P: SvgPostprocessor + 'static,
    {
        self.postprocessors.push(Arc::new(postprocessor));
    }

    pub fn process<'a>(&self, svg: &'a str) -> Result<Cow<'a, str>> {
        let mut current = match self.preset {
            SvgPipelinePreset::Parity => Cow::Borrowed(svg),
            SvgPipelinePreset::Readable => Cow::Owned(foreign_object_label_fallback_svg_text(svg)),
            SvgPipelinePreset::ResvgSafe => Cow::Owned(resvg_safe_svg(svg)),
        };

        for (index, postprocessor) in self.postprocessors.iter().enumerate() {
            let ctx = SvgPostprocessContext {
                preset: self.preset,
                pass_index: index,
                pass_name: postprocessor.name(),
            };
            current = postprocessor
                .process(current, &ctx)
                .map_err(|err| Error::svg_postprocess(postprocessor.name(), err.to_string()))?;
        }

        Ok(current)
    }

    pub fn process_to_string(&self, svg: &str) -> Result<String> {
        Ok(self.process(svg)?.into_owned())
    }
}

pub fn resvg_safe_svg(svg: &str) -> String {
    let svg = foreign_object_label_fallback_svg_text(svg);
    let svg = strip_foreign_objects(&svg);
    let svg = sanitize_style_elements(&svg);
    sanitize_element_attributes(&svg)
}

fn strip_foreign_objects(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut cursor = 0;

    while let Some(rel_start) = svg[cursor..].find("<foreignObject") {
        let start = cursor + rel_start;
        out.push_str(&svg[cursor..start]);

        let Some(open_end) = find_tag_end(svg, start) else {
            out.push_str(&svg[start..]);
            return out;
        };

        if svg[start..=open_end].trim_end().ends_with("/>") {
            cursor = open_end + 1;
            continue;
        }

        let close_start = open_end + 1;
        let Some(rel_close) = svg[close_start..].find("</foreignObject>") else {
            cursor = open_end + 1;
            continue;
        };
        cursor = close_start + rel_close + "</foreignObject>".len();
    }

    out.push_str(&svg[cursor..]);
    out
}

fn sanitize_style_elements(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut cursor = 0;

    while let Some(rel_start) = svg[cursor..].find("<style") {
        let start = cursor + rel_start;
        out.push_str(&svg[cursor..start]);

        let Some(open_end) = find_tag_end(svg, start) else {
            out.push_str(&svg[start..]);
            return out;
        };

        let content_start = open_end + 1;
        let Some(rel_close_start) = svg[content_start..].find("</style") else {
            out.push_str(&svg[start..]);
            return out;
        };
        let close_start = content_start + rel_close_start;
        let Some(close_end) = find_tag_end(svg, close_start) else {
            out.push_str(&svg[start..]);
            return out;
        };

        out.push_str(&svg[start..=open_end]);
        out.push_str(&sanitize_css(&svg[content_start..close_start]));
        out.push_str(&svg[close_start..=close_end]);
        cursor = close_end + 1;
    }

    out.push_str(&svg[cursor..]);
    out
}

fn sanitize_css(css: &str) -> String {
    let css = strip_unsupported_css_rules(css);
    let css = strip_animation_declarations(&css);
    strip_css_deg_units(&css)
}

fn strip_unsupported_css_rules(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut cursor = 0;

    while let Some(rel_open) = css[cursor..].find('{') {
        let open = cursor + rel_open;
        let selector = &css[cursor..open];
        let Some(close) = find_matching_brace(css, open) else {
            out.push_str(&css[cursor..]);
            return out;
        };

        let selector_lower = selector.to_ascii_lowercase();
        let unsupported = selector_lower.contains("@keyframes")
            || selector_lower.contains("@-webkit-keyframes")
            || selector_lower.contains(":root");

        if !unsupported {
            out.push_str(&css[cursor..=close]);
        }
        cursor = close + 1;
    }

    out.push_str(&css[cursor..]);
    out
}

fn strip_animation_declarations(css: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)(^|[;{])\s*animation(?:-[a-z-]+)?\s*:[^;}]*;?")
            .expect("valid animation declaration regex")
    });

    re.replace_all(css, |caps: &Captures<'_>| caps[1].to_string())
        .into_owned()
}

fn strip_css_deg_units(css: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE
        .get_or_init(|| Regex::new(r"(?i)(-?\d+(?:\.\d+)?)deg\b").expect("valid CSS degree regex"));

    re.replace_all(css, "$1").into_owned()
}

fn sanitize_element_attributes(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut cursor = 0;

    while let Some(rel_start) = svg[cursor..].find('<') {
        let start = cursor + rel_start;
        out.push_str(&svg[cursor..start]);

        let Some(end) = find_tag_end(svg, start) else {
            out.push_str(&svg[start..]);
            return out;
        };

        let tag = &svg[start..=end];
        out.push_str(&sanitize_tag_attributes(tag));
        cursor = end + 1;
    }

    out.push_str(&svg[cursor..]);
    out
}

fn sanitize_tag_attributes(tag: &str) -> Cow<'_, str> {
    if tag.starts_with("</")
        || tag.starts_with("<!--")
        || tag.starts_with("<!")
        || tag.starts_with("<?")
    {
        return Cow::Borrowed(tag);
    }

    static ATTR_RE: OnceLock<Regex> = OnceLock::new();
    let attr_re = ATTR_RE.get_or_init(|| {
        Regex::new(r#"\s+([A-Za-z_:][-A-Za-z0-9_:.]*)\s*=\s*"([^"]*)""#)
            .expect("valid SVG attribute regex")
    });

    let mut changed = false;
    let result = attr_re
        .replace_all(tag, |caps: &Captures<'_>| {
            let full = &caps[0];
            let name = &caps[1];
            let value = &caps[2];

            if should_drop_attribute(name, value) {
                changed = true;
                return String::new();
            }

            if let Some(value) = normalize_px_attribute(name, value) {
                changed = true;
                return format!(r#" {name}="{value}""#);
            }

            if name.eq_ignore_ascii_case("style") {
                let sanitized = sanitize_style_attribute(value);
                if sanitized.trim().is_empty() {
                    changed = true;
                    return String::new();
                }
                if sanitized != value {
                    changed = true;
                    return format!(r#" style="{sanitized}""#);
                }
            }

            full.to_string()
        })
        .into_owned();

    if changed {
        Cow::Owned(result)
    } else {
        Cow::Borrowed(tag)
    }
}

fn should_drop_attribute(name: &str, value: &str) -> bool {
    if name.eq_ignore_ascii_case("style") {
        return false;
    }

    let normalized = name.to_ascii_lowercase();
    let guarded = matches!(
        normalized.as_str(),
        "fill"
            | "stroke"
            | "width"
            | "height"
            | "x"
            | "y"
            | "x1"
            | "x2"
            | "y1"
            | "y2"
            | "r"
            | "cx"
            | "cy"
            | "rx"
            | "ry"
            | "stroke-width"
            | "transform"
            | "d"
            | "points"
    );

    guarded && is_invalid_svg_value(value)
}

fn normalize_px_attribute(name: &str, value: &str) -> Option<String> {
    let normalized = name.to_ascii_lowercase();
    let guarded = matches!(
        normalized.as_str(),
        "width"
            | "height"
            | "x"
            | "y"
            | "x1"
            | "x2"
            | "y1"
            | "y2"
            | "r"
            | "cx"
            | "cy"
            | "rx"
            | "ry"
            | "stroke-width"
    );
    if !guarded {
        return None;
    }

    let trimmed = value.trim();
    let number = trimmed.strip_suffix("px")?.trim();
    if number.parse::<f64>().is_ok_and(f64::is_finite) {
        Some(number.to_string())
    } else {
        None
    }
}

fn sanitize_style_attribute(value: &str) -> String {
    let mut out = Vec::new();

    for decl in value.split(';') {
        let trimmed = decl.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Some((property, raw_value)) = trimmed.split_once(':') else {
            out.push(strip_css_deg_units(trimmed));
            continue;
        };

        let property = property.trim();
        let value = raw_value.trim();
        if value.is_empty() || is_invalid_svg_value(value) {
            continue;
        }
        if property
            .trim()
            .to_ascii_lowercase()
            .starts_with("animation")
        {
            continue;
        }

        out.push(format!("{property}:{}", strip_css_deg_units(value)));
    }

    out.join(";")
}

fn is_invalid_svg_value(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return true;
    }

    let lower = value.to_ascii_lowercase();
    lower.contains("nan") || lower.contains("undefined") || lower.contains("infinity")
}

fn find_matching_brace(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in text[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn find_tag_end(svg: &str, start: usize) -> Option<usize> {
    let mut quote = None;
    for (offset, ch) in svg[start..].char_indices() {
        match ch {
            '"' | '\'' if quote == Some(ch) => quote = None,
            '"' | '\'' if quote.is_none() => quote = Some(ch),
            '>' if quote.is_none() => return Some(start + offset),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parity_pipeline_preserves_svg_exactly() {
        let svg = r#"<svg><style>@keyframes a{to{opacity:1}}</style><rect width="10"/></svg>"#;
        let out = SvgPipeline::parity().process(svg).unwrap();
        assert!(matches!(out, Cow::Borrowed(_)));
        assert_eq!(out, svg);
    }

    #[test]
    fn readable_pipeline_matches_foreign_object_fallback() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><g transform="translate(10,20)"><foreignObject width="80" height="48"><div xmlns="http://www.w3.org/1999/xhtml"><p>Layer 7\nHTTP</p></div></foreignObject></g></svg>"#;

        let expected = foreign_object_label_fallback_svg_text(svg);
        let out = SvgPipeline::readable().process_to_string(svg).unwrap();

        assert_eq!(out, expected);
        assert!(out.contains(">Layer 7</text>"));
        assert!(out.contains(">HTTP</text>"));
    }

    #[test]
    fn resvg_safe_pipeline_strips_generic_raster_hazards() {
        let svg = r#"<svg id="test" xmlns="http://www.w3.org/2000/svg"><style type="text/css">@keyframes bounce { 0% { transform: scale(1); } 100% { transform: scale(1.1); } } #test :root { --bg: white; } .node rect { animation: dash 1s linear; transform: rotate(45deg); fill: red; }</style><g transform="translate(undefined,NaN)"><foreignObject width="10" height="10"><div xmlns="http://www.w3.org/1999/xhtml"><p>Hello</p></div></foreignObject><rect width="10px" height="" fill="hsl(240, 100%, NaN%)" stroke="" style="fill: ; stroke: #333; transform: rotate(45deg); animation: dash 1s;"/></g></svg>"#;

        let out = SvgPipeline::resvg_safe().process_to_string(svg).unwrap();

        assert!(!out.contains("<foreignObject"));
        assert!(!out.contains("@keyframes"));
        assert!(!out.contains(":root"));
        assert!(!out.contains("animation"));
        assert!(!out.contains("deg"));
        assert!(!out.contains("NaN"));
        assert!(!out.contains("undefined"));
        assert!(!out.contains(r#"height="""#));
        assert!(!out.contains(r#"fill="hsl"#));
        assert!(!out.contains(r#"stroke="""#));
        assert!(out.contains(r#"width="10""#));
        assert!(out.contains("stroke:#333"));
        assert!(out.contains(">Hello</text>"));
    }

    struct AppendPass(&'static str);

    impl SvgPostprocessor for AppendPass {
        fn name(&self) -> &'static str {
            self.0
        }

        fn process<'a>(
            &self,
            svg: Cow<'a, str>,
            ctx: &SvgPostprocessContext<'_>,
        ) -> Result<Cow<'a, str>> {
            Ok(Cow::Owned(format!(
                "{}<!--{}:{}:{:?}-->",
                svg,
                ctx.pass_index(),
                ctx.pass_name(),
                ctx.preset()
            )))
        }
    }

    #[test]
    fn custom_postprocessors_run_after_builtin_preset_in_order() {
        let svg = r#"<svg><foreignObject width="10" height="10"><div><p>Hello</p></div></foreignObject></svg>"#;
        let pipeline = SvgPipeline::readable()
            .with_postprocessor(AppendPass("first"))
            .with_postprocessor(AppendPass("second"));

        let out = pipeline.process_to_string(svg).unwrap();

        let fallback = out.find("data-merman-foreignobject").unwrap();
        let first = out.find("<!--0:first:Readable-->").unwrap();
        let second = out.find("<!--1:second:Readable-->").unwrap();
        assert!(fallback < first);
        assert!(first < second);
    }

    struct ErrorPass;

    impl SvgPostprocessor for ErrorPass {
        fn name(&self) -> &'static str {
            "error-pass"
        }

        fn process<'a>(
            &self,
            _svg: Cow<'a, str>,
            _ctx: &SvgPostprocessContext<'_>,
        ) -> Result<Cow<'a, str>> {
            Err(Error::InvalidModel {
                message: "boom".to_string(),
            })
        }
    }

    #[test]
    fn custom_postprocessor_errors_surface_with_pass_name() {
        let err = SvgPipeline::parity()
            .with_postprocessor(ErrorPass)
            .process_to_string("<svg/>")
            .unwrap_err();

        let message = err.to_string();
        assert!(message.contains("error-pass"));
        assert!(message.contains("boom"));
    }
}
