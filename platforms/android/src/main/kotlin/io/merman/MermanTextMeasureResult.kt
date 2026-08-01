package io.merman

/**
 * A handled host text-measurement result with a protocol-complete shape.
 *
 * Use the named factories so every field required by [resultKind] is explicit. Returning `null`
 * from [MermanTextMeasurer.measure] keeps an operation unhandled and uses Merman's configured
 * fallback.
 */
class MermanTextMeasureResult private constructor(
    val resultKind: Int,
    val width: Double,
    val height: Double,
    val length: Double,
    val lineCount: Long,
    val bboxLeft: Double,
    val bboxRight: Double,
    val rawWidth: Double,
    val hasRawWidth: Boolean,
) {
    companion object {
        /** Creates a metrics result with every required field. */
        @JvmStatic
        fun metrics(
            width: Double,
            height: Double,
            lineCount: Long,
        ): MermanTextMeasureResult {
            requireNonNegativeFinite(width, "width")
            requireNonNegativeFinite(height, "height")
            require(lineCount > 0) { "lineCount must be greater than 0" }
            return MermanTextMeasureResult(
                resultKind = MermanTextMeasurementResultKind.METRICS,
                width = width,
                height = height,
                length = 0.0,
                lineCount = lineCount,
                bboxLeft = 0.0,
                bboxRight = 0.0,
                rawWidth = 0.0,
                hasRawWidth = false,
            )
        }

        /**
         * Creates a finite scalar result.
         *
         * Signed values are accepted because the two baseline-offset operations require them. The
         * native operation contract rejects negative values for every other scalar operation.
         */
        @JvmStatic
        fun length(length: Double): MermanTextMeasureResult {
            require(length.isFinite()) { "length must be finite" }
            return MermanTextMeasureResult(
                resultKind = MermanTextMeasurementResultKind.LENGTH,
                width = 0.0,
                height = 0.0,
                length = length,
                lineCount = 0,
                bboxLeft = 0.0,
                bboxRight = 0.0,
                rawWidth = 0.0,
                hasRawWidth = false,
            )
        }

        /** Creates a non-negative horizontal-extents result. */
        @JvmStatic
        fun horizontalExtents(
            left: Double,
            right: Double,
        ): MermanTextMeasureResult {
            requireNonNegativeFinite(left, "left")
            requireNonNegativeFinite(right, "right")
            return MermanTextMeasureResult(
                resultKind = MermanTextMeasurementResultKind.HORIZONTAL_EXTENTS,
                width = 0.0,
                height = 0.0,
                length = 0.0,
                lineCount = 0,
                bboxLeft = left,
                bboxRight = right,
                rawWidth = 0.0,
                hasRawWidth = false,
            )
        }

        /** Creates wrapped metrics with an optional natural, unwrapped width. */
        @JvmStatic
        @JvmOverloads
        fun wrappedWithRawWidth(
            width: Double,
            height: Double,
            lineCount: Long,
            rawWidth: Double? = null,
        ): MermanTextMeasureResult {
            requireNonNegativeFinite(width, "width")
            requireNonNegativeFinite(height, "height")
            require(lineCount > 0) { "lineCount must be greater than 0" }
            if (rawWidth != null) {
                requireNonNegativeFinite(rawWidth, "rawWidth")
            }
            return MermanTextMeasureResult(
                resultKind = MermanTextMeasurementResultKind.WRAPPED_WITH_RAW_WIDTH,
                width = width,
                height = height,
                length = 0.0,
                lineCount = lineCount,
                bboxLeft = 0.0,
                bboxRight = 0.0,
                rawWidth = rawWidth ?: 0.0,
                hasRawWidth = rawWidth != null,
            )
        }

        private fun requireNonNegativeFinite(value: Double, name: String) {
            require(value.isFinite()) { "$name must be finite" }
            require(value >= 0.0) { "$name must be non-negative" }
        }
    }
}
