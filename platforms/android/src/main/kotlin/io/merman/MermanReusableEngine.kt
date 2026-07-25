package io.merman

/** Reusable Android engine with an optional host text-measurement callback. */
class MermanReusableEngine(optionsJson: String? = null) : AutoCloseable {
    @Suppress("PLATFORM_CLASS_MAPPED_TO_KOTLIN")
    private val lifecycleLock = Object()
    private var handle: Long = nativeNew(optionsJson)

    fun setTextMeasurer(measurer: MermanTextMeasurer?) {
        withLiveHandle { nativeSetTextMeasurer(it, measurer) }
    }

    /** Executes any operation ID advertised by [MermanEngine.runtimeCatalogJson]. */
    fun executeBytes(
        operationId: String,
        source: String,
        optionsJson: String? = null,
        uri: String? = null,
    ): ByteArray = withLiveHandle {
        nativeExecute(it, operationId, source, optionsJson, uri)
    }

    fun renderSvg(source: String, optionsJson: String? = null): String =
        executeText("svg", source, optionsJson)

    fun renderAscii(source: String, optionsJson: String? = null): String =
        executeText("ascii", source, optionsJson)

    fun renderPng(source: String, optionsJson: String? = null): ByteArray =
        executeBytes("png", source, optionsJson)

    fun renderJpeg(source: String, optionsJson: String? = null): ByteArray =
        executeBytes("jpeg", source, optionsJson)

    fun renderPdf(source: String, optionsJson: String? = null): ByteArray =
        executeBytes("pdf", source, optionsJson)

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
            nativeFree(handle)
            handle = 0L
        }
    }

    private fun executeText(
        operationId: String,
        source: String,
        optionsJson: String? = null,
        uri: String? = null,
    ): String = executeBytes(operationId, source, optionsJson, uri).toString(Charsets.UTF_8)

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
        private external fun nativeNew(optionsJson: String?): Long

        @JvmStatic
        private external fun nativeFree(handle: Long)

        @JvmStatic
        private external fun nativeSetTextMeasurer(handle: Long, measurer: MermanTextMeasurer?)

        @JvmStatic
        private external fun nativeExecute(
            handle: Long,
            operationId: String,
            source: String,
            optionsJson: String?,
            uri: String?,
        ): ByteArray
    }
}
