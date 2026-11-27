//! Flowchart container colors and Agentflow's separate kind-first palette.

use super::*;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt;
use std::ops::Range;

const KINDS: [&str; 7] = [
    "tool",
    "task",
    "decision",
    "input",
    "refdoc",
    "connector",
    "action",
];

pub(super) struct ContainerPalette<'a> {
    borders: &'a [Value],
    backgrounds: &'a [Value],
    look: &'a str,
}

impl<'a> ContainerPalette<'a> {
    pub(super) fn new(config: &'a (impl crate::config::ConfigLookup + ?Sized)) -> Self {
        let variables = config.get("themeVariables");
        let colors = |key: &str| {
            variables
                .and_then(|value| value.get(key))
                .and_then(Value::as_array)
                .map_or(&[][..], Vec::as_slice)
        };
        let borders = if matches!(
            config.get("theme").and_then(Value::as_str),
            Some("redux-color" | "redux-dark-color")
        ) {
            colors("borderColorArray")
        } else {
            &[]
        };
        let look = config
            .get("look")
            .and_then(Value::as_str)
            .filter(|look| {
                !look.is_empty()
                    && look
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            })
            .unwrap_or("classic");
        Self {
            borders,
            backgrounds: colors("bkgColorArray"),
            look,
        }
    }

    pub(super) fn color_count(&self) -> usize {
        self.borders.len()
    }

    pub(super) fn background_color(&self, slot: usize) -> Option<&str> {
        slot.checked_rem(self.backgrounds.len())
            .and_then(|index| self.backgrounds[index].as_str())
    }

    pub(super) fn without_theme_colors(mut self) -> Self {
        self.borders = &[];
        self.backgrounds = &[];
        self
    }
}

fn container_slot(ordinal: usize, palette_len: usize) -> usize {
    (KINDS.len() + ordinal % palette_len.saturating_sub(KINDS.len()).max(1)) % palette_len
}

pub(super) fn container_color_slot(ctx: &FlowchartRenderCtx<'_>, id: &str) -> Option<usize> {
    let count = ctx.palette.color_count();
    let ordinal = ctx.model.subgraph_color_ordinal(id)?;
    (count > 0).then(|| {
        if ctx.diagram_type == "agentflow" {
            container_slot(ordinal, count)
        } else {
            ordinal % count
        }
    })
}

pub(super) fn write_css(
    out: &mut impl SvgOutput,
    diagram_id: impl std::fmt::Display + Copy,
    palette: &ContainerPalette<'_>,
    defer_palette: bool,
) -> Result<Option<usize>> {
    let _ = write!(
        out,
        "#{diagram_id} .flow-cluster rect{{fill:none;stroke-width:0.75px;}}"
    );
    out.checkpoint()?;
    if palette.borders.is_empty() {
        return Ok(None);
    }
    let look = palette.look;
    for (slot, kind) in KINDS.iter().enumerate() {
        out.checkpoint()?;
        let selector = format!(r##"#{diagram_id} [data-look="{look}"].node.af-kind-{kind}"##);
        let _ = write!(
            out,
            "{selector} rect,{selector} path,{selector} polygon{{{}}}",
            agentflow_declarations(palette, slot)
        );
    }
    if defer_palette {
        return Ok(Some(out.len()));
    }
    write_agentflow_container_css(out, diagram_id, palette, None)?;
    Ok(None)
}

fn agentflow_declarations(palette: &ContainerPalette<'_>, slot: usize) -> String {
    let borders = palette.borders;
    let backgrounds = palette.backgrounds;
    let color_value = |value: &serde_json::Value| {
        value
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| value.to_string())
    };
    let border = color_value(&borders[slot % borders.len()]);
    let fill = if backgrounds.is_empty() {
        String::new()
    } else {
        format!(
            "fill:{};",
            color_value(&backgrounds[slot % backgrounds.len()])
        )
    };
    format!("stroke:{border};{fill}")
}

pub(super) fn write_agentflow_container_css(
    out: &mut impl SvgOutput,
    diagram_id: impl std::fmt::Display + Copy,
    palette: &ContainerPalette<'_>,
    used: Option<&BTreeSet<usize>>,
) -> Result<()> {
    let borders = palette.borders;
    let look = palette.look;
    for ordinal in 0..borders.len().saturating_sub(KINDS.len()).max(1) {
        let slot = container_slot(ordinal, borders.len());
        if used.is_some_and(|used| !used.contains(&slot)) {
            continue;
        }
        out.checkpoint()?;
        let expanded = format!(
            r##"#{diagram_id} [data-look="{look}"][data-color-id="color-{slot}"].cluster"##
        );
        let collapsed =
            format!(r##"#{diagram_id} [data-look="{look}"][data-color-id="color-{slot}"].node"##);
        let _ = write!(
            out,
            "{expanded} rect,{collapsed} rect,{expanded} path,{collapsed} path{{{}}}",
            agentflow_declarations(palette, slot)
        );
    }
    out.checkpoint()
}

/// Port of flowchart/styles.ts genColor. Shape attributes are stamped using this same
/// border palette length; inline author styles retain their normal CSS precedence.
pub(super) fn write_flowchart_container_css(
    out: &mut impl SvgOutput,
    diagram_id: impl std::fmt::Display + Copy,
    palette: &ContainerPalette<'_>,
    slots: impl IntoIterator<Item = usize, IntoIter: ExactSizeIterator>,
) -> Result<()> {
    let look = palette.look;
    let backgrounds = (!palette.backgrounds.is_empty()).then_some(palette.backgrounds);
    let slots = slots.into_iter();
    let remaining_slots = slots.len().saturating_sub(1);
    let mut prefix = String::new();
    let mut fragments = String::new();
    for (index, slot) in slots.enumerate() {
        out.checkpoint()?;
        let border = container_palette_value(&palette.borders[slot]);
        let background =
            backgrounds.map(|values| container_palette_value(&values[slot % values.len()]));
        fragments.clear();
        let fill = background
            .as_ref()
            .map(|value| append_container_fragment(&mut fragments, format_args!("fill:{value};")))
            .unwrap_or_default();
        prefix.clear();
        let _ = write!(
            prefix,
            r##"#{diagram_id} [data-look="{look}"][data-color-id="color-{slot}"]"##
        );
        let collapsed = |fragments: &mut String, suffix: &str| {
            append_container_fragment(
                fragments,
                format_args!("{prefix}.node {suffix},{prefix}.rough-node {suffix}"),
            )
        };
        let lane = |fragments: &mut String, suffix: &str| {
            append_container_fragment(
                fragments,
                format_args!(
                    "{prefix}.swimlane.cluster .swimlane-title{suffix},{prefix}.swimlane.cluster .swimlane-body{suffix}"
                ),
            )
        };
        let lane_outline = lane(&mut fragments, " path:nth-of-type(2)");
        let _ = write!(
            out,
            "{prefix}.cluster:not(.swimlane) rect{{stroke:{border};{fill}}}{prefix}.cluster:not(.swimlane) path{{stroke:{border};{fill}}}{prefix}.swimlane.cluster rect.swimlane-title,{prefix}.swimlane.cluster rect.swimlane-body{{stroke:{border};{fill}}}{}{{stroke:{border};}}",
            &fragments[lane_outline],
            fill = &fragments[fill.clone()],
        );
        if let Some(background) = background {
            let lane_fill = lane(&mut fragments, " path:first-of-type");
            let _ = write!(out, "{}{{stroke:{background};}}", &fragments[lane_fill]);
        }
        let group = collapsed(&mut fragments, ".collapsed-group");
        let group_path = collapsed(&mut fragments, ".collapsed-group path");
        let indicator = collapsed(&mut fragments, ".collapsed-indicator");
        let separator = collapsed(&mut fragments, ".collapsed-separator");
        let selector_bytes = fragments.len() - fill.len();
        let written = write!(
            out,
            "{},{}{{stroke:{border};{fill}}}{}{{fill:{border};}}{}{{stroke:{border};}}",
            &fragments[group],
            &fragments[group_path],
            &fragments[indicator],
            &fragments[separator],
            fill = &fragments[fill],
        );
        if index == 0 && remaining_slots != 0 && written.is_ok() {
            // Four selectors use the prefix directly. The fragments contain the other selectors
            // after the fill declaration. Callers visit slots in ascending order.
            let per_slot = prefix
                .len()
                .saturating_mul(4)
                .saturating_add(selector_bytes);
            out.hint_additional_capacity(per_slot.saturating_mul(remaining_slots));
        }
    }
    out.checkpoint()
}

/// Keep opaque compatibility declarations and resource-bearing selectors on the full path.
pub(super) fn can_defer_palette(ctx: &FlowchartRenderCtx<'_>) -> Result<bool> {
    let palette = &ctx.palette;
    if palette.borders.is_empty() {
        return Ok(false);
    }
    for value in palette.borders.iter().chain(palette.backgrounds) {
        ctx.checkpoint_emit()?;
        let Some(value) = value.as_str() else {
            return Ok(false);
        };
        if value.contains(['\'', '"', '\\'])
            || !crate::mermaid_style::is_resource_free_css_value(value)
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The body contains the actual node and lane wrappers, plus any sanitized label markup.
/// Inspect attributes only when the document has a possible palette participant. If an opaque
/// fragment cannot be read as XML, keep the full palette and let the selected pipeline admit it.
pub(super) fn emitted_palette_slots(
    svg: &str,
    body_start: usize,
    palette_len: usize,
    ctx: &FlowchartRenderCtx<'_>,
) -> Result<Option<BTreeSet<usize>>> {
    const ATTRIBUTE: &str = "data-color-id";
    const SCAN_CHUNK: usize = 8_192;
    let bytes = &svg.as_bytes()[body_start..];
    let finder = memchr::memmem::Finder::new(ATTRIBUTE.as_bytes());
    let mut found = false;
    for start in (0..bytes.len()).step_by(SCAN_CHUNK) {
        ctx.checkpoint_emit()?;
        let end = start
            .saturating_add(SCAN_CHUNK + ATTRIBUTE.len() - 1)
            .min(bytes.len());
        if finder.find(&bytes[start..end]).is_some() {
            found = true;
            break;
        }
    }
    let mut slots = BTreeSet::new();
    if !found {
        return Ok(Some(slots));
    }
    let mut reader = quick_xml::Reader::from_str(svg);
    loop {
        ctx.checkpoint_emit()?;
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(tag) | quick_xml::events::Event::Empty(tag)) => {
                if finder.find(tag.as_ref().as_bytes()).is_none() {
                    continue;
                }
                let mut attributes = tag.attributes();
                for attribute in attributes.with_checks(false) {
                    let Ok(attribute) = attribute else {
                        return Ok(None);
                    };
                    if attribute.key.as_ref() == ATTRIBUTE {
                        let Ok(value) = quick_xml::escape::unescape(attribute.value.as_ref())
                        else {
                            return Ok(None);
                        };
                        if let Some(slot) = value
                            .strip_prefix("color-")
                            .and_then(|slot| slot.parse::<usize>().ok())
                            .filter(|slot| *slot < palette_len)
                        {
                            slots.insert(slot);
                        }
                    }
                }
            }
            Ok(quick_xml::events::Event::Eof) => return Ok(Some(slots)),
            Err(_) => return Ok(None),
            Ok(_) => {}
        }
    }
}

fn container_palette_value(value: &serde_json::Value) -> Cow<'_, str> {
    value.as_str().map_or_else(
        || Cow::Owned(value.to_string()),
        |string| Cow::Borrowed(string.trim()),
    )
}

fn append_container_fragment(buffer: &mut String, arguments: fmt::Arguments<'_>) -> Range<usize> {
    let start = buffer.len();
    let _ = buffer.write_fmt(arguments);
    start..buffer.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn palette_borrows_opaque_values_without_reordering_slots() {
        let config = json!({
            "theme": "redux-color", "look": "neo",
            "themeVariables": {"borderColorArray": [null, " blue "], "bkgColorArray": [12]}
        });
        let palette = ContainerPalette::new(&config);
        assert_eq!(palette.borders, [Value::Null, json!(" blue ")]);
        assert_eq!(palette.backgrounds, [json!(12)]);
        assert_eq!(palette.look, "neo");
        assert!(std::ptr::eq(
            &palette.borders[1],
            &config["themeVariables"]["borderColorArray"][1]
        ));
        assert!(std::ptr::eq(
            &palette.backgrounds[0],
            &config["themeVariables"]["bkgColorArray"][0]
        ));
    }
}
