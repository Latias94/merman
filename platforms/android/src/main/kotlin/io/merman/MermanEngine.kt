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
        validateRuntimeCatalogPayload(nativeRuntimeCatalogJson())
    }

    init {
        System.loadLibrary("merman_android_jni")
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

    /** Executes any operation ID exposed by [runtimeCatalogJson]. */
    @JvmStatic
    fun execute(
        operationId: String,
        source: String,
        optionsJson: String? = null,
        uri: String? = null,
    ): MermanOperationResult = nativeExecute(operationId, source, optionsJson, uri)

    @JvmStatic
    fun renderSvg(source: String, optionsJson: String? = null): String =
        executeText("svg", source, optionsJson)

    @JvmStatic
    fun renderAscii(source: String, optionsJson: String? = null): String =
        executeText("ascii", source, optionsJson)

    @JvmStatic
    fun renderPng(source: String, optionsJson: String? = null): ByteArray =
        execute("png", source, optionsJson).data

    @JvmStatic
    fun renderJpeg(source: String, optionsJson: String? = null): ByteArray =
        execute("jpeg", source, optionsJson).data

    @JvmStatic
    fun renderPdf(source: String, optionsJson: String? = null): ByteArray =
        execute("pdf", source, optionsJson).data

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
    ): String = execute(operationId, source, optionsJson, uri).data.toString(Charsets.UTF_8)

    private fun metadataJson(id: String): String = nativeMetadataJson(id)

    internal fun validateRuntimeCatalogPayload(json: String): String {
        try {
            val root = JSONObject(json)
            requireRequiredKeys(
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
            if (
                requiredJsonInt(root, "schema_version", "runtime catalog") !=
                RUNTIME_CONTRACT_SCHEMA_VERSION
            ) {
                throw MermanException("Unsupported Merman runtime contract schema")
            }
            if (
                requiredJsonInt(root, "transport_api_version", "runtime catalog") !=
                TRANSPORT_API_VERSION
            ) {
                throw MermanException("Merman Android transport API version mismatch")
            }
            if (requiredJsonString(root, "package_version", "runtime catalog").isEmpty()) {
                throw MermanException("Merman runtime catalog is missing package_version")
            }

            val capabilities = root.optJSONObject("capabilities")
                ?: throw MermanException("Merman runtime contract is missing capabilities")
            requireRequiredKeys(
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
            requireRequiredKeys(registry, setOf("diagram_family_count"), "runtime registry")
            if (requiredJsonInt(registry, "diagram_family_count", "runtime registry") < 0) {
                throw MermanException("Merman diagram family count is invalid")
            }

            val resources = root.optJSONObject("resources")
                ?: throw MermanException("Merman runtime catalog is missing resources")
            requireRequiredKeys(
                resources,
                setOf(
                    "general_binding_default_profile",
                    "cli_default_profile",
                    "limits",
                    "profiles",
                ),
                "runtime resources",
            )
            if (
                requiredJsonString(
                    resources,
                    "general_binding_default_profile",
                    "runtime resources",
                ).isEmpty() ||
                requiredJsonString(
                    resources,
                    "cli_default_profile",
                    "runtime resources",
                ).isEmpty() ||
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

    private fun requireRequiredKeys(value: JSONObject, expected: Set<String>, label: String) {
        val actual = value.keys().asSequence().toSet()
        val missing = expected - actual
        if (missing.isNotEmpty()) {
            throw MermanException("Merman $label is missing fields: ${missing.sorted()}")
        }
    }

    private fun requiredJsonInt(value: JSONObject, key: String, label: String): Int {
        val raw = value.opt(key)
        val integer = when (raw) {
            is Int -> raw.toLong()
            is Long -> raw
            else -> throw MermanException("Merman $label field `$key` must be a JSON integer")
        }
        if (integer !in Int.MIN_VALUE.toLong()..Int.MAX_VALUE.toLong()) {
            throw MermanException(
                "Merman $label field `$key` is outside the supported integer range",
            )
        }
        return integer.toInt()
    }

    private fun requiredJsonString(value: JSONObject, key: String, label: String): String {
        val raw = value.opt(key)
        if (raw !is String) {
            throw MermanException("Merman $label field `$key` must be a string")
        }
        return raw
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
        requireRequiredKeys(
            value,
            setOf("protocol_version", "provider_ids"),
            "text measurement contract",
        )
        if (
            requiredJsonInt(
                value,
                "protocol_version",
                "text measurement contract",
            ) !=
            MermanTextMeasurementOperation.PROTOCOL_VERSION
        ) {
            throw MermanException("Merman text measurement protocol version mismatch")
        }
        val providerIds = sortedUniqueStrings(
            value.optJSONArray("provider_ids"),
            "runtime text measurement providers",
        )
        if ("vendored" !in providerIds) {
            throw MermanException(
                "Merman runtime text measurement providers must include `vendored`",
            )
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
    ): MermanOperationResult

    @JvmStatic
    private external fun nativeMetadataJson(id: String): String
}
