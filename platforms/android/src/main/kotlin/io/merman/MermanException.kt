package io.merman

import org.json.JSONObject

data class MermanResourceErrorDetails(
    val cause: String,
    val limitId: String,
    val phase: String,
    val actual: Long,
    val max: Long,
    val profile: String,
)

data class MermanDiagnosticSpan(
    val start: Long,
    val end: Long,
    val kind: String,
)

data class MermanDiagnosticErrorDetails(
    val code: String,
    val span: MermanDiagnosticSpan?,
    val field: String?,
    val diagramType: String?,
)

data class MermanIconRegistryErrorDetails(
    val kindId: String,
    val packIndex: Long?,
    val registrationName: String?,
)

class MermanException private constructor(
    rawMessage: String,
    payload: JSONObject?,
    localCode: Int?,
    localCodeName: String?,
    localResourceDetails: MermanResourceErrorDetails?,
    localDiagnosticDetails: MermanDiagnosticErrorDetails?,
    localIconRegistryDetails: MermanIconRegistryErrorDetails?,
) : RuntimeException(payload?.optString("message")?.takeIf(String::isNotEmpty) ?: rawMessage) {
    constructor(message: String) : this(message, parsePayload(message), null, null, null, null, null)

    val code: Int? = payload
        ?.takeIf { it.has("code") && !it.isNull("code") }
        ?.optInt("code")
        ?: localCode
    val codeName: String? = payload
        ?.optString("code_name")
        ?.takeIf(String::isNotEmpty)
        ?: localCodeName
    val kind: MermanErrorKind = MermanErrorKind.fromWireName(payload?.optString("kind"))
    val capabilityId: String? = payload
        ?.takeIf { it.has("capability_id") && !it.isNull("capability_id") }
        ?.optString("capability_id")
        ?.takeIf(String::isNotEmpty)
    val resourceDetails: MermanResourceErrorDetails? =
        payload?.let(::parseResourceDetails) ?: localResourceDetails
    val diagnosticDetails: MermanDiagnosticErrorDetails? =
        payload?.let(::parseDiagnosticDetails) ?: localDiagnosticDetails
    val iconRegistryDetails: MermanIconRegistryErrorDetails? =
        payload?.let(::parseIconRegistryDetails) ?: localIconRegistryDetails

    internal companion object {
        private const val INTERNAL_ERROR_CODE = 9
        private const val INTERNAL_ERROR_CODE_NAME = "MERMAN_INTERNAL_ERROR"
        private const val RESOURCE_LIMIT_ERROR_CODE = 10
        private const val RESOURCE_LIMIT_ERROR_CODE_NAME = "MERMAN_RESOURCE_LIMIT_EXCEEDED"

        fun iconPackCountLimit(
            limit: MermanBindingConstructorResourceLimitSpec,
            actual: Long,
        ): MermanException = MermanException(
            rawMessage = "icon pack count exceeds the fixed registry ceiling",
            payload = null,
            localCode = RESOURCE_LIMIT_ERROR_CODE,
            localCodeName = RESOURCE_LIMIT_ERROR_CODE_NAME,
            localResourceDetails = MermanResourceErrorDetails(
                cause = "ceiling",
                limitId = limit.id,
                phase = limit.phase,
                actual = actual,
                max = limit.value,
                profile = "constructor-fixed",
            ),
            localDiagnosticDetails = null,
            localIconRegistryDetails = MermanIconRegistryErrorDetails(
                kindId = "resource_limit_exceeded",
                packIndex = null,
                registrationName = null,
            ),
        )

        fun internalContract(message: String): MermanException = MermanException(
            rawMessage = message,
            payload = null,
            localCode = INTERNAL_ERROR_CODE,
            localCodeName = INTERNAL_ERROR_CODE_NAME,
            localResourceDetails = null,
            localDiagnosticDetails = null,
            localIconRegistryDetails = null,
        )

        private fun parsePayload(message: String): JSONObject? =
            runCatching { JSONObject(message) }.getOrNull()

        private fun parseResourceDetails(payload: JSONObject): MermanResourceErrorDetails? = runCatching {
            val resource = payload.optJSONObject("details")?.optJSONObject("resource")
                ?: return null
            val cause = resource.getString("cause").takeIf(String::isNotEmpty) ?: return null
            val limitId = resource.getString("limit_id").takeIf(String::isNotEmpty) ?: return null
            val phase = resource.getString("phase").takeIf(String::isNotEmpty) ?: return null
            val actual = resource.getLong("actual").takeIf { it >= 0 } ?: return null
            val max = resource.getLong("max").takeIf { it >= 0 } ?: return null
            val profile = resource.getString("profile").takeIf(String::isNotEmpty) ?: return null
            MermanResourceErrorDetails(cause, limitId, phase, actual, max, profile)
        }.getOrNull()

        private fun parseDiagnosticDetails(payload: JSONObject): MermanDiagnosticErrorDetails? =
            runCatching {
                val diagnostic = payload.optJSONObject("details")?.optJSONObject("diagnostic")
                    ?: return null
                val code = diagnostic.getString("code").takeIf(String::isNotEmpty) ?: return null
                val span = diagnostic.optJSONObject("span")?.let { value ->
                    val start = value.getLong("start").takeIf { it >= 0 } ?: return null
                    val end = value.getLong("end").takeIf { it >= start } ?: return null
                    val kind = value.getString("kind").takeIf {
                        it == "exact" || it == "insertion-point" || it == "fallback"
                    } ?: return null
                    MermanDiagnosticSpan(start, end, kind)
                }
                val field = if (diagnostic.has("field") && !diagnostic.isNull("field")) {
                    diagnostic.getString("field")
                } else {
                    null
                }
                val diagramType = if (
                    diagnostic.has("diagram_type") && !diagnostic.isNull("diagram_type")
                ) {
                    diagnostic.getString("diagram_type")
                } else {
                    null
                }
                MermanDiagnosticErrorDetails(code, span, field, diagramType)
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
