package io.merman

import org.json.JSONArray
import org.json.JSONObject

/**
 * Stateless Android entry point for the transport-neutral Merman operation catalog.
 *
 * Native methods are registered during `JNI_OnLoad`; no Java-name-derived JNI symbols are part of
 * the public native library contract. The runtime catalog is validated before any operation so a
 * mismatched AAR/native slice fails at the boundary rather than during a render.
 */
object MermanEngine {
    const val TRANSPORT_API_VERSION: Int = 1
    private const val RUNTIME_CONTRACT_SCHEMA_VERSION: Int = 1

    private val runtimeCatalogJsonCache: String by lazy(LazyThreadSafetyMode.SYNCHRONIZED) {
        validateRuntimeCatalog(nativeRuntimeCatalogJson())
    }

    init {
        System.loadLibrary("merman_ffi")
        runtimeCatalogJsonCache
    }

    @JvmStatic
    internal fun ensureNativeReady() = Unit

    val packageVersion: String
        get() = JSONObject(runtimeCatalogJson())
            .getString("package_version")

    private val supportedDiagramsJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        metadataJson("supported-diagrams")
    }

    private val asciiCapabilitiesJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        metadataJson("ascii-capabilities")
    }

    private val diagramFamilyCapabilitiesJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        metadataJson("diagram-family-capabilities")
    }

    private val lintRuleCatalogJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        metadataJson("lint-rule-catalog")
    }

    private val supportedThemesJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        metadataJson("supported-themes")
    }

    private val supportedHostThemePresetsJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        metadataJson("supported-host-theme-presets")
    }

    /** Executes any operation ID exposed by [runtimeCatalogJson] and returns its original bytes. */
    @JvmStatic
    fun executeBytes(
        operationId: String,
        source: String,
        optionsJson: String? = null,
        uri: String? = null,
    ): ByteArray = nativeExecute(operationId, source, optionsJson, uri)

    @JvmStatic
    fun renderSvg(source: String, optionsJson: String? = null): String =
        executeText("svg", source, optionsJson)

    @JvmStatic
    fun renderAscii(source: String, optionsJson: String? = null): String =
        executeText("ascii", source, optionsJson)

    @JvmStatic
    fun renderPng(source: String, optionsJson: String? = null): ByteArray =
        executeBytes("png", source, optionsJson)

    @JvmStatic
    fun renderJpeg(source: String, optionsJson: String? = null): ByteArray =
        executeBytes("jpeg", source, optionsJson)

    @JvmStatic
    fun renderPdf(source: String, optionsJson: String? = null): ByteArray =
        executeBytes("pdf", source, optionsJson)

    @JvmStatic
    fun parseJson(source: String, optionsJson: String? = null): String =
        executeText("semantic-json", source, optionsJson)

    @JvmStatic
    fun layoutJson(source: String, optionsJson: String? = null): String =
        executeText("layout-json", source, optionsJson)

    @JvmStatic
    fun analyzeJson(source: String, optionsJson: String? = null): String =
        executeText("analysis-json", source, optionsJson)

    @JvmStatic
    fun analyzeDocumentJson(source: String, uri: String, optionsJson: String? = null): String =
        executeText("document-analysis-json", source, optionsJson, uri)

    @JvmStatic
    fun analyzeDocumentFactsJson(source: String, uri: String, optionsJson: String? = null): String =
        executeText("document-analysis-facts-json", source, optionsJson, uri)

    @JvmStatic
    fun validateJson(source: String, optionsJson: String? = null): String =
        executeText("validation-json", source, optionsJson)

    @JvmStatic
    fun supportedDiagramsJson(): String = supportedDiagramsJsonCache

    /** Returns the capability, registry, and resource facts for this native artifact. */
    @JvmStatic
    fun runtimeCatalogJson(): String = runtimeCatalogJsonCache

    @JvmStatic
    fun asciiCapabilitiesJson(): String = asciiCapabilitiesJsonCache

    @JvmStatic
    fun diagramFamilyCapabilitiesJson(): String = diagramFamilyCapabilitiesJsonCache

    @JvmStatic
    fun lintRuleCatalogJson(): String = lintRuleCatalogJsonCache

    @JvmStatic
    fun supportedThemesJson(): String = supportedThemesJsonCache

    @JvmStatic
    fun supportedHostThemePresetsJson(): String = supportedHostThemePresetsJsonCache

    private fun executeText(
        operationId: String,
        source: String,
        optionsJson: String? = null,
        uri: String? = null,
    ): String = executeBytes(operationId, source, optionsJson, uri).toString(Charsets.UTF_8)

    private fun metadataJson(id: String): String = nativeMetadataJson(id)

    private fun validateRuntimeCatalog(json: String): String {
        try {
            val root = JSONObject(json)
            requireExactKeys(
                root,
                setOf(
                    "schema_version",
                    "transport_api_version",
                    "package_version",
                    "capabilities",
                    "registry",
                    "resources",
                ),
                "runtime catalog",
            )
            if (root.optInt("schema_version", -1) != RUNTIME_CONTRACT_SCHEMA_VERSION) {
                throw MermanException("Unsupported Merman runtime contract schema")
            }
            if (root.optInt("transport_api_version", -1) != TRANSPORT_API_VERSION) {
                throw MermanException("Merman Android transport API version mismatch")
            }
            if (root.optString("package_version").isEmpty()) {
                throw MermanException("Merman runtime catalog is missing package_version")
            }

            val capabilities = root.optJSONObject("capabilities")
                ?: throw MermanException("Merman runtime contract is missing capabilities")
            requireExactKeys(
                capabilities,
                setOf(
                    "capability_ids",
                    "operation_ids",
                    "output_ids",
                    "system_adapter_ids",
                    "text_measurement",
                ),
                "runtime capabilities",
            )
            val capabilityIds = sortedUniqueStrings(
                capabilities.optJSONArray("capability_ids"),
                "runtime capability IDs",
            )
            val outputIds = sortedUniqueStrings(
                capabilities.optJSONArray("output_ids"),
                "runtime output IDs",
            )
            val operationIds = sortedUniqueStrings(
                capabilities.optJSONArray("operation_ids"),
                "runtime operation IDs",
            )
            if (!operationIds.containsAll(outputIds)) {
                throw MermanException(
                    "Merman runtime outputs must also be callable operations",
                )
            }
            val adapterIds = sortedUniqueStrings(
                capabilities.optJSONArray("system_adapter_ids"),
                "runtime system adapter IDs",
            )
            if (!capabilityIds.containsAll(adapterIds)) {
                throw MermanException(
                    "Merman runtime adapter IDs must also be capability IDs",
                )
            }
            validateTextMeasurement(capabilities.opt("text_measurement"), "svg" in capabilityIds)

            val registry = root.optJSONObject("registry")
                ?: throw MermanException("Merman runtime catalog is missing registry")
            requireExactKeys(registry, setOf("diagram_family_count"), "runtime registry")
            if (registry.optInt("diagram_family_count", -1) < 0) {
                throw MermanException("Merman diagram family count is invalid")
            }

            val resources = root.optJSONObject("resources")
                ?: throw MermanException("Merman runtime catalog is missing resources")
            requireExactKeys(
                resources,
                setOf(
                    "schema_version",
                    "general_binding_default_profile",
                    "cli_default_profile",
                    "limits",
                    "profiles",
                ),
                "runtime resources",
            )
            if (
                resources.optInt("schema_version", -1) < 1 ||
                resources.optString("general_binding_default_profile").isEmpty() ||
                resources.optString("cli_default_profile").isEmpty() ||
                resources.optJSONArray("limits") == null ||
                resources.optJSONArray("profiles") == null
            ) {
                throw MermanException("Merman runtime resource contract is invalid")
            }
            return json
        } catch (error: MermanException) {
            throw error
        } catch (error: Exception) {
            throw MermanException("Invalid Merman runtime catalog: ${error.message}")
        }
    }

    private fun sortedUniqueStrings(values: JSONArray?, label: String): Set<String> {
        values ?: throw MermanException("Merman $label is missing")
        val result = linkedSetOf<String>()
        var previous: String? = null
        for (index in 0 until values.length()) {
            val value = values.opt(index)
            if (value !is String || value.isEmpty()) {
                throw MermanException("Merman $label contains an invalid ID")
            }
            if (previous != null && previous >= value) {
                throw MermanException("Merman $label must be sorted and unique")
            }
            result += value
            previous = value
        }
        return result
    }

    private fun requireExactKeys(value: JSONObject, expected: Set<String>, label: String) {
        val actual = value.keys().asSequence().toSet()
        val missing = expected - actual
        val extra = actual - expected
        if (missing.isNotEmpty()) {
            throw MermanException("Merman $label is missing fields: ${missing.sorted()}")
        }
        if (extra.isNotEmpty()) {
            throw MermanException("Merman $label contains unknown fields: ${extra.sorted()}")
        }
    }

    private fun validateTextMeasurement(
        value: Any?,
        hasSvg: Boolean,
    ) {
        if (value == JSONObject.NULL) {
            if (hasSvg) {
                throw MermanException("Merman SVG capability requires text measurement")
            }
            return
        }
        if (!hasSvg || value !is JSONObject) {
            throw MermanException("Merman text measurement requires the SVG capability")
        }
        requireExactKeys(
            value,
            setOf("protocol_version", "provider_ids"),
            "text measurement contract",
        )
        if (
            value.optInt("protocol_version", -1) !=
            MermanTextMeasurementOperation.PROTOCOL_VERSION
        ) {
            throw MermanException("Merman text measurement protocol version mismatch")
        }
        val providerIds = sortedUniqueStrings(
            value.optJSONArray("provider_ids"),
            "runtime text measurement providers",
        )
        if (providerIds.isEmpty()) {
            throw MermanException("Merman runtime has no text measurement provider")
        }
    }

    @JvmStatic
    private external fun nativeRuntimeCatalogJson(): String

    @JvmStatic
    private external fun nativeExecute(
        operationId: String,
        source: String,
        optionsJson: String?,
        uri: String?,
    ): ByteArray

    @JvmStatic
    private external fun nativeMetadataJson(id: String): String
}
