use super::document::{
    ElevationEffect, ElevationSpec, FillInk, FormPatch, RoleOverrides, ThemeDocumentFields,
    TypographyPatch,
};
use super::{FontRole, Paint, SCHEMA_VERSION, Texture, ThemeError};

pub(super) fn document(doc: &ThemeDocumentFields) -> Result<(), ThemeError> {
    if doc.schema_version != SCHEMA_VERSION {
        return Err(ThemeError::invalid(
            "schema_version",
            "expected theme schema version 2",
        ));
    }
    if let Some(id) = doc.id.as_deref()
        && (id.len() > 48
            || !id.starts_with(|ch: char| ch.is_ascii_lowercase())
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'))
    {
        return Err(ThemeError::invalid(
            "id",
            "expected a lowercase theme ID of up to 48 characters",
        ));
    }
    if let Some(name) = &doc.name {
        bounds("name", name.chars().count() as f64, 1.0, 64.0)?;
    }
    if let Some(description) = &doc.description {
        bounds(
            "description",
            description.chars().count() as f64,
            0.0,
            240.0,
        )?;
    }
    if (doc.extends.is_none() || doc.appearances.is_some()) && doc.appearances().next().is_none() {
        return Err(ThemeError::invalid(
            "appearances",
            "declare at least one appearance",
        ));
    }
    if let Some(patch) = &doc.roles {
        roles("roles", patch)?;
    }
    if let Some(patch) = &doc.form {
        form("form", patch)?;
    }
    if let Some(patch) = &doc.typography {
        typography("typography", patch)?;
    }
    bounds("overrides", doc.overrides.len() as f64, 0.0, 32.0)?;

    for appearance in doc.appearances() {
        let path = format!("appearances.{appearance}");
        let Some(layer) = doc.layer(appearance) else {
            continue;
        };
        if doc.extends.is_none()
            && !layer.seed.as_ref().is_some_and(|seed| {
                seed.canvas.is_some() && seed.ink.is_some() && seed.accent.is_some()
            })
        {
            return Err(ThemeError::invalid(
                format!("{path}.seed"),
                "appearance requires seed canvas, ink, and accent",
            ));
        }
        if let Some(patch) = &layer.roles {
            roles(&format!("{path}.roles"), patch)?;
        }
        if let Some(data) = &layer.data {
            for (name, colors) in [("ink", &data.ink), ("fill", &data.fill)] {
                if let Some(colors) = colors {
                    bounds(
                        &format!("{path}.data.{name}"),
                        colors.len() as f64,
                        4.0,
                        16.0,
                    )?;
                }
            }
            if let Some(FillInk::Color(color)) = data.fill_ink
                && color.channels()[3] != 255
            {
                return Err(ThemeError::invalid(
                    format!("{path}.data.fill_ink"),
                    "use an opaque label color or auto",
                ));
            }
        }
        if let Some(canvas) = &layer.canvas {
            if let Some(padding) = canvas.padding_px {
                bounds(&format!("{path}.canvas.padding_px"), padding, 0.0, 64.0)?;
            }
            if let Some(textures) = &canvas.textures {
                bounds(
                    &format!("{path}.canvas.textures"),
                    textures.len() as f64,
                    0.0,
                    3.0,
                )?;
                for (index, texture) in textures.iter().enumerate() {
                    validate_texture(&format!("{path}.canvas.textures[{index}]"), texture)?;
                }
            }
        }
        if let Some(spec) = &layer.elevation {
            elevation(&format!("{path}.elevation"), spec)?;
        }
    }
    for (index, patch) in doc.overrides.iter().enumerate() {
        let path = format!("overrides[{index}]");
        bounds(
            &format!("{path}.families"),
            patch.families.len() as f64,
            1.0,
            34.0,
        )?;
        if patch.families.iter().any(|family| family.is_empty()) {
            return Err(ThemeError::invalid(
                format!("{path}.families"),
                "family IDs must be nonempty",
            ));
        }
        if let Some(patch) = &patch.roles {
            roles(&format!("{path}.roles"), patch)?;
        }
        if let Some(patch) = &patch.form {
            form(&format!("{path}.form"), patch)?;
        }
        if let Some(patch) = &patch.typography {
            typography(&format!("{path}.typography"), patch)?;
        }
        if let Some(spec) = &patch.elevation {
            elevation(&format!("{path}.elevation"), spec)?;
        }
    }
    Ok(())
}

fn bounds(path: &str, value: f64, min: f64, max: f64) -> Result<(), ThemeError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(ThemeError::invalid(
            path,
            format!("{path} must be between {min} and {max}"),
        ))
    }
}

fn fields(path: &str, values: &[(&str, Option<f64>, f64, f64)]) -> Result<(), ThemeError> {
    for &(name, value, min, max) in values {
        if let Some(value) = value
            && !(min..=max).contains(&value)
        {
            return bounds(&format!("{path}.{name}"), value, min, max);
        }
    }
    Ok(())
}

fn roles(path: &str, patch: &RoleOverrides) -> Result<(), ThemeError> {
    for (name, paint) in [
        ("canvas", &patch.canvas),
        ("surface.node", &patch.surface_node),
        ("surface.container", &patch.surface_container),
        ("surface.callout", &patch.surface_callout),
        ("surface.emphasis", &patch.surface_emphasis),
        ("stroke.border", &patch.stroke_border),
        ("stroke.connector", &patch.stroke_connector),
        ("stroke.guide", &patch.stroke_guide),
    ] {
        if let Some(Paint::LinearGradient(gradient)) = paint {
            let path = format!("{path}.{name}");
            bounds(&format!("{path}.angle_deg"), gradient.angle_deg, 0.0, 360.0)?;
            bounds(
                &format!("{path}.stops"),
                gradient.stops.len() as f64,
                2.0,
                8.0,
            )?;
            let mut previous = 0.0;
            for (index, stop) in gradient.stops.iter().enumerate() {
                let path = format!("{path}.stops[{index}]");
                bounds(&format!("{path}.offset"), stop.offset, previous, 1.0)?;
                bounds(&format!("{path}.opacity"), stop.opacity, 0.0, 1.0)?;
                previous = stop.offset;
            }
        }
    }
    Ok(())
}

fn typography(path: &str, patch: &TypographyPatch) -> Result<(), ThemeError> {
    for (name, stack) in [
        ("sans", &patch.sans),
        ("mono", &patch.mono),
        ("serif", &patch.serif),
    ] {
        if let Some(stack) = stack {
            let path = format!("{path}.{name}");
            bounds(&path, stack.len() as f64, 2.0, 8.0)?;
            for (index, name) in stack.iter().enumerate() {
                if name.trim().is_empty()
                    || name.chars().count() > 64
                    || name.chars().any(char::is_control)
                {
                    return Err(ThemeError::invalid(
                        format!("{path}[{index}]"),
                        "expected a font family name of 1 to 64 characters",
                    ));
                }
            }
        }
    }
    if patch.code_family == Some(FontRole::Serif) {
        return Err(ThemeError::invalid(
            format!("{path}.code_family"),
            "expected mono or sans",
        ));
    }
    fields(
        path,
        &[
            ("base_size_px", patch.base_size_px, 10.0, 24.0),
            (
                "title_weight",
                patch.title_weight.map(f64::from),
                400.0,
                800.0,
            ),
            (
                "label_weight",
                patch.label_weight.map(f64::from),
                400.0,
                700.0,
            ),
        ],
    )
}

fn form(path: &str, patch: &FormPatch) -> Result<(), ThemeError> {
    fields(
        path,
        &[
            ("radius", patch.radius, 0.0, 24.0),
            ("container_radius", patch.container_radius, 0.0, 24.0),
            ("stroke_width", patch.stroke_width, 0.5, 4.0),
            ("connector_width", patch.connector_width, 0.75, 4.0),
        ],
    )?;
    if let Some(chart) = &patch.chart {
        fields(
            &format!("{path}.chart"),
            &[
                ("bar_radius", chart.bar_radius, 0.0, 12.0),
                ("pie_gap_px", chart.pie_gap_px, 0.0, 6.0),
                ("pie_opacity", chart.pie_opacity, 0.65, 1.0),
            ],
        )?;
    }
    if let Some(spec) = &patch.elevation {
        elevation(&format!("{path}.elevation"), spec)?;
    }
    Ok(())
}

fn elevation(path: &str, spec: &ElevationSpec) -> Result<(), ThemeError> {
    match spec {
        ElevationSpec::Effect(ElevationEffect::LayeredStack { layers }) => {
            bounds(&format!("{path}.layers"), layers.len() as f64, 1.0, 4.0)?;
            for (index, layer) in layers.iter().enumerate() {
                fields(
                    &format!("{path}.layers[{index}]"),
                    &[
                        ("dx", Some(layer.dx), -16.0, 16.0),
                        ("dy", Some(layer.dy), -16.0, 16.0),
                        ("blur", Some(layer.blur), 0.0, 16.0),
                        ("opacity", Some(layer.opacity), 0.01, 1.0),
                    ],
                )?;
            }
            Ok(())
        }
        ElevationSpec::Effect(ElevationEffect::Glow { blur, opacity, .. }) => fields(
            path,
            &[
                ("blur", Some(*blur), 0.5, 12.0),
                ("opacity", Some(*opacity), 0.05, 0.85),
            ],
        ),
        ElevationSpec::Preset(_) => Ok(()),
    }
}

fn validate_texture(path: &str, texture: &Texture) -> Result<(), ThemeError> {
    match *texture {
        Texture::None => Ok(()),
        Texture::Grid {
            step_px,
            major_every,
            opacity,
            major_opacity,
            ..
        } => fields(
            path,
            &[
                ("step_px", Some(step_px), 4.0, 120.0),
                ("major_every", Some(f64::from(major_every)), 2.0, 12.0),
                ("opacity", Some(opacity), 0.01, 0.6),
                ("major_opacity", Some(major_opacity), 0.01, 0.8),
            ],
        ),
        Texture::Dots {
            step_px,
            radius_px,
            opacity,
            ..
        } => fields(
            path,
            &[
                ("step_px", Some(step_px), 4.0, 64.0),
                ("radius_px", Some(radius_px), 0.4, 4.0),
                ("opacity", Some(opacity), 0.02, 0.5),
            ],
        ),
        Texture::Ruled {
            line_height_px,
            opacity,
            margin_rule_x_px,
            ..
        } => fields(
            path,
            &[
                ("line_height_px", Some(line_height_px), 12.0, 64.0),
                ("opacity", Some(opacity), 0.02, 0.5),
                ("margin_rule_x_px", Some(margin_rule_x_px), 8.0, 96.0),
            ],
        ),
        Texture::Fibers {
            opacity, angle_deg, ..
        } => fields(
            path,
            &[
                ("opacity", Some(opacity), 0.01, 0.2),
                ("angle_deg", Some(angle_deg), 0.0, 180.0),
            ],
        ),
        Texture::Scanlines {
            step_px, opacity, ..
        } => fields(
            path,
            &[
                ("step_px", Some(step_px), 2.0, 16.0),
                ("opacity", Some(opacity), 0.01, 0.35),
            ],
        ),
        Texture::RadialWash {
            opacity,
            cx,
            cy,
            radius,
            ..
        } => fields(
            path,
            &[
                ("opacity", Some(opacity), 0.01, 0.35),
                ("cx", Some(cx), 0.0, 1.0),
                ("cy", Some(cy), 0.0, 1.0),
                ("radius", Some(radius), 0.1, 1.5),
            ],
        ),
    }
}
