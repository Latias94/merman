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

    @Test
    fun acceptsAdditiveRuntimeOutputContractFieldsAndNullLimits() {
        val catalog = JSONObject(MermanEngine.runtimeCatalogJson())
        catalog.put("future_catalog_field", true)
        val binaryContract = catalog.outputContract("png")
        val fonts = binaryContract.getJSONObject("system_fonts")
        val images = binaryContract.getJSONObject("embedded_images")
        val limits = images.getJSONObject("limits")
        binaryContract.put("future_output_field", JSONObject().put("version", 1))
        fonts.put("future_font_field", "supported")
        images.put("future_image_field", false)
        limits.put("future_limit_field", 1)
        for (key in listOf(
            "max_bytes_per_image",
            "max_total_bytes",
            "max_pixels_per_image",
            "max_total_pixels",
        )) {
            limits.put(key, JSONObject.NULL)
        }

        check(MermanEngine.validateRuntimeCatalogPayload(catalog.toString()) == catalog.toString())
    }

    @Test
    fun rejectsRuntimeOutputContractIdDrift() {
        val missing = JSONObject(MermanEngine.runtimeCatalogJson())
        missing.remove("output_contracts")
        checkCatalogRejected(missing.toString())

        val mismatched = JSONObject(MermanEngine.runtimeCatalogJson())
        mismatched.outputContract("svg").put("id", "svg-other")
        checkCatalogRejected(mismatched.toString())

        val duplicate = JSONObject(MermanEngine.runtimeCatalogJson())
        duplicate.outputContract("jpeg").put("id", "ascii")
        checkCatalogRejected(duplicate.toString())
    }

    @Test
    fun rejectsMalformedRuntimeOutputContractFields() {
        val badMediaType = JSONObject(MermanEngine.runtimeCatalogJson())
        badMediaType.outputContract("svg").put("media_type", 1)
        checkCatalogRejected(badMediaType.toString())

        val badFonts = JSONObject(MermanEngine.runtimeCatalogJson())
        badFonts.outputContract("png").getJSONObject("system_fonts").put("host_dependent", 1)
        checkCatalogRejected(badFonts.toString())

        val badImages = JSONObject(MermanEngine.runtimeCatalogJson())
        badImages.outputContract("png").getJSONObject("embedded_images")
            .put("source_ids", JSONArray(listOf(1)))
        checkCatalogRejected(badImages.toString())

        for (invalid in listOf(0, -1, 1.5, "1", true)) {
            val badLimit = JSONObject(MermanEngine.runtimeCatalogJson())
            badLimit.outputContract("png")
                .getJSONObject("embedded_images")
                .getJSONObject("limits")
                .put("max_bytes_per_image", invalid)
            checkCatalogRejected(badLimit.toString())
        }
    }

    private fun JSONObject.outputContract(id: String): JSONObject {
        val contracts = getJSONArray("output_contracts")
        for (index in 0 until contracts.length()) {
            val contract = contracts.getJSONObject(index)
            if (contract.getString("id") == id) return contract
        }
        error("runtime catalog fixture did not contain output contract `$id`")
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
