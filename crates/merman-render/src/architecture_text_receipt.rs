//! Actual Architecture text terminal observations sealed by the final writer.

use sha2::{Digest as _, Sha256};

/// Renderer-owned visual facts for one terminal covered by an Architecture Text receipt.
///
/// This workspace-private projection deliberately carries the final writer value and measured
/// target rectangle outside the SVG DOM. The non-published acceptance harness can consume these
/// facts without reconstructing Architecture layout geometry or introducing `data-*` proof
/// attributes into stable SVG output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArchitectureTextRole {
    Service,
    GroupTitle,
    EdgeLabel,
}

impl ArchitectureTextRole {
    const fn digest_name(self) -> &'static str {
        match self {
            Self::Service => "service",
            Self::GroupTitle => "group-title",
            Self::EdgeLabel => "edge-label",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchitectureTextTerminal {
    role: ArchitectureTextRole,
    identity: Box<str>,
    value: Box<str>,
    region_bits: [u64; 4],
    writer_fragment_digest: Option<[u8; 32]>,
    writer_run_count: usize,
}

impl ArchitectureTextTerminal {
    pub(crate) fn new(
        role: ArchitectureTextRole,
        identity: impl Into<Box<str>>,
        value: impl Into<Box<str>>,
        region: [f64; 4],
    ) -> Option<Self> {
        (region.into_iter().all(f64::is_finite) && region[2] > 0.0 && region[3] > 0.0).then(|| {
            Self {
                role,
                identity: identity.into(),
                value: value.into(),
                region_bits: region.map(f64::to_bits),
                writer_fragment_digest: None,
                writer_run_count: 0,
            }
        })
    }

    pub(crate) fn with_writer_facts(
        role: ArchitectureTextRole,
        identity: impl Into<Box<str>>,
        value: impl Into<Box<str>>,
        region: [f64; 4],
        writer_fragment_digest: [u8; 32],
        writer_run_count: usize,
    ) -> Option<Self> {
        if writer_run_count == 0 {
            return None;
        }
        Some(
            Self::new(role, identity, value, region)?
                .with_facts(writer_fragment_digest, writer_run_count),
        )
    }

    fn with_facts(mut self, writer_fragment_digest: [u8; 32], writer_run_count: usize) -> Self {
        self.writer_fragment_digest = Some(writer_fragment_digest);
        self.writer_run_count = writer_run_count;
        self
    }

    pub const fn role(&self) -> ArchitectureTextRole {
        self.role
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn region(&self) -> [f64; 4] {
        self.region_bits.map(f64::from_bits)
    }
}

/// Opaque production receipt for finalized Architecture text terminal observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchitectureTextReceipt {
    terminals: Box<[ArchitectureTextTerminal]>,
    digest: [u8; 32],
}

impl ArchitectureTextReceipt {
    pub(crate) fn seal(terminals: Vec<ArchitectureTextTerminal>) -> Option<Self> {
        if terminals.is_empty() {
            return None;
        }
        let mut hasher = Sha256::new();
        // Preserve the established domain so moving the observation owner does not change receipts.
        update_len_prefixed(&mut hasher, b"merman.architecture-text-cutover-receipt.v1");
        update_len(&mut hasher, terminals.len());
        for terminal in &terminals {
            update_len_prefixed(&mut hasher, terminal.role.digest_name().as_bytes());
            update_len_prefixed(&mut hasher, terminal.identity.as_bytes());
            update_len_prefixed(&mut hasher, terminal.value.as_bytes());
            for coordinate in terminal.region_bits {
                hasher.update(coordinate.to_be_bytes());
            }
            hasher.update([u8::from(terminal.writer_fragment_digest.is_some())]);
            if let Some(fragment_digest) = terminal.writer_fragment_digest {
                hasher.update(fragment_digest);
            }
            update_len(&mut hasher, terminal.writer_run_count);
        }
        Some(Self {
            terminals: terminals.into_boxed_slice(),
            digest: hasher.finalize().into(),
        })
    }

    pub fn terminals(&self) -> &[ArchitectureTextTerminal] {
        &self.terminals
    }

    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    #[cfg(test)]
    pub(crate) fn terminal_count(&self) -> usize {
        self.terminals.len()
    }
}

#[cfg(test)]
mod architecture_text_receipt_tests {
    use super::*;

    #[test]
    fn receipt_digest_binds_terminal_role_identity_value_and_region() {
        let terminal = |role, identity, value, region| {
            ArchitectureTextTerminal::new(role, identity, value, region)
                .expect("valid Architecture text terminal")
        };
        let baseline = ArchitectureTextReceipt::seal(vec![terminal(
            ArchitectureTextRole::EdgeLabel,
            "api->worker#0",
            "#dc2626",
            [10.0, 20.0, 30.0, 40.0],
        )])
        .expect("seal receipt");

        assert_eq!(baseline.terminal_count(), 1);
        for changed in [
            terminal(
                ArchitectureTextRole::Service,
                "api->worker#0",
                "#dc2626",
                [10.0, 20.0, 30.0, 40.0],
            ),
            terminal(
                ArchitectureTextRole::EdgeLabel,
                "api->db#0",
                "#dc2626",
                [10.0, 20.0, 30.0, 40.0],
            ),
            terminal(
                ArchitectureTextRole::EdgeLabel,
                "api->worker#0",
                "transparent",
                [10.0, 20.0, 30.0, 40.0],
            ),
            terminal(
                ArchitectureTextRole::EdgeLabel,
                "api->worker#0",
                "#dc2626",
                [11.0, 20.0, 30.0, 40.0],
            ),
        ] {
            let changed =
                ArchitectureTextReceipt::seal(vec![changed]).expect("seal changed receipt");
            assert_ne!(baseline.digest(), changed.digest());
        }
    }

    #[test]
    fn invalid_or_empty_terminal_sets_do_not_seal() {
        assert!(ArchitectureTextReceipt::seal(Vec::new()).is_none());
        assert!(
            ArchitectureTextTerminal::new(
                ArchitectureTextRole::Service,
                "api",
                "#dc2626",
                [0.0, 0.0, 0.0, 10.0],
            )
            .is_none()
        );
    }
}

fn update_len(hasher: &mut Sha256, len: usize) {
    hasher.update(u64::try_from(len).unwrap_or(u64::MAX).to_be_bytes());
}

fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    update_len(hasher, bytes.len());
    hasher.update(bytes);
}
