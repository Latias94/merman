package io.merman

import org.json.JSONObject

class MermanException private constructor(
    rawMessage: String,
    payload: JSONObject?,
) : RuntimeException(payload?.optString("message")?.takeIf(String::isNotEmpty) ?: rawMessage) {
    constructor(message: String) : this(message, parsePayload(message))

    val code: Int? = payload?.takeIf { it.has("code") && !it.isNull("code") }?.optInt("code")
    val codeName: String? = payload?.optString("code_name")?.takeIf(String::isNotEmpty)
    val kind: MermanErrorKind = MermanErrorKind.fromWireName(payload?.optString("kind"))
    val capabilityId: String? = payload
        ?.takeIf { it.has("capability_id") && !it.isNull("capability_id") }
        ?.optString("capability_id")
        ?.takeIf(String::isNotEmpty)

    private companion object {
        fun parsePayload(message: String): JSONObject? =
            runCatching { JSONObject(message) }.getOrNull()
    }
}
