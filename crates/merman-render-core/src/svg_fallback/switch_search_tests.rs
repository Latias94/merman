use super::{
    checkpoint_loop, find_tag_end_with_checkpoints, find_wrapping_switch_start,
    is_foreign_object_switch_native_fallback, is_start_switch_tag,
};
use std::convert::Infallible;

// Keep the original lookup as an independent oracle for the cursor's skipped prefix.
fn reference_wrapping_switch_start(svg: &str, before: usize) -> Option<usize> {
    let mut search_end = before;
    while search_end > 0 {
        let start = svg[..search_end].rfind("<switch")?;
        let open_end =
            find_tag_end_with_checkpoints(svg, start, &mut || Ok::<(), Infallible>(())).unwrap()?;
        if open_end < before && is_start_switch_tag(&svg[start..=open_end]) {
            return Some(start);
        }
        search_end = start;
    }
    None
}

fn reference_native_fallback(svg: &str, start: usize, end: usize) -> bool {
    let Some(switch_start) = reference_wrapping_switch_start(svg, start) else {
        return false;
    };
    let Some(switch_close) = svg[end..].find("</switch>") else {
        return false;
    };
    !svg[switch_start..start].contains("</switch>")
        && svg[end..end + switch_close].contains("<text")
}

#[test]
fn monotone_switch_search_matches_legacy_lookup_on_malformed_and_unicode_prefixes() {
    let fragments = [
        "",
        "😀日程",
        "<switch>",
        "<switch data-name='a>b'>",
        "<switch\n>",
        "<switch/>",
        "<switch / >",
        "<switching>",
        "<SWITCH>",
        "</switch>",
        "<!-- <switch -->",
        "<![CDATA[<switch>]]>",
        "<switch data-name='",
        "<switch",
        "<text>Native</text>",
    ];
    for left in fragments {
        for right in fragments {
            let svg = format!("<svg>{left}α{right}終</svg>");
            let mut search_from = 0;
            for before in svg.char_indices().map(|(offset, _)| offset) {
                let actual =
                    find_wrapping_switch_start(&svg, before, &mut search_from, &mut || {
                        Ok::<(), Infallible>(())
                    })
                    .unwrap();
                assert_eq!(
                    actual,
                    reference_wrapping_switch_start(&svg, before),
                    "prefix={:?}",
                    &svg[..before],
                );
            }
        }
    }
}

#[test]
fn monotone_switch_search_preserves_native_fallback_predicate() {
    let object = "<foreignObject><p>日程</p></foreignObject>";
    let examples = [
        format!(
            "<svg>😀{object}图{object}<switch>{object}<text>Native</text></switch>{object}</svg>"
        ),
        format!("<svg><switch/>{object}<switching>{object}</switching>{object}</svg>"),
        format!(
            "<svg><switch data-name='a>b'>{object}<switch>{object}</switch><text>A</text></switch>{object}</svg>"
        ),
        format!("<svg>{object}<!-- <switch -->{object}<switchX/>{object}</svg>"),
        format!("<svg><switch>{object}</switch><SWITCH>{object}</SWITCH>{object}</svg>"),
        format!("<svg><switch>{object}<text>A</text>{object}<text>B</text></switch>{object}</svg>"),
    ];
    for svg in examples {
        let mut search_from = 0;
        for (start, _) in svg.match_indices("<foreignObject") {
            let end = start + object.len();
            let actual = is_foreign_object_switch_native_fallback(
                &svg,
                start,
                end,
                &mut search_from,
                &mut || Ok::<(), Infallible>(()),
            )
            .unwrap();
            assert_eq!(actual, reference_native_fallback(&svg, start, end), "{svg}");
        }
    }
}

#[test]
fn no_switch_prefix_work_is_bounded_by_input_plus_per_label_overlap() {
    for count in [8, 32, 128, 500] {
        let svg = format!(
            "<svg>{}</svg>",
            "😀abc<foreignObject><p>日程</p></foreignObject>".repeat(count)
        );
        let mut search_from = 0;
        let mut old_bytes = 0;
        let mut new_bytes = 0;
        let mut checkpoints = 0;
        for (before, _) in svg.match_indices("<foreignObject") {
            old_bytes += before;
            // With no prefix matches, the checkpointed last-match search visits this exact span.
            new_bytes += before - search_from;
            assert_eq!(
                find_wrapping_switch_start(&svg, before, &mut search_from, &mut || {
                    checkpoints += 1;
                    Ok::<(), Infallible>(())
                }),
                Ok(None),
            );
        }
        assert!(new_bytes <= svg.len() + 9 * count);
        assert!(old_bytes > new_bytes * count / 4);

        // Bind the bound to the real checkpointed searches, not only the cursor arithmetic.
        // Per-call rounding and terminal checks add at most two checkpoints per label.
        let mut maximum_checkpoints = 2 * count;
        for iteration in 0..svg.len() + 9 * count {
            checkpoint_loop(iteration, &mut || {
                maximum_checkpoints += 1;
                Ok::<(), Infallible>(())
            })
            .unwrap();
        }
        assert!(checkpoints <= maximum_checkpoints);
    }
}

#[test]
fn monotone_switch_search_cancels_during_new_text_and_before_a_cached_hit() {
    let svg = format!(
        "<svg>{}<foreignObject/>{}<foreignObject/></svg>",
        "α".repeat(512),
        "表".repeat(2048),
    );
    let mut objects = svg.match_indices("<foreignObject");
    let mut search_from = 0;
    let first = objects.next().unwrap().0;
    assert_eq!(
        find_wrapping_switch_start(&svg, first, &mut search_from, &mut || {
            Ok::<(), usize>(())
        }),
        Ok(None),
    );
    assert!(search_from > 0 && search_from < first);
    let second = objects.next().unwrap().0;
    let mut checkpoints = 0;
    let result = find_wrapping_switch_start(&svg, second, &mut search_from, &mut || {
        checkpoints += 1;
        if checkpoints == 7 { Err(7) } else { Ok(()) }
    });
    assert_eq!(result, Err(7));

    let svg = "<svg><switch><foreignObject/></switch></svg>";
    let before = svg.find("<foreignObject").unwrap();
    let mut search_from = svg.find("<switch").unwrap();
    assert_eq!(
        find_wrapping_switch_start(svg, before, &mut search_from, &mut || Err(0)),
        Err(0),
    );
}
