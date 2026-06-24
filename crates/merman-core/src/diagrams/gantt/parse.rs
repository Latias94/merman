use super::*;
use crate::{EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol, SourceSpan};

fn strip_inline_comment(line: &str) -> &str {
    // Mermaid gantt does not treat `%%` as an inline comment delimiter for statements like `title`
    // or task lines (see `fixtures/gantt/task_inline_percent_comment.mmd`). It does, however,
    // accept full-line `%% ...` comments (and directive lines `%%{...}%%`).
    let t = line.trim_start();
    if t.starts_with("%%{") {
        return line;
    }
    if t.starts_with("%%") {
        return "";
    }
    line
}

fn split_statement_suffix(s: &str) -> &str {
    let mut end = s.len();
    for (i, c) in s.char_indices() {
        if c == '#' || c == ';' {
            end = i;
            break;
        }
    }
    &s[..end]
}

fn starts_with_ci(s: &str, prefix: &str) -> bool {
    // Avoid slicing by raw bytes: non-ASCII leading characters would panic if `prefix.len()` is
    // not on a UTF-8 boundary (e.g. task lines that start with CJK labels).
    s.get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

fn parse_keyword_arg<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let t = line.trim_start();
    if !starts_with_ci(t, keyword) {
        return None;
    }
    let after = &t[keyword.len()..];
    let ws = after.chars().next()?;
    if !ws.is_whitespace() {
        return None;
    }
    let rest = &after[ws.len_utf8()..];
    Some(split_statement_suffix(rest))
}

fn parse_keyword_arg_full_line<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let t = line.trim_start();
    if !starts_with_ci(t, keyword) {
        return None;
    }
    let after = &t[keyword.len()..];
    let ws = after.chars().next()?;
    if !ws.is_whitespace() {
        return None;
    }
    Some(&after[ws.len_utf8()..])
}

fn parse_key_colon_value(line: &str, key: &str) -> Option<String> {
    let t = line.trim_start();
    if !starts_with_ci(t, key) {
        return None;
    }
    let rest = t[key.len()..].trim_start();
    let rest = rest.strip_prefix(':')?;
    // Mermaid gantt's `accTitle:` / `accDescr:` values are end-of-line tokens (not `;`/`#`-terminated).
    Some(rest.trim().to_string())
}

fn parse_acc_descr_block(lines: &mut std::str::Lines<'_>, first_line: &str) -> Option<String> {
    let t = first_line.trim_start();
    if !starts_with_ci(t, "accDescr") {
        return None;
    }
    let rest = t["accDescr".len()..].trim_start();
    let rest = rest.strip_prefix('{')?;

    let mut buf = String::new();
    if let Some(end) = rest.find('}') {
        buf.push_str(&rest[..end]);
        return Some(buf.trim().to_string());
    }
    buf.push_str(rest);
    buf.push('\n');

    for line in lines {
        if let Some(end) = line.find('}') {
            buf.push_str(&line[..end]);
            break;
        }
        buf.push_str(line);
        buf.push('\n');
    }
    Some(buf.trim().to_string())
}

fn parse_click_statement(line: &str) -> Option<ClickStatementParts> {
    let t = line.trim_start();
    if !starts_with_ci(t, "click") {
        return None;
    }
    let rest = t["click".len()..].trim_start();
    let mut parts = rest.splitn(2, char::is_whitespace);
    let ids = parts.next()?.trim().to_string();
    let mut tail = parts.next().unwrap_or("").trim_start();

    let mut href: Option<String> = None;
    let mut call: Option<(String, Option<String>)> = None;

    while !tail.is_empty() {
        if starts_with_ci(tail, "href") {
            let mut r = tail["href".len()..].trim_start();
            if !r.starts_with('\"') {
                break;
            }
            r = &r[1..];
            let Some(end) = r.find('\"') else {
                break;
            };
            href = Some(r[..end].to_string());
            tail = r[end + 1..].trim_start();
            continue;
        }

        if starts_with_ci(tail, "call") {
            let r = tail["call".len()..].trim_start();
            let Some(paren) = r.find('(') else {
                break;
            };
            let name = r[..paren].trim().to_string();
            let after = &r[paren + 1..];
            let Some(end) = after.find(')') else {
                break;
            };
            let args_raw = after[..end].to_string();
            let args = if args_raw.trim().is_empty() {
                None
            } else {
                Some(args_raw)
            };
            call = Some((name, args));
            tail = after[end + 1..].trim_start();
            continue;
        }

        break;
    }

    Some((ids, href, call))
}

type ClickStatementParts = (String, Option<String>, Option<(String, Option<String>)>);

pub fn parse_gantt_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    collect_gantt_editor_facts_from_lines(code)
}

fn collect_gantt_editor_facts_from_lines(code: &str) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let mut header_seen = false;
    let mut in_frontmatter = false;
    let mut in_acc_descr_block = false;
    let mut offset = 0usize;

    for segment in code.split_inclusive('\n') {
        let line = line_without_ending(segment);
        collect_gantt_editor_line(
            line,
            offset,
            &mut header_seen,
            &mut in_frontmatter,
            &mut in_acc_descr_block,
            &mut facts,
        );
        offset += segment.len();
    }

    facts
}

fn line_without_ending(segment: &str) -> &str {
    let segment = segment.strip_suffix('\n').unwrap_or(segment);
    segment.strip_suffix('\r').unwrap_or(segment)
}

fn collect_gantt_editor_line(
    line: &str,
    line_start: usize,
    header_seen: &mut bool,
    in_frontmatter: &mut bool,
    in_acc_descr_block: &mut bool,
    facts: &mut EditorSemanticFacts,
) {
    let raw_trimmed = line.trim();
    if *in_frontmatter {
        if raw_trimmed == "---" {
            *in_frontmatter = false;
        }
        return;
    }
    if line_start == 0 && raw_trimmed == "---" {
        *in_frontmatter = true;
        return;
    }

    if *in_acc_descr_block {
        if line.contains('}') {
            *in_acc_descr_block = false;
        }
        return;
    }

    let stripped = strip_inline_comment(line);
    let trimmed = stripped.trim();
    if trimmed.is_empty() {
        return;
    }
    if trimmed.starts_with("%%{") {
        return;
    }

    if !*header_seen && starts_with_ci(trimmed, "gantt") {
        *header_seen = true;
        if let Some((rest, rest_start)) = gantt_header_rest(stripped, line_start)
            && !rest.trim().is_empty()
        {
            let recognized =
                collect_gantt_statement_editor_facts(rest, rest_start, in_acc_descr_block, facts);
            if !recognized {
                facts.mark_recovered();
            }
        }
        return;
    }

    if !*header_seen {
        facts.mark_recovered();
    }

    let recognized =
        collect_gantt_statement_editor_facts(stripped, line_start, in_acc_descr_block, facts);
    if !recognized {
        facts.mark_recovered();
    }
}

fn gantt_header_rest(line: &str, line_start: usize) -> Option<(&str, usize)> {
    let trimmed = line.trim_start();
    if !starts_with_ci(trimmed, "gantt") {
        return None;
    }

    let leading = line.len().saturating_sub(trimmed.len());
    let after_start = leading + "gantt".len();
    let after = &line[after_start..];
    let whitespace_len: usize = after
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf8)
        .sum();
    Some((
        &after[whitespace_len..],
        line_start + after_start + whitespace_len,
    ))
}

fn collect_gantt_statement_editor_facts(
    line: &str,
    line_start: usize,
    in_acc_descr_block: &mut bool,
    facts: &mut EditorSemanticFacts,
) -> bool {
    let stripped = strip_inline_comment(line);
    let trimmed = stripped.trim();
    if trimmed.is_empty() {
        return true;
    }

    if parse_keyword_arg(stripped, "dateFormat").is_some() {
        facts.push_directive_prefix("dateFormat");
        return true;
    }
    if starts_with_ci(trimmed, "inclusiveEndDates") {
        facts.push_directive_prefix("inclusiveEndDates");
        return true;
    }
    if starts_with_ci(trimmed, "topAxis") {
        facts.push_directive_prefix("topAxis");
        return true;
    }
    if parse_keyword_arg(stripped, "axisFormat").is_some() {
        facts.push_directive_prefix("axisFormat");
        return true;
    }
    if parse_keyword_arg(stripped, "tickInterval").is_some() {
        facts.push_directive_prefix("tickInterval");
        return true;
    }
    if parse_keyword_arg(stripped, "includes").is_some() {
        facts.push_directive_prefix("includes");
        return true;
    }
    if parse_keyword_arg(stripped, "excludes").is_some() {
        facts.push_directive_prefix("excludes");
        return true;
    }
    if parse_keyword_arg_full_line(stripped, "todayMarker").is_some() {
        facts.push_directive_prefix("todayMarker");
        return true;
    }
    if let Some(day) = parse_keyword_arg_full_line(stripped, "weekday") {
        facts.push_directive_prefix("weekday");
        let day = day.trim().to_lowercase();
        if !matches!(
            day.as_str(),
            "monday" | "tuesday" | "wednesday" | "thursday" | "friday" | "saturday" | "sunday"
        ) {
            facts.mark_recovered();
        }
        return true;
    }
    if let Some(day) = parse_keyword_arg_full_line(stripped, "weekend") {
        facts.push_directive_prefix("weekend");
        let day = day.trim().to_lowercase();
        if !matches!(day.as_str(), "friday" | "saturday") {
            facts.mark_recovered();
        }
        return true;
    }
    if parse_keyword_arg_full_line(stripped, "title").is_some() {
        facts.push_directive_prefix("title");
        return true;
    }
    if parse_keyword_arg_full_line(stripped, "section").is_some() {
        facts.push_directive_prefix("section");
        return true;
    }
    if parse_key_colon_value(stripped, "accTitle").is_some() {
        facts.push_directive_prefix("accTitle");
        return true;
    }
    if parse_key_colon_value(stripped, "accDescr").is_some() {
        facts.push_directive_prefix("accDescr");
        return true;
    }
    if let Some(block_open) = gantt_acc_descr_block_open(stripped) {
        facts.push_directive_prefix("accDescr");
        *in_acc_descr_block = block_open;
        return true;
    }
    if parse_click_statement(stripped).is_some() {
        facts.push_directive_prefix("click");
        collect_gantt_click_target_symbols(stripped, line_start, facts);
        return true;
    }

    collect_gantt_task_symbols(stripped, line_start, facts)
}

fn gantt_acc_descr_block_open(line: &str) -> Option<bool> {
    let trimmed = line.trim_start();
    if !starts_with_ci(trimmed, "accDescr") {
        return None;
    }

    let rest = trimmed["accDescr".len()..].trim_start();
    let rest = rest.strip_prefix('{')?;
    Some(!rest.contains('}'))
}

fn collect_gantt_click_target_symbols(
    line: &str,
    line_start: usize,
    facts: &mut EditorSemanticFacts,
) {
    let trimmed = line.trim_start();
    let leading = line.len().saturating_sub(trimmed.len());
    let rest_start = "click".len();
    let Some(rest) = trimmed.get(rest_start..) else {
        return;
    };
    let rest_leading: usize = rest
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf8)
        .sum();
    let ids_start = rest_start + rest_leading;
    let ids_tail = &trimmed[ids_start..];
    let ids_len = ids_tail
        .char_indices()
        .find_map(|(idx, ch)| ch.is_whitespace().then_some(idx))
        .unwrap_or(ids_tail.len());
    let ids = &ids_tail[..ids_len];
    let statement_span =
        SourceSpan::new(line_start + leading, line_start + leading + trimmed.len());

    push_gantt_delimited_id_symbols(
        ids,
        line_start + leading + ids_start,
        ',',
        "gantt click target",
        EditorSemanticKind::Function,
        statement_span,
        facts,
    );
}

fn collect_gantt_task_symbols(
    line: &str,
    line_start: usize,
    facts: &mut EditorSemanticFacts,
) -> bool {
    let task_stmt = line.trim_start();
    let leading = line.len().saturating_sub(task_stmt.len());
    let Some(colon) = task_stmt.find(':') else {
        return false;
    };

    let task_txt = &task_stmt[..colon];
    let task_data = split_statement_suffix(&task_stmt[colon + 1..]);
    if task_txt.is_empty() || task_data.trim().is_empty() {
        return true;
    }

    let statement_span =
        SourceSpan::new(line_start + leading, line_start + leading + task_stmt.len());
    collect_gantt_task_data_symbols(
        task_data,
        line_start + leading + colon + 1,
        statement_span,
        facts,
    );
    true
}

fn collect_gantt_task_data_symbols(
    task_data: &str,
    task_data_start: usize,
    statement_span: SourceSpan,
    facts: &mut EditorSemanticFacts,
) {
    let fields = split_gantt_fields(task_data, task_data_start)
        .into_iter()
        .filter_map(SpannedText::trim)
        .collect::<Vec<_>>();
    let mut field_start = 0usize;
    while fields
        .get(field_start)
        .is_some_and(|field| is_gantt_task_tag(field.text))
    {
        field_start += 1;
    }

    let fields = &fields[field_start..];
    match fields {
        [end_data] => push_gantt_relative_ref_symbols(end_data, statement_span, facts),
        [start_data, end_data] => {
            push_gantt_relative_ref_symbols(start_data, statement_span, facts);
            push_gantt_relative_ref_symbols(end_data, statement_span, facts);
        }
        [id, start_data, end_data] => {
            push_gantt_id_symbol(
                *id,
                "gantt task",
                EditorSemanticKind::Variable,
                statement_span,
                facts,
            );
            push_gantt_relative_ref_symbols(start_data, statement_span, facts);
            push_gantt_relative_ref_symbols(end_data, statement_span, facts);
        }
        _ => {}
    }
}

fn is_gantt_task_tag(text: &str) -> bool {
    matches!(text, "active" | "done" | "crit" | "milestone" | "vert")
}

fn push_gantt_relative_ref_symbols(
    field: &SpannedText<'_>,
    statement_span: SourceSpan,
    facts: &mut EditorSemanticFacts,
) {
    for keyword in ["after", "until"] {
        let Some(range) = relative_ref_ids_range(field.text, keyword) else {
            continue;
        };
        push_gantt_delimited_id_symbols(
            &field.text[range.clone()],
            field.start + range.start,
            ' ',
            "gantt dependency",
            EditorSemanticKind::Event,
            statement_span,
            facts,
        );
    }
}

fn push_gantt_delimited_id_symbols(
    text: &str,
    text_start: usize,
    delimiter: char,
    detail: &str,
    kind: EditorSemanticKind,
    statement_span: SourceSpan,
    facts: &mut EditorSemanticFacts,
) {
    let mut segment_start = 0usize;
    for (idx, ch) in text.char_indices() {
        if ch == delimiter {
            push_gantt_id_symbol(
                SpannedText {
                    text: &text[segment_start..idx],
                    start: text_start + segment_start,
                    end: text_start + idx,
                },
                detail,
                kind,
                statement_span,
                facts,
            );
            segment_start = idx + ch.len_utf8();
        }
    }

    push_gantt_id_symbol(
        SpannedText {
            text: &text[segment_start..],
            start: text_start + segment_start,
            end: text_start + text.len(),
        },
        detail,
        kind,
        statement_span,
        facts,
    );
}

fn push_gantt_id_symbol(
    field: SpannedText<'_>,
    detail: &str,
    kind: EditorSemanticKind,
    statement_span: SourceSpan,
    facts: &mut EditorSemanticFacts,
) {
    let Some(field) = field.trim() else {
        return;
    };
    facts.push_symbol(EditorSemanticSymbol::new(
        field.text,
        Some(detail.to_string()),
        kind,
        statement_span,
        field.span(),
    ));
}

fn split_gantt_fields(text: &str, text_start: usize) -> Vec<SpannedText<'_>> {
    let mut out = Vec::new();
    let mut field_start = 0usize;
    for (idx, ch) in text.char_indices() {
        if ch == ',' {
            out.push(SpannedText {
                text: &text[field_start..idx],
                start: text_start + field_start,
                end: text_start + idx,
            });
            field_start = idx + ch.len_utf8();
        }
    }

    out.push(SpannedText {
        text: &text[field_start..],
        start: text_start + field_start,
        end: text_start + text.len(),
    });
    out
}

#[derive(Debug, Clone, Copy)]
struct SpannedText<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

impl<'a> SpannedText<'a> {
    fn trim(self) -> Option<Self> {
        let leading = self.text.len().saturating_sub(self.text.trim_start().len());
        let text = &self.text[leading..];
        let trimmed_len = text.trim_end().len();
        if trimmed_len == 0 {
            return None;
        }

        Some(Self {
            text: &text[..trimmed_len],
            start: self.start + leading,
            end: self.start + leading + trimmed_len,
        })
    }

    fn span(self) -> SourceSpan {
        SourceSpan::new(self.start, self.end)
    }
}

pub fn parse_gantt(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let Some(db) = parse_gantt_db(code, meta)? else {
        return Ok(json!({}));
    };
    gantt_db_to_json(db, meta)
}

pub fn parse_gantt_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<GanttDiagramRenderModel> {
    let Some(mut db) = parse_gantt_db(code, meta)? else {
        return Ok(GanttDiagramRenderModel::default());
    };
    gantt_db_to_render_model(&mut db)
}

fn parse_gantt_db(code: &str, meta: &ParseMetadata) -> Result<Option<GanttDb>> {
    let mut db = GanttDb::default();
    db.clear();
    db.set_security_level(meta.effective_config.get_str("securityLevel"));
    if let Some(dm) = meta.effective_config.get_str("gantt.displayMode") {
        db.set_display_mode(dm);
    }

    let mut lines = code.lines();
    let mut header_seen = false;

    while let Some(line) = lines.next() {
        let stripped = strip_inline_comment(line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !header_seen {
            if starts_with_ci(trimmed, "gantt") {
                header_seen = true;
                let rest = trimmed["gantt".len()..].trim_start();
                if !rest.is_empty() {
                    parse_gantt_statement(rest, &mut db, &mut lines)?;
                }
                continue;
            }
            return Err(Error::DiagramParse {
                diagram_type: "gantt".to_string(),
                message: "expected gantt header".to_string(),
            });
        }

        parse_gantt_statement(stripped, &mut db, &mut lines)?;
    }

    if !header_seen {
        return Ok(None);
    }

    Ok(Some(db))
}

fn gantt_db_to_json(mut db: GanttDb, meta: &ParseMetadata) -> Result<Value> {
    let tasks = db.get_tasks()?;
    let tasks_json: Vec<Value> = tasks
        .into_iter()
        .map(|t| {
            let start_ms = t.start_time.map(|d| d.timestamp_millis());
            let end_ms = t.end_time.map(|d| d.timestamp_millis());
            let render_end_ms = t.render_end_time.map(|d| d.timestamp_millis());
            let raw_start = match &t.raw.start_time {
                StartTimeRaw::PrevTaskEnd => json!({ "type": "prevTaskEnd", "id": t.prev_task_id }),
                StartTimeRaw::GetStartDate { start_data } => {
                    json!({ "type": "getStartDate", "startData": start_data })
                }
            };
            json!({
                "section": t.section,
                "type": t.type_,
                "task": t.task,
                "id": t.id,
                "prevTaskId": t.prev_task_id,
                "order": t.order,
                "processed": t.processed,
                "classes": t.classes,
                "active": t.active,
                "done": t.done,
                "crit": t.crit,
                "milestone": t.milestone,
                "vert": t.vert,
                "manualEndTime": t.manual_end_time,
                "renderEndTime": render_end_ms,
                "raw": {
                    "data": t.raw.data,
                    "startTime": raw_start,
                    "endTime": { "data": t.raw.end_data },
                },
                "startTime": start_ms,
                "endTime": end_ms,
            })
        })
        .collect();

    Ok(json!({
        "type": meta.diagram_type,
        "title": if db.diagram_title.is_empty() { None::<String> } else { Some(db.diagram_title) },
        "accTitle": if db.acc_title.is_empty() { None::<String> } else { Some(db.acc_title) },
        "accDescr": if db.acc_descr.is_empty() { None::<String> } else { Some(db.acc_descr) },
        "dateFormat": db.date_format,
        "axisFormat": db.axis_format,
        "tickInterval": db.tick_interval,
        "todayMarker": db.today_marker,
        "includes": db.includes,
        "excludes": db.excludes,
        "inclusiveEndDates": db.inclusive_end_dates,
        "topAxis": db.top_axis,
        "weekday": db.weekday,
        "weekend": db.weekend,
        "displayMode": db.display_mode,
        "sections": db.sections,
        "tasks": tasks_json,
        "links": db.links,
        "clickEvents": db.click_events,
    }))
}

fn gantt_db_to_render_model(db: &mut GanttDb) -> Result<GanttDiagramRenderModel> {
    let tasks = db
        .get_tasks()?
        .into_iter()
        .map(raw_task_to_render_task)
        .collect::<Result<Vec<_>>>()?;

    Ok(GanttDiagramRenderModel {
        title: non_empty_opt(std::mem::take(&mut db.diagram_title)),
        acc_title: non_empty_opt(std::mem::take(&mut db.acc_title)),
        acc_descr: non_empty_opt(std::mem::take(&mut db.acc_descr)),
        date_format: std::mem::take(&mut db.date_format),
        axis_format: std::mem::take(&mut db.axis_format),
        tick_interval: db.tick_interval.take(),
        today_marker: std::mem::take(&mut db.today_marker),
        includes: std::mem::take(&mut db.includes),
        excludes: std::mem::take(&mut db.excludes),
        display_mode: std::mem::take(&mut db.display_mode),
        top_axis: db.top_axis,
        weekday: std::mem::take(&mut db.weekday),
        weekend: std::mem::take(&mut db.weekend),
        tasks,
    })
}

fn non_empty_opt(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

fn raw_task_to_render_task(t: RawTask) -> Result<GanttRenderTask> {
    let start_ms = task_time_ms(&t, "startTime", t.start_time)?;
    let end_ms = task_time_ms(&t, "endTime", t.end_time)?;

    Ok(GanttRenderTask {
        id: t.id,
        task: t.task,
        section: t.section,
        task_type: t.type_,
        classes: t.classes,
        active: t.active,
        done: t.done,
        crit: t.crit,
        milestone: t.milestone,
        vert: t.vert,
        order: t.order,
        start_ms,
        end_ms,
        render_end_ms: t.render_end_time.map(|d| d.timestamp_millis()),
    })
}

fn task_time_ms(task: &RawTask, field: &str, value: Option<DateTimeFixed>) -> Result<i64> {
    value
        .map(|d| d.timestamp_millis())
        .ok_or_else(|| Error::DiagramParse {
            diagram_type: "gantt".to_string(),
            message: format!("task `{}` has unresolved {field}", task.id),
        })
}

fn parse_gantt_statement(
    line: &str,
    db: &mut GanttDb,
    lines: &mut std::str::Lines<'_>,
) -> Result<()> {
    let stripped = strip_inline_comment(line);
    let t = stripped.trim();
    if t.is_empty() {
        return Ok(());
    }

    if let Some(v) = parse_keyword_arg(stripped, "dateFormat") {
        db.set_date_format(v);
        return Ok(());
    }
    if starts_with_ci(t, "inclusiveEndDates") {
        db.enable_inclusive_end_dates();
        return Ok(());
    }
    if starts_with_ci(t, "topAxis") {
        db.enable_top_axis();
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg(stripped, "axisFormat") {
        db.set_axis_format(v);
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg(stripped, "tickInterval") {
        db.set_tick_interval(v.trim());
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg(stripped, "includes") {
        db.set_includes(v);
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg(stripped, "excludes") {
        db.set_excludes(v);
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg_full_line(stripped, "todayMarker") {
        db.set_today_marker(v.trim());
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg_full_line(stripped, "weekday") {
        let day = v.trim().to_lowercase();
        if !matches!(
            day.as_str(),
            "monday" | "tuesday" | "wednesday" | "thursday" | "friday" | "saturday" | "sunday"
        ) {
            return Err(Error::DiagramParse {
                diagram_type: "gantt".to_string(),
                message: format!("invalid weekday: {day}"),
            });
        }
        db.set_weekday(&day);
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg_full_line(stripped, "weekend") {
        let day = v.trim().to_lowercase();
        if !matches!(day.as_str(), "friday" | "saturday") {
            return Err(Error::DiagramParse {
                diagram_type: "gantt".to_string(),
                message: format!("invalid weekend: {day}"),
            });
        }
        db.set_weekend(&day);
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg_full_line(stripped, "title") {
        db.set_diagram_title(v.trim());
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg_full_line(stripped, "section") {
        db.add_section(v.trim());
        return Ok(());
    }
    if let Some(v) = parse_key_colon_value(stripped, "accTitle") {
        db.set_acc_title(&v);
        return Ok(());
    }
    if let Some(v) = parse_key_colon_value(stripped, "accDescr") {
        db.set_acc_descr(&v);
        return Ok(());
    }
    if let Some(v) = parse_acc_descr_block(lines, stripped) {
        db.set_acc_descr(&v);
        return Ok(());
    }
    if let Some((ids, href, call)) = parse_click_statement(stripped) {
        if let Some((name, args)) = call {
            db.set_click_event(&ids, &name, args.as_deref());
        }
        if let Some(href) = href {
            db.set_link(&ids, &href);
        }
        return Ok(());
    }

    let task_stmt = stripped.trim_start();

    let Some(colon) = task_stmt.find(':') else {
        return Err(Error::DiagramParse {
            diagram_type: "gantt".to_string(),
            message: format!("unrecognized statement: {t}"),
        });
    };

    // Mermaid passes `taskTxt` through to the DB without trimming. This preserves any trailing
    // whitespace before the `:` delimiter (e.g. `Task1 :id,...` yields `Task1 `).
    let task_txt = &task_stmt[..colon];
    let mut task_data = task_stmt[colon + 1..].to_string();
    task_data = split_statement_suffix(&task_data).to_string();
    if task_txt.is_empty() || task_data.trim().is_empty() {
        return Ok(());
    }
    db.add_task(task_txt, &format!(":{task_data}"));
    Ok(())
}
