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
    fun rejectsCoercedRuntimeCatalogVersionFields() {
        val canonical = MermanEngine.runtimeCatalogJson()
        for ((expected, replacement) in listOf(
            "\"schema_version\":1" to "\"schema_version\":\"1\"",
            "\"schema_version\":1" to "\"schema_version\":1.0",
            "\"transport_api_version\":1" to "\"transport_api_version\":\"1\"",
            "\"transport_api_version\":1" to "\"transport_api_version\":1.0",
            "\"protocol_version\":1" to "\"protocol_version\":\"1\"",
            "\"protocol_version\":1" to "\"protocol_version\":1.0",
        )) {
            val catalog = canonical.replaceFirst(expected, replacement)
            check(catalog != canonical) {
                "runtime catalog fixture did not contain $expected"
            }
            checkCatalogRejected(catalog)
        }

        val catalog = JSONObject(canonical).put("package_version", 1)
        checkCatalogRejected(catalog.toString())
    }

    @Test
    fun rejectsSvgRuntimeCatalogWithoutVendoredTextMeasurement() {
        for (providers in listOf(emptyList(), listOf("host-callback"))) {
            val catalog = JSONObject(MermanEngine.runtimeCatalogJson())
            catalog
                .getJSONObject("capabilities")
                .getJSONObject("text_measurement")
                .put("provider_ids", JSONArray(providers))
            checkCatalogRejected(catalog.toString())
        }
    }

    private fun checkCatalogRejected(catalog: String) {
        val error = runCatching {
            MermanEngine.validateRuntimeCatalogPayload(catalog)
        }.exceptionOrNull()
        check(error is MermanException) {
            "malformed runtime catalog was accepted: $catalog"
        }
    }
}
