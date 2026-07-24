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
    fun executeBytes(operationId: String, source: String, uri: String? = null): ByteArray =
        withLiveHandle { nativeExecute(it, operationId, source, uri) }

    fun renderSvg(source: String): String = executeText("svg", source)

    fun renderAscii(source: String): String = executeText("ascii", source)

    fun renderPng(source: String): ByteArray = executeBytes("png", source)

    fun renderJpeg(source: String): ByteArray = executeBytes("jpeg", source)

    fun renderPdf(source: String): ByteArray = executeBytes("pdf", source)

    fun parseJson(source: String): String = executeText("semantic-json", source)

    fun layoutJson(source: String): String = executeText("layout-json", source)

    fun analyzeJson(source: String): String = executeText("analysis-json", source)

    fun analyzeDocumentJson(source: String, uri: String): String =
        executeText("document-analysis-json", source, uri)

    fun analyzeDocumentFactsJson(source: String, uri: String): String =
        executeText("document-analysis-facts-json", source, uri)

    fun validateJson(source: String): String = executeText("validation-json", source)

    override fun close() {
        synchronized(lifecycleLock) {
            if (handle == 0L) return
            nativeFree(handle)
            handle = 0L
        }
    }

    private fun executeText(operationId: String, source: String, uri: String? = null): String =
        executeBytes(operationId, source, uri).toString(Charsets.UTF_8)

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
            uri: String?,
        ): ByteArray
    }
}
