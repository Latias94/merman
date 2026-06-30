package io.merman

object MermanEngine {
    const val ABI_VERSION: Int = 2

    init {
        System.loadLibrary("merman_ffi")
        checkNativeAbi()
    }

    val packageVersion: String
        get() = nativePackageVersion()

    private val supportedDiagramsJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        nativeSupportedDiagramsJson()
    }

    private val asciiCapabilitiesJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        nativeAsciiCapabilitiesJson()
    }

    private val diagramFamilyCapabilitiesJsonCache: String by lazy(LazyThreadSafetyMode.PUBLICATION) {
        nativeDiagramFamilyCapabilitiesJson()
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
    fun validateJson(source: String, optionsJson: String? = null): String =
        nativeValidateJson(source, optionsJson)

    @JvmStatic
    fun supportedDiagramsJson(): String =
        supportedDiagramsJsonCache

    @JvmStatic
    fun asciiCapabilitiesJson(): String =
        asciiCapabilitiesJsonCache

    @JvmStatic
    fun diagramFamilyCapabilitiesJson(): String =
        diagramFamilyCapabilitiesJsonCache

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
    private external fun nativeAsciiCapabilitiesJson(): String

    @JvmStatic
    private external fun nativeDiagramFamilyCapabilitiesJson(): String

    @JvmStatic
    private external fun nativeSupportedThemesJson(): String

    @JvmStatic
    private external fun nativeSupportedHostThemePresetsJson(): String
}
