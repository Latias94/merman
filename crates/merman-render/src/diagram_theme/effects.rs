use super::ThemeCompileValidationError;
use super::canvas::ThemeColorValue;
use super::resources::{
    MAX_EFFECT_BINDINGS_HARD_CAP, MAX_EFFECT_GRAPHS_HARD_CAP,
    MAX_EFFECT_PRIMITIVES_PER_GRAPH_HARD_CAP, MAX_EFFECT_TURBULENCE_OCTAVES_HARD_CAP,
    ThemeResourceLimitExceeded, ThemeResourcePolicy,
};
use super::semantic::ThemeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EffectInput {
    SourceGraphic,
    Previous,
}

/// Color interpolation used by every primitive in an effect graph.
/// Existing typed effects retain the SVG default; CSS-derived recipes select sRGB explicitly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum EffectColorSpace {
    #[default]
    LinearRgb,
    Srgb,
}

impl EffectColorSpace {
    pub(crate) const fn svg_name(self) -> &'static str {
        match self {
            Self::LinearRgb => "linearRGB",
            Self::Srgb => "sRGB",
        }
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
    primitives: Vec<EffectPrimitive>,
    color_space: EffectColorSpace,
}

impl EffectGraph {
    pub fn new(
        id: impl Into<String>,
        primitives: impl IntoIterator<Item = EffectPrimitive>,
    ) -> Result<Self, ThemeCompileValidationError> {
        let id = id.into();
        validate_effect_id(&id)?;
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
            primitives,
            color_space: EffectColorSpace::default(),
        })
    }

    pub fn with_color_space(mut self, color_space: EffectColorSpace) -> Self {
        self.color_space = color_space;
        self
    }

    pub const fn color_space(&self) -> EffectColorSpace {
        self.color_space
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn primitives(&self) -> &[EffectPrimitive] {
        &self.primitives
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct EffectGraphResourceReceipt {
    primitive_count: usize,
    offset_magnitude: f32,
    blur_magnitude: f32,
    displacement_scale: f32,
    turbulence_octaves: usize,
}

impl EffectGraphResourceReceipt {
    fn from_primitives(primitives: &[EffectPrimitive]) -> Self {
        let mut receipt = Self {
            primitive_count: primitives.len(),
            ..Self::default()
        };
        for primitive in primitives {
            match primitive {
                EffectPrimitive::DropShadow {
                    offset_x,
                    offset_y,
                    blur_radius,
                    spread,
                    ..
                } => {
                    receipt.offset_magnitude = receipt
                        .offset_magnitude
                        .max(offset_x.abs())
                        .max(offset_y.abs());
                    receipt.blur_magnitude = receipt.blur_magnitude.max(*blur_radius).max(*spread);
                }
                EffectPrimitive::GaussianBlur { std_deviation, .. } => {
                    receipt.blur_magnitude = receipt.blur_magnitude.max(*std_deviation);
                }
                EffectPrimitive::Turbulence { octaves, .. } => {
                    receipt.turbulence_octaves =
                        receipt.turbulence_octaves.max(usize::from(*octaves));
                }
                EffectPrimitive::Displacement { scale, .. } => {
                    receipt.displacement_scale = receipt.displacement_scale.max(*scale);
                }
                EffectPrimitive::ColorMatrix { .. } => {}
            }
        }
        receipt
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
    resource_receipt: EffectResourceReceipt,
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
        self.resource_receipt
            .push_graph(EffectGraphResourceReceipt::from_primitives(
                &graph.primitives,
            ));
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
        self.resource_receipt.binding_count = self.bindings.len();
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
        self.resource_receipt.validate(resources)
    }
}

/// Immutable effect-resource facts captured while the authored graph is constructed.
#[derive(Debug, Clone, Default, PartialEq)]
struct EffectResourceReceipt {
    primitives_per_graph: Vec<usize>,
    binding_count: usize,
    offset_magnitude: f32,
    blur_magnitude: f32,
    displacement_scale: f32,
    turbulence_octaves: usize,
}

impl EffectResourceReceipt {
    fn push_graph(&mut self, graph: EffectGraphResourceReceipt) {
        self.primitives_per_graph.push(graph.primitive_count);
        self.offset_magnitude = self.offset_magnitude.max(graph.offset_magnitude);
        self.blur_magnitude = self.blur_magnitude.max(graph.blur_magnitude);
        self.displacement_scale = self.displacement_scale.max(graph.displacement_scale);
        self.turbulence_octaves = self.turbulence_octaves.max(graph.turbulence_octaves);
    }

    fn validate(&self, resources: &ThemeResourcePolicy) -> Result<(), ThemeResourceLimitExceeded> {
        resources.check_effect_graph_count(self.primitives_per_graph.len())?;
        for primitive_count in &self.primitives_per_graph {
            resources.check_effect_primitives_per_graph(*primitive_count)?;
        }
        resources.check_effect_binding_count(self.binding_count)?;
        resources.check_effect_offset_magnitude(self.offset_magnitude)?;
        resources.check_effect_blur_magnitude(self.blur_magnitude)?;
        resources.check_effect_displacement_scale(self.displacement_scale)?;
        resources.check_effect_turbulence_octaves(self.turbulence_octaves)
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
