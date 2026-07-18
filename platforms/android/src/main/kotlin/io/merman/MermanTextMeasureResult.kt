package io.merman

data class MermanTextMeasureResult(
    val resultKind: Int,
    val width: Double = 0.0,
    val height: Double = 0.0,
    val length: Double = 0.0,
    val lineCount: Long = 0,
    val bboxLeft: Double = 0.0,
    val bboxRight: Double = 0.0,
    val rawWidth: Double = 0.0,
    val hasRawWidth: Boolean = false,
)
