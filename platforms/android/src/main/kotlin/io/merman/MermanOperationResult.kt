package io.merman

/** Complete result envelope returned by a catalog operation. */
data class MermanOperationResult(
    val operationId: String,
    val mediaType: String,
    val data: ByteArray,
    val metadataJson: String,
)
