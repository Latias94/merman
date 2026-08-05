package io.merman

/** Complete binary-safe result envelope returned by a catalog operation. */
class MermanOperationResult internal constructor(
    val operationId: String,
    val mediaType: String,
    val data: ByteArray,
    metadataJson: String,
) {
    val metadata: MermanOperationMetadata = decodeMermanOperationMetadata(metadataJson)

    init {
        if (metadata.operationId != operationId) {
            throw MermanException("Merman result operation ID does not match its metadata")
        }
        if (metadata.mediaType != mediaType) {
            throw MermanException("Merman result media type does not match its metadata")
        }
        if (metadata.byteLength != data.size.toLong()) {
            throw MermanException("Merman result byte length does not match its metadata")
        }
    }

    /** Decodes a UTF-8 output such as SVG, ASCII, or JSON. */
    val utf8Text: String
        get() = data.toString(Charsets.UTF_8)
}
