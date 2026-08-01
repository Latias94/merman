package io.merman

import org.json.JSONObject

data class MermanResourceErrorDetails(
    val limitId: String,
    val phase: String,
    val actual: Long,
    val max: Long,
    val profile: String,
)

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
    val resourceDetails: MermanResourceErrorDetails? = payload?.let(::parseResourceDetails)

    private companion object {
        fun parsePayload(message: String): JSONObject? =
            runCatching { JSONObject(message) }.getOrNull()

        fun parseResourceDetails(payload: JSONObject): MermanResourceErrorDetails? = runCatching {
            val resource = payload.optJSONObject("details")?.optJSONObject("resource")
                ?: return null
            val limitId = resource.getString("limit_id").takeIf(String::isNotEmpty) ?: return null
            val phase = resource.getString("phase").takeIf(String::isNotEmpty) ?: return null
            val actual = resource.getLong("actual").takeIf { it >= 0 } ?: return null
            val max = resource.getLong("max").takeIf { it >= 0 } ?: return null
            val profile = resource.getString("profile").takeIf(String::isNotEmpty) ?: return null
            MermanResourceErrorDetails(limitId, phase, actual, max, profile)
        }.getOrNull()
    }
}
