package io.merman

/** Immutable services installed while constructing a reusable [MermanEngine]. */
class MermanEngineServices(
    val iconRegistry: MermanIconRegistry? = null,
    val textMeasurer: MermanTextMeasurer? = null,
)
