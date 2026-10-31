use crate::family::DiagramFamilyId;

/// A bitset declaration containing bits outside the selected family's paint vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid {family} paint ownership bits {bits:#010b}; valid mask is {valid_mask:#010b}")]
pub struct PaintInputMaskError {
    family: &'static str,
    bits: u8,
    valid_mask: u8,
}

impl PaintInputMaskError {
    pub const fn family(self) -> &'static str {
        self.family
    }

    pub const fn bits(self) -> u8 {
        self.bits
    }

    pub const fn valid_mask(self) -> u8 {
        self.valid_mask
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GitGraphPaintInput {
    PrimaryColor,
    MainBackground,
    TagBackground,
    PrimaryBorder,
    NodeBorder,
    TagBorder,
}

/// Declared Mermaid paint authority in the pinned GitGraph input order.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct GitGraphPaintInputs(u8);

impl GitGraphPaintInputs {
    pub const VALID_MASK: u8 = 0b11_1111;

    pub const fn try_from_owned_bits(bits: u8) -> Result<Self, PaintInputMaskError> {
        if bits & !Self::VALID_MASK != 0 {
            return Err(PaintInputMaskError {
                family: "gitGraph",
                bits,
                valid_mask: Self::VALID_MASK,
            });
        }
        Ok(Self(bits))
    }

    pub const fn owned_bits(self) -> u8 {
        self.0
    }

    pub const fn with_owned(self, input: GitGraphPaintInput) -> Self {
        Self(self.0 | (1 << input as u8))
    }

    pub const fn is_owned(self, input: GitGraphPaintInput) -> bool {
        self.0 & (1 << input as u8) != 0
    }

    pub const fn available_sources(self) -> u8 {
        !self.0 & Self::VALID_MASK
    }
}

impl TryFrom<u8> for GitGraphPaintInputs {
    type Error = PaintInputMaskError;

    fn try_from(bits: u8) -> Result<Self, Self::Error> {
        Self::try_from_owned_bits(bits)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ErPaintInput {
    Text,
    NodeText,
    Line,
    OddRow,
    EvenRow,
}

/// Declared Mermaid paint authority in the pinned ER input order.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ErPaintInputs(u8);

impl ErPaintInputs {
    pub const VALID_MASK: u8 = 0b1_1111;

    pub const fn try_from_owned_bits(bits: u8) -> Result<Self, PaintInputMaskError> {
        if bits & !Self::VALID_MASK != 0 {
            return Err(PaintInputMaskError {
                family: "er",
                bits,
                valid_mask: Self::VALID_MASK,
            });
        }
        Ok(Self(bits))
    }

    pub const fn owned_bits(self) -> u8 {
        self.0
    }

    pub const fn with_owned(self, input: ErPaintInput) -> Self {
        Self(self.0 | (1 << input as u8))
    }

    pub const fn is_owned(self, input: ErPaintInput) -> bool {
        self.0 & (1 << input as u8) != 0
    }
}

impl TryFrom<u8> for ErPaintInputs {
    type Error = PaintInputMaskError;

    fn try_from(bits: u8) -> Result<Self, Self::Error> {
        Self::try_from_owned_bits(bits)
    }
}

/// Family-specific paint facts; an absent mask and a present empty mask are distinct data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FamilyPaintInputs {
    GitGraph(GitGraphPaintInputs),
    Er(ErPaintInputs),
}

impl FamilyPaintInputs {
    /// The logical family whose presentation may consume this declaration.
    pub const fn family_id(self) -> DiagramFamilyId {
        match self {
            Self::GitGraph(_) => DiagramFamilyId::GIT_GRAPH,
            Self::Er(_) => DiagramFamilyId::ER,
        }
    }

    /// The full model kind against which a presentation owner checks this declaration.
    pub const fn render_model_kind(self) -> &'static str {
        self.family_id().as_str()
    }
}
