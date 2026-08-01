package io.merman

/** Reusable Android engine with an immutable optional host text-measurement callback. */
class MermanReusableEngine(
    optionsJson: String? = null,
    textMeasurer: MermanTextMeasurer? = null,
) : AutoCloseable {
    @Suppress("PLATFORM_CLASS_MAPPED_TO_KOTLIN")
    private val lifecycleLock = Object()
    private var handle: Long = nativeNew(optionsJson, textMeasurer)

    /** Executes any operation ID advertised by [MermanEngine.runtimeCatalogJson]. */
    fun execute(
        operationId: String,
        source: String,
        optionsJson: String? = null,
        uri: String? = null,
    ): MermanOperationResult = withLiveHandle {
        nativeExecute(it, operationId, source, optionsJson, uri)
    }

    fun renderSvg(source: String, optionsJson: String? = null): String =
        executeText("svg", source, optionsJson)

    fun renderAscii(source: String, optionsJson: String? = null): String =
        executeText("ascii", source, optionsJson)

    fun renderPng(source: String, optionsJson: String? = null): ByteArray =
        execute("png", source, optionsJson).data

    fun renderJpeg(source: String, optionsJson: String? = null): ByteArray =
        execute("jpeg", source, optionsJson).data

    fun renderPdf(source: String, optionsJson: String? = null): ByteArray =
        execute("pdf", source, optionsJson).data

    fun parseJson(source: String, optionsJson: String? = null): String =
        executeText("semantic-json", source, optionsJson)

    fun layoutJson(source: String, optionsJson: String? = null): String =
        executeText("layout-json", source, optionsJson)

    fun analyzeJson(source: String, optionsJson: String? = null): String =
        executeText("analysis-json", source, optionsJson)

    fun analyzeDocumentJson(source: String, uri: String, optionsJson: String? = null): String =
        executeText("document-analysis-json", source, optionsJson, uri)

    fun analyzeDocumentFactsJson(source: String, uri: String, optionsJson: String? = null): String =
        executeText("document-analysis-facts-json", source, optionsJson, uri)

    fun validateJson(source: String, optionsJson: String? = null): String =
        executeText("validation-json", source, optionsJson)

    override fun close() {
        synchronized(lifecycleLock) {
            if (handle == 0L) return
            if (nativeTryClose(handle)) {
                handle = 0L
            }
        }
    }

    private fun executeText(
        operationId: String,
        source: String,
        optionsJson: String? = null,
        uri: String? = null,
    ): String = execute(operationId, source, optionsJson, uri).data.toString(Charsets.UTF_8)

    private inline fun <T> withLiveHandle(call: (Long) -> T): T {
        val current = synchronized(lifecycleLock) {
            handle.takeIf { it != 0L }
                ?: throw MermanException("Merman reusable engine is closed")
        }
        return call(current)
    }

    private companion object {
        init {
            MermanEngine.ensureNativeReady()
        }

        @JvmStatic
        private external fun nativeNew(
            optionsJson: String?,
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
    }
}
