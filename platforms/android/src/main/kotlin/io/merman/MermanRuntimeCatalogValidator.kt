package io.merman

import org.json.JSONArray
import org.json.JSONObject

internal const val ANDROID_TRANSPORT_API_VERSION: Int = 1

/** Validates only the catalog fields consumed by the package-owned Android SDK. */
internal object MermanRuntimeCatalogValidator {
    private const val RUNTIME_CATALOG_SCHEMA_VERSION: Int = 1
    private const val BINDING_RESULT_PAYLOAD_ID: String = "binding-result"
    private const val BINDING_RESULT_PAYLOAD_VERSION: Int = 1

    fun validate(json: String): String {
        val catalog = try {
            JSONObject(json)
        } catch (error: Exception) {
            throw MermanException("Invalid Merman runtime catalog: ${error.message}")
        }

        if (requiredInt(catalog, "schema_version") != RUNTIME_CATALOG_SCHEMA_VERSION) {
            throw MermanException("Unsupported Merman runtime contract schema")
        }
        if (requiredInt(catalog, "transport_api_version") != ANDROID_TRANSPORT_API_VERSION) {
            throw MermanException("Merman Android transport API version mismatch")
        }
        val packageVersion = catalog.opt("package_version")
        if (packageVersion !is String || packageVersion.isEmpty()) {
            throw MermanException("Merman runtime catalog is missing package_version")
        }
        if (!supportsOptionsSchema(catalog.opt("options_schema_versions"))) {
            throw MermanException(
                "Merman runtime catalog does not advertise options schema " +
                    MERMAN_BINDING_OPTIONS_SCHEMA_VERSION,
            )
        }
        requireBindingResultSchema(catalog.opt("payload_schemas"))
        validateTextMeasurementProtocol(catalog.opt("capabilities"))

        return json
    }

    private fun requiredInt(catalog: JSONObject, key: String): Int {
        val raw = catalog.opt(key)
        val value = when (raw) {
            is Int -> raw.toLong()
            is Long -> raw
            else -> throw MermanException(
                "Merman runtime catalog field `$key` must be a JSON integer",
            )
        }
        if (value !in Int.MIN_VALUE.toLong()..Int.MAX_VALUE.toLong()) {
            throw MermanException(
                "Merman runtime catalog field `$key` is outside the supported integer range",
            )
        }
        return value.toInt()
    }

    private fun supportsOptionsSchema(value: Any?): Boolean {
        val versions = value as? JSONArray
            ?: throw MermanException("Merman runtime options schema versions are missing")
        for (index in 0 until versions.length()) {
            val version = when (val raw = versions.opt(index)) {
                is Int -> raw.toLong()
                is Long -> raw
                else -> throw MermanException(
                    "Merman runtime options schema versions must be JSON integers",
                )
            }
            if (version == MERMAN_BINDING_OPTIONS_SCHEMA_VERSION.toLong()) {
                return true
            }
        }
        return false
    }

    private fun requireBindingResultSchema(value: Any?) {
        val schemas = value as? JSONArray
            ?: throw MermanException("Merman runtime payload schemas are missing")
        for (index in 0 until schemas.length()) {
            val schema = schemas.opt(index) as? JSONObject ?: continue
            if (schema.opt("id") != BINDING_RESULT_PAYLOAD_ID) continue
            if (requiredInt(schema, "version") != BINDING_RESULT_PAYLOAD_VERSION) {
                throw MermanException("Merman binding-result payload schema version mismatch")
            }
            return
        }
        throw MermanException("Merman runtime catalog does not advertise binding-result schema 1")
    }

    private fun validateTextMeasurementProtocol(value: Any?) {
        val capabilities = value as? JSONObject
            ?: throw MermanException("Merman runtime capabilities are missing")
        when (val measurement = capabilities.opt("text_measurement")) {
            null, JSONObject.NULL -> Unit
            is JSONObject -> {
                if (
                    requiredInt(measurement, "protocol_version") !=
                    MermanTextMeasurementOperation.PROTOCOL_VERSION
                ) {
                    throw MermanException("Merman text-measurement protocol version mismatch")
                }
            }
            else -> throw MermanException("Merman runtime text-measurement contract is malformed")
        }
    }
}
