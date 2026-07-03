package io.merman

class MermanReusableEngine(optionsJson: String? = null) : AutoCloseable {
    private val lifecycleLock = Object()
    private var handle: Long = nativeNew(optionsJson)
    private var activeNativeThread: Thread? = null
    private var closeRequested = false

    fun setTextMeasurer(measurer: MermanTextMeasurer?) {
        withLiveHandle { nativeSetTextMeasurer(it, measurer) }
    }

    fun renderSvg(source: String): String =
        withLiveHandle { nativeRenderSvg(it, source) }

    fun renderAscii(source: String): String =
        withLiveHandle { nativeRenderAscii(it, source) }

    fun parseJson(source: String): String =
        withLiveHandle { nativeParseJson(it, source) }

    fun layoutJson(source: String): String =
        withLiveHandle { nativeLayoutJson(it, source) }

    fun analyzeJson(source: String): String =
        withLiveHandle { nativeAnalyzeJson(it, source) }

    fun validateJson(source: String): String =
        withLiveHandle { nativeValidateJson(it, source) }

    override fun close() {
        val handleToFree = synchronized(lifecycleLock) {
            if (handle == 0L) {
                0L
            } else {
                closeRequested = true
                val currentThread = Thread.currentThread()
                while (activeNativeThread != null && activeNativeThread !== currentThread) {
                    waitForActiveCallToFinish()
                }
                if (activeNativeThread === currentThread) {
                    0L
                } else {
                    takeHandleForClose()
                }
            }
        }
        freeHandle(handleToFree)
    }

    private inline fun <T> withLiveHandle(call: (Long) -> T): T {
        val current = beginNativeCall()
        try {
            return call(current)
        } finally {
            finishNativeCall()
        }
    }

    private fun beginNativeCall(): Long =
        synchronized(lifecycleLock) {
            val currentThread = Thread.currentThread()
            while (activeNativeThread != null && activeNativeThread !== currentThread) {
                waitForActiveCallToFinish()
            }
            if (activeNativeThread === currentThread) {
                throw MermanException(
                    "Merman reusable engine cannot be re-entered from a native callback"
                )
            }
            if (handle == 0L || closeRequested) {
                throw MermanException("Merman reusable engine is closed")
            }
            activeNativeThread = currentThread
            handle
        }

    private fun finishNativeCall() {
        val handleToFree = synchronized(lifecycleLock) {
            activeNativeThread = null
            val current = if (closeRequested) takeHandleForClose() else 0L
            lifecycleLock.notifyAll()
            current
        }
        freeHandle(handleToFree)
    }

    private fun waitForActiveCallToFinish() {
        try {
            lifecycleLock.wait()
        } catch (error: InterruptedException) {
            Thread.currentThread().interrupt()
            throw MermanException("Interrupted while waiting for Merman reusable engine")
        }
    }

    private fun takeHandleForClose(): Long {
        val current = handle
        handle = 0L
        return current
    }

    private fun freeHandle(handle: Long) {
        if (handle != 0L) {
            nativeFree(handle)
        }
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
        private external fun nativeAnalyzeJson(handle: Long, source: String): String

        @JvmStatic
        private external fun nativeValidateJson(handle: Long, source: String): String
    }
}
