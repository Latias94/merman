use crate::Result;
use crate::environment::TextMeasurementPhase;
use crate::svg::fallback::foreign_object_label_fallback_svg_text_controlled;
use crate::text::TextMeasurer;
use std::borrow::Cow;
#[cfg(test)]
use std::convert::Infallible;

use crate::svg::pipeline::final_validation::SvgStructureMetrics;
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessor};
use merman_render_core::svg_scan::{
    extract_quoted_attr, find_tag_end_with_checkpoints, find_with_checkpoints,
    next_svg_quoted_attr_with_checkpoints, rfind_with_checkpoints,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct ForeignObjectFallbackPostprocessor;

impl SvgPostprocessor for ForeignObjectFallbackPostprocessor {
    fn name(&self) -> &'static str {
        "foreign-object-fallback"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        let measurer = ctx.controlled_text_measurer(TextMeasurementPhase::Wrap);
        let structure =
            crate::svg::pipeline::final_validation::validate_well_formed_svg_with_execution(
                &svg,
                ctx.execution(),
            )?;
        apply_foreign_object_fallback(svg, &measurer, ctx.execution(), structure)
    }
}

pub(crate) fn apply_foreign_object_fallback<'a>(
    svg: Cow<'a, str>,
    text_measurer: &dyn TextMeasurer,
    execution: crate::svg::pipeline::SvgPostprocessExecution<'_>,
    structure: SvgStructureMetrics,
) -> Result<Cow<'a, str>> {
    apply_foreign_object_fallback_with_outcome(svg, text_measurer, execution, structure)
        .map(|(svg, _changed)| svg)
}

pub(crate) fn apply_foreign_object_fallback_with_outcome<'a>(
    svg: Cow<'a, str>,
    text_measurer: &dyn TextMeasurer,
    execution: crate::svg::pipeline::SvgPostprocessExecution<'_>,
    structure: SvgStructureMetrics,
) -> Result<(Cow<'a, str>, bool)> {
    execution.checkpoint()?;
    if find_with_checkpoints(&svg, "<foreignObject", &mut || execution.checkpoint())?.is_none() {
        return Ok((svg, false));
    }
    foreign_object_label_fallback_svg_text_controlled(&svg, text_measurer, execution, structure)
        .map(|svg| (Cow::Owned(svg), true))
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
        ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        apply_strip_foreign_objects(svg, || ctx.checkpoint())
    }
}

pub(crate) fn apply_strip_foreign_objects<'a>(
    svg: Cow<'a, str>,
    mut checkpoint: impl FnMut() -> Result<()>,
) -> Result<Cow<'a, str>> {
    checkpoint()?;
    if find_with_checkpoints(&svg, "<foreignObject", &mut checkpoint)?.is_none() {
        return Ok(svg);
    }
    strip_foreign_objects_with_checkpoints(&svg, &mut checkpoint).map(Cow::Owned)
}

pub(crate) fn apply_drop_switch_native_fallbacks<'a>(
    svg: Cow<'a, str>,
    mut checkpoint: impl FnMut() -> Result<()>,
) -> Result<Cow<'a, str>> {
    let marker = r#"data-merman-foreignobject-source="switch-native-fallback""#;
    checkpoint()?;
    if find_with_checkpoints(&svg, marker, &mut checkpoint)?.is_none() {
        return Ok(svg);
    }
    drop_switch_native_fallbacks_with_checkpoints(&svg, &mut checkpoint).map(Cow::Owned)
}

pub(crate) fn drop_switch_native_fallbacks_with_checkpoints<E>(
    svg: &str,
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<String, E> {
    let marker = r#"data-merman-foreignobject-source="switch-native-fallback""#;
    if find_with_checkpoints(svg, marker, checkpoint)?.is_none() {
        return Ok(svg.to_string());
    }
    let mut out = String::with_capacity(svg.len());
    let mut cursor = 0;

    while let Some(rel_start) = find_with_checkpoints(&svg[cursor..], marker, checkpoint)? {
        let attr_start = cursor + rel_start;
        let Some(group_start) = rfind_with_checkpoints(&svg[..attr_start], "<g", checkpoint)?
        else {
            out.push_str(&svg[cursor..attr_start]);
            cursor = attr_start + marker.len();
            continue;
        };
        if group_start < cursor {
            out.push_str(&svg[cursor..attr_start]);
            cursor = attr_start + marker.len();
            continue;
        }
        let Some((_, group_end)) = find_matching_g_end(svg, group_start, checkpoint)? else {
            out.push_str(&svg[cursor..attr_start]);
            cursor = attr_start + marker.len();
            continue;
        };
        out.push_str(&svg[cursor..group_start]);
        cursor = group_end;
    }

    out.push_str(&svg[cursor..]);
    checkpoint()?;
    Ok(out)
}

#[cfg(test)]
use crate::svg::foreign_object_label_fallback_svg_text;

#[cfg(test)]
pub(crate) fn foreign_object_fallback_svg(svg: &str, text_measurer: &dyn TextMeasurer) -> String {
    foreign_object_label_fallback_svg_text(svg, text_measurer)
}

#[cfg(test)]
fn infallible<T>(result: std::result::Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => match error {},
    }
}

#[cfg(test)]
pub(crate) fn drop_switch_native_fallbacks(svg: &str) -> String {
    infallible(drop_switch_native_fallbacks_with_checkpoints(
        svg,
        &mut || Ok::<(), Infallible>(()),
    ))
}

pub(crate) fn strip_foreign_objects_with_checkpoints<E>(
    svg: &str,
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<String, E> {
    let mut out = String::with_capacity(svg.len());
    let mut cursor = 0;
    let mut pending_switch_closes: Vec<(usize, usize)> = Vec::new();

    while let Some((start, open_end)) = find_next_foreign_object_start(svg, cursor, checkpoint)? {
        while let Some(&(close_start, close_end)) = pending_switch_closes.last() {
            if close_start > start {
                break;
            }
            pending_switch_closes.pop();
            if close_start >= cursor {
                out.push_str(&svg[cursor..close_start]);
            }
            out.push_str("</g>");
            cursor = close_end.max(cursor);
        }

        let fo_tag = &svg[start..=open_end];
        let fo_end = if fo_tag.trim_end().ends_with("/>") {
            open_end + 1
        } else if let Some((_, close_end)) =
            find_foreign_object_close(svg, open_end + 1, checkpoint)?
        {
            close_end
        } else {
            open_end + 1
        };

        let switch_wrapper = find_wrapping_switch(svg, cursor, start, fo_end, checkpoint)?;

        if let Some((switch_start, switch_close_start, switch_close_end)) = switch_wrapper {
            // Keep the native fallback's inherited presentation and coordinate system.
            // A neutral group replaces switch selection after the HTML branch is removed.
            out.push_str(&svg[cursor..switch_start]);
            let Some(switch_open_end) =
                find_tag_end_with_checkpoints(svg, switch_start, checkpoint)?
            else {
                out.push_str(&svg[switch_start..switch_close_end]);
                cursor = switch_close_end;
                continue;
            };
            let switch_tag = &svg[switch_start..=switch_open_end];
            out.push_str("<g");
            let mut attr_cursor = 0;
            while let Some(attr) =
                next_svg_quoted_attr_with_checkpoints(switch_tag, attr_cursor, checkpoint)?
            {
                let name = &switch_tag[attr.name_start..attr.name_end];
                if ![
                    "requiredFeatures",
                    "requiredExtensions",
                    "systemLanguage",
                    "externalResourcesRequired",
                ]
                .iter()
                .any(|conditional| name.eq_ignore_ascii_case(conditional))
                {
                    out.push_str(&switch_tag[attr.full_start..attr.full_end]);
                }
                attr_cursor = attr.full_end;
            }
            out.push('>');
            // Defer closing this unwrapped `<g>` until `switch_close_start` so any additional
            // `<foreignObject>` siblings or nested `<switch>` elements inside it are also stripped.
            pending_switch_closes.push((switch_close_start, switch_close_end));
            cursor = fo_end;
            continue;
        }

        out.push_str(&svg[cursor..start]);
        cursor = fo_end;
    }

    while let Some((close_start, close_end)) = pending_switch_closes.pop() {
        if close_start >= cursor {
            out.push_str(&svg[cursor..close_start]);
        }
        out.push_str("</g>");
        cursor = close_end.max(cursor);
    }

    out.push_str(&svg[cursor..]);
    checkpoint()?;
    Ok(out)
}

#[cfg(test)]
pub(crate) fn strip_foreign_objects(svg: &str) -> String {
    infallible(strip_foreign_objects_with_checkpoints(svg, &mut || {
        Ok::<(), Infallible>(())
    }))
}

fn find_wrapping_switch<E>(
    svg: &str,
    cursor: usize,
    foreign_object_start: usize,
    foreign_object_end: usize,
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<Option<(usize, usize, usize)>, E> {
    let Some(switch_start) =
        find_wrapping_switch_start(svg, cursor, foreign_object_start, checkpoint)?
    else {
        return Ok(None);
    };
    let Some(switch_open_end) = find_tag_end_with_checkpoints(svg, switch_start, checkpoint)?
    else {
        return Ok(None);
    };
    if !svg[switch_open_end + 1..foreign_object_start]
        .trim()
        .is_empty()
    {
        return Ok(None);
    }

    let Some((switch_close_start, switch_close_end)) =
        find_matching_switch_end(svg, switch_start, checkpoint)?
    else {
        return Ok(None);
    };
    if foreign_object_end > switch_close_start {
        return Ok(None);
    }

    // Generated in-place text is not a native switch fallback.
    let mut tail_start = foreign_object_end;
    loop {
        let tail = svg[tail_start..switch_close_start].trim_start();
        tail_start = switch_close_start - tail.len();
        if is_start_foreign_object_tag(tail)
            && let Some(end) = find_tag_end_with_checkpoints(svg, tail_start, checkpoint)?
        {
            let next_start = if svg[tail_start..=end].trim_end().ends_with("/>") {
                end + 1
            } else if let Some((_, close_end)) =
                find_foreign_object_close(svg, end + 1, checkpoint)?
            {
                close_end
            } else {
                return Ok(None);
            };
            if next_start <= switch_close_start {
                tail_start = next_start;
                continue;
            }
            return Ok(None);
        }
        if is_start_g_tag(tail)
            && let Some(end) = find_tag_end_with_checkpoints(svg, tail_start, checkpoint)?
            && extract_quoted_attr(&svg[tail_start..=end], "data-merman-foreignobject")
                == Some("fallback")
            && let Some((_, end)) = find_matching_g_end(svg, tail_start, checkpoint)?
            && end <= switch_close_start
        {
            tail_start = end;
            continue;
        }
        if is_start_text_tag(tail) {
            break;
        }
        if is_start_g_tag(tail)
            && let Some((_, g_end)) = find_matching_g_end(svg, tail_start, checkpoint)?
            && g_end <= switch_close_start
            && contains_start_text_tag(&svg[tail_start..g_end], checkpoint)?
        {
            break;
        }
        if is_start_switch_tag(tail)
            && let Some((_, inner_end)) = find_matching_switch_end(svg, tail_start, checkpoint)?
            && inner_end <= switch_close_start
            && contains_start_text_tag(&svg[tail_start..inner_end], checkpoint)?
        {
            break;
        }
        return Ok(None);
    }

    Ok(Some((switch_start, switch_close_start, switch_close_end)))
}

fn find_wrapping_switch_start<E>(
    svg: &str,
    cursor: usize,
    before: usize,
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<Option<usize>, E> {
    let mut search_end = before;
    while search_end > cursor {
        let Some(rel_start) =
            rfind_with_checkpoints(&svg[cursor..search_end], "<switch", checkpoint)?
        else {
            return Ok(None);
        };
        let start = cursor + rel_start;
        let Some(open_end) = find_tag_end_with_checkpoints(svg, start, checkpoint)? else {
            return Ok(None);
        };
        if open_end >= before {
            search_end = start;
            continue;
        }

        let tag = &svg[start..=open_end];
        if is_start_switch_tag(tag) {
            return Ok(Some(start));
        }

        search_end = start;
    }
    Ok(None)
}

pub(crate) fn drop_native_duplicate_fallbacks_with_checkpoints<E>(
    svg: &str,
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<String, E> {
    // Only switch provenance proves that native text replaces this HTML branch.
    // Equal text/positions cannot establish equivalent CSS, background or visibility.
    drop_switch_native_fallbacks_with_checkpoints(svg, checkpoint)
}

#[cfg(test)]
pub(crate) fn drop_native_duplicate_fallbacks(svg: &str) -> String {
    infallible(drop_native_duplicate_fallbacks_with_checkpoints(
        svg,
        &mut || Ok::<(), Infallible>(()),
    ))
}

#[cfg(test)]
fn text_tag_is_fallback(tag: &str) -> bool {
    extract_quoted_attr(tag, "class").is_some_and(|classes| {
        classes
            .split_whitespace()
            .any(|class| class == "merman-foreignobject-fallback-text")
    })
}

fn find_matching_g_end<E>(
    svg: &str,
    group_start: usize,
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<Option<(usize, usize)>, E> {
    let Some(open_end) = find_tag_end_with_checkpoints(svg, group_start, checkpoint)? else {
        return Ok(None);
    };
    if svg[group_start..=open_end].trim_end().ends_with("/>") {
        return Ok(Some((group_start, open_end + 1)));
    }

    let mut depth = 1usize;
    let mut cursor = open_end + 1;
    while let Some(rel_tag) = find_with_checkpoints(&svg[cursor..], "<", checkpoint)? {
        let tag_start = cursor + rel_tag;
        let Some(tag_end) = find_tag_end_with_checkpoints(svg, tag_start, checkpoint)? else {
            break;
        };
        let tag = &svg[tag_start..=tag_end];
        if is_start_g_tag(tag) {
            if !tag.trim_end().ends_with("/>") {
                depth += 1;
            }
        } else if is_end_g_tag(tag) {
            let Some(next_depth) = depth.checked_sub(1) else {
                return Ok(None);
            };
            depth = next_depth;
            if depth == 0 {
                return Ok(Some((tag_start, tag_end + 1)));
            }
        }
        cursor = tag_end + 1;
    }
    Ok(None)
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

fn is_start_switch_tag(tag: &str) -> bool {
    let bytes = tag.as_bytes();
    tag.starts_with("<switch")
        && bytes
            .get("<switch".len())
            .is_some_and(|b| b.is_ascii_whitespace() || *b == b'>' || *b == b'/')
        && !tag.trim_end().ends_with("/>")
}

fn is_end_switch_tag(tag: &str) -> bool {
    let Some(rest) = tag.strip_prefix("</switch") else {
        return false;
    };
    let Some(rest) = rest.strip_suffix('>') else {
        return false;
    };
    rest.bytes().all(|b| b.is_ascii_whitespace())
}

fn is_start_foreign_object_tag(tag: &str) -> bool {
    let bytes = tag.as_bytes();
    tag.starts_with("<foreignObject")
        && bytes
            .get("<foreignObject".len())
            .is_some_and(|b| b.is_ascii_whitespace() || *b == b'>' || *b == b'/')
}

fn is_end_foreign_object_tag(tag: &str) -> bool {
    let Some(rest) = tag.strip_prefix("</foreignObject") else {
        return false;
    };
    let Some(rest) = rest.strip_suffix('>') else {
        return false;
    };
    rest.bytes().all(|b| b.is_ascii_whitespace())
}

fn is_start_text_tag(tag: &str) -> bool {
    let bytes = tag.as_bytes();
    tag.starts_with("<text")
        && bytes
            .get("<text".len())
            .is_some_and(|b| b.is_ascii_whitespace() || *b == b'>' || *b == b'/')
}

fn contains_start_text_tag<E>(
    fragment: &str,
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<bool, E> {
    let mut cursor = 0;
    while let Some(rel) = find_with_checkpoints(&fragment[cursor..], "<text", checkpoint)? {
        let start = cursor + rel;
        if is_start_text_tag(&fragment[start..]) {
            return Ok(true);
        }
        cursor = start + "<text".len();
    }
    Ok(false)
}

fn find_next_foreign_object_start<E>(
    svg: &str,
    from: usize,
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<Option<(usize, usize)>, E> {
    let mut cursor = from;
    while let Some(rel) = find_with_checkpoints(&svg[cursor..], "<foreignObject", checkpoint)? {
        let start = cursor + rel;
        let Some(open_end) = find_tag_end_with_checkpoints(svg, start, checkpoint)? else {
            return Ok(None);
        };
        if is_start_foreign_object_tag(&svg[start..=open_end]) {
            return Ok(Some((start, open_end)));
        }
        cursor = open_end + 1;
    }
    Ok(None)
}

fn find_foreign_object_close<E>(
    svg: &str,
    from: usize,
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<Option<(usize, usize)>, E> {
    let mut cursor = from;
    while let Some(rel) = find_with_checkpoints(&svg[cursor..], "</foreignObject", checkpoint)? {
        let start = cursor + rel;
        let Some(end) = find_tag_end_with_checkpoints(svg, start, checkpoint)? else {
            return Ok(None);
        };
        if is_end_foreign_object_tag(&svg[start..=end]) {
            return Ok(Some((start, end + 1)));
        }
        cursor = start + "</foreignObject".len();
    }
    Ok(None)
}

fn find_matching_switch_end<E>(
    svg: &str,
    switch_start: usize,
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<Option<(usize, usize)>, E> {
    let Some(open_end) = find_tag_end_with_checkpoints(svg, switch_start, checkpoint)? else {
        return Ok(None);
    };
    if svg[switch_start..=open_end].trim_end().ends_with("/>") {
        return Ok(Some((switch_start, open_end + 1)));
    }

    let mut depth = 1usize;
    let mut cursor = open_end + 1;
    while let Some(rel_tag) = find_with_checkpoints(&svg[cursor..], "<", checkpoint)? {
        let tag_start = cursor + rel_tag;
        let Some(tag_end) = find_tag_end_with_checkpoints(svg, tag_start, checkpoint)? else {
            break;
        };
        let tag = &svg[tag_start..=tag_end];
        if is_start_switch_tag(tag) {
            depth += 1;
        } else if is_end_switch_tag(tag) {
            let Some(next_depth) = depth.checked_sub(1) else {
                return Ok(None);
            };
            depth = next_depth;
            if depth == 0 {
                return Ok(Some((tag_start, tag_end + 1)));
            }
        }
        cursor = tag_end + 1;
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{
        HostMeasurementResult, HostTextMeasurementRequest, HostTextMeasurer, MeasurementProfileId,
        RenderEnvironment, TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity,
    };
    use crate::svg::pipeline::SvgPipeline;
    use crate::text::{TextMeasurer, TextMetrics, TextStyle};
    use merman_model::{OperationControl, OperationPhase};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    fn render_session() -> crate::environment::RenderSession {
        RenderEnvironment::deterministic().begin_session().unwrap()
    }

    struct CancellingMissingHost {
        calls: Arc<AtomicUsize>,
        control: OperationControl,
    }

    impl HostTextMeasurer for CancellingMissingHost {
        fn measure(&self, _request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.control.cancel();
            Ok(None)
        }
    }

    struct CountingFallback(Arc<AtomicUsize>);

    impl TextMeasurer for CountingFallback {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            self.0.fetch_add(1, Ordering::Relaxed);
            TextMetrics {
                width: 41.0,
                height: 16.0,
                line_count: 1,
            }
        }
    }

    #[test]
    fn controlled_foreign_object_fallback_does_not_enter_backend_after_host_cancellation() {
        let control = OperationControl::new();
        let host_calls = Arc::new(AtomicUsize::new(0));
        let fallback_calls = Arc::new(AtomicUsize::new(0));
        let fallback_identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.svg-fallback").unwrap(),
            "v1",
        )
        .unwrap();
        let host_identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.svg-host").unwrap(),
            "v1",
        )
        .unwrap();
        let policy = TextMeasurementPolicy::host_display_with_fallback(
            host_identity,
            Arc::new(CancellingMissingHost {
                calls: Arc::clone(&host_calls),
                control: control.clone(),
            }),
            [TextMeasurementPhase::Wrap],
            TextMeasurementProfile::new(
                fallback_identity,
                Arc::new(CountingFallback(Arc::clone(&fallback_calls))),
            ),
        );
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(policy)
            .begin_session_with_control(control)
            .unwrap();
        let svg = r#"<svg><foreignObject width="120" height="24"><div style="white-space: break-spaces; width: 120px"><p>cancel me now</p></div></foreignObject></svg>"#;

        let error = SvgPipeline::readable()
            .process_to_string(svg, &session)
            .unwrap_err();
        let crate::Error::Cancelled(cancelled) = error else {
            panic!("expected structured cancellation");
        };
        assert_eq!(cancelled.phase, OperationPhase::Postprocess);
        assert_eq!(host_calls.load(Ordering::Relaxed), 1);
        assert_eq!(fallback_calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn strip_foreign_object_stage_stops_during_a_long_structural_scan() {
        let control = OperationControl::new().for_phase(OperationPhase::Postprocess);
        let svg = format!(
            "<svg>{}<foreignObject width=\"1\" height=\"1\"/></svg>",
            "x".repeat(16 * 1024)
        );
        let mut checkpoints = 0usize;

        let error = apply_strip_foreign_objects(Cow::Borrowed(svg.as_str()), || {
            checkpoints = checkpoints.saturating_add(1);
            if checkpoints == 5 {
                control.cancel();
            }
            control.checkpoint().map_err(Into::into)
        })
        .unwrap_err();

        let crate::Error::Cancelled(cancelled) = error else {
            panic!("expected structured cancellation");
        };
        assert_eq!(cancelled.phase, OperationPhase::Postprocess);
        assert_eq!(checkpoints, 5);
    }

    #[test]
    fn drop_native_duplicate_fallbacks_preserves_unproven_ordinary_paint() {
        let svg = r##"<svg>
<text x="100" y="12" text-anchor="middle" dominant-baseline="central" class="task">Make tea</text>
<g data-merman-foreignobject="fallback" class="dup">
  <rect/>
  <text x="100" y="12" text-anchor="middle" dominant-baseline="central" class="merman-foreignobject-fallback-text">Make tea</text>
</g>
<g data-merman-foreignobject="fallback" class="keep">
  <text class="merman-foreignobject-fallback-text">Only fallback</text>
</g>
</svg>"##;

        let out = drop_native_duplicate_fallbacks(svg);

        assert!(out.contains(r#"class="task">Make tea</text>"#));
        assert!(out.contains(r#"class="dup""#));
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
    fn strip_foreign_objects_unwraps_switch_with_native_text_fallback() {
        let svg = r##"<svg><switch><foreignObject x="10" y="20" width="100" height="50"><div xmlns="http://www.w3.org/1999/xhtml">Make tea</div></foreignObject><text x="60" y="45">Make tea</text></switch></svg>"##;
        let out = strip_foreign_objects(svg);

        assert!(
            !out.contains("<foreignObject"),
            "foreignObject should be stripped: {out}"
        );
        assert!(
            !out.contains("<switch>"),
            "switch wrapper should be removed: {out}"
        );
        assert!(
            !out.contains("</switch>"),
            "switch closing tag should be removed: {out}"
        );
        assert!(
            out.contains(r#"<text x="60" y="45">Make tea</text>"#),
            "text fallback should be preserved: {out}"
        );
    }

    #[test]
    fn strip_foreign_objects_unwraps_switch_with_grouped_text_fallback() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><switch><foreignObject width="120" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><span class="nodeLabel" style="color:#ffffff !important">Production Deploy</span></div></foreignObject><g><rect class="background" style="stroke: none"/><text y="-10.1" style="color:#ffffff !important;fill:#ffffff !important"><tspan>Production Deploy</tspan></text></g></switch></svg>"##;
        let session = render_session();
        let out = SvgPipeline::resvg_safe()
            .process_to_string(svg, &session)
            .unwrap();

        assert!(!out.contains("<foreignObject"), "{out}");
        assert!(
            !out.contains("<switch"),
            "switch should be unwrapped: {out}"
        );
        assert!(
            !out.contains("</switch>"),
            "switch close tag should be removed: {out}"
        );
        assert!(
            out.contains("fill:#ffffff !important"),
            "authored text fill should survive resvg_safe: {out}"
        );
        assert!(
            !out.contains(r#"data-merman-foreignobject="fallback""#),
            "native grouped fallback should not trigger duplicate fallback: {out}"
        );
    }

    #[test]
    fn strip_foreign_objects_unwraps_switch_with_attrs() {
        let svg = r##"<svg><switch data-renderer="future"><foreignObject x="10" y="20" width="100" height="50"><div xmlns="http://www.w3.org/1999/xhtml">Make tea</div></foreignObject><text x="60" y="45">Make tea</text></switch></svg>"##;
        let out = strip_foreign_objects(svg);

        assert!(
            !out.contains("<foreignObject"),
            "foreignObject should be stripped: {out}"
        );
        assert!(
            !out.contains("<switch"),
            "switch wrapper should be removed: {out}"
        );
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
        assert!(
            out.contains(r#"<text x="40" y="15">Line 1</text>"#),
            "{out}"
        );
        assert!(
            out.contains(r#"<text x="40" y="30">Line 2</text>"#),
            "{out}"
        );
    }

    #[test]
    fn resvg_safe_pipeline_preserves_switch_text_fallback() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><switch><foreignObject x="150" y="50" width="550" height="50"><div class="journey-section" xmlns="http://www.w3.org/1999/xhtml" style="display: table; height: 100%; width: 100%;"><div class="label" style="display: table-cell; text-align: center; vertical-align: middle;">Go to work</div></div></foreignObject><text x="425" y="75" fill="#333"><tspan x="425" dy="0">Go to work</tspan></text></switch></svg>"##;
        let session = render_session();
        let out = SvgPipeline::resvg_safe()
            .process_to_string(svg, &session)
            .unwrap();

        assert!(
            !out.contains("<foreignObject"),
            "foreignObject should be stripped: {out}"
        );
        assert!(!out.contains("<switch>"), "switch should be removed: {out}");
        assert!(
            out.contains("Go to work"),
            "text fallback should survive full pipeline: {out}"
        );
        assert!(
            !out.contains(r#"data-merman-foreignobject-source"#),
            "generated fallback should be dropped: {out}"
        );
    }

    #[test]
    fn strip_foreign_objects_handles_journey_switch_pattern() {
        let svg = r##"<svg><g><rect class="section-type-0"/><switch><foreignObject x="150" y="50" width="550" height="50"><div class="journey-section section-type-0" xmlns="http://www.w3.org/1999/xhtml" style="display: table; height: 100%; width: 100%;"><div class="label" style="display: table-cell; text-align: center; vertical-align: middle;">Go to work</div></div></foreignObject><text x="425" y="75" fill="#333" class="journey-section section-type-0" style="text-anchor: middle;"><tspan x="425" dy="0">Go to work</tspan></text></switch></g></svg>"##;
        let out = strip_foreign_objects(svg);

        assert!(
            !out.contains("<foreignObject"),
            "foreignObject should be stripped: {out}"
        );
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
    fn strip_foreign_objects_preserves_switch_fallback_for_self_closing_foreign_object() {
        let svg = r#"<svg><switch><foreignObject width="80" height="24"/><text x="10" y="20">Fallback</text></switch></svg>"#;
        let out = strip_foreign_objects(svg);

        assert!(!out.contains("<switch"), "{out}");
        assert!(!out.contains("<foreignObject"), "{out}");
        assert!(
            out.contains(r#"<g><text x="10" y="20">Fallback</text></g>"#),
            "native SVG fallback after self-closing foreignObject must be preserved: {out}"
        );
    }

    #[test]
    fn drop_switch_native_fallbacks_removes_tagged_groups() {
        let svg = r##"<svg><text x="60" y="45">Make tea</text><g data-merman-foreignobject="fallback" data-merman-foreignobject-source="switch-native-fallback" class="merman-foreignobject-fallback"><text class="merman-foreignobject-fallback-text">Make tea</text></g><g data-merman-foreignobject="fallback" class="merman-foreignobject-fallback"><text class="merman-foreignobject-fallback-text">Other label</text></g></svg>"##;
        let out = drop_switch_native_fallbacks(svg);

        assert!(
            !out.contains("switch-native-fallback"),
            "tagged fallback group should be removed: {out}"
        );
        assert!(
            out.contains("Other label"),
            "non-switch fallback should be kept: {out}"
        );
        assert!(
            out.contains(r#"<text x="60" y="45">Make tea</text>"#),
            "native text should remain: {out}"
        );
    }

    #[test]
    fn resvg_safe_can_optionally_drop_native_duplicate_fallbacks() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
<g transform="translate(0,0)">
  <text x="100" y="12" text-anchor="middle" dominant-baseline="central" class="task">Make tea</text>
  <foreignObject width="200" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>Make tea</p></div></foreignObject>
</g>
<g transform="translate(0,40)">
  <foreignObject width="200" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>Only fallback</p></div></foreignObject>
</g>
</svg>"##;

        let session = render_session();
        let out = SvgPipeline::resvg_safe()
            .with_drop_native_duplicate_fallbacks(true)
            .process_to_string(svg, &session)
            .unwrap();

        assert!(!out.contains("<foreignObject"));
        assert_eq!(
            out.matches(r#"data-merman-foreignobject="fallback""#)
                .count(),
            2,
            "{out}"
        );
        assert!(out.contains("Only fallback"));
        assert!(out.contains(r#"class="task">Make tea</text>"#));
    }

    #[test]
    fn strip_foreign_objects_checks_tag_delimiters_and_whitespace_etags() {
        let svg = r#"<svg><foreignObjectExtra>keep</foreignObjectExtra><foreignObject width="10" height="10"><div>drop</div></foreignObject  ><text>ok</text></svg>"#;
        let out = strip_foreign_objects(svg);
        assert!(
            out.contains("<foreignObjectExtra>keep</foreignObjectExtra>"),
            "{out}"
        );
        assert!(!out.contains("drop"), "{out}");
        assert!(out.contains("<text>ok</text>"), "{out}");
        crate::xml_dom::Document::parse(&out).expect("output must be well-formed XML");
    }

    #[test]
    fn strip_foreign_objects_tracks_nested_switch_depth_and_strips_all_inner_foreign_objects() {
        let svg = concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
            r#"<switch id="outer">"#,
            r#"<foreignObject width="10" height="10"><div>Outer HTML</div></foreignObject >"#,
            r#"<foreignObject width="10" height="10"/>"#,
            r#"<g><switch id="inner">"#,
            r#"<foreignObject width="10" height="10"><div>Inner HTML</div></foreignObject>"#,
            r#"<text x="1" y="2">Inner Text</text>"#,
            r#"<foreignObject width="5" height="5"><div>Trailing HTML</div></foreignObject>"#,
            r#"</switch></g>"#,
            r#"</switch></svg>"#,
        );
        let out = strip_foreign_objects(svg);
        assert!(!out.contains("<foreignObject"), "{out}");
        assert!(!out.contains("<switch"), "{out}");
        assert!(!out.contains("</switch"), "{out}");
        assert!(!out.contains("Outer HTML"), "{out}");
        assert!(!out.contains("Inner HTML"), "{out}");
        assert!(!out.contains("Trailing HTML"), "{out}");
        assert!(
            out.contains(r#"<text x="1" y="2">Inner Text</text>"#),
            "{out}"
        );
        crate::xml_dom::Document::parse(&out).expect("nested switch unwrap must stay well-formed");
    }
}
