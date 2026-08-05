package io.merman

import androidx.test.ext.junit.runners.AndroidJUnit4
import io.merman.examples.runMermanSmoke
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class MermanInstrumentedSmokeTest {
    @Test
    fun runsPublicSmokeIncludingThrowingTextMeasurerFallback() {
        runMermanSmoke()
    }

    @Test
    fun parsesStructuredResourceFailureDetails() {
        val error = MermanException(
            """{"version":1,"ok":false,"code":10,"code_name":"MERMAN_RESOURCE_LIMIT_EXCEEDED","kind":"generic","capability_id":null,"details":{"resource":{"limit_id":"max_embedded_image_bytes","phase":"embedded_image_decode","actual":5,"max":4,"profile":"constrained"}},"message":"embedded image is too large"}""",
        )

        check(error.resourceDetails == MermanResourceErrorDetails(
            limitId = "max_embedded_image_bytes",
            phase = "embedded_image_decode",
            actual = 5,
            max = 4,
            profile = "constrained",
        ))
    }

    @Test
    fun rejectsCoercedOrIncompatibleRuntimeCatalogHandshakeFields() {
        val canonical = Merman.runtimeCatalogJson()
        for ((expected, replacement) in listOf(
            "\"schema_version\":1" to "\"schema_version\":\"1\"",
            "\"schema_version\":1" to "\"schema_version\":1.0",
            "\"transport_api_version\":1" to "\"transport_api_version\":\"1\"",
            "\"transport_api_version\":1" to "\"transport_api_version\":1.0",
        )) {
            val catalog = canonical.replaceFirst(expected, replacement)
            check(catalog != canonical) {
                "runtime catalog fixture did not contain $expected"
            }
            checkCatalogRejected(catalog)
        }

        checkCatalogRejected(JSONObject(canonical).put("package_version", 1).toString())
        checkCatalogRejected(
            JSONObject(canonical)
                .put("options_schema_versions", JSONArray().put(1))
                .toString(),
        )
        checkCatalogRejected(
            JSONObject(canonical)
                .put("options_schema_versions", JSONArray().put("2"))
                .toString(),
        )

        val badResultSchema = JSONObject(canonical)
        val payloadSchemas = badResultSchema.getJSONArray("payload_schemas")
        for (index in 0 until payloadSchemas.length()) {
            val schema = payloadSchemas.getJSONObject(index)
            if (schema.getString("id") == "binding-result") {
                schema.put("version", 2)
            }
        }
        checkCatalogRejected(badResultSchema.toString())

        val badTextMeasurement = JSONObject(canonical)
        badTextMeasurement
            .getJSONObject("capabilities")
            .getJSONObject("text_measurement")
            .put("protocol_version", 2)
        checkCatalogRejected(badTextMeasurement.toString())
    }

    @Test
    fun acceptsAdditiveRuntimeCatalogFields() {
        val catalog = JSONObject(Merman.runtimeCatalogJson())
        catalog.put("future_catalog_field", JSONObject().put("version", 1))
        catalog.getJSONObject("resources").put("future_resource_field", true)
        catalog.getJSONArray("options_schema_versions").put(99)

        check(Merman.validateRuntimeCatalogPayload(catalog.toString()) == catalog.toString())

        val supportedDiagrams = Merman.metadataJson("supported-diagrams")
        check(supportedDiagrams == Merman.supportedDiagramsJson())
    }

    @Test
    fun emptyIconRegistryIsNormalizedToNoService() {
        val services = MermanEngineServices(
            iconRegistry = MermanIconRegistry.fromPacks(emptyList()),
        )
        MermanEngine(services = services).use { engine ->
            check(engine.parseJson("flowchart TD\nA --> B").contains("flowchart-v2"))
        }
    }

    @Test
    fun rejectsUnpairedUtf16BeforeIconPackDecoding() {
        val invalidPacks = listOf(
            MermanIconPack("{\"prefix\":\"bad\",\"icons\":{},\"future\":\"\uD800\"}"),
            MermanIconPack("{\"prefix\":\"ok\",\"icons\":{}}", "\uDC00"),
        )

        for (pack in invalidPacks) {
            val error = runCatching {
                MermanEngine(
                    services = MermanEngineServices(
                        iconRegistry = MermanIconRegistry.fromPacks(listOf(pack)),
                    ),
                ).use { }
            }.exceptionOrNull()
            check(error is MermanException)
            check(error.iconRegistryDetails?.kindId == "invalid_utf8")
            check(error.iconRegistryDetails?.packIndex == 0L)
        }
    }

    private fun checkCatalogRejected(catalog: String) {
        val error = runCatching {
            Merman.validateRuntimeCatalogPayload(catalog)
        }.exceptionOrNull()
        check(error is MermanException) {
            "malformed runtime catalog was accepted: $catalog"
        }
    }
}
