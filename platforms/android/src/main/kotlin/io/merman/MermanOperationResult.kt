package io.merman

import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction

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
            throw MermanException.internalContract(
                "Merman result operation ID does not match its metadata",
            )
        }
        if (metadata.mediaType != mediaType) {
            throw MermanException.internalContract(
                "Merman result media type does not match its metadata",
            )
        }
        if (metadata.byteLength != data.size.toLong()) {
            throw MermanException.internalContract(
                "Merman result byte length does not match its metadata",
            )
        }
    }

    /** Decodes a UTF-8 output such as SVG, ASCII, or JSON. */
    val utf8Text: String
        get() {
            val decoder = Charsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
            return try {
                decoder.decode(ByteBuffer.wrap(data)).toString()
            } catch (_: CharacterCodingException) {
                throw MermanException.internalContract(
                    "Merman operation result data is not valid UTF-8",
                )
            }
        }
}
