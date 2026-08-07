package io.merman

import org.json.JSONObject

/** Typed schema-1 metadata returned with every operation result. */
data class MermanOperationMetadata(
    val version: Int,
    val operationId: String,
    val mediaType: String,
    val runtimePolicy: String,
    val byteLength: Long,
    val outputPlan: MermanOutputPlan?,
    val rawJson: String,
) {
    /** Complete metadata document, including fields unknown to this SDK version. */
    val jsonObject: JSONObject
        get() = JSONObject(rawJson)
}

/** Open output-plan vocabulary carried by [MermanOperationMetadata]. */
abstract class MermanOutputPlan internal constructor() {
    abstract val kind: String
}

/** Effective raster dimensions after resource-limit planning. */
data class MermanRasterOutputPlan(
    val requestedWidthPx: Double,
    val requestedHeightPx: Double,
    val widthPx: Int,
    val heightPx: Int,
    val requestedScale: Double,
    val effectiveScale: Double,
    val limited: Boolean,
) : MermanOutputPlan() {
    override val kind: String = "raster"
}

/** Effective rasterization budget for SVG filter groups embedded in a PDF. */
data class MermanPdfFilterImagesOutputPlan(
    val filteredGroups: Long,
    val requestedScale: Double,
    val effectiveScale: Double,
    val requestedImagePixels: Long,
    val effectiveImagePixels: Long,
    val limited: Boolean,
) : MermanOutputPlan() {
    override val kind: String = "pdf-filter-images"
}

/** Future output-plan kind preserved without forcing exhaustive source switches. */
data class MermanUnknownOutputPlan(
    override val kind: String,
    val rawJson: String,
) : MermanOutputPlan() {
    val jsonObject: JSONObject
        get() = JSONObject(rawJson)
}
