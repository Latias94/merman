package io.merman

import org.json.JSONObject

/**
 * Stateless Android entry point for the transport-neutral Merman operation catalog.
 *
 * Native methods are registered during `JNI_OnLoad`; no Java-name-derived JNI symbols are part of
 * the public native library contract. The runtime catalog schema and transport API are validated
 * before any operation. Exact release equality is not checked; Kotlin classes and native slices
 * must come from the same AAR.
 */
object MermanEngine {
    const val TRANSPORT_API_VERSION: Int = ANDROID_TRANSPORT_API_VERSION

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

    internal fun validateRuntimeCatalogPayload(json: String): String =
        MermanRuntimeCatalogValidator.validate(json)

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
