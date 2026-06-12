use crate::Result;
use crate::entities::decode_entities_minimal;
use crate::svg::foreign_object_label_fallback_svg_text;
use std::borrow::Cow;
use std::collections::HashSet;

use super::util::{extract_quoted_attr, find_tag_end};
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessor};

#[derive(Debug, Clone, Copy, Default)]
pub struct ForeignObjectFallbackPostprocessor;

impl SvgPostprocessor for ForeignObjectFallbackPostprocessor {
    fn name(&self) -> &'static str {
        "foreign-object-fallback"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        _ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        if !svg.contains("<foreignObject") {
            return Ok(svg);
        }
        Ok(Cow::Owned(foreign_object_fallback_svg(&svg)))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StripForeignObjectPostprocessor;

impl SvgPostprocessor for StripForeignObjectPostprocessor {
    fn name(&self) -> &'static str {
        "strip-foreign-object"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        _ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        if !svg.contains("<foreignObject") {
            return Ok(svg);
        }
        Ok(Cow::Owned(strip_foreign_objects(&svg)))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DropNativeDuplicateFallbacksPostprocessor;

impl SvgPostprocessor for DropNativeDuplicateFallbacksPostprocessor {
    fn name(&self) -> &'static str {
        "drop-native-duplicate-fallbacks"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        _ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        if !svg.contains(r#"data-merman-foreignobject="fallback""#) {
            return Ok(svg);
        }
        Ok(Cow::Owned(drop_native_duplicate_fallbacks(&svg)))
    }
}

pub(crate) fn foreign_object_fallback_svg(svg: &str) -> String {
    foreign_object_label_fallback_svg_text(svg)
}

pub(crate) fn strip_foreign_objects(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut cursor = 0;

    while let Some(rel_start) = svg[cursor..].find("<foreignObject") {
        let start = cursor + rel_start;

        // Check if this <foreignObject> is inside a <switch> element.
        // If so, unwrap the <switch>: remove <switch>, remove <foreignObject>, keep remaining
        // children (the <text> fallback).
        let before = &svg[cursor..start];
        let switch_handled = if let Some(switch_rel) = before.rfind("<switch>") {
            let switch_start = cursor + switch_rel;
            // Verify there's nothing meaningful between <switch> and <foreignObject>
            let between = svg[switch_start + "<switch>".len()..start].trim();
            if between.is_empty() {
                // Find the closing </switch>
                if let Some(switch_close_rel) = svg[start..].find("</switch>") {
                    let switch_close_end = start + switch_close_rel + "</switch>".len();
                    // Output everything before <switch>
                    out.push_str(&svg[cursor..switch_start]);
                    // Find end of </foreignObject> inside the switch
                    let fo_open_end = find_tag_end(svg, start);
                    if let Some(fo_open_end) = fo_open_end {
                        if !svg[start..=fo_open_end].trim_end().ends_with("/>") {
                            let fo_close_start = fo_open_end + 1;
                            if let Some(fo_close_rel) =
                                svg[fo_close_start..].find("</foreignObject>")
                            {
                                let after_fo =
                                    fo_close_start + fo_close_rel + "</foreignObject>".len();
                                // Keep everything between </foreignObject> and </switch>
                                let inner_content =
                                    &svg[after_fo..start + switch_close_rel];
                                out.push_str(inner_content);
                            }
                        }
                    }
                    cursor = switch_close_end;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        if switch_handled {
            continue;
        }

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

pub fn drop_native_duplicate_fallbacks(svg: &str) -> String {
    let native_text = collect_native_text_contents(svg);
    if native_text.is_empty() {
        return svg.to_string();
    }

    let mut out = String::with_capacity(svg.len());
    let mut cursor = 0;
    while let Some(rel_start) = svg[cursor..].find(r#"data-merman-foreignobject="fallback""#) {
        let attr_start = cursor + rel_start;
        let Some(group_start) = svg[..attr_start].rfind("<g") else {
            out.push_str(&svg[cursor..attr_start]);
            cursor = attr_start;
            continue;
        };
        if group_start < cursor {
            out.push_str(&svg[cursor..attr_start]);
            cursor = attr_start;
            continue;
        }
        let Some((close_start, group_end)) = find_matching_g_end(svg, group_start) else {
            out.push_str(&svg[cursor..attr_start]);
            cursor = attr_start;
            continue;
        };
        let Some(open_end) = find_tag_end(svg, group_start) else {
            out.push_str(&svg[cursor..attr_start]);
            cursor = attr_start;
            continue;
        };

        let fallback_text = normalize_text_content(&svg[open_end + 1..close_start]);
        if native_text.contains(fallback_text.trim()) {
            out.push_str(&svg[cursor..group_start]);
        } else {
            out.push_str(&svg[cursor..group_end]);
        }
        cursor = group_end;
    }

    out.push_str(&svg[cursor..]);
    out
}

fn collect_native_text_contents(svg: &str) -> HashSet<String> {
    let mut contents = HashSet::new();
    let mut cursor = 0;
    while let Some(rel_start) = svg[cursor..].find("<text") {
        let start = cursor + rel_start;
        let Some(open_end) = find_tag_end(svg, start) else {
            break;
        };
        let tag = &svg[start..=open_end];
        if text_tag_is_fallback(tag) || tag.trim_end().ends_with("/>") {
            cursor = open_end + 1;
            continue;
        }

        let close_start = open_end + 1;
        let Some(rel_close) = svg[close_start..].find("</text>") else {
            cursor = open_end + 1;
            continue;
        };
        let close = close_start + rel_close;
        let text = normalize_text_content(&svg[close_start..close]);
        if !text.is_empty() {
            contents.insert(text);
        }
        cursor = close + "</text>".len();
    }
    contents
}

fn text_tag_is_fallback(tag: &str) -> bool {
    extract_quoted_attr(tag, "class").is_some_and(|classes| {
        classes
            .split_whitespace()
            .any(|class| class == "merman-foreignobject-fallback-text")
    })
}

fn normalize_text_content(fragment: &str) -> String {
    decode_entities_minimal(&strip_tags(fragment))
        .trim()
        .to_string()
}

fn strip_tags(fragment: &str) -> String {
    let mut out = String::with_capacity(fragment.len());
    let mut in_tag = false;
    for ch in fragment.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn find_matching_g_end(svg: &str, group_start: usize) -> Option<(usize, usize)> {
    let open_end = find_tag_end(svg, group_start)?;
    if svg[group_start..=open_end].trim_end().ends_with("/>") {
        return Some((group_start, open_end + 1));
    }

    let mut depth = 1usize;
    let mut cursor = open_end + 1;
    while let Some(rel_tag) = svg[cursor..].find('<') {
        let tag_start = cursor + rel_tag;
        let Some(tag_end) = find_tag_end(svg, tag_start) else {
            break;
        };
        let tag = &svg[tag_start..=tag_end];
        if is_start_g_tag(tag) {
            if !tag.trim_end().ends_with("/>") {
                depth += 1;
            }
        } else if is_end_g_tag(tag) {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some((tag_start, tag_end + 1));
            }
        }
        cursor = tag_end + 1;
    }
    None
}

fn is_start_g_tag(tag: &str) -> bool {
    let bytes = tag.as_bytes();
    tag.starts_with("<g")
        && bytes
            .get(2)
            .is_some_and(|b| b.is_ascii_whitespace() || *b == b'>' || *b == b'/')
}

fn is_end_g_tag(tag: &str) -> bool {
    let bytes = tag.as_bytes();
    tag.starts_with("</g")
        && bytes
            .get(3)
            .is_some_and(|b| b.is_ascii_whitespace() || *b == b'>')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::pipeline::SvgPipeline;

    #[test]
    fn drop_native_duplicate_fallbacks_removes_only_matching_fallback_groups() {
        let svg = r##"<svg>
<text class="task">Make tea</text>
<g data-merman-foreignobject="fallback" class="dup">
  <rect/>
  <text class="merman-foreignobject-fallback-text">Make tea</text>
</g>
<g data-merman-foreignobject="fallback" class="keep">
  <text class="merman-foreignobject-fallback-text">Only fallback</text>
</g>
</svg>"##;

        let out = drop_native_duplicate_fallbacks(svg);

        assert!(out.contains(r#"<text class="task">Make tea</text>"#));
        assert!(!out.contains(r#"class="dup""#));
        assert!(out.contains(r#"class="keep""#));
        assert!(out.contains("Only fallback"));
    }

    #[test]
    fn fallback_text_class_scanner_handles_single_quoted_attrs() {
        assert!(text_tag_is_fallback(
            r#"<text class = 'label merman-foreignobject-fallback-text'>"#
        ));
        assert!(!text_tag_is_fallback(r#"<text class = 'label task'>"#));
    }

    #[test]
    fn strip_foreign_objects_unwraps_switch_keeping_text_fallback() {
        let svg = r##"<svg><switch><foreignObject x="10" y="20" width="100" height="50"><div xmlns="http://www.w3.org/1999/xhtml">Make tea</div></foreignObject><text x="60" y="45">Make tea</text></switch></svg>"##;
        let out = strip_foreign_objects(svg);

        assert!(!out.contains("<foreignObject"), "foreignObject should be stripped: {out}");
        assert!(!out.contains("<switch>"), "switch wrapper should be removed: {out}");
        assert!(!out.contains("</switch>"), "switch closing tag should be removed: {out}");
        assert!(
            out.contains(r#"<text x="60" y="45">Make tea</text>"#),
            "text fallback should be preserved: {out}"
        );
    }

    #[test]
    fn strip_foreign_objects_handles_switch_with_multiple_text_elements() {
        let svg = r##"<svg><switch><foreignObject x="0" y="0" width="80" height="40"><div xmlns="http://www.w3.org/1999/xhtml">Line 1</div></foreignObject><text x="40" y="15">Line 1</text><text x="40" y="30">Line 2</text></switch></svg>"##;
        let out = strip_foreign_objects(svg);

        assert!(!out.contains("<foreignObject"), "{out}");
        assert!(!out.contains("<switch>"), "{out}");
        assert!(out.contains(r#"<text x="40" y="15">Line 1</text>"#), "{out}");
        assert!(out.contains(r#"<text x="40" y="30">Line 2</text>"#), "{out}");
    }

    #[test]
    fn resvg_safe_pipeline_preserves_switch_text_fallback() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><switch><foreignObject x="150" y="50" width="550" height="50"><div class="journey-section" xmlns="http://www.w3.org/1999/xhtml" style="display: table; height: 100%; width: 100%;"><div class="label" style="display: table-cell; text-align: center; vertical-align: middle;">Go to work</div></div></foreignObject><text x="425" y="75" fill="#333"><tspan x="425" dy="0">Go to work</tspan></text></switch></svg>"##;
        let out = SvgPipeline::resvg_safe().process_to_string(svg).unwrap();

        assert!(
            !out.contains("<foreignObject"),
            "foreignObject should be stripped: {out}"
        );
        assert!(
            !out.contains("<switch>"),
            "switch should be removed: {out}"
        );
        assert!(
            out.contains("Go to work"),
            "text fallback should survive full pipeline: {out}"
        );
    }

    #[test]
    fn strip_foreign_objects_handles_journey_switch_pattern() {
        let svg = r##"<svg><g><rect class="section-type-0"/><switch><foreignObject x="150" y="50" width="550" height="50"><div class="journey-section section-type-0" xmlns="http://www.w3.org/1999/xhtml" style="display: table; height: 100%; width: 100%;"><div class="label" style="display: table-cell; text-align: center; vertical-align: middle;">Go to work</div></div></foreignObject><text x="425" y="75" fill="#333" class="journey-section section-type-0" style="text-anchor: middle;"><tspan x="425" dy="0">Go to work</tspan></text></switch></g></svg>"##;
        let out = strip_foreign_objects(svg);

        assert!(!out.contains("<foreignObject"), "foreignObject should be stripped: {out}");
        assert!(!out.contains("<switch>"), "switch should be removed: {out}");
        assert!(
            out.contains("Go to work"),
            "section text should be preserved: {out}"
        );
        assert!(
            out.contains(r#"<text x="425" y="75""#),
            "text element should be preserved: {out}"
        );
    }

    #[test]
    fn strip_foreign_objects_still_works_without_switch() {
        let svg = r#"<svg><foreignObject width="80" height="24"><div>Hello</div></foreignObject><text>World</text></svg>"#;
        let out = strip_foreign_objects(svg);

        assert!(!out.contains("<foreignObject"), "{out}");
        assert!(out.contains("<text>World</text>"), "{out}");
    }

    #[test]
    fn resvg_safe_can_optionally_drop_native_duplicate_fallbacks() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
<text class="task">Make tea</text>
<g transform="translate(0,0)">
  <foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>Make tea</p></div></foreignObject>
</g>
<g transform="translate(0,40)">
  <foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>Only fallback</p></div></foreignObject>
</g>
</svg>"##;

        let out = SvgPipeline::resvg_safe()
            .with_postprocessor(DropNativeDuplicateFallbacksPostprocessor)
            .process_to_string(svg)
            .unwrap();

        assert!(!out.contains("<foreignObject"));
        assert_eq!(
            out.matches(r#"data-merman-foreignobject="fallback""#)
                .count(),
            1,
            "{out}"
        );
        assert!(out.contains("Only fallback"));
        assert!(out.contains(r#"<text class="task">Make tea</text>"#));
    }
}
