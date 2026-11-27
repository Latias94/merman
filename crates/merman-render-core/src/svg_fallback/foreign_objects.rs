use super::{MAX_SELECTOR_ANCESTRY_DEPTH, selector_index_limit};
use crate::resources::{OperationWorkMeter, ResourceLimitPhase};
use crate::svg_scan::{
    SvgTagScanner, end_tag_name, find_with_checkpoints, is_non_rendering_container,
    next_svg_quoted_attr_with_checkpoints, start_tag_name,
};
use merman_model::OperationPhase;
use std::{borrow::Cow, collections::HashSet};

const FALLBACK_SOURCE_ATTRIBUTE: &str = "data-merman-foreignobject-source";
const SWITCH_NATIVE_FALLBACK: &str = "switch-native-fallback";

#[derive(Default)]
pub(super) struct ForeignObjectSwitches {
    wrappers: HashSet<usize>,
    pub(super) native_objects: HashSet<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Branch {
    Html,
    Generated,
    Candidate,
    Link,
    Other,
}

#[derive(Default)]
struct SwitchChildren {
    html: Vec<usize>,
    first_retained: Option<(Branch, usize)>,
    conditional: bool,
}

impl SwitchChildren {
    fn child(&mut self, child: &Element<'_>) {
        match child.branch {
            Branch::Html => self.html.push(child.start),
            Branch::Generated if !self.html.is_empty() => {}
            _ if self.first_retained.is_none() => {
                let branch = if child.has_text {
                    child.branch
                } else {
                    Branch::Other
                };
                self.first_retained = Some((branch, child.start));
            }
            _ => {}
        }
    }

    fn native_branch(&self) -> Option<(Branch, usize)> {
        self.first_retained
            .filter(|(branch, _)| matches!(branch, Branch::Candidate | Branch::Link))
    }
}

struct Element<'a> {
    name: &'a str,
    start: usize,
    branch: Branch,
    has_text: bool,
    switch: Option<SwitchChildren>,
}

impl Element<'_> {
    fn child(&mut self, child: &Self) {
        if let Some(switch) = &mut self.switch {
            switch.child(child);
        }
        self.has_text |= child.has_text
            && child.branch != Branch::Generated
            && !is_non_rendering_container(self.name);
    }
}

impl ForeignObjectSwitches {
    /// Recognizes switches whose first retained branch supplies real native text. Comments,
    /// CDATA and the contents of a foreign element cannot supply a native branch.
    pub(super) fn scan<E>(
        svg: &str,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
        mut depth_limit: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, E> {
        let mut result = Self::default();
        if find_with_checkpoints(svg, "<switch", checkpoint)?.is_none() {
            return Ok(result);
        }
        let mut scanner = SvgTagScanner::new(svg);
        let mut elements: Vec<Element<'_>> = Vec::new();
        while let Some(tag) = scanner.next_with_checkpoints(checkpoint)? {
            if let Some(name) = start_tag_name(tag.raw()) {
                let html = name == "foreignObject";
                if html
                    && scanner
                        .matching_end_with_checkpoints(tag, checkpoint)?
                        .is_none()
                {
                    break;
                }
                if elements.is_empty() && name != "switch" {
                    continue;
                }
                let branch = match name {
                    "foreignObject" => Branch::Html,
                    "g" if attribute_is(
                        tag.raw(),
                        "data-merman-foreignobject",
                        "fallback",
                        checkpoint,
                    )? =>
                    {
                        Branch::Generated
                    }
                    "g" | "switch" | "text" => Branch::Candidate,
                    "a" => Branch::Link,
                    _ => Branch::Other,
                };
                let element = Element {
                    name,
                    start: tag.start(),
                    branch,
                    has_text: name == "text",
                    switch: if name == "switch" {
                        Some(SwitchChildren {
                            conditional: switch_has_conditions(tag.raw(), checkpoint)?,
                            ..SwitchChildren::default()
                        })
                    } else {
                        None
                    },
                };
                if html || tag.is_self_closing() {
                    if let Some(parent) = elements.last_mut() {
                        parent.child(&element);
                    }
                } else {
                    if elements.len() == MAX_SELECTOR_ANCESTRY_DEPTH {
                        depth_limit(elements.len() + 1)?;
                        // The infallible fallback helper leaves over-deep switches unresolved.
                        break;
                    }
                    elements.push(element);
                }
            } else if let Some(name) = end_tag_name(tag.raw())
                && !elements.is_empty()
            {
                let Some(mut element) = elements.pop().filter(|element| element.name == name)
                else {
                    break;
                };
                if let Some(switch) = &element.switch {
                    element.has_text = switch.native_branch().is_some();
                }
                if let Some(parent) = elements.last_mut() {
                    parent.child(&element);
                }
                if let Some(switch) = element.switch
                    && let Some(&first_html) = switch.html.first()
                    && let Some((branch, native_start)) = switch.native_branch()
                {
                    if !switch.conditional
                        && branch == Branch::Candidate
                        && first_html < native_start
                    {
                        result.wrappers.extend([element.start, tag.start()]);
                    }
                    result.native_objects.extend(switch.html);
                }
            }
        }
        Ok(result)
    }
}

fn switch_has_conditions<E>(
    tag: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<bool, E> {
    let mut cursor = 0;
    while let Some(attr) = next_svg_quoted_attr_with_checkpoints(tag, cursor, checkpoint)? {
        if matches!(
            &tag[attr.name_start..attr.name_end],
            "requiredFeatures"
                | "requiredExtensions"
                | "systemLanguage"
                | "externalResourcesRequired"
        ) {
            return Ok(true);
        }
        cursor = attr.full_end;
    }
    Ok(false)
}

fn attribute_is<E>(
    tag: &str,
    name: &str,
    value: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<bool, E> {
    let mut cursor = 0;
    while let Some(attr) = next_svg_quoted_attr_with_checkpoints(tag, cursor, checkpoint)? {
        if tag[attr.name_start..attr.name_end] == *name {
            return Ok(tag[attr.value_start..attr.value_end] == *value);
        }
        cursor = attr.full_end;
    }
    Ok(false)
}

/// Removes HTML branches and replaces proven, unconditional switch wrappers with groups.
pub fn strip_foreign_objects<'a>(
    svg: Cow<'a, str>,
    meter: &OperationWorkMeter,
) -> crate::Result<Cow<'a, str>> {
    let mut checkpoint = || {
        meter
            .checkpoint(OperationPhase::Postprocess)
            .map_err(crate::Error::from)
    };
    if find_with_checkpoints(&svg, "<foreignObject", &mut checkpoint)?.is_none() {
        return Ok(svg);
    }
    let mut out = String::new();
    let changed = strip_foreign_objects_with_checkpoints(
        &svg,
        |fragment| append_postprocess(&mut out, fragment, meter),
        &mut checkpoint,
        |depth| {
            Err(selector_index_limit(
                meter,
                depth,
                MAX_SELECTOR_ANCESTRY_DEPTH,
            ))
        },
    )?;
    if changed {
        checkpoint()?;
        Ok(Cow::Owned(out))
    } else {
        Ok(svg)
    }
}

/// Removes generated fallback groups whose switch already carries native text.
pub fn drop_switch_native_fallbacks<'a>(
    svg: Cow<'a, str>,
    meter: &OperationWorkMeter,
) -> crate::Result<Cow<'a, str>> {
    let mut checkpoint = || {
        meter
            .checkpoint(OperationPhase::Postprocess)
            .map_err(crate::Error::from)
    };
    if find_with_checkpoints(&svg, SWITCH_NATIVE_FALLBACK, &mut checkpoint)?.is_none() {
        return Ok(svg);
    }
    let mut scanner = SvgTagScanner::new(&svg);
    let mut out = String::new();
    let mut cursor = 0;
    while let Some(tag) = scanner.next_with_checkpoints(&mut checkpoint)? {
        if start_tag_name(tag.raw()) != Some("g")
            || !attribute_is(
                tag.raw(),
                FALLBACK_SOURCE_ATTRIBUTE,
                SWITCH_NATIVE_FALLBACK,
                &mut checkpoint,
            )?
        {
            continue;
        }
        let Some(close) = scanner.matching_end_with_checkpoints(tag, &mut checkpoint)? else {
            break;
        };
        append_postprocess(&mut out, &svg[cursor..tag.start()], meter)?;
        cursor = close.end();
    }
    if cursor == 0 {
        return Ok(svg);
    }
    append_postprocess(&mut out, &svg[cursor..], meter)?;
    checkpoint()?;
    Ok(Cow::Owned(out))
}

fn strip_foreign_objects_with_checkpoints<E>(
    svg: &str,
    mut append: impl FnMut(&str) -> Result<(), E>,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
    depth_limit: impl FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    let switches = ForeignObjectSwitches::scan(svg, checkpoint, depth_limit)?;
    let mut scanner = SvgTagScanner::new(svg);
    let mut cursor = 0;
    while let Some(tag) = scanner.next_with_checkpoints(checkpoint)? {
        let name = start_tag_name(tag.raw());
        if name == Some("foreignObject") {
            let Some(close) = scanner.matching_end_with_checkpoints(tag, checkpoint)? else {
                break;
            };
            append(&svg[cursor..tag.start()])?;
            cursor = close.end();
        } else if switches.wrappers.contains(&tag.start()) {
            append(&svg[cursor..tag.start()])?;
            // A neutral group retains presentation and transforms without branch selection.
            if name == Some("switch") {
                append("<g")?;
                append(&tag.raw()["<switch".len()..])?;
            } else {
                append("</g")?;
                append(&tag.raw()["</switch".len()..])?;
            }
            cursor = tag.end();
        }
    }
    checkpoint()?;
    if cursor == 0 {
        return Ok(false);
    }
    append(&svg[cursor..])?;
    Ok(true)
}

fn append_postprocess(
    out: &mut String,
    fragment: &str,
    meter: &OperationWorkMeter,
) -> crate::Result<()> {
    // The document emission sink observes Emit. Keep capacity admission and cancellation in
    // Postprocess for these rewrites, and leave the projected-byte ledger untouched.
    let required = out.len().checked_add(fragment.len()).ok_or_else(|| {
        meter.terminate_svg_byte_count_overflow(
            ResourceLimitPhase::SvgPostprocess,
            OperationPhase::Postprocess,
        )
    })?;
    meter.preflight_svg_byte_count(
        required,
        ResourceLimitPhase::SvgPostprocess,
        OperationPhase::Postprocess,
    )?;
    if required > out.capacity() {
        let target = meter.svg_byte_budget().reservation_capacity(
            out.len(),
            out.capacity(),
            fragment.len(),
        )?;
        out.try_reserve_exact(target - out.len())
            .map_err(|_| crate::Error::InvalidModel {
                message: "failed to reserve bounded SVG output".to_owned(),
            })?;
    }
    out.push_str(fragment);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::RenderResourcePolicy;
    use merman_model::OperationControl;
    use std::convert::Infallible;

    #[test]
    fn strip_foreign_object_stage_stops_during_a_long_structural_scan() {
        let control = OperationControl::new().for_phase(OperationPhase::Postprocess);
        let svg = format!(
            "<svg>{}<foreignObject width=\"1\" height=\"1\"/></svg>",
            "x".repeat(16 * 1024)
        );
        let mut checkpoints = 0usize;

        let error = strip_foreign_objects_with_checkpoints(
            &svg,
            |_| Ok(()),
            &mut || {
                checkpoints = checkpoints.saturating_add(1);
                if checkpoints == 5 {
                    control.cancel();
                }
                control.checkpoint().map_err(crate::Error::from)
            },
            |_| Ok(()),
        )
        .unwrap_err();

        let crate::Error::Cancelled(cancelled) = error else {
            panic!("expected structured cancellation");
        };
        assert_eq!(cancelled.phase, OperationPhase::Postprocess);
        assert_eq!(checkpoints, 5);
    }

    #[test]
    fn unchanged_lexical_hits_keep_the_borrowed_document() {
        let svg = "<svg><!-- <foreignObject/> switch-native-fallback --><text>Label</text></svg>";
        let meter = OperationWorkMeter::new(RenderResourcePolicy::default());
        for process in [strip_foreign_objects, drop_switch_native_fallbacks] {
            let result = process(Cow::Borrowed(svg), &meter).unwrap();
            assert!(matches!(result, Cow::Borrowed(source) if source == svg));
        }
    }

    #[test]
    fn text_inside_non_rendering_containers_does_not_replace_html() {
        for container in ["defs", "symbol", "clipPath", "mask", "marker", "pattern"] {
            let svg = format!(
                "<svg><switch><foreignObject/><g><{container}><text>Definition</text></{container}></g></switch></svg>"
            );
            let switches =
                ForeignObjectSwitches::scan(&svg, &mut || Ok::<(), Infallible>(()), |_| Ok(()))
                    .unwrap();
            assert!(switches.native_objects.is_empty(), "{svg}");
            assert!(switches.wrappers.is_empty(), "{svg}");
        }
    }

    #[test]
    fn only_complete_case_sensitive_switch_elements_supply_a_native_branch() {
        let svg = concat!(
            "<svg><switch/><foreignObject/><SWITCH><foreignObject/><text>Uppercase</text></SWITCH>",
            "<?note <switch>?> <!-- <switch> --> <![CDATA[<switch>]]>",
            "<foreignObject/><text>Outside</text>",
            "<switch><foreignObject/><text>Unclosed</text></svg>",
        );
        let switches =
            ForeignObjectSwitches::scan(svg, &mut || Ok::<(), Infallible>(()), |_| Ok(())).unwrap();
        assert!(switches.native_objects.is_empty());
        assert!(switches.wrappers.is_empty());
    }

    #[test]
    fn malformed_switch_tags_do_not_supply_native_fallback_evidence() {
        for (open, close) in [
            ("<switch / >", "</switch>"),
            ("<switch>", "</switch extra>"),
        ] {
            let svg = format!("<svg>{open}<foreignObject/><text>Native</text>{close}</svg>");
            let switches =
                ForeignObjectSwitches::scan(&svg, &mut || Ok::<(), Infallible>(()), |_| Ok(()))
                    .unwrap();
            assert!(switches.native_objects.is_empty(), "{svg}");
            assert!(switches.wrappers.is_empty(), "{svg}");
        }
    }

    #[test]
    fn nested_switch_scan_does_linear_work_across_sibling_foreign_objects() {
        let measure = |count: usize| {
            let svg = format!(
                "<svg><switch>{}<g><switch><text>Native</text></switch></g></switch ></svg>",
                "<foreignObject><!-- <switch> --><div>日程</div></foreignObject>".repeat(count),
            );
            let mut checkpoints = 0;
            let switches = ForeignObjectSwitches::scan(
                &svg,
                &mut || {
                    checkpoints += 1;
                    Ok::<(), Infallible>(())
                },
                |_| Ok(()),
            )
            .unwrap();
            assert_eq!(switches.native_objects.len(), count);
            assert_eq!(switches.wrappers.len(), 2);
            checkpoints
        };
        let small = measure(128);
        assert!(measure(512) <= 4 * small);
    }

    #[test]
    fn switch_scan_can_cancel_inside_an_opaque_foreign_object_span() {
        let svg = format!(
            "<svg><switch><foreignObject><!-- {} --></foreignObject></switch></svg>",
            "表".repeat(2048),
        );
        let mut checkpoints = 0;
        let result = ForeignObjectSwitches::scan(
            &svg,
            &mut || {
                checkpoints += 1;
                if checkpoints == 20 { Err(20) } else { Ok(()) }
            },
            |_| Ok(()),
        );
        assert!(matches!(result, Err(20)));
    }
}
