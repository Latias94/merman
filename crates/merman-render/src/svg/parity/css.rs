use super::*;

// Shared Mermaid diagram CSS fragments (split from legacy.rs).
//
// Keep Mermaid@11.12.2 ordering quirks to preserve DOM parity.

pub(super) fn info_css(diagram_id: &str) -> String {
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:16px;fill:#333;}}"#,
        id, font
    );
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:#552222;}}#{} .error-text{{fill:#552222;stroke:#552222;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:#333333;stroke:#333333;}}#{} .marker.cross{{stroke:#333333;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:16px;}}#{} p{{margin:0;}}#{} :root{{--mermaid-font-family:{};}}"#,
        id, font, id, id, font
    );
    out
}

pub(super) fn requirement_css(diagram_id: &str) -> String {
    // Mirrors Mermaid@11.12.2 `diagrams/requirement/styles.js` + shared base stylesheet ordering.
    // Keep `:root` last (matches upstream fixtures).
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:16px;fill:#333;}}"#,
        id, font
    );
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:#552222;}}#{} .error-text{{fill:#552222;stroke:#552222;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:#333333;stroke:#333333;}}#{} .marker.cross{{stroke:#333333;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:16px;}}#{} p{{margin:0;}}"#,
        id, font, id
    );

    // Requirement diagram styles (duplicated marker/svg rules are present upstream).
    let _ = write!(
        &mut out,
        r#"#{} marker{{fill:#333333;stroke:#333333;}}#{} marker.cross{{stroke:#333333;}}#{} svg{{font-family:{};font-size:16px;}}"#,
        id, id, id, font
    );
    let _ = write!(
        &mut out,
        r#"#{} .reqBox{{fill:#ECECFF;fill-opacity:1.0;stroke:hsl(240, 60%, 86.2745098039%);stroke-width:1;}}#{} .reqTitle,#{} .reqLabel{{fill:#131300;}}#{} .reqLabelBox{{fill:rgba(232,232,232, 0.8);fill-opacity:1.0;}}#{} .req-title-line{{stroke:hsl(240, 60%, 86.2745098039%);stroke-width:1;}}#{} .relationshipLine{{stroke:#333333;stroke-width:1;}}#{} .relationshipLabel{{fill:black;}}#{} .divider{{stroke:#9370DB;stroke-width:1;}}#{} .label{{font-family:{};color:#333;}}#{} .label text,#{} span{{fill:#333;color:#333;}}#{} .labelBkg{{background-color:rgba(232,232,232, 0.8);}}"#,
        id, id, id, id, id, id, id, id, id, font, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} :root{{--mermaid-font-family:{};}}"#,
        id, font
    );
    out
}

pub(super) fn er_css(diagram_id: &str) -> String {
    // Mirrors Mermaid@11.12.2 ER unified renderer stylesheet ordering (see `diagrams/er/styles.js`
    // and shared base stylesheet).
    // Keep `:root` last (matches upstream fixtures).
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:16px;fill:#333;}}"#,
        id, font
    );
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:#552222;}}#{} .error-text{{fill:#552222;stroke:#552222;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:#333333;stroke:#333333;}}#{} .marker.cross{{stroke:#333333;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:16px;}}#{} p{{margin:0;}}"#,
        id, font, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .entityBox{{fill:#ECECFF;stroke:#9370DB;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .relationshipLabelBox{{fill:hsl(80, 100%, 96.2745098039%);opacity:0.7;background-color:hsl(80, 100%, 96.2745098039%);}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .relationshipLabelBox rect{{opacity:0.5;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .labelBkg{{background-color:rgba(248.6666666666, 255, 235.9999999999, 0.5);}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .edgeLabel .label{{fill:#9370DB;font-size:14px;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .label{{font-family:{};color:#333;}}"#,
        id, font
    );
    // Mermaid duplicates `.edge-pattern-dashed` (base rule earlier sets dasharray:3).
    let _ = write!(
        &mut out,
        r#"#{} .edge-pattern-dashed{{stroke-dasharray:8,8;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .node rect,#{} .node circle,#{} .node ellipse,#{} .node polygon{{fill:#ECECFF;stroke:#9370DB;stroke-width:1px;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .relationshipLine{{stroke:#333333;stroke-width:1;fill:none;}}"#,
        id
    );
    // Mermaid duplicates `.marker` (base rule earlier sets fill/stroke to #333333).
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:none!important;stroke:#333333!important;stroke-width:1;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} :root{{--mermaid-font-family:{};}}"#,
        id, font
    );
    out
}

pub(super) fn pie_css(diagram_id: &str) -> String {
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let mut out = info_css(diagram_id);
    let _ = write!(
        &mut out,
        r#"#{} .pieCircle{{stroke:black;stroke-width:2px;opacity:0.7;}}#{} .pieOuterCircle{{stroke:black;stroke-width:2px;fill:none;}}#{} .pieTitleText{{text-anchor:middle;font-size:25px;fill:black;font-family:{};}}#{} .slice{{font-family:{};fill:#333;font-size:17px;}}#{} .legend text{{fill:black;font-family:{};font-size:17px;}}"#,
        id, id, id, font, id, font, id, font
    );
    out
}

pub(super) fn sankey_css(diagram_id: &str) -> String {
    // Mermaid's sankey diagram uses the same base CSS as "info-like" diagrams, plus a `.label`
    // rule, and keeps `:root` last.
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:16px;fill:#333;}}"#,
        id, font
    );
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:#552222;}}#{} .error-text{{fill:#552222;stroke:#552222;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:#333333;stroke:#333333;}}#{} .marker.cross{{stroke:#333333;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:16px;}}#{} p{{margin:0;}}#{} .label{{font-family:{};}}#{} :root{{--mermaid-font-family:{};}}"#,
        id, font, id, id, font, id, font
    );
    out
}

pub(super) fn treemap_css(diagram_id: &str) -> String {
    // Keep `:root` last (matches upstream Mermaid treemap SVG baselines).
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:16px;fill:#333;}}"#,
        id, font
    );
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:#552222;}}#{} .error-text{{fill:#552222;stroke:#552222;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:#333333;stroke:#333333;}}#{} .marker.cross{{stroke:#333333;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:16px;}}#{} p{{margin:0;}}"#,
        id, font, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .treemapNode.section{{stroke:black;stroke-width:1;fill:#efefef;}}#{} .treemapNode.leaf{{stroke:black;stroke-width:1;fill:#efefef;}}#{} .treemapLabel{{fill:black;font-size:12px;}}#{} .treemapValue{{fill:black;font-size:10px;}}#{} .treemapTitle{{fill:black;font-size:14px;}}"#,
        id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} :root{{--mermaid-font-family:{};}}"#,
        id, font
    );
    out
}

pub(super) fn xychart_css(diagram_id: &str) -> String {
    // Mermaid does not ship dedicated XYChart styles at 11.12.2 (it relies on theme variables and
    // inline attributes). Keep the shared base stylesheet for consistency with upstream SVG
    // baselines. The compare tooling ignores `<style>` content in parity mode.
    info_css(diagram_id)
}

pub(super) fn gantt_css(diagram_id: &str) -> String {
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let root_rule = format!(r#"#{} :root{{--mermaid-font-family:{};}}"#, id, font);
    let mut out = info_css(diagram_id);
    if let Some(prefix) = out.strip_suffix(&root_rule) {
        out = prefix.to_string();
    }

    let _ = write!(
        &mut out,
        r#"#{} .mermaid-main-font{{font-family:{};}}"#,
        id, font
    );
    let _ = write!(&mut out, r#"#{} .exclude-range{{fill:#eeeeee;}}"#, id);
    let _ = write!(&mut out, r#"#{} .section{{stroke:none;opacity:0.2;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} .section0{{fill:rgba(102, 102, 255, 0.49);}}"#,
        id
    );
    let _ = write!(&mut out, r#"#{} .section2{{fill:#fff400;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} .section1,#{} .section3{{fill:white;opacity:0.2;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .sectionTitle0{{fill:#333;}}#{} .sectionTitle1{{fill:#333;}}#{} .sectionTitle2{{fill:#333;}}#{} .sectionTitle3{{fill:#333;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .sectionTitle{{text-anchor:start;font-family:{};}}"#,
        id, font
    );
    let _ = write!(
        &mut out,
        r#"#{} .grid .tick{{stroke:lightgrey;opacity:0.8;shape-rendering:crispEdges;}}#{} .grid .tick text{{font-family:{};fill:#333;}}#{} .grid path{{stroke-width:0;}}"#,
        id, id, font, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .today{{fill:none;stroke:red;stroke-width:2px;}}"#,
        id
    );
    let _ = write!(&mut out, r#"#{} .task{{stroke-width:2;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} .taskText{{text-anchor:middle;font-family:{};}}#{} .taskTextOutsideRight{{fill:black;text-anchor:start;font-family:{};}}#{} .taskTextOutsideLeft{{fill:black;text-anchor:end;}}"#,
        id, font, id, font, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .task.clickable{{cursor:pointer;}}#{} .taskText.clickable{{cursor:pointer;fill:#003163!important;font-weight:bold;}}#{} .taskTextOutsideLeft.clickable{{cursor:pointer;fill:#003163!important;font-weight:bold;}}#{} .taskTextOutsideRight.clickable{{cursor:pointer;fill:#003163!important;font-weight:bold;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .taskText0,#{} .taskText1,#{} .taskText2,#{} .taskText3{{fill:white;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .task0,#{} .task1,#{} .task2,#{} .task3{{fill:#8a90dd;stroke:#534fbc;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .taskTextOutside0,#{} .taskTextOutside2{{fill:black;}}#{} .taskTextOutside1,#{} .taskTextOutside3{{fill:black;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .active0,#{} .active1,#{} .active2,#{} .active3{{fill:#bfc7ff;stroke:#534fbc;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .activeText0,#{} .activeText1,#{} .activeText2,#{} .activeText3{{fill:black!important;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .done0,#{} .done1,#{} .done2,#{} .done3{{stroke:grey;fill:lightgrey;stroke-width:2;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .doneText0,#{} .doneText1,#{} .doneText2,#{} .doneText3{{fill:black!important;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .crit0,#{} .crit1,#{} .crit2,#{} .crit3{{stroke:#ff8888;fill:red;stroke-width:2;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .activeCrit0,#{} .activeCrit1,#{} .activeCrit2,#{} .activeCrit3{{stroke:#ff8888;fill:#bfc7ff;stroke-width:2;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .doneCrit0,#{} .doneCrit1,#{} .doneCrit2,#{} .doneCrit3{{stroke:#ff8888;fill:lightgrey;stroke-width:2;cursor:pointer;shape-rendering:crispEdges;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .milestone{{transform:rotate(45deg) scale(0.8,0.8);}}#{} .milestoneText{{font-style:italic;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .doneCritText0,#{} .doneCritText1,#{} .doneCritText2,#{} .doneCritText3{{fill:black!important;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .vert{{stroke:navy;}}#{} .vertText{{font-size:15px;text-anchor:middle;fill:navy!important;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .activeCritText0,#{} .activeCritText1,#{} .activeCritText2,#{} .activeCritText3{{fill:black!important;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .titleText{{text-anchor:middle;font-size:18px;fill:#333;font-family:{};}}"#,
        id, font
    );

    out.push_str(&root_rule);
    out
}
