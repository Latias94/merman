use merman_render_core::svg_scan::{
    SvgTagScanner, end_tag_name, find_with_checkpoints, next_svg_quoted_attr_with_checkpoints,
    start_tag_name,
};
use rustc_hash::FxHashSet;

pub(super) const FALLBACK_SOURCE_ATTRIBUTE: &str = "data-merman-foreignobject-source";
pub(super) const SWITCH_NATIVE_FALLBACK: &str = "switch-native-fallback";

#[derive(Default)]
pub(super) struct ForeignObjectSwitches {
    pub(super) wrappers: FxHashSet<usize>,
    pub(super) native_objects: FxHashSet<usize>,
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
    first_retained: Option<Branch>,
}

impl SwitchChildren {
    fn child(&mut self, child: &Element<'_>) {
        if self.first_retained.is_some() {
            return;
        }
        match child.branch {
            Branch::Html => self.html.push(child.start),
            Branch::Generated if !self.html.is_empty() => {}
            _ => {
                self.first_retained = Some(if child.has_text {
                    child.branch
                } else {
                    Branch::Other
                });
            }
        }
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
        self.has_text |= child.branch != Branch::Generated && child.has_text;
    }
}

impl ForeignObjectSwitches {
    /// Recognizes direct HTML branches followed by a real native text branch. Comments,
    /// CDATA and the contents of a foreign element cannot supply a native branch.
    pub(super) fn scan<E>(
        svg: &str,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
        mut admit_depth: impl FnMut(usize) -> Result<(), E>,
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
                    switch: (name == "switch").then(SwitchChildren::default),
                };
                if html || tag.is_self_closing() {
                    if let Some(parent) = elements.last_mut() {
                        parent.child(&element);
                    }
                } else {
                    admit_depth(elements.len() + 1)?;
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
                    element.has_text = matches!(
                        switch.first_retained,
                        Some(Branch::Candidate | Branch::Link)
                    );
                }
                if let Some(parent) = elements.last_mut() {
                    parent.child(&element);
                }
                if let Some(switch) = element.switch
                    && !switch.html.is_empty()
                    && matches!(
                        switch.first_retained,
                        Some(Branch::Candidate | Branch::Link)
                    )
                {
                    if switch.first_retained == Some(Branch::Candidate) {
                        result.wrappers.extend([element.start, tag.start()]);
                    }
                    result.native_objects.extend(switch.html);
                }
            }
        }
        Ok(result)
    }
}

pub(super) fn attribute_is<E>(
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;

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
