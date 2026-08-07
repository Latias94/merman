use merman_bindings_core::ValidatedArtifactContract;

static ARTIFACT_CONTRACT: ValidatedArtifactContract =
    merman_bindings_core::native_sdk_artifact_contract!(AndroidJni);

pub(crate) fn android_artifact_contract() -> &'static ValidatedArtifactContract {
    &ARTIFACT_CONTRACT
}
