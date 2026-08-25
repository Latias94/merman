use super::*;
use crate::zenuml::{
    ZENUML_FRAME_TITLE_CLASS, ZENUML_FRAME_TITLE_SELECTOR, ZenumlArrowStyle, ZenumlCommentLayout,
    ZenumlCreationLayout, ZenumlDiagramLayout, ZenumlFragmentLayout, ZenumlLayoutFragmentKind,
    ZenumlMessageLayout, ZenumlParticipantLayout, ZenumlReturnLayout, ZenumlSelfCallLayout,
    ZenumlTitleThemePlan, resolve_zenuml_emojis_in_text, resolve_zenuml_title,
    zenuml_emoji_unicode,
};
use merman_core::diagrams::zenuml::ZenumlDiagramRenderModel;

const FRAME_HEADER_HEIGHT: f64 = 28.0;
const CONTENT_PADDING: f64 = 10.0;

pub(super) fn render_zenuml_diagram_svg_model(
    layout: &ZenumlDiagramLayout,
    model: &ZenumlDiagramRenderModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    title_theme: &ZenumlTitleThemePlan,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("zenuml");
    let title = resolve_zenuml_title(model, diagram_title);
    let mut title_theme_receipt = title_theme.begin_terminal_receipt();
    let content_left = 1.0 + CONTENT_PADDING + layout.frame_border_left;
    let view_width =
        layout.width + content_left + CONTENT_PADDING + layout.frame_border_right + 1.0;
    let view_height = layout.height + CONTENT_PADDING * 2.0 + FRAME_HEADER_HEIGHT - 1.0;
    let use_max_width = effective_config
        .get("sequence")
        .and_then(|sequence| sequence.get("useMaxWidth"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let bounds = root_svg::DiagramBounds::from_view_box(0.0, 0.0, view_width, view_height);
    let root_spec = root_svg::RootViewportSpec::mermaid(bounds, use_max_width);
    let mut out = BoundedSvgOutput::new(options.work_meter());
    let mut chrome = root_svg::RootChrome::new(diagram_id, "zenuml");
    chrome.dom.trailing_newline = false;
    let root_document =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::ZENUML, diagram_id)
            .write_open(&mut out, root_spec, chrome)?;
    options.checkpoint_emit()?;

    out.push_str("<defs><style>");
    write_zenuml_css(
        &mut out,
        title_theme.fill_css().unwrap_or(ZENUML_DEFAULT_TITLE_FILL),
    );
    out.push_str("</style></defs>");
    out.checkpoint()?;
    if let (Some(receipt), Some(fill_css)) = (title_theme_receipt.as_mut(), title_theme.fill_css())
    {
        receipt.record_stylesheet_writer(ZENUML_FRAME_TITLE_SELECTOR, fill_css);
    }
    let _ = write!(
        &mut out,
        r#"<rect class="frame-border-outer" x="0" y="0" width="{}" height="{}" rx="4"/><rect class="frame-border-inner" x="1" y="1" width="{}" height="{}" rx="3"/>"#,
        fmt(view_width),
        fmt(view_height),
        fmt(view_width - 2.0),
        fmt(view_height - 2.0),
    );
    out.checkpoint()?;
    let header_y = FRAME_HEADER_HEIGHT + 6.0;
    let _ = write!(
        &mut out,
        r#"<line class="frame-header-line" x1="1" y1="{}" x2="{}" y2="{}"/>"#,
        fmt(header_y - 0.5),
        fmt(view_width - 1.0),
        fmt(header_y - 0.5),
    );
    out.checkpoint()?;
    if let Some(title) = title {
        let _ = write!(
            &mut out,
            r#"<text x="5" y="{}" dominant-baseline="central" class="{}">{}</text>"#,
            fmt((header_y - 0.5) / 2.0),
            ZENUML_FRAME_TITLE_CLASS,
            escape_xml(&resolve_emoji_in_text(title)),
        );
        out.checkpoint()?;
        if let Some(receipt) = title_theme_receipt.as_mut() {
            receipt.record_frame_title_text_writer("text", ZENUML_FRAME_TITLE_CLASS, title);
        }
    }
    let _ = write!(
        &mut out,
        r#"<g class="zenuml-content" transform="translate({}, {})">"#,
        fmt(content_left),
        fmt(header_y),
    );
    out.checkpoint()?;
    let creation_names = layout
        .creations
        .iter()
        .map(|creation| creation.participant.name.as_str())
        .collect::<std::collections::HashSet<_>>();

    for group in &layout.groups {
        let rect_x = group.x - 0.5;
        let rect_y = group.y - 2.0;
        let title = (!group.name.is_empty()).then(|| escape_xml(&group.name));
        let _ = write!(
            &mut out,
            r#"<g class="participant-group"><rect x="{}" y="{}" width="{}" height="{}" class="group-outline" stroke-width="1" stroke-dasharray="4 3"/>"#,
            fmt(rect_x),
            fmt(rect_y),
            fmt(group.width + 1.0),
            fmt(group.height + 1.5),
        );
        if let Some(title) = title {
            let _ = write!(
                &mut out,
                r#"<rect x="{}" y="{}" width="{}" height="19.5" class="group-title-bg"/><text x="{}" y="{}" text-anchor="middle" dominant-baseline="middle" class="group-title-text">{}</text>"#,
                fmt(group.x + 0.5),
                fmt(rect_y + 0.5),
                fmt(group.width - 1.0),
                fmt(group.x + group.width / 2.0),
                fmt(group.y + 10.0),
                title,
            );
        }
        out.push_str("</g>");
        out.checkpoint()?;
    }
    for lifeline in &layout.lifelines {
        let _ = write!(
            &mut out,
            r#"<line class="lifeline" data-participant="{}" x1="{}" y1="{}" x2="{}" y2="{}" stroke-dasharray="5,5" stroke-dashoffset="5" shape-rendering="crispEdges"/>"#,
            escape_attr(&lifeline.participant_name),
            fmt(lifeline.x + 0.5),
            fmt(lifeline.top_y),
            fmt(lifeline.x + 0.5),
            fmt(lifeline.bottom_y),
        );
        out.checkpoint()?;
    }
    for participant in layout
        .participants
        .iter()
        .filter(|participant| !creation_names.contains(participant.name.as_str()))
    {
        render_participant(&mut out, participant)?;
    }
    for occurrence in &layout.occurrences {
        let _ = write!(
            &mut out,
            r#"<rect class="occurrence" data-statement="{}" data-participant="{}" x="{}" y="{}" width="{}" height="{}" rx="1"/>"#,
            escape_attr(&occurrence.statement_id),
            escape_attr(&occurrence.participant_name),
            fmt(occurrence.x + 1.0),
            fmt(occurrence.y + 1.0),
            fmt(occurrence.width - 2.0),
            fmt(occurrence.height - 2.0),
        );
        out.checkpoint()?;
    }
    for creation in &layout.creations {
        render_participant(&mut out, &creation.participant)?;
    }
    for message in &layout.messages {
        render_message(&mut out, message)?;
    }
    for self_call in &layout.self_calls {
        render_self_call(&mut out, self_call)?;
    }
    for creation in &layout.creations {
        render_creation(&mut out, creation)?;
    }
    for returned in &layout.returns {
        render_return(&mut out, returned)?;
    }
    for fragment in &layout.fragments {
        render_fragment(&mut out, fragment)?;
    }
    for divider in &layout.dividers {
        let center_x = divider.width / 2.0;
        let label = divider
            .label
            .trim()
            .trim_start_matches('=')
            .trim_end_matches('=')
            .trim();
        let label = resolve_emoji_in_text(label);
        let rect_width = divider.label_width + 17.0;
        let rect_height = 27.0;
        let rect_x = center_x - rect_width / 2.0;
        let rect_y = divider.y - rect_height / 2.0;
        let outer_left = rect_x - 0.5;
        let outer_right = rect_x + rect_width + 0.5;
        let _ = write!(
            &mut out,
            r#"<g class="divider" data-statement="{}"><line x1="0" y1="{}" x2="{}" y2="{}" class="divider-line"/><line x1="{}" y1="{}" x2="{}" y2="{}" class="divider-line"/><rect x="{}" y="{}" width="{}" height="{}" rx="2" class="divider-bg"/><text x="{}" y="{}" text-anchor="middle" dominant-baseline="central" class="divider-label">{}</text></g>"#,
            escape_attr(&divider.statement_id),
            fmt(divider.y),
            fmt(outer_left),
            fmt(divider.y),
            fmt(outer_right),
            fmt(divider.y),
            fmt(divider.width),
            fmt(divider.y),
            fmt(rect_x),
            fmt(rect_y),
            fmt(rect_width),
            fmt(rect_height),
            fmt(center_x),
            fmt(divider.y),
            escape_xml(&label),
        );
        out.checkpoint()?;
    }
    for comment in &layout.comments {
        render_comment(&mut out, comment)?;
    }
    out.push_str("</g></svg>");
    out.checkpoint()?;
    let rooted_svg = root_document.complete(out.finish()?)?;
    if title_theme_receipt.is_some_and(|receipt| !title_theme.record_terminal(receipt)) {
        return Err(crate::Error::InvalidModel {
            message: "ZenUML title theme receipt did not match text.frame-title".to_string(),
        });
    }
    Ok(rooted_svg)
}

fn render_participant(
    out: &mut impl SvgOutput,
    participant: &ZenumlParticipantLayout,
) -> Result<()> {
    let y = participant.y;
    let x = participant.x - participant.width / 2.0 + 1.0;
    let fill = participant
        .color
        .as_deref()
        .map(|color| {
            let color = if color.starts_with('#') {
                color.to_string()
            } else {
                format!("#{color}")
            };
            format!(r#" style="fill:{};""#, escape_attr(&color))
        })
        .unwrap_or_default();
    let _ = write!(
        out,
        r#"<g class="participant" data-participant="{}"><rect class="participant-box" x="{}" y="{}" width="{}" height="{}" rx="3"{}/>"#,
        escape_attr(&participant.name),
        fmt(x),
        fmt(y + 1.0),
        fmt(participant.width - 2.0),
        fmt(participant.height - 2.0),
        fill,
    );
    out.checkpoint()?;
    if participant.is_starter {
        render_participant_icon(out, "actor", participant.x - 14.0, y + 8.0)?;
        out.push_str("</g>");
        return out.checkpoint();
    }
    let text_y = y + participant.height / 2.0 - 0.25;
    let label_y = if participant.stereotype.is_some() {
        text_y + 8.0
    } else {
        text_y
    };
    let icon = participant
        .participant_type
        .as_deref()
        .and_then(crate::zenuml::zenuml_participant_icon_key);
    let emoji = participant
        .emoji
        .as_deref()
        .map(|shortcode| zenuml_emoji_unicode(shortcode).unwrap_or(shortcode));
    let mut text_x = participant.x;
    let mut text_anchor = "middle";
    if let Some(icon) = icon {
        let emoji_extra = if emoji.is_some() { 20.0 } else { 0.0 };
        let group_width = 24.0 + 4.0 + 16.0 + participant.label_width + emoji_extra;
        let group_x = participant.x - group_width / 2.0;
        let icon_x = group_x + 4.0;
        let icon_y =
            y + (participant.height - 24.0) / 2.0 + if icon == "boundary" { 2.75 } else { 0.0 };
        render_participant_icon(out, icon, icon_x, icon_y)?;
        if let Some(emoji) = emoji {
            let emoji_x = icon_x + 28.0;
            render_participant_emoji(out, emoji, emoji_x, label_y)?;
            text_x = emoji_x + 24.0;
        } else {
            text_x = group_x + 36.0;
        }
        text_anchor = "start";
    } else if let Some(emoji) = emoji {
        let group_width = 28.0 + participant.label_width;
        let inner_width = participant
            .stereotype_width
            .unwrap_or_default()
            .max(group_width);
        let group_x = participant.x - inner_width / 2.0;
        render_participant_emoji(out, emoji, group_x, label_y)?;
        text_x = group_x + 24.0;
        text_anchor = "start";
    }
    if let Some(stereotype) = &participant.stereotype {
        let stereotype_x = if icon.is_some() {
            text_x + participant.label_width / 2.0
        } else {
            participant.x
        };
        let _ = write!(
            out,
            r#"<text class="stereotype-label" x="{}" y="{}" text-anchor="middle" dominant-baseline="central" font-size="16" style="fill:#222;">«{}»</text>"#,
            fmt(stereotype_x),
            fmt(text_y - 8.0),
            escape_xml(stereotype),
        );
        out.checkpoint()?;
    }
    let text_length = if participant.name.contains(':') {
        format!(
            r#" textLength="{}" lengthAdjust="spacing""#,
            fmt(participant.label_width)
        )
    } else {
        String::new()
    };
    let _ = write!(
        out,
        r#"<text class="participant-label" x="{}" y="{}" text-anchor="{}" dominant-baseline="central"{}>{}</text></g>"#,
        fmt(text_x),
        fmt(label_y),
        text_anchor,
        text_length,
        escape_xml(&participant.label),
    );
    out.checkpoint()
}

fn render_participant_emoji(out: &mut impl SvgOutput, emoji: &str, x: f64, y: f64) -> Result<()> {
    let _ = write!(
        out,
        r#"<text x="{}" y="{}" dominant-baseline="central" font-family="'Apple Color Emoji','Segoe UI Emoji','Noto Color Emoji','Twemoji Mozilla',sans-serif" class="participant-emoji">{}</text>"#,
        fmt(x),
        fmt(y),
        escape_xml(emoji),
    );
    out.checkpoint()
}

struct ParticipantIcon {
    svg: &'static str,
    view_box_width: f64,
    view_box_height: f64,
    attributes: &'static str,
}

fn render_participant_icon(out: &mut impl SvgOutput, key: &str, x: f64, y: f64) -> Result<()> {
    let Some(icon) = participant_icon(key) else {
        return Ok(());
    };
    let scale = 24.0 / icon.view_box_width.max(icon.view_box_height);
    let content = svg_asset_content(icon.svg);
    let attributes = if icon.attributes.is_empty() {
        String::new()
    } else {
        format!(" {}", icon.attributes)
    };
    let _ = write!(
        out,
        r#"<g class="participant-icon" data-icon="{}" transform="translate({}, {}) scale({})"{}>{}</g>"#,
        escape_attr(key),
        fmt(x),
        fmt(y),
        fmt(scale),
        attributes,
        content,
    );
    out.checkpoint()
}

fn participant_icon(key: &str) -> Option<ParticipantIcon> {
    Some(match key {
        "actor" => ParticipantIcon {
            svg: include_str!("../../../assets/zenuml/actor.svg"),
            view_box_width: 24.0,
            view_box_height: 24.0,
            attributes: r#"fill="none""#,
        },
        "database" => ParticipantIcon {
            svg: include_str!("../../../assets/zenuml/database.svg"),
            view_box_width: 24.0,
            view_box_height: 24.0,
            attributes: r#"fill="none""#,
        },
        "boundary" => ParticipantIcon {
            svg: include_str!("../../../assets/zenuml/boundary.svg"),
            view_box_width: 101.0,
            view_box_height: 78.0,
            attributes: r#"fill="none""#,
        },
        "control" => ParticipantIcon {
            svg: include_str!("../../../assets/zenuml/control.svg"),
            view_box_width: 77.0,
            view_box_height: 86.0,
            attributes: r#"fill="none""#,
        },
        "entity" => ParticipantIcon {
            svg: include_str!("../../../assets/zenuml/entity.svg"),
            view_box_width: 77.0,
            view_box_height: 80.0,
            attributes: r#"fill="none""#,
        },
        "ec2" => ParticipantIcon {
            svg: include_str!("../../../assets/zenuml/ec2.svg"),
            view_box_width: 48.0,
            view_box_height: 48.0,
            attributes: "",
        },
        "lambda" => ParticipantIcon {
            svg: include_str!("../../../assets/zenuml/lambda.svg"),
            view_box_width: 48.0,
            view_box_height: 48.0,
            attributes: "",
        },
        "sqs" => ParticipantIcon {
            svg: include_str!("../../../assets/zenuml/sqs.svg"),
            view_box_width: 48.0,
            view_box_height: 48.0,
            attributes: "",
        },
        "sns" => ParticipantIcon {
            svg: include_str!("../../../assets/zenuml/sns.svg"),
            view_box_width: 48.0,
            view_box_height: 48.0,
            attributes: "",
        },
        "iam" => ParticipantIcon {
            svg: include_str!("../../../assets/zenuml/iam.svg"),
            view_box_width: 48.0,
            view_box_height: 48.0,
            attributes: "",
        },
        "azurefunction" => ParticipantIcon {
            svg: include_str!("../../../assets/zenuml/azurefunction.svg"),
            view_box_width: 18.0,
            view_box_height: 18.0,
            attributes: "",
        },
        _ => return None,
    })
}

fn render_message(out: &mut impl SvgOutput, message: &ZenumlMessageLayout) -> Result<()> {
    let left_to_right = message.from_x < message.to_x;
    let from_x = if left_to_right {
        message.from_x + 1.0
    } else {
        message.from_x
    };
    let to_x = if left_to_right {
        message.to_x
    } else {
        message.to_x + 1.0
    };
    let direction = if left_to_right { 1.0 } else { -1.0 };
    let label_x = (message.from_x + message.to_x) / 2.0 - direction * 3.5 + 0.5;
    let label_y = message.y - 3.5;
    let line_y = message.y - 0.5;
    let dash = if message.arrow_style == ZenumlArrowStyle::Dashed {
        r#" stroke-dasharray="6,4""#
    } else {
        ""
    };
    let _ = write!(
        out,
        r#"<g class="message" data-statement="{}"><line class="message-line" x1="{}" y1="{}" x2="{}" y2="{}"{} />"#,
        escape_attr(&message.statement_id),
        fmt(from_x),
        fmt(line_y),
        fmt(to_x),
        fmt(line_y),
        dash,
    );
    out.checkpoint()?;
    render_arrow_head(out, to_x, line_y, message.is_reverse, message.arrow_style)?;
    let _ = write!(
        out,
        r#"<text class="message-label" x="{}" y="{}" text-anchor="middle"{}>{}</text><text class="seq-number" x="{}" y="{}" text-anchor="end">{}</text></g>"#,
        fmt(label_x),
        fmt(label_y),
        style_attr(&message.style),
        escape_xml(&resolve_emoji_in_text(&message.label)),
        fmt(from_x.min(to_x) - 4.0),
        fmt(label_y),
        escape_xml(&message.number),
    );
    out.checkpoint()
}

fn render_self_call(out: &mut impl SvgOutput, call: &ZenumlSelfCallLayout) -> Result<()> {
    let asynchronous = call.arrow_style == ZenumlArrowStyle::Open;
    let label_y = call.y + if asynchronous { 15.0 } else { 12.0 };
    let svg_y = call.y + if asynchronous { 20.0 } else { 14.0 };
    let path_end = if asynchronous { "L1,15" } else { "L14,15" };
    let arrow_tx = if asynchronous { 0.0 } else { 7.0 };
    let arrow_path = if asynchronous {
        "M1 1.25 L6.15 4.5 L1 7.75"
    } else {
        "M1 1.25 L6.15 4.5 L1 7.75 Z"
    };
    let arrow_fill = if asynchronous { "none" } else { "#000" };
    let _ = write!(
        out,
        r##"<g class="message self-call" data-statement="{}"><svg x="{}" y="{}" width="30" height="24"><path d="M0,2 L26,2 Q28,2 28,4 L28,13 Q28,15 26,15 {}" fill="none" stroke="#000" stroke-width="2"/><g transform="translate({}, 10)"><svg height="10" width="7" viewBox="0 0 7 9"><g transform="scale(-1, 1) translate(-7, 0)"><path d="{}" stroke="#000" stroke-linecap="round" fill="{}" stroke-width="2"/></g></svg></g></svg><text x="{}" y="{}" text-anchor="start" class="message-label"{}>{}</text><text x="{}" y="{}" text-anchor="end" class="seq-number">{}</text></g>"##,
        escape_attr(&call.statement_id),
        fmt(call.x + 1.0),
        fmt(svg_y),
        path_end,
        fmt(arrow_tx),
        arrow_path,
        arrow_fill,
        fmt(call.x + 6.0),
        fmt(label_y),
        style_attr(&call.style),
        escape_xml(&resolve_emoji_in_text(call.label.trim())),
        fmt(call.x - 3.0),
        fmt(call.y + 12.0),
        escape_xml(&call.number),
    );
    out.checkpoint()
}

fn render_creation(out: &mut impl SvgOutput, creation: &ZenumlCreationLayout) -> Result<()> {
    let message = &creation.message;
    let participant = &creation.participant;
    let reverse = message.to_x < message.from_x;
    let to_x = if reverse {
        participant.x + participant.width / 2.0
    } else {
        participant.x - participant.width / 2.0
    };
    let from_x = if reverse {
        message.from_x
    } else {
        message.from_x + 1.0
    };
    let label_x = from_x + (to_x - from_x) / 2.0 + if reverse { 3.5 } else { -3.0 };
    let label_y = message.y - 3.0;
    let _ = write!(
        out,
        r#"<g class="creation" data-statement="{}"><line x1="{}" y1="{}" x2="{}" y2="{}" class="message-line" stroke-dasharray="6,4"/>"#,
        escape_attr(&creation.statement_id),
        fmt(from_x),
        fmt(message.y),
        fmt(to_x),
        fmt(message.y),
    );
    out.checkpoint()?;
    render_open_polyline(out, to_x, message.y, reverse, "arrow-head arrow-open")?;
    let rendered_label = render_creation_label(&message.label, &message.style);
    let _ = write!(
        out,
        r#"<text x="{}" y="{}" text-anchor="middle" class="message-label">{}</text><text x="{}" y="{}" text-anchor="end" class="seq-number">{}</text></g>"#,
        fmt(label_x),
        fmt(label_y),
        rendered_label,
        fmt(from_x.min(to_x) - 4.0),
        fmt(label_y),
        escape_xml(&message.number),
    );
    out.checkpoint()
}

fn render_return(out: &mut impl SvgOutput, returned: &ZenumlReturnLayout) -> Result<()> {
    if returned.is_self {
        let icon_x = returned.from_x + 4.0;
        let icon_y = returned.y - 12.0;
        let _ = write!(
            out,
            r#"<g class="return return-self" data-statement="{}"><g transform="translate({},{}) scale(0.0234375)"><path d="M256 0C114.84 0 0 114.84 0 256s114.84 256 256 256 256-114.84 256-256S397.16 0 256 0Zm0 469.33c-117.63 0-213.33-95.7-213.33-213.33S138.37 42.67 256 42.67 469.33 138.37 469.33 256 373.63 469.33 256 469.33Z" class="return-icon"/><path d="M288 192h-87.16l27.58-27.58a21.33 21.33 0 1 0-30.17-30.17l-64 64a21.33 21.33 0 0 0 0 30.17l64 64a21.33 21.33 0 0 0 30.17-30.17l-27.58-27.58H288a53.33 53.33 0 0 1 0 106.67h-32a21.33 21.33 0 0 0 0 42.66h32a96 96 0 0 0 0-192Z" class="return-icon"/></g><text x="{}" y="{}" text-anchor="start" class="return-label">{}</text></g>"#,
            escape_attr(&returned.statement_id),
            fmt(icon_x),
            fmt(icon_y),
            fmt(icon_x + 16.0),
            fmt(returned.y - 1.0),
            escape_xml(&resolve_emoji_in_text(&returned.label)),
        );
        return out.checkpoint();
    }
    let line_y = returned.y.floor();
    let label_x = returned.from_x.min(returned.to_x)
        + (returned.to_x - returned.from_x).abs() / 2.0
        + if returned.is_reverse { 3.5 } else { -3.5 };
    let label_y = line_y - 3.0;
    let _ = write!(
        out,
        r#"<g class="return" data-statement="{}"><line x1="{}" y1="{}" x2="{}" y2="{}" class="return-line"/>"#,
        escape_attr(&returned.statement_id),
        fmt(returned.from_x),
        fmt(line_y),
        fmt(returned.to_x),
        fmt(line_y),
    );
    out.checkpoint()?;
    render_open_polyline(
        out,
        returned.to_x,
        line_y,
        returned.is_reverse,
        "return-arrow",
    )?;
    let _ = write!(
        out,
        r#"<text x="{}" y="{}" text-anchor="middle" class="return-label">{}</text><text x="{}" y="{}" text-anchor="end" class="seq-number">{}</text></g>"#,
        fmt(label_x),
        fmt(label_y),
        escape_xml(&resolve_emoji_in_text(&returned.label)),
        fmt(returned.from_x.min(returned.to_x) - 4.0),
        fmt(label_y),
        escape_xml(&returned.number),
    );
    out.checkpoint()
}

fn render_arrow_head(
    out: &mut impl SvgOutput,
    tip_x: f64,
    tip_y: f64,
    points_left: bool,
    style: ZenumlArrowStyle,
) -> Result<()> {
    let filled = style == ZenumlArrowStyle::Solid;
    let x = if points_left { tip_x } else { tip_x - 7.0 };
    let transform = if points_left {
        r#" transform="scale(-1, 1) translate(-7, 0)""#
    } else {
        ""
    };
    let close = if filled { " Z" } else { "" };
    let _ = write!(
        out,
        r##"<svg x="{}" y="{}" width="7" height="10" viewBox="0 0 7 9" overflow="visible" class="arrow-head{}"><g{}><path d="M1 1.25 L6.15 4.5 L1 7.75{}" stroke="#000" stroke-linecap="round" stroke-width="2" fill="{}"/></g></svg>"##,
        fmt(x),
        fmt(tip_y - 5.0),
        if filled { "" } else { " arrow-open" },
        transform,
        close,
        if filled { "#000" } else { "none" },
    );
    out.checkpoint()
}

fn render_open_polyline(
    out: &mut impl SvgOutput,
    tip_x: f64,
    tip_y: f64,
    points_left: bool,
    class: &str,
) -> Result<()> {
    let direction = if points_left { 1.0 } else { -1.0 };
    let base_x = tip_x + direction * 5.15;
    let _ = write!(
        out,
        r#"<polyline points="{},{} {},{} {},{}" fill="none" stroke-linecap="round" class="{}"/>"#,
        fmt(base_x),
        fmt(tip_y - 3.25),
        fmt(tip_x),
        fmt(tip_y),
        fmt(base_x),
        fmt(tip_y + 3.25),
        class,
    );
    out.checkpoint()
}

fn render_creation_label(
    label: &str,
    style: &std::collections::BTreeMap<String, String>,
) -> String {
    let style = style_attr(style);
    if let Some(inner) = label
        .strip_prefix('«')
        .and_then(|label| label.strip_suffix('»'))
    {
        if inner == "create" {
            return format!("<tspan{style}>{}</tspan>", escape_xml(label));
        }
        return format!(
            "<tspan{style}>«</tspan><tspan dx=\"4\"{style}>{}</tspan><tspan dx=\"4\"{style}>»</tspan>",
            escape_xml(inner)
        );
    }
    format!("<tspan{style}>{}</tspan>", escape_xml(label))
}

fn render_fragment(out: &mut impl SvgOutput, fragment: &ZenumlFragmentLayout) -> Result<()> {
    let (class, kind_label) = match fragment.kind {
        ZenumlLayoutFragmentKind::Loop => ("loop", "Loop"),
        ZenumlLayoutFragmentKind::Alternative => ("alt", "Alt"),
        ZenumlLayoutFragmentKind::Parallel => ("par", "Par"),
        ZenumlLayoutFragmentKind::Optional => ("opt", "Opt"),
        ZenumlLayoutFragmentKind::Critical => ("critical", "Critical"),
        ZenumlLayoutFragmentKind::Section => ("section", "Section"),
        ZenumlLayoutFragmentKind::TryCatchFinally => ("tcf", "Try"),
        ZenumlLayoutFragmentKind::Reference => ("ref", "Ref"),
    };
    let header_x = fragment.x + 1.0;
    let header_y = fragment.header_y;
    let _ = write!(
        out,
        r#"<g class="fragment fragment-{}" data-statement="{}"><rect class="fragment-border" x="{}" y="{}" width="{}" height="{}" rx="4"/><rect class="fragment-header" x="{}" y="{}" width="{}" height="25"/>"#,
        class,
        escape_attr(&fragment.statement_id),
        fmt(fragment.x + 0.5),
        fmt(fragment.y + 0.5),
        fmt(fragment.width - 1.0),
        fmt(fragment.height - 1.0),
        fmt(header_x),
        fmt(header_y),
        fmt(fragment.width - 2.0),
    );
    out.checkpoint()?;
    render_fragment_icon(out, fragment.kind, header_x + 4.0, header_y)?;
    let _ = write!(
        out,
        r#"<text x="{}" y="{}" dominant-baseline="central" class="fragment-label">{}</text><text x="{}" y="{}" text-anchor="end" dominant-baseline="central" class="seq-number">{}</text>"#,
        fmt(header_x + 26.0),
        fmt(header_y + 12.0),
        kind_label,
        fmt(fragment.x - 3.0),
        fmt(header_y + 8.0),
        escape_xml(&fragment.number),
    );
    out.checkpoint()?;
    if !fragment.label.is_empty() {
        render_bracketed_label(
            out,
            header_x,
            header_y + 40.0,
            &fragment.label,
            fragment.label_width,
            "fragment-condition",
        )?;
    }
    for section in fragment.sections.iter().skip(1) {
        let separator_y = section.y + 0.5;
        let _ = write!(
            out,
            r#"<line class="fragment-separator" x1="{}" y1="{}" x2="{}" y2="{}"/>"#,
            fmt(fragment.x + 1.0 + section.content_inset_left.unwrap_or_default()),
            fmt(separator_y),
            fmt(fragment.x + fragment.width - 1.0),
            fmt(separator_y),
        );
        out.checkpoint()?;
        if let Some(inner) = &section.inner_label {
            render_bracketed_label(
                out,
                fragment.x + 1.0,
                section.y + 16.0,
                inner,
                section.inner_label_width,
                "fragment-section-label",
            )?;
        } else if let (Some(keyword), Some(detail)) = (&section.keyword, &section.detail) {
            let keyword_width = section.keyword_width.unwrap_or(0.0);
            let background_width = keyword_width + section.detail_width.unwrap_or(0.0) + 16.0;
            let _ = write!(
                out,
                r##"<g opacity="0.65"><rect x="{}" y="{}" width="{}" height="20" fill="#fff"/><text x="{}" y="{}" class="fragment-section-label" fill="#222">{}</text><text x="{}" y="{}" class="fragment-section-label" fill="#222">{}</text></g>"##,
                fmt(fragment.x + 1.0),
                fmt(section.y + 1.0),
                fmt(background_width),
                fmt(fragment.x + 5.0),
                fmt(section.y + 16.0),
                escape_xml(keyword),
                fmt(fragment.x + 13.0 + keyword_width),
                fmt(section.y + 16.0),
                escape_xml(detail),
            );
            out.checkpoint()?;
        } else if !section.label.is_empty() {
            let _ = write!(
                out,
                r##"<g opacity="0.65"><rect x="{}" y="{}" width="{}" height="20" fill="#fff"/><text x="{}" y="{}" class="fragment-section-label">{}</text></g>"##,
                fmt(fragment.x + 1.0),
                fmt(section.y + 1.0),
                fmt(section.label_width.unwrap_or(0.0) + 8.0),
                fmt(fragment.x + 5.0),
                fmt(section.y + 16.0),
                escape_xml(&section.label),
            );
            out.checkpoint()?;
        }
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn render_bracketed_label(
    out: &mut impl SvgOutput,
    x: f64,
    y: f64,
    inner: &str,
    inner_width: Option<f64>,
    class: &str,
) -> Result<()> {
    let inner_x = x + 7.89;
    let close_x = inner_x + inner_width.unwrap_or(0.0) + 4.0;
    let _ = write!(
        out,
        r#"<g><text x="{}" y="{}" class="{}">[</text><text x="{}" y="{}" class="{}" opacity="0.65">{}</text><text x="{}" y="{}" class="{}">]</text></g>"#,
        fmt(x),
        fmt(y),
        class,
        fmt(inner_x),
        fmt(y),
        class,
        escape_xml(&resolve_emoji_in_text(inner)),
        fmt(close_x),
        fmt(y),
        class,
    );
    out.checkpoint()
}

fn render_fragment_icon(
    out: &mut impl SvgOutput,
    kind: ZenumlLayoutFragmentKind,
    x: f64,
    y: f64,
) -> Result<()> {
    let (key, raw, view_box, attributes) = match kind {
        ZenumlLayoutFragmentKind::Alternative => (
            "alt",
            include_str!("../../../assets/zenuml/fragment-alt.svg"),
            "0 0 24 24",
            r#" fill="none""#,
        ),
        ZenumlLayoutFragmentKind::Optional => (
            "opt",
            include_str!("../../../assets/zenuml/fragment-opt.svg"),
            "0 0 24 24",
            r#" fill="none""#,
        ),
        ZenumlLayoutFragmentKind::Parallel => (
            "par",
            include_str!("../../../assets/zenuml/fragment-par.svg"),
            "0 0 24 24",
            r#" fill="none""#,
        ),
        ZenumlLayoutFragmentKind::Critical => (
            "critical",
            include_str!("../../../assets/zenuml/fragment-critical.svg"),
            "0 0 24 24",
            r#" fill="none""#,
        ),
        ZenumlLayoutFragmentKind::Loop => (
            "loop",
            include_str!("../../../assets/zenuml/fragment-loop.svg"),
            "0 0 1024 1024",
            r##" fill="#000" stroke="none""##,
        ),
        ZenumlLayoutFragmentKind::TryCatchFinally => (
            "tcf",
            include_str!("../../../assets/zenuml/fragment-tcf.svg"),
            "0 0 76 76",
            r##" fill="#000" stroke="none""##,
        ),
        ZenumlLayoutFragmentKind::Section => (
            "section",
            include_str!("../../../assets/zenuml/fragment-section.svg"),
            "0 0 15 15",
            r##" fill="#000" stroke="none""##,
        ),
        ZenumlLayoutFragmentKind::Reference => (
            "ref",
            include_str!("../../../assets/zenuml/fragment-ref.svg"),
            "0 0 24 24",
            r#" fill="none""#,
        ),
    };
    let content = svg_asset_content(raw);
    let _ = write!(
        out,
        r#"<svg class="fragment-icon" data-icon="{}" x="{}" y="{}" width="20" height="24" viewBox="{}"{}>{}</svg>"#,
        key,
        fmt(x),
        fmt(y),
        view_box,
        attributes,
        content,
    );
    out.checkpoint()
}

fn svg_asset_content(svg: &'static str) -> &'static str {
    let Some(svg_start) = svg.find("<svg") else {
        return svg;
    };
    let Some(open_end) = svg[svg_start..].find('>') else {
        return svg;
    };
    let content_start = svg_start + open_end + 1;
    let Some(content_end) = svg.rfind("</svg>") else {
        return svg;
    };
    svg[content_start..content_end].trim()
}

fn render_comment(out: &mut impl SvgOutput, comment: &ZenumlCommentLayout) -> Result<()> {
    let lines = markdown_comment_lines(&comment.text);
    let _ = write!(
        out,
        r#"<text class="comment-text" data-statement="{}"{}>"#,
        escape_attr(&comment.statement_id),
        style_attr(&comment.style),
    );
    out.checkpoint()?;
    for (index, line) in lines.iter().enumerate() {
        if index == 0 {
            let _ = write!(
                out,
                r#"<tspan x="{}" y="{}">{}</tspan>"#,
                fmt(comment.x),
                fmt(comment.y),
                if line.is_empty() { " " } else { line },
            );
        } else {
            let _ = write!(
                out,
                r#"<tspan x="{}" dy="20">{}</tspan>"#,
                fmt(comment.x),
                if line.is_empty() { " " } else { line },
            );
        }
        out.checkpoint()?;
    }
    out.push_str("</text>");
    out.checkpoint()
}

fn markdown_comment_lines(markdown: &str) -> Vec<String> {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

    let mut lines = vec![String::new()];
    let mut style = MarkdownRunStyle::default();
    let parser = Parser::new_ext(markdown, Options::ENABLE_STRIKETHROUGH);
    for event in parser {
        match event {
            Event::Start(Tag::Strong) => style.strong = true,
            Event::End(TagEnd::Strong) => style.strong = false,
            Event::Start(Tag::Emphasis) => style.emphasis = true,
            Event::End(TagEnd::Emphasis) => style.emphasis = false,
            Event::Start(Tag::CodeBlock(_)) => style.code_block = true,
            Event::End(TagEnd::CodeBlock) => style.code_block = false,
            Event::Text(text) => append_markdown_text(&mut lines, &text, style),
            Event::Code(text) => append_markdown_text(&mut lines, &text, style),
            Event::SoftBreak | Event::HardBreak => lines.push(String::new()),
            Event::End(TagEnd::Paragraph) if lines.last().is_some_and(|line| !line.is_empty()) => {
                lines.push(String::new());
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                append_markdown_text(&mut lines, &html, style);
            }
            _ => {}
        }
    }
    while lines.len() > 1 && lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

#[derive(Clone, Copy, Default)]
struct MarkdownRunStyle {
    strong: bool,
    emphasis: bool,
    code_block: bool,
}

fn append_markdown_text(lines: &mut Vec<String>, text: &str, style: MarkdownRunStyle) {
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            lines.push(String::new());
        }
        let escaped = escape_xml(line);
        if escaped.is_empty() {
            continue;
        }
        if !style.strong && !style.emphasis && !style.code_block {
            lines.last_mut().unwrap().push_str(&escaped);
            continue;
        }
        lines.last_mut().unwrap().push_str("<tspan");
        if style.strong {
            lines.last_mut().unwrap().push_str(r#" font-weight="bold""#);
        }
        if style.emphasis {
            lines
                .last_mut()
                .unwrap()
                .push_str(r#" font-style="italic""#);
        }
        if style.code_block {
            lines
                .last_mut()
                .unwrap()
                .push_str(r#" font-family="monospace""#);
        }
        lines.last_mut().unwrap().push('>');
        lines.last_mut().unwrap().push_str(&escaped);
        lines.last_mut().unwrap().push_str("</tspan>");
    }
}

fn style_attr(style: &std::collections::BTreeMap<String, String>) -> String {
    if style.is_empty() {
        return String::new();
    }
    let declarations = style
        .iter()
        .map(|(name, value)| format!("{name}:{}", escape_attr(value)))
        .collect::<Vec<_>>()
        .join(";");
    format!(r#" style="{declarations}""#)
}

fn resolve_emoji_in_text(text: &str) -> String {
    resolve_zenuml_emojis_in_text(text)
}

fn zenuml_css() -> &'static str {
    r#"
.frame-border-outer{fill:#666}.frame-border-inner,.frame-header-bg{fill:#fff}.frame-header-line{stroke:#666;stroke-width:1;shape-rendering:crispEdges}.frame-title{font-family:Helvetica,Verdana,serif;font-size:16px;font-weight:600;fill:#222}.participant-box{fill:#fff;stroke:#666;stroke-width:2}.participant-label{font-family:Helvetica,Verdana,serif;font-size:16px;fill:#222}.participant-icon{color:#222}.participant-icon [fill="currentColor"]:not([stroke]){stroke:#666;stroke-width:1}.participant-emoji{font-size:16px}.stereotype-label{font-family:Helvetica,Verdana,serif;font-size:16px;fill:#222}.lifeline{stroke:#666;stroke-width:1}.message-line{stroke:#000;stroke-width:2;shape-rendering:crispEdges}.message-label{font-family:Helvetica,Verdana,serif;font-size:14px;fill:#222}.arrow-head{fill:#000;stroke:#000;stroke-width:2}.arrow-open{fill:none}.occurrence{fill:#dedede;stroke:#666;stroke-width:2;shape-rendering:crispEdges}.fragment-border{fill:none;stroke:#666;stroke-width:1;shape-rendering:crispEdges}.fragment-header{fill:#dedede;fill-opacity:.498;stroke:none;shape-rendering:crispEdges}.fragment-label{font-family:Helvetica,Verdana,serif;font-size:14px;font-weight:600;fill:#000}.fragment-condition{font-family:Helvetica,Verdana,serif;font-size:14px;fill:#000}.fragment-separator{stroke:#e5e7eb;stroke-width:1;shape-rendering:crispEdges}.fragment-section-label{font-family:Helvetica,Verdana,serif;font-size:14px;fill:#000}.return-line{stroke:#000;stroke-width:2;stroke-dasharray:6,4;shape-rendering:crispEdges}.return-arrow{stroke:#000;stroke-width:2;fill:none}.return-label{font-family:Helvetica,Verdana,serif;font-size:14px;fill:#222}.return-icon{fill:#222}.divider-line{stroke:#aa3;stroke-width:1}.divider-bg{fill:#fff5ad;stroke:#aa3;stroke-width:1}.divider-label{font-family:Helvetica,Verdana,serif;font-size:14px;fill:#333}.comment-text{font-family:Helvetica,Verdana,serif;font-size:14px;fill:#333;opacity:.5}.seq-number{font-family:Helvetica,Verdana,serif;font-size:12px;font-weight:100;fill:#6b7280}.group-outline{fill:none;stroke:#666}.group-title-bg{fill:#fff;stroke:none}.group-title-text{font-family:Helvetica,Verdana,serif;font-size:13px;font-weight:400;fill:#222}
"#
}

const ZENUML_DEFAULT_TITLE_FILL: &str = "#222";

fn write_zenuml_css(out: &mut impl SvgOutput, title_fill: &str) {
    const TITLE_RULE_OPEN: &str = ".frame-title{";
    const DEFAULT_FILL_SUFFIX: &str = "fill:#222";

    let (prefix, title_rule_and_suffix) = zenuml_css()
        .split_once(TITLE_RULE_OPEN)
        .expect("ZenUML CSS must contain the frame-title rule");
    let (title_rule, suffix) = title_rule_and_suffix
        .split_once('}')
        .expect("ZenUML frame-title rule must be closed");
    let title_rule = title_rule
        .strip_suffix(DEFAULT_FILL_SUFFIX)
        .expect("ZenUML frame-title rule must end with its default fill");

    out.push_str(prefix);
    out.push_str(TITLE_RULE_OPEN);
    out.push_str(title_rule);
    out.push_str("fill:");
    out.push_str(title_fill);
    out.push('}');
    out.push_str(suffix);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};

    #[test]
    fn zenuml_css_has_one_final_frame_title_fill_owner() {
        let mut baseline = String::new();
        write_zenuml_css(&mut baseline, ZENUML_DEFAULT_TITLE_FILL);
        assert_eq!(baseline, zenuml_css());

        let mut themed = String::new();
        write_zenuml_css(&mut themed, "transparent");
        assert!(themed.contains(
            ".frame-title{font-family:Helvetica,Verdana,serif;font-size:16px;font-weight:600;fill:transparent}"
        ));
        assert_eq!(themed.matches(".frame-title{").count(), 1);
        assert!(!themed.contains(".frame-title{fill:"));
    }

    #[test]
    fn zenuml_polyline_propagates_the_bounded_svg_sink_failure() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, 1)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let mut out = BoundedSvgOutput::new(&meter);

        let error = render_open_polyline(&mut out, 32.0, 48.0, false, "return-arrow")
            .expect_err("the bounded sink must stop ZenUML polyline emission");

        assert!(matches!(error, crate::Error::ResourceLimitExceeded(_)));
        assert!(out.as_str().is_empty());
    }

    #[test]
    fn fenced_comment_code_closes_monospace_runs_per_svg_line() {
        let lines = markdown_comment_lines("```text\nfirst\nsecond\n```");

        assert_eq!(
            lines,
            [
                r#"<tspan font-family="monospace">first</tspan>"#,
                r#"<tspan font-family="monospace">second</tspan>"#,
            ]
        );
    }

    #[test]
    fn vendored_icon_content_does_not_add_nested_svg_viewports() {
        for raw in [
            include_str!("../../../assets/zenuml/actor.svg"),
            include_str!("../../../assets/zenuml/ec2.svg"),
            include_str!("../../../assets/zenuml/fragment-loop.svg"),
        ] {
            let content = svg_asset_content(raw);
            assert!(!content.contains("<svg"));
            assert!(!content.contains("</svg>"));
            assert!(content.contains('<'));
        }
    }
}
