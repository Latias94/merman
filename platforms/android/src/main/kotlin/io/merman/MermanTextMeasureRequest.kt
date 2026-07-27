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
    val wrapMode: MermanTextWrapMode,
    val direction: MermanTextDirection,
    val whiteSpace: MermanTextWhiteSpace,
    val phase: MermanTextMeasurementPhase,
    val operation: Int,
) {
    private constructor(
        text: String,
        fontFamily: String,
        fontSize: Double,
        fontWeight: String,
        fontStyle: String,
        maxWidth: Double?,
        lineHeight: Double,
        letterSpacing: Double,
        wordSpacing: Double,
        wrapMode: Int,
        direction: Int,
        whiteSpace: Int,
        phase: Int,
        operation: Int,
    ) : this(
        text = text,
        fontFamily = fontFamily,
        fontSize = fontSize,
        fontWeight = fontWeight,
        fontStyle = fontStyle,
        maxWidth = maxWidth,
        lineHeight = lineHeight,
        letterSpacing = letterSpacing,
        wordSpacing = wordSpacing,
        wrapMode = requireNotNull(MermanTextWrapMode.fromCode(wrapMode)) {
            "unknown text wrap-mode code $wrapMode"
        },
        direction = requireNotNull(MermanTextDirection.fromCode(direction)) {
            "unknown text direction code $direction"
        },
        whiteSpace = requireNotNull(MermanTextWhiteSpace.fromCode(whiteSpace)) {
            "unknown text white-space code $whiteSpace"
        },
        phase = requireNotNull(MermanTextMeasurementPhase.fromCode(phase)) {
            "unknown text-measurement phase code $phase"
        },
        operation = operation,
    )
}
