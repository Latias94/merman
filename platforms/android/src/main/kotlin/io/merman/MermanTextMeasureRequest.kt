package io.merman

data class MermanTextMeasureRequest(
    val text: String,
    val fontFamily: String,
    val fontSize: Double,
    val fontWeight: String,
    val fontStyle: String,
    val maxWidth: Double?,
    val lineHeight: Double,
    val letterSpacing: Double,
    val wordSpacing: Double,
    val wrapMode: Int,
    val direction: Int,
    val whiteSpace: Int,
)
