package io.merman

import org.json.JSONTokener
import org.json.JSONObject

/** Compatibility projection for resource counts that fit a signed [Long]. */
data class MermanResourceErrorDetails(
    val cause: String,
    val limitId: String,
    val phase: String,
    val actual: Long,
    val max: Long,
    val profile: String,
)

/** Lossless projection for native unsigned 64-bit resource counts. */
data class MermanExactResourceErrorDetails(
    val cause: String,
    val limitId: String,
    val phase: String,
    val actual: String,
    val max: String,
    val profile: String,
)

private data class ParsedResourceErrorDetails(
    val exact: MermanExactResourceErrorDetails,
    val compatible: MermanResourceErrorDetails?,
)

private data class DecodedMermanErrorMessage(
    val message: String,
    val payload: JSONObject?,
    val localCode: Int?,
    val localCodeName: String?,
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
    val requestedMaxWidth: Long? = null,
    val actualWidth: Long? = null,
    val widthProfile: String? = null,
    val fallbackReason: String? = null,
)

data class MermanIconRegistryErrorDetails(
    val kindId: String,
    val packIndex: Long?,
    val registrationName: String?,
)

data class MermanCancelledDetails(
    val reason: String,
    val phase: String,
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
    private constructor(decoded: DecodedMermanErrorMessage) : this(
        rawMessage = decoded.message,
        payload = decoded.payload,
        localCode = decoded.localCode,
        localCodeName = decoded.localCodeName,
        localResourceDetails = null,
        localDiagnosticDetails = null,
        localIconRegistryDetails = null,
    )

    constructor(message: String) : this(decodeErrorMessage(message))

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
    private val parsedResourceDetails: ParsedResourceErrorDetails? =
        payload?.let(::parseResourceDetails)
    val exactResourceDetails: MermanExactResourceErrorDetails? =
        parsedResourceDetails?.exact ?: localResourceDetails?.toExactResourceDetails()
    val resourceDetails: MermanResourceErrorDetails? =
        parsedResourceDetails?.compatible ?: localResourceDetails
    val diagnosticDetails: MermanDiagnosticErrorDetails? =
        payload?.let(::parseDiagnosticDetails) ?: localDiagnosticDetails
    val iconRegistryDetails: MermanIconRegistryErrorDetails? =
        payload?.let(::parseIconRegistryDetails) ?: localIconRegistryDetails
    val cancellationDetails: MermanCancelledDetails? =
        payload?.let(::parseCancellationDetails)

    internal companion object {
        private const val INTERNAL_ERROR_CODE = 9
        private const val INTERNAL_ERROR_CODE_NAME = "MERMAN_INTERNAL_ERROR"
        private const val RESOURCE_LIMIT_ERROR_CODE = 10
        private const val RESOURCE_LIMIT_ERROR_CODE_NAME = "MERMAN_RESOURCE_LIMIT_EXCEEDED"
        private const val CANCELLED_ERROR_CODE = 12
        private const val INVALID_NATIVE_ERROR_MESSAGE =
            "Merman Android transport returned an invalid error payload"
        private val STATUS_CODE_NAMES = mapOf(
            1 to "MERMAN_INVALID_ARGUMENT",
            2 to "MERMAN_UTF8_ERROR",
            3 to "MERMAN_OPTIONS_JSON_ERROR",
            4 to "MERMAN_NO_DIAGRAM",
            5 to "MERMAN_PARSE_ERROR",
            6 to "MERMAN_RENDER_ERROR",
            7 to "MERMAN_UNSUPPORTED_OPERATION",
            8 to "MERMAN_PANIC",
            INTERNAL_ERROR_CODE to INTERNAL_ERROR_CODE_NAME,
            RESOURCE_LIMIT_ERROR_CODE to RESOURCE_LIMIT_ERROR_CODE_NAME,
            11 to "MERMAN_BUSY",
            CANCELLED_ERROR_CODE to "MERMAN_CANCELLED",
        )
        private val CANCELLATION_REASONS = setOf("requested", "deadline_exceeded")
        private val CANCELLATION_PHASE_PATTERN = Regex("^[a-z][a-z0-9_-]{0,63}$")

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

        private fun decodeErrorMessage(message: String): DecodedMermanErrorMessage {
            val looksLikeJsonObject = message.dropWhile(Char::isWhitespace).startsWith('{')
            val payload = parseJsonObject(message)
            if (payload == null) {
                return if (looksLikeJsonObject) invalidNativeErrorMessage() else {
                    DecodedMermanErrorMessage(message, null, null, null)
                }
            }
            return if (isValidNativeErrorPayload(payload)) {
                DecodedMermanErrorMessage(message, payload, null, null)
            } else {
                invalidNativeErrorMessage()
            }
        }

        private fun parseJsonObject(message: String): JSONObject? = runCatching {
            val tokenizer = JSONTokener(message)
            val payload = tokenizer.nextValue() as? JSONObject ?: return null
            if (tokenizer.nextClean() != '\u0000') {
                return null
            }
            payload
        }.getOrNull()

        private fun invalidNativeErrorMessage(): DecodedMermanErrorMessage =
            DecodedMermanErrorMessage(
                message = INVALID_NATIVE_ERROR_MESSAGE,
                payload = null,
                localCode = INTERNAL_ERROR_CODE,
                localCodeName = INTERNAL_ERROR_CODE_NAME,
            )

        private fun isValidNativeErrorPayload(payload: JSONObject): Boolean {
            if (
                payload.strictInt("version") !=
                MERMAN_REQUIRED_PAYLOAD_SCHEMA_VERSIONS["binding-result"] ||
                payload.opt("ok") != false
            ) {
                return false
            }
            val code = payload.strictInt("code") ?: return false
            val codeName = payload.strictString("code_name") ?: return false
            if (STATUS_CODE_NAMES[code] != codeName) {
                return false
            }
            val kind = payload.strictString("kind") ?: return false
            if (!payload.has("capability_id")) {
                return false
            }
            val capabilityId = if (payload.isNull("capability_id")) {
                null
            } else {
                payload.strictString("capability_id") ?: return false
            }
            if (payload.opt("message") !is String) {
                return false
            }
            if (!hasValidKindRelation(code, kind, capabilityId)) {
                return false
            }
            return hasValidDetailsRelation(payload, code, kind, capabilityId)
        }

        private fun hasValidKindRelation(
            code: Int,
            kind: String,
            capabilityId: String?,
        ): Boolean = when (kind) {
            "generic" -> capabilityId == null
            "unknown-operation" -> code == 7 && capabilityId == null
            "missing-capability" -> code == 7 && capabilityId != null
            "busy" -> code == 11 && capabilityId == null
            "reentrant-call" -> code == 1 && capabilityId == null
            else -> false
        }

        private fun hasValidDetailsRelation(
            payload: JSONObject,
            code: Int,
            kind: String,
            capabilityId: String?,
        ): Boolean {
            val details = if (payload.has("details")) {
                payload.optJSONObject("details") ?: return false
            } else {
                null
            }
            val hasResource = details?.has("resource") == true
            val hasDiagnostic = details?.has("diagnostic") == true
            val hasIconRegistry = details?.has("icon_registry") == true
            val hasCancellation = details?.has("cancellation") == true
            if (hasResource && parseResourceDetails(payload) == null) {
                return false
            }
            if (hasDiagnostic && parseDiagnosticDetails(payload) == null) {
                return false
            }
            if (hasIconRegistry && parseIconRegistryDetails(payload) == null) {
                return false
            }
            if (hasCancellation && parseCancellationDetails(payload) == null) {
                return false
            }
            if (hasResource && (code != RESOURCE_LIMIT_ERROR_CODE || kind != "generic")) {
                return false
            }
            if (code == RESOURCE_LIMIT_ERROR_CODE && !hasResource) {
                return false
            }
            if (hasIconRegistry && capabilityId != null) {
                return false
            }
            return if (code == CANCELLED_ERROR_CODE) {
                kind == "generic" &&
                    capabilityId == null &&
                    hasCancellation &&
                    !hasResource &&
                    !hasDiagnostic &&
                    !hasIconRegistry
            } else {
                !hasCancellation
            }
        }

        private fun JSONObject.strictInt(key: String): Int? = when (val value = opt(key)) {
            is Byte -> value.toInt()
            is Short -> value.toInt()
            is Int -> value
            is Long -> value.takeIf { it in Int.MIN_VALUE..Int.MAX_VALUE }?.toInt()
            else -> null
        }

        private fun JSONObject.strictString(key: String): String? = opt(key) as? String

        private fun JSONObject.strictNonEmptyString(key: String): String? =
            strictString(key)?.takeIf(String::isNotEmpty)

        private fun JSONObject.strictLong(key: String): Long? = when (val value = opt(key)) {
            is Byte -> value.toLong()
            is Short -> value.toLong()
            is Int -> value.toLong()
            is Long -> value
            else -> null
        }

        private fun parseResourceDetails(payload: JSONObject): ParsedResourceErrorDetails? =
            runCatching {
                val resource = payload.optJSONObject("details")?.optJSONObject("resource")
                    ?: return null
                val cause = resource.strictNonEmptyString("cause") ?: return null
                val limitId = resource.strictNonEmptyString("limit_id") ?: return null
                val phase = resource.strictNonEmptyString("phase") ?: return null
                val actual = resource.unsignedDecimal("actual") ?: return null
                val max = resource.unsignedDecimal("max") ?: return null
                val profile = resource.strictNonEmptyString("profile") ?: return null
                ParsedResourceErrorDetails(
                    exact = MermanExactResourceErrorDetails(
                        cause,
                        limitId,
                        phase,
                        actual,
                        max,
                        profile,
                    ),
                    compatible = actual.toLongOrNull()?.let { actualLong ->
                        max.toLongOrNull()?.let { maxLong ->
                            MermanResourceErrorDetails(
                                cause,
                                limitId,
                                phase,
                                actualLong,
                                maxLong,
                                profile,
                            )
                        }
                    },
                )
            }.getOrNull()

        private fun JSONObject.unsignedDecimal(key: String): String? {
            val decimal = when (val value = get(key)) {
                is Byte, is Short, is Int, is Long -> value.toString()
                is String -> value
                else -> return null
            }
            if (decimal.isEmpty() || decimal.any { it !in '0'..'9' }) {
                return null
            }
            return decimal.toULongOrNull()?.toString()
        }

        private fun MermanResourceErrorDetails.toExactResourceDetails():
            MermanExactResourceErrorDetails =
            MermanExactResourceErrorDetails(
                cause = cause,
                limitId = limitId,
                phase = phase,
                actual = actual.toString(),
                max = max.toString(),
                profile = profile,
            )

        private fun parseDiagnosticDetails(payload: JSONObject): MermanDiagnosticErrorDetails? =
            runCatching {
                val diagnostic = payload.optJSONObject("details")?.optJSONObject("diagnostic")
                    ?: return null
                val code = diagnostic.strictNonEmptyString("code") ?: return null
                val span = if (!diagnostic.has("span") || diagnostic.isNull("span")) {
                    null
                } else {
                    val value = diagnostic.opt("span") as? JSONObject ?: return null
                    val start = value.strictLong("start")?.takeIf { it >= 0 } ?: return null
                    val end = value.strictLong("end")?.takeIf { it >= start } ?: return null
                    val kind = value.strictNonEmptyString("kind")?.takeIf {
                        it == "exact" || it == "insertion-point" || it == "fallback"
                    } ?: return null
                    if (kind == "insertion-point" && end != start) return null
                    MermanDiagnosticSpan(start, end, kind)
                }
                val field = if (diagnostic.has("field") && !diagnostic.isNull("field")) {
                    diagnostic.strictString("field") ?: return null
                } else {
                    null
                }
                val diagramType = if (
                    diagnostic.has("diagram_type") && !diagnostic.isNull("diagram_type")
                ) {
                    diagnostic.strictString("diagram_type") ?: return null
                } else {
                    null
                }
                val requestedMaxWidth = if (
                    diagnostic.has("requested_max_width") &&
                    !diagnostic.isNull("requested_max_width")
                ) {
                    diagnostic.strictLong("requested_max_width")?.takeIf { it >= 0 } ?: return null
                } else {
                    null
                }
                val actualWidth = if (
                    diagnostic.has("actual_width") && !diagnostic.isNull("actual_width")
                ) {
                    diagnostic.strictLong("actual_width")?.takeIf { it >= 0 } ?: return null
                } else {
                    null
                }
                val widthProfile = if (
                    diagnostic.has("width_profile") && !diagnostic.isNull("width_profile")
                ) {
                    diagnostic.strictString("width_profile") ?: return null
                } else {
                    null
                }
                val fallbackReason = if (
                    diagnostic.has("fallback_reason") &&
                    !diagnostic.isNull("fallback_reason")
                ) {
                    diagnostic.strictString("fallback_reason") ?: return null
                } else {
                    null
                }
                MermanDiagnosticErrorDetails(
                    code = code,
                    span = span,
                    field = field,
                    diagramType = diagramType,
                    requestedMaxWidth = requestedMaxWidth,
                    actualWidth = actualWidth,
                    widthProfile = widthProfile,
                    fallbackReason = fallbackReason,
                )
            }.getOrNull()

        private fun parseIconRegistryDetails(payload: JSONObject): MermanIconRegistryErrorDetails? =
            runCatching {
                val iconRegistry = payload.optJSONObject("details")?.optJSONObject("icon_registry")
                    ?: return null
                val kindId = iconRegistry.strictNonEmptyString("kind_id") ?: return null
                val packIndex = if (
                    iconRegistry.has("pack_index") && !iconRegistry.isNull("pack_index")
                ) {
                    iconRegistry.strictLong("pack_index")?.takeIf { it >= 0 } ?: return null
                } else {
                    null
                }
                val registrationName = if (
                    iconRegistry.has("registration_name") &&
                    !iconRegistry.isNull("registration_name")
                ) {
                    iconRegistry.strictString("registration_name") ?: return null
                } else {
                    null
                }
                MermanIconRegistryErrorDetails(kindId, packIndex, registrationName)
            }.getOrNull()

        private fun parseCancellationDetails(payload: JSONObject): MermanCancelledDetails? =
            runCatching {
                val cancellation = payload.optJSONObject("details")?.optJSONObject("cancellation")
                    ?: return null
                val reason = cancellation.strictNonEmptyString("reason")
                    ?.takeIf(CANCELLATION_REASONS::contains) ?: return null
                val phase = cancellation.strictNonEmptyString("phase")
                    ?.takeIf(CANCELLATION_PHASE_PATTERN::matches) ?: return null
                MermanCancelledDetails(reason, phase)
            }.getOrNull()
    }
}
