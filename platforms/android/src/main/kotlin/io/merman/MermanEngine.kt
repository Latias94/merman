package io.merman

/**
 * Reusable Android engine with immutable constructor-owned services.
 *
 * Calls may run concurrently when [services] has no host callback. A callback-backed engine
 * serializes operation admission and rejects competing or re-entrant calls with typed failures.
 * [close] is idempotent; a busy or re-entrant failure leaves the complete engine intact for retry.
 */
class MermanEngine(
    optionsJson: String? = null,
    services: MermanEngineServices? = null,
) : AutoCloseable {
    @Suppress("PLATFORM_CLASS_MAPPED_TO_KOTLIN")
    private val lifecycleLock = Object()
    private var handle: Long

    init {
        val iconPackSet = services?.iconPackSet
        handle = if (iconPackSet == null) {
            nativeNew(optionsJson, emptyArray(), emptyArray(), services?.textMeasurer)
        } else {
            iconPackSet.withBorrowedPacks { packJson, registrationNames ->
                nativeNew(optionsJson, packJson, registrationNames, services.textMeasurer)
            }
        }
    }

    /** Executes any operation ID advertised by [Merman.runtimeCatalogJson]. */
    fun execute(
        operationId: String,
        source: String,
        optionsJson: String? = null,
        uri: String? = null,
    ): MermanOperationResult = withLiveHandle {
        nativeExecute(it, operationId, source, optionsJson, uri)
    }

    /** Executes an operation with caller-owned cooperative cancellation or a relative deadline. */
    fun execute(
        operationId: String,
        source: String,
        control: MermanOperationControl,
        optionsJson: String? = null,
        uri: String? = null,
    ): MermanOperationResult = withLiveHandle {
        nativeExecuteControlled(
            it,
            operationId,
            source,
            optionsJson,
            uri,
            control.tokenForExecution(),
        )
    }

    fun renderSvg(source: String, optionsJson: String? = null): String =
        executeText(MermanBindingOperationId.SVG, source, optionsJson)

    fun renderAscii(source: String, optionsJson: String? = null): String =
        executeText(MermanBindingOperationId.ASCII, source, optionsJson)

    fun renderPng(source: String, optionsJson: String? = null): ByteArray =
        renderPngResult(source, optionsJson).data

    fun renderPngResult(source: String, optionsJson: String? = null): MermanOperationResult =
        execute(MermanBindingOperationId.PNG, source, optionsJson)

    fun renderJpeg(source: String, optionsJson: String? = null): ByteArray =
        renderJpegResult(source, optionsJson).data

    fun renderJpegResult(source: String, optionsJson: String? = null): MermanOperationResult =
        execute(MermanBindingOperationId.JPEG, source, optionsJson)

    fun renderPdf(source: String, optionsJson: String? = null): ByteArray =
        renderPdfResult(source, optionsJson).data

    fun renderPdfResult(source: String, optionsJson: String? = null): MermanOperationResult =
        execute(MermanBindingOperationId.PDF, source, optionsJson)

    fun parseJson(source: String, optionsJson: String? = null): String =
        executeText(MermanBindingOperationId.SEMANTIC_JSON, source, optionsJson)

    fun layoutJson(source: String, optionsJson: String? = null): String =
        executeText(MermanBindingOperationId.LAYOUT_JSON, source, optionsJson)

    fun analyzeJson(source: String, optionsJson: String? = null): String =
        executeText(MermanBindingOperationId.ANALYSIS_JSON, source, optionsJson)

    fun analysisFactsJson(source: String, optionsJson: String? = null): String =
        executeText(MermanBindingOperationId.ANALYSIS_FACTS_JSON, source, optionsJson)

    fun analyzeDocumentJson(source: String, uri: String, optionsJson: String? = null): String =
        executeText(MermanBindingOperationId.DOCUMENT_ANALYSIS_JSON, source, optionsJson, uri)

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

    fun validateJson(source: String, optionsJson: String? = null): String =
        executeText(MermanBindingOperationId.VALIDATION_JSON, source, optionsJson)

    fun svgPlanJson(source: String, optionsJson: String? = null): String =
        executeText(MermanBindingOperationId.SVG_PLAN_JSON, source, optionsJson)

    override fun close() {
        val current = synchronized(lifecycleLock) {
            handle.takeIf { it != 0L } ?: return
        }
        if (nativeTryClose(current)) {
            synchronized(lifecycleLock) {
                if (handle == current) {
                    handle = 0L
                }
            }
        }
    }

    private fun executeText(
        operationId: String,
        source: String,
        optionsJson: String? = null,
        uri: String? = null,
    ): String = execute(operationId, source, optionsJson, uri).utf8Text

    private inline fun <T> withLiveHandle(call: (Long) -> T): T {
        val current = synchronized(lifecycleLock) {
            handle.takeIf { it != 0L } ?: throw MermanException("Merman engine is closed")
        }
        return call(current)
    }

    private companion object {
        init {
            Merman.ensureNativeReady()
        }

        @JvmStatic
        private external fun nativeNew(
            optionsJson: String?,
            iconPackJson: Array<String>,
            iconPackRegistrationNames: Array<String?>,
            textMeasurer: MermanTextMeasurer?,
        ): Long

        @JvmStatic
        private external fun nativeTryClose(handle: Long): Boolean

        @JvmStatic
        private external fun nativeExecute(
            handle: Long,
            operationId: String,
            source: String,
            optionsJson: String?,
            uri: String?,
        ): MermanOperationResult

        @JvmStatic
        private external fun nativeExecuteControlled(
            handle: Long,
            operationId: String,
            source: String,
            optionsJson: String?,
            uri: String?,
            controlToken: Long,
        ): MermanOperationResult
    }
}
