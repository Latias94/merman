use super::MANIFEST_SCHEMA_VERSION;
use crate::XtaskError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct UpstreamSvgSource {
    pub(super) mermaid_version: String,
    pub(super) mermaid_cli_version: String,
    pub(super) mermaid_source_tag: String,
    pub(super) mermaid_source_commit: String,
    pub(super) package_json_sha256: String,
    pub(super) package_lock_sha256: String,
    pub(super) mermaid_config_sha256: String,
    pub(super) renderer_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct UpstreamSvgFixtureProvenance {
    pub(super) input_sha256: String,
    pub(super) svg_sha256: String,
    pub(super) renderer_profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct UpstreamSvgExcludedFixture {
    pub(super) input_sha256: String,
    pub(super) reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum UpstreamSvgAttestationMode {
    Generated,
    AdoptedExisting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UpstreamSvgBrowserEnvironment {
    pub(crate) product: String,
    pub(crate) version: String,
    pub(crate) revision: String,
    pub(crate) timezone: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UpstreamSvgPuppeteerEnvironment {
    pub(crate) version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UpstreamSvgOperatingSystemEnvironment {
    pub(crate) platform: String,
    pub(crate) arch: String,
    pub(crate) release: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UpstreamSvgRuntimeEnvironment {
    pub(crate) esm_version: String,
    pub(crate) iife_version: String,
    pub(crate) mermaid_package_sha256: String,
    pub(crate) mermaid_cli_package_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UpstreamSvgFontProbeEnvironment {
    pub(crate) revision: String,
    pub(crate) sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UpstreamSvgRenderEnvironment {
    pub(crate) browser: UpstreamSvgBrowserEnvironment,
    pub(crate) puppeteer: UpstreamSvgPuppeteerEnvironment,
    pub(crate) operating_system: UpstreamSvgOperatingSystemEnvironment,
    pub(crate) mermaid_runtime: UpstreamSvgRuntimeEnvironment,
    pub(crate) font_probe: UpstreamSvgFontProbeEnvironment,
}

impl UpstreamSvgRenderEnvironment {
    pub(crate) fn validate(&self) -> Result<(), XtaskError> {
        for (field, value) in [
            ("browser.product", self.browser.product.as_str()),
            ("browser.version", self.browser.version.as_str()),
            ("browser.revision", self.browser.revision.as_str()),
            ("browser.timezone", self.browser.timezone.as_str()),
            ("puppeteer.version", self.puppeteer.version.as_str()),
            (
                "operating_system.platform",
                self.operating_system.platform.as_str(),
            ),
            ("operating_system.arch", self.operating_system.arch.as_str()),
            (
                "operating_system.release",
                self.operating_system.release.as_str(),
            ),
            (
                "mermaid_runtime.esm_version",
                self.mermaid_runtime.esm_version.as_str(),
            ),
            (
                "mermaid_runtime.iife_version",
                self.mermaid_runtime.iife_version.as_str(),
            ),
            (
                "mermaid_runtime.mermaid_package_sha256",
                self.mermaid_runtime.mermaid_package_sha256.as_str(),
            ),
            (
                "mermaid_runtime.mermaid_cli_package_sha256",
                self.mermaid_runtime.mermaid_cli_package_sha256.as_str(),
            ),
            ("font_probe.revision", self.font_probe.revision.as_str()),
            ("font_probe.sha256", self.font_probe.sha256.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(XtaskError::UpstreamSvgFailed(format!(
                    "generated upstream SVG render environment field {field} must not be empty"
                )));
            }
        }
        for (field, digest) in [
            ("font_probe.sha256", self.font_probe.sha256.as_str()),
            (
                "mermaid_runtime.mermaid_package_sha256",
                self.mermaid_runtime.mermaid_package_sha256.as_str(),
            ),
            (
                "mermaid_runtime.mermaid_cli_package_sha256",
                self.mermaid_runtime.mermaid_cli_package_sha256.as_str(),
            ),
        ] {
            if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(XtaskError::UpstreamSvgFailed(format!(
                    "generated upstream SVG render environment {field} must be a 64-character hexadecimal SHA-256 digest"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "mode", rename_all = "kebab-case")]
pub(super) enum UpstreamSvgAttestation {
    Generated {
        render_environment: Box<UpstreamSvgRenderEnvironment>,
    },
    AdoptedExisting,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUpstreamSvgAttestation {
    mode: UpstreamSvgAttestationMode,
    render_environment: Option<UpstreamSvgRenderEnvironment>,
}

impl<'de> Deserialize<'de> for UpstreamSvgAttestation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawUpstreamSvgAttestation::deserialize(deserializer)?;
        match (raw.mode, raw.render_environment) {
            (UpstreamSvgAttestationMode::Generated, Some(render_environment)) => {
                Ok(Self::generated(render_environment))
            }
            (UpstreamSvgAttestationMode::Generated, None) => {
                Err(serde::de::Error::missing_field("render_environment"))
            }
            (UpstreamSvgAttestationMode::AdoptedExisting, None) => Ok(Self::adopted_existing()),
            (UpstreamSvgAttestationMode::AdoptedExisting, Some(_)) => {
                Err(serde::de::Error::custom(
                    "adopted-existing attestation must not carry a render_environment",
                ))
            }
        }
    }
}

impl UpstreamSvgAttestation {
    pub(super) fn generated(render_environment: UpstreamSvgRenderEnvironment) -> Self {
        Self::Generated {
            render_environment: Box::new(render_environment),
        }
    }

    pub(super) fn adopted_existing() -> Self {
        Self::AdoptedExisting
    }

    pub(super) fn mode(&self) -> UpstreamSvgAttestationMode {
        match self {
            Self::Generated { .. } => UpstreamSvgAttestationMode::Generated,
            Self::AdoptedExisting => UpstreamSvgAttestationMode::AdoptedExisting,
        }
    }

    #[cfg(test)]
    pub(super) fn render_environment(&self) -> Option<&UpstreamSvgRenderEnvironment> {
        match self {
            Self::Generated { render_environment } => Some(render_environment.as_ref()),
            Self::AdoptedExisting => None,
        }
    }

    pub(super) fn validate(&self) -> Result<(), XtaskError> {
        match self {
            Self::Generated { render_environment } => render_environment.validate(),
            Self::AdoptedExisting => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct UpstreamSvgManifest {
    pub(super) schema_version: u32,
    pub(super) source: UpstreamSvgSource,
    pub(super) attestation: UpstreamSvgAttestation,
    pub(super) complete: bool,
    pub(super) fixtures: BTreeMap<String, UpstreamSvgFixtureProvenance>,
    pub(super) excluded: BTreeMap<String, UpstreamSvgExcludedFixture>,
}

impl UpstreamSvgManifest {
    pub(super) fn empty(source: UpstreamSvgSource, attestation: UpstreamSvgAttestation) -> Self {
        Self {
            schema_version: MANIFEST_SCHEMA_VERSION,
            source,
            attestation,
            complete: false,
            fixtures: BTreeMap::new(),
            excluded: BTreeMap::new(),
        }
    }
}
