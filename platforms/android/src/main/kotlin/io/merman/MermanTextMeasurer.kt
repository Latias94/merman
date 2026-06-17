package io.merman

fun interface MermanTextMeasurer {
    fun measure(request: MermanTextMeasureRequest): MermanTextMeasureResult?
}
