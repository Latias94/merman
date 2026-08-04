package io.merman

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.fail
import org.junit.Test

class MermanRuntimeCatalogValidatorTest {
    @Test
    fun knownResourceLimitValuesAreRuntimeImmutable() {
        @Suppress("UNCHECKED_CAST")
        val values = MermanResourceLimitId.knownValues as MutableList<MermanResourceLimitId>
        val original = values.first()

        try {
            values[0] = MermanResourceLimitId.MAX_MODEL_ITEMS
            fail("known resource limit values accepted mutation")
        } catch (_: UnsupportedOperationException) {
        }

        assertSame(original, MermanResourceLimitId.knownValues.first())
    }

    @Test
    fun runtimeResourceLimitIdsPreserveUnknownFutureValues() {
        val known = MermanResourceLimitId.fromId("max_source_bytes")
        assertSame(MermanResourceLimitId.MAX_SOURCE_BYTES, known)
        assertEquals("source", known.phase)

        val future = MermanResourceLimitId.fromId("future_limit")
        assertEquals("future_limit", future.id)
        assertFalse(future.isKnown)
        assertNull(future.phase)
        assertNull(future.overridable)
        assertNull(future.minimumValue)
        assertEquals(future, MermanResourceLimitId.fromId("future_limit"))
    }

    @Test
    fun acceptsCurrentHandshakeAndAdditiveFieldsWithoutLoadingNativeLibrary() {
        val catalog = validCatalog()
            .put("future_catalog_field", JSONObject().put("version", 1))
        catalog.getJSONArray("options_schema_versions")
            .put(MERMAN_BINDING_OPTIONS_SCHEMA_VERSION)
            .put(99)

        assertEquals(
            catalog.toString(),
            MermanRuntimeCatalogValidator.validate(catalog.toString()),
        )

        val withoutTextMeasurement = validCatalog()
        withoutTextMeasurement
            .getJSONObject("capabilities")
            .put("text_measurement", JSONObject.NULL)
        assertEquals(
            withoutTextMeasurement.toString(),
            MermanRuntimeCatalogValidator.validate(withoutTextMeasurement.toString()),
        )
    }

    @Test
    fun rejectsMalformedOrIncompatibleHandshakeWithoutLoadingNativeLibrary() {
        val invalidCatalogs = listOf(
            "not-json",
            "[]",
            JSONObject().toString(),
            validCatalog().put("schema_version", "1").toString(),
            validCatalog().toString().replace("\"schema_version\":1", "\"schema_version\":1.5"),
            validCatalog().put("schema_version", 2).toString(),
            validCatalog().put("transport_api_version", "1").toString(),
            validCatalog().toString().replace(
                "\"transport_api_version\":1",
                "\"transport_api_version\":1.5",
            ),
            validCatalog().put("transport_api_version", 2).toString(),
            validCatalog().put("package_version", "").toString(),
            validCatalog().put("package_version", 1).toString(),
            validCatalog().removeAndReturn("options_schema_versions"),
            validCatalog().put("options_schema_versions", JSONArray().put(1)).toString(),
            validCatalog().put("options_schema_versions", JSONArray().put("2")).toString(),
            validCatalog().toString().replace(
                "\"options_schema_versions\":[2]",
                "\"options_schema_versions\":[2.5]",
            ),
            validCatalog().removeAndReturn("payload_schemas"),
            validCatalog().put(
                "payload_schemas",
                JSONArray().put(
                    JSONObject()
                        .put("id", "binding-result")
                        .put("version", 2),
                ),
            ).toString(),
            validCatalog().put(
                "payload_schemas",
                JSONArray().put(
                    JSONObject()
                        .put("id", "binding-result")
                        .put("version", "1"),
                ),
            ).toString(),
            validCatalog().removeAndReturn("capabilities"),
            validCatalog().withTextMeasurement(JSONObject().put("protocol_version", 2)),
            validCatalog().withTextMeasurement(JSONObject().put("protocol_version", "1")),
            validCatalog().withTextMeasurement("invalid"),
        )

        invalidCatalogs.forEach(::assertRejected)
    }

    private fun assertRejected(catalog: String) {
        try {
            MermanRuntimeCatalogValidator.validate(catalog)
            fail("malformed runtime catalog was accepted: $catalog")
        } catch (_: MermanException) {
        }
    }

    private fun validCatalog(): JSONObject = JSONObject()
        .put("schema_version", 1)
        .put("transport_api_version", ANDROID_TRANSPORT_API_VERSION)
        .put("package_version", "test")
        .put(
            "options_schema_versions",
            JSONArray().put(MERMAN_BINDING_OPTIONS_SCHEMA_VERSION),
        )
        .put(
            "payload_schemas",
            JSONArray()
                .put(
                    JSONObject()
                        .put("id", "binding-result")
                        .put("version", 1),
                )
                .put(
                    JSONObject()
                        .put("id", "operation-metadata")
                        .put("version", 1),
                ),
        )
        .put(
            "capabilities",
            JSONObject().put(
                "text_measurement",
                JSONObject().put(
                    "protocol_version",
                    MermanTextMeasurementOperation.PROTOCOL_VERSION,
                ),
            ),
        )

    private fun JSONObject.removeAndReturn(key: String): String {
        remove(key)
        return toString()
    }

    private fun JSONObject.withTextMeasurement(value: Any): String {
        getJSONObject("capabilities").put("text_measurement", value)
        return toString()
    }
}
