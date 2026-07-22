package io.merman

import org.json.JSONObject

object MermanEngine {
    const val ABI_VERSION: Int = MermanTextMeasurementOperation.ABI_VERSION
    private const val RUNTIME_CONTRACT_SCHEMA_VERSION: Int = 4
    private val SUPPORTED_SYSTEM_ADAPTER_IDS = setOf(
        "system-clock",
        "system-timezone",
        "system-random",
        "system-timing",
    )

    init {
        System.loadLibrary("merman_ffi")
        checkNativeAbi()
    }

    @JvmStatic
    internal fun ensureNativeReady() = Unit

    val packageVersion: String
        get() = nativePackageVersion()

    private val supportedDiagramsJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        nativeSupportedDiagramsJson()
    }

    private val runtimeContractJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        validateRuntimeContract(nativeRuntimeContractJson())
    }

    private val asciiCapabilitiesJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        nativeAsciiCapabilitiesJson()
    }

    private val diagramFamilyCapabilitiesJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        nativeDiagramFamilyCapabilitiesJson()
    }

    private val lintRuleCatalogJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        nativeLintRuleCatalogJson()
    }

    private val supportedThemesJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        nativeSupportedThemesJson()
    }

    private val supportedHostThemePresetsJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        nativeSupportedHostThemePresetsJson()
    }

    @JvmStatic
    fun renderSvg(source: String, optionsJson: String? = null): String =
        nativeRenderSvg(source, optionsJson)

    @JvmStatic
    fun renderAscii(source: String, optionsJson: String? = null): String =
        nativeRenderAscii(source, optionsJson)

    @JvmStatic
    fun parseJson(source: String, optionsJson: String? = null): String =
        nativeParseJson(source, optionsJson)

    @JvmStatic
    fun layoutJson(source: String, optionsJson: String? = null): String =
        nativeLayoutJson(source, optionsJson)

    @JvmStatic
    fun analyzeJson(source: String, optionsJson: String? = null): String =
        nativeAnalyzeJson(source, optionsJson)

    @JvmStatic
    fun analyzeDocumentJson(source: String, uri: String, optionsJson: String? = null): String =
        nativeAnalyzeDocumentJson(source, optionsJson, uri)

    @JvmStatic
    fun analyzeDocumentFactsJson(source: String, uri: String, optionsJson: String? = null): String =
        nativeAnalyzeDocumentFactsJson(source, optionsJson, uri)

    @JvmStatic
    fun validateJson(source: String, optionsJson: String? = null): String =
        nativeValidateJson(source, optionsJson)

    @JvmStatic
    fun supportedDiagramsJson(): String =
        supportedDiagramsJsonCache

    /** Returns the versioned ABI, feature, registry, and resource contract. */
    @JvmStatic
    fun runtimeContractJson(): String =
        runtimeContractJsonCache

    @JvmStatic
    fun asciiCapabilitiesJson(): String =
        asciiCapabilitiesJsonCache

    @JvmStatic
    fun diagramFamilyCapabilitiesJson(): String =
        diagramFamilyCapabilitiesJsonCache

    @JvmStatic
    fun lintRuleCatalogJson(): String =
        lintRuleCatalogJsonCache

    @JvmStatic
    fun supportedThemesJson(): String =
        supportedThemesJsonCache

    @JvmStatic
    fun supportedHostThemePresetsJson(): String =
        supportedHostThemePresetsJsonCache

    private fun checkNativeAbi() {
        val nativeAbi = nativeAbiVersion()
        if (nativeAbi != ABI_VERSION) {
            throw MermanException("Merman ABI mismatch: expected $ABI_VERSION, got $nativeAbi")
        }
        if (nativeBufferStructSize() <= 0L || nativeResultStructSize() <= 0L) {
            throw MermanException("Merman ABI struct size check failed")
        }
    }

    private fun validateRuntimeContract(json: String): String {
        try {
            val root = JSONObject(json)
            if (root.optInt("schema_version", -1) != RUNTIME_CONTRACT_SCHEMA_VERSION) {
                throw MermanException("Unsupported Merman runtime contract schema")
            }
            val features = root.optJSONObject("features")
                ?: throw MermanException("Merman runtime contract is missing features")
            if (features.has("core_host")) {
                throw MermanException("Merman runtime contract contains removed core_host field")
            }
            val systemAdapterIds = features.optJSONArray("system_adapter_ids")
                ?: throw MermanException(
                    "Merman runtime contract is missing system_adapter_ids",
                )
            val seenSystemAdapterIds = mutableSetOf<String>()
            for (index in 0 until systemAdapterIds.length()) {
                val adapterId = systemAdapterIds.opt(index)
                if (
                    adapterId !is String ||
                    adapterId !in SUPPORTED_SYSTEM_ADAPTER_IDS ||
                    !seenSystemAdapterIds.add(adapterId)
                ) {
                    throw MermanException(
                        "Merman runtime contract contains an unsupported system adapter ID",
                    )
                }
            }
            return json
        } catch (error: MermanException) {
            throw error
        } catch (error: Exception) {
            throw MermanException("Invalid Merman runtime contract: ${error.message}")
        }
    }

    @JvmStatic
    private external fun nativeAbiVersion(): Int

    @JvmStatic
    private external fun nativePackageVersion(): String

    @JvmStatic
    private external fun nativeBufferStructSize(): Long

    @JvmStatic
    private external fun nativeResultStructSize(): Long

    @JvmStatic
    private external fun nativeRenderSvg(source: String, optionsJson: String?): String

    @JvmStatic
    private external fun nativeRenderAscii(source: String, optionsJson: String?): String

    @JvmStatic
    private external fun nativeParseJson(source: String, optionsJson: String?): String

    @JvmStatic
    private external fun nativeLayoutJson(source: String, optionsJson: String?): String

    @JvmStatic
    private external fun nativeAnalyzeJson(source: String, optionsJson: String?): String

    @JvmStatic
    private external fun nativeAnalyzeDocumentJson(
        source: String,
        optionsJson: String?,
        uri: String,
    ): String

    @JvmStatic
    private external fun nativeAnalyzeDocumentFactsJson(
        source: String,
        optionsJson: String?,
        uri: String,
    ): String

    @JvmStatic
    private external fun nativeValidateJson(source: String, optionsJson: String?): String

    @JvmStatic
    private external fun nativeSupportedDiagramsJson(): String

    @JvmStatic
    private external fun nativeRuntimeContractJson(): String

    @JvmStatic
    private external fun nativeAsciiCapabilitiesJson(): String

    @JvmStatic
    private external fun nativeDiagramFamilyCapabilitiesJson(): String

    @JvmStatic
    private external fun nativeLintRuleCatalogJson(): String

    @JvmStatic
    private external fun nativeSupportedThemesJson(): String

    @JvmStatic
    private external fun nativeSupportedHostThemePresetsJson(): String
}
