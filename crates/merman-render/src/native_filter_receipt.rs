use std::str::FromStr as _;

use sha2::{Digest as _, Sha256};

const RECEIPT_DOMAIN: &[u8] = b"merman-native-svg-filter-receipt-v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[doc(hidden)]
pub struct NativeSvgHardShadow {
    filter_id: String,
    region_bits: [u32; 4],
    offset_bits: [u32; 2],
    color_rgba: [u8; 4],
    reference_count: u32,
}

impl NativeSvgHardShadow {
    pub fn new(
        filter_id: impl Into<String>,
        region: [f32; 4],
        offset: [f32; 2],
        color_css: &str,
        reference_count: usize,
    ) -> Option<Self> {
        let filter_id = filter_id.into();
        if filter_id.is_empty()
            || filter_id.len() > 1024
            || filter_id
                .chars()
                .any(|character| character.is_control() || character.is_whitespace())
        {
            return None;
        }
        if region
            .into_iter()
            .chain(offset)
            .any(|value| !value.is_finite())
            || region[2] <= 0.0
            || region[3] <= 0.0
        {
            return None;
        }
        let reference_count = u32::try_from(reference_count).ok()?;
        if reference_count == 0 {
            return None;
        }
        let color = svgtypes::Color::from_str(color_css.trim()).ok()?;
        Some(Self {
            filter_id,
            region_bits: region.map(normalized_f32_bits),
            offset_bits: offset.map(normalized_f32_bits),
            color_rgba: [color.red, color.green, color.blue, color.alpha],
            reference_count,
        })
    }

    pub fn filter_id(&self) -> &str {
        &self.filter_id
    }

    pub fn region(&self) -> [f32; 4] {
        self.region_bits.map(f32::from_bits)
    }

    pub fn offset(&self) -> [f32; 2] {
        self.offset_bits.map(f32::from_bits)
    }

    pub const fn color_rgba(&self) -> [u8; 4] {
        self.color_rgba
    }

    pub const fn reference_count(&self) -> u32 {
        self.reference_count
    }
}

/// Opaque equality receipt for the exact hard-shadow filters emitted by a family renderer.
///
/// The workspace exporter independently reconstructs the same receipt from the terminal SVG and
/// the resolved `usvg` tree. Native target admission compares the two receipts instead of trusting
/// a capability name or a filter count.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeSvgFilterReceipt {
    digest: [u8; 32],
    hard_shadow_count: u32,
    reference_count: u32,
}

impl NativeSvgFilterReceipt {
    pub fn from_hard_shadows(
        shadows: impl IntoIterator<Item = NativeSvgHardShadow>,
    ) -> Option<Self> {
        let mut shadows = shadows.into_iter().collect::<Vec<_>>();
        if shadows.is_empty() {
            return None;
        }
        shadows.sort_unstable();
        if shadows
            .windows(2)
            .any(|pair| pair[0].filter_id == pair[1].filter_id)
        {
            return None;
        }

        let hard_shadow_count = u32::try_from(shadows.len()).ok()?;
        let reference_count = shadows.iter().try_fold(0u32, |total, shadow| {
            total.checked_add(shadow.reference_count)
        })?;
        let mut hasher = Sha256::new();
        update_len_prefixed(&mut hasher, RECEIPT_DOMAIN);
        hasher.update(hard_shadow_count.to_le_bytes());
        hasher.update(reference_count.to_le_bytes());
        for shadow in shadows {
            update_len_prefixed(&mut hasher, shadow.filter_id.as_bytes());
            for bits in shadow.region_bits {
                hasher.update(bits.to_le_bytes());
            }
            for bits in shadow.offset_bits {
                hasher.update(bits.to_le_bytes());
            }
            hasher.update(shadow.color_rgba);
            hasher.update(shadow.reference_count.to_le_bytes());
        }

        Some(Self {
            digest: hasher.finalize().into(),
            hard_shadow_count,
            reference_count,
        })
    }

    pub const fn hard_shadow_count(self) -> u32 {
        self.hard_shadow_count
    }

    pub const fn reference_count(self) -> u32 {
        self.reference_count
    }
}

fn normalized_f32_bits(value: f32) -> u32 {
    if value == 0.0 {
        0.0f32.to_bits()
    } else {
        value.to_bits()
    }
}

fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shadow(id: &str, offset_x: f32, references: usize) -> NativeSvgHardShadow {
        NativeSvgHardShadow::new(
            id,
            [-0.2, -0.2, 1.4, 1.4],
            [offset_x, 5.0],
            "#111827",
            references,
        )
        .expect("valid hard shadow")
    }

    #[test]
    fn receipt_is_order_independent_but_value_sensitive() {
        let first = NativeSvgFilterReceipt::from_hard_shadows([
            shadow("alpha", 4.0, 2),
            shadow("beta", 6.0, 1),
        ])
        .unwrap();
        let reordered = NativeSvgFilterReceipt::from_hard_shadows([
            shadow("beta", 6.0, 1),
            shadow("alpha", 4.0, 2),
        ])
        .unwrap();
        let changed = NativeSvgFilterReceipt::from_hard_shadows([
            shadow("alpha", 5.0, 2),
            shadow("beta", 6.0, 1),
        ])
        .unwrap();

        assert_eq!(first, reordered);
        assert_ne!(first, changed);
        assert_eq!(first.hard_shadow_count(), 2);
        assert_eq!(first.reference_count(), 3);
    }

    #[test]
    fn duplicate_effect_ids_and_empty_receipts_are_rejected() {
        assert!(NativeSvgFilterReceipt::from_hard_shadows([]).is_none());
        assert!(
            NativeSvgFilterReceipt::from_hard_shadows([
                shadow("duplicate", 4.0, 1),
                shadow("duplicate", 5.0, 1),
            ])
            .is_none()
        );
    }
}
