use super::*;
use crate::zenuml::{
    ZenumlDiagramLayout, ZenumlFragmentLayout, ZenumlLayoutMessageKind, ZenumlMessageLayout,
    ZenumlParticipantLayout,
};
use merman_core::diagrams::zenuml::{ZenumlDiagramRenderModel, ZenumlFragmentKind};

const FRAME_HEADER_HEIGHT: f64 = 28.0;
const CONTENT_PADDING: f64 = 10.0;

pub(super) fn render_zenuml_diagram_svg_model(
    layout: &ZenumlDiagramLayout,
    model: &ZenumlDiagramRenderModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("zenuml");
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
    let mut out = String::new();
    let mut chrome = root_svg::RootChrome::new(diagram_id, "zenuml");
    let aria_labelledby = model
        .acc_title
        .as_ref()
        .map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = model
        .acc_descr
        .as_ref()
        .map(|_| format!("chart-desc-{diagram_id}"));
    chrome.aria_labelledby = aria_labelledby.as_deref();
    chrome.aria_describedby = aria_describedby.as_deref();
    chrome.dom.trailing_newline = false;
    let root_document =
        root_svg::RootViewportContext::new(crate::family::RenderFamilyKind::Zenuml, diagram_id)
            .write_open(&mut out, root_spec, chrome)?;

    out.push_str("<defs><style>");
    out.push_str(zenuml_css());
    out.push_str("</style></defs>");
    if let Some(title) = &model.acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{}">{}</title>"#,
            escape_attr(diagram_id),
            escape_xml(title),
        );
    }
    if let Some(description) = &model.acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{}">{}</desc>"#,
            escape_attr(diagram_id),
            escape_xml(description),
        );
    }
    let _ = write!(
        &mut out,
        r#"<rect class="frame-border-outer" x="0" y="0" width="{}" height="{}" rx="4"/><rect class="frame-border-inner" x="1" y="1" width="{}" height="{}" rx="3"/>"#,
        fmt(view_width),
        fmt(view_height),
        fmt(view_width - 2.0),
        fmt(view_height - 2.0),
    );
    let header_y = FRAME_HEADER_HEIGHT + 6.0;
    let _ = write!(
        &mut out,
        r#"<line class="frame-header-line" x1="1" y1="{}" x2="{}" y2="{}"/>"#,
        fmt(header_y - 0.5),
        fmt(view_width - 1.0),
        fmt(header_y - 0.5),
    );
    let title = model
        .title
        .as_deref()
        .filter(|title| !title.is_empty())
        .or_else(|| diagram_title.filter(|title| !title.is_empty()));
    if let Some(title) = title {
        let _ = write!(
            &mut out,
            r#"<text x="5" y="{}" dominant-baseline="central" class="frame-title">{}</text>"#,
            fmt((header_y - 0.5) / 2.0),
            escape_xml(title),
        );
    }
    let _ = write!(
        &mut out,
        r#"<g class="zenuml-content" transform="translate({}, {})">"#,
        fmt(content_left),
        fmt(header_y),
    );

    for group in &layout.groups {
        let _ = write!(
            &mut out,
            r#"<g class="participant-group"><rect x="{}" y="{}" width="{}" height="{}" rx="3"/><rect class="group-title-bg" x="{}" y="{}" width="{}" height="20"/><text class="group-title" x="{}" y="{}">{}</text></g>"#,
            fmt(group.x),
            fmt(group.y),
            fmt(group.width),
            fmt(group.height),
            fmt(group.x),
            fmt(group.y),
            fmt(group.width),
            fmt(group.x + 6.0),
            fmt(group.y + 15.0),
            escape_xml(&group.name),
        );
    }
    for lifeline in &layout.lifelines {
        let _ = write!(
            &mut out,
            r#"<line class="lifeline" data-participant="{}" x1="{}" y1="{}" x2="{}" y2="{}"/>"#,
            escape_attr(&lifeline.participant_name),
            fmt(lifeline.x),
            fmt(lifeline.top_y),
            fmt(lifeline.x),
            fmt(lifeline.bottom_y),
        );
    }
    for participant in layout
        .participants
        .iter()
        .filter(|participant| !participant.created)
    {
        render_participant(&mut out, participant, None);
    }
    for occurrence in &layout.occurrences {
        let _ = write!(
            &mut out,
            r#"<rect class="occurrence" data-statement="{}" x="{}" y="{}" width="{}" height="{}"/>"#,
            escape_attr(&occurrence.statement_id),
            fmt(occurrence.x),
            fmt(occurrence.y),
            fmt(occurrence.width),
            fmt(occurrence.height),
        );
    }
    for participant in layout
        .participants
        .iter()
        .filter(|participant| participant.created)
    {
        render_participant(&mut out, participant, None);
    }
    for message in &layout.messages {
        render_message(&mut out, message);
    }
    for fragment in &layout.fragments {
        render_fragment(&mut out, fragment);
    }
    for divider in &layout.dividers {
        let center_x = divider.width / 2.0;
        let rect_width = divider.label_width + 17.0;
        let rect_height = 27.0;
        let rect_x = center_x - rect_width / 2.0;
        let rect_y = divider.y - rect_height / 2.0;
        let outer_left = rect_x - 0.5;
        let outer_right = rect_x + rect_width + 0.5;
        let _ = write!(
            &mut out,
            r#"<g class="divider" data-statement="{}"><line x1="0" y1="{}" x2="{}" y2="{}"/><line x1="{}" y1="{}" x2="{}" y2="{}"/><rect x="{}" y="{}" width="{}" height="{}" rx="2"/><text x="{}" y="{}" text-anchor="middle" dominant-baseline="central">{}</text></g>"#,
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
            escape_xml(&divider.label),
        );
    }
    for comment in &layout.comments {
        let _ = write!(
            &mut out,
            r#"<text class="comment-text" data-statement="{}" x="{}" y="{}">{}</text>"#,
            escape_attr(&comment.statement_id),
            fmt(comment.x),
            fmt(comment.y),
            escape_xml(&comment.text),
        );
    }
    let bottom_y = layout.height + 12.0;
    for participant in layout
        .participants
        .iter()
        .filter(|participant| participant.show_bottom)
    {
        render_participant(&mut out, participant, Some(bottom_y));
    }
    out.push_str("</g></svg>");
    root_document.complete(out)
}

fn render_participant(out: &mut String, participant: &ZenumlParticipantLayout, y: Option<f64>) {
    let y = y.unwrap_or(participant.y);
    let x = participant.x - participant.width / 2.0;
    let fill = participant
        .color
        .as_deref()
        .map(|color| format!(r#" style="fill:{}""#, escape_attr(color)))
        .unwrap_or_default();
    let classes = if y == participant.y {
        "participant"
    } else {
        "participant participant-bottom"
    };
    let _ = write!(
        out,
        r#"<g class="{}" data-participant="{}"><rect class="participant-box" x="{}" y="{}" width="{}" height="{}" rx="3"{}/>"#,
        classes,
        escape_attr(&participant.name),
        fmt(x),
        fmt(y),
        fmt(participant.width),
        fmt(participant.height),
        fill,
    );
    if participant.is_starter {
        let _ = write!(
            out,
            r#"<circle class="starter-head" cx="{}" cy="{}" r="5"/><path class="starter-body" d="M {} {} L {} {} M {} {} L {} {} M {} {} L {} {}"/>"#,
            fmt(participant.x),
            fmt(y + 11.0),
            fmt(participant.x),
            fmt(y + 16.0),
            fmt(participant.x),
            fmt(y + 29.0),
            fmt(participant.x - 7.0),
            fmt(y + 21.0),
            fmt(participant.x + 7.0),
            fmt(y + 21.0),
            fmt(participant.x),
            fmt(y + 29.0),
            fmt(participant.x - 6.0),
            fmt(y + 37.0),
        );
        out.push_str("</g>");
        return;
    }
    let mut label_y = y + participant.height / 2.0 + 5.0;
    if let Some(stereotype) = &participant.stereotype {
        let _ = write!(
            out,
            r#"<text class="stereotype-label" x="{}" y="{}" text-anchor="middle">«{}»</text>"#,
            fmt(participant.x),
            fmt(y + 14.0),
            escape_xml(stereotype),
        );
        label_y += 7.0;
    }
    let type_prefix = participant
        .participant_type
        .as_deref()
        .map(|kind| format!("@{kind} "))
        .unwrap_or_default();
    let emoji_prefix = participant
        .emoji
        .as_deref()
        .map(|emoji| format!("[{emoji}] "))
        .unwrap_or_default();
    let _ = write!(
        out,
        r#"<text class="participant-label" x="{}" y="{}" text-anchor="middle">{}{}{}</text></g>"#,
        fmt(participant.x),
        fmt(label_y),
        escape_xml(&type_prefix),
        escape_xml(&emoji_prefix),
        escape_xml(&participant.label),
    );
}

fn render_message(out: &mut String, message: &ZenumlMessageLayout) {
    if message.is_self && message.kind != ZenumlLayoutMessageKind::Return {
        let x = message.from_x;
        let y = message.y;
        let _ = write!(
            out,
            r#"<g class="message self-call" data-statement="{}"><path d="M {} {} h 28 v 24 h -14"/><polyline points="{},{} {},{} {},{}"/><text class="message-label" x="{}" y="{}">{}</text><text class="seq-number" x="{}" y="{}" text-anchor="end">{}</text></g>"#,
            escape_attr(&message.statement_id),
            fmt(x),
            fmt(y + 16.0),
            fmt(x + 14.0),
            fmt(y + 35.0),
            fmt(x + 20.0),
            fmt(y + 40.0),
            fmt(x + 14.0),
            fmt(y + 45.0),
            fmt(x + 6.0),
            fmt(y + 12.0),
            escape_xml(&message.label),
            fmt(x - 4.0),
            fmt(y + 12.0),
            escape_xml(&message.number),
        );
        return;
    }
    let reverse = message.to_x < message.from_x;
    let line_class = if message.kind == ZenumlLayoutMessageKind::Return {
        "return-line"
    } else {
        "message-line"
    };
    let open = matches!(
        message.kind,
        ZenumlLayoutMessageKind::Asynchronous
            | ZenumlLayoutMessageKind::Creation
            | ZenumlLayoutMessageKind::Return
    );
    let direction = if reverse { -1.0 } else { 1.0 };
    let tip_x = message.to_x;
    let base_x = tip_x - direction * 6.0;
    let label_x = (message.from_x + message.to_x) / 2.0 - direction * 3.5 + 0.5;
    let _ = write!(
        out,
        r#"<g class="message" data-statement="{}"><line class="{}" x1="{}" y1="{}" x2="{}" y2="{}"/><polyline class="arrow-head{}" points="{},{} {},{} {},{}"/><text class="message-label" x="{}" y="{}" text-anchor="middle">{}</text><text class="seq-number" x="{}" y="{}" text-anchor="end">{}</text></g>"#,
        escape_attr(&message.statement_id),
        line_class,
        fmt(message.from_x),
        fmt(message.y - 0.5),
        fmt(message.to_x),
        fmt(message.y - 0.5),
        if open { " arrow-open" } else { "" },
        fmt(base_x),
        fmt(message.y - 4.0),
        fmt(tip_x),
        fmt(message.y - 0.5),
        fmt(base_x),
        fmt(message.y + 3.0),
        fmt(label_x),
        fmt(message.y - 4.0),
        escape_xml(&message.label),
        fmt(message.from_x.min(message.to_x) - 4.0),
        fmt(message.y - 4.0),
        escape_xml(&message.number),
    );
}

fn render_fragment(out: &mut String, fragment: &ZenumlFragmentLayout) {
    let class = match fragment.kind {
        ZenumlFragmentKind::Loop => "loop",
        ZenumlFragmentKind::Alternative => "alt",
        ZenumlFragmentKind::Parallel => "par",
        ZenumlFragmentKind::Optional => "opt",
        ZenumlFragmentKind::Critical => "critical",
        ZenumlFragmentKind::Section => "section",
        ZenumlFragmentKind::TryCatchFinally => "tcf",
    };
    let _ = write!(
        out,
        r#"<g class="fragment fragment-{}" data-statement="{}"><rect class="fragment-border" x="{}" y="{}" width="{}" height="{}"/><path class="fragment-header" d="M {} {} h 74 l -10 24 h -64 Z"/><text class="fragment-label" x="{}" y="{}">{}</text>"#,
        class,
        escape_attr(&fragment.statement_id),
        fmt(fragment.x),
        fmt(fragment.y),
        fmt(fragment.width),
        fmt(fragment.height),
        fmt(fragment.x),
        fmt(fragment.header_y),
        fmt(fragment.x + 7.0),
        fmt(fragment.header_y + 17.0),
        escape_xml(fragment.label.as_deref().unwrap_or(class)),
    );
    for (index, y) in fragment.section_y.iter().enumerate().skip(1) {
        let _ = write!(
            out,
            r#"<line class="fragment-separator" x1="{}" y1="{}" x2="{}" y2="{}"/>"#,
            fmt(fragment.x),
            fmt(*y),
            fmt(fragment.x + fragment.width),
            fmt(*y),
        );
        if let Some(Some(label)) = fragment.section_labels.get(index) {
            let _ = write!(
                out,
                r#"<text class="fragment-section-label" x="{}" y="{}">{}</text>"#,
                fmt(fragment.x + 7.0),
                fmt(*y + 16.0),
                escape_xml(label),
            );
        }
    }
    out.push_str("</g>");
}

fn zenuml_css() -> &'static str {
    r#"
.frame-border-outer{fill:#666}.frame-border-inner,.frame-header-bg{fill:#fff}.frame-header-line{stroke:#666;stroke-width:1;shape-rendering:crispEdges}.frame-title{font-family:Helvetica,Verdana,serif;font-size:16px;font-weight:600;fill:#222}.participant-box{fill:#fff;stroke:#666;stroke-width:2}.participant-label{font-family:Helvetica,Verdana,serif;font-size:16px;fill:#222}.stereotype-label{font-family:Helvetica,Verdana,serif;font-size:12px;fill:#222}.starter-head{fill:#222}.starter-body{fill:none;stroke:#222;stroke-width:2}.lifeline{stroke:#666;stroke-width:1;stroke-dasharray:5,5}.message-line{stroke:#000;stroke-width:2}.return-line{stroke:#000;stroke-width:2;stroke-dasharray:6,4}.message-label{font-family:Helvetica,Verdana,serif;font-size:14px;fill:#222}.arrow-head{fill:#000;stroke:#000;stroke-width:2}.arrow-open{fill:none}.occurrence{fill:#dedede;stroke:#666;stroke-width:2}.fragment-border{fill:none;stroke:#666;stroke-width:1}.fragment-header{fill:#dedede;fill-opacity:.498}.fragment-label{font-family:Helvetica,Verdana,serif;font-size:14px;font-weight:600;fill:#000}.fragment-separator{stroke:#e5e7eb;stroke-width:1}.fragment-section-label{font-family:Helvetica,Verdana,serif;font-size:14px;fill:#000}.comment-text{font-family:Helvetica,Verdana,serif;font-size:14px;fill:#333;opacity:.5}.seq-number{font-family:Helvetica,Verdana,serif;font-size:12px;font-weight:100;fill:#6b7280}.participant-group>rect:first-child{fill:none;stroke:#666;stroke-dasharray:5,5}.group-title-bg{fill:#fff}.group-title{font-family:Helvetica,Verdana,serif;font-size:12px;fill:#222}.divider line{stroke:#aa3}.divider rect{fill:#fff5ad;stroke:#aa3}.divider text{font-family:Helvetica,Verdana,serif;font-size:14px;fill:#333}
"#
}
