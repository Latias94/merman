fn node_render_dimensions(
    layout_shape: Option<&str>,
    metrics: crate::text::TextMetrics,
    padding: f64,
) -> (f64, f64) {
    // This function mirrors Mermaid `@11.12.2` node shape sizing rules at the "rendering-elements"
    // layer, but uses our headless `TextMeasurer` metrics instead of DOM `getBBox()`.
    //
    // References:
    // - `packages/mermaid/src/diagrams/flowchart/flowDb.ts` (shape assignment + padding)
    // - `packages/mermaid/src/rendering-util/rendering-elements/shapes/*.ts` (shape bounds)
    // Mermaid's DOM `getBBox()` can legitimately return 0 for empty/whitespace-only labels.
    // Do not clamp to 1px here, otherwise we skew layout widths (notably `max-width`) by 1px.
    let text_w = metrics.width.max(0.0);
    let text_h = metrics.height.max(0.0);
    let p = padding.max(0.0);

    let shape = layout_shape.unwrap_or("squareRect");

    fn circle_points(
        center_x: f64,
        center_y: f64,
        radius: f64,
        num_points: usize,
        start_deg: f64,
        end_deg: f64,
        negate: bool,
    ) -> Vec<(f64, f64)> {
        let start = start_deg.to_radians();
        let end = end_deg.to_radians();
        let angle_range = end - start;
        let angle_step = if num_points > 1 {
            angle_range / (num_points as f64 - 1.0)
        } else {
            0.0
        };
        let mut out: Vec<(f64, f64)> = Vec::with_capacity(num_points);
        for i in 0..num_points {
            let a = start + (i as f64) * angle_step;
            let x = center_x + radius * a.cos();
            let y = center_y + radius * a.sin();
            if negate {
                out.push((-x, -y));
            } else {
                out.push((x, y));
            }
        }
        out
    }

    fn bbox_of_points(points: &[(f64, f64)]) -> Option<(f64, f64, f64, f64)> {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for &(x, y) in points {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
            Some((min_x, min_y, max_x, max_y))
        } else {
            None
        }
    }

    fn f32_dims(w: f64, h: f64) -> (f64, f64) {
        let w_f32 = w as f32;
        let h_f32 = h as f32;
        if w_f32.is_finite()
            && h_f32.is_finite()
            && w_f32.is_sign_positive()
            && h_f32.is_sign_positive()
        {
            (w_f32 as f64, h_f32 as f64)
        } else {
            (w.max(0.0), h.max(0.0))
        }
    }

    fn generate_full_sine_wave_points(
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        amplitude: f64,
        num_cycles: f64,
    ) -> Vec<(f64, f64)> {
        // Ported from Mermaid `generateFullSineWavePoints` (50 segments).
        let steps: usize = 50;
        let delta_x = x2 - x1;
        let delta_y = y2 - y1;
        let cycle_length = if num_cycles.abs() < 1e-9 {
            delta_x
        } else {
            delta_x / num_cycles
        };
        let frequency = if cycle_length.abs() < 1e-9 {
            0.0
        } else {
            (2.0 * std::f64::consts::PI) / cycle_length
        };
        let mid_y = y1 + delta_y / 2.0;

        let mut points: Vec<(f64, f64)> = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let t = (i as f64) / (steps as f64);
            let x = x1 + t * delta_x;
            let y = mid_y + amplitude * (frequency * (x - x1)).sin();
            points.push((x, y));
        }
        points
    }

    match shape {
        // Default flowchart process node.
        "squareRect" => (text_w + 4.0 * p, text_h + 2.0 * p),

        // Mermaid uses a few aliases for the same rounded-rectangle shape across layers.
        // In FlowDB output (flowchart-v2), this commonly appears as `rounded`.
        "roundedRect" | "rounded" | "event" => (text_w + 2.0 * p, text_h + 2.0 * p),

        // Note (rendering-elements/state note box).
        "note" => (text_w + 2.0 * p, text_h + 2.0 * p),

        // Diamond (decision/question).
        "diamond" | "question" | "diam" | "decision" => {
            let w = text_w + p;
            let h = text_h + p;
            let s = w + h;
            (s, s)
        }

        // Hexagon.
        "hexagon" | "hex" | "prepare" => {
            let h = text_h + p;
            let w0 = text_w + 2.5 * p;
            // Match Mermaid@11.12.2 evaluation order:
            // `halfWidth = w / 2; m = halfWidth / 6; halfWidth += m; width = 2 * halfWidth`.
            let mut half_width = w0 / 2.0;
            let m = half_width / 6.0;
            half_width += m;
            (half_width * 2.0, h)
        }

        // Stadium/terminator.
        "stadium" | "terminal" | "pill" => {
            let h = text_h + p;
            let w = text_w + h / 4.0 + p;
            (w, h)
        }

        // Subroutine/subprocess (framed rectangle): adds an 8px "frame" on both sides.
        "subroutine" | "fr-rect" | "subproc" | "subprocess" | "framed-rectangle" => {
            let w = text_w + p;
            let h = text_h + p;
            (w + 16.0, h)
        }

        // Cylinder/database.
        "cylinder" | "cyl" | "db" | "database" => {
            let w = text_w + p;
            let rx = w / 2.0;
            let ry = rx / (2.5 + w / 50.0);
            // Mermaid's cylinder path height ends up including two extra `ry` from the ellipses.
            // See `createCylinderPathD` + `translate(..., -(h/2 + ry))`.
            let height = text_h + p + 3.0 * ry;
            (w, height)
        }

        // Flowchart v2 tilted cylinder ("horizontal-cylinder").
        "h-cyl" | "das" | "horizontal-cylinder" => {
            // Mermaid `tiltedCylinder.ts`:
            // - `labelPadding` defaults to `halfPadding` (i.e. `node.padding / 2`) for classic look.
            // - `h = bbox.height + labelPadding`
            // - `ry = h / 2`, `rx = ry / (2.5 + h / 50)`
            // - `w = bbox.width + rx + labelPadding`
            // - the rendered `<path>` bbox expands by `rx` on both sides (arc extents), so Dagre
            //   sees `out_w = w + 2*rx` via `updateNodeBounds(...)`.
            let label_padding = p / 2.0;
            let h = text_h + label_padding;
            let ry = h / 2.0;
            let rx = if ry == 0.0 {
                0.0
            } else {
                ry / (2.5 + h / 50.0)
            };
            let w = text_w + rx + label_padding;
            (w + 2.0 * rx, h)
        }

        // Flowchart v2 window-pane ("internal-storage").
        "win-pane" | "internal-storage" | "window-pane" => {
            // Mermaid `windowPane.ts`: base `w/h` uses `2 * padding`, then the final bbox expands
            // by `rectOffset` (only on the top/left edges) after `updateNodeBounds(...)`.
            let w = (text_w + 2.0 * p).max(0.0);
            let h = (text_h + 2.0 * p).max(0.0);
            let rect_offset = 5.0;
            f32_dims(w + rect_offset, h + rect_offset)
        }

        // Circle.
        "circle" | "circ" => {
            // Mermaid uses half-padding for circles and bases radius on label width.
            let d = text_w + p;
            (d, d)
        }

        // Double circle.
        "doublecircle" | "dbl-circ" | "double-circle" => {
            // `gap = 5` is hard-coded in Mermaid.
            let d = text_w + p + 10.0;
            (d, d)
        }

        // Small start circle (stateStart in rendering-elements).
        "sm-circ" | "small-circle" | "start" => (14.0, 14.0),

        // Stop framed circle (stateEnd in rendering-elements).
        //
        // Mermaid renders this through RoughJS' ellipse path and then uses `getBBox()` for Dagre.
        // Chromium's bbox for the generated path is slightly wider than 14px at 11.12.2.
        "fr-circ" | "framed-circle" | "stop" => (14.013_293_266_296_387, 14.0),

        // Fork/join bar (uses `lineColor` fill/stroke; no label).
        "fork" | "join" => (70.0, 10.0),

        // Choice diamond (stateChoice in rendering-elements).
        "choice" => (28.0, 28.0),

        // Flowchart v2 lightning bolt (Communication link). Mermaid clears `node.label`.
        "bolt" | "com-link" | "lightning-bolt" => (35.0, 70.0),

        // Flowchart v2 filled circle (junction). Mermaid clears `node.label`.
        // Width comes from RoughJS `circle` bbox at 11.12.2.
        "f-circ" | "junction" | "filled-circle" => (14.013_293_266_296_387, 14.0),

        // Flowchart v2 crossed circle (summary). Mermaid clears `node.label`.
        // Width comes from RoughJS `circle` bbox at 11.12.2 with radius=30.
        "cross-circ" | "summary" | "crossed-circle" => (60.056_972_503_662_11, 60.0),

        // Flowchart v2 delay / halfRoundedRectangle (rendering-elements).
        "delay" | "half-rounded-rectangle" => {
            let min_width = 80.0;
            let min_height = 50.0;
            let w = (text_w + 2.0 * p).max(min_width);
            let h = (text_h + 2.0 * p).max(min_height);
            let radius = h / 2.0;
            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((-w / 2.0, -h / 2.0));
            points.push((w / 2.0 - radius, -h / 2.0));
            points.extend(circle_points(
                -w / 2.0 + radius,
                0.0,
                radius,
                50,
                90.0,
                270.0,
                true,
            ));
            points.push((w / 2.0 - radius, h / 2.0));
            points.push((-w / 2.0, h / 2.0));
            let (min_x, min_y, max_x, max_y) =
                bbox_of_points(&points).unwrap_or((-w / 2.0, -h / 2.0, w / 2.0, h / 2.0));
            f32_dims((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Flowchart v2 lined cylinder (Disk storage).
        "lin-cyl" | "disk" | "lined-cylinder" => {
            let w = text_w + p;
            let rx = w / 2.0;
            let ry = rx / (2.5 + w / 50.0);
            let height = text_h + p + 3.0 * ry;
            f32_dims(w, height)
        }

        // Flowchart v2 curved trapezoid (Display).
        "curv-trap" | "display" | "curved-trapezoid" => {
            let min_width = 80.0;
            let min_height = 20.0;
            let w = (text_w + 2.0 * p).mul_add(1.25, 0.0).max(min_width);
            let h = (text_h + 2.0 * p).max(min_height);
            let radius = h / 2.0;
            let total_width = w;
            let total_height = h;
            let rw = total_width - radius;
            let tw = total_height / 4.0;

            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((rw, 0.0));
            points.push((tw, 0.0));
            points.push((0.0, total_height / 2.0));
            points.push((tw, total_height));
            points.push((rw, total_height));
            points.extend(circle_points(
                -rw,
                -total_height / 2.0,
                radius,
                50,
                270.0,
                90.0,
                true,
            ));

            let (min_x, min_y, max_x, max_y) =
                bbox_of_points(&points).unwrap_or((0.0, 0.0, total_width, total_height));
            f32_dims((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Flowchart v2 divided rectangle (Divided process).
        "div-rect" | "div-proc" | "divided-rectangle" | "divided-process" => {
            let w = text_w + p;
            let h = text_h + p;
            let rect_offset = h * 0.2;
            let x = -w / 2.0;
            let y = -h / 2.0 - rect_offset / 2.0;
            let points: Vec<(f64, f64)> = vec![
                (x, y + rect_offset),
                (-x, y + rect_offset),
                (-x, -y),
                (x, -y),
                (x, y),
                (-x, y),
                (-x, y + rect_offset),
            ];
            let (min_x, min_y, max_x, max_y) = bbox_of_points(&points).unwrap_or((x, y, -x, -y));
            f32_dims((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Flowchart v2 triangle (Extract).
        "tri" | "extract" | "triangle" => {
            let w = text_w + p;
            let h = w + text_h;
            f32_dims(h, h)
        }

        // Flowchart v2 flipped triangle (Manual file).
        "manual-file" | "flipped-triangle" | "flip-tri" => {
            let w = text_w + p;
            let h = w + text_h;
            f32_dims(h, h)
        }

        // Flowchart v2 sloped rectangle (Manual input).
        "manual-input" | "sloped-rectangle" | "sl-rect" => {
            let w = (text_w + 2.0 * p).max(0.0);
            let h = (text_h + 2.0 * p).max(0.0);
            f32_dims(w, (1.5 * h).max(0.0))
        }

        // Flowchart v2 document (wave-edged rectangle).
        "doc" | "document" => {
            let w = (text_w + 2.0 * p).max(0.0);
            let h = (text_h + 2.0 * p).max(0.0);
            let wave_amplitude = h / 8.0;
            let final_h = h + wave_amplitude;
            let min_width = 70.0;
            let extra_w = if w < min_width {
                (min_width - w) / 2.0
            } else {
                0.0
            };

            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((-w / 2.0 - extra_w, final_h / 2.0));
            points.extend(generate_full_sine_wave_points(
                -w / 2.0 - extra_w,
                final_h / 2.0,
                w / 2.0 + extra_w,
                final_h / 2.0,
                wave_amplitude,
                0.8,
            ));
            points.push((w / 2.0 + extra_w, -final_h / 2.0));
            points.push((-w / 2.0 - extra_w, -final_h / 2.0));

            let (min_x, min_y, max_x, max_y) = bbox_of_points(&points).unwrap_or((
                -w / 2.0,
                -final_h / 2.0,
                w / 2.0,
                final_h / 2.0,
            ));
            f32_dims((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Flowchart v2 stacked document (multi-wave edged rectangle).
        "docs" | "documents" | "st-doc" | "stacked-document" => {
            let w = (text_w + 2.0 * p).max(0.0);
            let h = (text_h + 2.0 * p).max(0.0);
            let wave_amplitude = h / 4.0;
            let final_h = h + wave_amplitude;
            let rect_offset = 5.0;
            let x = -w / 2.0;
            let y = -final_h / 2.0;

            let wave_points = generate_full_sine_wave_points(
                x - rect_offset,
                y + final_h + rect_offset,
                x + w - rect_offset,
                y + final_h + rect_offset,
                wave_amplitude,
                0.8,
            );
            let (_last_x, last_y) = wave_points[wave_points.len() - 1];

            let mut outer_points: Vec<(f64, f64)> = Vec::new();
            outer_points.push((x - rect_offset, y + rect_offset));
            outer_points.push((x - rect_offset, y + final_h + rect_offset));
            outer_points.extend(wave_points.iter().copied());
            outer_points.push((x + w - rect_offset, last_y - rect_offset));
            outer_points.push((x + w, last_y - rect_offset));
            outer_points.push((x + w, last_y - 2.0 * rect_offset));
            outer_points.push((x + w + rect_offset, last_y - 2.0 * rect_offset));
            outer_points.push((x + w + rect_offset, y - rect_offset));
            outer_points.push((x + rect_offset, y - rect_offset));
            outer_points.push((x + rect_offset, y));
            outer_points.push((x, y));
            outer_points.push((x, y + rect_offset));

            let (min_x, min_y, max_x, max_y) =
                bbox_of_points(&outer_points).unwrap_or((x, y, x + w, y + final_h));
            f32_dims((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Flowchart v2 stacked rectangle (multi-process).
        "st-rect" | "procs" | "processes" | "stacked-rectangle" => {
            // Mermaid `multiRect.ts`: base `w/h` uses `2 * padding` and the final bbox expands by
            // `2 * rectOffset` in both dimensions.
            let w = (text_w + 2.0 * p).max(0.0);
            let h = (text_h + 2.0 * p).max(0.0);
            let rect_offset = 5.0;
            f32_dims(w + 2.0 * rect_offset, h + 2.0 * rect_offset)
        }

        // Flowchart v2 paper-tape / wave rectangle.
        "paper-tape" | "flag" => {
            let min_width = 100.0;
            let min_height = 50.0;
            let w = (text_w + 2.0 * p).max(min_width);
            let h = (text_h + 2.0 * p).max(min_height);
            let wave_amplitude = (h * 0.2).min(h / 4.0);
            let final_h = h + wave_amplitude * 2.0;

            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((-w / 2.0, final_h / 2.0));
            points.extend(generate_full_sine_wave_points(
                -w / 2.0,
                final_h / 2.0,
                w / 2.0,
                final_h / 2.0,
                wave_amplitude,
                1.0,
            ));
            points.push((w / 2.0, -final_h / 2.0));
            points.extend(generate_full_sine_wave_points(
                w / 2.0,
                -final_h / 2.0,
                -w / 2.0,
                -final_h / 2.0,
                wave_amplitude,
                -1.0,
            ));
            let (min_x, min_y, max_x, max_y) = bbox_of_points(&points).unwrap_or((
                -w / 2.0,
                -final_h / 2.0,
                w / 2.0,
                final_h / 2.0,
            ));
            f32_dims((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Flowchart v2 lined document.
        "lin-doc" | "lined-document" => {
            let w = (text_w + 2.0 * p).max(0.0);
            let h = (text_h + 2.0 * p).max(0.0);
            let wave_amplitude = h / 4.0;
            let final_h = h + wave_amplitude;
            let extra = (w / 2.0) * 0.1;

            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((-w / 2.0 - extra, -final_h / 2.0));
            points.push((-w / 2.0 - extra, final_h / 2.0));
            points.extend(generate_full_sine_wave_points(
                -w / 2.0 - extra,
                final_h / 2.0,
                w / 2.0 + extra,
                final_h / 2.0,
                wave_amplitude,
                0.8,
            ));
            points.push((w / 2.0 + extra, -final_h / 2.0));
            points.push((-w / 2.0 - extra, -final_h / 2.0));
            points.push((-w / 2.0, -final_h / 2.0));
            points.push((-w / 2.0, (final_h / 2.0) * 1.1));
            points.push((-w / 2.0, -final_h / 2.0));

            let (min_x, min_y, max_x, max_y) = bbox_of_points(&points).unwrap_or((
                -w / 2.0,
                -final_h / 2.0,
                w / 2.0,
                final_h / 2.0,
            ));
            f32_dims((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Flowchart v2 tagged rectangle.
        "tag-rect" | "tagged-rectangle" | "tag-proc" | "tagged-process" => {
            let w = (text_w + 2.0 * p).max(0.0);
            let h = (text_h + 2.0 * p).max(0.0);
            let x = -w / 2.0;
            let y = -h / 2.0;
            let tag_width = 0.2 * h;
            let tag_height = 0.2 * h;
            let rect_points = vec![
                (x - tag_width / 2.0, y),
                (x + w + tag_width / 2.0, y),
                (x + w + tag_width / 2.0, y + h),
                (x - tag_width / 2.0, y + h),
            ];
            let tag_points = vec![
                (x + w - tag_width / 2.0, y + h),
                (x + w + tag_width / 2.0, y + h),
                (x + w + tag_width / 2.0, y + h - tag_height),
            ];
            let mut pts = rect_points;
            pts.extend(tag_points);
            let (min_x, min_y, max_x, max_y) = bbox_of_points(&pts).unwrap_or((x, y, x + w, y + h));
            f32_dims((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Flowchart v2 tagged document.
        "tag-doc" | "tagged-document" => {
            let w = (text_w + 2.0 * p).max(0.0);
            let h = (text_h + 2.0 * p).max(0.0);
            let wave_amplitude = h / 4.0;
            let final_h = h + wave_amplitude;
            let extra = (w / 2.0) * 0.1;
            let tag_width = 0.2 * w;
            let tag_height = 0.2 * h;

            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((-w / 2.0 - extra, final_h / 2.0));
            points.extend(generate_full_sine_wave_points(
                -w / 2.0 - extra,
                final_h / 2.0,
                w / 2.0 + extra,
                final_h / 2.0,
                wave_amplitude,
                0.8,
            ));
            points.push((w / 2.0 + extra, -final_h / 2.0));
            points.push((-w / 2.0 - extra, -final_h / 2.0));

            let x = -w / 2.0 + extra;
            let y = -final_h / 2.0 - tag_height * 0.4;
            let mut tag_points: Vec<(f64, f64)> = Vec::new();
            tag_points.push((x + w - tag_width, (y + h) * 1.4));
            tag_points.push((x + w, y + h - tag_height));
            tag_points.push((x + w, (y + h) * 0.9));
            tag_points.extend(generate_full_sine_wave_points(
                x + w,
                (y + h) * 1.3,
                x + w - tag_width,
                (y + h) * 1.5,
                -h * 0.03,
                0.5,
            ));

            points.extend(tag_points);
            let (min_x, min_y, max_x, max_y) = bbox_of_points(&points).unwrap_or((
                -w / 2.0,
                -final_h / 2.0,
                w / 2.0,
                final_h / 2.0,
            ));
            f32_dims((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Flowchart v2 trapezoidal pentagon (Loop limit).
        "notch-pent" | "loop-limit" | "notched-pentagon" => {
            let min_width = 60.0;
            let min_height = 20.0;
            let w = (text_w + 2.0 * p).max(min_width);
            let h = (text_h + 2.0 * p).max(min_height);
            f32_dims(w, h)
        }

        // Flowchart v2 bow-tie rect (Stored data).
        "bow-rect" | "stored-data" | "bow-tie-rectangle" => {
            let w = text_w + p + 20.0;
            let h = text_h + p;
            f32_dims(w, h)
        }

        // Hourglass/collate (label cleared, but label group still emitted).
        "hourglass" | "collate" => (30.0, 30.0),

        // Card/notched rectangle: adds a fixed 12px notch width.
        "notch-rect" | "notched-rectangle" | "card" => (text_w + p + 12.0, text_h + p),

        // Shaded process / lined rectangle: adds 8px on both sides (total +16).
        "lin-rect" | "lined-rectangle" | "lined-process" | "lin-proc" | "shaded-process" => {
            (text_w + 2.0 * p + 16.0, text_h + 2.0 * p)
        }

        // Text block: bbox + 1x padding (not 2x).
        "text" => (text_w + p, text_h + p),

        // Curly brace comment shapes (rendering-elements).
        "comment" | "brace" | "brace-l" => {
            let w = text_w + p;
            let h = text_h + p;
            let radius = (h * 0.1).max(5.0);
            let group_tx = radius;
            let mut points: Vec<(f64, f64)> = Vec::new();
            points.extend(circle_points(
                w / 2.0,
                -h / 2.0,
                radius,
                30,
                -90.0,
                0.0,
                true,
            ));
            points.push((-w / 2.0 - radius, radius));
            points.extend(circle_points(
                w / 2.0 + radius * 2.0,
                -radius,
                radius,
                20,
                -180.0,
                -270.0,
                true,
            ));
            points.extend(circle_points(
                w / 2.0 + radius * 2.0,
                radius,
                radius,
                20,
                -90.0,
                -180.0,
                true,
            ));
            points.push((-w / 2.0 - radius, -h / 2.0));
            points.extend(circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0, true));

            let mut rect_points: Vec<(f64, f64)> = Vec::new();
            rect_points.extend([(w / 2.0, -h / 2.0 - radius), (-w / 2.0, -h / 2.0 - radius)]);
            rect_points.extend(circle_points(
                w / 2.0,
                -h / 2.0,
                radius,
                20,
                -90.0,
                0.0,
                true,
            ));
            rect_points.push((-w / 2.0 - radius, -radius));
            rect_points.extend(circle_points(
                w / 2.0 + w * 0.1,
                -radius,
                radius,
                20,
                -180.0,
                -270.0,
                true,
            ));
            rect_points.extend(circle_points(
                w / 2.0 + w * 0.1,
                radius,
                radius,
                20,
                -90.0,
                -180.0,
                true,
            ));
            rect_points.push((-w / 2.0 - radius, h / 2.0));
            rect_points.extend(circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0, true));
            rect_points.extend([(-w / 2.0, h / 2.0 + radius), (w / 2.0, h / 2.0 + radius)]);
            for p in points.iter_mut().chain(rect_points.iter_mut()) {
                p.0 += group_tx;
            }
            let mut all_points: Vec<(f64, f64)> =
                Vec::with_capacity(points.len() + rect_points.len());
            all_points.extend(points);
            all_points.extend(rect_points);
            let (min_x, min_y, max_x, max_y) =
                bbox_of_points(&all_points).unwrap_or((-w / 2.0, -h / 2.0, w / 2.0, h / 2.0));
            ((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }
        "brace-r" => {
            let w = text_w + p;
            let h = text_h + p;
            let radius = (h * 0.1).max(5.0);
            let group_tx = -radius;
            let mut rect_points: Vec<(f64, f64)> = Vec::new();
            rect_points.extend([(-w / 2.0, -h / 2.0 - radius), (w / 2.0, -h / 2.0 - radius)]);
            rect_points.extend(circle_points(
                w / 2.0,
                -h / 2.0,
                radius,
                20,
                -90.0,
                0.0,
                false,
            ));
            rect_points.push((w / 2.0 + radius, -radius));
            rect_points.extend(circle_points(
                w / 2.0 + radius * 2.0,
                -radius,
                radius,
                20,
                -180.0,
                -270.0,
                false,
            ));
            rect_points.extend(circle_points(
                w / 2.0 + radius * 2.0,
                radius,
                radius,
                20,
                -90.0,
                -180.0,
                false,
            ));
            rect_points.push((w / 2.0 + radius, h / 2.0));
            rect_points.extend(circle_points(
                w / 2.0,
                h / 2.0,
                radius,
                20,
                0.0,
                90.0,
                false,
            ));
            rect_points.extend([(w / 2.0, h / 2.0 + radius), (-w / 2.0, h / 2.0 + radius)]);
            for p in &mut rect_points {
                p.0 += group_tx;
            }
            let (min_x, min_y, max_x, max_y) =
                bbox_of_points(&rect_points).unwrap_or((-w / 2.0, -h / 2.0, w / 2.0, h / 2.0));
            ((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }
        "braces" => {
            let w = text_w + p;
            let h = text_h + p;
            let radius = (h * 0.1).max(5.0);
            let group_tx = radius - radius / 4.0;
            let mut rect_points: Vec<(f64, f64)> = Vec::new();
            rect_points.extend([(w / 2.0, -h / 2.0 - radius), (-w / 2.0, -h / 2.0 - radius)]);
            rect_points.extend(circle_points(
                w / 2.0,
                -h / 2.0,
                radius,
                20,
                -90.0,
                0.0,
                true,
            ));
            rect_points.push((-w / 2.0 - radius, -radius));
            rect_points.extend(circle_points(
                w / 2.0 + radius * 2.0,
                -radius,
                radius,
                20,
                -180.0,
                -270.0,
                true,
            ));
            rect_points.extend(circle_points(
                w / 2.0 + radius * 2.0,
                radius,
                radius,
                20,
                -90.0,
                -180.0,
                true,
            ));
            rect_points.push((-w / 2.0 - radius, h / 2.0));
            rect_points.extend(circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0, true));
            rect_points.extend([
                (-w / 2.0, h / 2.0 + radius),
                (w / 2.0 - radius - radius / 2.0, h / 2.0 + radius),
            ]);
            rect_points.extend(circle_points(
                -w / 2.0 + radius + radius / 2.0,
                -h / 2.0,
                radius,
                20,
                -90.0,
                -180.0,
                true,
            ));
            rect_points.push((w / 2.0 - radius / 2.0, radius));
            rect_points.extend(circle_points(
                -w / 2.0 - radius / 2.0,
                -radius,
                radius,
                20,
                0.0,
                90.0,
                true,
            ));
            rect_points.extend(circle_points(
                -w / 2.0 - radius / 2.0,
                radius,
                radius,
                20,
                -90.0,
                0.0,
                true,
            ));
            rect_points.push((w / 2.0 - radius / 2.0, -radius));
            rect_points.extend(circle_points(
                -w / 2.0 + radius + radius / 2.0,
                h / 2.0,
                radius,
                30,
                -180.0,
                -270.0,
                true,
            ));
            for p in &mut rect_points {
                p.0 += group_tx;
            }
            let (min_x, min_y, max_x, max_y) =
                bbox_of_points(&rect_points).unwrap_or((-w / 2.0, -h / 2.0, w / 2.0, h / 2.0));
            ((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        }

        // Lean and trapezoid variants (parallelograms/trapezoids).
        "lean_right" | "lean-r" | "lean-right" | "in-out" | "lean_left" | "lean-l"
        | "lean-left" | "out-in" | "trapezoid" | "trap-b" | "priority" | "trapezoid-bottom" => {
            let w = text_w + p;
            let h = text_h + p;
            (w + h, h)
        }

        // Inverted trapezoid uses `2 * padding` on both axes in Mermaid.
        "inv_trapezoid" | "inv-trapezoid" | "trap-t" | "manual" | "trapezoid-top" => {
            let w = text_w + 2.0 * p;
            let h = text_h + 2.0 * p;
            (w + h, h)
        }

        // Odd node (`>... ]`) is rendered using `rect_left_inv_arrow`.
        "odd" | "rect_left_inv_arrow" => {
            let w = text_w + p;
            let h = text_h + p;
            (w + h / 4.0, h)
        }

        // Ellipses are currently broken upstream but still emitted by FlowDB.
        // Keep a reasonable headless size for layout stability.
        "ellipse" => (text_w + 2.0 * p, text_h + 2.0 * p),

        // Fallback: treat unknown shapes as default rectangles.
        _ => (text_w + 4.0 * p, text_h + 2.0 * p),
    }
}

pub(crate) fn flowchart_node_render_dimensions(
    layout_shape: Option<&str>,
    metrics: crate::text::TextMetrics,
    padding: f64,
) -> (f64, f64) {
    node_render_dimensions(layout_shape, metrics, padding)
}

pub(super) fn node_layout_dimensions(
    layout_shape: Option<&str>,
    metrics: crate::text::TextMetrics,
    padding: f64,
    state_padding: f64,
    wrap_mode: crate::text::WrapMode,
    node_icon: Option<&str>,
    node_img: Option<&str>,
    node_pos: Option<&str>,
    node_asset_width: Option<f64>,
    node_asset_height: Option<f64>,
) -> (f64, f64) {
    let shape = layout_shape.unwrap_or("squareRect");

    if (shape == "imageSquare" || shape == "icon" || shape.starts_with("icon"))
        && (node_icon.is_some() || node_img.is_some())
    {
        if shape == "imageSquare" {
            if node_img.is_some_and(|s| !s.trim().is_empty()) {
                let asset_w = node_asset_width.unwrap_or(48.0).max(1.0);
                let asset_h = node_asset_height.unwrap_or(48.0).max(1.0);
                return (asset_w, asset_h);
            }
        } else if node_icon.is_some_and(|s| !s.trim().is_empty()) {
            let has_label = metrics.width > 0.0 && metrics.height > 0.0;
            let label_padding = if has_label { 8.0 } else { 0.0 };
            let label_bbox_w = if has_label { metrics.width + 4.0 } else { 0.0 };
            let label_bbox_h = if has_label { metrics.height + 4.0 } else { 0.0 };

            let asset_h = node_asset_height.unwrap_or(48.0).max(1.0);
            let asset_w = node_asset_width.unwrap_or(48.0).max(1.0);
            let icon_size = asset_h.max(asset_w);

            let outer_w = icon_size.max(label_bbox_w);
            let outer_h = icon_size + label_padding + label_bbox_h;

            // Mermaid icon helpers support `pos=t` for top-aligned labels, but that does not
            // change the node's outer bbox.
            let _ = node_pos;
            return (outer_w, outer_h);
        }
    }

    let (render_w, render_h) = node_render_dimensions(Some(shape), metrics, padding);

    // Mermaid `forkJoin.ts` inflates the Dagre node dimensions by `state.padding / 2` after
    // `updateNodeBounds(...)`, but does not re-render the rectangle with the inflated size. Keep
    // our layout spacing consistent with upstream by applying the same inflation here.
    if matches!(shape, "fork" | "join") {
        let extra = (state_padding / 2.0).max(0.0);
        return (render_w + extra, render_h + extra);
    }

    // Mermaid flowchart-v2 renders nodes using the "rendering-elements" layer:
    // 1) it generates SVG paths (roughjs-based even for non-handDrawn look),
    // 2) calls `updateNodeBounds(node, shapeElem)` which sets `node.width/height` from `getBBox()`,
    // 3) then feeds those updated dimensions into Dagre for layout.
    //
    // For stadium shapes the rough path is built from sampled arc points (`generateCirclePoints`,
    // 50 points over 180deg) and the resulting path bbox is slightly narrower than the theoretical
    // `w = bbox.width + h/4 + padding` used to generate the points. That bbox width is what Dagre
    // uses for spacing, which affects node x-positions and ultimately the root `viewBox`.
    if matches!(shape, "stadium" | "terminal" | "pill") {
        fn include_circle_points(
            center_x: f64,
            center_y: f64,
            radius: f64,
            table: &[(f64, f64)],
            mut include: impl FnMut(f64, f64),
        ) {
            for &(cos, sin) in table {
                let x = center_x + radius * cos;
                let y = center_y + radius * sin;
                include(-x, -y);
            }
        }

        let w = render_w.max(0.0);
        let h = render_h.max(0.0);
        if w > 0.0 && h > 0.0 {
            let radius = h / 2.0;
            let mut min_x = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            let mut include = |x: f64, y: f64| {
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            };

            include(-w / 2.0 + radius, -h / 2.0);
            include(w / 2.0 - radius, -h / 2.0);
            include_circle_points(
                -w / 2.0 + radius,
                0.0,
                radius,
                &crate::trig_tables::STADIUM_ARC_90_270_COS_SIN,
                &mut include,
            );
            include(w / 2.0 - radius, h / 2.0);
            include_circle_points(
                w / 2.0 - radius,
                0.0,
                radius,
                &crate::trig_tables::STADIUM_ARC_270_450_COS_SIN,
                &mut include,
            );

            if min_x.is_finite() && max_x.is_finite() && min_y.is_finite() && max_y.is_finite() {
                let bbox_w = (max_x - min_x).max(0.0);
                let bbox_h = (max_y - min_y).max(0.0);

                // Mermaid flowchart-v2 feeds Dagre with dimensions produced by `getBBox()`, and
                // Chromium returns those extents as f32-rounded values. Matching that lattice is
                // important for strict SVG `data-points` parity, since tiny width differences
                // propagate into Dagre x-coordinates.
                let w_f32 = bbox_w as f32;
                let h_f32 = bbox_h as f32;
                if w_f32.is_finite()
                    && h_f32.is_finite()
                    && w_f32.is_sign_positive()
                    && h_f32.is_sign_positive()
                {
                    return (w_f32 as f64, h_f32 as f64);
                }

                return (bbox_w, bbox_h);
            }
        }
    }

    // Chromium's `getBBox()` for HTML-label hexagons consistently lands on an `f32` lattice in
    // upstream baselines, while SVG-label hexagons preserve the exact path-derived width. Keep
    // the narrower `f32` quantization only for HTML-label flowchart nodes so both profiles align.
    if matches!(shape, "hexagon" | "hex" | "prepare")
        && matches!(wrap_mode, crate::text::WrapMode::HtmlLike)
    {
        let w_f32 = render_w as f32;
        let h_f32 = render_h as f32;
        if w_f32.is_finite()
            && h_f32.is_finite()
            && w_f32.is_sign_positive()
            && h_f32.is_sign_positive()
        {
            return (w_f32 as f64, h_f32 as f64);
        }
    }

    // Mermaid flowchart-v2 cylinder layout dimensions are derived from `updateNodeBounds(...)`
    // over an SVG `<path>` with arc commands. On Chromium this tends to round the bbox height to
    // the next representable f32 value above the theoretical height, which affects Dagre spacing
    // and therefore edge points (`data-points`) in strict parity mode.
    if matches!(shape, "cylinder" | "cyl" | "db" | "database") {
        let h_f32 = render_h as f32;
        if h_f32.is_finite() && h_f32.is_sign_positive() {
            let bits = h_f32.to_bits();
            if bits < u32::MAX {
                let bumped = f32::from_bits(bits + 1) as f64;
                return (render_w, bumped);
            }
        }
    }

    (render_w, render_h)
}
