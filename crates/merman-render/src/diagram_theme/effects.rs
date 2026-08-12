use super::ThemeCompileValidationError;
use super::canvas::ThemeColorValue;
use super::resources::{
    MAX_EFFECT_BINDINGS_HARD_CAP, MAX_EFFECT_GRAPHS_HARD_CAP,
    MAX_EFFECT_PRIMITIVES_PER_GRAPH_HARD_CAP, MAX_EFFECT_TURBULENCE_OCTAVES_HARD_CAP,
    ThemeResourceLimitExceeded, ThemeResourcePolicy,
};
use super::semantic::ThemeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectInput {
    SourceGraphic,
    Previous,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FilterRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl FilterRegion {
    pub const fn bounded(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub(crate) fn validate(self) -> Result<(), ThemeCompileValidationError> {
        if [self.x, self.y, self.width, self.height]
            .into_iter()
            .any(|value| !value.is_finite())
            || self.width <= 0.0
            || self.height <= 0.0
        {
            return Err(ThemeCompileValidationError::InvalidNumber {
                field: "effects.filter_region",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum EffectPrimitive {
    DropShadow {
        input: EffectInput,
        offset_x: f32,
        offset_y: f32,
        blur_radius: f32,
        spread: f32,
        color: ThemeColorValue,
    },
    GaussianBlur {
        input: EffectInput,
        std_deviation: f32,
    },
    ColorMatrix {
        input: EffectInput,
        values: [f32; 20],
    },
    Turbulence {
        input: EffectInput,
        base_frequency_x: f32,
        base_frequency_y: f32,
        octaves: u8,
        seed: i32,
    },
    Displacement {
        input: EffectInput,
        map_input: EffectInput,
        scale: f32,
    },
}

impl EffectPrimitive {
    pub(crate) fn validate(&self) -> Result<(), ThemeCompileValidationError> {
        match self {
            Self::DropShadow {
                offset_x,
                offset_y,
                blur_radius,
                spread,
                ..
            } => {
                validate_finite(*offset_x, "effects.drop_shadow.offset_x")?;
                validate_finite(*offset_y, "effects.drop_shadow.offset_y")?;
                validate_nonnegative(*blur_radius, "effects.drop_shadow.blur_radius")?;
                validate_nonnegative(*spread, "effects.drop_shadow.spread")
            }
            Self::GaussianBlur { std_deviation, .. } => {
                validate_nonnegative(*std_deviation, "effects.gaussian_blur.std_deviation")
            }
            Self::ColorMatrix { values, .. } => {
                if values.iter().any(|value| !value.is_finite()) {
                    return Err(ThemeCompileValidationError::InvalidNumber {
                        field: "effects.color_matrix.values",
                    });
                }
                Ok(())
            }
            Self::Turbulence {
                base_frequency_x,
                base_frequency_y,
                octaves,
                ..
            } => {
                validate_nonnegative(*base_frequency_x, "effects.turbulence.base_frequency_x")?;
                validate_nonnegative(*base_frequency_y, "effects.turbulence.base_frequency_y")?;
                if *octaves == 0 || usize::from(*octaves) > MAX_EFFECT_TURBULENCE_OCTAVES_HARD_CAP {
                    return Err(ThemeCompileValidationError::InvalidNumber {
                        field: "effects.turbulence.octaves",
                    });
                }
                Ok(())
            }
            Self::Displacement { scale, .. } => {
                validate_nonnegative(*scale, "effects.displacement.scale")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffectGraph {
    id: String,
    region: FilterRegion,
    primitives: Vec<EffectPrimitive>,
}

impl EffectGraph {
    pub fn new(
        id: impl Into<String>,
        region: FilterRegion,
        primitives: impl IntoIterator<Item = EffectPrimitive>,
    ) -> Result<Self, ThemeCompileValidationError> {
        let id = id.into();
        validate_effect_id(&id)?;
        region.validate()?;
        let primitives = primitives.into_iter().collect::<Vec<_>>();
        if primitives.is_empty() || primitives.len() > MAX_EFFECT_PRIMITIVES_PER_GRAPH_HARD_CAP {
            return Err(ThemeCompileValidationError::InvalidCollection {
                field: "effects.graph.primitives",
            });
        }
        for primitive in &primitives {
            primitive.validate()?;
        }
        validate_inputs(&primitives)?;
        Ok(Self {
            id,
            region,
            primitives,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn region(&self) -> FilterRegion {
        self.region
    }

    pub fn primitives(&self) -> &[EffectPrimitive] {
        &self.primitives
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectBinding {
    target: ThemeTarget,
    effect_id: String,
}

impl EffectBinding {
    pub fn new(
        target: ThemeTarget,
        effect_id: impl Into<String>,
    ) -> Result<Self, ThemeCompileValidationError> {
        let effect_id = effect_id.into();
        validate_effect_id(&effect_id)?;
        Ok(Self { target, effect_id })
    }

    pub const fn target(&self) -> ThemeTarget {
        self.target
    }

    pub fn effect_id(&self) -> &str {
        &self.effect_id
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DiagramEffectSet {
    graphs: Vec<EffectGraph>,
    bindings: Vec<EffectBinding>,
}

impl DiagramEffectSet {
    pub fn with_graph(mut self, graph: EffectGraph) -> Result<Self, ThemeCompileValidationError> {
        if self.graphs.len() >= MAX_EFFECT_GRAPHS_HARD_CAP {
            return Err(ThemeCompileValidationError::LimitExceeded {
                field: "effects.graphs",
            });
        }
        if self.graphs.iter().any(|existing| existing.id == graph.id) {
            return Err(ThemeCompileValidationError::DuplicateId {
                field: "effects.graph.id",
            });
        }
        self.graphs.push(graph);
        Ok(self)
    }

    pub fn with_binding(
        mut self,
        binding: EffectBinding,
    ) -> Result<Self, ThemeCompileValidationError> {
        if self.bindings.len() >= MAX_EFFECT_BINDINGS_HARD_CAP {
            return Err(ThemeCompileValidationError::LimitExceeded {
                field: "effects.bindings",
            });
        }
        if self
            .bindings
            .iter()
            .any(|existing| existing.target == binding.target)
        {
            return Err(ThemeCompileValidationError::DuplicateId {
                field: "effects.binding.target",
            });
        }
        self.bindings.push(binding);
        Ok(self)
    }

    pub fn graphs(&self) -> &[EffectGraph] {
        &self.graphs
    }

    pub fn bindings(&self) -> &[EffectBinding] {
        &self.bindings
    }

    pub(crate) fn graph(&self, id: &str) -> Option<&EffectGraph> {
        self.graphs.iter().find(|graph| graph.id == id)
    }

    pub(crate) fn validate(&self) -> Result<(), ThemeCompileValidationError> {
        if self.graphs.len() > MAX_EFFECT_GRAPHS_HARD_CAP {
            return Err(ThemeCompileValidationError::LimitExceeded {
                field: "effects.graphs",
            });
        }
        if self.bindings.len() > MAX_EFFECT_BINDINGS_HARD_CAP {
            return Err(ThemeCompileValidationError::LimitExceeded {
                field: "effects.bindings",
            });
        }
        let targets = self
            .bindings
            .iter()
            .map(|binding| binding.target)
            .collect::<std::collections::BTreeSet<_>>();
        if targets.len() != self.bindings.len() {
            return Err(ThemeCompileValidationError::DuplicateId {
                field: "effects.binding.target",
            });
        }
        let ids = self
            .graphs
            .iter()
            .map(|graph| graph.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        if ids.len() != self.graphs.len() {
            return Err(ThemeCompileValidationError::DuplicateId {
                field: "effects.graph.id",
            });
        }
        for binding in &self.bindings {
            if !ids.contains(binding.effect_id.as_str()) {
                return Err(ThemeCompileValidationError::UnknownId {
                    field: "effects.binding.effect_id",
                });
            }
        }
        Ok(())
    }

    pub(crate) fn check_resources(
        &self,
        resources: &ThemeResourcePolicy,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        resources.check_effect_graph_count(self.graphs.len())?;
        for graph in &self.graphs {
            resources.check_effect_primitives_per_graph(graph.primitives.len())?;
        }
        resources.check_effect_binding_count(self.bindings.len())?;

        let mut offset_magnitude = 0.0_f32;
        let mut filter_region_magnitude = 0.0_f32;
        let mut blur_magnitude = 0.0_f32;
        let mut displacement_scale = 0.0_f32;
        let mut turbulence_octaves = 0_usize;

        for graph in &self.graphs {
            let region = graph.region;
            filter_region_magnitude = filter_region_magnitude
                .max(region.x.abs())
                .max(region.y.abs())
                .max(region.width.abs())
                .max(region.height.abs());

            for primitive in &graph.primitives {
                match primitive {
                    EffectPrimitive::DropShadow {
                        offset_x,
                        offset_y,
                        blur_radius,
                        spread,
                        ..
                    } => {
                        offset_magnitude = offset_magnitude.max(offset_x.abs()).max(offset_y.abs());
                        blur_magnitude = blur_magnitude.max(*blur_radius).max(*spread);
                    }
                    EffectPrimitive::GaussianBlur { std_deviation, .. } => {
                        blur_magnitude = blur_magnitude.max(*std_deviation);
                    }
                    EffectPrimitive::Turbulence { octaves, .. } => {
                        turbulence_octaves = turbulence_octaves.max(usize::from(*octaves));
                    }
                    EffectPrimitive::Displacement { scale, .. } => {
                        displacement_scale = displacement_scale.max(*scale);
                    }
                    EffectPrimitive::ColorMatrix { .. } => {}
                }
            }
        }

        resources.check_effect_offset_magnitude(offset_magnitude)?;
        resources.check_effect_filter_region_magnitude(filter_region_magnitude)?;
        resources.check_effect_blur_magnitude(blur_magnitude)?;
        resources.check_effect_displacement_scale(displacement_scale)?;
        resources.check_effect_turbulence_octaves(turbulence_octaves)
    }
}

fn validate_inputs(primitives: &[EffectPrimitive]) -> Result<(), ThemeCompileValidationError> {
    for (index, primitive) in primitives.iter().enumerate() {
        let max_input = index;
        let inputs = match primitive {
            EffectPrimitive::DropShadow { input, .. }
            | EffectPrimitive::GaussianBlur { input, .. }
            | EffectPrimitive::ColorMatrix { input, .. }
            | EffectPrimitive::Turbulence { input, .. } => [Some(*input), None],
            EffectPrimitive::Displacement {
                input, map_input, ..
            } => [Some(*input), Some(*map_input)],
        };
        for input in inputs.into_iter().flatten() {
            if matches!(input, EffectInput::Previous) && max_input == 0 {
                return Err(ThemeCompileValidationError::InvalidCollection {
                    field: "effects.graph.inputs",
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_effect_id(id: &str) -> Result<(), ThemeCompileValidationError> {
    if id.is_empty()
        || id.len() > 128
        || id.chars().any(|character| {
            character.is_control()
                || !(character.is_ascii_alphanumeric() || "-_".contains(character))
        })
    {
        return Err(ThemeCompileValidationError::InvalidValue {
            field: "effects.graph.id",
        });
    }
    Ok(())
}

fn validate_finite(value: f32, field: &'static str) -> Result<(), ThemeCompileValidationError> {
    if !value.is_finite() {
        return Err(ThemeCompileValidationError::InvalidNumber { field });
    }
    Ok(())
}

fn validate_nonnegative(
    value: f32,
    field: &'static str,
) -> Result<(), ThemeCompileValidationError> {
    if !value.is_finite() || value < 0.0 {
        return Err(ThemeCompileValidationError::InvalidNumber { field });
    }
    Ok(())
}
