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
        for ((path, value) in listOf(
            listOf("schema_version") to "1",
            listOf("schema_version") to 1.0,
            listOf("transport_api_version") to "1",
            listOf("transport_api_version") to 1.0,
            listOf("package_version") to 1,
            listOf("capabilities", "text_measurement", "protocol_version") to "1",
            listOf("capabilities", "text_measurement", "protocol_version") to 1.0,
        )) {
            val catalog = JSONObject(MermanEngine.runtimeCatalogJson())
            putNested(catalog, path, value)
            checkCatalogRejected(catalog)
        }
    }

    @Test
    fun rejectsSvgRuntimeCatalogWithoutVendoredTextMeasurement() {
        for (providers in listOf(emptyList(), listOf("host-callback"))) {
            val catalog = JSONObject(MermanEngine.runtimeCatalogJson())
            catalog
                .getJSONObject("capabilities")
                .getJSONObject("text_measurement")
                .put("provider_ids", JSONArray(providers))
            checkCatalogRejected(catalog)
        }
    }

    private fun checkCatalogRejected(catalog: JSONObject) {
        val error = runCatching {
            MermanEngine.validateRuntimeCatalogPayload(catalog.toString())
        }.exceptionOrNull()
        check(error is MermanException) {
            "malformed runtime catalog was accepted: $catalog"
        }
    }

    private fun putNested(root: JSONObject, path: List<String>, value: Any) {
        var target = root
        for (key in path.dropLast(1)) {
            target = target.getJSONObject(key)
        }
        target.put(path.last(), value)
    }
}
