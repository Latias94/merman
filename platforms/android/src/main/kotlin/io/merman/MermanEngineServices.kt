package io.merman

/** Immutable services installed while constructing a reusable [MermanEngine]. */
class MermanEngineServices(
    val iconPackSet: MermanIconPackSet? = null,
    val textMeasurer: MermanTextMeasurer? = null,
)
