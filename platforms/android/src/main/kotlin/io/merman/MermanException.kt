package io.merman

import org.json.JSONObject

data class MermanResourceErrorDetails(
    val limitId: String,
    val phase: String,
    val actual: Long,
    val max: Long,
    val profile: String,
)

data class MermanIconRegistryErrorDetails(
    val kindId: String,
    val packIndex: Long?,
    val registrationName: String?,
)

class MermanException private constructor(
    rawMessage: String,
    payload: JSONObject?,
    localResourceDetails: MermanResourceErrorDetails?,
    localIconRegistryDetails: MermanIconRegistryErrorDetails?,
) : RuntimeException(payload?.optString("message")?.takeIf(String::isNotEmpty) ?: rawMessage) {
    constructor(message: String) : this(message, parsePayload(message), null, null)

    val code: Int? = payload?.takeIf { it.has("code") && !it.isNull("code") }?.optInt("code")
    val codeName: String? = payload?.optString("code_name")?.takeIf(String::isNotEmpty)
    val kind: MermanErrorKind = MermanErrorKind.fromWireName(payload?.optString("kind"))
    val capabilityId: String? = payload
        ?.takeIf { it.has("capability_id") && !it.isNull("capability_id") }
        ?.optString("capability_id")
        ?.takeIf(String::isNotEmpty)
    val resourceDetails: MermanResourceErrorDetails? =
        payload?.let(::parseResourceDetails) ?: localResourceDetails
    val iconRegistryDetails: MermanIconRegistryErrorDetails? =
        payload?.let(::parseIconRegistryDetails) ?: localIconRegistryDetails

    internal companion object {
        fun iconRegistryPackCountLimit(
            limit: MermanBindingConstructorResourceLimitSpec,
            actual: Long,
        ): MermanException = MermanException(
            rawMessage = "icon registry pack count exceeds the fixed registry ceiling",
            payload = null,
            localResourceDetails = MermanResourceErrorDetails(
                limitId = limit.id,
                phase = limit.phase,
                actual = actual,
                max = limit.value,
                profile = "constructor-fixed",
            ),
            localIconRegistryDetails = MermanIconRegistryErrorDetails(
                kindId = "resource_limit_exceeded",
                packIndex = null,
                registrationName = null,
            ),
        )

        private fun parsePayload(message: String): JSONObject? =
            runCatching { JSONObject(message) }.getOrNull()

        private fun parseResourceDetails(payload: JSONObject): MermanResourceErrorDetails? = runCatching {
            val resource = payload.optJSONObject("details")?.optJSONObject("resource")
                ?: return null
            val limitId = resource.getString("limit_id").takeIf(String::isNotEmpty) ?: return null
            val phase = resource.getString("phase").takeIf(String::isNotEmpty) ?: return null
            val actual = resource.getLong("actual").takeIf { it >= 0 } ?: return null
            val max = resource.getLong("max").takeIf { it >= 0 } ?: return null
            val profile = resource.getString("profile").takeIf(String::isNotEmpty) ?: return null
            MermanResourceErrorDetails(limitId, phase, actual, max, profile)
        }.getOrNull()

        private fun parseIconRegistryDetails(payload: JSONObject): MermanIconRegistryErrorDetails? =
            runCatching {
                val iconRegistry = payload.optJSONObject("details")?.optJSONObject("icon_registry")
                    ?: return null
                val kindId = iconRegistry.getString("kind_id")
                    .takeIf(String::isNotEmpty) ?: return null
                val packIndex = if (
                    iconRegistry.has("pack_index") && !iconRegistry.isNull("pack_index")
                ) {
                    iconRegistry.getLong("pack_index").takeIf { it >= 0 } ?: return null
                } else {
                    null
                }
                val registrationName = if (
                    iconRegistry.has("registration_name") &&
                    !iconRegistry.isNull("registration_name")
                ) {
                    iconRegistry.getString("registration_name")
                } else {
                    null
                }
                MermanIconRegistryErrorDetails(kindId, packIndex, registrationName)
            }.getOrNull()
    }
}
