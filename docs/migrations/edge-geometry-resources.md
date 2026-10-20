# Edge geometry resource migration

`edge-geometry-json` computes the Flowchart, Swimlane, and Agentflow paint routes and label positions without producing SVG. The payload remains schema 1 with the same occurrence order, duplicate identifiers, points, and label centers.

Encoded geometry now uses `max_geometry_json_bytes`. The operation accepts source/model limits, `max_layout_work_units`, and `max_prepared_text_retained_bytes`; explicit SVG-byte, SVG-element, and native SVG-backend overrides are rejected as irrelevant. Replace an operation-local `max_svg_bytes` override with `max_geometry_json_bytes` when the intended bound is the returned JSON size. SVG and native-export operations continue to use their own existing output limits.

Rust typed vectors do not consume an encoded-byte budget. Call `EdgePaintGeometryOutput::write_json_controlled` for bounded JSON encoding with the current operation control. Completed geometry owns its points and identifiers and remains independent of a retained parse snapshot.

Other successful built-in families return empty edges after semantic and input admission, without loading unused layout or font services. Custom registry JSON remains unsupported. Geometry-only output cannot certify portable SVG; `RequirePortable` returns an explicit unsupported-target error.

This updates the current behavior documented historically in the [0.8 to 0.9 upgrade guide](../release/V080_TO_V090_UPGRADE_GUIDE.md). Native ABI 3 operation ordinals and operation identifiers are unchanged.
