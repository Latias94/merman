package io.merman

import org.json.JSONObject

/**
 * Stateless Android entry point for discovery and one-shot operations.
 *
 * Native methods are registered during `JNI_OnLoad`; no Java-name-derived JNI symbols are part of
 * the public native library contract. The package validates the runtime catalog before use so the
 * Kotlin and native slices must come from the same AAR.
 */
object Merman {
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
        get() = JSONObject(runtimeCatalogJson()).getString("package_version")

    private val supportedDiagramsJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        metadataJson(MermanBindingMetadataId.SUPPORTED_DIAGRAMS)
    }

    private val asciiCapabilitiesJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        metadataJson(MermanBindingMetadataId.ASCII_CAPABILITIES)
    }

    private val diagramFamilyCapabilitiesJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        metadataJson(MermanBindingMetadataId.DIAGRAM_FAMILY_CAPABILITIES)
    }

    private val lintRuleCatalogJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        metadataJson(MermanBindingMetadataId.LINT_RULE_CATALOG)
    }

    private val supportedThemesJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        metadataJson(MermanBindingMetadataId.SUPPORTED_THEMES)
    }

    private val presentationCatalogJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        metadataJson(MermanBindingMetadataId.PRESENTATION_CATALOG)
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
        executeText(MermanBindingOperationId.SVG, source, optionsJson)

    @JvmStatic
    fun renderAscii(source: String, optionsJson: String? = null): String =
        executeText(MermanBindingOperationId.ASCII, source, optionsJson)

    @JvmStatic
    fun renderPng(source: String, optionsJson: String? = null): ByteArray =
        renderPngResult(source, optionsJson).data

    @JvmStatic
    fun renderPngResult(source: String, optionsJson: String? = null): MermanOperationResult =
        execute(MermanBindingOperationId.PNG, source, optionsJson)

    @JvmStatic
    fun renderJpeg(source: String, optionsJson: String? = null): ByteArray =
        renderJpegResult(source, optionsJson).data

    @JvmStatic
    fun renderJpegResult(source: String, optionsJson: String? = null): MermanOperationResult =
        execute(MermanBindingOperationId.JPEG, source, optionsJson)

    @JvmStatic
    fun renderPdf(source: String, optionsJson: String? = null): ByteArray =
        renderPdfResult(source, optionsJson).data

    @JvmStatic
    fun renderPdfResult(source: String, optionsJson: String? = null): MermanOperationResult =
        execute(MermanBindingOperationId.PDF, source, optionsJson)

    @JvmStatic
    fun parseJson(source: String, optionsJson: String? = null): String =
        executeText(MermanBindingOperationId.SEMANTIC_JSON, source, optionsJson)

    @JvmStatic
    fun layoutJson(source: String, optionsJson: String? = null): String =
        executeText(MermanBindingOperationId.LAYOUT_JSON, source, optionsJson)

    @JvmStatic
    fun analyzeJson(source: String, optionsJson: String? = null): String =
        executeText(MermanBindingOperationId.ANALYSIS_JSON, source, optionsJson)

    @JvmStatic
    fun analysisFactsJson(source: String, optionsJson: String? = null): String =
        executeText(MermanBindingOperationId.ANALYSIS_FACTS_JSON, source, optionsJson)

    @JvmStatic
    fun analyzeDocumentJson(source: String, uri: String, optionsJson: String? = null): String =
        executeText(MermanBindingOperationId.DOCUMENT_ANALYSIS_JSON, source, optionsJson, uri)

    @JvmStatic
    fun analyzeDocumentFactsJson(
        source: String,
        uri: String,
        optionsJson: String? = null,
    ): String = executeText(
        MermanBindingOperationId.DOCUMENT_ANALYSIS_FACTS_JSON,
        source,
        optionsJson,
        uri,
    )

    @JvmStatic
    fun validateJson(source: String, optionsJson: String? = null): String =
        executeText(MermanBindingOperationId.VALIDATION_JSON, source, optionsJson)

    @JvmStatic
    fun svgPlanJson(source: String, optionsJson: String? = null): String =
        executeText(MermanBindingOperationId.SVG_PLAN_JSON, source, optionsJson)

    @JvmStatic
    fun supportedDiagramsJson(): String = supportedDiagramsJsonCache

    /** Returns the capability, registry, resource, and constructor-service facts for this AAR. */
    @JvmStatic
    fun runtimeCatalogJson(): String = runtimeCatalogJsonCache

    /** Returns any metadata collection ID advertised by [runtimeCatalogJson]. */
    @JvmStatic
    fun metadataJson(id: String): String = nativeMetadataJson(id)

    @JvmStatic
    fun asciiCapabilitiesJson(): String = asciiCapabilitiesJsonCache

    @JvmStatic
    fun diagramFamilyCapabilitiesJson(): String = diagramFamilyCapabilitiesJsonCache

    @JvmStatic
    fun lintRuleCatalogJson(): String = lintRuleCatalogJsonCache

    @JvmStatic
    fun supportedThemesJson(): String = supportedThemesJsonCache

    @JvmStatic
    fun presentationCatalogJson(): String = presentationCatalogJsonCache

    private fun executeText(
        operationId: String,
        source: String,
        optionsJson: String? = null,
        uri: String? = null,
    ): String = execute(operationId, source, optionsJson, uri).utf8Text

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
