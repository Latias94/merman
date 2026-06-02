package io.merman

object MermanEngine {
    const val ABI_VERSION: Int = 2

    init {
        System.loadLibrary("merman_ffi")
        checkNativeAbi()
    }

    val packageVersion: String
        get() = nativePackageVersion()

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
    fun validateJson(source: String, optionsJson: String? = null): String =
        nativeValidateJson(source, optionsJson)

    @JvmStatic
    fun supportedDiagramsJson(): String =
        nativeSupportedDiagramsJson()

    @JvmStatic
    fun asciiSupportedDiagramsJson(): String =
        nativeAsciiSupportedDiagramsJson()

    @JvmStatic
    fun themesJson(): String =
        nativeThemesJson()

    private fun checkNativeAbi() {
        val nativeAbi = nativeAbiVersion()
        if (nativeAbi != ABI_VERSION) {
            throw MermanException("Merman ABI mismatch: expected $ABI_VERSION, got $nativeAbi")
        }
        if (nativeBufferStructSize() <= 0L || nativeResultStructSize() <= 0L) {
            throw MermanException("Merman ABI struct size check failed")
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
    private external fun nativeValidateJson(source: String, optionsJson: String?): String

    @JvmStatic
    private external fun nativeSupportedDiagramsJson(): String

    @JvmStatic
    private external fun nativeAsciiSupportedDiagramsJson(): String

    @JvmStatic
    private external fun nativeThemesJson(): String
}
