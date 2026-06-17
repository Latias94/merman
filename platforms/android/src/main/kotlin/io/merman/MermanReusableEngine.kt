package io.merman

class MermanReusableEngine(optionsJson: String? = null) : AutoCloseable {
    private var handle: Long = nativeNew(optionsJson)

    fun setTextMeasurer(measurer: MermanTextMeasurer?) {
        nativeSetTextMeasurer(requireHandle(), measurer)
    }

    fun renderSvg(source: String): String =
        nativeRenderSvg(requireHandle(), source)

    fun renderAscii(source: String): String =
        nativeRenderAscii(requireHandle(), source)

    fun parseJson(source: String): String =
        nativeParseJson(requireHandle(), source)

    fun layoutJson(source: String): String =
        nativeLayoutJson(requireHandle(), source)

    fun validateJson(source: String): String =
        nativeValidateJson(requireHandle(), source)

    override fun close() {
        val current = handle
        if (current != 0L) {
            handle = 0L
            nativeFree(current)
        }
    }

    private fun requireHandle(): Long {
        val current = handle
        if (current == 0L) {
            throw MermanException("Merman reusable engine is closed")
        }
        return current
    }

    private companion object {
        init {
            System.loadLibrary("merman_ffi")
        }

        @JvmStatic
        private external fun nativeNew(optionsJson: String?): Long

        @JvmStatic
        private external fun nativeFree(handle: Long)

        @JvmStatic
        private external fun nativeSetTextMeasurer(handle: Long, measurer: MermanTextMeasurer?)

        @JvmStatic
        private external fun nativeRenderSvg(handle: Long, source: String): String

        @JvmStatic
        private external fun nativeRenderAscii(handle: Long, source: String): String

        @JvmStatic
        private external fun nativeParseJson(handle: Long, source: String): String

        @JvmStatic
        private external fun nativeLayoutJson(handle: Long, source: String): String

        @JvmStatic
        private external fun nativeValidateJson(handle: Long, source: String): String
    }
}
